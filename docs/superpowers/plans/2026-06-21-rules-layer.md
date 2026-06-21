# 璀璨宝石规则层实现计划 (Rules Layer Implementation Plan)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 新建一个符合 `docs/rules.md` 的纯 Rust 规则层 `src/rules/`，零 Bevy 依赖，可独立测试；旧 `src/game.rs` 与 `src/battle.rs` 本阶段不动。

**Architecture:** 纯函数式状态机。`GameState` 是唯一可变状态；所有动作经 `apply_action` 单一入口，返回 `(ActionOutcome, Vec<GameEvent>)`；需玩家选择（弃牌/选贵族）时挂起，由 `resume` 续接。`rand` 仅在 `GameState::new` 初始化时使用，之后状态确定可回放。

**Tech Stack:** Rust 1.90 (edition 2024), `rand = "0.9"`. 已有 Bevy 0.18 依赖不动。测试用 `#[cfg(test)]` 内联模块 + 固定 seed 的 `StdRng::seed_from_u64`.

**Spec:** `docs/superpowers/specs/2026-06-21-rules-layer-design.md`

**重要约定:** 项目不是 git 仓库（环境标注 "Is a git repository: no"）。本计划的 "Commit" 步骤在无 git 时跳过即可——用 `git status` 探测，若非仓库则该步改为"运行 `cargo test` 确认全绿"作为阶段收尾。TDD 红-绿循环不变。

**rand 0.9 API 速查（计划内统一用法）:**
```rust
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
// 创建: let mut rng = StdRng::seed_from_u64(42);
// 洗牌: vec.shuffle(&mut rng);   // SliceRandom::shuffle(&mut self, rng: &mut R)
// GameState::new 签名: pub fn new(player_count: usize, rng: &mut impl Rng) -> Result<Self, RuleError>
```

---

## 文件结构总览

新建 `src/rules/` 模块树（13 文件），`src/main.rs` 加 `mod rules;`，`Cargo.toml` 加 `rand`。各文件职责：

| 文件 | 职责 |
|---|---|
| `src/rules/mod.rs` | 子模块声明 + `pub use` 重导出公共 API |
| `src/rules/color.rs` | `GemColor`(含金), `CardColor`(不含金), `PlayerId` |
| `src/rules/token.rs` | `TokenSet`(6字段), `Bank` |
| `src/rules/card.rs` | `CardLevel`, `GemCost`(5字段), `CardBonus`(5字段), `DevelopmentCard`, `CardId`, `CardStore`, `standard_deck()` |
| `src/rules/noble.rs` | `Noble`, `NobleId`, `NobleStore`, `NobleBoard`, `standard_nobles()` |
| `src/rules/player.rs` | `PlayerState` |
| `src/rules/market.rs` | `CardDecks`, `Market` |
| `src/rules/state.rs` | `GameState`, `GameState::new` |
| `src/rules/actions.rs` | `PlayerAction`, `ActionOutcome`, `ActionResult`, `Resume`, `apply_action`, `resume` |
| `src/rules/validation.rs` | `validate_action` 及 `can_*` 谓词 |
| `src/rules/scoring.rs` | `calculate_score`, `compare_players`, `eligible_nobles` |
| `src/rules/events.rs` | `GameEvent` |
| `src/rules/error.rs` | `RuleError` |
| `src/main.rs` | 加一行 `mod rules;` |
| `Cargo.toml` | 加 `rand = "0.9"` |

依赖方向（无环）：`state` → {`player`,`market`,`noble`,`token`,`card`}; `actions` → {`validation`,`scoring`,`events`,`error`,`state`}; `scoring` → {`card`,`noble`,`player`}; `validation` → {`card`,`token`,`player`,`market`,`state`,`error`}.

---

## Task 1: 项目骨架与依赖

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/main.rs`
- Create: `src/rules/mod.rs`

- [ ] **Step 1: 加 rand 依赖**

修改 `Cargo.toml`，在 `[dependencies]` 下加 `rand`：

```toml
[dependencies]
# Bevy 0.18 is the newest release compatible with the Rust 1.90 toolchain
# currently installed in this workspace (Bevy 0.19 requires Rust 1.95).
bevy = "0.18.1"
rand = "0.9"
```

- [ ] **Step 2: 创建 rules 模块占位**

创建 `src/rules/mod.rs`：

```rust
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
```

各子模块尚不存在，编译会失败。下一步逐个创建最小占位使其编译。

- [ ] **Step 3: 创建所有子模块最小占位**

为每个 `mod` 创建文件，内容为空 `//! <职责>` 注释 + 空体。例如 `src/rules/color.rs`：

```rust
//! 颜色与玩家 ID。
```

对 `token.rs` `card.rs` `noble.rs` `player.rs` `market.rs` `state.rs` `actions.rs` `validation.rs` `scoring.rs` `events.rs` `error.rs` 全部如此创建（各放一行 `//! ` 注释）。此时 `mod.rs` 的 `pub use` 找不到符号，仍编译失败——这是预期的，后续任务逐步填充。

- [ ] **Step 4: 在 main.rs 注册模块**

修改 `src/main.rs`，在现有 `mod battle;` `mod game;` 下方加：

```rust
mod battle;
mod game;
mod rules;
```

- [ ] **Step 5: 运行 cargo build 确认结构被识别**

Run: `cargo build`
Expected: 编译失败，错误来自 `rules/mod.rs` 的 `pub use`（符号未定义）。**确认错误仅来自 rules 模块**，无其他语法错误。这是本任务的终点——骨架就位，后续任务填充符号。

> 注：若希望本任务即编译通过，可临时把 `mod.rs` 里所有 `pub use` 注释掉，下一任务再逐个启用。推荐保留 `pub use` 以便尽早暴露缺失。

---

## Task 2: 颜色与玩家 ID (`color.rs`)

**Files:**
- Modify: `src/rules/color.rs`
- Test: `src/rules/color.rs` 内 `#[cfg(test)]`

- [ ] **Step 1: 写失败测试**

在 `src/rules/color.rs` 末尾加测试模块：

```rust
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
```

- [ ] **Step 2: 运行测试确认通过**

Run: `cargo test --lib rules::color`
Expected: 3 tests PASS.

> 说明：本任务直接写完整实现+测试一并落（类型简单，TDD 红绿在此处合并不损失价值）。后续复杂模块严格红-绿分离。

- [ ] **Step 3: 启用 mod.rs 重导出**

`src/rules/mod.rs` 中 `pub use color::{CardColor, GemColor, PlayerId};` 已存在（Task 1 写入），无需改动。

- [ ] **Step 4: 提交**

Run: `git add -A && git commit -m "feat(rules): add GemColor/CardColor/PlayerId"`
若非 git 仓库则跳过，运行 `cargo test --lib rules::color` 确认全绿。

---

## Task 3: 错误类型 (`error.rs`)

**Files:**
- Modify: `src/rules/error.rs`
- Test: 内联

- [ ] **Step 1: 写实现与测试**

替换 `src/rules/error.rs` 全部内容：

```rust
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
```

- [ ] **Step 2: 运行测试**

Run: `cargo test --lib rules::error`
Expected: 1 test PASS.

- [ ] **Step 3: 提交**

Run: `git add -A && git commit -m "feat(rules): add RuleError"`
非仓库则跳过。

---

## Task 4: 筹码与银行 (`token.rs`)

**Files:**
- Modify: `src/rules/token.rs`
- Test: 内联

- [ ] **Step 1: 写实现与测试**

在 `src/rules/token.rs` 写测试（实现尚未写）：

```rust
//! 筹码集合与公共银行。

use crate::rules::color::{CardColor, GemColor};

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
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
        let s = TokenSet { white: 1, blue: 2, green: 3, red: 4, black: 5, gold: 6 };
        assert_eq!(s.total(), 21);
    }

    #[test]
    fn bank_take_insufficient_does_not_mutate() {
        let mut b = Bank { tokens: TokenSet { white: 1, ..Default::default() } };
        assert!(!b.take(GemColor::White, 2));
        assert_eq!(b.tokens.white, 1);
    }
}
```

> 注：上面把实现与测试一并贴出（因 `field_mut` 等相互依赖，分离红绿无收益）。执行时：先只贴 `#[cfg(test)]` 之上不含实现的部分会编译不过——故实际按"写完整文件 → 跑测试"执行。

- [ ] **Step 2: 运行测试**

Run: `cargo test --lib rules::token`
Expected: 3 tests PASS.

- [ ] **Step 3: 提交**

Run: `git add -A && git commit -m "feat(rules): add TokenSet and Bank"`
非仓库则跳过。

---

## Task 5: 卡牌、费用、折扣、牌库数据 (`card.rs`)

**Files:**
- Modify: `src/rules/card.rs`
- Test: 内联

> **牌库数据说明：** 90 张逐张数据由作者凭记忆硬编码，存在数值偏差风险。已知统计特征已由测试锁定：40/30/20、每色加成均匀、加成色费用为 0、分值随等级递增。若发现具体卡牌数值不符正版，替换 `standard_deck()` 内对应行即可，不影响规则逻辑。

- [ ] **Step 1: 写实现（类型 + 数据 + 折扣）**

替换 `src/rules/card.rs` 全部内容：

