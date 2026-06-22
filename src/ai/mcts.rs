//! 可取消、可限时的单线程 MO-ISMCTS。
//!
//! 搜索以根玩家的脱敏观察为输入，每次迭代重新 determinize 一个一致的可能世界，
//! 并在每次树内决策前对当前行动者做 actor-relative 重采样（保留各玩家私有盲抽牌
//! 身份）。树节点按 `InfoSetKey` 索引，边按 `AiDecision` 索引；每次访问信息集时为
//! 所有当前合法边累加 `availability`，UCT 选择在已存在边中取最大，未存在合法边则
//! 按权重稳定扩展一条。每个迭代在扩展一条新边后下降到该子节点并调用 `rollout`，
//! 把 rollout 的根玩家奖励回传到路径上每条边。
//!
//! 终止条件三选一：达到迭代上限、超过时间截止、收到外部取消。前两者返回当前最佳
//! 根决策；后者返回 `AiError::Cancelled`。

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use rand::SeedableRng;
use rand::rngs::StdRng;

use crate::rules::PlayerAction;
use crate::rules::PlayerId;

use super::decision::{AiDecision, AiError, DecisionContext, SimulationState};
use super::determinization::{PrivateKnowledge, determinize};
use super::observation::{AiObservation, InfoSetKey};
use super::rollout::{fallback_decision, rollout};

/// 搜索配置。`normal()` 给出 1 秒预算、10 万节点上限、60 回合 rollout。
#[derive(Clone, Debug)]
pub struct MctsConfig {
    pub time_limit: Duration,
    pub iteration_limit: Option<u32>,
    pub max_nodes: usize,
    pub max_rollout_turns: u16,
    pub exploration: f32,
    pub rollout_epsilon: f32,
}

impl MctsConfig {
    pub fn normal() -> Self {
        Self {
            time_limit: Duration::from_secs(1),
            iteration_limit: None,
            max_nodes: 100_000,
            max_rollout_turns: 60,
            exploration: 2.0_f32.sqrt(),
            rollout_epsilon: 0.15,
        }
    }

    pub fn for_iterations(iterations: u32) -> Self {
        Self {
            iteration_limit: Some(iterations),
            time_limit: Duration::MAX,
            ..Self::normal()
        }
    }
}

/// 外部取消句柄。原子位标志，可在任意线程触发。
#[derive(Clone, Default)]
pub struct SearchControl(Arc<AtomicBool>);

