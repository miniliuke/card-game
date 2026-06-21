use std::time::{SystemTime, UNIX_EPOCH};

use bevy::{
    prelude::*,
    ui::{ColorStop, LinearGradient, UiScale, Val2},
    window::PrimaryWindow,
};

use crate::{rules::*, AppState};

// === 常量与调色板 ===
const INK: Color = Color::srgb(0.028, 0.033, 0.047);
const PANEL: Color = Color::srgb(0.072, 0.082, 0.108);
const CREAM: Color = Color::srgb(0.95, 0.91, 0.82);
const MUTED: Color = Color::srgb(0.55, 0.58, 0.64);
const GOLD: Color = Color::srgb(0.91, 0.68, 0.29);
const GOLD_BRIGHT: Color = Color::srgb(1.0, 0.82, 0.44);
const OUTLINE: Color = Color::srgba(1.0, 1.0, 1.0, 0.11);

const VISIBLE_PER_LEVEL: usize = 4;

pub struct BattlePlugin;

impl Plugin for BattlePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AnimationCounts>()
            .init_resource::<ActionQueue>()
            .init_resource::<FocusCursor>()
            .init_resource::<UiDirty>()
            .init_resource::<PendingEvents>()
            .init_resource::<PendingPhase>()
            .init_resource::<PendingNobleCandidates>()
            .init_resource::<TokenPicker>()
            .init_resource::<DiscardBuffer>()
            .init_resource::<TurnCount>()
            .add_systems(OnEnter(AppState::Battle), setup_battle)
            .add_systems(OnExit(AppState::Battle), cleanup_battle);
    }
}

// === 动画组件（复用） ===
#[derive(Component)]
struct FlyAnimation {
    timer: Timer,
    target: Vec2,
}

#[derive(Component)]
struct DealAnimation {
    timer: Timer,
}

#[derive(Component)]
struct BattleScreen;

// === BattleAction（Bevy 组件，挂按钮上） ===

/// 拿 3 不同色的固定 3 元组（不能用 Vec，因 Component 需 Copy）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Triple([GemColor; 3]);

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
enum BattleAction {
    TakeThreeDifferentTokens(Triple),
    TakeTwoSameTokens(GemColor),
    ReserveVisibleCard { level: CardLevel, idx: usize },
    ReserveDeckCard(CardLevel),
    BuyVisibleCard { level: CardLevel, idx: usize },
    BuyReservedCard(usize),
}

impl BattleAction {
    fn to_player_action(self) -> PlayerAction {
        match self {
            Self::TakeThreeDifferentTokens(Triple([a, b, c])) => {
                PlayerAction::TakeThreeDifferentTokens(vec![a, b, c])
            }
            Self::TakeTwoSameTokens(c) => PlayerAction::TakeTwoSameTokens(c),
            Self::ReserveVisibleCard { level, idx } => {
                PlayerAction::ReserveVisibleCard { level, idx }
            }
            Self::ReserveDeckCard(level) => PlayerAction::ReserveDeckCard(level),
            Self::BuyVisibleCard { level, idx } => {
                PlayerAction::BuyVisibleCard { level, idx }
            }
            Self::BuyReservedCard(i) => PlayerAction::BuyReservedCard(i),
        }
    }
}

#[derive(Component)]
struct BattleRoot;

#[derive(Component)]
struct PlayerPanel(PlayerId);

#[derive(Component)]
struct PlayerScoreText(PlayerId);

#[derive(Component)]
struct PlayerStateText(PlayerId);

#[derive(Component)]
struct PlayerColorText {
    player: PlayerId,
    color: GemColor,
}

#[derive(Component)]
struct PlayerGoldText(PlayerId);

/// 保留卡行容器（每玩家一个）。
#[derive(Component)]
struct ReservedRow(PlayerId);

/// 单张保留卡按钮（owner 面板上可 Buy）。
#[derive(Component)]
struct ReservedCardButton {
    player: PlayerId,
    idx: usize,
}

/// 贵族行容器。
#[derive(Component)]
struct NoblesRow(PlayerId);

#[derive(Component)]
struct CardSlot {
    level: CardLevel,
    slot: usize,
}

#[derive(Component)]
struct CardButton {
    level: CardLevel,
    slot: usize,
}

#[derive(Component)]
struct DeckReserveButton(CardLevel);