```rust
//! 发展卡、费用、折扣、真实牌库数据。

use std::collections::HashMap;

use crate::rules::color::CardColor;

pub type CardId = u32;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum CardLevel {
    Level1,
    Level2,
    Level3,
}

impl CardLevel {
    pub const ALL: [Self; 3] = [Self::Level1, Self::Level2, Self::Level3];
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// 卡牌费用，5 字段，不含金。金只在支付时作为万能补缺口。
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct GemCost {
    pub white: u8,
    pub blue: u8,
    pub green: u8,
    pub red: u8,
    pub black: u8,
}

impl GemCost {
    pub fn get(self, color: CardColor) -> u8 {
        match color {
            CardColor::White => self.white,
            CardColor::Blue => self.blue,
            CardColor::Green => self.green,
            CardColor::Red => self.red,
            CardColor::Black => self.black,
        }
    }

    /// 应用折扣：每色 max(need - bonus, 0)。
    pub fn after_discount(self, bonus: CardBonus) -> Self {
        Self {
            white: self.white.saturating_sub(bonus.white),
            blue: self.blue.saturating_sub(bonus.blue),
            green: self.green.saturating_sub(bonus.green),
            red: self.red.saturating_sub(bonus.red),
            black: self.black.saturating_sub(bonus.black),
        }
    }

    /// 折扣后仍需支付的总普通宝石数（金色需补的缺口上限）。
    pub fn total_missing(self) -> u8 {
        self.white + self.blue + self.green + self.red + self.black
    }
}

/// 玩家已购发展卡按色计数，作为购买折扣。
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct CardBonus {
    pub white: u8,
    pub blue: u8,
    pub green: u8,
    pub red: u8,
    pub black: u8,
}

impl CardBonus {
    pub fn get(self, color: CardColor) -> u8 {
        match color {
            CardColor::White => self.white,
            CardColor::Blue => self.blue,
            CardColor::Green => self.green,
            CardColor::Red => self.red,
            CardColor::Black => self.black,
        }
    }

    pub fn add(&mut self, color: CardColor) {
        match color {
            CardColor::White => self.white += 1,
            CardColor::Blue => self.blue += 1,
            CardColor::Green => self.green += 1,
            CardColor::Red => self.red += 1,
            CardColor::Black => self.black += 1,
        }
    }

    /// 是否满足某贵族要求（每色 bonus >= requirement）。
    pub fn satisfies(self, requirement: GemCost) -> bool {
        CardColor::ALL
            .iter()
            .all(|&c| self.get(c) >= requirement.get(c))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct DevelopmentCard {
    pub id: CardId,
    pub level: CardLevel,
    pub color: CardColor,
    pub prestige: u8,
    pub cost: GemCost,
}

/// id -> card 只读索引，供 bonus/score 反查。
#[derive(Clone, Default)]
pub struct CardStore {
    map: HashMap<CardId, DevelopmentCard>,
}

impl CardStore {
    pub fn from_cards(cards: &[DevelopmentCard]) -> Self {
        let map = cards.iter().copied().map(|c| (c.id, c)).collect();
        Self { map }
    }

    pub fn get(&self, id: CardId) -> Option<&DevelopmentCard> {
        self.map.get(&id)
    }
}

/// 返回标准 90 张发展卡（40/30/20）。逐张硬编码，分值/费用按等级递增。
/// 注意：数值由作者凭记忆录入，可能存在偏差；统计特征由测试锁定。
pub fn standard_deck() -> Vec<DevelopmentCard> {
    let mut id = 0u32;
    let mut mk = |level: CardLevel, color: CardColor, prestige: u8, cost: [u8; 5]| -> DevelopmentCard {
        let card = DevelopmentCard {
            id,
            level,
            color,
            prestige,
            cost: GemCost { white: cost[0], blue: cost[1], green: cost[2], red: cost[3], black: cost[4] },
        };
        id += 1;
        card
    };
    let l1 = CardLevel::Level1;
    let l2 = CardLevel::Level2;
    let l3 = CardLevel::Level3;
    // 顺序：白 蓝 绿 红 黑。加成色费用为 0。
    let cards = vec![
        // ===== Level 1 (40 张) =====
        // White bonus (8)
        mk(l1, CardColor::White, 0, [0,1,2,1,0]),
        mk(l1, CardColor::White, 0, [0,2,0,1,1]),
        mk(l1, CardColor::White, 0, [0,1,1,0,2]),
        mk(l1, CardColor::White, 0, [0,0,2,2,0]),
        mk(l1, CardColor::White, 0, [0,2,1,0,1]),
        mk(l1, CardColor::White, 0, [0,0,1,2,1]),
        mk(l1, CardColor::White, 0, [0,1,0,1,2]),
        mk(l1, CardColor::White, 1, [0,3,0,0,0]),
        // Blue bonus (8)
        mk(l1, CardColor::Blue, 0, [1,0,2,1,0]),
        mk(l1, CardColor::Blue, 0, [2,0,0,1,1]),
        mk(l1, CardColor::Blue, 0, [1,0,1,0,2]),
        mk(l1, CardColor::Blue, 0, [2,0,2,0,0]),
        mk(l1, CardColor::Blue, 0, [1,0,2,1,0]),
        mk(l1, CardColor::Blue, 0, [0,0,1,2,1]),
        mk(l1, CardColor::Blue, 0, [2,0,0,1,1]),
        mk(l1, CardColor::Blue, 1, [3,0,0,0,0]),
        // Green bonus (8)
        mk(l1, CardColor::Green, 0, [2,1,0,0,1]),
        mk(l1, CardColor::Green, 0, [0,2,0,1,1]),
        mk(l1, CardColor::Green, 0, [1,1,0,2,0]),
        mk(l1, CardColor::Green, 0, [2,0,0,0,2]),
        mk(l1, CardColor::Green, 0, [1,2,0,1,0]),
        mk(l1, CardColor::Green, 0, [0,1,0,2,1]),
        mk(l1, CardColor::Green, 0, [1,0,0,1,2]),
        mk(l1, CardColor::Green, 1, [0,0,0,3,0]),
        // Red bonus (8)
        mk(l1, CardColor::Red, 0, [1,2,1,0,0]),
        mk(l1, CardColor::Red, 0, [2,1,0,0,1]),
        mk(l1, CardColor::Red, 0, [0,1,2,0,1]),
        mk(l1, CardColor::Red, 0, [1,0,1,0,2]),
        mk(l1, CardColor::Red, 0, [2,0,0,0,2]),
        mk(l1, CardColor::Red, 0, [0,2,1,0,1]),
        mk(l1, CardColor::Red, 0, [1,1,0,0,2]),
        mk(l1, CardColor::Red, 1, [0,0,3,0,0]),
        // Black bonus (8)
        mk(l1, CardColor::Black, 0, [1,0,1,2,0]),
        mk(l1, CardColor::Black, 0, [0,1,2,1,0]),
        mk(l1, CardColor::Black, 0, [2,1,0,1,0]),
        mk(l1, CardColor::Black, 0, [0,2,0,1,1]),
        mk(l1, CardColor::Black, 0, [1,0,2,0,1]),
        mk(l1, CardColor::Black, 0, [0,1,0,2,1]),
        mk(l1, CardColor::Black, 0, [2,0,1,0,1]),
        mk(l1, CardColor::Black, 1, [0,0,0,0,3]),

        // ===== Level 2 (30 张) =====
        // White bonus (6)
        mk(l2, CardColor::White, 1, [0,2,2,0,3]),
        mk(l2, CardColor::White, 1, [0,3,0,3,2]),
        mk(l2, CardColor::White, 1, [0,0,3,2,3]),
        mk(l2, CardColor::White, 2, [0,5,0,0,0]),
        mk(l2, CardColor::White, 2, [0,0,5,0,0]),
        mk(l2, CardColor::White, 2, [0,0,0,5,0]),
        // Blue bonus (6)
        mk(l2, CardColor::Blue, 1, [2,0,2,3,0]),
        mk(l2, CardColor::Blue, 1, [3,0,3,0,2]),
        mk(l2, CardColor::Blue, 1, [0,0,2,3,3]),
        mk(l2, CardColor::Blue, 2, [5,0,0,0,0]),
        mk(l2, CardColor::Blue, 2, [0,0,5,0,0]),
        mk(l2, CardColor::Blue, 2, [0,0,0,0,5]),
        // Green bonus (6)
        mk(l2, CardColor::Green, 1, [2,3,0,0,2]),
        mk(l2, CardColor::Green, 1, [3,2,0,3,0]),
        mk(l2, CardColor::Green, 1, [2,0,0,3,3]),
        mk(l2, CardColor::Green, 2, [0,5,0,0,0]),
        mk(l2, CardColor::Green, 2, [5,0,0,0,0]),
        mk(l2, CardColor::Green, 2, [0,0,0,0,5]),
        // Red bonus (6)
        mk(l2, CardColor::Red, 1, [3,0,2,0,2]),
        mk(l2, CardColor::Red, 1, [0,3,3,0,2]),
        mk(l2, CardColor::Red, 1, [2,2,0,0,3]),
        mk(l2, CardColor::Red, 2, [0,0,5,0,0]),
        mk(l2, CardColor::Red, 2, [0,0,0,5,0]),
        mk(l2, CardColor::Red, 2, [5,0,0,0,0]),
        // Black bonus (6)
        mk(l2, CardColor::Black, 1, [0,2,3,2,0]),
        mk(l2, CardColor::Black, 1, [2,0,3,3,0]),
        mk(l2, CardColor::Black, 1, [3,2,2,0,0]),
        mk(l2, CardColor::Black, 2, [0,0,0,0,5]),
        mk(l2, CardColor::Black, 2, [0,5,0,0,0]),
        mk(l2, CardColor::Black, 2, [0,0,5,0,0]),

        // ===== Level 3 (20 张) =====
        // White bonus (4)
        mk(l3, CardColor::White, 3, [0,3,3,5,3]),
        mk(l3, CardColor::White, 4, [0,0,0,6,4]),
        mk(l3, CardColor::White, 4, [0,5,5,0,3]),
        mk(l3, CardColor::White, 5, [0,0,0,7,0]),
        // Blue bonus (4)
        mk(l3, CardColor::Blue, 3, [5,0,3,3,3]),
        mk(l3, CardColor::Blue, 4, [4,0,0,0,6]),
        mk(l3, CardColor::Blue, 4, [3,0,5,5,0]),
        mk(l3, CardColor::Blue, 5, [0,0,0,0,7]),
        // Green bonus (4)
        mk(l3, CardColor::Green, 3, [3,5,0,3,3]),
        mk(l3, CardColor::Green, 4, [6,4,0,0,0]),
        mk(l3, CardColor::Green, 4, [0,3,0,5,5]),
        mk(l3, CardColor::Green, 5, [7,0,0,0,0]),
        // Red bonus (4)
        mk(l3, CardColor::Red, 3, [3,3,5,0,3]),
        mk(l3, CardColor::Red, 4, [0,6,4,0,0]),
        mk(l3, CardColor::Red, 4, [5,0,3,0,5]),
        mk(l3, CardColor::Red, 5, [0,7,0,0,0]),
        // Black bonus (4)
        mk(l3, CardColor::Black, 3, [3,3,3,5,0]),
        mk(l3, CardColor::Black, 4, [0,0,6,4,0]),
        mk(l3, CardColor::Black, 4, [5,5,0,3,0]),
        mk(l3, CardColor::Black, 5, [0,0,0,7,0]),
    ];
    cards
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_deck_has_40_30_20() {
        let deck = standard_deck();
        assert_eq!(deck.len(), 90);
        assert_eq!(deck.iter().filter(|c| c.level == CardLevel::Level1).count(), 40);
        assert_eq!(deck.iter().filter(|c| c.level == CardLevel::Level2).count(), 30);
        assert_eq!(deck.iter().filter(|c| c.level == CardLevel::Level3).count(), 20);
    }

    #[test]
    fn each_level_balanced_across_colors() {
        let deck = standard_deck();
        for level in CardLevel::ALL {
            for color in CardColor::ALL {
                let count = deck.iter().filter(|c| c.level == level && c.color == color).count();
                let expected = match level {
                    CardLevel::Level1 => 8,
                    CardLevel::Level2 => 6,
                    CardLevel::Level3 => 4,
                };
                assert_eq!(count, expected, "level {level:?} color {color:?}");
            }
        }
    }

    #[test]
    fn bonus_color_cost_is_zero() {
        for card in standard_deck() {
            assert_eq!(card.cost.get(card.color), 0, "card {} bonus color cost nonzero", card.id);
        }
    }

    #[test]
    fn ids_are_unique() {
        let deck = standard_deck();
        let mut ids: Vec<_> = deck.iter().map(|c| c.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 90);
    }

    #[test]
    fn after_discount_floors_at_zero() {
        let cost = GemCost { white: 3, blue: 2, ..Default::default() };
        let bonus = CardBonus { white: 1, blue: 3, ..Default::default() };
        let after = cost.after_discount(bonus);
        assert_eq!(after.white, 2);
        assert_eq!(after.blue, 0);
    }

    #[test]
    fn bonus_satisfies_requirement() {
        let bonus = CardBonus { white: 4, blue: 4, ..Default::default() };
        let req = GemCost { white: 4, blue: 4, ..Default::default() };
        assert!(bonus.satisfies(req));
        let req2 = GemCost { white: 4, blue: 5, ..Default::default() };
        assert!(!bonus.satisfies(req2));
    }
}
```

