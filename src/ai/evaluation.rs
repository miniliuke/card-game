//! 非终局局面评估：六个归一化特征的加权和。
//!
//! 仅当 rollout 达到深度上限时使用评估函数；终局只优化胜率。每个特征计算根玩家
//! 相对手的优势并限制到 `[-1, 1]`，加权优势 `a` 再限制到 `[-1, 1]`，最终估计值为
//! `0.5 + 0.5 * a`。所有权重集中在 `EvaluationWeights`，首版不引入学习或自动调参。

use crate::rules::{
    CardColor, CardLevel, CardStore, GameState, NobleBoard, NobleId, PlayerId, PlayerState,
    ReserveOrigin, ReservedCard, TokenSet,
};

/// 六个特征的权重。和为 1.0。
#[derive(Clone, Copy, Debug)]
pub struct EvaluationWeights {
    pub score: f32,
    pub engine: f32,
    pub nobles: f32,
    pub buying_power: f32,
    pub token_efficiency: f32,
    pub reservations: f32,
}

impl Default for EvaluationWeights {
    fn default() -> Self {
        Self {
            score: 0.35,
            engine: 0.20,
            nobles: 0.15,
            buying_power: 0.15,
            token_efficiency: 0.10,
            reservations: 0.05,
        }
    }
}

/// 把 `(left - right) / scale` 限制到 `[-1, 1]`。
fn normalized_difference(left: f32, right: f32, scale: f32) -> f32 {
    ((left - right) / scale).clamp(-1.0, 1.0)
}

/// 评估 `root` 视角的局面价值，返回 `[0, 1]` 的胜率估计。终局返回 1.0/0.0。
pub fn evaluate(game: &GameState, root: PlayerId) -> f32 {
    if let Some(winner) = game.winner {
        return if winner == root { 1.0 } else { 0.0 };
    }
    let opponent = opponent_of(root, game);
    let weights = EvaluationWeights::default();
    let advantage = weights.score * score_feature(game, root, opponent)
        + weights.engine * engine_feature(game, root, opponent)
        + weights.nobles * noble_feature(game, root, opponent)
        + weights.buying_power * buying_power_feature(game, root, opponent)
        + weights.token_efficiency * token_efficiency_feature(game, root, opponent)
        + weights.reservations * reservation_feature(game, root, opponent);
    0.5 + 0.5 * advantage.clamp(-1.0, 1.0)
}

fn opponent_of(root: PlayerId, game: &GameState) -> PlayerId {
    // 固定 2 人局：对手是另一玩家。
    if root == 0 { 1 } else { 0 }.min(game.players.len().saturating_sub(1))
}

fn player_score(game: &GameState, player: PlayerId) -> f32 {
    game.player(player)
        .score(&game.card_store, &game.noble_store) as f32
}

/// 分数优势：分差 / 15。
fn score_feature(game: &GameState, root: PlayerId, opponent: PlayerId) -> f32 {
    normalized_difference(player_score(game, root), player_score(game, opponent), 15.0)
}

/// 永久折扣优势：五色折扣总数 + 0.2×覆盖色数，差值 / 15。
fn engine_feature(game: &GameState, root: PlayerId, opponent: PlayerId) -> f32 {
    let root_bonus = game.player(root).bonus(&game.card_store);
    let opp_bonus = game.player(opponent).bonus(&game.card_store);
    let root_engine = bonus_engine_value(root_bonus);
    let opp_engine = bonus_engine_value(opp_bonus);
    normalized_difference(root_engine, opp_engine, 15.0)
}

fn bonus_engine_value(bonus: crate::rules::CardBonus) -> f32 {
    let total: f32 = CardColor::ALL.iter().map(|c| bonus.get(*c) as f32).sum();
    let distinct = CardColor::ALL.iter().filter(|c| bonus.get(**c) > 0).count() as f32;
    total + 0.2 * distinct
}

/// 贵族进度优势：对当前可见贵族需求中最佳完成比例的差值。
fn noble_feature(game: &GameState, root: PlayerId, opponent: PlayerId) -> f32 {
    let root_progress = best_noble_progress(game, root);
    let opp_progress = best_noble_progress(game, opponent);
    normalized_difference(root_progress, opp_progress, 1.0)
}

