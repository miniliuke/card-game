# Battle / Game 适配规则层 — 设计文档

- 日期：2026-06-21
- 主题：将旧 demo 的 `battle.rs` / `game.rs` 改造为驱动 `src/rules/` 纯规则层的 Bevy UI
- 状态：已通过 brainstorming，待用户复核后进入 writing-plans

## 1. 背景与问题

`src/rules/` 已实现完整的《璀璨宝石》纯规则层：6 色（含金）、90 张标准发展卡、10 张贵族、按人数缩放的银行、`apply_action` / `resume` 双入口、`GameEvent` 事件流、15 分终局与最终轮结算。

但 `src/game.rs` + `src/battle.rs` 仍是早期 demo：

- `game.rs` 是简化引擎：5 色（无金）、3×4 市场、仅 `take_card` / `take_token` / `end_turn` 三个动作，无折扣、无保留、无贵族、无筹码上限、无终局判定。卡牌用 `name: String` / `bonus: GemColor` / `points` / `costs[5]` 程序化生成。
- `battle.rs` 的 UI 直接调用 `GameSession`，三个 `BattleAction` 分支硬编码。

规则层与 demo 在结构上不兼容（5 色 vs 6 色、无金 vs 含金、3 动作 vs 6 动作 + 2 挂起态），无法用薄包装适配。

### 范围决策（已与用户确认）

- **完整适配**：本次一次性接入全部 6 个动作 + 保留牌展示 + 贵族区 + 筹码上限弃牌 + 贵族选择 + 15 分终局 / 最终轮流程。
- **Token UX**：选择缓冲区 + 确认（点 3 色入缓冲，Confirm 提交 `TakeThreeDifferentTokens`；颜色按钮上的 `×2` 徽章直接提交 `TakeTwoSameTokens`）。
- **保留卡**：每个玩家面板的 Reserved 行显示该玩家保留卡（对手可见），仅 owner 面板带 Buy 按钮。
- **动画**：事件驱动，扩展现有飞牌 / 发牌动画，按 `GameEvent` 流播放。
- **玩家数布局**：2 人用现有 rich 左右面板；3/4 人用顶部紧凑卡片一排。

## 2. 架构方案：方案 A — 增量适配

### 2.1 方案对比（已选 A）

- **方案 A（选定）**：`rules::GameState` 入资源，`game.rs` 删除，`battle.rs` 重写指向 `rules::*`。`apply_action` / `resume` 产物入 `PendingEvents`，`play_events` 逐个播放动画，挂起态用 `BattlePhase` 枚举 + 覆盖层处理。
- 方案 B（否决）：新建 `play.rs` 屏幕，冻结旧 `battle.rs` / `game.rs`。重复有效 UI 代码，工作量更大。
- 方案 C（否决）：`game.rs` 作类型桥梁包装 `rules::*`。demo 类型与规则层结构差异过大，包装退化为并行副本，挂起态无处安放。

### 2.2 模块布局

- **`game.rs` → 删除**。`main.rs` 移除 `mod game;`。
- **`battle.rs` → 完全重写**，`use crate::rules::*`。复用有效工件：调色板常量（`INK` / `PANEL` / `GOLD` / `CREAM` / 等）、`gem_color` / `color_name` / `color_short` 辅助、`spawn_ambient_shapes`、焦点光标概念、响应式缩放。替换不兼容工件（旧 `BattleAction` 三分支、`apply_actions`、`game::LEVEL_COUNT` / `SLOTS_PER_LEVEL` 引用）。
- **`rules` → 不改动**。该层已纯净且满足需求。
- **`main.rs`**：`AppState` 保留 `Menu` / `Battle`；`NewRun` → `AppState::Battle` 不变。`OnEnter(AppState::Battle)` 从时间种子构建 `GameState`（替换旧 `GameSession::new`）。

### 2.3 核心桥梁资源

- `BattleModel(GameState)` — 规则状态。
- `PendingEvents(Vec<GameEvent>)` — `apply_action` / `resume` 产物队列，`play_events` 消费。
- `BattlePhase` — `Idle` / `AwaitDiscard { excess }` / `AwaitNobleChoice { candidates }` / `GameOver { winner, standings }`。非 `Idle` 路由到覆盖层系统。
- `PendingPhase` — 动画播完后才提交为 `BattlePhase`（见 §5）。

状态机：**`Idle → apply action → events play → (Idle | AwaitDiscard | AwaitNobleChoice | GameOver)`**，`resume` 回到 events play 再到 Idle 或下一步。