- [ ] **Step 2: 运行测试**

Run: `cargo test --lib rules::card`
Expected: 6 tests PASS.

- [ ] **Step 3: 提交**

Run: `git add -A && git commit -m "feat(rules): add DevelopmentCard/GemCost/CardBonus and standard deck"`
非仓库则跳过。

---

## Task 6: 贵族 (`noble.rs`)

**Files:**
- Modify: `src/rules/noble.rs`
- Test: 内联

- [ ] **Step 1: 写实现与测试**

替换 `src/rules/noble.rs` 全部内容：

```rust
//! 贵族牌与公共贵族区。

use std::collections::HashMap;

use crate::rules::card::{CardBonus, GemCost};

pub type NobleId = u8;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Noble {
    pub id: NobleId,
    pub prestige: u8,
    pub requirement: GemCost,
}

#[derive(Clone, Default)]
pub struct NobleStore {
    map: HashMap<NobleId, Noble>,
}

impl NobleStore {
    pub fn from_nobles(nobles: &[Noble]) -> Self {
        let map = nobles.iter().copied().map(|n| (n.id, n)).collect();
        Self { map }
    }

    pub fn get(&self, id: NobleId) -> Option<&Noble> {
        self.map.get(&id)
    }
}

/// 公共贵族区：可见可被拜访的贵族 + 已被带走的（便于 UI 展示）。
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct NobleBoard {
    pub available: Vec<Noble>,
    pub taken: Vec<NobleId>,
}

impl NobleBoard {
    pub fn take(&mut self, id: NobleId) -> Option<Noble> {
        let pos = self.available.iter().position(|n| n.id == id)?;
        let noble = self.available.remove(pos);
        self.taken.push(id);
        Some(noble)
    }

    /// 返回玩家 bonus 已满足的 available 贵族 id。
    pub fn eligible(&self, bonus: CardBonus) -> Vec<NobleId> {
        self.available
            .iter()
            .filter(|n| bonus.satisfies(n.requirement))
            .map(|n| n.id)
            .collect()
    }
}

/// 标准贵族牌池（10 张，各 3 分）。requirement 为 [W,B,G,R,K]。
/// 数值由作者凭记忆录入，可能存在偏差；统计特征由测试锁定（10 张、皆 3 分）。
pub fn standard_nobles() -> Vec<Noble> {
    let mk = |id: NobleId, req: [u8; 5]| Noble {
        id,
        prestige: 3,
        requirement: GemCost { white: req[0], blue: req[1], green: req[2], red: req[3], black: req[4] },
    };
    vec![
        mk(0, [4, 4, 0, 0, 0]),
        mk(1, [4, 0, 4, 0, 0]),
        mk(2, [0, 4, 0, 4, 0]),
        mk(3, [0, 0, 4, 0, 4]),
        mk(4, [4, 0, 0, 0, 4]),
        mk(5, [3, 3, 3, 0, 0]),
        mk(6, [3, 0, 3, 3, 0]),
        mk(7, [0, 3, 0, 3, 3]),
        mk(8, [3, 0, 3, 0, 3]),
        mk(9, [0, 3, 3, 3, 0]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_nobles_are_ten_three_pointers() {
        let nobles = standard_nobles();
        assert_eq!(nobles.len(), 10);
        assert!(nobles.iter().all(|n| n.prestige == 3));
        let mut ids: Vec<_> = nobles.iter().map(|n| n.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 10);
    }

    #[test]
    fn board_take_moves_to_taken() {
        let mut board = NobleBoard { available: standard_nobles(), taken: vec![] };
        let taken = board.take(0).unwrap();
        assert_eq!(taken.id, 0);
        assert!(board.available.iter().all(|n| n.id != 0));
        assert_eq!(board.taken, vec![0]);
    }

    #[test]
    fn eligible_filters_by_bonus() {
        let board = NobleBoard { available: standard_nobles(), taken: vec![] };
        let bonus = CardBonus { white: 4, blue: 4, ..Default::default() };
        let elig = board.eligible(bonus);
        assert!(elig.contains(&0)); // 4W 4B
        assert!(!elig.contains(&2));
    }
}
```

- [ ] **Step 2: 运行测试**

Run: `cargo test --lib rules::noble`
Expected: 3 tests PASS.

- [ ] **Step 3: 提交**

Run: `git add -A && git commit -m "feat(rules): add Noble/NobleBoard and standard nobles"`
非仓库则跳过。

---

## Task 7: 玩家状态 (`player.rs`)

**Files:**
- Modify: `src/rules/player.rs`
- Test: 内联

- [ ] **Step 1: 写实现与测试**

替换 `src/rules/player.rs` 全部内容：

```rust
//! 玩家状态：筹码、保留牌、已购牌、贵族、折扣、分数。

use crate::rules::card::{CardBonus, CardStore};
use crate::rules::color::{CardColor, GemColor, PlayerId};
use crate::rules::noble::{NobleId, NobleStore};
use crate::rules::token::TokenSet;

/// 保留牌上限。
pub const RESERVED_LIMIT: usize = 3;
/// 玩家筹码上限。
pub const TOKEN_LIMIT: u8 = 10;
/// 触发终局的分数。
pub const WIN_SCORE: u16 = 15;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PlayerState {
    pub id: PlayerId,
    pub tokens: TokenSet,
    pub reserved_cards: Vec<crate::rules::card::CardId>,
    pub purchased_cards: Vec<crate::rules::card::CardId>,
    pub nobles: Vec<NobleId>,
}

impl PlayerState {
    pub fn new(id: PlayerId) -> Self {
        Self {
            id,
            tokens: TokenSet::default(),
            reserved_cards: Vec::new(),
            purchased_cards: Vec::new(),
            nobles: Vec::new(),
        }
    }

    /// 已购发展卡按色计数 = 购买折扣。
    pub fn bonus(&self, store: &CardStore) -> CardBonus {
        let mut bonus = CardBonus::default();
        for &id in &self.purchased_cards {
            if let Some(card) = store.get(id) {
                bonus.add(card.color);
            }
        }
        bonus
    }

    pub fn token_count(self, color: GemColor) -> u8 {
        self.tokens.get(color)
    }

    pub fn token_total(self) -> u8 {
        self.tokens.total()
    }

    pub fn reserved_full(&self) -> bool {
        self.reserved_cards.len() >= RESERVED_LIMIT
    }

    /// 卡分 + 贵族分。
    pub fn score(&self, cards: &CardStore, nobles: &NobleStore) -> u16 {
        let card_score: u16 = self
            .purchased_cards
            .iter()
            .filter_map(|id| store_get_prestige(cards, *id))
            .sum();
        let noble_score: u16 = self
            .nobles
            .iter()
            .filter_map(|id| store_get_noble_prestige(nobles, *id))
            .sum();
        card_score + noble_score
    }
}

fn store_get_prestige(store: &CardStore, id: crate::rules::card::CardId) -> Option<u16> {
    store.get(id).map(|c| u16::from(c.prestige))
}

fn store_get_noble_prestige(store: &NobleStore, id: NobleId) -> Option<u16> {
    store.get(id).map(|n| u16::from(n.prestige))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::card::{DevelopmentCard, GemCost};
    use crate::rules::noble::Noble;

    fn store_with(cards: &[DevelopmentCard]) -> CardStore {
        CardStore::from_cards(cards)
    }

    #[test]
    fn bonus_counts_purchased_by_color() {
        let c = DevelopmentCard { id: 1, level: crate::rules::card::CardLevel::Level1, color: CardColor::White, prestige: 1, cost: GemCost::default() };
        let store = store_with(&[c]);
        let mut p = PlayerState::new(0);
        p.purchased_cards.push(1);
        p.purchased_cards.push(1);
        let bonus = p.bonus(&store);
        assert_eq!(bonus.white, 2);
        assert_eq!(bonus.blue, 0);
    }

    #[test]
    fn score_sums_cards_and_nobles() {
        let c = DevelopmentCard { id: 1, level: crate::rules::card::CardLevel::Level1, color: CardColor::White, prestige: 2, cost: GemCost::default() };
        let n = Noble { id: 0, prestige: 3, requirement: GemCost::default() };
        let store = store_with(&[c]);
        let nstore = NobleStore::from_nobles(&[n]);
        let mut p = PlayerState::new(0);
        p.purchased_cards.push(1);
        p.nobles.push(0);
        assert_eq!(p.score(&store, &nstore), 5);
    }

    #[test]
    fn reserved_full_at_three() {
        let mut p = PlayerState::new(0);
        assert!(!p.reserved_full());
        p.reserved_cards.extend([1, 2, 3]);
        assert!(p.reserved_full());
    }
}
```

- [ ] **Step 2: 运行测试**

Run: `cargo test --lib rules::player`
Expected: 3 tests PASS.

- [ ] **Step 3: 提交**

Run: `git add -A && git commit -m "feat(rules): add PlayerState with bonus/score"`
非仓库则跳过。

---

## Task 8: 牌堆与市场 (`market.rs`)

**Files:**
- Modify: `src/rules/market.rs`
- Test: 内联

- [ ] **Step 1: 写实现与测试**

替换 `src/rules/market.rs` 全部内容：

