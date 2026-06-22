下面按**后续让 AI 生成 Bevy 规则代码**的角度，描述《璀璨宝石 / Splendor》的卡牌库结构、公共区、玩家资产、拿币与买牌规则。重点是把规则拆成可编码的数据模型和判定逻辑。

---

# 1. 游戏核心对象

《璀璨宝石》的核心资源有三类：

1. **发展卡牌**
2. **宝石筹码**
3. **贵族牌**

其中真正“买卖”的对象主要是**发展卡牌**。买牌后，牌会永久留在玩家面前，提供：

* 颜色折扣
* 可能的胜利分数

---

# 2. 宝石颜色

游戏中一共有 6 种筹码颜色：

```rust
enum GemColor {
    White,  // 白色，钻石
    Blue,   // 蓝色，蓝宝石
    Green,  // 绿色，祖母绿
    Red,    // 红色，红宝石
    Black,  // 黑色，缟玛瑙
    Gold,   // 金色，万能宝石
}
```

其中前 5 种是普通宝石，金色是万能宝石。

发展卡牌只属于前 5 种颜色之一：

```rust
enum CardColor {
    White,
    Blue,
    Green,
    Red,
    Black,
}
```

金色不作为卡牌颜色，只作为万能支付资源。

---

# 3. 发展卡牌库

发展卡牌分为 3 个等级：

```rust
enum CardLevel {
    Level1,
    Level2,
    Level3,
}
```

每个等级有一个独立牌堆。

常见实体结构可以这样设计：

```rust
struct DevelopmentCard {
    id: CardId,
    level: CardLevel,
    color: CardColor,
    prestige: u8,
    cost: GemCost,
}
```

其中 `prestige` 是胜利分数，`cost` 是购买费用。

费用结构：

```rust
struct GemCost {
    white: u8,
    blue: u8,
    green: u8,
    red: u8,
    black: u8,
}
```

注意：发展卡的费用不包含金色。金色只在支付时作为万能资源补足缺口。

---

# 4. 卡牌库数量

标准《璀璨宝石》发展卡共有 90 张：

| 等级  |   数量 |
| --- | ---: |
| 一级牌 | 40 张 |
| 二级牌 | 30 张 |
| 三级牌 | 20 张 |

每个等级单独洗牌，形成三个牌堆：

```rust
struct CardDecks {
    level1: Vec<DevelopmentCard>,
    level2: Vec<DevelopmentCard>,
    level3: Vec<DevelopmentCard>,
}
```

公共区每个等级展示 4 张牌：

```rust
struct Market {
    level1_visible: Vec<CardId>, // 最多 4 张
    level2_visible: Vec<CardId>,
    level3_visible: Vec<CardId>,
}
```

初始化时：

1. 洗三组牌堆；
2. 每个等级从牌堆顶部翻出 4 张；
3. 放入公共市场。

---

# 5. 公共卡牌区补牌规则

当玩家购买或保留一张公共区的发展卡后：

1. 该卡从公共区移除；
2. 如果对应等级牌堆还有牌；
3. 立即从对应牌堆翻一张补到公共区；
4. 如果牌堆为空，则该位置空着。

伪代码：

```rust
fn refill_market(level: CardLevel, game: &mut GameState) {
    let visible = game.market.visible_cards_mut(level);
    let deck = game.decks.deck_mut(level);

    while visible.len() < 4 && !deck.is_empty() {
        let card = deck.pop().unwrap();
        visible.push(card.id);
    }
}
```

---

# 6. 贵族牌库

贵族牌不是购买的，而是自动拜访玩家。

贵族牌通常由以下结构表示：

```rust
struct Noble {
    id: NobleId,
    prestige: u8,      // 通常是 3 分
    requirement: GemCost,
}
```

贵族牌要求的是玩家已购买的发展卡数量，而不是筹码数量。

例如：

```text
需要：
白色发展卡 3 张
蓝色发展卡 3 张
绿色发展卡 3 张
```

玩家满足后，贵族自动加入玩家区域，提供分数。

---

# 7. 贵族数量

