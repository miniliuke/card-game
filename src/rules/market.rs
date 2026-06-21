//! 牌堆与公共市场。

use crate::rules::card::{CardId, CardLevel, DevelopmentCard};

const VISIBLE_PER_LEVEL: usize = 4;

#[derive(Clone, Debug)]
pub struct CardDecks {
    pub level1: Vec<DevelopmentCard>,
    pub level2: Vec<DevelopmentCard>,
    pub level3: Vec<DevelopmentCard>,
}

impl CardDecks {
    pub fn deck_mut(&mut self, level: CardLevel) -> &mut Vec<DevelopmentCard> {
        match level {
            CardLevel::Level1 => &mut self.level1,
            CardLevel::Level2 => &mut self.level2,
            CardLevel::Level3 => &mut self.level3,
        }
    }

    pub fn deck(&self, level: CardLevel) -> &Vec<DevelopmentCard> {
        match level {
            CardLevel::Level1 => &self.level1,
            CardLevel::Level2 => &self.level2,
            CardLevel::Level3 => &self.level3,
        }
    }

    pub fn pop(&mut self, level: CardLevel) -> Option<DevelopmentCard> {
        self.deck_mut(level).pop()
    }

    pub fn remaining(&self, level: CardLevel) -> usize {
        self.deck(level).len()
    }
}

/// 公共市场。每等级最多 4 张可见卡，存完整卡牌（取卡免查表）。
#[derive(Clone, Debug, Default)]
pub struct Market {
    pub level1_visible: Vec<DevelopmentCard>,
    pub level2_visible: Vec<DevelopmentCard>,
    pub level3_visible: Vec<DevelopmentCard>,
}

impl Market {
    pub fn visible(&self, level: CardLevel) -> &[DevelopmentCard] {
        match level {
            CardLevel::Level1 => &self.level1_visible,
            CardLevel::Level2 => &self.level2_visible,
            CardLevel::Level3 => &self.level3_visible,
        }
    }

    pub fn visible_mut(&mut self, level: CardLevel) -> &mut Vec<DevelopmentCard> {
        match level {
            CardLevel::Level1 => &mut self.level1_visible,
            CardLevel::Level2 => &mut self.level2_visible,
            CardLevel::Level3 => &mut self.level3_visible,
        }
    }

    /// 取指定等级第 idx 张（0-based）。idx 越界返回 None。
    pub fn take(&mut self, level: CardLevel, idx: usize) -> Option<DevelopmentCard> {
        let v = self.visible_mut(level);
        if idx >= v.len() {
            return None;
        }
        Some(v.remove(idx))
    }

    /// 立即从对应牌堆补一张到 4 张。返回新补入的 CardId（若补了）。
    /// 符合 rules.md §5：购买/保留可见卡后立即补牌。
    pub fn refill(&mut self, level: CardLevel, deck: &mut CardDecks) -> Option<CardId> {
        let v = self.visible_mut(level);
        while v.len() < VISIBLE_PER_LEVEL {
            match deck.pop(level) {
                Some(card) => {
                    let id = card.id;
                    v.push(card);
                    return Some(id); // 每次只补一张（取走一张只需补一张）
                }
                None => return None,
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::card::GemCost;
    use crate::rules::color::CardColor;

    fn card(id: CardId, level: CardLevel) -> DevelopmentCard {
        DevelopmentCard { id, level, color: CardColor::White, prestige: 0, cost: GemCost::default() }
    }

    #[test]
    fn take_removes_by_index() {
        let mut m = Market::default();
        m.level1_visible = vec![card(1, CardLevel::Level1), card(2, CardLevel::Level1)];
        let taken = m.take(CardLevel::Level1, 0).unwrap();
        assert_eq!(taken.id, 1);
        assert_eq!(m.visible(CardLevel::Level1).len(), 1);
    }

    #[test]
    fn refill_pulls_from_deck_until_four() {
        let mut deck = CardDecks { level1: vec![card(5, CardLevel::Level1), card(6, CardLevel::Level1)], level2: vec![], level3: vec![] };
        let mut m = Market { level1_visible: vec![card(1, CardLevel::Level1), card(2, CardLevel::Level1), card(3, CardLevel::Level1)], level2_visible: vec![], level3_visible: vec![] };
        // Vec::pop 取尾部为牌堆顶，故补入 id=6。
        let id = m.refill(CardLevel::Level1, &mut deck);
        assert_eq!(id, Some(6));
        assert_eq!(m.visible(CardLevel::Level1).len(), 4);
    }

    #[test]
    fn refill_returns_none_when_deck_empty_and_not_full() {
        let mut deck = CardDecks { level1: vec![], level2: vec![], level3: vec![] };
        let mut m = Market { level1_visible: vec![card(1, CardLevel::Level1)], level2_visible: vec![], level3_visible: vec![] };
        assert_eq!(m.refill(CardLevel::Level1, &mut deck), None);
        assert_eq!(m.visible(CardLevel::Level1).len(), 1);
    }
}