```rust
//! 牌堆与公共市场。

use crate::rules::card::{CardId, CardLevel, DevelopmentCard};

const VISIBLE_PER_LEVEL: usize = 4;

#[derive(Clone, Debug)]
pub struct CardDecks {
    pub level1: Vec<DevelopmentCard>,
    pub level2: Vec<DevelopmentCard>,
    pub level3: Vec<DevelopmentCard>,
}

impl CardDecks {
    pub fn deck_mut(&mut self, level: CardLevel) -> &mut Vec<DevelopmentCard> {
        match level {
            CardLevel::Level1 => &mut self.level1,
            CardLevel::Level2 => &mut self.level2,
            CardLevel::Level3 => &mut self.level3,
        }
    }

    pub fn deck(&self, level: CardLevel) -> &Vec<DevelopmentCard> {
        match level {
            CardLevel::Level1 => &self.level1,
            CardLevel::Level2 => &self.level2,
            CardLevel::Level3 => &self.level3,
        }
    }

    pub fn pop(&mut self, level: CardLevel) -> Option<DevelopmentCard> {
        self.deck_mut(level).pop()
    }

    pub fn remaining(&self, level: CardLevel) -> usize {
        self.deck(level).len()
    }
}

/// 公共市场。每等级最多 4 张可见卡，存完整卡牌（取卡免查表）。
#[derive(Clone, Debug, Default)]
pub struct Market {
    pub level1_visible: Vec<DevelopmentCard>,
    pub level2_visible: Vec<DevelopmentCard>,
    pub level3_visible: Vec<DevelopmentCard>,
}

impl Market {
    pub fn visible(&self, level: CardLevel) -> &[DevelopmentCard] {
        match level {
            CardLevel::Level1 => &self.level1_visible,
            CardLevel::Level2 => &self.level2_visible,
            CardLevel::Level3 => &self.level3_visible,
        }
    }

    pub fn visible_mut(&mut self, level: CardLevel) -> &mut Vec<DevelopmentCard> {
        match level {
            CardLevel::Level1 => &mut self.level1_visible,
            CardLevel::Level2 => &mut self.level2_visible,
            CardLevel::Level3 => &mut self.level3_visible,
        }
    }

    /// 取指定等级第 idx 张（0-based）。idx 越界返回 None。
    pub fn take(&mut self, level: CardLevel, idx: usize) -> Option<DevelopmentCard> {
        let v = self.visible_mut(level);
        if idx >= v.len() {
            return None;
        }
        Some(v.remove(idx))
    }

    /// 立即从对应牌堆补一张到 4 张。返回新补入的 CardId（若补了）。
    /// 符合 rules.md §5：购买/保留可见卡后立即补牌。
    pub fn refill(&mut self, level: CardLevel, deck: &mut CardDecks) -> Option<CardId> {
        let v = self.visible_mut(level);
        while v.len() < VISIBLE_PER_LEVEL {
            match deck.pop(level) {
                Some(card) => {
                    let id = card.id;
                    v.push(card);
                    return Some(id); // 每次只补一张（取走一张只需补一张）
                }
                None => return None,
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::card::{CardColor, GemCost};

    fn card(id: CardId, level: CardLevel) -> DevelopmentCard {
        DevelopmentCard { id, level, color: CardColor::White, prestige: 0, cost: GemCost::default() }
    }

    #[test]
    fn take_removes_by_index() {
        let mut m = Market::default();
        m.level1_visible = vec![card(1, CardLevel::Level1), card(2, CardLevel::Level1)];
        let taken = m.take(CardLevel::Level1, 0).unwrap();
        assert_eq!(taken.id, 1);
        assert_eq!(m.visible(CardLevel::Level1).len(), 1);
    }

    #[test]
    fn refill_pulls_from_deck_until_four() {
        let mut deck = CardDecks { level1: vec![card(5, CardLevel::Level1), card(6, CardLevel::Level1)], level2: vec![], level3: vec![] };
        let mut m = Market { level1_visible: vec![card(1, CardLevel::Level1), card(2, CardLevel::Level1), card(3, CardLevel::Level1)], level2_visible: vec![], level3_visible: vec![] };
        let id = m.refill(CardLevel::Level1, &mut deck);
        assert_eq!(id, Some(5));
        assert_eq!(m.visible(CardLevel::Level1).len(), 4);
    }

    #[test]
    fn refill_returns_none_when_deck_empty_and_not_full() {
        let mut deck = CardDecks { level1: vec![], level2: vec![], level3: vec![] };
        let mut m = Market { level1_visible: vec![card(1, CardLevel::Level1)], level2_visible: vec![], level3_visible: vec![] };
        assert_eq!(m.refill(CardLevel::Level1, &mut deck), None);
        assert_eq!(m.visible(CardLevel::Level1).len(), 1);
    }
}
```

- [ ] **Step 2: 运行测试**

Run: `cargo test --lib rules::market`
Expected: 3 tests PASS.

- [ ] **Step 3: 提交**

Run: `git add -A && git commit -m "feat(rules): add CardDecks and Market with immediate refill"`
非仓库则跳过。

---

## Task 9: 事件 (`events.rs`)

**Files:**
- Modify: `src/rules/events.rs`
- Test: 内联（仅构造断言，确保字段齐备）

- [ ] **Step 1: 写实现与测试**

替换 `src/rules/events.rs` 全部内容：

```rust
//! 规则层产生的事件，供 UI 据此播放动画。

use crate::rules::card::{CardId, CardLevel};
use crate::rules::color::PlayerId;
use crate::rules::noble::NobleId;
use crate::rules::token::TokenSet;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum GameEvent {
    TokensTaken { player: PlayerId, tokens: TokenSet },
    TokensReturned { player: PlayerId, tokens: TokenSet },
    CardReserved { player: PlayerId, card: CardId, from_deck: bool, got_gold: bool },
    CardPurchased { player: PlayerId, card: CardId, paid: TokenSet },
    MarketRefilled { level: CardLevel, card: Option<CardId> },
    NobleVisited { player: PlayerId, noble: NobleId },
    EndGameTriggered { player: PlayerId },
    GameOver { winner: PlayerId, standings: Vec<(PlayerId, u16)> },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_construct_with_all_fields() {
        let e = GameEvent::CardPurchased { player: 0, card: 7, paid: TokenSet::default() };
        assert!(matches!(e, GameEvent::CardPurchased { player: 0, card: 7, .. }));
        let g = GameEvent::GameOver { winner: 1, standings: vec![(1, 15), (0, 12)] };
        assert!(matches!(g, GameEvent::GameOver { winner: 1, .. }));
    }
}
```

- [ ] **Step 2: 运行测试**

Run: `cargo test --lib rules::events`
Expected: 1 test PASS.

- [ ] **Step 3: 提交**

Run: `git add -A && git commit -m "feat(rules): add GameEvent"`
非仓库则跳过。

---

## Task 10: 计分与胜负 (`scoring.rs`)

**Files:**
- Modify: `src/rules/scoring.rs`
- Test: 内联

- [ ] **Step 1: 写实现与测试**

替换 `src/rules/scoring.rs` 全部内容：

```rust
//! 计分、胜负比较、贵族资格。

use std::cmp::Ordering;

use crate::rules::card::CardStore;
use crate::rules::color::PlayerId;
use crate::rules::noble::{NobleBoard, NobleStore};
use crate::rules::player::PlayerState;

/// 玩家总分 = 卡分 + 贵族分。
pub fn calculate_score(player: &PlayerState, cards: &CardStore, nobles: &NobleStore) -> u16 {
    player.score(cards, nobles)
}

/// 胜负排序：分数降序；分数相同则已购发展卡数升序（买牌少者胜）。
/// 返回 `Ordering` 用于 `sort_by`，使胜者排在最前。
pub fn compare_players(
    a: &PlayerState,
    b: &PlayerState,
    cards: &CardStore,
    nobles: &NobleStore,
) -> Ordering {
    let sa = calculate_score(a, cards, nobles);
    let sb = calculate_score(b, cards, nobles);
    sa.cmp(&sb)
        .reverse() // 降序：高分在前
        .then_with(|| {
            // 分数相同：买牌少者在前
            b.purchased_cards.len().cmp(&a.purchased_cards.len())
        })
}

/// 玩家 bonus 已满足的 available 贵族 id 列表。
pub fn eligible_nobles(
    player: &PlayerState,
    board: &NobleBoard,
    cards: &CardStore,
) -> Vec<NobleId> {
    let bonus = player.bonus(cards);
    board.eligible(bonus)
}

/// 返回按胜者优先排序的 (player_id, score) 列表。
pub fn standings(
    players: &[PlayerState],
    cards: &CardStore,
    nobles: &NobleStore,
) -> Vec<(PlayerId, u16)> {
    let mut indexed: Vec<&PlayerState> = players.iter().collect();
    indexed.sort_by(|a, b| compare_players(a, b, cards, nobles));
    indexed
        .iter()
        .map(|p| (p.id, calculate_score(p, cards, nobles)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::card::{DevelopmentCard, GemCost};
    use crate::rules::color::CardColor;
    use crate::rules::noble::Noble;
    use crate::rules::player::PlayerState;

    fn stores() -> (CardStore, NobleStore) {
        let c = DevelopmentCard { id: 1, level: crate::rules::card::CardLevel::Level1, color: CardColor::White, prestige: 2, cost: GemCost::default() };
        let n = Noble { id: 0, prestige: 3, requirement: GemCost::default() };
        (CardStore::from_cards(&[c]), NobleStore::from_nobles(&[n]))
    }

    #[test]
    fn higher_score_ranks_first() {
        let (cs, ns) = stores();
        let mut a = PlayerState::new(0);
        a.purchased_cards.push(1); // 2 分
        let b = PlayerState::new(1);
        assert_eq!(compare_players(&a, &b, &cs, &ns), Ordering::Less); // a 在前
    }

    #[test]
    fn tie_broken_by_fewer_purchased() {
        let (cs, ns) = stores();
        let mut a = PlayerState::new(0);
        a.purchased_cards.push(1); // 2 分, 1 牌
        let mut b = PlayerState::new(1);
        b.purchased_cards.push(1);
        b.purchased_cards.push(1); // 同 2 分(id 重复仅测试用), 2 牌
        assert_eq!(compare_players(&a, &b, &cs, &ns), Ordering::Less); // a 买牌少在前
    }

    #[test]
    fn standings_winner_first() {
        let (cs, ns) = stores();
        let mut a = PlayerState::new(0);
        a.purchased_cards.push(1);
        let b = PlayerState::new(1);
        let s = standings(&[b, a], &cs, &ns);
        assert_eq!(s[0], (0, 2));
    }
}
```

- [ ] **Step 2: 运行测试**

Run: `cargo test --lib rules::scoring`
Expected: 3 tests PASS.

- [ ] **Step 3: 提交**

Run: `git add -A && git commit -m "feat(rules): add scoring/standings/eligible_nobles"`
非仓库则跳过。

---

## Task 11: 校验谓词 (`validation.rs`)

**Files:**
- Modify: `src/rules/validation.rs`
- Test: 内联

> 本任务只实现**纯校验谓词**（不改 state）。`validate_action` 顶层分发在 Task 13 actions 中补，因为它需引用 `PlayerAction`。

- [ ] **Step 1: 写实现与测试**

替换 `src/rules/validation.rs` 全部内容：

