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
pub use card::{standard_deck, CardBonus, CardId, CardLevel, CardStore, DevelopmentCard, GemCost}; // Task 5
pub use noble::{standard_nobles, Noble, NobleBoard, NobleId, NobleStore}; // Task 6
pub use player::{PlayerState, ReserveOrigin, ReservedCard}; // Task 7
pub use market::{CardDecks, Market}; // Task 8
pub use state::GameState; // Task 12
pub use actions::{ActionOutcome, ActionResult, PlayerAction, Resume, apply_action, legal_actions, resume, validate_action}; // Task 13
pub use scoring::{calculate_score, compare_players, eligible_nobles}; // Task 10
pub use events::GameEvent; // Task 9
pub use error::RuleError; // Task 3
pub use validation::can_afford; // AI 评估与 rollout 复用支付能力校验
