//! 玩家状态：筹码、保留牌、已购牌、贵族、折扣、分数。

use crate::rules::card::{CardBonus, CardStore};
use crate::rules::color::{CardColor, GemColor, PlayerId};
use crate::rules::noble::{NobleId, NobleStore};
use crate::rules::token::TokenSet;

/// 保留牌上限。
pub const RESERVED_LIMIT: usize = 3;
/// 玩家筹码上限。
pub const TOKEN_LIMIT: u8 = 10;
/// 触发终局的分数。
pub const WIN_SCORE: u16 = 15;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PlayerState {
    pub id: PlayerId,
    pub tokens: TokenSet,
    pub reserved_cards: Vec<crate::rules::card::CardId>,
    pub purchased_cards: Vec<crate::rules::card::CardId>,
    pub nobles: Vec<NobleId>,
}

impl PlayerState {
    pub fn new(id: PlayerId) -> Self {
        Self {
            id,
            tokens: TokenSet::default(),
            reserved_cards: Vec::new(),
            purchased_cards: Vec::new(),
            nobles: Vec::new(),
        }
    }

    /// 已购发展卡按色计数 = 购买折扣。
    pub fn bonus(&self, store: &CardStore) -> CardBonus {
        let mut bonus = CardBonus::default();
        for &id in &self.purchased_cards {
            if let Some(card) = store.get(id) {
                bonus.add(card.color);
            }
        }
        bonus
    }

    pub fn token_count(&self, color: GemColor) -> u8 {
        self.tokens.get(color)
    }

    pub fn token_total(&self) -> u8 {
        self.tokens.total()
    }

    pub fn reserved_full(&self) -> bool {
        self.reserved_cards.len() >= RESERVED_LIMIT
    }

    /// 卡分 + 贵族分。
    pub fn score(&self, cards: &CardStore, nobles: &NobleStore) -> u16 {
        let card_score: u16 = self
            .purchased_cards
            .iter()
            .filter_map(|id| store_get_prestige(cards, *id))
            .sum();
        let noble_score: u16 = self
            .nobles
            .iter()
            .filter_map(|id| store_get_noble_prestige(nobles, *id))
            .sum();
        card_score + noble_score
    }
}

fn store_get_prestige(store: &CardStore, id: crate::rules::card::CardId) -> Option<u16> {
    store.get(id).map(|c| u16::from(c.prestige))
}

fn store_get_noble_prestige(store: &NobleStore, id: NobleId) -> Option<u16> {
    store.get(id).map(|n| u16::from(n.prestige))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::card::{DevelopmentCard, GemCost};
    use crate::rules::noble::Noble;

    fn store_with(cards: &[DevelopmentCard]) -> CardStore {
        CardStore::from_cards(cards)
    }

    #[test]
    fn bonus_counts_purchased_by_color() {
        let c = DevelopmentCard { id: 1, level: crate::rules::card::CardLevel::Level1, color: CardColor::White, prestige: 1, cost: GemCost::default() };
        let store = store_with(&[c]);
        let mut p = PlayerState::new(0);
        p.purchased_cards.push(1);
        p.purchased_cards.push(1);
        let bonus = p.bonus(&store);
        assert_eq!(bonus.white, 2);
        assert_eq!(bonus.blue, 0);
    }

    #[test]
    fn score_sums_cards_and_nobles() {
        let c = DevelopmentCard { id: 1, level: crate::rules::card::CardLevel::Level1, color: CardColor::White, prestige: 2, cost: GemCost::default() };
        let n = Noble { id: 0, prestige: 3, requirement: GemCost::default() };
        let store = store_with(&[c]);
        let nstore = NobleStore::from_nobles(&[n]);
        let mut p = PlayerState::new(0);
        p.purchased_cards.push(1);
        p.nobles.push(0);
        assert_eq!(p.score(&store, &nstore), 5);
    }

    #[test]
    fn reserved_full_at_three() {
        let mut p = PlayerState::new(0);
        assert!(!p.reserved_full());
        p.reserved_cards.extend([1, 2, 3]);
        assert!(p.reserved_full());
    }
}