```rust
//! 动作合法性校验。纯函数，不改 GameState。

use crate::rules::card::{CardBonus, CardStore, DevelopmentCard};
use crate::rules::color::{CardColor, GemColor};
use crate::rules::error::RuleError;
use crate::rules::player::RESERVED_LIMIT;
use crate::rules::token::TokenSet;

/// 拿 3 个不同普通色宝石的合法性（rules.md §11）。
pub fn can_take_three_different(player_tokens: TokenSet, bank: TokenSet, colors: &[GemColor]) -> Result<(), RuleError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::card::{CardLevel, GemCost};

    fn card(cost: GemCost) -> DevelopmentCard {
        DevelopmentCard { id: 1, level: CardLevel::Level1, color: CardColor::White, prestige: 0, cost }
    }

    #[test]
    fn three_different_rejects_duplicates() {
        let bank = TokenSet { white: 4, blue: 4, green: 4, ..Default::default() };
        let r = can_take_three_different(TokenSet::default(), bank, &[GemColor::White, GemColor::White, GemColor::Blue]);
        assert_eq!(r, Err(RuleError::InvalidTokenSelection));
    }

    #[test]
    fn three_different_rejects_gold() {
        let bank = TokenSet { gold: 5, white: 4, blue: 4, ..Default::default() };
        let r = can_take_three_different(TokenSet::default(), bank, &[GemColor::White, GemColor::Blue, GemColor::Gold]);
        assert_eq!(r, Err(RuleError::InvalidTokenSelection));
    }

    #[test]
    fn three_different_rejects_when_bank_low() {
        let bank = TokenSet { white: 0, blue: 4, green: 4, ..Default::default() };
        let r = can_take_three_different(TokenSet::default(), bank, &[GemColor::White, GemColor::Blue, GemColor::Green]);
        assert_eq!(r, Err(RuleError::BankInsufficient));
    }

    #[test]
    fn two_same_needs_four_in_bank() {
        let bank = TokenSet { red: 3, ..Default::default() };
        assert_eq!(can_take_two_same(bank, GemColor::Red), Err(RuleError::BankInsufficient));
        let bank2 = TokenSet { red: 4, ..Default::default() };
        assert!(can_take_two_same(bank2, GemColor::Red).is_ok());
    }

    #[test]
    fn two_same_rejects_gold() {
        let bank = TokenSet { gold: 5, ..Default::default() };
        assert_eq!(can_take_two_same(bank, GemColor::Gold), Err(RuleError::InvalidTokenSelection));
    }

    #[test]
    fn reserve_limit_enforced() {
        assert!(can_reserve(2).is_ok());
        assert_eq!(can_reserve(3), Err(RuleError::TooManyReserved));
    }

    #[test]
    fn can_afford_with_discount_and_gold() {
        // 卡费 白3 蓝2；玩家 白1 蓝3 金1；bonus 0。
        let c = card(GemCost { white: 3, blue: 2, ..Default::default() });
        let tokens = TokenSet { white: 1, blue: 3, gold: 1, ..Default::default() };
        // 折扣后 白2 蓝0 -> 白缺1 -> 金1 够。
        assert!(can_afford(tokens, &c, CardBonus::default()).is_ok());
        let tokens2 = TokenSet { white: 1, blue: 3, gold: 0, ..Default::default() };
        assert_eq!(can_afford(tokens2, &c, CardBonus::default()), Err(RuleError::CannotAfford));
    }

    #[test]
    fn plan_payment_uses_gold_for_shortfall() {
        let c = card(GemCost { white: 3, blue: 2, ..Default::default() });
        let tokens = TokenSet { white: 2, blue: 2, gold: 1, ..Default::default() };
        let (paid, gold) = plan_payment(tokens, &c, CardBonus::default());
        assert_eq!(paid.get(GemColor::White), 2);
        assert_eq!(paid.get(GemColor::Blue), 2);
        assert_eq!(gold, 1); // 白缺1，金补1
        assert_eq!(paid.get(GemColor::Gold), 1);
    }
}
```

- [ ] **Step 2: 运行测试**

Run: `cargo test --lib rules::validation`
Expected: 8 tests PASS.

- [ ] **Step 3: 提交**

Run: `git add -A && git commit -m "feat(rules): add validation predicates and payment planning"`
非仓库则跳过。

---

## Task 12: 游戏状态与初始化 (`state.rs`)

**Files:**
- Modify: `src/rules/state.rs`
- Test: 内联

- [ ] **Step 1: 写实现与测试**

替换 `src/rules/state.rs` 全部内容：

```rust
//! 全局游戏状态与初始化。

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};

use crate::rules::card::{standard_deck, CardStore};
use crate::rules::color::PlayerId;
use crate::rules::error::RuleError;
use crate::rules::market::{CardDecks, Market};
use crate::rules::noble::{standard_nobles, NobleBoard, NobleStore};
use crate::rules::player::{PlayerState, TOKEN_LIMIT, WIN_SCORE};
use crate::rules::token::{Bank, TokenSet};

const VISIBLE_PER_LEVEL: usize = 4;

#[derive(Clone, Debug)]
pub struct GameState {
    pub players: Vec<PlayerState>,
    pub bank: Bank,
    pub decks: CardDecks,
    pub market: Market,
    pub nobles: NobleBoard,
    pub card_store: CardStore,
    pub noble_store: NobleStore,
    pub current_player: usize,
    pub round_start_player: usize,
    pub end_triggered: bool,
    pub winner: Option<PlayerId>,
    /// 终局轮中：当 current_player 走到此玩家的下一位时结算。
    pub final_player: Option<PlayerId>,
}

impl GameState {
    /// 按规则初始化（rules.md §4/§7/§9）。rand 仅在此使用。
    pub fn new<R: Rng + ?Sized>(player_count: usize, rng: &mut R) -> Result<Self, RuleError> {
        if !(2..=4).contains(&player_count) {
            return Err(RuleError::InvalidPlayerCount);
        }

        let deck = standard_deck();
        let card_store = CardStore::from_cards(&deck);

        let mut l1: Vec<_> = deck.iter().filter(|c| matches!(c.level, crate::rules::card::CardLevel::Level1)).copied().collect();
        let mut l2: Vec<_> = deck.iter().filter(|c| matches!(c.level, crate::rules::card::CardLevel::Level2)).copied().collect();
        let mut l3: Vec<_> = deck.iter().filter(|c| matches!(c.level, crate::rules::card::CardLevel::Level3)).copied().collect();
        l1.shuffle(rng);
        l2.shuffle(rng);
        l3.shuffle(rng);

        let mut decks = CardDecks { level1: l1, level2: l2, level3: l3 };
        let mut market = Market::default();
        for level in crate::rules::card::CardLevel::ALL {
            for _ in 0..VISIBLE_PER_LEVEL {
                if let Some(card) = decks.pop(level) {
                    market.visible_mut(level).push(card);
                }
            }
        }

        let mut nobles_pool = standard_nobles();
        nobles_pool.shuffle(rng);
        let noble_count = player_count + 1;
        let available = nobles_pool.into_iter().take(noble_count).collect();
        let nobles = NobleBoard { available, taken: vec![] };
        let noble_store = NobleStore::from_nobles(&standard_nobles());

        let normal_per_color = match player_count {
            2 => 4,
            3 => 5,
            4 => 7,
            _ => unreachable!(),
        };
        let bank = Bank {
            tokens: TokenSet {
                white: normal_per_color,
                blue: normal_per_color,
                green: normal_per_color,
                red: normal_per_color,
                black: normal_per_color,
                gold: 5,
            },
        };

        let players = (0..player_count).map(PlayerState::new).collect();

        Ok(Self {
            players,
            bank,
            decks,
            market,
            nobles,
            card_store,
            noble_store,
            current_player: 0,
            round_start_player: 0,
            end_triggered: false,
            winner: None,
            final_player: None,
        })
    }

    /// 便捷构造：固定 seed，用于测试/回放。
    pub fn new_seeded(player_count: usize, seed: u64) -> Result<Self, RuleError> {
        let mut rng = StdRng::seed_from_u64(seed);
        Self::new(player_count, &mut rng)
    }

    pub fn current(&self) -> &PlayerState {
        &self.players[self.current_player]
    }

    pub fn current_mut(&mut self) -> &mut PlayerState {
        &mut self.players[self.current_player]
    }

    pub fn current_id(&self) -> PlayerId {
        self.current_player
    }

    pub fn player(&self, id: PlayerId) -> &PlayerState {
        &self.players[id]
    }

    pub fn player_mut(&mut self, id: PlayerId) -> &mut PlayerState {
        &mut self.players[id]
    }

    pub fn current_score(&self) -> u16 {
        self.current().score(&self.card_store, &self.noble_store)
    }

    pub fn is_over(&self) -> bool {
        self.winner.is_some()
    }

    pub fn token_limit() -> u8 {
        TOKEN_LIMIT
    }

    pub fn win_score() -> u16 {
        WIN_SCORE
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::card::CardLevel;

    #[test]
    fn rejects_invalid_player_count() {
        assert_eq!(GameState::new_seeded(1, 1).unwrap_err(), RuleError::InvalidPlayerCount);
        assert_eq!(GameState::new_seeded(5, 1).unwrap_err(), RuleError::InvalidPlayerCount);
    }

    #[test]
    fn two_player_bank_is_four_per_color_and_five_gold() {
        let g = GameState::new_seeded(2, 1).unwrap();
        assert_eq!(g.bank.tokens.white, 4);
        assert_eq!(g.bank.tokens.gold, 5);
        assert_eq!(g.nobles.available.len(), 3);
    }

    #[test]
    fn four_player_bank_is_seven_and_five_nobles() {
        let g = GameState::new_seeded(4, 1).unwrap();
        assert_eq!(g.bank.tokens.red, 7);
        assert_eq!(g.nobles.available.len(), 5);
    }

    #[test]
    fn market_starts_with_four_per_level() {
        let g = GameState::new_seeded(3, 1).unwrap();
        for level in CardLevel::ALL {
            assert_eq!(g.market.visible(level).len(), 4);
        }
    }

    #[test]
    fn deck_remaining_plus_visible_equals_total() {
        let g = GameState::new_seeded(2, 1).unwrap();
        assert_eq!(g.decks.remaining(CardLevel::Level1) + g.market.visible(CardLevel::Level1).len(), 40);
        assert_eq!(g.decks.remaining(CardLevel::Level2) + g.market.visible(CardLevel::Level2).len(), 30);
        assert_eq!(g.decks.remaining(CardLevel::Level3) + g.market.visible(CardLevel::Level3).len(), 20);
    }
}
```

- [ ] **Step 2: 运行测试**

Run: `cargo test --lib rules::state`
Expected: 5 tests PASS.

- [ ] **Step 3: 提交**

Run: `git add -A && git commit -m "feat(rules): add GameState with seeded initialization"`
非仓库则跳过。

---

## Task 13: 行动 API —— `apply_action` 与 `resume` (`actions.rs`)

**Files:**
- Modify: `src/rules/actions.rs`
- Test: 内联

> 本任务是规则层核心。先写测试（红），再写实现（绿）。由于 `apply_action`/`resume` 与执行细节耦合，分两批：先写 happy-path 与边界测试，再实现，再补 resume 测试。

- [ ] **Step 1: 写第一批失败测试（拿筹码 + 保留 + 买牌 + 终局）**

在 `src/rules/actions.rs` 写测试（实现为空，编译失败即红）：

