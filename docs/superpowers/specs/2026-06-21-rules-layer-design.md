# 璀璨宝石规则层设计 (Rules Layer Design)

- 日期: 2026-06-21
- 状态: 已批准 (设计阶段)
- 依据: `docs/rules.md`
- 范围: 仅纯 Rust 规则层 (新建 `src/rules/`)；旧 `src/game.rs` 与 `src/battle.rs` 本阶段不动

---

## 1. 背景与目标

`docs/rules.md` 是一份按"可编码规则"角度拆解的《璀璨宝石 / Splendor》规则说明，明确建议先做**纯 Rust 规则层**，再接 Bevy ECS。

当前 `src/game.rs` 是一份简化的示例实现，与规则严重不符：

- 无金色筹码、无保留牌、无贵族、无 15 分终局
- 拿牌免费 (无费用支付、无折扣)
- 拿筹码无"3 不同 / 2 同 (≥4)"约束
- 无 10 筹码上限、无弃牌、无贵族选择
- 牌库是程序生成的占位卡 (每级 12 张)，而非真实 90 张
- 公共区在**回合末**补牌，而非购买/保留后**立即**补
- 行动只有 `TakeCard` / `TakeToken` / `EndTurn` 三种

`src/battle.rs` (1300+ 行) 是一套精致的 Bevy UI，与上述简化 API 深度耦合。

本设计的目标：**新建一个符合 `rules.md` 的纯 Rust 规则层**，零 Bevy 依赖，可独立测试，为后续 UI 适配打基础。本阶段不修改 `game.rs` 与 `battle.rs`，二者保持原样编译运行。

## 2. 关键决策

| 决策 | 选择 |
|---|---|
| 范围 | 仅纯规则层 (新建 `src/rules/`，旧 `game.rs` 保留不动) |
| 牌库数据 | 内置真实璀璨宝石 90 张牌 + 贵族数据 |
| 玩家数 | 2–4 人，按 rules.md §9 调整筹码与贵族数 |
| 随机性 | 引入 `rand` crate，仅在 `GameState::new` 初始化时使用 |
| 行动 API | `apply_action` 单一入口 + `resume` 续接方法 |
| 事件 | 返回 `GameEvent` 列表供 UI 消费 |
| 代码结构 | 方案 A：按 rules.md §22 逐文件拆分 (13 个小文件) |

## 3. 架构总览

规则层为纯 Rust 库，位于 `src/rules/`，**零 Bevy 依赖**。`main.rs` 的 `mod rules;` 把它纳入编译，但 `battle.rs` / `game.rs` 本阶段不调用它 (旧的简化实现保持原样，互不干扰)。

```
调用方(Bevy) ──> apply_action(state, player, action) ──> (ActionOutcome, Vec<GameEvent>)
                            │                                    │
                            ▼                                    ▼
                   validation.rs 校验                     事件供UI播放动画
                            │
                            ▼
                  execute → 改 GameState → check_nobles → check_end_game → advance_turn
```

核心约束：

- **纯函数式状态机**：`GameState` 是唯一可变状态；所有动作经 `apply_action` 单一入口；需玩家选择 (弃牌/选贵族) 时返回挂起态，由 `resume_*` 续接。
- **规则与显示解耦**：规则层只返回 `GameEvent`，不产生任何 Bevy 副作用。
- **可回放/可测试**：`rand` 仅在 `GameState::new` 初始化时使用；`GameState` 之后不再持随机源，给定初始状态则动作序列确定。

## 4. 数据模型

### 4.1 颜色 (`color.rs`)

```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum GemColor { White, Blue, Green, Red, Black, Gold }

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum CardColor { White, Blue, Green, Red, Black }  // 不含 Gold
```

`GemColor` 含金 (万能支付资源，不能直接拿、只能保留时获得)。`CardColor` 是发展卡的颜色，不含金。两者分开，避免"金色卡牌"这类非法状态。`GemColor::NORMAL` 返回 5 种普通色，`GemColor::index()` 给 `0..=5` (金 = 5)。

### 4.2 筹码 (`token.rs`)

```rust
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct TokenSet { white, blue, green, red, black, gold: u8 }  // 6 字段

pub struct Bank { tokens: TokenSet }   // 公共筹码池
```

`TokenSet` 含金。`get(GemColor) / set / add / remove(NonGold) / total()`。普通宝石操作用 `CardColor` (编译期排除金) 或运行时校验金色不参与拿取。

### 4.3 费用 (`card.rs`)

```rust
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct GemCost { white, blue, green, red, black: u8 }  // 5 字段，不含金

impl GemCost {
    fn after_discount(self, bonus: CardBonus) -> GemCost { /* max(need-bonus,0) */ }
}
```