#[derive(Component)]
struct DeckCountText(CardLevel);

#[derive(Component)]
struct ReserveMarketButton {
    level: CardLevel,
    slot: usize,
}

#[derive(Component)]
struct SupplyButton(GemColor);

#[derive(Component)]
struct SupplyX2Button(GemColor);

#[derive(Component)]
struct SupplyCountText(GemColor);

#[derive(Component)]
struct ConfirmTake3Button;

#[derive(Component)]
struct ClearSelectionButton;

#[derive(Component)]
struct SelectionHudText;

#[derive(Component)]
struct TurnText;

#[derive(Component)]
struct StatusText;

#[derive(Component)]
struct NobleBoardArea;

#[derive(Component)]
struct NobleBadgeOnBoard(NobleId);

/// 键盘焦点标记。zone 标识区域；按钮挂此组件供 keyboard_actions 定位。
#[derive(Component, Clone, Copy)]
struct Focusable {
    zone: FocusZone,
    normal_border: Color,
}

#[derive(Resource, Default)]
struct AnimationCounts {
    flying: usize,
    dealing: usize,
}

impl AnimationCounts {
    fn busy(&self) -> bool {
        self.flying + self.dealing > 0
    }
}

// === 核心资源 ===

#[derive(Resource)]
struct BattleModel(GameState);

#[derive(Resource, Default, Clone)]
struct PendingEvents(Vec<GameEvent>);

#[derive(Resource, Default, Clone, PartialEq, Eq, Debug)]
struct TurnCount(u32);

#[derive(Resource, Clone, PartialEq, Eq, Debug)]
enum BattlePhase {
    Idle,
    AwaitDiscard { excess: u8 },
    AwaitNobleChoice { candidates: Vec<NobleId> },
    GameOver { winner: PlayerId, standings: Vec<(PlayerId, u16)> },
}

impl Default for BattlePhase {
    fn default() -> Self {
        Self::Idle
    }
}

/// 动画/事件播完后才提交为 BattlePhase（避免动画未完就弹覆盖层）。
#[derive(Resource, Default, Clone, PartialEq, Eq, Debug)]
struct PendingPhase(Option<BattlePhase>);

/// 买牌触发"先弃牌再选贵族"时，暂存贵族候选。
#[derive(Resource, Default, Clone, PartialEq, Eq, Debug)]
struct PendingNobleCandidates(Option<Vec<NobleId>>);

#[derive(Resource, Default)]
struct ActionQueue(Vec<BattleAction>);

#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
struct FocusCursor {
    zone: FocusZone,
}

#[derive(Resource)]
struct UiDirty(bool);

impl Default for UiDirty {
    fn default() -> Self {
        Self(true)
    }
}

/// 拿筹码选择缓冲区（TakeThreeDifferentTokens）。选满 3 后 Confirm 提交。
#[derive(Resource, Default, Clone, PartialEq, Eq, Debug)]
struct TokenPicker {
    selected: Vec<GemColor>,
}

/// 弃牌覆盖层：玩家选择归还的筹码。total 必须等于 excess。
#[derive(Resource, Default, Clone, PartialEq, Eq, Debug)]
struct DiscardBuffer {
    returned: TokenSet,
    excess: u8,
}

/// 键盘焦点区域。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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

impl Default for FocusZone {
    fn default() -> Self {
        Self::Market { level: CardLevel::Level1, slot: 0 }
    }
}

/// 纯函数：判定此刻是否应把 PendingPhase 提交为 BattlePhase。
/// 条件：事件队列空 && 不忙 && PendingPhase 非空。
fn should_commit_phase(
    pending: &PendingEvents,
    busy: bool,
    _current_phase: &BattlePhase,
    pending_phase: &PendingPhase,
) -> bool {
    pending.0.is_empty() && !busy && pending_phase.0.is_some()
}

/// 输入门控：仅 Idle + 不忙 + 无待播事件时允许新行动。
fn can_act(phase: &BattlePhase, busy: bool, pending: &PendingEvents) -> bool {
    matches!(phase, BattlePhase::Idle) && !busy && pending.0.is_empty()
}