游戏开始时，根据玩家人数摆放贵族：

| 玩家数 | 贵族数量 |
| --- | ---: |
| 2 人 |  3 张 |
| 3 人 |  4 张 |
| 4 人 |  5 张 |

也就是：

```rust
noble_count = player_count + 1
```

---

# 8. 玩家状态

每个玩家至少需要记录：

```rust
struct PlayerState {
    id: PlayerId,

    tokens: TokenSet,              // 当前持有筹码
    reserved_cards: Vec<CardId>,   // 保留牌，最多 3 张
    purchased_cards: Vec<CardId>,  // 已购买发展卡
    nobles: Vec<NobleId>,          // 已获得贵族
}
```

筹码结构：

```rust
struct TokenSet {
    white: u8,
    blue: u8,
    green: u8,
    red: u8,
    black: u8,
    gold: u8,
}
```

玩家的折扣来自已购买的发展卡数量：

```rust
struct CardBonus {
    white: u8,
    blue: u8,
    green: u8,
    red: u8,
    black: u8,
}
```

计算方式：

```rust
fn calculate_bonus(player: &PlayerState, cards: &CardStore) -> CardBonus {
    let mut bonus = CardBonus::default();

    for card_id in &player.purchased_cards {
        let card = cards.get(*card_id);
        match card.color {
            CardColor::White => bonus.white += 1,
            CardColor::Blue => bonus.blue += 1,
            CardColor::Green => bonus.green += 1,
            CardColor::Red => bonus.red += 1,
            CardColor::Black => bonus.black += 1,
        }
    }

    bonus
}
```

---

# 9. 筹码总量

标准规则中，普通筹码数量取决于玩家人数：

| 玩家数 | 每种普通宝石数量 | 金币数量 |
| --- | -------: | ---: |
| 2 人 |   每种 4 个 |  5 个 |
| 3 人 |   每种 5 个 |  5 个 |
| 4 人 |   每种 7 个 |  5 个 |

公共区筹码池：

```rust
struct Bank {
    tokens: TokenSet,
}
```

---

# 10. 每回合可选行动

玩家每回合只能执行一个主要行动：

```rust
enum PlayerAction {
    TakeThreeDifferentTokens(Vec<GemColor>),
    TakeTwoSameTokens(GemColor),
    ReserveVisibleCard(CardId),
    ReserveDeckCard(CardLevel),
    BuyVisibleCard(CardId),
    BuyReservedCard(CardId),
}
```

注意：金色筹码不能直接拿，只有保留卡牌时才可能获得。

---

# 11. 拿不同颜色宝石

玩家通常从公共筹码区拿 3 个不同颜色的普通宝石。如果公共区只剩 1 或 2 种普通颜色有筹码，则拿走所有仍可用的颜色，每种 1 个。

条件：

1. 只能拿普通颜色；
2. 不能拿金色；
3. 所选颜色必须不同；
4. 每种颜色公共区至少有 1 个；
5. 有至少 3 种颜色可选时必须拿 3 种；不足 3 种时必须选择所有可用颜色；
6. 拿完后，玩家筹码总数不能长期超过 10 个。

示例：

```text
拿：白、蓝、绿
```

合法。

```text
拿：白、白、蓝
```

非法，因为不是 3 种不同颜色。

伪代码：

```rust
fn can_take_three_different(bank: &Bank, colors: &[GemColor]) -> bool {
    let required = available_normal_color_count(bank).min(3);
    if required == 0 || colors.len() != required {
        return false;
    }

    if colors.contains(&GemColor::Gold) {
        return false;
    }

    if !all_different(colors) {
        return false;
    }

    colors.iter().all(|color| bank.tokens.get(*color) >= 1)
}
```

---

# 12. 拿 2 个相同颜色宝石

玩家可以从公共区拿 2 个相同颜色的普通宝石。

条件：

1. 只能拿普通颜色；
2. 不能拿金色；
3. 该颜色公共区必须至少有 4 个；
4. 玩家拿走 2 个。

例如：