`GemCost` 与 `TokenSet` 分开：费用不含金，杜绝"卡牌费用里出现金"的非法状态。

### 4.4 发展卡 / 等级 (`card.rs`)

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CardLevel { Level1, Level2, Level3 }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DevelopmentCard {
    pub id: CardId,
    pub level: CardLevel,
    pub color: CardColor,
    pub prestige: u8,
    pub cost: GemCost,
}
```

`CardId` = `u32`。真实 90 张牌数据由 `card.rs` 内的 `pub fn standard_deck()` 返回 `Vec<DevelopmentCard>` (按等级分三组返回 `CardDecks`，见 4.7)。

### 4.5 贵族 (`noble.rs`)

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Noble {
    pub id: NobleId,
    pub prestige: u8,           // 通常 3
    pub requirement: GemCost,   // 要求的是已购发展卡数量(按颜色)
}
pub fn standard_nobles() -> Vec<Noble>  // 标准贵族牌池
```

`requirement` 复用 `GemCost` (5 字段)，语义为"该颜色发展卡数量"。

### 4.6 玩家 (`player.rs`)

```rust
pub type PlayerId = usize;

#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct PlayerState {
    pub id: PlayerId,
    pub tokens: TokenSet,
    pub reserved_cards: Vec<CardId>,   // 最多 3
    pub purchased_cards: Vec<CardId>,
    pub nobles: Vec<NobleId>,
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct CardBonus { white, blue, green, red, black: u8 }  // 折扣=已购卡按色计数

impl PlayerState {
    fn bonus(&self, cards: &CardStore) -> CardBonus
    fn score(&self, cards: &CardStore, nobles: &NobleStore) -> u16
    fn token_total(&self) -> u8
}
```

### 4.7 牌堆 / 市场 (`market.rs`)

```rust
pub struct CardDecks { level1: Vec<DevelopmentCard>, level2: _, level3: _ }  // pop 从顶
pub struct Market { level1_visible: Vec<DevelopmentCard>, level2: _, level3: _ }  // 最多各4
pub struct NobleBoard { available: Vec<Noble>, taken: Vec<NobleId> }

impl Market {
    fn visible(&self, level: CardLevel) -> &[DevelopmentCard]
    fn take(&mut self, level: CardLevel, idx: usize) -> Option<DevelopmentCard>
    fn refill(&mut self, level: CardLevel, deck: &mut CardDecks) -> Option<CardId>
}
```

市场存**完整卡牌**而非仅 `CardId` (避免每次取卡都查 `CardStore`)；`CardStore` / `NobleStore` 仍保留作 `id→card` 查询的只读索引，供 `bonus` / `score` 使用。

### 4.8 全局状态 (`state.rs`)

```rust
pub struct GameState {
    pub players: Vec<PlayerState>,
    pub bank: Bank,
    pub decks: CardDecks,
    pub market: Market,
    pub nobles: NobleBoard,
    pub card_store: CardStore,    // id -> card 只读
    pub noble_store: NobleStore,
    pub current_player: usize,
    pub round_start_player: usize,
    pub end_triggered: bool,
    pub winner: Option<PlayerId>,
}
```

## 5. 行动 API 与回合流程

### 5.1 行动枚举 (`actions.rs`)

```rust
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PlayerAction {
    TakeThreeDifferentTokens(Vec<GemColor>),   // 恰好3个不同普通色，不含金
    TakeTwoSameTokens(GemColor),               // 普通色，该色公共区≥4
    ReserveVisibleCard { level: CardLevel, idx: usize },
    ReserveDeckCard(CardLevel),                // 盲抽牌堆顶
    BuyVisibleCard { level: CardLevel, idx: usize },
    BuyReservedCard(usize),                    // 玩家保留区索引
}
```

### 5.2 结果与续接 (`actions.rs`)

```rust
pub enum ActionOutcome {
    Complete,                                      // 行动完成，回合可直接结束
    NeedDiscardTokens { excess: u8 },              // 超过10，需归还 excess 个
    NeedChooseNoble { candidates: Vec<NobleId> },  // 同时满足多个贵族，需选一个
    NeedFinalDiscardThenChooseNoble { excess: u8, candidates: Vec<NobleId> },  // 两者叠加
}

pub struct ActionResult {
    pub outcome: ActionOutcome,
    pub events: Vec<GameEvent>,
}

pub enum Resume {
    DiscardTokens(TokenSet),          // 玩家决定归还的筹码
    ChooseNoble(NobleId),             // 玩家选择的贵族
}

pub fn apply_action(state: &mut GameState, player: PlayerId, action: PlayerAction)
    -> Result<ActionResult, RuleError>

pub fn resume(state: &mut GameState, player: PlayerId, resume: Resume)
    -> Result<ActionResult, RuleError>
```

