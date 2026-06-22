//! 筹码集合与公共银行。

use crate::rules::color::{CardColor, GemColor};

#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, Debug)]
pub struct TokenSet {
    pub white: u8,
    pub blue: u8,
    pub green: u8,
    pub red: u8,
    pub black: u8,
    pub gold: u8,
}

impl TokenSet {
    pub fn get(self, color: GemColor) -> u8 {
        match color {
            GemColor::White => self.white,
            GemColor::Blue => self.blue,
            GemColor::Green => self.green,
            GemColor::Red => self.red,
            GemColor::Black => self.black,
            GemColor::Gold => self.gold,
        }
    }

    pub fn set(&mut self, color: GemColor, value: u8) {
        *self.field_mut(color) = value;
    }

    pub fn add(&mut self, color: GemColor, amount: u8) {
        *self.field_mut(color) = self.get(color).saturating_add(amount);
    }

    /// 扣减；若不足返回 false 且不改动。
    pub fn remove(&mut self, color: GemColor, amount: u8) -> bool {
        let cur = self.get(color);
        if cur < amount {
            return false;
        }
        *self.field_mut(color) = cur - amount;
        true
    }

    pub fn total(self) -> u8 {
        self.white + self.blue + self.green + self.red + self.black + self.gold
    }

    /// 两集合逐色相加（饱和）。
    pub fn combine(self, other: Self) -> Self {
        Self {
            white: self.white.saturating_add(other.white),
            blue: self.blue.saturating_add(other.blue),
            green: self.green.saturating_add(other.green),
            red: self.red.saturating_add(other.red),
            black: self.black.saturating_add(other.black),
            gold: self.gold.saturating_add(other.gold),
        }
    }

    fn field_mut(&mut self, color: GemColor) -> &mut u8 {
        match color {
            GemColor::White => &mut self.white,
            GemColor::Blue => &mut self.blue,
            GemColor::Green => &mut self.green,
            GemColor::Red => &mut self.red,
            GemColor::Black => &mut self.black,
            GemColor::Gold => &mut self.gold,
        }
    }
}

/// 公共筹码池。
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Bank {
    pub tokens: TokenSet,
}

impl Bank {
    pub fn take(&mut self, color: GemColor, amount: u8) -> bool {
        self.tokens.remove(color, amount)
    }

    pub fn give(&mut self, color: GemColor, amount: u8) {
        self.tokens.add(color, amount);
    }
}

/// 从 `CardColor` 构造 `TokenSet`（用于费用/折扣等不含金的集合）。
impl From<CardColor> for TokenSet {
    fn from(color: CardColor) -> Self {
        let mut s = Self::default();
        s.set(color.to_gem(), 1);
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_set_add_remove_roundtrip() {
        let mut s = TokenSet::default();
        s.add(GemColor::Red, 3);
        assert_eq!(s.get(GemColor::Red), 3);
        assert!(s.remove(GemColor::Red, 2));
        assert_eq!(s.get(GemColor::Red), 1);
        assert!(!s.remove(GemColor::Red, 5));
        assert_eq!(s.get(GemColor::Red), 1);
    }

    #[test]
    fn total_counts_all_six() {
        let s = TokenSet {
            white: 1,
            blue: 2,
            green: 3,
            red: 4,
            black: 5,
            gold: 6,
        };
        assert_eq!(s.total(), 21);
    }

    #[test]
    fn bank_take_insufficient_does_not_mutate() {
        let mut b = Bank {
            tokens: TokenSet {
                white: 1,
                ..Default::default()
            },
        };
        assert!(!b.take(GemColor::White, 2));
        assert_eq!(b.tokens.white, 1);
    }
}
