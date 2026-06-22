//! 启发式 rollout 策略与保底合法决策。
//!
//! `fallback_decision` 总是返回一个当前上下文下的合法决策——它对每个合法决策克隆并
//! 应用一次，按评估增益取指数权重，选最大权重者（平局按 `format!("{decision:?}")`
//! 字典序稳定化）。`rollout` 在搜索内部以 `1 - epsilon` 概率走权重选择、`epsilon`
//! 概率随机抽样，受 `max_complete_turns` 与外部 `should_stop` 取消信号约束。
//!
//! rollout 只访问 `SimulationState` 暴露的 API 与 `evaluate`，不复制规则、不读取
//! 隐藏信息。

use rand::Rng;
use rand::rngs::StdRng;

use crate::rules::PlayerId;

use super::decision::{AiDecision, AiError, DecisionContext, SimulationState};
use super::evaluation::evaluate;

/// rollout 结果：终局（或受限于回合上限）时根玩家视角的胜率估计与完整回合计数。
#[derive(Debug)]
pub struct RolloutResult {
    pub reward: f32,
    pub complete_turns: u16,
}

/// 返回当前上下文下权重最大的合法决策；平局按 `format!("{decision:?}")` 字典序。
/// 若无合法决策返回 `AiError::NoLegalDecision`。
pub fn fallback_decision(state: &SimulationState) -> Result<AiDecision, AiError> {
    let decisions = state.legal_decisions()?;
    if decisions.is_empty() {
        return Err(AiError::NoLegalDecision);
    }
    choose_best_by_weight(state, &decisions)
}

/// 受限 rollout：在 `max_complete_turns` 内按 epsilon 权重策略模拟到终局或上限。
/// `should_stop` 在每个模拟决策点被调用；返回 `true` 时立即以 `AiError::Cancelled` 退出。
pub fn rollout<R: Rng + ?Sized>(
    state: &mut SimulationState,
    root: PlayerId,
    max_complete_turns: u16,
    epsilon: f32,
    rng: &mut R,
    should_stop: impl Fn() -> bool,
) -> Result<RolloutResult, AiError> {
    let mut complete_turns = 0u16;
    while !state.game.is_over() && complete_turns < max_complete_turns {
        if should_stop() {
            return Err(AiError::Cancelled);
        }
        let before_player = state.game.current_id();
        let decisions = state.legal_decisions()?;
        if decisions.is_empty() {
            return Err(AiError::NoLegalDecision);
        }
        let decision = if rng.random::<f32>() < epsilon {
            decisions[rng.random_range(0..decisions.len())].clone()
        } else {
            weighted_choice(state, &decisions, rng)
        };
        state.apply_decision(decision)?;
        // 仅 MainTurn 上下文下当前玩家变化视为"一个完整回合"。
        if matches!(state.context, DecisionContext::MainTurn)
            && state.game.current_id() != before_player
        {
            complete_turns = complete_turns.saturating_add(1);
        }
    }
    Ok(RolloutResult {
        reward: evaluate(&state.game, root),
        complete_turns,
    })
}

/// 每个合法决策的权重：克隆状态、应用决策、用评估增益取指数。应用失败给 0.01 兜底。
fn decision_weight(state: &SimulationState, decision: &AiDecision) -> f32 {
    let actor = state.game.current_id();
    let before = evaluate(&state.game, actor);
    let mut after = state.clone();
    if after.apply_decision(decision.clone()).is_err() {
        return 0.01;
    }
    let gain = evaluate(&after.game, actor) - before;
    (gain * 4.0).exp().max(0.01)
}

/// 在 `decisions` 中按累积权重抽一个；权重总和为 0 时退化为均匀随机。
fn weighted_choice<R: Rng + ?Sized>(
    state: &SimulationState,
    decisions: &[AiDecision],
    rng: &mut R,
) -> AiDecision {
    let weights: Vec<f32> = decisions.iter().map(|d| decision_weight(state, d)).collect();
    let total: f32 = weights.iter().copied().sum();
    if total <= 0.0 {
        return decisions[rng.random_range(0..decisions.len())].clone();
    }
    let draw = rng.random::<f32>() * total;
    let mut cumulative = 0.0_f32;
    for (decision, weight) in decisions.iter().zip(weights.iter()) {
        cumulative += *weight;
        if draw < cumulative {
            return decision.clone();
        }
    }
    decisions.last().expect("non-empty decisions").clone()
}

/// 在权重最大者中按 `format!("{decision:?}")` 字典序打破平局。
fn choose_best_by_weight(
    state: &SimulationState,
    decisions: &[AiDecision],
) -> Result<AiDecision, AiError> {
    let mut best: Option<(f32, String, AiDecision)> = None;
    for decision in decisions {
        let weight = decision_weight(state, decision);
        let key = format!("{decision:?}");
        let is_better = match &best {
            None => true,
            Some((bw, bk, _)) => {
                (weight > *bw) || (weight == *bw && key < *bk)
            }
        };
        if is_better {
            best = Some((weight, key, decision.clone()));
        }
    }
    best.map(|(_, _, decision)| decision)
        .ok_or(AiError::NoLegalDecision)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::decision::DecisionContext;
    use crate::rules::GameState;
    use rand::SeedableRng;

    #[test]
    fn fallback_is_always_legal() {
        let game = GameState::new_seeded(2, 47).unwrap();
        let sim = SimulationState::new(game, DecisionContext::MainTurn);
        let decision = fallback_decision(&sim).unwrap();
        assert!(sim.legal_decisions().unwrap().contains(&decision));
    }

    #[test]
    fn rollout_stops_at_the_complete_turn_limit() {
        let game = GameState::new_seeded(2, 53).unwrap();
        let mut sim = SimulationState::new(game, DecisionContext::MainTurn);
        let mut rng = StdRng::seed_from_u64(59);
        let result = rollout(&mut sim, 0, 3, 0.15, &mut rng, || false).unwrap();
        assert!(result.complete_turns <= 3);
        assert!((0.0..=1.0).contains(&result.reward));
    }

    #[test]
    fn rollout_returns_cancelled_when_should_stop() {
        let game = GameState::new_seeded(2, 71).unwrap();
        let mut sim = SimulationState::new(game, DecisionContext::MainTurn);
        let mut rng = StdRng::seed_from_u64(73);
        let result = rollout(&mut sim, 0, 60, 0.15, &mut rng, || true);
        assert_eq!(result.unwrap_err(), AiError::Cancelled);
    }

    #[test]
    fn fallback_is_deterministic_under_same_state() {
        let game = GameState::new_seeded(2, 83).unwrap();
        let sim = SimulationState::new(game, DecisionContext::MainTurn);
        let left = fallback_decision(&sim).unwrap();
        let right = fallback_decision(&sim).unwrap();
        assert_eq!(left, right);
    }
}