fn setup_battle(mut commands: Commands) {
    let seed = now_seed();
    let state = GameState::new_seeded(2, seed).expect("2-player game always valid");
    let model = BattleModel(state);

    commands.insert_resource(BattlePhase::Idle);
    commands.init_resource::<PendingEvents>();
    commands.init_resource::<PendingPhase>();
    commands.init_resource::<PendingNobleCandidates>();
    commands.init_resource::<TokenPicker>();
    commands.init_resource::<DiscardBuffer>();
    commands.init_resource::<TurnCount>();
    commands.insert_resource(FocusCursor::default());
    commands.insert_resource(ActionQueue::default());
    commands.insert_resource(AnimationCounts::default());
    commands.insert_resource(UiDirty(true));

    commands
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundGradient::from(LinearGradient {
                angle: 2.35,
                stops: vec![
                    ColorStop::new(INK, percent(0)),
                    ColorStop::new(Color::srgb(0.07, 0.055, 0.085), percent(52)),
                    ColorStop::new(Color::srgb(0.025, 0.072, 0.075), percent(100)),
                ],
                ..default()
            }),
            BattleScreen,
            BattleRoot,
        ))
        .with_children(|root| {
            spawn_ambient_shapes(root);
            spawn_top_bar(root);
            spawn_noble_board(root, &model);
            root.spawn(Node {
                width: percent(100),
                max_width: px(1680),
                flex_grow: 1.0,
                align_self: AlignSelf::Center,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Stretch,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(px(24), px(14)),
                column_gap: px(18),
                ..default()
            })
            .with_children(|main| {
                spawn_player_panel(main, 0);
                spawn_market(main, &model);
                spawn_player_panel(main, 1);
            });
            spawn_footer(root);
        });

    commands.insert_resource(model);
}

fn cleanup_battle(
    mut commands: Commands,
    screen: Single<Entity, With<BattleScreen>>,
    mut ui_scale: ResMut<UiScale>,
) {
    commands.entity(*screen).despawn();
    commands.remove_resource::<BattleModel>();
    commands.remove_resource::<BattlePhase>();
    commands.remove_resource::<PendingEvents>();
    commands.remove_resource::<PendingPhase>();
    commands.remove_resource::<PendingNobleCandidates>();
    commands.remove_resource::<TokenPicker>();
    commands.remove_resource::<DiscardBuffer>();
    commands.remove_resource::<TurnCount>();
    ui_scale.0 = 1.0;
}

fn spawn_player_panel(parent: &mut ChildSpawnerCommands, player: PlayerId) {
    parent
        .spawn((
            Node {
                width: percent(19),
                min_width: px(205),
                max_width: px(270),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(px(16)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(12)),
                row_gap: px(10),
                ..default()
            },
            BackgroundColor(Color::srgba(0.055, 0.063, 0.085, 0.92)),
            BorderColor::all(if player == 0 { GOLD } else { OUTLINE }),
            BoxShadow(vec![ShadowStyle {
                color: Color::srgba(0.0, 0.0, 0.0, 0.26),
                x_offset: px(0),
                y_offset: px(12),
                spread_radius: px(0),
                blur_radius: px(26),
            }]),
            PlayerPanel(player),
        ))
        .with_children(|panel| {
            // Header: name + score
            panel
                .spawn(Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    margin: UiRect::bottom(px(4)),
                    ..default()
                })
                .with_children(|header| {
                    header.spawn((
                        Text::new(format!("PLAYER {}", player + 1)),
                        TextFont { font_size: 15.0, ..default() },
                        TextColor(CREAM),
                    ));
                    header.spawn((
                        Text::new("0 PTS"),
                        TextFont { font_size: 11.0, ..default() },
                        TextColor(GOLD),
                        PlayerScoreText(player),
                    ));
                });
            // State line
            panel.spawn((
                Text::new("WAITING"),
                TextFont { font_size: 9.0, ..default() },
                TextColor(MUTED),
                PlayerStateText(player),
            ));
            // 5 normal color rows
            for color in GemColor::NORMAL {
                spawn_player_color_row(panel, player, color);
            }
            // Gold row
            spawn_player_gold_row(panel, player);
            // Reserved row container
            panel
                .spawn((Node { width: percent(100), row_gap: px(4), ..default() }, ReservedRow(player)))
                .with_children(|row| {
                    row.spawn((
                        Text::new("RESERVED (0/3)"),
                        TextFont { font_size: 8.0, ..default() },
                        TextColor(MUTED.with_alpha(0.7)),
                    ));
                });
            // Nobles row container
            panel
                .spawn((Node { width: percent(100), ..default() }, NoblesRow(player)))
                .with_children(|row| {
                    row.spawn((
                        Text::new("NOBLES"),
                        TextFont { font_size: 8.0, ..default() },
                        TextColor(MUTED.with_alpha(0.7)),
                    ));
                });
            // Filler
            panel.spawn(Node { flex_grow: 1.0, ..default() });
        });
}

