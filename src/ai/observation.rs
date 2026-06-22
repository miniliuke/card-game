//! 脱敏的逐玩家观察与稳定信息集键。
//!
//! 后台任务只接收 `AiObservation`，绝不接收完整 `GameState`——这是安全约束而非
//! 编码约定。观察包含观察者可见的全部公开信息 + 观察者自己的私有信息；对手盲抽
//! 保留牌仅暴露等级，不含 `CardId`。`information_set_key` 只哈希观察者可见字段，
//! 使"只改变某玩家不可见的牌"不会改变该玩家的信息集键。

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::rules::{
    CardId, CardLevel, DevelopmentCard, GameState, Noble, NobleId, PlayerId, PlayerState,
    ReserveOrigin, ReservedCard, TokenSet,
};

use super::decision::DecisionContext;

/// 观察者看到的保留牌：已知（自己持有或市场来源）或仅知等级（对手盲抽）。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ObservedReservation {
    Known(ReservedCard),
    HiddenBlind(CardLevel),
}

/// 观察者看到的某玩家公开状态。
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ObservedPlayer {
    pub id: PlayerId,
    pub tokens: TokenSet,
    pub reserved: Vec<ObservedReservation>,
    pub purchased_cards: Vec<CardId>,
    pub nobles: Vec<NobleId>,
}

/// 脱敏后的完整观察。不含任何观察者不可见的 `CardId`（如对手盲抽牌身份、真实牌堆顺序）。
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AiObservation {
    pub observer: PlayerId,
    pub players: Vec<ObservedPlayer>,
    pub bank: TokenSet,
    pub market: [Vec<DevelopmentCard>; 3],
    pub deck_remaining: [usize; 3],
    pub nobles_available: Vec<Noble>,
    pub nobles_taken: Vec<NobleId>,
    pub current_player: PlayerId,
    pub round_start_player: PlayerId,
    pub end_triggered: bool,
    pub winner: Option<PlayerId>,
    pub final_player: Option<PlayerId>,
}

/// 信息集键：观察者可见字段的稳定哈希摘要。同等可见信息 → 同键。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct InfoSetKey(pub u64);

impl AiObservation {
    /// 从完整 `GameState` 构造 `observer` 视角的脱敏观察。
    /// 市场来源保留牌对所有观察者公开；盲抽保留牌仅对持有者（`owner == observer`）可见，
    /// 对其他人只暴露等级。
    pub fn from_game(game: &GameState, observer: PlayerId) -> Self {
        let players = game
            .players
            .iter()
            .map(|player| observe_player(player, observer))
            .collect();

        let market = [
            game.market.visible(CardLevel::Level1).to_vec(),
            game.market.visible(CardLevel::Level2).to_vec(),
            game.market.visible(CardLevel::Level3).to_vec(),
        ];
        let deck_remaining = [
            game.decks.remaining(CardLevel::Level1),
            game.decks.remaining(CardLevel::Level2),
            game.decks.remaining(CardLevel::Level3),
        ];

        Self {
            observer,
            players,
            bank: game.bank.tokens,
            market,
            deck_remaining,
            nobles_available: game.nobles.available.clone(),
            nobles_taken: game.nobles.taken.clone(),
            current_player: game.current_player,
            round_start_player: game.round_start_player,
            end_triggered: game.end_triggered,
            winner: game.winner,
            final_player: game.final_player,
        }
    }

    /// 哈希观察者可见的全部字段 + 决策上下文。不访问 `GameState` 或隐藏 `CardId`。
    pub fn information_set_key(&self, context: &DecisionContext) -> InfoSetKey {
        let mut hasher = DefaultHasher::new();
        self.observer.hash(&mut hasher);
        self.players.hash(&mut hasher);
        self.bank.hash(&mut hasher);
        self.market.hash(&mut hasher);
        self.deck_remaining.hash(&mut hasher);
        self.nobles_available.hash(&mut hasher);
        self.nobles_taken.hash(&mut hasher);
        self.current_player.hash(&mut hasher);
        self.round_start_player.hash(&mut hasher);
        self.end_triggered.hash(&mut hasher);
        self.winner.hash(&mut hasher);
        self.final_player.hash(&mut hasher);
        context.hash(&mut hasher);
        InfoSetKey(hasher.finish())
    }
}

fn observe_player(player: &PlayerState, observer: PlayerId) -> ObservedPlayer {
    let reserved = player
        .reserved_cards
        .iter()
        .copied()
        .map(|card| {
            if player.id == observer || card.is_public() {
                ObservedReservation::Known(card)
            } else {
                // 对手盲抽：只暴露等级，不含 CardId。
                let level = match card.origin {
                    ReserveOrigin::BlindDeck(level) => level,
                    ReserveOrigin::Market => unreachable!("market reservations are public"),
                };
                ObservedReservation::HiddenBlind(level)
            }
        })
        .collect();

    ObservedPlayer {
        id: player.id,
        tokens: player.tokens,
        reserved,
        purchased_cards: player.purchased_cards.clone(),
        nobles: player.nobles.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{GameState, ReserveOrigin, ReservedCard};

    #[test]
    fn opponent_blind_card_is_redacted_but_own_card_is_known() {
        let mut game = GameState::new_seeded(2, 17).unwrap();
        let blind = game.decks.pop(CardLevel::Level1).unwrap();
        game.players[1].reserved_cards.push(ReservedCard::new(
            blind.id,
            ReserveOrigin::BlindDeck(CardLevel::Level1),
        ));

        let human = AiObservation::from_game(&game, 0);
        let cpu = AiObservation::from_game(&game, 1);
        assert_eq!(
            human.players[1].reserved[0],
            ObservedReservation::HiddenBlind(CardLevel::Level1)
        );
        assert_eq!(
            cpu.players[1].reserved[0],
            ObservedReservation::Known(ReservedCard::new(
                blind.id,
                ReserveOrigin::BlindDeck(CardLevel::Level1),
            ))
        );
    }

    #[test]
    fn changing_an_unseen_card_does_not_change_the_information_set_key() {
        let mut left = GameState::new_seeded(2, 21).unwrap();
        let mut right = left.clone();
        let left_card = left.decks.pop(CardLevel::Level1).unwrap();
        let right_card = right.decks.level1.remove(0);
        left.players[1].reserved_cards.push(ReservedCard::new(
            left_card.id,
            ReserveOrigin::BlindDeck(CardLevel::Level1),
        ));
        right.players[1].reserved_cards.push(ReservedCard::new(
            right_card.id,
            ReserveOrigin::BlindDeck(CardLevel::Level1),
        ));
        let context = DecisionContext::MainTurn;
        assert_eq!(
            AiObservation::from_game(&left, 0).information_set_key(&context),
            AiObservation::from_game(&right, 0).information_set_key(&context),
        );
    }

    #[test]
    fn market_reservation_is_visible_to_both_players() {
        let mut game = GameState::new_seeded(2, 23).unwrap();
        let market_card = game.market.visible(CardLevel::Level1)[0];
        game.players[1]
            .reserved_cards
            .push(ReservedCard::new(market_card.id, ReserveOrigin::Market));
        let human = AiObservation::from_game(&game, 0);
        assert_eq!(
            human.players[1].reserved[0],
            ObservedReservation::Known(ReservedCard::new(market_card.id, ReserveOrigin::Market))
        );
    }
}