impl SearchControl {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// 单条边的统计：访问次数、累计根玩家奖励、可用次数。
#[derive(Clone, Debug, Default)]
struct EdgeStats {
    visits: u32,
    total_reward: f32,
    availability: u32,
}

/// 信息集节点：自身访问次数 + 各决策边统计。
#[derive(Clone, Debug, Default)]
struct Node {
    visits: u32,
    edges: HashMap<AiDecision, EdgeStats>,
}

/// 搜索指标：耗时、迭代数、节点数、是否走保底、根决策统计。
#[derive(Clone, Debug)]
pub struct AiSearchMetrics {
    pub elapsed: Duration,
    pub iterations: u32,
    pub nodes: usize,
    pub used_fallback: bool,
    pub root_actions: Vec<RootActionStat>,
}

/// 根决策的汇总统计，用于稳定选择与对外可观测性。
#[derive(Clone, Debug)]
pub struct RootActionStat {
    pub decision: AiDecision,
    pub visits: u32,
    pub mean_reward: f32,
}

#[cfg(test)]
impl RootActionStat {
    fn test_action(index: usize, visits: u32, mean_reward: f32) -> Self {
        Self {
            decision: AiDecision::Action(PlayerAction::BuyReservedCard(index)),
            visits,
            mean_reward,
        }
    }
}

/// 搜索结果：选定决策、所用 seed、指标。
#[derive(Clone, Debug)]
pub struct AiSearchResult {
    pub decision: AiDecision,
    pub seed: u64,
    pub metrics: AiSearchMetrics,
}

/// UCT 得分。`maximizing_root` 决定 exploitation 用原始均值还是 1-均值
/// （对手节点应最小化根玩家奖励）。
fn uct_score(edge: &EdgeStats, maximizing_root: bool, exploration: f32) -> f32 {
    if edge.visits == 0 {
        return f32::INFINITY;
    }
    let mean = edge.total_reward / edge.visits as f32;
    let exploitation = if maximizing_root { mean } else { 1.0 - mean };
    let available = edge.availability.max(1) as f32;
    exploitation + exploration * (available.ln() / edge.visits as f32).sqrt()
}

/// 入口：在根观察上运行 MO-ISMCTS，返回根决策 + 指标。
pub fn search(
    observation: AiObservation,
    context: DecisionContext,
    root: PlayerId,
    seed: u64,
    config: MctsConfig,
    control: SearchControl,
) -> Result<AiSearchResult, AiError> {
    let started = Instant::now();
    let deadline = started.checked_add(config.time_limit);
    let mut rng = StdRng::seed_from_u64(seed);
    let root_sim = determinize(&observation, context.clone(), &mut rng)?;
    let fallback = fallback_decision(&root_sim)?;
    let mut tree: HashMap<InfoSetKey, Node> = HashMap::new();
    let mut iterations = 0u32;

    while config
        .iteration_limit
        .is_none_or(|limit| iterations < limit)
        && deadline.is_none_or(|limit| Instant::now() < limit)
        && !control.is_cancelled()
    {
        let mut simulation = determinize(&observation, context.clone(), &mut rng)?;
        let mut knowledge = PrivateKnowledge::from_state(&simulation.game);
        let mut path: Vec<(InfoSetKey, AiDecision)> = Vec::new();
        let iteration = tree_iteration(
            &mut tree,
            &mut simulation,
            &mut knowledge,
            root,
            &config,
            &control,
            deadline,
            &mut rng,
            &mut path,
        );
        match iteration {
            Ok(()) => iterations += 1,
            Err(AiError::Cancelled) if control.is_cancelled() => {
                return Err(AiError::Cancelled);
            }
            Err(AiError::Cancelled) => break,
            Err(error) => return Err(error),
        }
    }

    if control.is_cancelled() {
        return Err(AiError::Cancelled);
    }
    let root_key = observation.information_set_key(&context);
    let root_stats = collect_root_stats(tree.get(&root_key));
    let selected = choose_root_action(&root_stats).unwrap_or_else(|| fallback.clone());
    Ok(AiSearchResult {
        decision: selected,
        seed,
        metrics: AiSearchMetrics {
            elapsed: started.elapsed(),
            iterations,
            nodes: tree.len(),
            used_fallback: root_stats.is_empty(),
            root_actions: root_stats,
        },
    })
}

/// 单次树迭代：下降、扩展一条新边、rollout、回传。
///
/// 在每个信息集节点：
/// 1. 对当前行动者做 actor-relative 重采样（保护各玩家私有盲抽牌身份）。
/// 2. 取当前合法决策；为每条合法边累加 `availability`。
/// 3. 若有未存在的合法边且树未达 `max_nodes`，按权重稳定扩展一条新边，下降，
///    并在下降后立即 rollout；否则按 UCT 选最大已存在边继续下降。
/// 4. 终局或无合法决策时直接 rollout/结算。
#[allow(clippy::too_many_arguments)]
fn tree_iteration<R: rand::Rng + ?Sized>(
    tree: &mut HashMap<InfoSetKey, Node>,
    simulation: &mut SimulationState,
    knowledge: &mut PrivateKnowledge,
    root: PlayerId,
    config: &MctsConfig,
    control: &SearchControl,
    deadline: Option<Instant>,
    rng: &mut R,
    path: &mut Vec<(InfoSetKey, AiDecision)>,
) -> Result<(), AiError> {
    let stop = || control.is_cancelled() || deadline.is_some_and(|limit| Instant::now() >= limit);

    loop {
        if simulation.game.is_over() {
            // 终局：直接用终局评估作为回传奖励（0/1）。
            let reward = super::evaluation::evaluate(&simulation.game, root);
            backpropagate(tree, path, root, reward);
            return Ok(());
        }

        // 每次树决策前对当前行动者做 actor-relative 重采样。
        let actor = simulation.game.current_id();
        // 重采样可能在退化局面下因候选不守恒失败（树内 apply_decision 改变了牌堆/保留
        // 分布，与标准牌库逐级分配不再平衡）。此时以当前评估值结算本迭代并退出，
        // 不让单次退化采样中止整次搜索。
        if knowledge
            .redeterminize_for_actor(simulation, actor, rng)
            .is_err()
        {
            let reward = super::evaluation::evaluate(&simulation.game, root);
            backpropagate(tree, path, root, reward);
            return Ok(());
        }

        let key = AiObservation::from_game(&simulation.game, actor)
            .information_set_key(&simulation.context);
        let legal = simulation.legal_decisions()?;
        if legal.is_empty() {
            // 非终局但无合法决策（极端牌堆耗尽 + 保留满）：以当前评估值结算本迭代，
            // 不让单次退化采样中止整次搜索。
            let reward = super::evaluation::evaluate(&simulation.game, root);
            backpropagate(tree, path, root, reward);
            return Ok(());
        }

        // 在持有节点可变借用前先判定是否允许扩展。
        let can_expand = tree.len() < config.max_nodes;
        let node = tree.entry(key).or_default();
        node.visits += 1;
        // 每次访问信息集时为每条当前合法边累加 availability。
        for decision in &legal {
            node.edges.entry(decision.clone()).or_default().availability += 1;
        }

        // 找未存在的合法边（树未满时扩展）。
        let missing: Vec<AiDecision> = if can_expand {
            legal
                .iter()
                .filter(|d| !node.edges.contains_key(*d))
                .cloned()
                .collect()
        } else {
            Vec::new()
        };

        let (chosen, expanded_new) = if let Some(best_missing) = pick_expand(missing, simulation) {
            (best_missing, true)
        } else {
            // 全部合法边已存在：UCT 选最大。
            let maximizing = actor == root;
            let mut best: Option<&AiDecision> = None;
            let mut best_score = f32::NEG_INFINITY;
            for decision in &legal {
                let edge = node.edges.get(decision).expect("edge present");
                let score = uct_score(edge, maximizing, config.exploration);
                if score > best_score {
                    best_score = score;
                    best = Some(decision);
                }
            }
            (best.expect("non-empty legal").clone(), false)
        };

        path.push((key, chosen.clone()));
        simulation.apply_decision(chosen)?;

        if expanded_new {
            // 扩展一条新边后停止下降，立即 rollout。
            if simulation.game.is_over() {
                let reward = super::evaluation::evaluate(&simulation.game, root);
                backpropagate(tree, path, root, reward);
                return Ok(());
            }
            let result = rollout(
                simulation,
                root,
                config.max_rollout_turns,
                config.rollout_epsilon,
                rng,
                stop,
            );
            match result {
                Ok(rollout_out) => {
                    backpropagate(tree, path, root, rollout_out.reward);
                    return Ok(());
                }
                Err(AiError::Cancelled) => return Err(AiError::Cancelled),
                Err(other) => return Err(other),
            }
        }
        // 否则继续下降到下一信息集。
    }
}

/// 在待扩展的候选中按 rollout `decision_weight` 降序、`format!("{decision:?}")`
/// 字典序升序，取第一条。候选为空返回 None。
fn pick_expand(missing: Vec<AiDecision>, simulation: &SimulationState) -> Option<AiDecision> {
    if missing.is_empty() {
        return None;
    }
    let mut best: Option<(f32, String, AiDecision)> = None;
    for decision in missing {
        let weight = super::rollout::decision_weight_for(simulation, &decision);
        let key = format!("{decision:?}");
        let is_better = match &best {
            None => true,
            Some((bw, bk, _)) => (weight > *bw) || (weight == *bw && key < *bk),
        };
        if is_better {
            best = Some((weight, key, decision));
        }
    }
    best.map(|(_, _, d)| d)
}

/// 把根玩家奖励回传到路径上每条边（同值），并累加各节点已由访问处理。
fn backpropagate(
    tree: &mut HashMap<InfoSetKey, Node>,
    path: &[(InfoSetKey, AiDecision)],
    _root: PlayerId,
    reward: f32,
) {
    for (key, decision) in path {
        let node = tree.entry(*key).or_default();
        let edge = node.edges.entry(decision.clone()).or_default();
        edge.visits += 1;
        edge.total_reward += reward;
    }
}

/// 从根节点收集 `RootActionStat` 列表（按 visits 降序、mean 降序、字典序升序）。
fn collect_root_stats(root: Option<&Node>) -> Vec<RootActionStat> {
    let Some(node) = root else {
        return Vec::new();
    };
    let mut stats: Vec<RootActionStat> = node
        .edges
        .iter()
        .map(|(decision, edge)| RootActionStat {
            decision: decision.clone(),
            visits: edge.visits,
            mean_reward: if edge.visits == 0 {
                0.0
            } else {
                edge.total_reward / edge.visits as f32
            },
        })
        .collect();
    sort_root_stats(&mut stats);
    stats
}

/// 稳定排序：visits 降序 → mean_reward 降序 → debug 字典序升序。
fn sort_root_stats(stats: &mut [RootActionStat]) {
    stats.sort_by(|a, b| {
        b.visits
            .cmp(&a.visits)
            .then_with(|| {
                b.mean_reward
                    .partial_cmp(&a.mean_reward)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| format!("{:?}", a.decision).cmp(&format!("{:?}", b.decision)))
    });
}

/// 从根统计中选首条决策。空列表返回 None。
fn choose_root_action(stats: &[RootActionStat]) -> Option<AiDecision> {
    stats.first().map(|s| s.decision.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::decision::DecisionContext;
    use crate::rules::GameState;
    use rand::Rng;

    #[test]
    fn opponent_uct_inverts_root_reward() {
        let high_for_root = EdgeStats {
            visits: 10,
            total_reward: 9.0,
            availability: 20,
        };
        let low_for_root = EdgeStats {
            visits: 10,
            total_reward: 2.0,
            availability: 20,
        };
        assert!(
            uct_score(&high_for_root, true, 2.0_f32.sqrt())
                > uct_score(&low_for_root, true, 2.0_f32.sqrt())
        );
        assert!(
            uct_score(&high_for_root, false, 2.0_f32.sqrt())
                < uct_score(&low_for_root, false, 2.0_f32.sqrt())
        );
    }

    #[test]
    fn root_choice_prefers_visits_before_mean_reward() {
        let a = RootActionStat::test_action(0, 20, 0.55);
        let b = RootActionStat::test_action(1, 10, 0.95);
        assert_eq!(choose_root_action(&[a.clone(), b]), Some(a.decision));
    }

    #[test]
    fn fixed_seed_and_iteration_limit_are_deterministic() {
        let game = GameState::new_seeded(2, 61).unwrap();
        let observation = AiObservation::from_game(&game, 0);
        let config = MctsConfig::for_iterations(128);
        let left = search(
            observation.clone(),
            DecisionContext::MainTurn,
            0,
            67,
            config.clone(),
            SearchControl::new(),
        )
        .unwrap();
        let right = search(
            observation,
            DecisionContext::MainTurn,
            0,
            67,
            config,
            SearchControl::new(),
        )
        .unwrap();
        assert_eq!(left.decision, right.decision);
        assert_eq!(left.metrics.iterations, 128);
    }

    #[test]
    fn node_limit_returns_legal_result() {
        let game = GameState::new_seeded(2, 89).unwrap();
        let observation = AiObservation::from_game(&game, 0);
        let config = MctsConfig {
            max_nodes: 1,
            ..MctsConfig::for_iterations(32)
        };
        let result = search(
            observation,
            DecisionContext::MainTurn,
            0,
            97,
            config,
            SearchControl::new(),
        )
        .unwrap();
        // 结果必须是当前合法决策之一。
        let game = GameState::new_seeded(2, 89).unwrap();
        let sim = SimulationState::new(game, DecisionContext::MainTurn);
        assert!(sim.legal_decisions().unwrap().contains(&result.decision));
    }

    #[test]
    fn pre_cancelled_returns_cancelled_error() {
        let game = GameState::new_seeded(2, 101).unwrap();
        let observation = AiObservation::from_game(&game, 0);
        let control = SearchControl::new();
        control.cancel();
        let result = search(
            observation,
            DecisionContext::MainTurn,
            0,
            103,
            MctsConfig::for_iterations(64),
            control,
        );
        assert_eq!(result.unwrap_err(), AiError::Cancelled);
    }

    #[test]
    fn seeded_ai_games_terminate_with_only_legal_decisions() {
        for seed in 0..4u64 {
            let game = GameState::new_seeded(2, seed).unwrap();
            let mut simulation = SimulationState::new(game, DecisionContext::MainTurn);
            for decision_index in 0..600u64 {
                if simulation.game.is_over() {
                    break;
                }
                let player = simulation.game.current_id();
                let observation = AiObservation::from_game(&simulation.game, player);
                let result = match search(
                    observation,
                    simulation.context.clone(),
                    player,
                    seed ^ decision_index,
                    MctsConfig::for_iterations(64),
                    SearchControl::new(),
                ) {
                    Ok(r) => r,
                    Err(AiError::NoLegalDecision) => {
                        // 罕见死锁：当前玩家无合法行动且游戏未结束（牌堆耗尽 + 保留满 +
                        // 买不起 + 银行不足以拿筹码）。规则层无 pass 行动，视为该局终止。
                        break;
                    }
                    Err(e) => panic!("seed {seed} step {decision_index} search err: {e:?}"),
                };
                assert!(
                    simulation
                        .legal_decisions()
                        .unwrap()
                        .contains(&result.decision),
                    "seed {seed} 决策 #{decision_index} 非法: {:?}",
                    result.decision
                );
                simulation.apply_decision(result.decision).unwrap();
            }
            // 终止条件：要么游戏结束，要么死锁（无合法决策）。
            assert!(
                simulation.game.is_over() || simulation.legal_decisions().unwrap().is_empty(),
                "seed {seed} 未在 600 步内结束也未死锁"
            );
        }
    }

    /// 强度基准汇总。
    struct BenchmarkSummary {
        games: u32,
        mcts_wins: u32,
    }

    impl BenchmarkSummary {
        fn win_rate(&self) -> f32 {
            self.mcts_wins as f32 / self.games as f32
        }
    }

    fn benchmark_against_random(seeds: u64, iterations: u32, base_seed: u64) -> BenchmarkSummary {
        let mut summary = BenchmarkSummary {
            games: 0,
            mcts_wins: 0,
        };
        for seed in 0..seeds {
            for mcts_seat in [0u64, 1u64] {
                let winner =
                    play_benchmark_game(seed, mcts_seat as PlayerId, iterations, base_seed);
                summary.games += 1;
                // 平局（None）不计为 MCTS 胜。
                if winner == Some(mcts_seat as PlayerId) {
                    summary.mcts_wins += 1;
                }
            }
        }
        summary
    }

    fn play_benchmark_game(
        game_seed: u64,
        mcts_seat: PlayerId,
        iterations: u32,
        policy_seed: u64,
    ) -> Option<PlayerId> {
        let game = GameState::new_seeded(2, game_seed).unwrap();
        let mut simulation = SimulationState::new(game, DecisionContext::MainTurn);
        let mut random = StdRng::seed_from_u64(policy_seed ^ game_seed ^ mcts_seat as u64);
        for decision_index in 0..600_u64 {
            if let Some(winner) = simulation.game.winner {
                return Some(winner);
            }
            let player = simulation.game.current_id();
            let legal = match simulation.legal_decisions() {
                Ok(l) if !l.is_empty() => l,
                _ => return None, // 死锁：无合法决策，视为平局（MCTS 不胜）。
            };
            let decision = if player == mcts_seat {
                let observation = AiObservation::from_game(&simulation.game, player);
                match search(
                    observation,
                    simulation.context.clone(),
                    player,
                    policy_seed ^ game_seed ^ decision_index,
                    MctsConfig::for_iterations(iterations),
                    SearchControl::new(),
                ) {
                    Ok(r) => r.decision,
                    Err(_) => return None,
                }
            } else {
                legal[random.random_range(0..legal.len())].clone()
            };
            simulation.apply_decision(decision).unwrap();
        }
        None // 600 步未结束，视为平局。
    }

    #[test]
    #[ignore = "manual AI strength benchmark"]
    fn mcts_beats_random_at_least_sixty_five_percent() {
        let summary = benchmark_against_random(100, 256, 0xA11CE);
        assert!(
            summary.mcts_wins * 100 >= summary.games * 65,
            "MCTS wins {}/{} ({:.1}%)",
            summary.mcts_wins,
            summary.games,
            summary.win_rate() * 100.0
        );
    }
}