/// 该玩家对可用贵族中最接近完成的需求比例（0..1）。无可用贵族返回 0。
fn best_noble_progress(game: &GameState, player: PlayerId) -> f32 {
    let bonus = game.player(player).bonus(&game.card_store);
    game.nobles
        .available
        .iter()
        .map(|noble| requirement_completion(bonus, noble.requirement))
        .fold(0.0_f32, f32::max)
}

/// 每色 `bonus / requirement`（requirement 为 0 视为已满足=1），取最小值。
fn requirement_completion(
    bonus: crate::rules::CardBonus,
    requirement: crate::rules::GemCost,
) -> f32 {
    let mut worst = 1.0_f32;
    for color in CardColor::ALL {
        let need = requirement.get(color);
        if need == 0 {
            continue;
        }
        let have = bonus.get(color);
        worst = worst.min(have as f32 / need as f32);
    }
    worst.clamp(0.0, 1.0)
}

/// 即时购买力优势：当前可买牌的最大效用差值（效用 = 声望×2 + 1 + 贵族进度增量）。
fn buying_power_feature(game: &GameState, root: PlayerId, opponent: PlayerId) -> f32 {
    let root_power = max_affordable_utility(game, root);
    let opp_power = max_affordable_utility(game, opponent);
    normalized_difference(root_power, opp_power, 12.0)
}

fn max_affordable_utility(game: &GameState, player: PlayerId) -> f32 {
    let p = game.player(player);
    let bonus = p.bonus(&game.card_store);
    let mut best: f32 = 0.0;
    for level in CardLevel::ALL {
        for card in game.market.visible(level) {
            if crate::rules::can_afford(p.tokens, card, bonus).is_ok() {
                best = best.max(card_utility(game, player, card));
            }
        }
    }
    for reserved in &p.reserved_cards {
        if let Some(card) = game.card_store.get(reserved.card_id) {
            if crate::rules::can_afford(p.tokens, card, bonus).is_ok() {
                best = best.max(card_utility(game, player, card));
            }
        }
    }
    best
}

/// 卡牌效用：声望×2 + 1 永久折扣 + 该色贵族进度增量。
fn card_utility(game: &GameState, player: PlayerId, card: &crate::rules::DevelopmentCard) -> f32 {
    let before = best_noble_progress(game, player);
    let mut hypothetical_bonus = game.player(player).bonus(&game.card_store);
    hypothetical_bonus.add(card.color);
    let after = game
        .nobles
        .available
        .iter()
        .map(|noble| requirement_completion(hypothetical_bonus, noble.requirement))
        .fold(0.0_f32, f32::max);
    card.prestige as f32 * 2.0 + 1.0 + (after - before).max(0.0)
}

/// 筹码效率优势：持有筹码对最便宜可见/己方保留目标缺口的匹配度 / max(token_total,1)。
fn token_efficiency_feature(game: &GameState, root: PlayerId, opponent: PlayerId) -> f32 {
    let root_eff = token_efficiency(game, root);
    let opp_eff = token_efficiency(game, opponent);
    normalized_difference(root_eff, opp_eff, 1.0)
}

fn token_efficiency(game: &GameState, player: PlayerId) -> f32 {
    let p = game.player(player);
    let bonus = p.bonus(&game.card_store);
    let total = p.token_total() as f32;
    if total == 0.0 {
        return 0.0;
    }
    // 找最便宜的可见/己方保留卡（折扣后缺口最小），计算持有筹码对该缺口的覆盖比例。
    let mut best_match = 0.0_f32;
    let mut found_target = false;
    for level in CardLevel::ALL {
        for card in game.market.visible(level) {
            let (deficit, cost) = token_deficit(p.tokens, card, bonus);
            if cost == 0 {
                continue;
            }
            found_target = true;
            best_match = best_match.max(useful_token_ratio(p.tokens, deficit));
        }
    }
    for reserved in &p.reserved_cards {
        if let Some(card) = game.card_store.get(reserved.card_id) {
            let (deficit, cost) = token_deficit(p.tokens, card, bonus);
            if cost == 0 {
                continue;
            }
            found_target = true;
            best_match = best_match.max(useful_token_ratio(p.tokens, deficit));
        }
    }
    if !found_target {
        return 0.0;
    }
    best_match / total
}