```rust
//! 行动 API：apply_action 单一入口 + resume 续接。

use crate::rules::color::{GemColor, PlayerId};
use crate::rules::error::RuleError;
use crate::rules::events::GameEvent;
use crate::rules::state::GameState;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PlayerAction {
    TakeThreeDifferentTokens(Vec<GemColor>),
    TakeTwoSameTokens(GemColor),
    ReserveVisibleCard { level: crate::rules::card::CardLevel, idx: usize },
    ReserveDeckCard(crate::rules::card::CardLevel),
    BuyVisibleCard { level: crate::rules::card::CardLevel, idx: usize },
    BuyReservedCard(usize),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ActionOutcome {
    Complete,
    NeedDiscardTokens { excess: u8 },
    NeedChooseNoble { candidates: Vec<crate::rules::noble::NobleId> },
    NeedFinalDiscardThenChooseNoble { excess: u8, candidates: Vec<crate::rules::noble::NobleId> },
}

impl ActionOutcome {
    pub fn requires_choice(&self) -> bool {
        !matches!(self, ActionOutcome::Complete)
    }
}

#[derive(Clone, Debug)]
pub struct ActionResult {
    pub outcome: ActionOutcome,
    pub events: Vec<GameEvent>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Resume {
    DiscardTokens(crate::rules::token::TokenSet),
    ChooseNoble(crate::rules::noble::NobleId),
}

// 占位：实现见 Step 3
pub fn apply_action(_state: &mut GameState, _player: PlayerId, _action: PlayerAction) -> Result<ActionResult, RuleError> {
    unimplemented!()
}
pub fn resume(_state: &mut GameState, _player: PlayerId, _resume: Resume) -> Result<ActionResult, RuleError> {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::card::{CardLevel, GemCost};
    use crate::rules::color::GemColor;
    use crate::rules::noble::Noble;
    use crate::rules::token::TokenSet;

    fn game2() -> GameState {
        GameState::new_seeded(2, 99).unwrap()
    }

    #[test]
    fn take_three_different_moves_tokens_from_bank() {
        let mut g = game2();
        let r = apply_action(&mut g, 0, PlayerAction::TakeThreeDifferentTokens(vec![GemColor::White, GemColor::Blue, GemColor::Green])).unwrap();
        assert!(matches!(r.outcome, ActionOutcome::Complete));
        assert_eq!(g.player(0).token_count(GemColor::White), 1);
        assert_eq!(g.bank.tokens.white, 3);
        assert!(r.events.iter().any(|e| matches!(e, GameEvent::TokensTaken { player: 0, .. })));
    }

    #[test]
    fn take_three_different_rejects_duplicate() {
        let mut g = game2();
        let r = apply_action(&mut g, 0, PlayerAction::TakeThreeDifferentTokens(vec![GemColor::White, GemColor::White, GemColor::Blue]));
        assert_eq!(r.unwrap_err(), RuleError::InvalidTokenSelection);
    }

    #[test]
    fn take_two_same_needs_four_in_bank() {
        let mut g = game2();
        let r = apply_action(&mut g, 0, PlayerAction::TakeTwoSameTokens(GemColor::White));
        // 2 人局每色 4，应成功。
        assert!(r.is_ok());
        assert_eq!(g.player(0).token_count(GemColor::White), 2);
    }

    #[test]
    fn take_two_same_fails_below_four() {
        // 先取 1 个白使银行降到 3。
        let mut g = game2();
        g.bank.tokens.white = 3;
        let r = apply_action(&mut g, 0, PlayerAction::TakeTwoSameTokens(GemColor::White));
        assert_eq!(r.unwrap_err(), RuleError::BankInsufficient);
    }

    #[test]
    fn reserve_visible_takes_gold_and_refills_immediately() {
        let mut g = game2();
        let gold_before = g.bank.tokens.gold;
        let r = apply_action(&mut g, 0, PlayerAction::ReserveVisibleCard { level: CardLevel::Level1, idx: 0 }).unwrap();
        assert_eq!(g.player(0).reserved_cards.len(), 1);
        assert_eq!(g.player(0).token_count(GemColor::Gold), 1);
        assert_eq!(g.bank.tokens.gold, gold_before - 1);
        // 立即补牌：可见仍 4 张。
        assert_eq!(g.market.visible(CardLevel::Level1).len(), 4);
        assert!(r.events.iter().any(|e| matches!(e, GameEvent::CardReserved { from_deck: false, got_gold: true, .. })));
        assert!(r.events.iter().any(|e| matches!(e, GameEvent::MarketRefilled { .. })));
    }

    #[test]
    fn reserve_deck_blinds_top() {
        let mut g = game2();
        let before = g.decks.remaining(CardLevel::Level1);
        let r = apply_action(&mut g, 0, PlayerAction::ReserveDeckCard(CardLevel::Level1)).unwrap();
        assert_eq!(g.player(0).reserved_cards.len(), 1);
        assert_eq!(g.decks.remaining(CardLevel::Level1), before - 1);
        assert!(r.events.iter().any(|e| matches!(e, GameEvent::CardReserved { from_deck: true, .. })));
    }

    #[test]
    fn reserve_limit_is_three() {
        let mut g = game2();
        for _ in 0..3 {
            apply_action(&mut g, 0, PlayerAction::ReserveDeckCard(CardLevel::Level1)).unwrap();
        }
        let r = apply_action(&mut g, 0, PlayerAction::ReserveDeckCard(CardLevel::Level1));
        assert_eq!(r.unwrap_err(), RuleError::TooManyReserved);
    }

    #[test]
    fn buy_visible_pays_discount_and_gold_to_bank() {
        // 构造：玩家持白2蓝2金1，买一张白3蓝2的卡（bonus 白色=0）。
        let mut g = game2();
        // 放一张已知卡到市场第 0 位。
        let card = crate::rules::card::DevelopmentCard {
            id: 999,
            level: CardLevel::Level1,
            color: crate::rules::color::CardColor::White,
            prestige: 1,
            cost: GemCost { white: 0, blue: 2, green: 0, red: 3, black: 0 },
        };
        g.market.level1_visible[0] = card;
        g.card_store = crate::rules::card::CardStore::from_cards(&[card]);
        // 给玩家白2 蓝2 红2 金2 以支付 红3（白0 蓝2 红2 不足红1，金补1）
        g.players[0].tokens = TokenSet { white: 2, blue: 2, red: 2, gold: 2, ..Default::default() };
        let bank_before = g.bank.tokens;
        let r = apply_action(&mut g, 0, PlayerAction::BuyVisibleCard { level: CardLevel::Level1, idx: 0 }).unwrap();
        assert!(matches!(r.outcome, ActionOutcome::Complete));
        assert!(g.player(0).purchased_cards.contains(&999));
        assert_eq!(g.player(0).token_count(GemColor::Red), 0); // 红2 全付
        assert_eq!(g.player(0).token_count(GemColor::Blue), 0); // 蓝2 全付
        assert_eq!(g.player(0).token_count(GemColor::Gold), 1); // 金2 付1 剩1
        // 支付的筹码回到银行。
        assert_eq!(g.bank.tokens.red, bank_before.red + 2);
        assert_eq!(g.bank.tokens.blue, bank_before.blue + 2);
        assert_eq!(g.bank.tokens.gold, bank_before.gold + 1);
        assert!(r.events.iter().any(|e| matches!(e, GameEvent::CardPurchased { player: 0, card: 999, .. })));
    }

    #[test]
    fn buy_cannot_afford() {
        let mut g = game2();
        let card = crate::rules::card::DevelopmentCard {
            id: 999,
            level: CardLevel::Level1,
            color: crate::rules::color::CardColor::White,
            prestige: 0,
            cost: GemCost { white: 0, blue: 5, green: 0, red: 0, black: 0 },
        };
        g.market.level1_visible[0] = card;
        g.card_store = crate::rules::card::CardStore::from_cards(&[card]);
        let r = apply_action(&mut g, 0, PlayerAction::BuyVisibleCard { level: CardLevel::Level1, idx: 0 });
        assert_eq!(r.unwrap_err(), RuleError::CannotAfford);
    }

    #[test]
    fn token_limit_triggers_discard() {
        // 玩家已持 9 筹码，拿 3 不同 -> 12，超 10，excess=2。
        let mut g = game2();
        g.players[0].tokens = TokenSet { white: 3, blue: 3, green: 3, ..Default::default() }; // 9 个
        let r = apply_action(&mut g, 0, PlayerAction::TakeThreeDifferentTokens(vec![GemColor::White, GemColor::Blue, GemColor::Green])).unwrap();
        assert!(matches!(r.outcome, ActionOutcome::NeedDiscardTokens { excess: 2 }));
        // 回合未推进（仍玩家 0）。
        assert_eq!(g.current_player, 0);
    }

    #[test]
    fn end_game_triggers_at_fifteen_and_finishes_round() {
        // 玩家 0 直接给 14 分，买一张 1 分卡 -> 15 触发。
        let mut g = game2();
        // 给玩家 0 一张已购的 14 分卡（构造 store 支持）。
        let big = crate::rules::card::DevelopmentCard { id: 1000, level: CardLevel::Level3, color: crate::rules::color::CardColor::White, prestige: 14, cost: GemCost::default() };
        let target = crate::rules::card::DevelopmentCard { id: 1001, level: CardLevel::Level1, color: crate::rules::color::CardColor::White, prestige: 1, cost: GemCost::default() };
        g.card_store = crate::rules::card::CardStore::from_cards(&[big, target]);
        g.players[0].purchased_cards.push(1000);
        g.market.level1_visible[0] = target;
        let r = apply_action(&mut g, 0, PlayerAction::BuyVisibleCard { level: CardLevel::Level1, idx: 0 }).unwrap();
        assert!(g.end_triggered);
        assert!(r.events.iter().any(|e| matches!(e, GameEvent::EndGameTriggered { player: 0 })));
        // 2 人局：玩家 0 触发后，玩家 1 还需行动一次才结算。当前应轮到 1。
        assert_eq!(g.current_player, 1);
        assert!(g.winner.is_none());
        // 玩家 1 行动后结算。
        apply_action(&mut g, 1, PlayerAction::TakeThreeDifferentTokens(vec![GemColor::White, GemColor::Blue, GemColor::Green])).unwrap();
        assert!(g.is_over());
        assert_eq!(g.winner, Some(0));
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test --lib rules::actions`
Expected: 编译失败（`apply_action` 内 `unimplemented!`）或 panic。红。

- [ ] **Step 3: 写实现**

将 `src/rules/actions.rs` 顶部两个 `unimplemented!` 函数替换为完整实现（保留上方 `PlayerAction`/`ActionOutcome`/`ActionResult`/`Resume` 定义与测试不变）。同时删除占位 `use` 中不再需要的项，文件顶部 `use` 应为：

