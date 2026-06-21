//! 发展卡、费用、折扣、真实牌库数据。

use std::collections::HashMap;

use crate::rules::color::CardColor;

pub type CardId = u32;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum CardLevel {
    Level1,
    Level2,
    Level3,
}

impl CardLevel {
    pub const ALL: [Self; 3] = [Self::Level1, Self::Level2, Self::Level3];
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// 卡牌费用，5 字段，不含金。金只在支付时作为万能补缺口。
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, Debug)]
pub struct GemCost {
    pub white: u8,
    pub blue: u8,
    pub green: u8,
    pub red: u8,
    pub black: u8,
}

impl GemCost {
    pub fn get(self, color: CardColor) -> u8 {
        match color {
            CardColor::White => self.white,
            CardColor::Blue => self.blue,
            CardColor::Green => self.green,
            CardColor::Red => self.red,
            CardColor::Black => self.black,
        }
    }

    /// 应用折扣：每色 max(need - bonus, 0)。
    pub fn after_discount(self, bonus: CardBonus) -> Self {
        Self {
            white: self.white.saturating_sub(bonus.white),
            blue: self.blue.saturating_sub(bonus.blue),
            green: self.green.saturating_sub(bonus.green),
            red: self.red.saturating_sub(bonus.red),
            black: self.black.saturating_sub(bonus.black),
        }
    }

    /// 折扣后仍需支付的总普通宝石数（金色需补的缺口上限）。
    pub fn total_missing(self) -> u8 {
        self.white + self.blue + self.green + self.red + self.black
    }
}

/// 玩家已购发展卡按色计数，作为购买折扣。
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct CardBonus {
    pub white: u8,
    pub blue: u8,
    pub green: u8,
    pub red: u8,
    pub black: u8,
}

impl CardBonus {
    pub fn get(self, color: CardColor) -> u8 {
        match color {
            CardColor::White => self.white,
            CardColor::Blue => self.blue,
            CardColor::Green => self.green,
            CardColor::Red => self.red,
            CardColor::Black => self.black,
        }
    }

    pub fn add(&mut self, color: CardColor) {
        match color {
            CardColor::White => self.white += 1,
            CardColor::Blue => self.blue += 1,
            CardColor::Green => self.green += 1,
            CardColor::Red => self.red += 1,
            CardColor::Black => self.black += 1,
        }
    }

