//! 颜色与玩家 ID。

/// 全部宝石颜色，含金色（万能）。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum GemColor {
    White,
    Blue,
    Green,
    Red,
    Black,
    Gold,
}

/// 发展卡颜色，不含金色。金色只作支付资源，不作为卡牌颜色。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum CardColor {
    White,
    Blue,
    Green,
    Red,
    Black,
}

/// 玩家标识。对应 `GameState::players` 的下标。
pub type PlayerId = usize;

impl GemColor {
    /// 5 种普通宝石颜色（不含金）。
    pub const NORMAL: [Self; 5] = [Self::White, Self::Blue, Self::Green, Self::Red, Self::Black];

    /// 稳定下标 0..=5，金 = 5。用于数组索引。
    pub const fn index(self) -> usize {
        self as usize
    }

    /// 金色为真。
    pub const fn is_gold(self) -> bool {
        matches!(self, Self::Gold)
    }
}

impl CardColor {
    pub const ALL: [Self; 5] = [Self::White, Self::Blue, Self::Green, Self::Red, Self::Black];

    /// 转为对应的普通宝石颜色。
    pub const fn to_gem(self) -> GemColor {
        match self {
            Self::White => GemColor::White,
            Self::Blue => GemColor::Blue,
            Self::Green => GemColor::Green,
            Self::Red => GemColor::Red,
            Self::Black => GemColor::Black,
        }
    }

    pub const fn index(self) -> usize {
        self as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_colors_exclude_gold() {
        assert_eq!(GemColor::NORMAL.len(), 5);
        assert!(!GemColor::NORMAL.contains(&GemColor::Gold));
    }

    #[test]
    fn gold_has_index_five() {
        assert_eq!(GemColor::Gold.index(), 5);
        assert!(GemColor::Gold.is_gold());
        assert!(!GemColor::White.is_gold());
    }

    #[test]
    fn card_color_maps_to_gem() {
        assert_eq!(CardColor::Blue.to_gem(), GemColor::Blue);
    }
}