fn spawn_player_color_row(parent: &mut ChildSpawnerCommands, player: PlayerId, color: GemColor) {
    parent
        .spawn((
            Node {
                width: percent(100),
                min_height: px(40),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::axes(px(10), px(6)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(8)),
                ..default()
            },
            BackgroundColor(gem_color(color).with_alpha(0.13)),
            BorderColor::all(gem_color(color).with_alpha(0.38)),
        ))
        .with_children(|row| {
            row.spawn((
                Text::new(color_name(color)),
                TextFont { font_size: 10.0, ..default() },
                TextColor(CREAM),
            ));
            row.spawn((
                Text::new("C 0 / T 0"),
                TextFont { font_size: 10.0, ..default() },
                TextColor(CREAM),
                PlayerColorText { player, color },
            ));
        });
}

fn spawn_player_gold_row(parent: &mut ChildSpawnerCommands, player: PlayerId) {
    parent
        .spawn((
            Node {
                width: percent(100),
                min_height: px(32),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::axes(px(10), px(5)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(8)),
                ..default()
            },
            BackgroundColor(gem_color(GemColor::Gold).with_alpha(0.13)),
            BorderColor::all(gem_color(GemColor::Gold).with_alpha(0.38)),
        ))
        .with_children(|row| {
            row.spawn((
                Text::new("GOLD"),
                TextFont { font_size: 10.0, ..default() },
                TextColor(CREAM),
            ));
            row.spawn((
                Text::new("T 0"),
                TextFont { font_size: 10.0, ..default() },
                TextColor(CREAM),
                PlayerGoldText(player),
            ));
        });
}

fn spawn_market(parent: &mut ChildSpawnerCommands, model: &BattleModel) {
    parent
        .spawn(Node {
            width: percent(60),
            max_width: px(900),
            flex_grow: 1.0,
            flex_direction: FlexDirection::Column,
            row_gap: px(8),
            ..default()
        })
        .with_children(|market| {
            market
                .spawn(Node {
                    width: percent(100),
                    align_items: AlignItems::End,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::axes(px(4), px(0)),
                    ..default()
                })
                .with_children(|title| {
                    title.spawn((
                        Text::new("PUBLIC MARKET"),
                        TextFont { font_size: 19.0, ..default() },
                        TextColor(CREAM),
                    ));
                    title.spawn((
                        Text::new("Buy [click] / Reserve [R] / Blind [deck]"),
                        TextFont { font_size: 9.0, ..default() },
                        TextColor(MUTED),
                    ));
                });

            // Level3 顶 -> Level1 底
            for level in [CardLevel::Level3, CardLevel::Level2, CardLevel::Level1] {
                spawn_market_row(market, level, model);
            }
            spawn_token_supply(market, model);
            spawn_selection_hud(market);
        });
}

fn spawn_market_row(parent: &mut ChildSpawnerCommands, level: CardLevel, model: &BattleModel) {
    parent
        .spawn(Node {
            width: percent(100),
            flex_grow: 1.0,
            min_height: px(128),
            max_height: px(158),
            align_items: AlignItems::Stretch,
            column_gap: px(8),
            ..default()
        })
        .with_children(|row| {
            // Deck 盲抽按钮 + 计数
            row.spawn((
                Button,
                Node {
                    width: px(72),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(9)),
                    row_gap: px(5),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.03, 0.035, 0.05, 0.8)),
                BorderColor::all(GOLD.with_alpha(0.32 + level.index() as f32 * 0.12)),
                DeckReserveButton(level),
            ))
            .with_children(|deck| {
                deck.spawn((
                    Text::new(format!("TIER {}", level.index() + 1)),
                    TextFont { font_size: 9.0, ..default() },
                    TextColor(GOLD),
                ));
                deck.spawn((
                    Text::new(format!("{:02}", model.0.decks.remaining(level))),
                    TextFont { font_size: 21.0, ..default() },
                    TextColor(CREAM),
                    DeckCountText(level),
                ));
                deck.spawn((
                    Text::new("BLIND"),
                    TextFont { font_size: 8.0, ..default() },
                    TextColor(MUTED),
                ));
            });

            // 4 个槽
            for slot in 0..VISIBLE_PER_LEVEL {
                row.spawn((card_slot_node(), CardSlot { level, slot }))
                    .with_children(|slot_parent| {
                        if let Some(card) = model.0.market.visible(level).get(slot) {
                            spawn_card_button(slot_parent, *card, level, slot);
                        }
                    });
            }
        });
}

