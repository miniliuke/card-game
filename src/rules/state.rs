//! 全局游戏状态与初始化。

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};

use crate::rules::card::{standard_deck, CardStore};
use crate::rules::color::PlayerId;
use crate::rules::error::RuleError;
use crate::rules::market::{CardDecks, Market};
use crate::rules::noble::{standard_nobles, NobleBoard, NobleStore};
use crate::rules::player::{PlayerState, TOKEN_LIMIT, WIN_SCORE};
use crate::rules::token::{Bank, TokenSet};

const VISIBLE_PER_LEVEL: usize = 4;

#[derive(Clone, Debug)]
pub struct GameState {
    pub players: Vec<PlayerState>,
    pub bank: Bank,
    pub decks: CardDecks,
    pub market: Market,
    pub nobles: NobleBoard,
    pub card_store: CardStore,
    pub noble_store: NobleStore,
    pub current_player: usize,
    pub round_start_player: usize,
    pub end_triggered: bool,
    pub winner: Option<PlayerId>,
    /// 终局轮中：当 current_player 走到此玩家的下一位时结算。
    pub final_player: Option<PlayerId>,
}

impl GameState {
    /// 按规则初始化（rules.md §4/§7/§9）。rand 仅在此使用。
    pub fn new<R: Rng + ?Sized>(player_count: usize, rng: &mut R) -> Result<Self, RuleError> {
        if !(2..=4).contains(&player_count) {
            return Err(RuleError::InvalidPlayerCount);
        }

        let deck = standard_deck();
        let card_store = CardStore::from_cards(&deck);

        let mut l1: Vec<_> = deck.iter().filter(|c| matches!(c.level, crate::rules::card::CardLevel::Level1)).copied().collect();
        let mut l2: Vec<_> = deck.iter().filter(|c| matches!(c.level, crate::rules::card::CardLevel::Level2)).copied().collect();
        let mut l3: Vec<_> = deck.iter().filter(|c| matches!(c.level, crate::rules::card::CardLevel::Level3)).copied().collect();
        l1.shuffle(rng);
        l2.shuffle(rng);
        l3.shuffle(rng);

        let mut decks = CardDecks { level1: l1, level2: l2, level3: l3 };
        let mut market = Market::default();
        for level in crate::rules::card::CardLevel::ALL {
            for _ in 0..VISIBLE_PER_LEVEL {
                if let Some(card) = decks.pop(level) {
                    market.visible_mut(level).push(card);
                }
            }
        }

        let mut nobles_pool = standard_nobles();
        nobles_pool.shuffle(rng);
        let noble_count = player_count + 1;
        let available = nobles_pool.into_iter().take(noble_count).collect();
        let nobles = NobleBoard { available, taken: vec![] };
        let noble_store = NobleStore::from_nobles(&standard_nobles());

        let normal_per_color = match player_count {
            2 => 4,
            3 => 5,
            4 => 7,
            _ => unreachable!(),
        };
        let bank = Bank {
            tokens: TokenSet {
                white: normal_per_color,
                blue: normal_per_color,
                green: normal_per_color,
                red: normal_per_color,
                black: normal_per_color,
                gold: 5,
            },
        };

        let players = (0..player_count).map(PlayerState::new).collect();

        Ok(Self {
            players,
            bank,
            decks,
            market,
            nobles,
            card_store,
            noble_store,
            current_player: 0,
            round_start_player: 0,
            end_triggered: false,
            winner: None,
            final_player: None,
        })
    }

    /// 便捷构造：固定 seed，用于测试/回放。
    pub fn new_seeded(player_count: usize, seed: u64) -> Result<Self, RuleError> {
        let mut rng = StdRng::seed_from_u64(seed);
        Self::new(player_count, &mut rng)
    }

    pub fn current(&self) -> &PlayerState {
        &self.players[self.current_player]
    }

    pub fn current_mut(&mut self) -> &mut PlayerState {
        &mut self.players[self.current_player]
    }

    pub fn current_id(&self) -> PlayerId {
        self.current_player
    }

    pub fn player(&self, id: PlayerId) -> &PlayerState {
        &self.players[id]
    }

    pub fn player_mut(&mut self, id: PlayerId) -> &mut PlayerState {
        &mut self.players[id]
    }

    pub fn current_score(&self) -> u16 {
        self.current().score(&self.card_store, &self.noble_store)
    }

    pub fn is_over(&self) -> bool {
        self.winner.is_some()
    }

    pub fn token_limit() -> u8 {
        TOKEN_LIMIT
    }

    pub fn win_score() -> u16 {
        WIN_SCORE
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::card::CardLevel;

    #[test]
    fn rejects_invalid_player_count() {
        assert_eq!(GameState::new_seeded(1, 1).unwrap_err(), RuleError::InvalidPlayerCount);
        assert_eq!(GameState::new_seeded(5, 1).unwrap_err(), RuleError::InvalidPlayerCount);
    }

    #[test]
    fn two_player_bank_is_four_per_color_and_five_gold() {
        let g = GameState::new_seeded(2, 1).unwrap();
        assert_eq!(g.bank.tokens.white, 4);
        assert_eq!(g.bank.tokens.gold, 5);
        assert_eq!(g.nobles.available.len(), 3);
    }

    #[test]
    fn four_player_bank_is_seven_and_five_nobles() {
        let g = GameState::new_seeded(4, 1).unwrap();
        assert_eq!(g.bank.tokens.red, 7);
        assert_eq!(g.nobles.available.len(), 5);
    }

    #[test]
    fn market_starts_with_four_per_level() {
        let g = GameState::new_seeded(3, 1).unwrap();
        for level in CardLevel::ALL {
            assert_eq!(g.market.visible(level).len(), 4);
        }
    }

    #[test]
    fn deck_remaining_plus_visible_equals_total() {
        let g = GameState::new_seeded(2, 1).unwrap();
        assert_eq!(g.decks.remaining(CardLevel::Level1) + g.market.visible(CardLevel::Level1).len(), 40);
        assert_eq!(g.decks.remaining(CardLevel::Level2) + g.market.visible(CardLevel::Level2).len(), 30);
        assert_eq!(g.decks.remaining(CardLevel::Level3) + g.market.visible(CardLevel::Level3).len(), 20);
    }
}
