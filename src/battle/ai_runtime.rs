//! AI 异步搜索运行时：在 Bevy 异步计算线程池上跑 MO-ISMCTS，
//! 把结果以类型化 `AiDecision` 经 `RuleDecisionQueue` 回流到主线程。
//!
//! 安全约束：后台任务闭包只捕获 `AiObservation`（脱敏观察）、`DecisionContext`、
//! `MctsConfig`、`SearchControl` 与 seed——绝不捕获 `GameState` / `BattleModel` /
//! `Commands` / ECS 查询 / 事件队列。后台 panic 被 `catch_unwind` 兜底，退化为
//! 合法保底决策，不会传导到 Bevy 主循环。
//!
//! 请求令牌（`AiRequestToken`）记录 match_id / state_version / player / context /
//! request_id 五元组；提交时逐一比对当前状态，过期结果直接丢弃并重新发起。

use std::panic::AssertUnwindSafe;

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, poll_once};

use crate::ai::{
    AiDecision, AiError, AiObservation, AiSearchResult, DecisionContext, DecisionContextKind,
    MctsConfig, SearchControl, determinize, fallback_decision, search,
};
use crate::rules::{PlayerId, Resume};

use super::{BattleModel, BattlePhase, BattleRevision, PendingEvents, RuleDecisionQueue};

/// 单个玩家的控制器：人类或带配置的计算机。
#[derive(Clone)]
pub(super) enum PlayerController {
    Human,
    Computer(MctsConfig),
}

/// 双方控制器配置。当前固定 2 人局：人类 vs CPU。
#[derive(Resource, Clone)]
pub(super) struct PlayerControllers([PlayerController; 2]);

impl PlayerControllers {
    pub(super) fn human_vs_cpu() -> Self {
        Self([
            PlayerController::Human,
            PlayerController::Computer(MctsConfig::normal()),
        ])
    }

    /// `player` 是否由人类操控。
    pub(super) fn is_human(&self, player: PlayerId) -> bool {
        matches!(self.0[player], PlayerController::Human)
    }

    /// 取 `player` 的 MCTS 配置；非 CPU 返回 None。
    fn config(&self, player: PlayerId) -> Option<&MctsConfig> {
        match &self.0[player] {
            PlayerController::Computer(cfg) => Some(cfg),
            PlayerController::Human => None,
        }
    }
}

/// 唯一标识一次 AI 请求的五元组。提交时与当前状态逐字段比对。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AiRequestToken {
    pub match_id: u64,
    pub state_version: u64,
    pub player: PlayerId,
    pub context_kind: DecisionContextKind,
    pub request_id: u64,
}

/// 判定令牌是否与当前 (match, version, player, context, request) 完全匹配。
fn token_matches(
    token: &AiRequestToken,
    match_id: u64,
    state_version: u64,
    player: PlayerId,
    context_kind: DecisionContextKind,
    request_id: u64,
) -> bool {
    token.match_id == match_id
        && token.state_version == state_version
        && token.player == player
        && token.context_kind == context_kind
        && token.request_id == request_id
}

/// 后台任务失败原因。
#[derive(Clone, Debug)]
enum AiTaskFailure {
    Search(AiError),
    Panicked,
}

/// 后台任务最终产出。成功走 `Completed`；失败但有合法保底走 `Fallback`；
/// 取消走 `Cancelled`（即便保底也构造不出时）。
enum AiTaskOutcome {
    Completed(AiSearchResult),
    Fallback {
        decision: AiDecision,
        seed: u64,
        reason: AiTaskFailure,
    },
    Cancelled,
}

/// 一次进行中的后台搜索。
pub(super) struct ActiveSearch {
    token: AiRequestToken,
    control: SearchControl,
    task: Task<AiTaskOutcome>,
}

/// AI 运行时资源：单活跃搜索 + 单待提交结果。
#[derive(Resource)]
pub(super) struct AiRuntime {
    pub(super) match_id: u64,
    next_request_id: u64,
    /// 单调递增的"决策序号"，用作 seed 混入源，避免同一 (match,player) 复用 seed。
    decision_index: u64,
    pub(super) active: Option<ActiveSearch>,
    /// 后台已完成但尚未安全提交（动画/事件未排空）的结果。
    pub(super) ready: Option<(AiRequestToken, AiSearchResult)>,
}