fn card_slot_node() -> Node {
    Node {
        flex_grow: 1.0,
        flex_basis: percent(0),
        min_width: px(92),
        height: percent(100),
        ..default()
    }
}

fn spawn_card_button(
    parent: &mut ChildSpawnerCommands,
    card: DevelopmentCard,
    level: CardLevel,
    slot: usize,
) {
    parent
        .spawn((
            Button,
            Node {
                width: percent(100),
                height: percent(100),
                min_height: px(126),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::all(px(9)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(9)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundGradient::from(LinearGradient {
                angle: 2.3,
                stops: vec![
                    ColorStop::new(gem_color(card.color.to_gem()).with_alpha(0.26), percent(0)),
                    ColorStop::new(PANEL, percent(58)),
                    ColorStop::new(Color::srgb(0.035, 0.039, 0.055), percent(100)),
                ],
                ..default()
            }),
            BorderColor::all(gem_color(card.color.to_gem()).with_alpha(0.68)),
            UiTransform::default(),
            BoxShadow(vec![ShadowStyle {
                color: Color::srgba(0.0, 0.0, 0.0, 0.28),
                x_offset: px(0),
                y_offset: px(7),
                spread_radius: px(0),
                blur_radius: px(13),
            }]),
            CardButton { level, slot },
            BattleAction::BuyVisibleCard { level, idx: slot },
        ))
        .with_children(|face| {
            face.spawn(Node {
                width: percent(100),
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            })
            .with_children(|top| {
                top.spawn((
                    Text::new(format!("T{}", level.index() + 1)),
                    TextFont { font_size: 9.0, ..default() },
                    TextColor(GOLD),
                ));
                top.spawn((
                    Text::new(format!("{} PTS", card.prestige)),
                    TextFont { font_size: 9.0, ..default() },
                    TextColor(CREAM),
                ));
            });
            face.spawn(Node {
                width: percent(100),
                justify_content: JustifyContent::Center,
                column_gap: px(3),
                ..default()
            })
            .with_children(|costs| {
                for color in CardColor::ALL {
                    let amount = card.cost.get(color);
                    costs
                        .spawn((
                            Node {
                                width: px(18),
                                height: px(18),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::MAX,
                                border: UiRect::all(px(1)),
                                ..default()
                            },
                            BackgroundColor(gem_color(color.to_gem())
                                .with_alpha(if amount == 0 { 0.18 } else { 0.72 })),
                            BorderColor::all(gem_color(color.to_gem()).with_alpha(0.85)),
                        ))
                        .with_children(|dot| {
                            dot.spawn((
                                Text::new(amount.to_string()),
                                TextFont { font_size: 8.0, ..default() },
                                TextColor(if matches!(color, CardColor::White) {
                                    INK
                                } else {
                                    CREAM
                                }),
                            ));
                        });
                }
            });
            // R 保留按钮（叠加底部）
            face.spawn((
                Button,
                Node {
                    position_type: PositionType::Absolute,
                    right: px(4),
                    bottom: px(4),
                    width: px(20),
                    height: px(18),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(4)),
                    ..default()
                },
                BackgroundColor(GOLD.with_alpha(0.2)),
                BorderColor::all(GOLD.with_alpha(0.6)),
                ReserveMarketButton { level, slot },
            ))
            .with_children(|r| {
                r.spawn((
                    Text::new("R"),
                    TextFont { font_size: 9.0, ..default() },
                    TextColor(GOLD_BRIGHT),
                ));
            });
        });
}

fn spawn_token_supply(parent: &mut ChildSpawnerCommands, model: &BattleModel) {
    parent
        .spawn((
            Node {
                width: percent(100),
                height: px(82),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::all(px(9)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(10)),
                column_gap: px(8),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.046, 0.064, 0.88)),
            BorderColor::all(OUTLINE),
        ))
        .with_children(|supply| {
            supply.spawn((
                Text::new("CURRENCY"),
                TextFont { font_size: 9.0, ..default() },
                TextColor(MUTED),
                Node { width: px(58), ..default() },
            ));
            // 5 normal color buttons
            for color in GemColor::NORMAL {
                spawn_supply_button(supply, color, model);
            }
            // Gold info (no button)
            supply
                .spawn((
                    Node {
                        width: px(58),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                ))
                .with_children(|gold| {
                    gold.spawn((
                        Text::new("GOLD"),
                        TextFont { font_size: 8.0, ..default() },
                        TextColor(GOLD),
                    ));
                    gold.spawn((
                        Text::new(format!("x{}", model.0.bank.tokens.gold)),
                        TextFont { font_size: 14.0, ..default() },
                        TextColor(CREAM),
                    ));
                });
        });
}

fn spawn_supply_button(parent: &mut ChildSpawnerCommands, color: GemColor, model: &BattleModel) {
    let count = model.0.bank.tokens.get(color);
    parent
        .spawn((
            Button,
            Node {
                flex_grow: 1.0,
                height: px(58),
                min_width: px(64),
                position_type: PositionType::Relative,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: px(7),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(9)),
                ..default()
            },
            BackgroundColor(gem_color(color).with_alpha(0.13)),
            BorderColor::all(gem_color(color).with_alpha(0.55)),
            UiTransform::default(),
            SupplyButton(color),
        ))
        .with_children(|token| {
            token
                .spawn((
                    Node {
                        width: px(28),
                        height: px(28),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        border_radius: BorderRadius::MAX,
                        border: UiRect::all(px(2)),
                        ..default()
                    },
                    BackgroundColor(gem_color(color)),
                    BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.34)),
                ))
                .with_children(|coin| {
                    coin.spawn((
                        Text::new(color_short(color)),
                        TextFont { font_size: 9.0, ..default() },
                        TextColor(if matches!(color, GemColor::White) { INK } else { CREAM }),
                    ));
                });
            token.spawn((
                Text::new(format!("x{count}")),
                TextFont { font_size: 12.0, ..default() },
                TextColor(CREAM),
                SupplyCountText(color),
            ));
            // x2 badge (shown when count >= 4)
            if count >= 4 {
                token.spawn((
                    Button,
                    Node {
                        position_type: PositionType::Absolute,
                        right: px(2),
                        top: px(2),
                        width: px(22),
                        height: px(16),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        border: UiRect::all(px(1)),
                        border_radius: BorderRadius::all(px(4)),
                        ..default()
                    },
                    BackgroundColor(GOLD.with_alpha(0.85)),
                    BorderColor::all(GOLD_BRIGHT),
                    SupplyX2Button(color),
                ))
                .with_children(|x2| {
                    x2.spawn((
                        Text::new("x2"),
                        TextFont { font_size: 8.0, ..default() },
                        TextColor(INK),
                    ));
                });
            }
        });
}