## 3. Action 模型与挂起态

### 3.1 `BattleAction`（与 `PlayerAction` 一一对应）

```rust
enum BattleAction {
    TakeThreeDifferentTokens(Vec<GemColor>),   // 选满 3 色后入队
    TakeTwoSameTokens(GemColor),
    ReserveVisibleCard { level: CardLevel, idx: usize },
    ReserveDeckCard(CardLevel),                 // 每级牌堆一个盲抽按钮
    BuyVisibleCard { level: CardLevel, idx: usize },
    BuyReservedCard(usize),                     // reserved_idx
}
```

一个 `BattleAction` 编码完整用户意图（含 token 选择），符合规则层"每回合一个主要行动"约束。

### 3.2 Token 选择缓冲区（`Res<TokenPicker>`）

- `selected: Vec<GemColor>`（最多 3）。
- **单点颜色按钮**：加入 `selected`（去重、≤3、银行≥1、非金）。
- **颜色按钮 `×2` 徽章**（仅 `bank.get(color) >= 4` 时显示并启用）：清空 `selected` 并入队 `TakeTwoSameTokens(color)`，一次性确认。
- **Confirm Take 3 按钮**：`selected.len() == 3` 时启用 → 入队 `TakeThreeDifferentTokens(selected.clone())` → 清缓冲。
- **Clear 按钮**：清缓冲。
- 选中态：被选颜色按钮金边高亮 + 计数徽章 `1/3`。

### 3.3 `BattlePhase` 与覆盖层

```rust
enum BattlePhase {
    Idle,
    AwaitDiscard { excess: u8 },
    AwaitNobleChoice { candidates: Vec<NobleId> },
    GameOver { winner: PlayerId, standings: Vec<(PlayerId, u16)> },
}
```

- **`AwaitDiscard { excess }`**：覆盖层"归还 N 个筹码"。玩家点击自己持有的筹码归还（逐色累加 `return_buffer`，`total == excess` 时 Confirm 启用）→ `resume(DiscardTokens(return_buffer))`。仅当前玩家面板启用归还交互。
- **`AwaitNobleChoice { candidates }`**：覆盖层列出候选贵族（requirement + prestige），点击 → `resume(ChooseNoble(id))`。
- **`GameOver`**：结果覆盖层（排名 + 胜者），"返回菜单" → `AppState::Menu`。

### 3.4 Action → outcome 转换（`apply_actions` 系统）

1. `BattleAction` → `PlayerAction`（直译）。
2. `apply_action(&mut state, current_player, player_action)` → `ActionResult { outcome, events }`。
3. `events` 追加到 `PendingEvents`。
4. 按 `outcome` 设 `PendingPhase`：
   - `Complete` → `PendingPhase = None`（保持 Idle）。
   - `NeedDiscardTokens { excess }` → `PendingPhase = AwaitDiscard { excess }`。
   - `NeedChooseNoble { candidates }` → `PendingPhase = AwaitNobleChoice { candidates }`。
   - `NeedFinalDiscardThenChooseNoble { excess, candidates }` → `PendingPhase = AwaitDiscard { excess }` + `pending_noble_candidates = Some(candidates)`。
5. `events` 含 `GameOver` → `PendingPhase = GameOver { winner, standings }`。

### 3.5 输入门控

`mouse_actions` / `keyboard_actions` 仅在 `BattlePhase::Idle && !animations.busy() && PendingEvents.is_empty()` 时写 `ActionQueue`。覆盖层系统在对应非 `Idle` 阶段运行。保证玩家不会在弃牌 / 选贵族中途触发新行动。

## 4. UI 布局重构

复用现有视觉骨架（调色板、ambient shapes、top bar、player panel、market row、token supply、footer、focus cursor、responsive scale），改动如下。

### 4.1 Top bar

`"ROUND 01 / PLAYER 1 TURN"` → `"TURN {n}  /  PLAYER {current+1}"`（规则层无 round / turn 计数字段，`n` 由 `battle.rs` 侧维护一个本地 `turn_count: u32`，每次 `apply_action` / `resume` 成功推进回合后 +1）。`end_triggered` 时追加显示 `"FINAL ROUND"` 标记。

### 4.2 Player panel（2 人 rich）