```text
公共区红宝石有 4 个，可以拿 2 红。
公共区红宝石有 3 个，不可以拿 2 红。
```

伪代码：

```rust
fn can_take_two_same(bank: &Bank, color: GemColor) -> bool {
    if color == GemColor::Gold {
        return false;
    }

    bank.tokens.get(color) >= 4
}
```

---

# 13. 玩家筹码上限

玩家回合结束时，最多只能持有 10 个筹码。

包括：

* 普通宝石
* 金色宝石

如果超过 10 个，需要归还任意筹码到公共区，直到剩 10 个。

可以设计成：

```rust
enum ActionResult {
    Complete,
    NeedDiscardTokens { excess: u8 },
}
```

例如：

```text
玩家原本有 8 个筹码，拿 3 个后变成 11 个。
必须归还 1 个筹码。
```

---

# 14. 保留卡牌规则

玩家可以保留一张发展卡。保留卡分两种：

1. 保留公共区可见卡；
2. 从某个等级牌堆盲抽一张保留。

保留后，这张卡进入玩家的 `reserved_cards`。

限制：

1. 每个玩家最多保留 3 张牌；
2. 保留牌不会立刻提供颜色折扣；
3. 保留牌不会立刻提供分数；
4. 保留时如果公共区还有金色筹码，玩家拿 1 个金色；
5. 如果没有金色，也可以保留，但拿不到金币；
6. 保留公共卡后要补牌；
7. 保留牌堆顶牌时，不展示给其他玩家。

伪代码：

```rust
fn can_reserve(player: &PlayerState) -> bool {
    player.reserved_cards.len() < 3
}
```

保留公共卡：

```rust
fn reserve_visible_card(
    game: &mut GameState,
    player_id: PlayerId,
    card_id: CardId,
) -> ActionResult {
    let player = game.player_mut(player_id);

    assert!(player.reserved_cards.len() < 3);

    remove_from_market(card_id);
    player.reserved_cards.push(card_id);

    if game.bank.tokens.gold > 0 {
        game.bank.tokens.gold -= 1;
        player.tokens.gold += 1;
    }

    refill_market(card.level, game);

    check_token_limit(player)
}
```

保留牌堆顶牌：

```rust
fn reserve_deck_card(
    game: &mut GameState,
    player_id: PlayerId,
    level: CardLevel,
) -> ActionResult {
    let player = game.player_mut(player_id);

    assert!(player.reserved_cards.len() < 3);

    let card = game.decks.deck_mut(level).pop().unwrap();
    player.reserved_cards.push(card.id);

    if game.bank.tokens.gold > 0 {
        game.bank.tokens.gold -= 1;
        player.tokens.gold += 1;
    }

    check_token_limit(player)
}
```

---

# 15. 买牌规则

玩家可以买：

1. 公共区可见发展卡；
2. 自己保留的发展卡。

买牌时要支付费用。支付时先计算折扣：

```text
实际费用 = max(卡牌费用 - 玩家对应颜色发展卡数量, 0)
```

例如：

```text
卡牌费用：
白 3，蓝 2，绿 0，红 0，黑 0

玩家已有发展卡：
白 1，蓝 3

实际需要：
白 max(3 - 1, 0) = 2
蓝 max(2 - 3, 0) = 0
```

金色宝石可以补任意颜色的缺口。

---

# 16. 是否买得起

判断玩家是否买得起一张牌：

```rust
fn can_afford(
    player: &PlayerState,
    card: &DevelopmentCard,
    cards: &CardStore,
) -> bool {
    let bonus = calculate_bonus(player, cards);
    let required = card.cost.after_discount(bonus);

    let mut missing = 0;

    for color in NORMAL_COLORS {
        let need = required.get(color);
        let have = player.tokens.get(color);

        if have < need {
            missing += need - have;
        }
    }

    player.tokens.gold >= missing
}
```

---

# 17. 支付规则

支付时应该优先使用对应颜色普通宝石，再用金色补缺口。

例如：

```text
实际费用：
白 3，蓝 2

玩家持有：
白 2，蓝 2，金 1

支付：
白 2
蓝 2
金 1 补白色缺口
```

