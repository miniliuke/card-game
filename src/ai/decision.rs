//! 决策上下文、`AiDecision` 与 `SimulationState`（完整回合模拟包装）。
//!
//! `SimulationState::apply_decision` 是搜索内部唯一的状态转移入口：它把
//! `AiDecision` 映射到规则层 `apply_action` / `resume`，再把返回的
//! `ActionOutcome` 映射为下一个 `DecisionContext`。AI 不复制规则，只调用规则 API。

use crate::rules::{
    ActionOutcome, ActionResult, GameEvent, GemColor, NobleId, PlayerAction, Resume, RuleError,
    TokenSet, apply_action, legal_actions, resume,
};

/// 当前待决断的上下文。三类决策各自独立获得搜索预算。
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum DecisionContext {
    MainTurn,
    Discard { excess: u8 },
    ChooseNoble { candidates: Vec<NobleId> },
}

/// `DecisionContext` 的判别种类，用作请求 token 的轻量匹配键。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum DecisionContextKind {
    MainTurn,
    Discard,
    ChooseNoble,
}

impl DecisionContext {
    pub fn kind(&self) -> DecisionContextKind {
        match self {
            Self::MainTurn => DecisionContextKind::MainTurn,
            Self::Discard { .. } => DecisionContextKind::Discard,
            Self::ChooseNoble { .. } => DecisionContextKind::ChooseNoble,
        }
    }
}

/// AI 提交的单一决策。最终都通过规则层 `apply_action` / `resume` 落地。
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum AiDecision {
    Action(PlayerAction),
    Discard(TokenSet),
    ChooseNoble(NobleId),
}