impl AiRuntime {
    pub(super) fn new(match_id: u64) -> Self {
        Self {
            match_id,
            next_request_id: 1,
            decision_index: 0,
            active: None,
            ready: None,
        }
    }

    /// 取消并丢弃任何进行中的后台搜索（用于 Battle 清理）。
    pub(super) fn cancel_active(&mut self) {
        if let Some(active) = self.active.take() {
            active.control.cancel();
        }
        self.ready = None;
    }

    /// 把后台任务产出收纳为 ready（仅当 token 仍有效）。
    fn accept_task_outcome(&mut self, token: AiRequestToken, outcome: AiTaskOutcome) {
        match outcome {
            AiTaskOutcome::Completed(result) => {
                self.ready = Some((token, result));
            }
            AiTaskOutcome::Fallback { decision, seed, .. } => {
                // 保底决策直接构造成一个"伪搜索结果"提交，保留 seed 供指标。
                self.ready = Some((
                    token,
                    AiSearchResult {
                        decision,
                        seed,
                        metrics: crate::ai::AiSearchMetrics {
                            elapsed: std::time::Duration::ZERO,
                            iterations: 0,
                            nodes: 0,
                            used_fallback: true,
                            root_actions: Vec::new(),
                        },
                    },
                ));
            }
            AiTaskOutcome::Cancelled => {
                // 取消的结果不收纳；调用方会重新发起。
            }
        }
    }
}

impl Default for AiRuntime {
    fn default() -> Self {
        Self::new(0)
    }
}

/// 当前 BattlePhase + PendingPhase 推导出的待决断上下文。
/// None 表示当前没有需要 AI 决策的挂起点（Idle 或 GameOver）。
fn current_decision_context(
    phase: &BattlePhase,
    pending: Option<&BattlePhase>,
) -> Option<DecisionContext> {
    // PendingPhase 优先：动画/事件未排空时，下一待决断点在 pending 里。
    if let Some(p) = pending {
        return phase_to_context(p);
    }
    phase_to_context(phase)
}

fn phase_to_context(phase: &BattlePhase) -> Option<DecisionContext> {
    match phase {
        BattlePhase::Idle => Some(DecisionContext::MainTurn),
        BattlePhase::AwaitDiscard { excess } => Some(DecisionContext::Discard { excess: *excess }),
        BattlePhase::AwaitNobleChoice { candidates } => Some(DecisionContext::ChooseNoble {
            candidates: candidates.clone(),
        }),
        BattlePhase::GameOver { .. } => None,
    }
}

/// 从 (match_id, decision_index, player) 混合出 seed（wrapping，可复现）。
fn seed_for(match_id: u64, decision_index: u64, player: PlayerId) -> u64 {
    match_id
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(decision_index.wrapping_mul(0xC2B2_AE3D_27D4_EB4F))
        .wrapping_add(player as u64)
        ^ 0x9E37_79B9_7F4A_7C15
}

/// 在保底路径上构造一个合法决策（后台搜索失败/panic 后兜底）。
/// 失败时返回 Cancelled——这只会发生在观察无法 determinize 或确无合法决策时。
fn fallback_outcome(
    observation: AiObservation,
    context: DecisionContext,
    seed: u64,
    reason: AiTaskFailure,
) -> AiTaskOutcome {
    use rand::SeedableRng;
    use rand::rngs::StdRng;
    let mut rng = StdRng::seed_from_u64(seed);
    match determinize(&observation, context, &mut rng) {
        Ok(sim) => match fallback_decision(&sim) {
            Ok(decision) => AiTaskOutcome::Fallback {
                decision,
                seed,
                reason,
            },
            Err(AiError::Cancelled) => AiTaskOutcome::Cancelled,
            Err(_) => AiTaskOutcome::Cancelled,
        },
        Err(_) => AiTaskOutcome::Cancelled,
    }
}