- Header：`PLAYER {n}` + `{score} PTS`（`score = player.score(card_store, noble_store)`）。
- 状态行：`ACTIVE` / `WAITING`；`end_triggered && id == final_player` 显示 `FINAL`。
- 5 色行（不含金）：`C {bonus}  /  T {tokens}`，`bonus = player.bonus(store).get(color)`，`tokens = player.token_count(color.to_gem())`。
- 新增 **金色行**：`GOLD  /  T {gold}`。
- 新增 **Reserved 行**（最多 3 槽）：显示该玩家保留卡（缩小卡面，含 cost + prestige），owner 面板每张带 Buy 按钮 → `BuyReservedCard(idx)`。对手面板同样显示，无 Buy 按钮。
- 新增 **Nobles 行**：显示该玩家已获贵族（小型徽章 + prestige）。空槽虚线占位。

### 4.3 Market（中央）

- 行顺序：Level3（顶）→ Level2 → Level1（底），与现有 `(0..LEVEL_COUNT).rev()` 一致。
- 每槽复用 `spawn_card_button`：`T{level}` / `{prestige} PTS`（0 分灰显或隐藏）/ 5 色 cost dots（`card.cost.get(CardColor::ALL[i])`，0 灰显）。颜色用 `gem_color(color.to_gem())`。点击 → `BuyVisibleCard { level, idx }`。
- 每行左侧 deck 计数块：`TIER {n}` / `{remaining:02}` / `DECK`，**整块改为按钮** → `ReserveDeckCard(level)`（`remaining == 0` 或 `player.reserved_full()` 时 disabled）。
- 市场卡右下角 **"R" 保留按钮**（叠加卡面）→ `ReserveVisibleCard { level, idx }`（`reserved_full()` 时 disabled）。

### 4.4 Token supply（市场下方）

- 5 个普通色按钮 + 选中态。每按钮 `×{bank.get(color)}`；`bank.get(color) >= 4` 时显示可点 `×2` 徽章。
- 选中缓冲区 HUD：`{selected.len()}/3` + Confirm + Clear（`selected` 非空时显示）。
- 金色：不可直接拿，显示 `×{bank.gold}` 作信息（无按钮）。

### 4.5 Footer

- 操作提示更新为：`ARROWS MOVE / ENTER BUY / R RESERVE / T TAKE TOKEN / CONFIRM 3 / ESC MENU`。终局无 `E`（规则层回合自动推进，无 EndTurn）。
- StatusText：反馈最近事件（"Player 1 bought X"、"Need to discard 2 tokens"、"Choose a noble" 等）。

### 4.6 覆盖层（非 Idle 阶段）

半透明全屏遮罩 + 居中面板，UI 树顶层（footer 之后 spawn）。

- **Discard overlay**：标题"Discard {excess} tokens"，当前玩家筹码列表（每色 `×N` 可点减），running `return_buffer` 总数，Confirm（`total == excess` 启用）。
- **Noble overlay**：标题"Choose a noble"，候选贵族卡片（prestige + requirement 5 色 dots），点击选择。
- **GameOver overlay**：标题"GAME OVER"，排名列表 `(1. PLAYER n — {score} pts)`，"BACK TO MENU" 按钮。

### 4.7 玩家数布局

- **2 人**：现有左 / 右玩家面板 + 中央市场（现状）。
- **3/4 人**：顶部一排紧凑玩家卡片（每个 ~30% 宽，score + token 简表 + reserved/noble 计数徽章；完整 reserved 列表仅在 active 玩家卡片展开显示）。中央市场居中，底部 token supply + footer。当前玩家卡片金边高亮。

## 5. 事件 → 动画流水线

### 5.1 队列与节流

- `PendingEvents(Vec<GameEvent>)`：`apply_actions` 一次性 push 整个 `ActionResult.events`。
- `AnimationCounts { flying, dealing, ... }`：沿用"忙则暂停输入"语义。新 `BattleAction` 仅在 `PendingEvents` 空 **且** `!animations.busy()` 时被 `apply_actions` 消费。
- `resume`（弃牌 / 选贵族）不等动画——玩家主动覆盖层交互，规则层已设计为即时推进。

### 5.2 `play_events` 系统（新，替换旧 `animate_deals` 中夹带的回合文本更新）

每帧从 `PendingEvents` 头部取 **一个** 事件，按类型 spawn 动画实体并推进 `AnimationCounts`：