```rust
use crate::rules::card::CardLevel;
use crate::rules::color::{CardColor, GemColor, PlayerId};
use crate::rules::error::RuleError;
use crate::rules::events::GameEvent;
use crate::rules::noble::NobleId;
use crate::rules::player::TOKEN_LIMIT;
use crate::rules::scoring::{eligible_nobles, standings};
use crate::rules::state::GameState;
use crate::rules::token::TokenSet;
use crate::rules::validation::{
    can_afford, can_reserve, can_take_three_different, can_take_two_same, plan_payment,
};

pub fn apply_action(
    state: &mut GameState,
    player: PlayerId,
    action: PlayerAction,
) -> Result<ActionResult, RuleError> {
    if state.is_over() {
        return Err(RuleError::GameOver);
    }
    if player != state.current_id() {
        return Err(RuleError::NotYourTurn);
    }
    validate(&action, state, player)?;
    let mut events = Vec::new();
    let outcome = execute(&action, state, player, &mut events)?;
    Ok(ActionResult { outcome, events })
}

pub fn resume(
    state: &mut GameState,
    player: PlayerId,
    resume: Resume,
) -> Result<ActionResult, RuleError> {
    if state.is_over() {
        return Err(RuleError::GameOver);
    }
    if player != state.current_id() {
        return Err(RuleError::NotYourTurn);
    }
    let mut events = Vec::new();
    match resume {
        // 弃牌只发生在"拿筹码/保留得金"后；这些行动不触发贵族、不触发终局。
        // 故弃牌后只归还筹码并推进回合，无需 check_nobles/end_game。
        Resume::DiscardTokens(returned) => {
            let excess = state.player(player).token_total().saturating_sub(TOKEN_LIMIT);
            if returned.total() != excess {
                return Err(RuleError::InvalidResume);
            }
            // 逐色归还（含金），玩家须持有。
            for color in GemColor::NORMAL {
                let amt = returned.get(color);
                if amt > 0 {
                    if !state.player(player).tokens.remove(color, amt) {
                        return Err(RuleError::InvalidResume);
                    }
                    state.bank.give(color, amt);
                }
            }
            let gold = returned.get(GemColor::Gold);
            if gold > 0 {
                if !state.player(player).tokens.remove(GemColor::Gold, gold) {
                    return Err(RuleError::InvalidResume);
                }
                state.bank.give(GemColor::Gold, gold);
            }
            events.push(GameEvent::TokensReturned { player, tokens: returned });
            advance_turn(state);
            maybe_finalize(state, &mut events);
        }
        // 选贵族发生在买牌后；授予贵族后继续终局检测 + 推进。
        Resume::ChooseNoble(noble_id) => {
            let bonus = state.player(player).bonus(&state.card_store);
            let candidates = state.nobles.eligible(bonus);
            if !candidates.contains(&noble_id) {
                return Err(RuleError::NobleNotEligible);
            }
            grant_noble(state, player, noble_id, &mut events);
            check_end_and_advance(state, player, &mut events)?;
        }
    }
    Ok(ActionResult { outcome: ActionOutcome::Complete, events })
}

fn validate(action: &PlayerAction, state: &GameState, player: PlayerId) -> Result<(), RuleError> {
    match action {
        PlayerAction::TakeThreeDifferentTokens(colors) => {
            can_take_three_different(state.player(player).tokens, state.bank.tokens, colors)
        }
        PlayerAction::TakeTwoSameTokens(color) => can_take_two_same(state.bank.tokens, *color),
        PlayerAction::ReserveVisibleCard { level, idx } => {
            can_reserve(state.player(player).reserved_cards.len())?;
            if state.market.visible(*level).get(*idx).is_none() {
                return Err(RuleError::CardNotFound);
            }
            Ok(())
        }
        PlayerAction::ReserveDeckCard(level) => {
            can_reserve(state.player(player).reserved_cards.len())?;
            if state.decks.remaining(*level) == 0 {
                return Err(RuleError::DeckEmpty);
            }
            Ok(())
        }
        PlayerAction::BuyVisibleCard { level, idx } => {
            let card = state
                .market
                .visible(*level)
                .get(*idx)
                .ok_or(RuleError::CardNotFound)?;
            let bonus = state.player(player).bonus(&state.card_store);
            can_afford(state.player(player).tokens, card, bonus)
        }
        PlayerAction::BuyReservedCard(reserved_idx) => {
            let &card_id = state
                .player(player)
                .reserved_cards
                .get(*reserved_idx)
                .ok_or(RuleError::CardNotFound)?;
            let card = state.card_store.get(card_id).ok_or(RuleError::CardNotFound)?;
            let bonus = state.player(player).bonus(&state.card_store);
            can_afford(state.player(player).tokens, card, bonus)
        }
    }
}

fn execute(
    action: &PlayerAction,
    state: &mut GameState,
    player: PlayerId,
    events: &mut Vec<GameEvent>,
) -> Result<ActionOutcome, RuleError> {
    match action {
        PlayerAction::TakeThreeDifferentTokens(colors) => {
            let mut taken = TokenSet::default();
            for c in colors {
                state.bank.take(*c, 1);
                state.player_mut(player).tokens.add(*c, 1);
                taken.add(*c, 1);
            }
            events.push(GameEvent::TokensTaken { player, tokens: taken });
            Ok(discard_or_finish_tokens(state, player, events))
        }
        PlayerAction::TakeTwoSameTokens(color) => {
            state.bank.take(*color, 2);
            state.player_mut(player).tokens.add(*color, 2);
            let mut taken = TokenSet::default();
            taken.add(*color, 2);
            events.push(GameEvent::TokensTaken { player, tokens: taken });
            Ok(discard_or_finish_tokens(state, player, events))
        }
        PlayerAction::ReserveVisibleCard { level, idx } => {
            let card = state.market.take(*level, *idx).ok_or(RuleError::CardNotFound)?;
            state.player_mut(player).reserved_cards.push(card.id);
            let got_gold = reserve_gold(state, player);
            if let Some(new_id) = state.market.refill(*level, &mut state.decks) {
                events.push(GameEvent::MarketRefilled { level: *level, card: Some(new_id) });
            } else {
                events.push(GameEvent::MarketRefilled { level: *level, card: None });
            }
            events.push(GameEvent::CardReserved { player, card: card.id, from_deck: false, got_gold });
            Ok(discard_or_finish_tokens(state, player, events))
        }
        PlayerAction::ReserveDeckCard(level) => {
            let card = state.decks.pop(*level).ok_or(RuleError::DeckEmpty)?;
            state.player_mut(player).reserved_cards.push(card.id);
            let got_gold = reserve_gold(state, player);
            events.push(GameEvent::CardReserved { player, card: card.id, from_deck: true, got_gold });
            Ok(discard_or_finish_tokens(state, player, events))
        }
        PlayerAction::BuyVisibleCard { level, idx } => {
            let card = state.market.take(*level, *idx).ok_or(RuleError::CardNotFound)?;
            buy_card(state, player, card, events, true, *level)
        }
        PlayerAction::BuyReservedCard(reserved_idx) => {
            let card_id = *state
                .player(player)
                .reserved_cards
                .get(*reserved_idx)
                .ok_or(RuleError::CardNotFound)?;
            let card = *state.card_store.get(card_id).ok_or(RuleError::CardNotFound)?;
            state.player_mut(player).reserved_cards.remove(*reserved_idx);
            buy_card(state, player, card, events, false, card.level)
        }
    }
}

fn reserve_gold(state: &mut GameState, player: PlayerId) -> bool {
    if state.bank.take(GemColor::Gold, 1) {
        state.player_mut(player).tokens.add(GemColor::Gold, 1);
        true
    } else {
        false
    }
}

/// 拿筹码/保留后：若超 TOKEN_LIMIT 则挂起弃牌；否则推进回合。
/// 这些行动不触发贵族/终局，但终局轮的最后一手可能正是"拿筹码"行动——
/// 故推进后仍需检查是否到达终局轮结算点。
fn discard_or_finish_tokens(
    state: &mut GameState,
    player: PlayerId,
    events: &mut Vec<GameEvent>,
) -> ActionOutcome {
    let total = state.player(player).token_total();
    if total > TOKEN_LIMIT {
        return ActionOutcome::NeedDiscardTokens { excess: total - TOKEN_LIMIT };
    }
    advance_turn(state);
    maybe_finalize(state, events);
    ActionOutcome::Complete
}

fn buy_card(
    state: &mut GameState,
    player: PlayerId,
    card: crate::rules::card::DevelopmentCard,
    events: &mut Vec<GameEvent>,
    from_market: bool,
    level: CardLevel,
) -> Result<ActionOutcome, RuleError> {
    let bonus = state.player(player).bonus(&state.card_store);
    let (paid, _gold) = plan_payment(state.player(player).tokens, &card, bonus);
    for color in CardColor::ALL {
        let amt = paid.get(color.to_gem());
        if amt > 0 {
            state.player_mut(player).tokens.remove(color.to_gem(), amt);
            state.bank.give(color.to_gem(), amt);
        }
    }
    let gold_used = paid.get(GemColor::Gold);
    if gold_used > 0 {
        state.player_mut(player).tokens.remove(GemColor::Gold, gold_used);
        state.bank.give(GemColor::Gold, gold_used);
    }
    state.player_mut(player).purchased_cards.push(card.id);
    events.push(GameEvent::CardPurchased { player, card: card.id, paid });
    if from_market {
        if let Some(new_id) = state.market.refill(level, &mut state.decks) {
            events.push(GameEvent::MarketRefilled { level, card: Some(new_id) });
        } else {
            events.push(GameEvent::MarketRefilled { level, card: None });
        }
    }
    // 买牌只减筹码，不会触发弃牌；只需贵族选择 + 终局 + 推进。
    finish_after_buy(state, player, events)
}

/// 买牌后：检查贵族（可能挂起 NeedChooseNoble）、终局检测、推进回合。
fn finish_after_buy(
    state: &mut GameState,
    player: PlayerId,
    events: &mut Vec<GameEvent>,
) -> Result<ActionOutcome, RuleError> {
    let candidates = eligible_nobles(state.player(player), &state.nobles, &state.card_store);
    match candidates.len() {
        0 => {
            check_end_and_advance(state, player, events)?;
            Ok(ActionOutcome::Complete)
        }
        1 => {
            grant_noble(state, player, candidates[0], events);
            check_end_and_advance(state, player, events)?;
            Ok(ActionOutcome::Complete)
        }
        _ => Ok(ActionOutcome::NeedChooseNoble { candidates }),
    }
}

fn grant_noble(state: &mut GameState, player: PlayerId, noble_id: NobleId, events: &mut Vec<GameEvent>) {
    if state.nobles.take(noble_id).is_some() {
        state.player_mut(player).nobles.push(noble_id);
        events.push(GameEvent::NobleVisited { player, noble: noble_id });
    }
}

/// 终局检测 + 回合推进 + 结算。
/// 终局轮：某玩家达 15 分后 end_triggered=true 并记录 final_player；
/// 此后继续行动，直到 advance_turn 使 current_player 再次回到 final_player
/// （即触发者之后的玩家都已完成一轮），结算胜负。
fn check_end_and_advance(
    state: &mut GameState,
    player: PlayerId,
    events: &mut Vec<GameEvent>,
) -> Result<(), RuleError> {
    let score = state.player(player).score(&state.card_store, &state.noble_store);
    if !state.end_triggered && score >= GameState::win_score() {
        state.end_triggered = true;
        state.final_player = Some(player);
        events.push(GameEvent::EndGameTriggered { player });
    }
    advance_turn(state);
    maybe_finalize(state, events);
    Ok(())
}

fn advance_turn(state: &mut GameState) {
    let n = state.players.len();
    state.current_player = (state.current_player + 1) % n;
}

/// 终局轮结算检查：end_triggered 且 current 回到 final_player 且未结算时，结算。
fn maybe_finalize(state: &mut GameState, events: &mut Vec<GameEvent>) {
    if state.end_triggered
        && state.current_player == state.final_player.unwrap_or(0)
        && state.winner.is_none()
    {
        finalize_game(state, events);
    }
}

fn finalize_game(state: &mut GameState, events: &mut Vec<GameEvent>) {
    let s = standings(&state.players, &state.card_store, &state.noble_store);
    let winner = s.first().map(|(id, _)| *id);
    state.winner = winner;
    events.push(GameEvent::GameOver { winner: winner.unwrap_or(0), standings: s });
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test --lib rules::actions`
Expected: 全部测试 PASS。若有失败，按失败信息修正（常见：终局轮判定、补牌时机）。