fn spawn_selection_hud(market: &mut ChildSpawnerCommands) {
    market
        .spawn(Node {
            width: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            column_gap: px(8),
            ..default()
        })
        .with_children(|hud| {
            hud.spawn((
                Text::new("0/3"),
                TextFont { font_size: 11.0, ..default() },
                TextColor(MUTED),
                SelectionHudText,
            ));
            hud.spawn((
                Button,
                Node {
                    width: px(90),
                    height: px(26),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(6)),
                    ..default()
                },
                BackgroundColor(GOLD.with_alpha(0.3)),
                BorderColor::all(GOLD.with_alpha(0.5)),
                ConfirmTake3Button,
            ))
            .with_children(|b| {
                b.spawn((
                    Text::new("TAKE 3"),
                    TextFont { font_size: 10.0, ..default() },
                    TextColor(CREAM),
                ));
            });
            hud.spawn((
                Button,
                Node {
                    width: px(70),
                    height: px(26),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(6)),
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.05)),
                BorderColor::all(OUTLINE),
                ClearSelectionButton,
            ))
            .with_children(|b| {
                b.spawn((
                    Text::new("CLEAR"),
                    TextFont { font_size: 10.0, ..default() },
                    TextColor(MUTED),
                ));
            });
        });
}

fn spawn_ambient_shapes(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: px(420),
            height: px(420),
            right: px(-180),
            top: px(-210),
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BackgroundColor(Color::srgba(0.22, 0.55, 0.50, 0.07)),
    ));
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: px(360),
            height: px(360),
            left: px(-180),
            bottom: px(-210),
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BackgroundColor(Color::srgba(0.91, 0.68, 0.29, 0.055)),
    ));
}