- `TokensTaken { player, tokens }`：每个被拿颜色 spawn 金币 fly 动画，目标 = 对应玩家面板。`flying += colors`。
- `TokensReturned { player, tokens }`：反向，金币从玩家飞回 supply。
- `CardReserved { player, card, from_deck, got_gold }`：卡牌（市场槽或牌堆图标）飞向玩家 reserved 行；`got_gold` 时额外金币飞向玩家。注意规则层对可见卡保留的实际事件顺序是先 `MarketRefilled` 再 `CardReserved`（见 `actions.rs::execute`），`play_events` 按该顺序逐个播放即可：先补牌动画、再卡牌飞向 reserved。
- `CardPurchased { player, card, paid }`：卡牌飞向玩家 purchased 区；`paid` 各色筹码从玩家飞回 supply。
- `MarketRefilled { level, card }`：空槽 spawn 新卡 + `DealAnimation`（沿用现有发牌动画）。`card == None` → 显示空占位。
- `NobleVisited { player, noble }`：贵族徽章从公共贵族区飞向玩家 nobles 行。
- `EndGameTriggered { player }`：status 文本置"Final round!"，可选金色脉冲（轻量）。
- `GameOver { winner, standings }`：设 `PendingPhase = GameOver`，触发覆盖层 spawn（动画播完后）。

事件按顺序逐个播放（不并发），保证视觉因果链清晰。

### 5.3 Phase 切换时机

- `apply_actions` 调用 `apply_action` / `resume` 后，**先记录 `PendingPhase`**，**不立即切 `BattlePhase`**。
- `commit_pending_phase` 系统：当 `PendingEvents.is_empty() && !animations.busy() && PendingPhase.is_some()` → 把 `PendingPhase` 移入 `BattlePhase`，spawn 对应覆盖层，清 `PendingPhase`。保证覆盖层在所有飞牌 / 发牌动画结束后才出现。
- `Idle` 期间 `apply_actions` 正常消费 `ActionQueue`。

### 5.4 UI 刷新策略

- 沿用 `UiDirty` + `refresh_battle_ui` 全量刷新文本 / 边框模式。触发点改为 **每个事件播放完成后** 置 `UiDirty = true`（而非旧"行动后立即 dirty"）。分数 / 筹码 / 库存数字与动画同步。
- 市场槽增删（`MarketRefilled` / 卡被买走 / 保留）由 `play_events` 直接 `commands.spawn` / `despawn` 卡片实体，事件驱动。

### 5.5 事件可视化映射表

| GameEvent | 动画 | AnimationCounts | UI 刷新 |
|---|---|---|---|
| TokensTaken | N 个金币飞向玩家 | flying += N | dirty |
| TokensReturned | 金币飞回 supply | flying += N | dirty |
| CardReserved | 卡飞向 reserved 行 + (可选)金币 | flying += 1/2 | dirty |
| CardPurchased | 卡飞向 purchased + 筹码飞回 | flying += 1 + paid 数 | dirty |
| MarketRefilled | 空槽 spawn 新卡 + DealAnimation | dealing += 1 | dirty |
| NobleVisited | 贵族徽章飞向玩家 | flying += 1 | dirty |
| EndGameTriggered | status 文本脉冲 | — | dirty |
| GameOver | 设 PendingPhase + spawn 覆盖层 | — | dirty |

## 6. 输入系统与系统调度

### 6.1 键盘焦点模型（区域 + 区域内索引）

```rust
enum FocusZone {
    Market { level: CardLevel, slot: usize },
    DeckReserve { level: CardLevel },
    Supply { color: GemColor },
    SupplyX2 { color: GemColor },
    ConfirmTake3,
    ClearSelection,
    Reserved { player: PlayerId, idx: usize },
    ReserveMarket { level: CardLevel, slot: usize },
}
struct FocusCursor { zone: FocusZone }
```

- 方向键在区域内移动（市场内 3×4 网格上下左右），`Tab` 在区域间跳转，`Enter` 激活当前 zone 对应 `BattleAction`。
- 覆盖层激活时焦点切到覆盖层（Discard overlay 筹码行 / Noble overlay 候选卡 / GameOver 返回按钮），`Enter` 确认。

### 6.2 鼠标

沿用旧 `mouse_actions`：`Interaction::Pressed` 按钮按 `BattleAction` / 覆盖层标记入队或触发 `resume`。鼠标不走 `FocusCursor`，但点击同步更新 cursor。

### 6.3 系统调度（替换旧 `BattlePlugin::build` 的 `.chain()`）

