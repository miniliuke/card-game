//! 从脱敏观察构造观察者一致的可能世界（determinization）。
//!
//! 守恒约束：每张标准 90 张卡恰好位于市场、牌堆、保留区或已购区之一。已知的卡
//! （市场、已购、己方保留）从候选池移除；未知卡（对手盲抽、牌堆）按等级无放回
//! 采样分配。每次 MCTS 迭代重新采样，避免单一牌序主导结果。
//!
//! `redeterminize_for_actor` 在行动者切换时按该行动者观察重新采样其不可见信息，
//! 同时用 `PrivateKnowledge` 账本保护各玩家自己持有的盲抽牌身份——后采样的行动者
//! 不能抹去早先行动者的私有知识。

use std::collections::HashSet;

use rand::Rng;
use rand::seq::SliceRandom;

use crate::rules::{
    Bank, CardDecks, CardId, CardLevel, CardStore, DevelopmentCard, GameState, Market, NobleBoard,
    NobleId, NobleStore, PlayerId, PlayerState, ReserveOrigin, ReservedCard, TokenSet,
    standard_deck, standard_nobles,
};

use super::decision::{AiError, DecisionContext, SimulationState};
use super::observation::{AiObservation, ObservedReservation};

/// 从观察构造一个观察者一致的可能世界。
pub fn determinize<R: Rng + ?Sized>(
    observation: &AiObservation,
    context: DecisionContext,
    rng: &mut R,
) -> Result<SimulationState, AiError> {
    let standard = standard_deck();

    // 收集所有"已知位置"的卡 id：市场、已购、已知保留（自己持有或市场来源）。
    let mut known: HashSet<CardId> = HashSet::new();
    for row in &observation.market {
        known.extend(row.iter().map(|card| card.id));
    }
    for player in &observation.players {
        known.extend(player.purchased_cards.iter().copied());
        known.extend(
            player
                .reserved
                .iter()
                .filter_map(|reserved| match reserved {
                    ObservedReservation::Known(card) => Some(card.card_id),
                    ObservedReservation::HiddenBlind(_) => None,
                }),
        );
    }

    // 剩余候选卡按等级分桶并洗牌，供未知盲抽槽与牌堆无放回采样。
    let mut candidates: [Vec<DevelopmentCard>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for card in standard
        .iter()
        .copied()
        .filter(|card| !known.contains(&card.id))
    {
        candidates[level_index(card.level)].push(card);
    }
    for cards in &mut candidates {
        cards.shuffle(rng);
    }

    let players = materialize_players(&observation.players, &mut candidates)?;
    let decks = materialize_decks(observation.deck_remaining, &mut candidates)?;
    // 所有候选必须分配完毕，否则观察不自洽。
    if candidates.iter().any(|cards| !cards.is_empty()) {
        return Err(AiError::InvalidObservation("unallocated cards remain"));
    }

    let card_store = CardStore::from_cards(&standard);
    let nobles = standard_nobles();
    let game = GameState {
        players,
        bank: Bank {
            tokens: observation.bank,
        },
        decks,
        market: materialize_market(&observation.market),
        nobles: NobleBoard {
            available: observation.nobles_available.clone(),
            taken: observation.nobles_taken.clone(),
        },
        card_store,
        noble_store: NobleStore::from_nobles(&nobles),
        current_player: observation.current_player,
        round_start_player: observation.round_start_player,
        end_triggered: observation.end_triggered,
        winner: observation.winner,
        final_player: observation.final_player,
    };
    Ok(SimulationState::new(game, context))
}

fn level_index(level: CardLevel) -> usize {
    match level {
        CardLevel::Level1 => 0,
        CardLevel::Level2 => 1,
        CardLevel::Level3 => 2,
    }
}

fn materialize_players(
    observed: &[super::observation::ObservedPlayer],
    candidates: &mut [Vec<DevelopmentCard>; 3],
) -> Result<Vec<PlayerState>, AiError> {
    let mut players = Vec::with_capacity(observed.len());
    for op in observed {
        let mut reserved_cards = Vec::with_capacity(op.reserved.len());
        for reservation in &op.reserved {
            let card = match reservation {
                ObservedReservation::Known(known) => *known,
                ObservedReservation::HiddenBlind(level) => {
                    let pool = &mut candidates[level_index(*level)];
                    let sampled = pool.pop().ok_or(AiError::InvalidObservation(
                        "not enough hidden cards to fill blind reservations",
                    ))?;
                    ReservedCard::new(sampled.id, ReserveOrigin::BlindDeck(*level))
                }
            };
            reserved_cards.push(card);
        }
        let mut player = PlayerState::new(op.id);
        player.tokens = op.tokens;
        player.reserved_cards = reserved_cards;
        player.purchased_cards = op.purchased_cards.clone();
        player.nobles = op.nobles.clone();
        players.push(player);
    }
    Ok(players)
}

fn materialize_decks(
    deck_remaining: [usize; 3],
    candidates: &mut [Vec<DevelopmentCard>; 3],
) -> Result<CardDecks, AiError> {
    let take =
        |pool: &mut Vec<DevelopmentCard>, count: usize| -> Result<Vec<DevelopmentCard>, AiError> {
            if pool.len() < count {
                return Err(AiError::InvalidObservation(
                    "not enough cards to rebuild deck",
                ));
            }
            let split_off = pool.len() - count;
            Ok(pool.split_off(split_off))
        };
    Ok(CardDecks {
        level1: take(&mut candidates[0], deck_remaining[0])?,
        level2: take(&mut candidates[1], deck_remaining[1])?,
        level3: take(&mut candidates[2], deck_remaining[2])?,
    })
}

fn materialize_market(market: &[Vec<DevelopmentCard>; 3]) -> Market {
    Market {
        level1_visible: market[0].clone(),
        level2_visible: market[1].clone(),
        level3_visible: market[2].clone(),
    }
}

/// 各玩家自己持有的盲抽保留牌身份账本。
///
/// 行动者相对重采样时，用账本恢复该行动者的 `Known` 盲抽条目，使后续采样不能抹去
/// 早先行动者已确认的私有知识。当某玩家盲抽或购入盲抽保留牌时，只更新该玩家账本。
#[derive(Clone, Debug)]
pub struct PrivateKnowledge {
    blind_by_player: Vec<Vec<ReservedCard>>,
}

impl PrivateKnowledge {
    /// 从完整状态快照各玩家的盲抽保留牌（按槽位顺序）。
    pub fn from_state(state: &GameState) -> Self {
        let blind_by_player = state
            .players
            .iter()
            .map(|player| {
                player
                    .reserved_cards
                    .iter()
                    .copied()
                    .filter(|card| matches!(card.origin, ReserveOrigin::BlindDeck(_)))
                    .collect()
            })
            .collect();
        Self { blind_by_player }
    }

    /// 在 `actor` 视角重新采样不可见信息，但保留各玩家账本中的盲抽牌身份。
    ///
    /// 流程：从当前公开状态构造 `actor` 的脱敏观察 → 用账本把该 `actor` 的盲抽条目
    /// 覆写为账本记录的身份（槽位顺序）→ 调用 `determinize` → 把新世界拷回
    /// `simulation.game`，保留 `simulation.context`。
    ///
    /// 注意：账本用 `Known` 覆写该 `actor` 的盲抽条目（即便观察已将其标为 `Known`，
    /// 也以账本值为准），使先前为其他行动者采样时对游戏状态的改动不会污染该
    /// 行动者的私有知识。
    pub fn redeterminize_for_actor<R: Rng + ?Sized>(
        &self,
        simulation: &mut SimulationState,
        actor: PlayerId,
        rng: &mut R,
    ) -> Result<(), AiError> {
        let mut observation = AiObservation::from_game(&simulation.game, actor);
        // 用账本覆写 actor 自己的盲抽牌身份（槽位顺序对应）。
        let ledger = self.blind_by_player.get(actor).cloned().unwrap_or_default();
        restore_actor_blind(&mut observation, actor, &ledger);

        let context = simulation.context.clone();
        let fresh = determinize(&observation, context, rng)?;
        simulation.game = fresh.game;
        Ok(())
    }
}

/// 用账本覆写 `actor` 视角下该 `actor` 自己的盲抽保留牌身份。
///
/// 对每个 `HiddenBlind` 或 `Known(BlindDeck)` 条目，按槽位顺序用账本中的 `ReservedCard`
/// 替换为 `Known`。等级不匹配时保留原观察（保守）。这样即便先前为其他行动者采样
/// 改动了游戏状态，该行动者的私有盲抽牌身份始终由账本锚定。
fn restore_actor_blind(observation: &mut AiObservation, actor: PlayerId, ledger: &[ReservedCard]) {
    let observed_player = &mut observation.players[actor];
    let mut ledger_iter = ledger.iter().copied();
    for reservation in observed_player.reserved.iter_mut() {
        let level = match reservation {
            ObservedReservation::HiddenBlind(level) => Some(*level),
            ObservedReservation::Known(card) => match card.origin {
                ReserveOrigin::BlindDeck(level) => Some(level),
                ReserveOrigin::Market => None,
            },
        };
        let Some(level) = level else { continue };
        if let Some(known) = ledger_iter.next() {
            if matches!(known.origin, ReserveOrigin::BlindDeck(l) if l == level) {
                *reservation = ObservedReservation::Known(known);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::decision::DecisionContext;
    use crate::ai::observation::AiObservation;
    use crate::rules::{CardLevel, GameState, PlayerId, ReserveOrigin, ReservedCard};
    use rand::SeedableRng;
    use rand::rngs::StdRng;
    use std::collections::HashSet;

    #[test]
    fn determinization_preserves_every_card_exactly_once() {
        let game = game_with_opponent_blind_reservation();
        let observation = AiObservation::from_game(&game, 0);
        let mut rng = StdRng::seed_from_u64(31);
        let simulation = determinize(&observation, DecisionContext::MainTurn, &mut rng).unwrap();
        let ids = all_located_card_ids(&simulation.game);
        assert_eq!(ids.len(), 90);
        let unique: HashSet<_> = ids.iter().copied().collect();
        assert_eq!(unique.len(), 90);
        assert_eq!(
            simulation.game.decks.remaining(CardLevel::Level1),
            observation.deck_remaining[0]
        );
    }

    #[test]
    fn different_seeds_change_hidden_world_but_not_public_state() {
        let game = game_with_opponent_blind_reservation();
        let observation = AiObservation::from_game(&game, 0);
        let mut a = StdRng::seed_from_u64(1);
        let mut b = StdRng::seed_from_u64(2);
        let left = determinize(&observation, DecisionContext::MainTurn, &mut a).unwrap();
        let right = determinize(&observation, DecisionContext::MainTurn, &mut b).unwrap();
        assert_ne!(left.game.decks.level1, right.game.decks.level1);
        assert_eq!(
            left.game.market.level1_visible,
            right.game.market.level1_visible
        );
        assert_eq!(left.game.bank, right.game.bank);
    }

    fn game_with_opponent_blind_reservation() -> GameState {
        let mut game = GameState::new_seeded(2, 29).unwrap();
        let card = game.decks.pop(CardLevel::Level1).unwrap();
        game.players[1].reserved_cards.push(ReservedCard::new(
            card.id,
            ReserveOrigin::BlindDeck(CardLevel::Level1),
        ));
        game
    }

    fn all_located_card_ids(game: &GameState) -> Vec<CardId> {
        let mut ids = Vec::new();
        for level in CardLevel::ALL {
            ids.extend(game.market.visible(level).iter().map(|card| card.id));
            ids.extend(game.decks.deck(level).iter().map(|card| card.id));
        }
        for player in &game.players {
            ids.extend(player.reserved_cards.iter().map(|card| card.card_id));
            ids.extend(player.purchased_cards.iter().copied());
        }
        ids
    }

    #[test]
    fn redeterminize_for_actor_preserves_own_blind_reservation() {
        // 玩家 1 持有一张盲抽牌；玩家 0 没有。先做一次根 determinization（观察者 0），
        // 此时玩家 1 的盲抽牌被重新采样。之后在玩家 1 → 0 → 1 的相对重采样中，
        // 玩家 1 自己的盲抽牌身份应被账本稳定恢复（两次玩家 1 视角看到同一 id），
        // 而玩家 0 不可见的牌可能变化。卡牌守恒始终成立。
        let game = game_with_opponent_blind_reservation();
        let observation = AiObservation::from_game(&game, 0);
        let mut rng = StdRng::seed_from_u64(7);
        let mut simulation =
            determinize(&observation, DecisionContext::MainTurn, &mut rng).unwrap();
        let knowledge = PrivateKnowledge::from_state(&simulation.game);

        // 账本记录的是根 determinization 后玩家 1 的盲抽牌身份。
        let ledger_blind_id = simulation.game.players[1].reserved_cards[0].card_id;

        knowledge
            .redeterminize_for_actor(&mut simulation, 1, &mut rng)
            .unwrap();
        // 玩家 1 视角：自己的盲抽牌应被账本恢复为 ledger 值。
        let after_first = simulation.game.players[1].reserved_cards[0].card_id;
        assert_eq!(after_first, ledger_blind_id);

        knowledge
            .redeterminize_for_actor(&mut simulation, 0, &mut rng)
            .unwrap();
        knowledge
            .redeterminize_for_actor(&mut simulation, 1, &mut rng)
            .unwrap();
        // 再次回到玩家 1 视角：自己的盲抽牌身份不变。
        let after_second = simulation.game.players[1].reserved_cards[0].card_id;
        assert_eq!(after_second, ledger_blind_id);

        // 卡牌守恒不变。
        let ids = all_located_card_ids(&simulation.game);
        let unique: HashSet<_> = ids.iter().copied().collect();
        assert_eq!(ids.len(), 90);
        assert_eq!(unique.len(), 90);
    }

    #[test]
    fn determinize_is_card_conserving_without_reservations() {
        let game = GameState::new_seeded(2, 5).unwrap();
        let observation = AiObservation::from_game(&game, 0);
        let mut rng = StdRng::seed_from_u64(13);
        let simulation = determinize(&observation, DecisionContext::MainTurn, &mut rng).unwrap();
        let ids = all_located_card_ids(&simulation.game);
        let unique: HashSet<_> = ids.iter().copied().collect();
        assert_eq!(ids.len(), 90);
        assert_eq!(unique.len(), 90);
        // 公开状态应与观察一致。
        assert_eq!(simulation.game.bank.tokens, observation.bank);
        assert_eq!(simulation.game.market.level1_visible, observation.market[0]);
    }

    #[test]
    fn redeterminize_preserves_context() {
        let game = game_with_opponent_blind_reservation();
        let observation = AiObservation::from_game(&game, 0);
        let mut rng = StdRng::seed_from_u64(19);
        let mut simulation =
            determinize(&observation, DecisionContext::MainTurn, &mut rng).unwrap();
        let knowledge = PrivateKnowledge::from_state(&simulation.game);
        knowledge
            .redeterminize_for_actor(&mut simulation, 0, &mut rng)
            .unwrap();
        assert_eq!(simulation.context, DecisionContext::MainTurn);
    }
}