/// 返回（折扣后仍缺的逐色普通缺口集合，总普通成本）。`cost` 仅用于判断是否免费。
fn token_deficit(
    tokens: TokenSet,
    card: &crate::rules::DevelopmentCard,
    bonus: crate::rules::CardBonus,
) -> (TokenSet, u8) {
    let required = card.cost.after_discount(bonus);
    let mut deficit = TokenSet::default();
    let mut total = 0u8;
    for color in CardColor::ALL {
        let need = required.get(color);
        let have = tokens.get(color.to_gem());
        if have < need {
            let missing = need - have;
            deficit.set(color.to_gem(), missing);
            total += missing;
        }
    }
    (deficit, total)
}

/// 持有筹码中对缺口有用的比例：逐色 min(have, deficit) 之和 / deficit 总和。
fn useful_token_ratio(tokens: TokenSet, deficit: TokenSet) -> f32 {
    let deficit_total = deficit.total() as f32;
    if deficit_total == 0.0 {
        return 0.0;
    }
    let useful: u8 = CardColor::ALL
        .iter()
        .map(|c| tokens.get(c.to_gem()).min(deficit.get(c.to_gem())))
        .sum();
    // 金可补任意缺口。
    let gold_useful = tokens
        .get(crate::rules::GemColor::Gold)
        .min(deficit.total());
    (useful + gold_useful) as f32 / deficit_total
}

/// 保留牌质量优势：最大保留卡效用 / 12，占满 3 槽且皆 0 分时 -0.15。
fn reservation_feature(game: &GameState, root: PlayerId, opponent: PlayerId) -> f32 {
    let root_res = reservation_quality(game, root);
    let opp_res = reservation_quality(game, opponent);
    normalized_difference(root_res, opp_res, 1.0)
}

fn reservation_quality(game: &GameState, player: PlayerId) -> f32 {
    let p = game.player(player);
    let mut best: f32 = 0.0;
    let mut all_zero_prestige = !p.reserved_cards.is_empty();
    for reserved in &p.reserved_cards {
        if let Some(card) = game.card_store.get(reserved.card_id) {
            let utility = card_utility(game, player, card) / 12.0;
            best = best.max(utility);
            if card.prestige > 0 {
                all_zero_prestige = false;
            }
        }
    }
    if p.reserved_cards.len() >= 3 && all_zero_prestige {
        best - 0.15
    } else {
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::GameState;

    #[test]
    fn terminal_result_is_binary_from_root_perspective() {
        let mut game = GameState::new_seeded(2, 41).unwrap();
        game.winner = Some(1);
        assert_eq!(evaluate(&game, 1), 1.0);
        assert_eq!(evaluate(&game, 0), 0.0);
    }

    #[test]
    fn score_and_engine_advantage_raise_cutoff_value() {
        let base = GameState::new_seeded(2, 43).unwrap();
        let mut improved = base.clone();
        let card = improved.market.level1_visible.remove(0);
        improved.players[0].purchased_cards.push(card.id);
        assert!(evaluate(&improved, 0) > evaluate(&base, 0));
    }

    #[test]
    fn cutoff_value_is_always_a_probability() {
        for seed in 0..32 {
            let game = GameState::new_seeded(2, seed).unwrap();
            assert!((0.0..=1.0).contains(&evaluate(&game, 0)));
        }
    }

    #[test]
    fn neutral_start_evaluates_near_half() {
        let game = GameState::new_seeded(2, 0).unwrap();
        let value = evaluate(&game, 0);
        // 对称起手，双方优势接近 0，评估值应接近 0.5。
        assert!((0.45..=0.55).contains(&value), "value was {value}");
    }
}