```
Update (in_state Battle):
  mouse_actions.run_if(can_act)
  keyboard_actions.run_if(can_act)
  discard_overlay_input.run_if(in_phase(AwaitDiscard))
  noble_overlay_input.run_if(in_phase(AwaitNobleChoice))
  gameover_overlay_input.run_if(in_phase(GameOver))
  apply_actions                 // 消费 ActionQueue → PendingEvents + PendingPhase
  play_events                   // 逐个消费 PendingEvents → spawn 动画/设 PendingPhase
  animate_flights
  animate_deals
  commit_pending_phase          // 动画空闲且事件空 → PendingPhase 提交为 BattlePhase + spawn 覆盖层
  refresh_battle_ui             // UiDirty 时刷新
  update_focus_visuals
  button_hover_effects
  responsive_battle_layout
```

`can_act = BattlePhase::Idle && !animations.busy() && PendingEvents.is_empty()`，作为 `apply_actions` / `mouse` / `keyboard` 的 `run_if`。

### 6.4 `apply_actions` 重写（统一流程）

1. 若 `PendingPhase.is_some()` 或 `animations.busy()` 或 `PendingEvents` 非空 → return（防重入）。
2. 取 `ActionQueue.drain()`。
3. 对每个 `BattleAction`：
   a. 映射为 `PlayerAction`。
   b. `apply_action(state, current, action)` → match `Ok` / `Err`。
   c. `Ok`：events → `PendingEvents`，outcome → `PendingPhase`。
   d. `Err(RuleError)`：status 文本提示（"Cannot afford"、"Bank insufficient" 等），不推进。
4. `resume` 路径由 `overlay_input` 直接触发，不经 `ActionQueue`。

### 6.5 `NeedFinalDiscardThenChooseNoble` 两段式

`apply_actions` 拆为 `PendingPhase = AwaitDiscard { excess }` + `pending_noble_candidates = Some(candidates)`。`discard_overlay_input` Confirm 后 `resume` 成功 → 检测 `pending_noble_candidates.is_some()` → 设 `PendingPhase = AwaitNobleChoice { candidates }`（而非回 Idle）。

## 7. 文件结构、测试与边界情况

### 7.1 `battle.rs` 内部分区

用 `// === SECTION ===` 注释分隔（沿用现有风格）：

- 常量与调色板（复用）
- 资源：`BattleModel` / `ActionQueue` / `FocusCursor` / `AnimationCounts` / `UiDirty`，**新增** `PendingEvents` / `BattlePhase` / `PendingPhase` / `TokenPicker` / `DiscardBuffer` / `PendingNobleCandidates`
- 组件：`BattleScreen` / `BattleRoot` / `Focusable` / `CardButton` / `CardSlot` / `PlayerPanel` / 各文本组件，**新增** `ReservedCardButton` / `ReserveMarketButton` / `DeckReserveButton` / `SupplyButton` / `SupplyX2Button` / `ConfirmTake3Button` / `ClearSelectionButton` / `NobleBadge` / `Overlay*` / `FocusableZone`
- `BattleAction` enum（对齐 `PlayerAction`）
- `setup_battle`：时间种子 `GameState::new_seeded(player_count, seed)`，`player_count` 先固定 2
- spawn 函数：复用并扩展 `spawn_player_panel` / `spawn_market` / `spawn_market_row` / `spawn_card_button` / `spawn_token_supply` / `spawn_footer`，新增 `spawn_reserved_row` / `spawn_nobles_row` / `spawn_noble_board` / `spawn_*_overlay`
- 输入系统：`mouse_actions` / `keyboard_actions` / `discard_overlay_input` / `noble_overlay_input` / `gameover_overlay_input`
- 核心：`apply_actions` / `play_events` / `commit_pending_phase`
- 动画：`animate_flights` / `animate_deals`（复用，扩展事件类型）
- 刷新：`refresh_battle_ui` / `update_focus_visuals` / `button_hover_effects` / `responsive_battle_layout`
- 辅助：`gem_color` / `color_name` / `color_short`（`GemColor` → `Color`，需处理 6 色含金）
- `#[cfg(test)] mod tests`

### 7.2 3/4 人局触发

菜单 `NewRun` 当前直接进 2 人。先不做菜单选人数；`setup_battle` 内常量 `const PLAYER_COUNT: usize = 2;` 控制。3/4 布局代码在 `spawn_player_panels` 按 `player_count` 分支（2 → 左右 rich；3/4 → 顶部 compact 一排），保证规则层已支持的 3/4 能跑通，入口暂留 2。

### 7.3 测试策略（`battle.rs` 内 `#[cfg(test)]`）

**纯逻辑单测**（不启 Bevy）：