fn spawn_top_bar(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Node {
            width: percent(100),
            height: px(58),
            padding: UiRect::axes(px(30), px(0)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            border: UiRect::bottom(px(1)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.02, 0.025, 0.038, 0.72)),
        BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.07)),
    ))
    .with_children(|bar| {
        bar.spawn((
            Text::new("ARCANA TABLE  /  MARKET"),
            TextFont { font_size: 15.0, ..default() },
            TextColor(CREAM),
        ));
        bar.spawn((
            Text::new("TURN 1  /  PLAYER 1"),
            TextFont { font_size: 12.0, ..default() },
            TextColor(GOLD),
            TurnText,
        ));
    });
}

fn spawn_noble_board(root: &mut ChildSpawnerCommands, model: &BattleModel) {
    root
        .spawn((
            Node {
                width: percent(100),
                height: px(64),
                align_items: AlignItems::Center,
                column_gap: px(10),
                padding: UiRect::axes(px(12), px(6)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(10)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.07, 0.85)),
            BorderColor::all(GOLD.with_alpha(0.3)),
            NobleBoardArea,
        ))
        .with_children(|board| {
            board.spawn((
                Text::new("NOBLES"),
                TextFont { font_size: 9.0, ..default() },
                TextColor(GOLD),
            ));
            for noble in &model.0.nobles.available {
                spawn_noble_badge(board, noble.id, noble.requirement, noble.prestige);
            }
        });
}

fn spawn_noble_badge(
    parent: &mut ChildSpawnerCommands,
    id: NobleId,
    req: GemCost,
    prestige: u8,
) {
    parent
        .spawn((
            Node {
                width: px(48),
                height: px(48),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(8)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.91, 0.68, 0.29, 0.12)),
            BorderColor::all(GOLD.with_alpha(0.6)),
            NobleBadgeOnBoard(id),
        ))
        .with_children(|b| {
            b.spawn((
                Text::new(format!("{prestige}P {req_w}.{req_b}.{req_g}.{req_r}.{req_k}",
                    req_w = req.white, req_b = req.blue, req_g = req.green, req_r = req.red, req_k = req.black)),
                TextFont { font_size: 7.0, ..default() },
                TextColor(CREAM),
                TextLayout::new_with_justify(Justify::Center),
            ));
        });
}

fn spawn_footer(root: &mut ChildSpawnerCommands) {
    root.spawn(Node {
        width: percent(100),
        height: px(48),
        padding: UiRect::axes(px(30), px(0)),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::SpaceBetween,
        ..default()
    })
    .with_children(|footer| {
        footer.spawn((
            Text::new("CLICK BUY / R RESERVE / CLICK TOKEN x2 / TAKE 3 / ESC MENU"),
            TextFont { font_size: 9.0, ..default() },
            TextColor(MUTED),
        ));
        footer.spawn((
            Text::new("Choose an action."),
            TextFont { font_size: 10.0, ..default() },
            TextColor(GOLD),
            StatusText,
        ));
    });
}

// === 辅助函数 ===
fn now_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(1, |d| d.as_nanos() as u64)
}

fn color_name(color: GemColor) -> &'static str {
    match color {
        GemColor::White => "WHITE",
        GemColor::Blue => "BLUE",
        GemColor::Green => "GREEN",
        GemColor::Red => "RED",
        GemColor::Black => "BLACK",
        GemColor::Gold => "GOLD",
    }
}

fn color_short(color: GemColor) -> &'static str {
    match color {
        GemColor::White => "W",
        GemColor::Blue => "U",
        GemColor::Green => "G",
        GemColor::Red => "R",
        GemColor::Black => "B",
        GemColor::Gold => "*",
    }
}