### 5.3 回合主流程 (遵循 rules.md §25)

```
apply_action(action):
  validate(action)?                      // validation.rs
  events = execute(action)               // 改 state，产生事件
  if outcome.requires_choice():
      return ActionResult{ outcome, events }   // 挂起，等 resume
  events += check_nobles()               // 可能又产生 NeedChooseNoble
  if outcome.requires_choice(): return ...
  check_end_game()                       // 可能触发 end_triggered / winner
  advance_turn()
  return Complete

resume(resume):
  应用玩家选择(弃牌/选贵族) → 事件
  继续未完成的 check_nobles / check_end_game / advance_turn
  return Complete
```

### 5.4 关键执行规则 (落点到 validation/execute)

- **拿不同色**：通常拿 3 个互异普通色；公共区不足 3 种普通色时拿所有可用颜色；拿后若 `token_total > 10` → `NeedDiscardTokens{excess}`。
- **拿2相同**：普通色且公共区 ≥4。
- **保留可见卡**：`reserved.len() < 3`；移卡 → 补牌 (立即)；公共区有金则拿 1 金；可能触发弃牌。
- **盲抽保留**：`reserved.len() < 3`；`deck.pop()`；有金则拿 1 金。
- **买可见卡/保留卡**：`can_afford` (折扣后缺口 ≤ 金) → 优先用对应普通色支付、金补缺口 → 筹码回银行 → 加入已购 → 补牌 (仅可见卡) → `check_nobles` → `check_end_game`。
- **补牌**：购买/保留可见卡后**立即**补 (rules.md §5)，不同于旧实现的"回合末补"。
- **advance_turn**：`current_player = (current + 1) % n`；`end_triggered` 时继续到 `round_start_player` 前一位再结算胜负。

### 5.5 错误 (`error.rs`)

```rust
pub enum RuleError {
    NotYourTurn, TooManyReserved, BankInsufficient, TokenLimitExceeded,
    CardNotFound, CannotAfford, InvalidTokenSelection, NobleNotEligible,
    DeckEmpty, InvalidResume, GameOver, InvalidPlayerCount,
}
```

## 6. 校验、计分、终局

### 6.1 校验 (`validation.rs`)

纯函数式校验，不改 state，返回 `Result<(), RuleError>`：

```rust
pub fn validate_action(state: &GameState, player: PlayerId, action: &PlayerAction)
    -> Result<(), RuleError>
```

按 action 分支调用 `can_take_three_different` / `can_take_two_same` / `can_reserve` / `can_afford` 等 (rules.md §11/§12/§14/§16 伪代码直接落地)。校验不通过即返回对应 `RuleError`，state 不被修改。`can_afford` 用 `card.cost.after_discount(bonus)` + 金补缺口判定。

### 6.2 贵族拜访 (`noble.rs` / `scoring.rs`)

买牌后调用：

```rust
pub fn eligible_nobles(player: &PlayerState, board: &NobleBoard, store: &CardStore) -> Vec<NobleId>
// 取玩家 bonus，过滤 board.available 中 requirement 被 bonus 满足者
```

- 0 个 → 无事。
- 1 个 → 自动获得，移入 `player.nobles`，从 `NobleBoard.available` 移除，产 `NobleVisited` 事件。
- ≥2 个 → 返回 `NeedChooseNoble{candidates}`，`resume(ChooseNoble(id))` 后落实。

注意：贵族要求基于**已购发展卡数量** (`bonus`)，不是筹码。一个玩家回合内买多张牌也只在买牌后检查一次 (标准规则)。

### 6.3 计分与胜负 (`scoring.rs`)

```rust
pub fn calculate_score(player, cards: &CardStore, nobles: &NobleStore) -> u16
// card prestige 之和 + noble prestige 之和

pub fn compare_players(a: &PlayerState, b: &PlayerState, ...) -> Ordering
// 分数降序；分数相同则 purchased_cards.len() 升序(买牌少者胜)
```

### 6.4 终局 (`state.rs` / `actions.rs`)

- 买牌后检查：当前玩家分数 ≥ 15 → `end_triggered = true`，产 `EndGameTriggered` 事件，**不立即结束**。
- 之后继续 `advance_turn`，直到 `current_player` 走到 `round_start_player` 的**前一位**完成行动，再结算：
  - 对所有玩家按 `compare_players` 排序，`winner = 排名第一`，产 `GameOver` 事件。