支付后的筹码全部回到公共区。

伪代码：

```rust
fn pay_for_card(
    player: &mut PlayerState,
    bank: &mut Bank,
    card: &DevelopmentCard,
    bonus: CardBonus,
) {
    let required = card.cost.after_discount(bonus);

    for color in NORMAL_COLORS {
        let need = required.get(color);
        let have = player.tokens.get(color);

        let pay_normal = have.min(need);

        player.tokens.remove(color, pay_normal);
        bank.tokens.add(color, pay_normal);

        let remaining = need - pay_normal;

        if remaining > 0 {
            player.tokens.gold -= remaining;
            bank.tokens.gold += remaining;
        }
    }
}
```

---

# 18. 买牌后的效果

买牌成功后：

1. 卡牌从公共区或保留区移除；
2. 玩家支付筹码；
3. 卡牌加入玩家已购买区；
4. 卡牌立即提供对应颜色折扣；
5. 卡牌分数立即计入玩家总分；
6. 如果是公共区卡牌，要补一张新牌；
7. 检查是否获得贵族；
8. 检查是否触发游戏结束。

玩家总分：

```rust
fn calculate_score(player: &PlayerState, cards: &CardStore, nobles: &NobleStore) -> u8 {
    let card_score: u8 = player.purchased_cards
        .iter()
        .map(|id| cards.get(*id).prestige)
        .sum();

    let noble_score: u8 = player.nobles
        .iter()
        .map(|id| nobles.get(*id).prestige)
        .sum();

    card_score + noble_score
}
```

---

# 19. 贵族拜访规则

每次玩家买完发展卡后，需要检查是否满足贵族条件。

条件基于玩家已购买的发展卡数量，而不是筹码数量。

例如贵族要求：

```text
白 4，蓝 4
```

玩家已买：

```text
白色发展卡 4 张
蓝色发展卡 4 张
```

则满足。

如果玩家同时满足多个贵族，标准规则中该玩家本回合只能获得其中一个贵族，通常由玩家选择。

可编码为：

```rust
fn eligible_nobles(player: &PlayerState, nobles: &[Noble], cards: &CardStore) -> Vec<NobleId> {
    let bonus = calculate_bonus(player, cards);

    nobles.iter()
        .filter(|noble| bonus.satisfies(noble.requirement))
        .map(|noble| noble.id)
        .collect()
}
```

然后：

```rust
enum NobleResult {
    None,
    GainOne(NobleId),
    NeedChoose(Vec<NobleId>),
}
```

---

# 20. 游戏结束规则

当任意玩家达到 15 分时，触发游戏结束。

但不是立即结束，而是继续到当前轮结束，让所有玩家回合数相同。

例如 4 人游戏：

```text
玩家 A 达到 15 分。
如果 A 是本轮第 2 个行动的玩家，那么玩家 C、D 还要各行动一次。
然后结算胜负。
```

可以记录：

```rust
struct GameState {
    players: Vec<PlayerState>,
    current_player_index: usize,
    first_player_index: usize,
    end_triggered: bool,
    final_round_until_player: PlayerId,
}
```

更简单的实现方式：

```rust
struct GameState {
    players: Vec<PlayerState>,
    current_player_index: usize,
    round_start_player_index: usize,
    end_triggered: bool,
}
```

当某个玩家达到 15 分：

```rust
game.end_triggered = true;
```

之后继续走，直到轮到起始玩家前一位完成行动。

---

# 21. 胜负判定

最终分数最高者获胜。

如果平分，比较已购买发展卡数量，购买卡更少者获胜。

```rust
fn compare_players(a: &PlayerState, b: &PlayerState) -> Ordering {
    let score_a = calculate_score(a);
    let score_b = calculate_score(b);

    score_a.cmp(&score_b)
        .then_with(|| {
            // 分数相同，购买发展卡更少者更优
            b.purchased_cards.len().cmp(&a.purchased_cards.len())
        })
}
```

注意这里比较方向要小心。如果用降序排序，逻辑可以写得更直观。

---