fn gem_color(color: GemColor) -> Color {
    match color {
        GemColor::White => Color::srgb(0.88, 0.86, 0.78),
        GemColor::Blue => Color::srgb(0.20, 0.47, 0.78),
        GemColor::Green => Color::srgb(0.22, 0.61, 0.43),
        GemColor::Red => Color::srgb(0.78, 0.25, 0.24),
        GemColor::Black => Color::srgb(0.12, 0.13, 0.17),
        GemColor::Gold => Color::srgb(0.91, 0.68, 0.29),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gem_color_handles_all_six() {
        for c in [GemColor::White, GemColor::Blue, GemColor::Green, GemColor::Red, GemColor::Black, GemColor::Gold] {
            let _ = gem_color(c); // 不 panic 即可
        }
    }

    #[test]
    fn color_name_covers_gold() {
        assert_eq!(color_name(GemColor::Gold), "GOLD");
    }

    #[test]
    fn pending_phase_not_committed_while_events_pending() {
        let mut pending = PendingEvents::default();
        pending.0.push(GameEvent::TokensTaken { player: 0, tokens: TokenSet::default() });
        let phase = BattlePhase::Idle;
        let pp = PendingPhase::default(); // None
        let busy = false;
        assert!(!should_commit_phase(&pending, busy, &phase, &pp));
    }

    #[test]
    fn pending_phase_not_committed_while_busy() {
        let pending = PendingEvents::default(); // 空
        let phase = BattlePhase::Idle;
        let pp = PendingPhase(Some(BattlePhase::AwaitDiscard { excess: 1 }));
        let busy = true;
        assert!(!should_commit_phase(&pending, busy, &phase, &pp));
    }

    #[test]
    fn pending_phase_committed_when_idle_events_empty_and_not_busy() {
        let pending = PendingEvents::default();
        let phase = BattlePhase::Idle;
        let pp = PendingPhase(Some(BattlePhase::AwaitNobleChoice { candidates: vec![0] }));
        let busy = false;
        assert!(should_commit_phase(&pending, busy, &phase, &pp));
    }

    #[test]
    fn pending_phase_none_never_commits() {
        let pending = PendingEvents::default();
        let phase = BattlePhase::Idle;
        let pp = PendingPhase::default(); // None
        let busy = false;
        assert!(!should_commit_phase(&pending, busy, &phase, &pp));
    }

    #[test]
    fn maps_all_actions_to_player_action() {
        let cases: Vec<(BattleAction, PlayerAction)> = vec![
            (
                BattleAction::TakeThreeDifferentTokens(Triple([GemColor::White, GemColor::Blue, GemColor::Green])),
                PlayerAction::TakeThreeDifferentTokens(vec![GemColor::White, GemColor::Blue, GemColor::Green]),
            ),
            (
                BattleAction::TakeTwoSameTokens(GemColor::Red),
                PlayerAction::TakeTwoSameTokens(GemColor::Red),
            ),
            (
                BattleAction::ReserveVisibleCard { level: CardLevel::Level1, idx: 2 },
                PlayerAction::ReserveVisibleCard { level: CardLevel::Level1, idx: 2 },
            ),
            (
                BattleAction::ReserveDeckCard(CardLevel::Level2),
                PlayerAction::ReserveDeckCard(CardLevel::Level2),
            ),
            (
                BattleAction::BuyVisibleCard { level: CardLevel::Level3, idx: 0 },
                PlayerAction::BuyVisibleCard { level: CardLevel::Level3, idx: 0 },
            ),
            (
                BattleAction::BuyReservedCard(1),
                PlayerAction::BuyReservedCard(1),
            ),
        ];
        for (ba, expected) in cases {
            assert_eq!(ba.to_player_action(), expected);
        }
    }

    #[test]
    fn can_act_only_when_idle_not_busy_events_empty() {
        let idle = BattlePhase::Idle;
        let non_idle = BattlePhase::AwaitDiscard { excess: 1 };
        let empty = PendingEvents::default();

        assert!(can_act(&idle, false, &empty));
        assert!(!can_act(&idle, true, &empty));       // busy
        assert!(!can_act(&non_idle, false, &empty)); // not idle
        let mut pending = PendingEvents::default();
        pending.0.push(GameEvent::NobleVisited { player: 0, noble: 0 });
        assert!(!can_act(&idle, false, &pending));    // events pending
    }
}