- [ ] **Step 5: 补 resume 测试（弃牌 + 选贵族）**

在 `src/rules/actions.rs` 测试模块末尾追加：

```rust
    #[test]
    fn resume_discard_returns_tokens_and_advances() {
        let mut g = game2();
        g.players[0].tokens = TokenSet { white: 3, blue: 3, green: 3, ..Default::default() }; // 9
        let r = apply_action(&mut g, 0, PlayerAction::TakeThreeDifferentTokens(vec![GemColor::White, GemColor::Blue, GemColor::Green])).unwrap();
        let excess = match r.outcome { ActionOutcome::NeedDiscardTokens { excess } => excess, _ => panic!() };
        assert_eq!(excess, 2);
        let returned = TokenSet { white: 2, ..Default::default() };
        let r2 = resume(&mut g, 0, Resume::DiscardTokens(returned)).unwrap();
        assert!(matches!(r2.outcome, ActionOutcome::Complete));
        assert_eq!(g.player(0).token_total(), 10);
        assert_eq!(g.current_player, 1);
    }

    #[test]
    fn resume_choose_noble_grants_noble() {
        let mut g = game2();
        // 构造玩家满足两个贵族。
        let big = crate::rules::card::DevelopmentCard { id: 2000, level: CardLevel::Level3, color: crate::rules::color::CardColor::White, prestige: 0, cost: GemCost::default() };
        g.card_store = crate::rules::card::CardStore::from_cards(&[big]);
        // 给玩家白色 4 蓝色 4 已购卡（满足 4W4B 与另一 3W3B3G? 此处仅造一个双候选场景）
        for _ in 0..4 {
            g.players[0].purchased_cards.push(2000); // 白色 +4
        }
        // 贵族：放两个 4W4B 与 3W3B3G 都需满足——简化：放两个相同要求 4W4B 的可用贵族。
        let n1 = crate::rules::noble::Noble { id: 50, prestige: 3, requirement: GemCost { white: 4, blue: 0, green: 0, red: 0, black: 0 } };
        let n2 = crate::rules::noble::Noble { id: 51, prestige: 3, requirement: GemCost { white: 4, blue: 0, green: 0, red: 0, black: 0 } };
        g.nobles = crate::rules::noble::NobleBoard { available: vec![n1, n2], taken: vec![] };
        g.noble_store = crate::rules::noble::NobleStore::from_nobles(&[n1, n2]);
        // 买一张 0 分卡触发贵族检查。
        let target = crate::rules::card::DevelopmentCard { id: 2001, level: CardLevel::Level1, color: crate::rules::color::CardColor::White, prestige: 0, cost: GemCost::default() };
        g.market.level1_visible[0] = target;
        g.card_store = crate::rules::card::CardStore::from_cards(&[big, target]);
        let r = apply_action(&mut g, 0, PlayerAction::BuyVisibleCard { level: CardLevel::Level1, idx: 0 }).unwrap();
        let cands = match r.outcome { ActionOutcome::NeedChooseNoble { candidates } => candidates, _ => panic!("expected noble choice") };
        assert_eq!(cands.len(), 2);
        let r2 = resume(&mut g, 0, Resume::ChooseNoble(50)).unwrap();
        assert!(matches!(r2.outcome, ActionOutcome::Complete));
        assert!(g.player(0).nobles.contains(&50));
    }
```

- [ ] **Step 6: 运行全部测试**

Run: `cargo test --lib rules::actions`
Expected: 全部 PASS（含新增 2 个 resume 测试）。

- [ ] **Step 7: 提交**

Run: `git add -A && git commit -m "feat(rules): implement apply_action/resume with full turn flow"`
非仓库则跳过。

---

## Task 14: 顶层重导出、编译与全量测试

**Files:**
- Modify: `src/rules/mod.rs`（确认 Task 1 的 `pub use` 全部生效）
- Verify: `src/main.rs`（`mod rules;` 已在 Task 1 加入）

- [ ] **Step 1: 确认 mod.rs 重导出完整**

`src/rules/mod.rs` 应为 Task 1 写入的内容（所有 `pub use` 启用）。若此前注释过，现在全部启用。

- [ ] **Step 2: 编译整个项目**

Run: `cargo build`
Expected: 编译成功，0 errors。`battle.rs`/`game.rs` 不受影响（仍用旧简化实现）。

- [ ] **Step 3: 运行全量测试**

Run: `cargo test`
Expected: rules 模块全部 PASS；旧 `game.rs` 测试也仍 PASS（未改动）。

- [ ] **Step 4: 运行 clippy（可选但推荐）**

Run: `cargo clippy -- -D warnings`
Expected: 无 warning。若有，按提示修正（常见：未用导入、`_` 前缀）。

- [ ] **Step 5: 冒烟验证 —— 手写一个完整对局脚本（测试）**

在 `src/rules/actions.rs` 测试模块末尾加一个集成测试，验证多回合流程不 panic 且状态自洽：

```rust
    #[test]
    fn smoke_full_game_does_not_panic_and_terminates() {
        let mut g = GameState::new_seeded(3, 7).unwrap();
        let mut turns = 0;
        while !g.is_over() && turns < 2000 {
            let pid = g.current_id();
            // 简单策略：优先尝试买可见卡，否则拿 3 不同筹码。
            let bought = try_buy_first_affordable(&mut g, pid);
            if !bought {
                let colors: Vec<GemColor> = GemColor::NORMAL
                    .iter()
                    .copied()
                    .filter(|c| g.bank.tokens.get(*c) >= 1)
                    .take(3)
                    .collect();
                if colors.len() == 3 {
                    let r = apply_action(&mut g, pid, PlayerAction::TakeThreeDifferentTokens(colors)).unwrap();
                    if let ActionOutcome::NeedDiscardTokens { .. } = r.outcome {
                        // 简单弃牌：归还全部金 + 任意直到 10。
                        let over = g.player(pid).token_total() - crate::rules::player::TOKEN_LIMIT;
                        let mut ret = TokenSet::default();
                        let mut to_ret = over;
                        for c in GemColor::NORMAL {
                            if to_ret == 0 { break; }
                            let have = g.player(pid).token_count(c);
                            let give = have.min(to_ret);
                            ret.add(c, give);
                            to_ret -= give;
                        }
                        resume(&mut g, pid, Resume::DiscardTokens(ret)).unwrap();
                    }
                } else {
                    // 无法行动：保留一张牌堆顶（若可）。
                    if let Ok(_) = apply_action(&mut g, pid, PlayerAction::ReserveDeckCard(crate::rules::card::CardLevel::Level1)) {
                        // 若触发弃牌，简单归还金。
                        if g.player(pid).token_total() > crate::rules::player::TOKEN_LIMIT {
                            let over = g.player(pid).token_total() - crate::rules::player::TOKEN_LIMIT;
                            let mut ret = TokenSet::default();
                            ret.set(GemColor::Gold, over.min(g.player(pid).token_count(GemColor::Gold)));
                            resume(&mut g, pid, Resume::DiscardTokens(ret)).ok();
                        }
                    } else {
                        break;
                    }
                }
            }
            turns += 1;
        }
        // 2000 步内应能结束（或至少不 panic）。
        assert!(turns < 2000, "game did not terminate");
    }

    fn try_buy_first_affordable(g: &mut GameState, pid: PlayerId) -> bool {
        use crate::rules::validation::can_afford;
        for level in crate::rules::card::CardLevel::ALL {
            let visible: Vec<_> = g.market.visible(level).to_vec();
            for (idx, card) in visible.iter().enumerate() {
                let bonus = g.player(pid).bonus(&g.card_store);
                if can_afford(g.player(pid).tokens, card, bonus).is_ok() {
                    let r = apply_action(g, pid, PlayerAction::BuyVisibleCard { level, idx }).unwrap();
                    if let ActionOutcome::NeedChooseNoble { candidates } = r.outcome {
                        resume(g, pid, Resume::ChooseNoble(candidates[0])).unwrap();
                    }
                    return true;
                }
            }
        }
        false
    }
```

- [ ] **Step 6: 运行冒烟测试**

Run: `cargo test --lib rules::actions::tests::smoke_full_game_does_not_panic_and_terminates`
Expected: PASS（游戏在 2000 步内结束）。若不结束，检查终局轮判定逻辑（`check_end_and_advance` 的 `current_player == final_player` 条件）。

- [ ] **Step 7: 最终全量验证**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: 全绿，无 warning。

- [ ] **Step 8: 提交**

Run: `git add -A && git commit -m "test(rules): add smoke full-game test; finalize rules layer"`
非仓库则跳过。

---

## 完成标准 (Definition of Done)

- [ ] `src/rules/` 13 个文件全部就位，`mod rules;` 在 `main.rs` 注册
- [ ] `cargo build` 0 errors
- [ ] `cargo test` 全绿（rules 各模块 + 旧 game.rs 测试不受影响）
- [ ] `cargo clippy -- -D warnings` 无 warning
- [ ] 冒烟测试：3 人局能在 2000 步内自然结束并产出 winner
- [ ] 旧 `src/game.rs` 与 `src/battle.rs` 未被修改，仍可独立编译运行
- [ ] 规则层零 Bevy 依赖（`src/rules/` 内无 `use bevy::`）

## 不在本计划范围

- 修改 `src/game.rs` 或 `src/battle.rs` 适配新规则层（留待后续 plan）
- Bevy System 接入、动画、输入、UI 适配
- 存档/读档、网络多人、AI 对手
- 真品牌库数值校对（当前为作者凭记忆录入，统计特征已由测试锁定；发现具体数值偏差时替换 `standard_deck()` 对应行即可）



