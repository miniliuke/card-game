//! 规则层错误。

/// 所有规则校验与执行失败统一返回此错误。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RuleError {
    /// 行动发起者不是当前回合玩家。
    NotYourTurn,
    /// 保留牌已达上限（3 张）。
    TooManyReserved,
    /// 公共筹码池该色不足。
    BankInsufficient,
    /// 玩家筹码超过 10 上限且未处理弃牌。
    TokenLimitExceeded,
    /// 指定的卡牌/槽位不存在。
    CardNotFound,
    /// 玩家买不起该卡（折扣+金仍不足）。
    CannotAfford,
    /// 拿筹码选择非法（重复色/含金/数量不对/公共区不足）。
    InvalidTokenSelection,
    /// 指定贵族不满足条件或不在线上。
    NobleNotEligible,
    /// 指定等级牌堆已空，无法盲抽/补牌。
    DeckEmpty,
    /// resume 与当前挂起态不匹配，或无挂起态可续。
    InvalidResume,
    /// 游戏已结束，不能再行动。
    GameOver,
    /// 玩家数不在 2..=4。
    InvalidPlayerCount,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_are_copy_and_eq() {
        assert_eq!(RuleError::DeckEmpty, RuleError::DeckEmpty);
        assert_ne!(RuleError::GameOver, RuleError::InvalidResume);
    }
}