- 终局轮约定：若 `end_triggered` 时正好是起始玩家本人达到 15 分 (即他就是本轮第一位行动者)，那么其余 `n-1` 人各行动一次后即结算。
- 一旦 `winner.is_some()` (终局轮已走完)，任何 `apply_action` / `resume` 返回 `Err(RuleError::GameOver)`。

### 6.5 事件 (`events.rs`)

```rust
pub enum GameEvent {
    TokensTaken { player, tokens: TokenSet },
    TokensReturned { player, tokens: TokenSet },      // 弃牌归还
    CardReserved { player, card: CardId, from_deck: bool, got_gold: bool },
    CardPurchased { player, card: CardId, paid: TokenSet },
    MarketRefilled { level: CardLevel, card: Option<CardId> },
    NobleVisited { player, noble: NobleId },
    EndGameTriggered { player: PlayerId },
    GameOver { winner: PlayerId, standings: Vec<(PlayerId, u16)> },
}
```

事件按发生顺序入 `ActionResult.events`，足够 UI diff 还原任何状态变化 (`paid` 字段记录实际支付的普通色+金，便于动画区分)。

## 7. 初始化

```rust
impl GameState {
    pub fn new(player_count: usize, rng: &mut impl Rng) -> Result<Self, RuleError>
}
```

`new` 内部：

1. 校验 `player_count ∈ 2..=4`，否则 `Err(InvalidPlayerCount)`。
2. `standard_deck()` 得 90 张，按等级分三组，各组 `rng.shuffle`。
3. 翻出每等级 4 张入 `Market`，余下入 `CardDecks`。
4. `standard_nobles()` 得贵族池，`rng.shuffle` 后取 `player_count + 1` 张入 `NobleBoard.available`。
5. 按 rules.md §9 设 `Bank`：2 人每种普通 4、3 人 5、4 人 7；金固定 5。
6. 玩家 `Vec` 长度 = `player_count`，`id = 0..n`，全部默认空。
7. `current_player = 0`，`round_start_player = 0`，`end_triggered = false`，`winner = None`。

`rand` 仅在此处用；之后 `GameState` 不持 `Rng`，给定初始状态则动作序列确定 (可回放/可测)。`main.rs` 加 `mod rules;` 纳入编译；`battle.rs` / `game.rs` 本阶段不动。

## 8. 测试策略 (TDD，每个模块配 `#[cfg(test)]`)

| 模块 | 关键测试 |
|---|---|
| `token.rs` | add/remove/total；金色不参与普通操作 |
| `card.rs` | `standard_deck()` 返回 40/30/20；`after_discount` 折扣与下限 |
| `noble.rs` | `standard_nobles()` 数量；`eligible_nobles` 命中/未命中 |
| `validation.rs` | 拿3不同 (含重复色/含金/不足)、拿2同 (<4)、保留上限、`can_afford` 含金补缺口 |
| `actions.rs` | 6 种行动各自 happy path；补牌立即触发；保留得金；买牌折扣+金支付+回银行；筹码上限触发弃牌；选贵族 0/1/多；终局触发与最终轮走完结算；平分比买牌数 |
| `scoring.rs` | 卡分+贵族分；平分时买牌少者胜 |
| `state.rs` | `new` 各人数筹码/贵族数正确；非法人数 |

测试用固定 `rng` (如 `rand::rngs::StdRng::seed_from_u64(...)`) 保证可重复。

## 9. 文件清单

```
src/rules/
  mod.rs          pub use 重导出
  color.rs        GemColor, CardColor
  token.rs        TokenSet, Bank
  card.rs         CardLevel, DevelopmentCard, GemCost, CardBonus, CardStore, standard_deck()
  noble.rs        Noble, NobleStore, NobleBoard, standard_nobles()
  player.rs       PlayerState, PlayerId
  market.rs       CardDecks, Market
  state.rs        GameState::new
  actions.rs      PlayerAction, ActionOutcome, ActionResult, Resume, apply_action, resume
  validation.rs   validate_action 及 can_* 谓词
  scoring.rs      calculate_score, compare_players, eligible_nobles
  events.rs       GameEvent
  error.rs        RuleError
src/main.rs       增加 mod rules;
```

`Cargo.toml` 加 `rand = "0.9"`。

## 10. 不在本阶段范围

- 修改 `src/game.rs` (旧简化实现保留)。
- 修改 `src/battle.rs` (UI 暂不适配新规则层)。
- Bevy System 接入、动画、输入。
- 存档/读档、网络多人、AI 对手。

这些留待后续阶段 (见后续实现 plan)。