# 22. 推荐的规则模块拆分

后续生成 Bevy 代码时，不建议一上来就把规则写进 `System` 里。建议先做纯 Rust 规则层，再接 Bevy ECS。

可以拆成：

```text
rules/
  mod.rs
  color.rs
  token.rs
  card.rs
  noble.rs
  player.rs
  market.rs
  actions.rs
  validation.rs
  scoring.rs
```

核心思想：

```text
Bevy 负责显示、输入、动画、状态切换
规则层负责判断动作是否合法、修改 GameState
```

---

# 23. 关键数据结构草案

```rust
struct GameState {
    players: Vec<PlayerState>,
    bank: Bank,
    decks: CardDecks,
    market: Market,
    nobles: Vec<Noble>,
    current_player_index: usize,
    end_triggered: bool,
}

struct PlayerState {
    id: PlayerId,
    tokens: TokenSet,
    reserved_cards: Vec<CardId>,
    purchased_cards: Vec<CardId>,
    nobles: Vec<NobleId>,
}

struct DevelopmentCard {
    id: CardId,
    level: CardLevel,
    color: CardColor,
    prestige: u8,
    cost: GemCost,
}

struct Noble {
    id: NobleId,
    prestige: u8,
    requirement: GemCost,
}

struct Bank {
    tokens: TokenSet,
}

struct Market {
    level1_visible: Vec<CardId>,
    level2_visible: Vec<CardId>,
    level3_visible: Vec<CardId>,
}
```

---

# 24. 建议给 AI 的代码生成提示词

你后续可以这样让 AI 生成规则代码：

```text
请用 Rust 为一个 Bevy 2D 卡牌游戏实现类似《璀璨宝石》的纯规则层。

要求：
1. 不要写 Bevy UI 和动画，只写纯 Rust 规则模块。
2. 实现宝石颜色、卡牌等级、发展卡、贵族、玩家、银行、市场、游戏状态。
3. 支持以下动作：
   - 拿 3 个不同颜色宝石
   - 拿 2 个相同颜色宝石
   - 保留公共区卡牌
   - 从牌堆盲抽保留
   - 购买公共区卡牌
   - 购买自己保留的卡牌
4. 实现动作合法性校验。
5. 实现购买折扣、金币万能支付、支付后筹码回银行。
6. 实现保留卡最多 3 张。
7. 实现玩家筹码上限 10 个，超过后返回 NeedDiscardTokens 状态。
8. 实现贵族自动拜访逻辑；如果满足多个贵族，返回 NeedChooseNoble。
9. 实现 15 分触发终局，最终轮结束后比较胜负。
10. 规则层要和 Bevy 解耦，方便后续在 Bevy System 中调用。
```

---

# 25. 最小可执行规则流程

一次玩家回合大致可以抽象成：

```rust
fn apply_action(
    game: &mut GameState,
    player_id: PlayerId,
    action: PlayerAction,
) -> Result<ActionOutcome, RuleError> {
    validate_action(game, player_id, &action)?;

    let outcome = execute_action(game, player_id, action)?;

    if outcome.requires_player_choice() {
        return Ok(outcome);
    }

    check_nobles(game, player_id)?;
    check_end_game(game, player_id);
    advance_turn(game);

    Ok(outcome)
}
```

建议不要一开始就把“动画飞牌”“翻牌”“筹码移动”混在规则里。规则层只返回事件：

```rust
enum GameEvent {
    TokensTaken { player: PlayerId, tokens: TokenSet },
    CardReserved { player: PlayerId, card: CardId },
    CardPurchased { player: PlayerId, card: CardId },
    MarketRefilled { level: CardLevel, card: Option<CardId> },
    NobleVisited { player: PlayerId, noble: NobleId },
    NeedDiscardTokens { player: PlayerId, count: u8 },
    NeedChooseNoble { player: PlayerId, nobles: Vec<NobleId> },
    EndGameTriggered { player: PlayerId },
}
```

然后 Bevy 根据这些事件播放动画即可。规则层越纯，后面 AI 生成 Bevy 代码越容易。