/// 系统：当当前玩家是 CPU 且无活跃/待提交请求时，发起一次后台搜索。
pub(super) fn start_ai_search(
    controllers: Res<PlayerControllers>,
    model: Res<BattleModel>,
    phase: Res<BattlePhase>,
    pending_phase: Res<super::PendingPhase>,
    revision: Res<BattleRevision>,
    mut runtime: ResMut<AiRuntime>,
) {
    if runtime.active.is_some() || runtime.ready.is_some() {
        return;
    }
    let pid = model.0.current_id();
    if controllers.is_human(pid) {
        return;
    }
    if model.0.is_over() {
        return;
    }
    let Some(context) = current_decision_context(&phase, pending_phase.0.as_ref()) else {
        return;
    };
    let Some(config) = controllers.config(pid) else {
        return;
    };
    let config = config.clone();
    let observation = AiObservation::from_game(&model.0, pid);
    let context_kind = context.kind();
    let request_id = runtime.next_request_id;
    runtime.next_request_id = runtime.next_request_id.wrapping_add(1);
    runtime.decision_index = runtime.decision_index.wrapping_add(1);
    let seed = seed_for(runtime.match_id, runtime.decision_index, pid);
    let token = AiRequestToken {
        match_id: runtime.match_id,
        state_version: revision.0,
        player: pid,
        context_kind,
        request_id,
    };
    let control = SearchControl::new();
    let control_for_task = control.clone();

    // 闭包只捕获脱敏观察 + 上下文 + 配置 + 控制句柄 + seed——无 GameState/ECS。
    let fallback_observation = observation.clone();
    let fallback_context = context.clone();
    let task = AsyncComputeTaskPool::get().spawn(async move {
        let searched = std::panic::catch_unwind(AssertUnwindSafe(|| {
            search(observation, context, pid, seed, config, control_for_task)
        }));
        match searched {
            Ok(Ok(result)) => AiTaskOutcome::Completed(result),
            Ok(Err(AiError::Cancelled)) => AiTaskOutcome::Cancelled,
            Ok(Err(error)) => fallback_outcome(
                fallback_observation,
                fallback_context,
                seed,
                AiTaskFailure::Search(error),
            ),
            Err(_) => fallback_outcome(
                fallback_observation,
                fallback_context,
                seed,
                AiTaskFailure::Panicked,
            ),
        }
    });
    runtime.active = Some(ActiveSearch {
        token,
        control,
        task,
    });
}

/// 系统：非阻塞轮询活跃任务；完成则收纳为 ready。
pub(super) fn poll_ai_search(mut runtime: ResMut<AiRuntime>) {
    let outcome = {
        let Some(active) = runtime.active.as_mut() else {
            return;
        };
        if !active.task.is_finished() {
            return;
        }
        let outcome = block_on(poll_once(&mut active.task));
        let token = active.token;
        outcome.map(|o| (token, o))
    };
    if let Some((token, outcome)) = outcome {
        // 任务已取出，清空 active 槽。
        runtime.accept_task_outcome(token, outcome);
        runtime.active = None;
    }
}

