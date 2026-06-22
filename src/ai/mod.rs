//! 纯 Rust AI 搜索层：零 Bevy 依赖。
//!
//! 模块布局：
//! - `decision`：决策上下文、`AiDecision`、`SimulationState`（完整回合模拟包装）。
//! - `observation`：脱敏观察与信息集键。
//! - `determinization`：从观察构造一致的可能世界。
//! - `evaluation`：非终局局面评估。
//! - `rollout`：轻量随机化 rollout 策略。
//! - `mcts`：可取消、可限时的 MO-ISMCTS。
//!
//! 公共类型通过 `pub use` 重导出，调用方只需 `use crate::ai::*`。

#![allow(dead_code)]
#![allow(unused_imports)]

mod decision;
mod determinization;
mod evaluation;
mod mcts;
mod observation;
mod rollout;

pub use decision::{AiDecision, AiError, DecisionContext, DecisionContextKind, SimulationState};
pub use determinization::{PrivateKnowledge, determinize};
pub use evaluation::{EvaluationWeights, evaluate};
pub use mcts::{
    AiSearchMetrics, AiSearchResult, MctsConfig, RootActionStat, SearchControl, search,
};
pub use observation::{AiObservation, InfoSetKey, ObservedPlayer, ObservedReservation};
pub use rollout::{RolloutResult, fallback_decision, rollout};
