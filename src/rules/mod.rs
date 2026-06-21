//! 璀璨宝石纯规则层。零 Bevy 依赖。
//!
//! 公共 API 通过 `pub use` 重导出，调用方只需 `use crate::rules::*`。
//!
//! 注：各 `pub use` 随对应子模块实现逐步启用，避免引用未定义符号阻塞编译。

mod color;
mod token;
mod card;
mod noble;
mod player;
mod market;
mod state;
mod actions;
mod validation;
mod scoring;
mod events;
mod error;

pub use color::{CardColor, GemColor, PlayerId};
pub use token::{Bank, TokenSet}; // Task 4
pub use card::{CardBonus, CardId, CardLevel, CardStore, DevelopmentCard, GemCost}; // Task 5
pub use noble::{Noble, NobleBoard, NobleId, NobleStore}; // Task 6
pub use player::PlayerState; // Task 7
pub use market::{CardDecks, Market}; // Task 8
// pub use state::GameState;                                // Task 12
// pub use actions::{ActionOutcome, ActionResult, PlayerAction, Resume, apply_action, resume}; // Task 13
// pub use validation::validate_action;                    // Task 13
// pub use scoring::{calculate_score, compare_players, eligible_nobles}; // Task 10
pub use events::GameEvent; // Task 9
pub use error::RuleError; // Task 3