/// 系统：提交 ready 结果到 `RuleDecisionQueue`（仅当 token 仍匹配且无动画/事件）。
#[allow(clippy::too_many_arguments)]
pub(super) fn submit_ready_ai_decision(
    model: Res<BattleModel>,
    phase: Res<BattlePhase>,
    pending_phase: Res<super::PendingPhase>,
    revision: Res<BattleRevision>,
    pending_events: Res<PendingEvents>,
    anim: Res<super::AnimationCounts>,
    mut runtime: ResMut<AiRuntime>,
    mut queue: ResMut<RuleDecisionQueue>,
) {
    let Some((token, result)) = runtime.ready.take() else {
        return;
    };
    // 提交安全条件：无待播事件、无动画忙。
    if !pending_events.0.is_empty() || anim.busy() {
        // 放回 ready，等下一帧。
        runtime.ready = Some((token, result));
        return;
    }
    let pid = model.0.current_id();
    // token 过期：丢弃，让 start_ai_search 重新发起。
    let current_context_kind =
        current_decision_context(&phase, pending_phase.0.as_ref()).map(|c| c.kind());
    let still_valid = token_matches(
        &token,
        runtime.match_id,
        revision.0,
        pid,
        current_context_kind.unwrap_or(DecisionContextKind::MainTurn),
        token.request_id,
    ) && current_context_kind == Some(token.context_kind);
    if !still_valid {
        return;
    }
    // 把 AiDecision 映射到 QueuedRuleDecision。
    let queued = match result.decision {
        AiDecision::Action(action) => super::QueuedRuleDecision::Action(action),
        AiDecision::Discard(tokens) => {
            super::QueuedRuleDecision::Resume(Resume::DiscardTokens(tokens))
        }
        AiDecision::ChooseNoble(noble) => {
            super::QueuedRuleDecision::Resume(Resume::ChooseNoble(noble))
        }
    };
    queue.0.push(queued);
}

/// 系统：取消过期/冲突的活跃搜索（如人类行为或状态变更使当前请求失效）。
/// 当前实现：若活跃 token 与当前状态不匹配，则取消并丢弃。
pub(super) fn cancel_stale_search(
    model: Res<BattleModel>,
    phase: Res<BattlePhase>,
    pending_phase: Res<super::PendingPhase>,
    revision: Res<BattleRevision>,
    mut runtime: ResMut<AiRuntime>,
) {
    let Some(active) = runtime.active.as_ref() else {
        return;
    };
    let pid = model.0.current_id();
    let current_context_kind =
        current_decision_context(&phase, pending_phase.0.as_ref()).map(|c| c.kind());
    let valid = token_matches(
        &active.token,
        runtime.match_id,
        revision.0,
        pid,
        current_context_kind.unwrap_or(DecisionContextKind::MainTurn),
        active.token.request_id,
    ) && current_context_kind == Some(active.token.context_kind);
    if valid {
        return;
    }
    // 取出活跃搜索（take 后 active 为 None），取消并丢弃。
    if let Some(active) = runtime.active.take() {
        active.control.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_exact_current_request_is_accepted() {
        let token = AiRequestToken {
            match_id: 10,
            state_version: 3,
            player: 1,
            context_kind: DecisionContextKind::MainTurn,
            request_id: 7,
        };
        assert!(token_matches(
            &token,
            10,
            3,
            1,
            DecisionContextKind::MainTurn,
            7
        ));
        assert!(!token_matches(
            &token,
            10,
            4,
            1,
            DecisionContextKind::MainTurn,
            7
        ));
        assert!(!token_matches(
            &token,
            10,
            3,
            0,
            DecisionContextKind::MainTurn,
            7
        ));
        assert!(!token_matches(
            &token,
            10,
            3,
            1,
            DecisionContextKind::Discard,
            7
        ));
    }

    #[test]
    fn human_input_is_allowed_only_for_human_current_player() {
        let controllers = PlayerControllers::human_vs_cpu();
        assert!(controllers.is_human(0));
        assert!(!controllers.is_human(1));
    }

    #[test]
    fn seed_is_deterministic_for_same_inputs() {
        assert_eq!(seed_for(10, 3, 1), seed_for(10, 3, 1));
        assert_ne!(seed_for(10, 3, 1), seed_for(10, 4, 1));
        assert_ne!(seed_for(10, 3, 0), seed_for(10, 3, 1));
    }

    #[test]
    fn current_decision_context_prefers_pending() {
        let phase = BattlePhase::Idle;
        let pending = BattlePhase::AwaitDiscard { excess: 2 };
        let ctx = current_decision_context(&phase, Some(&pending)).unwrap();
        assert!(matches!(ctx, DecisionContext::Discard { excess: 2 }));
    }

    #[test]
    fn game_over_yields_no_context() {
        let phase = BattlePhase::GameOver {
            winner: 0,
            standings: vec![(0, 15)],
        };
        assert!(current_decision_context(&phase, None).is_none());
    }
}
