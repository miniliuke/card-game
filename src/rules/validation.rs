//! 动作合法性校验。纯函数，不改 GameState。

use crate::rules::card::{CardBonus, CardStore, DevelopmentCard};
use crate::rules::color::{CardColor, GemColor};
use crate::rules::error::RuleError;
use crate::rules::player::RESERVED_LIMIT;
use crate::rules::token::TokenSet;

/// 拿 3 个不同普通色宝石的合法性（rules.md §11）。
pub fn can_take_three_different(
    player_tokens: TokenSet,
    bank: TokenSet,
    colors: &[GemColor],
) -> Result<(), RuleError> {
    if colors.len() != 3 {
        return Err(RuleError::InvalidTokenSelection);
    }
    if colors.iter().any(|c| c.is_gold()) {
        return Err(RuleError::InvalidTokenSelection);
    }
    if !all_different(colors) {
        return Err(RuleError::InvalidTokenSelection);
    }
    for c in colors {
        if bank.get(*c) < 1 {
            return Err(RuleError::BankInsufficient);
        }
    }
    // 注意：拿后是否超 10 由 execute 阶段判定（NeedDiscardTokens），此处不阻断。
    let _ = player_tokens;
    Ok(())
}

/// 拿 2 个相同普通色宝石的合法性（rules.md §12）。
pub fn can_take_two_same(bank: TokenSet, color: GemColor) -> Result<(), RuleError> {
    if color.is_gold() {
        return Err(RuleError::InvalidTokenSelection);
    }
    if bank.get(color) < 4 {
        return Err(RuleError::BankInsufficient);
    }
    Ok(())
}

/// 保留牌是否还有空位（rules.md §14）。
pub fn can_reserve(reserved_count: usize) -> Result<(), RuleError> {
    if reserved_count >= RESERVED_LIMIT {
        return Err(RuleError::TooManyReserved);
    }
    Ok(())
}

/// 是否买得起：折扣后每色普通宝石缺口之和 <= 持有金（rules.md §16）。
pub fn can_afford(
    player_tokens: TokenSet,
    card: &DevelopmentCard,
    bonus: CardBonus,
) -> Result<(), RuleError> {
    let required = card.cost.after_discount(bonus);
    let mut missing = 0u8;
    for color in CardColor::ALL {
        let need = required.get(color);
        let have = player_tokens.get(color.to_gem());
        if have < need {
            missing = missing.saturating_add(need - have);
        }
    }
    if player_tokens.get(GemColor::Gold) < missing {
        return Err(RuleError::CannotAfford);
    }
    Ok(())
}

fn all_different(colors: &[GemColor]) -> bool {
    for i in 0..colors.len() {
        for j in (i + 1)..colors.len() {
            if colors[i] == colors[j] {
                return false;
            }
        }
    }
    true
}

/// 计算支付方案：每色先付 min(持有普通, 折扣后需求)，缺口用金补。返回 (支付的普通色集合, 用的金数)。
/// 调用前应已通过 can_afford。
pub fn plan_payment(
    player_tokens: TokenSet,
    card: &DevelopmentCard,
    bonus: CardBonus,
) -> (TokenSet, u8) {
    let required = card.cost.after_discount(bonus);
    let mut paid = TokenSet::default();
    let mut gold_needed = 0u8;
    for color in CardColor::ALL {
        let need = required.get(color);
        let have = player_tokens.get(color.to_gem());
        let pay_normal = have.min(need);
        paid.set(color.to_gem(), pay_normal);
        let remaining = need - pay_normal;
        if remaining > 0 {
            gold_needed += remaining;
        }
    }
    paid.set(GemColor::Gold, gold_needed);
    (paid, gold_needed)
}

// 顶层 validate_action 分发在 actions.rs 中实现（需引用 PlayerAction）。

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::card::{CardLevel, GemCost};

    fn card(cost: GemCost) -> DevelopmentCard {
        DevelopmentCard {
            id: 1,
            level: CardLevel::Level1,
            color: CardColor::White,
            prestige: 0,
            cost,
        }
    }

    #[test]
    fn three_different_rejects_duplicates() {
        let bank = TokenSet {
            white: 4,
            blue: 4,
            green: 4,
            ..Default::default()
        };
        let r = can_take_three_different(
            TokenSet::default(),
            bank,
            &[GemColor::White, GemColor::White, GemColor::Blue],
        );
        assert_eq!(r, Err(RuleError::InvalidTokenSelection));
    }

    #[test]
    fn three_different_rejects_gold() {
        let bank = TokenSet {
            gold: 5,
            white: 4,
            blue: 4,
            ..Default::default()
        };
        let r = can_take_three_different(
            TokenSet::default(),
            bank,
            &[GemColor::White, GemColor::Blue, GemColor::Gold],
        );
        assert_eq!(r, Err(RuleError::InvalidTokenSelection));
    }

    #[test]
    fn three_different_rejects_when_bank_low() {
        let bank = TokenSet {
            white: 0,
            blue: 4,
            green: 4,
            ..Default::default()
        };
        let r = can_take_three_different(
            TokenSet::default(),
            bank,
            &[GemColor::White, GemColor::Blue, GemColor::Green],
        );
        assert_eq!(r, Err(RuleError::BankInsufficient));
    }

    #[test]
    fn two_same_needs_four_in_bank() {
        let bank = TokenSet {
            red: 3,
            ..Default::default()
        };
        assert_eq!(
            can_take_two_same(bank, GemColor::Red),
            Err(RuleError::BankInsufficient)
        );
        let bank2 = TokenSet {
            red: 4,
            ..Default::default()
        };
        assert!(can_take_two_same(bank2, GemColor::Red).is_ok());
    }

    #[test]
    fn two_same_rejects_gold() {
        let bank = TokenSet {
            gold: 5,
            ..Default::default()
        };
        assert_eq!(
            can_take_two_same(bank, GemColor::Gold),
            Err(RuleError::InvalidTokenSelection)
        );
    }

    #[test]
    fn reserve_limit_enforced() {
        assert!(can_reserve(2).is_ok());
        assert_eq!(can_reserve(3), Err(RuleError::TooManyReserved));
    }

    #[test]
    fn can_afford_with_discount_and_gold() {
        // 卡费 白3 蓝2；玩家 白2 蓝3 金1；bonus 0。
        let c = card(GemCost {
            white: 3,
            blue: 2,
            ..Default::default()
        });
        let tokens = TokenSet {
            white: 2,
            blue: 3,
            gold: 1,
            ..Default::default()
        };
        // 折扣后 白3 蓝2 -> 白缺1 -> 金1 够。
        assert!(can_afford(tokens, &c, CardBonus::default()).is_ok());
        let tokens2 = TokenSet {
            white: 2,
            blue: 3,
            gold: 0,
            ..Default::default()
        };
        assert_eq!(
            can_afford(tokens2, &c, CardBonus::default()),
            Err(RuleError::CannotAfford)
        );
    }

    #[test]
    fn plan_payment_uses_gold_for_shortfall() {
        let c = card(GemCost {
            white: 3,
            blue: 2,
            ..Default::default()
        });
        let tokens = TokenSet {
            white: 2,
            blue: 2,
            gold: 1,
            ..Default::default()
        };
        let (paid, gold) = plan_payment(tokens, &c, CardBonus::default());
        assert_eq!(paid.get(GemColor::White), 2);
        assert_eq!(paid.get(GemColor::Blue), 2);
        assert_eq!(gold, 1); // 白缺1，金补1
        assert_eq!(paid.get(GemColor::Gold), 1);
    }
}
