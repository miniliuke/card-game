//! 璀璨宝石纯规则层。零 Bevy 依赖。
//!
//! 公共 API 通过 `pub use` 重导出，调用方只需 `use crate::rules::*`。

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
pub use token::{Bank, TokenSet};
pub use card::{CardBonus, CardId, CardLevel, CardStore, DevelopmentCard, GemCost};
pub use noble::{Noble, NobleBoard, NobleId, NobleStore};
pub use player::PlayerState;
pub use market::{CardDecks, Market};
pub use state::GameState;
pub use actions::{ActionOutcome, ActionResult, PlayerAction, Resume, apply_action, resume};
pub use validation::validate_action;
pub use scoring::{calculate_score, compare_players, eligible_nobles};
pub use events::GameEvent;
pub use error::RuleError;
