//! 璀璨宝石纯规则层。零 Bevy 依赖。
//!
//! 公共 API 通过 `pub use` 重导出，调用方只需 `use crate::rules::*`。
//!
//! 注：本模块设计为独立可复用的规则层 API，供后续 Bevy System 调用。
//! 当前二进制尚未接入，故 `pub` 项暂未被 crate 外消费；
//! 为避免 `dead_code`/`unused_imports` 噪音干扰，整体放行。

#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::module_inception)]

mod actions;
mod card;
mod color;
mod error;
mod events;
mod market;
mod noble;
mod player;
mod scoring;
mod state;
mod token;
mod validation;

pub use actions::{
    ActionOutcome, ActionResult, PlayerAction, Resume, apply_action, legal_actions, resume,
    validate_action,
}; // Task 13
pub use card::{CardBonus, CardId, CardLevel, CardStore, DevelopmentCard, GemCost, standard_deck}; // Task 5
pub use color::{CardColor, GemColor, PlayerId};
pub use error::RuleError; // Task 3
pub use events::GameEvent; // Task 9
pub use market::{CardDecks, Market}; // Task 8
pub use noble::{Noble, NobleBoard, NobleId, NobleStore, standard_nobles}; // Task 6
pub use player::{PlayerState, ReserveOrigin, ReservedCard}; // Task 7
pub use scoring::{calculate_score, compare_players, eligible_nobles}; // Task 10
pub use state::GameState; // Task 12
pub use token::{Bank, TokenSet}; // Task 4
pub use validation::{can_afford, required_different_token_count};