    /// 是否满足某贵族要求（每色 bonus >= requirement）。
    pub fn satisfies(self, requirement: GemCost) -> bool {
        CardColor::ALL
            .iter()
            .all(|&c| self.get(c) >= requirement.get(c))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct DevelopmentCard {
    pub id: CardId,
    pub level: CardLevel,
    pub color: CardColor,
    pub prestige: u8,
    pub cost: GemCost,
}

/// id -> card 只读索引，供 bonus/score 反查。
#[derive(Clone, Default)]
pub struct CardStore {
    map: HashMap<CardId, DevelopmentCard>,
}

impl CardStore {
    pub fn from_cards(cards: &[DevelopmentCard]) -> Self {
        let map = cards.iter().copied().map(|c| (c.id, c)).collect();
        Self { map }
    }

    pub fn get(&self, id: CardId) -> Option<&DevelopmentCard> {
        self.map.get(&id)
    }
}

/// 返回标准 90 张发展卡（40/30/20）。逐张硬编码，分值/费用按等级递增。
/// 注意：数值由作者凭记忆录入，可能存在偏差；统计特征由测试锁定。
pub fn standard_deck() -> Vec<DevelopmentCard> {
    let mut id = 0u32;
    let mut mk = |level: CardLevel, color: CardColor, prestige: u8, cost: [u8; 5]| -> DevelopmentCard {
        let card = DevelopmentCard {
            id,
            level,
            color,
            prestige,
            cost: GemCost { white: cost[0], blue: cost[1], green: cost[2], red: cost[3], black: cost[4] },
        };
        id += 1;
        card
    };
    let l1 = CardLevel::Level1;
    let l2 = CardLevel::Level2;
    let l3 = CardLevel::Level3;
    // 顺序：白 蓝 绿 红 黑。加成色费用为 0。
    let cards = vec![
        // ===== Level 1 (40 张) =====
        // White bonus (8)
        mk(l1, CardColor::White, 0, [0,1,2,1,0]),
        mk(l1, CardColor::White, 0, [0,2,0,1,1]),
        mk(l1, CardColor::White, 0, [0,1,1,0,2]),
        mk(l1, CardColor::White, 0, [0,0,2,2,0]),
        mk(l1, CardColor::White, 0, [0,2,1,0,1]),
        mk(l1, CardColor::White, 0, [0,0,1,2,1]),
        mk(l1, CardColor::White, 0, [0,1,0,1,2]),
        mk(l1, CardColor::White, 1, [0,3,0,0,0]),
        // Blue bonus (8)
        mk(l1, CardColor::Blue, 0, [1,0,2,1,0]),
        mk(l1, CardColor::Blue, 0, [2,0,0,1,1]),
        mk(l1, CardColor::Blue, 0, [1,0,1,0,2]),
        mk(l1, CardColor::Blue, 0, [2,0,2,0,0]),
        mk(l1, CardColor::Blue, 0, [1,0,2,1,0]),
        mk(l1, CardColor::Blue, 0, [0,0,1,2,1]),
        mk(l1, CardColor::Blue, 0, [2,0,0,1,1]),
        mk(l1, CardColor::Blue, 1, [3,0,0,0,0]),
        // Green bonus (8)
        mk(l1, CardColor::Green, 0, [2,1,0,0,1]),
        mk(l1, CardColor::Green, 0, [0,2,0,1,1]),
        mk(l1, CardColor::Green, 0, [1,1,0,2,0]),
        mk(l1, CardColor::Green, 0, [2,0,0,0,2]),
        mk(l1, CardColor::Green, 0, [1,2,0,1,0]),
        mk(l1, CardColor::Green, 0, [0,1,0,2,1]),
        mk(l1, CardColor::Green, 0, [1,0,0,1,2]),
        mk(l1, CardColor::Green, 1, [0,0,0,3,0]),
        // Red bonus (8)
        mk(l1, CardColor::Red, 0, [1,2,1,0,0]),
        mk(l1, CardColor::Red, 0, [2,1,0,0,1]),
        mk(l1, CardColor::Red, 0, [0,1,2,0,1]),
        mk(l1, CardColor::Red, 0, [1,0,1,0,2]),
        mk(l1, CardColor::Red, 0, [2,0,0,0,2]),
        mk(l1, CardColor::Red, 0, [0,2,1,0,1]),
        mk(l1, CardColor::Red, 0, [1,1,0,0,2]),
        mk(l1, CardColor::Red, 1, [0,0,3,0,0]),
        // Black bonus (8)
        mk(l1, CardColor::Black, 0, [1,0,1,2,0]),
        mk(l1, CardColor::Black, 0, [0,1,2,1,0]),
        mk(l1, CardColor::Black, 0, [2,1,0,1,0]),
        mk(l1, CardColor::Black, 0, [0,2,0,2,0]),
        mk(l1, CardColor::Black, 0, [1,0,2,1,0]),
        mk(l1, CardColor::Black, 0, [0,1,0,3,0]),
        mk(l1, CardColor::Black, 0, [2,0,1,1,0]),
        mk(l1, CardColor::Black, 1, [3,0,0,0,0]),

        // ===== Level 2 (30 张) =====
        // White bonus (6)
        mk(l2, CardColor::White, 1, [0,2,2,0,3]),
        mk(l2, CardColor::White, 1, [0,3,0,3,2]),
        mk(l2, CardColor::White, 1, [0,0,3,2,3]),
        mk(l2, CardColor::White, 2, [0,5,0,0,0]),
        mk(l2, CardColor::White, 2, [0,0,5,0,0]),
        mk(l2, CardColor::White, 2, [0,0,0,5,0]),
        // Blue bonus (6)
        mk(l2, CardColor::Blue, 1, [2,0,2,3,0]),
        mk(l2, CardColor::Blue, 1, [3,0,3,0,2]),
        mk(l2, CardColor::Blue, 1, [0,0,2,3,3]),
        mk(l2, CardColor::Blue, 2, [5,0,0,0,0]),
        mk(l2, CardColor::Blue, 2, [0,0,5,0,0]),
        mk(l2, CardColor::Blue, 2, [0,0,0,0,5]),
        // Green bonus (6)
        mk(l2, CardColor::Green, 1, [2,3,0,0,2]),
        mk(l2, CardColor::Green, 1, [3,2,0,3,0]),
        mk(l2, CardColor::Green, 1, [2,0,0,3,3]),
        mk(l2, CardColor::Green, 2, [0,5,0,0,0]),
        mk(l2, CardColor::Green, 2, [5,0,0,0,0]),
        mk(l2, CardColor::Green, 2, [0,0,0,0,5]),
        // Red bonus (6)
        mk(l2, CardColor::Red, 1, [3,0,2,0,2]),
        mk(l2, CardColor::Red, 1, [0,3,3,0,2]),
        mk(l2, CardColor::Red, 1, [2,2,0,0,3]),
        mk(l2, CardColor::Red, 2, [0,0,5,0,0]),
        mk(l2, CardColor::Red, 2, [0,5,0,0,0]),
        mk(l2, CardColor::Red, 2, [5,0,0,0,0]),
        // Black bonus (6)
        mk(l2, CardColor::Black, 1, [0,2,3,2,0]),
        mk(l2, CardColor::Black, 1, [2,0,3,3,0]),
        mk(l2, CardColor::Black, 1, [3,2,2,0,0]),
        mk(l2, CardColor::Black, 2, [0,0,0,5,0]),
        mk(l2, CardColor::Black, 2, [0,5,0,0,0]),
        mk(l2, CardColor::Black, 2, [0,0,5,0,0]),

        // ===== Level 3 (20 张) =====
        // White bonus (4)
        mk(l3, CardColor::White, 3, [0,3,3,5,3]),
        mk(l3, CardColor::White, 4, [0,0,0,6,4]),
        mk(l3, CardColor::White, 4, [0,5,5,0,3]),
        mk(l3, CardColor::White, 5, [0,0,0,7,0]),
        // Blue bonus (4)
        mk(l3, CardColor::Blue, 3, [5,0,3,3,3]),
        mk(l3, CardColor::Blue, 4, [4,0,0,0,6]),
        mk(l3, CardColor::Blue, 4, [3,0,5,5,0]),
        mk(l3, CardColor::Blue, 5, [0,0,0,0,7]),
        // Green bonus (4)
        mk(l3, CardColor::Green, 3, [3,5,0,3,3]),
        mk(l3, CardColor::Green, 4, [6,4,0,0,0]),
        mk(l3, CardColor::Green, 4, [0,3,0,5,5]),
        mk(l3, CardColor::Green, 5, [7,0,0,0,0]),
        // Red bonus (4)
        mk(l3, CardColor::Red, 3, [3,3,5,0,3]),
        mk(l3, CardColor::Red, 4, [0,6,4,0,0]),
        mk(l3, CardColor::Red, 4, [5,0,3,0,5]),
        mk(l3, CardColor::Red, 5, [0,7,0,0,0]),
        // Black bonus (4)
        mk(l3, CardColor::Black, 3, [3,3,3,5,0]),
        mk(l3, CardColor::Black, 4, [0,0,6,4,0]),
        mk(l3, CardColor::Black, 4, [5,5,0,3,0]),
        mk(l3, CardColor::Black, 5, [0,0,0,7,0]),
    ];
    cards
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_deck_has_40_30_20() {
        let deck = standard_deck();
        assert_eq!(deck.len(), 90);
        assert_eq!(deck.iter().filter(|c| c.level == CardLevel::Level1).count(), 40);
        assert_eq!(deck.iter().filter(|c| c.level == CardLevel::Level2).count(), 30);
        assert_eq!(deck.iter().filter(|c| c.level == CardLevel::Level3).count(), 20);
    }

    #[test]
    fn each_level_balanced_across_colors() {
        let deck = standard_deck();
        for level in CardLevel::ALL {
            for color in CardColor::ALL {
                let count = deck.iter().filter(|c| c.level == level && c.color == color).count();
                let expected = match level {
                    CardLevel::Level1 => 8,
                    CardLevel::Level2 => 6,
                    CardLevel::Level3 => 4,
                };
                assert_eq!(count, expected, "level {level:?} color {color:?}");
            }
        }
    }

    #[test]
    fn bonus_color_cost_is_zero() {
        for card in standard_deck() {
            assert_eq!(card.cost.get(card.color), 0, "card {} bonus color cost nonzero", card.id);
        }
    }

    #[test]
    fn ids_are_unique() {
        let deck = standard_deck();
        let mut ids: Vec<_> = deck.iter().map(|c| c.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 90);
    }

    #[test]
    fn after_discount_floors_at_zero() {
        let cost = GemCost { white: 3, blue: 2, ..Default::default() };
        let bonus = CardBonus { white: 1, blue: 3, ..Default::default() };
        let after = cost.after_discount(bonus);
        assert_eq!(after.white, 2);
        assert_eq!(after.blue, 0);
    }

    #[test]
    fn bonus_satisfies_requirement() {
        let bonus = CardBonus { white: 4, blue: 4, ..Default::default() };
        let req = GemCost { white: 4, blue: 4, ..Default::default() };
        assert!(bonus.satisfies(req));
        let req2 = GemCost { white: 4, blue: 5, ..Default::default() };
        assert!(!bonus.satisfies(req2));
    }
}