- `BattleAction → PlayerAction` 映射正确性。
- `can_act` 门控条件（phase / animation / events 组合）。
- `PendingPhase` 提交时机（事件空 + 不忙才提交）。
- `NeedFinalDiscardThenChooseNoble` 两段式状态机转换。
- `gem_color` 6 色映射含金。

**不写 Bevy UI 集成测试**（Bevy `App` 头部测试易脆、CI 慢；规则层已有 `smoke_full_game` 覆盖完整对局）。现有 `card_slot_size_does_not_depend_on_its_contents` 测试保留（`card_slot_node` 仍存在）。

### 7.4 边界情况清单

1. **牌堆空**：`MarketRefilled { card: None }` → 槽位显示空占位（复用 `spawn_empty_slot`），不 spawn 新卡。
2. **银行某色 0**：supply 按钮禁用（灰显 + 不可点）；`×2` 徽章隐藏。
3. **reserved 满 3**：所有 reserve 入口（市场 R 按钮、deck 盲抽按钮）禁用。
4. **买不起**：`RuleError::CannotAfford` → status 提示，不弹覆盖层，phase 保持 Idle，玩家可重选。
5. **拿筹码触发弃牌**：`NeedDiscardTokens` → 动画播完后弹 discard overlay；弃牌 `resume` 后若 `pending_noble_candidates` 有值则继续弹 noble overlay。
6. **终局轮**：`end_triggered = true` 后继续行动直到 `current_player == final_player`；期间 top bar 显示 "FINAL ROUND"；`GameOver` 事件到达后弹结果覆盖层。
7. **2 人局 final round**：玩家 0 达 15 → 触发，玩家 1 行动一次后结算（规则层已处理，UI 仅需正确显示 phase 切换）。
8. **玩家无法行动**：规则层 `smoke` 测试已证明总有合法行动（拿筹码 / 保留）；UI 不需要"pass"。若银行某色全空且无法凑 3 不同色，玩家仍可保留卡（盲抽或可见卡），不会卡死。
9. **重叠动画**：`play_events` 逐个消费 + `AnimationCounts.busy()` 门控，避免飞牌与发牌视觉冲突。
10. **覆盖层激活时 ESC**：discard / noble overlay 下 ESC → 返回菜单（放弃当前对局），与 Idle 一致；不提供"取消弃牌"（规则要求必须弃）。

### 7.5 风险与取舍

- `battle.rs` 行数增长（预计 ~1400-1600 行，与现有 ~1338 行相当）：可接受（原型阶段，单文件便于改动）；若实现中超过 ~1600 行，再拆 `battle/ui.rs` / `battle/input.rs` / `battle/animate.rs`。
- 键盘焦点 zone 模型比旧单索引复杂，但可维护性更好；实现时优先保证鼠标可用，键盘作为增量。
- 3/4 人 compact 卡片信息密度有限，但保证可玩；rich 化留作后续。

### 7.6 不在本次范围

- 菜单选玩家数 UI（常量 2 人先跑通）。
- 音效、卡牌美术资源。
- 网络多人 / 热座切换提示（当前即热座，轮流操作同一屏幕）。
- 保留卡对手隐藏（已选可见方案）。
- 规则层任何改动。

## 8. 实现路径概览（供 writing-plans 展开）

1. 删除 `game.rs`，`main.rs` 移除 `mod game;`。
2. `battle.rs` 资源与组件骨架：`BattleModel(GameState)` / `PendingEvents` / `BattlePhase` / `PendingPhase` / `TokenPicker` / `DiscardBuffer` / `PendingNobleCandidates`；新 `BattleAction`。
3. `setup_battle` 改用 `GameState::new_seeded`。
4. spawn 函数扩展：reserved 行、nobles 行、noble board、deck 盲抽按钮、市场 R 按钮、token supply ×2 徽章 + Confirm/Clear、三个覆盖层。
5. `apply_actions` 重写为统一 `apply_action` 调用 + outcome → PendingPhase。
6. `play_events` + `commit_pending_phase` 新系统。
7. `animate_flights` / `animate_deals` 扩展事件类型。
8. `refresh_battle_ui` 扩展（score / bonus / tokens / gold / reserved / nobles / final round）。
9. 焦点 zone 模型 + 覆盖层输入系统。
10. 3/4 人 compact 布局分支。
11. 纯逻辑单测。
12. 手动跑通完整对局（买牌 / 拿筹码 / 保留 / 弃牌 / 选贵族 / 终局）。