/// AI 层错误。不携带隐藏信息，可安全记录。
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum AiError {
    NoLegalDecision,
    InvalidObservation(&'static str),
    Rule(RuleError),
    Cancelled,
    UnsupportedCombinedOutcome,
}

impl From<RuleError> for AiError {
    fn from(value: RuleError) -> Self {
        Self::Rule(value)
    }
}

/// 搜索内部包装状态：`GameState` + 当前决策上下文。
#[derive(Clone, Debug)]
pub struct SimulationState {
    pub game: crate::rules::GameState,
    pub context: DecisionContext,
}

impl SimulationState {
    pub fn new(game: crate::rules::GameState, context: DecisionContext) -> Self {
        Self { game, context }
    }

    /// 枚举当前上下文下所有合法决策。
    pub fn legal_decisions(&self) -> Result<Vec<AiDecision>, AiError> {
        let player = self.game.current_id();
        match &self.context {
            DecisionContext::MainTurn => Ok(legal_actions(&self.game, player)
                .into_iter()
                .map(AiDecision::Action)
                .collect()),
            DecisionContext::Discard { excess } => {
                Ok(enumerate_discards(self.game.player(player).tokens, *excess)
                    .into_iter()
                    .map(AiDecision::Discard)
                    .collect())
            }
            DecisionContext::ChooseNoble { candidates } => Ok(candidates
                .iter()
                .copied()
                .map(AiDecision::ChooseNoble)
                .collect()),
        }
    }

    /// 应用一个决策，返回规则事件，并把上下文推进到下一个待决断点。
    pub fn apply_decision(&mut self, decision: AiDecision) -> Result<Vec<GameEvent>, AiError> {
        let player = self.game.current_id();
        let result: ActionResult = match (&self.context, decision) {
            (DecisionContext::MainTurn, AiDecision::Action(action)) => {
                apply_action(&mut self.game, player, action)?
            }
            (DecisionContext::Discard { .. }, AiDecision::Discard(tokens)) => {
                resume(&mut self.game, player, Resume::DiscardTokens(tokens))?
            }
            (DecisionContext::ChooseNoble { .. }, AiDecision::ChooseNoble(noble)) => {
                resume(&mut self.game, player, Resume::ChooseNoble(noble))?
            }
            _ => return Err(AiError::NoLegalDecision),
        };
        self.context = match result.outcome {
            ActionOutcome::Complete => DecisionContext::MainTurn,
            ActionOutcome::NeedDiscardTokens { excess } => DecisionContext::Discard { excess },
            ActionOutcome::NeedChooseNoble { candidates } => {
                DecisionContext::ChooseNoble { candidates }
            }
            ActionOutcome::NeedFinalDiscardThenChooseNoble { .. } => {
                return Err(AiError::UnsupportedCombinedOutcome);
            }
        };
        Ok(result.events)
    }
}

/// 枚举总量恰好等于 `excess`、且逐色不超过玩家持有量的所有 `TokenSet`。
///
/// 按 `[White, Blue, Green, Red, Black, Gold]` 顺序递归分配 `0..=min(have, remaining)`，
/// 仅在最终剩余量为 0 时推入结果。输出按六色计数排序并去重。
pub(crate) fn enumerate_discards(have: TokenSet, excess: u8) -> Vec<TokenSet> {
    let colors = [
        GemColor::White,
        GemColor::Blue,
        GemColor::Green,
        GemColor::Red,
        GemColor::Black,
        GemColor::Gold,
    ];
    let mut out = Vec::new();
    let mut current = TokenSet::default();
    recurse_discards(&colors, 0, have, excess, &mut current, &mut out);

    // 按六色计数稳定排序并去重（递归已保证无重复，但显式去重更稳妥）。
    out.sort_by(|a, b| {
        for color in colors.iter() {
            match a.get(*color).cmp(&b.get(*color)) {
                std::cmp::Ordering::Equal => continue,
                non_equal => return non_equal,
            }
        }
        std::cmp::Ordering::Equal
    });
    out.dedup();
    out
}

fn recurse_discards(
    colors: &[GemColor; 6],
    depth: usize,
    have: TokenSet,
    remaining: u8,
    current: &mut TokenSet,
    out: &mut Vec<TokenSet>,
) {
    if depth == colors.len() {
        if remaining == 0 {
            out.push(*current);
        }
        return;
    }
    let color = colors[depth];
    let max = have.get(color).min(remaining);
    for amt in 0..=max {
        current.set(color, amt);
        recurse_discards(colors, depth + 1, have, remaining - amt, current, out);
    }
    current.set(color, 0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{GameState, PlayerAction, TokenSet};

    #[test]
    fn discard_decisions_return_exactly_the_excess() {
        let mut game = GameState::new_seeded(2, 7).unwrap();
        game.players[0].tokens = TokenSet {
            white: 4,
            blue: 4,
            green: 4,
            ..Default::default()
        };
        let sim = SimulationState::new(game, DecisionContext::Discard { excess: 2 });
        let decisions = sim.legal_decisions().unwrap();
        assert!(decisions.iter().all(|decision| match decision {
            AiDecision::Discard(tokens) => tokens.total() == 2,
            _ => false,
        }));
    }

    #[test]
    fn applying_main_decision_updates_the_pending_context() {
        let mut game = GameState::new_seeded(2, 11).unwrap();
        game.players[0].tokens = TokenSet {
            white: 3,
            blue: 3,
            green: 3,
            ..Default::default()
        };
        let mut sim = SimulationState::new(game, DecisionContext::MainTurn);
        sim.apply_decision(AiDecision::Action(PlayerAction::TakeThreeDifferentTokens(
            vec![GemColor::White, GemColor::Blue, GemColor::Green],
        )))
        .unwrap();
        assert_eq!(sim.context, DecisionContext::Discard { excess: 2 });
    }

    #[test]
    fn enumerate_discards_covers_all_two_token_combos() {
        let have = TokenSet {
            white: 2,
            blue: 2,
            ..Default::default()
        };
        let combos = enumerate_discards(have, 2);
        // 总量恰好为 2 的组合：white 0..=2 × blue 0..=2 中 white+blue==2 的有 3 个。
        assert_eq!(combos.len(), 3);
        assert!(combos.iter().all(|t| t.total() == 2));
        assert!(combos.contains(&TokenSet {
            white: 2,
            ..Default::default()
        }));
        assert!(combos.contains(&TokenSet {
            white: 1,
            blue: 1,
            ..Default::default()
        }));
        assert!(combos.contains(&TokenSet {
            blue: 2,
            ..Default::default()
        }));
    }

    #[test]
    fn enumerate_discards_respects_holdings() {
        let have = TokenSet {
            white: 1,
            blue: 0,
            ..Default::default()
        };
        let combos = enumerate_discards(have, 1);
        // 只能弃 1 白。
        assert_eq!(
            combos,
            vec![TokenSet {
                white: 1,
                ..Default::default()
            }]
        );
    }
}
