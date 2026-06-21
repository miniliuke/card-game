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
            .add_systems(OnExit(AppState::Battle), cleanup_battle)
            .add_systems(
                Update,
                (
                    mouse_actions.run_if(input_gate),
                    apply_actions,
                    play_events,
                    commit_pending_phase,
                    animate_flights,
                    animate_deals,
                    refresh_battle_ui,
                    refresh_selection_hud,
                    update_focus_visuals,
                    button_hover_effects,
                    responsive_battle_layout,
                )
                    .chain()
                    .run_if(in_state(AppState::Battle)),
            );
    }
}

fn input_gate(
    phase: Res<BattlePhase>,
    anim: Res<AnimationCounts>,
    pending: Res<PendingEvents>,
) -> bool {
    can_act(&phase, anim.busy(), &pending)
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

/// 把 ActionOutcome 映射为待提交的 BattlePhase（None = 保持 Idle）。
/// 注意 NeedFinalDiscardThenChooseNoble 只映射到 discard 阶段；
/// candidates 由调用方存入 PendingNobleCandidates。
fn outcome_to_pending(outcome: ActionOutcome) -> Option<BattlePhase> {
    match outcome {
        ActionOutcome::Complete => None,
        ActionOutcome::NeedDiscardTokens { excess } => {
            Some(BattlePhase::AwaitDiscard { excess })
        }
        ActionOutcome::NeedChooseNoble { candidates } => {
            Some(BattlePhase::AwaitNobleChoice { candidates })
        }
        ActionOutcome::NeedFinalDiscardThenChooseNoble { excess, .. } => {
            Some(BattlePhase::AwaitDiscard { excess })
        }
    }
}

/// 若 events 含 GameOver，返回对应 BattlePhase。
fn game_over_phase(events: &[GameEvent]) -> Option<BattlePhase> {
    events.iter().find_map(|e| match e {
        GameEvent::GameOver { winner, standings } => {
            Some(BattlePhase::GameOver { winner: *winner, standings: standings.clone() })
        }
        _ => None,
    })
}

/// 从 NeedFinalDiscardThenChooseNoble 提取 candidates。
fn final_noble_candidates(outcome: &ActionOutcome) -> Option<Vec<NobleId>> {
    match outcome {
        ActionOutcome::NeedFinalDiscardThenChooseNoble { candidates, .. } => {
            Some(candidates.clone())
        }
        _ => None,
    }
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

fn mouse_actions(
    mut interactions: Query<
        (&Interaction, &mut BorderColor, Option<&BattleAction>, Entity),
        Changed<Interaction>,
    >,
    supply: Query<(&Interaction, &SupplyButton), Changed<Interaction>>,
    supply_x2: Query<(&Interaction, &SupplyX2Button), Changed<Interaction>>,
    confirm: Query<&Interaction, (Changed<Interaction>, With<ConfirmTake3Button>)>,
    clear: Query<&Interaction, (Changed<Interaction>, With<ClearSelectionButton>)>,
    mut picker: ResMut<TokenPicker>,
    mut queue: ResMut<ActionQueue>,
    model: Res<BattleModel>,
) {
    // Supply single click -> buffer
    for (interaction, btn) in &supply {
        if matches!(*interaction, Interaction::Pressed) {
            let c = btn.0;
            if picker.selected.len() < 3
                && !picker.selected.contains(&c)
                && model.0.bank.tokens.get(c) >= 1
            {
                picker.selected.push(c);
            }
        }
    }
    // x2 click -> direct enqueue
    for (interaction, btn) in &supply_x2 {
        if matches!(*interaction, Interaction::Pressed) {
            queue.0.push(BattleAction::TakeTwoSameTokens(btn.0));
            picker.selected.clear();
        }
    }
    // Confirm take 3
    if let Ok(interaction) = confirm.single() {
        if matches!(*interaction, Interaction::Pressed) && picker.selected.len() == 3 {
            let triple = Triple([
                picker.selected[0],
                picker.selected[1],
                picker.selected[2],
            ]);
            queue.0.push(BattleAction::TakeThreeDifferentTokens(triple));
            picker.selected.clear();
        }
    }
    // Clear
    if let Ok(interaction) = clear.single() {
        if matches!(*interaction, Interaction::Pressed) {
            picker.selected.clear();
        }
    }
    // BattleAction buttons (market buy / reserve market / deck / reserved)
    for (interaction, _border, action, _entity) in &mut interactions {
        if matches!(*interaction, Interaction::Pressed) {
            if let Some(a) = action {
                queue.0.push(*a);
            }
        }
    }
}

fn apply_actions(
    mut commands: Commands,
    mut queue: ResMut<ActionQueue>,
    mut model: ResMut<BattleModel>,
    mut pending_events: ResMut<PendingEvents>,
    mut pending_phase: ResMut<PendingPhase>,
    mut pending_nobles: ResMut<PendingNobleCandidates>,
    anim: Res<AnimationCounts>,
    mut status: Single<&mut Text, With<StatusText>>,
    mut dirty: ResMut<UiDirty>,
    mut turn: ResMut<TurnCount>,
) {
    // 防重入
    if pending_phase.0.is_some() || anim.busy() || !pending_events.0.is_empty() {
        return;
    }
    let actions = std::mem::take(&mut queue.0);
    if actions.is_empty() {
        return;
    }
    for action in actions {
        let pid = model.0.current_id();
        let result = match apply_action(&mut model.0, pid, action.to_player_action()) {
            Ok(r) => r,
            Err(e) => {
                ***status = rule_error_message(e).to_string();
                continue;
            }
        };
        // 暂存 candidates 若为 final discard + noble
        if let Some(cands) = final_noble_candidates(&result.outcome) {
            pending_nobles.0 = Some(cands);
        }
        // GameOver 优先
        if let Some(phase) = game_over_phase(&result.events) {
            pending_events.0.extend(result.events);
            pending_phase.0 = Some(phase);
        } else if let Some(phase) = outcome_to_pending(result.outcome) {
            pending_events.0.extend(result.events);
            pending_phase.0 = Some(phase);
        } else {
            pending_events.0.extend(result.events);
        }
        turn.0 += 1;
    }
    // 让 status/dirty 反映；事件播放系统会逐个置 dirty
    let _ = (&mut commands, &mut dirty);
}

fn rule_error_message(e: RuleError) -> &'static str {
    match e {
        RuleError::NotYourTurn => "Not your turn.",
        RuleError::TooManyReserved => "Reserved cards full (3).",
        RuleError::BankInsufficient => "Bank has insufficient tokens.",
        RuleError::TokenLimitExceeded => "Token limit exceeded.",
        RuleError::CardNotFound => "Card not found.",
        RuleError::CannotAfford => "Cannot afford that card.",
        RuleError::InvalidTokenSelection => "Invalid token selection.",
        RuleError::NobleNotEligible => "Noble not eligible.",
        RuleError::DeckEmpty => "Deck is empty.",
        RuleError::InvalidResume => "Invalid resume.",
        RuleError::GameOver => "Game is over.",
        RuleError::InvalidPlayerCount => "Invalid player count.",
    }
}

fn play_events(
    mut commands: Commands,
    root: Single<Entity, With<BattleRoot>>,
    mut pending: ResMut<PendingEvents>,
    mut anim: ResMut<AnimationCounts>,
    mut dirty: ResMut<UiDirty>,
    mut status: Single<&mut Text, With<StatusText>>,
    model: Res<BattleModel>,
    card_slots: Query<(Entity, &CardSlot)>,
) {
    if pending.0.is_empty() {
        return;
    }
    let event = pending.0.remove(0);
    let active = model.0.current_id();
    let dir = if active == 0 { -1.0 } else { 1.0 };
    match &event {
        GameEvent::TokensTaken { player, tokens } => {
            ***status = format!("Player {} took tokens.", player + 1);
            let n = tokens.total();
            for (i, c) in GemColor::NORMAL.iter().enumerate() {
                let amt = tokens.get(*c);
                if amt > 0 {
                    spawn_fly_coin(&mut commands, *root, *c, dir, i as f32, *player, n);
                    anim.flying += 1;
                }
            }
        }
        GameEvent::TokensReturned { player, tokens } => {
            ***status = format!("Player {} returned tokens.", player + 1);
            for c in GemColor::NORMAL {
                let amt = tokens.get(c);
                if amt > 0 {
                    for _ in 0..amt {
                        spawn_fly_coin_back(&mut commands, *root, c, dir, *player);
                        anim.flying += 1;
                    }
                }
            }
        }
        GameEvent::CardReserved { player, got_gold, .. } => {
            ***status = format!(
                "Player {} reserved a card{}.",
                player + 1,
                if *got_gold { " (+gold)" } else { "" }
            );
            spawn_fly_card(&mut commands, *root, dir, *player);
            anim.flying += 1;
            if *got_gold {
                spawn_fly_coin(&mut commands, *root, GemColor::Gold, dir, 0.0, *player, 1);
                anim.flying += 1;
            }
        }
        GameEvent::CardPurchased { player, paid, .. } => {
            ***status = format!("Player {} bought a card.", player + 1);
            spawn_fly_card(&mut commands, *root, dir, *player);
            anim.flying += 1;
            for c in GemColor::NORMAL {
                let amt = paid.get(c);
                if amt > 0 {
                    for _ in 0..amt {
                        spawn_fly_coin_back(&mut commands, *root, c, dir, *player);
                        anim.flying += 1;
                    }
                }
            }
        }
        GameEvent::MarketRefilled { level, card } => {
            // 注：此处用"最末槽"近似定位补牌槽位——markets refill 是 push 到 visible 尾部，
            // 故 visible 最后一张即新补卡。动画可能落在非实际空槽（视觉瑕疵），但数据正确
            // （下一帧 refresh 不会重建市场槽，故仅发牌动画的落点可能不准；可接受）。
            if card.is_some() {
                if let Some(card_obj) = model.0.market.visible(*level).last() {
                    if let Some((slot_entity, _)) = card_slots
                        .iter()
                        .find(|(_, s)| s.level == *level && model.0.market.visible(*level).len() == s.slot + 1)
                    {
                        commands.entity(slot_entity).with_children(|p| {
                            let mut e = p.spawn((
                                Node {
                                    width: percent(100),
                                    height: percent(100),
                                    ..default()
                                },
                                UiTransform::default(),
                                DealAnimation {
                                    timer: Timer::from_seconds(0.34, TimerMode::Once),
                                },
                            ));
                            let _ = e;
                            spawn_card_button_inner(p, *card_obj, *level);
                        });
                        anim.dealing += 1;
                    }
                }
            }
        }
        GameEvent::NobleVisited { player, noble } => {
            ***status = format!("Player {} was visited by noble #{}.", player + 1, noble);
            spawn_fly_noble(&mut commands, *root, dir, *player);
            anim.flying += 1;
        }
        GameEvent::EndGameTriggered { player } => {
            ***status = format!("Player {} reached 15! Final round.", player + 1);
        }
        GameEvent::GameOver { winner, standings } => {
            ***status = format!(
                "Game over! Winner: Player {} ({} pts).",
                winner + 1,
                standings.first().map(|(_, s)| *s).unwrap_or(0)
            );
        }
    }
    dirty.0 = true;
}

/// 卡面 spawn（无 Buy/Reserve 按钮，纯视觉，用于发牌动画）。
fn spawn_card_button_inner(parent: &mut ChildSpawnerCommands, card: DevelopmentCard, level: CardLevel) {
    parent
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
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
        ))
        .with_children(|face| {
            face.spawn((
                Text::new(format!("T{} {}P", level.index() + 1, card.prestige)),
                TextFont { font_size: 9.0, ..default() },
                TextColor(GOLD),
            ));
        });
}

fn spawn_fly_coin(
    commands: &mut Commands,
    root: Entity,
    color: GemColor,
    dir: f32,
    offset: f32,
    _player: PlayerId,
    _total: u8,
) {
    commands.entity(root).with_children(|overlay| {
        overlay
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: px(38),
                    height: px(38),
                    left: percent(50),
                    bottom: px(65.0 + offset * 6.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border: UiRect::all(px(2)),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BackgroundColor(gem_color(color)),
                BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.42)),
                UiTransform::default(),
                FlyAnimation {
                    timer: Timer::from_seconds(0.45, TimerMode::Once),
                    target: Vec2::new(dir * 500.0, -175.0),
                },
            ))
            .with_children(|coin| {
                coin.spawn((
                    Text::new(color_short(color)),
                    TextFont { font_size: 10.0, ..default() },
                    TextColor(if matches!(color, GemColor::White) { INK } else { CREAM }),
                ));
            });
    });
}

fn spawn_fly_coin_back(
    commands: &mut Commands,
    root: Entity,
    color: GemColor,
    dir: f32,
    _player: PlayerId,
) {
    commands.entity(root).with_children(|overlay| {
        overlay
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: px(32),
                    height: px(32),
                    left: percent(50),
                    top: percent(50),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border: UiRect::all(px(2)),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BackgroundColor(gem_color(color)),
                BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.42)),
                UiTransform::default(),
                FlyAnimation {
                    timer: Timer::from_seconds(0.45, TimerMode::Once),
                    target: Vec2::new(-dir * 0.0, 175.0),
                },
            ));
    });
}

fn spawn_fly_card(commands: &mut Commands, root: Entity, dir: f32, _player: PlayerId) {
    commands.entity(root).with_children(|overlay| {
        overlay.spawn((
            Node {
                position_type: PositionType::Absolute,
                width: px(60),
                height: px(80),
                left: percent(50),
                top: percent(50),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
            BackgroundColor(PANEL),
            BorderColor::all(GOLD.with_alpha(0.8)),
            UiTransform::default(),
            FlyAnimation {
                timer: Timer::from_seconds(0.5, TimerMode::Once),
                target: Vec2::new(dir * 520.0, 150.0),
            },
        ));
    });
}

fn spawn_fly_noble(commands: &mut Commands, root: Entity, dir: f32, _player: PlayerId) {
    commands.entity(root).with_children(|overlay| {
        overlay.spawn((
            Node {
                position_type: PositionType::Absolute,
                width: px(44),
                height: px(44),
                left: percent(50),
                top: px(70),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(8)),
                ..default()
            },
            BackgroundColor(GOLD.with_alpha(0.3)),
            BorderColor::all(GOLD_BRIGHT),
            UiTransform::default(),
            FlyAnimation {
                timer: Timer::from_seconds(0.6, TimerMode::Once),
                target: Vec2::new(dir * 500.0, 150.0),
            },
        ));
    });
}

fn commit_pending_phase(
    pending_events: Res<PendingEvents>,
    anim: Res<AnimationCounts>,
    phase: Res<BattlePhase>,
    pending_phase: Res<PendingPhase>,
    mut phase_mut: ResMut<BattlePhase>,
    mut pending_phase_mut: ResMut<PendingPhase>,
    mut commands: Commands,
    root: Single<Entity, With<BattleRoot>>,
) {
    if !should_commit_phase(&pending_events, anim.busy(), &phase, &pending_phase) {
        return;
    }
    let new_phase = pending_phase.0.clone().expect("checked non-None");
    *phase_mut = new_phase.clone();
    pending_phase_mut.0 = None;
    // spawn 对应覆盖层（实现见 Task 14；此处仅 GameOver 立即处理占位）
    match new_phase {
        BattlePhase::AwaitDiscard { .. } | BattlePhase::AwaitNobleChoice { .. }
        | BattlePhase::GameOver { .. } => {
            let _ = (&mut commands, &root);
        }
        BattlePhase::Idle => {}
    }
}

fn animate_flights(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut FlyAnimation, &mut UiTransform)>,
    mut anim: ResMut<AnimationCounts>,
    mut dirty: ResMut<UiDirty>,
) {
    for (entity, mut animation, mut transform) in &mut query {
        animation.timer.tick(time.delta());
        let t = animation.timer.fraction();
        let eased = 1.0 - (1.0 - t).powi(3);
        transform.translation = Val2::px(animation.target.x * eased, animation.target.y * eased);
        transform.scale = Vec2::splat(1.0 - eased * 0.72);
        if animation.timer.is_finished() {
            commands.entity(entity).despawn();
            anim.flying = anim.flying.saturating_sub(1);
            dirty.0 = true;
        }
    }
}

fn animate_deals(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut DealAnimation, &mut UiTransform)>,
    mut anim: ResMut<AnimationCounts>,
) {
    for (entity, mut animation, mut transform) in &mut query {
        animation.timer.tick(time.delta());
        let t = animation.timer.fraction();
        let eased = 1.0 - (1.0 - t).powi(3);
        transform.translation = Val2::px(-145.0 * (1.0 - eased), 0.0);
        transform.scale = Vec2::splat(0.74 + eased * 0.26);
        if animation.timer.is_finished() {
            commands.entity(entity).remove::<DealAnimation>();
            *transform = UiTransform::default();
            anim.dealing = anim.dealing.saturating_sub(1);
        }
    }
    let _ = &mut commands;
}

#[allow(clippy::too_many_arguments)]
fn refresh_battle_ui(
    mut commands: Commands,
    model: Res<BattleModel>,
    mut dirty: ResMut<UiDirty>,
    mut turn: ResMut<TurnCount>,
    mut texts: ParamSet<(
        Query<&mut Text, With<TurnText>>,
        Query<(&PlayerScoreText, &mut Text)>,
        Query<(&PlayerColorText, &mut Text)>,
        Query<(&PlayerGoldText, &mut Text)>,
        Query<(&DeckCountText, &mut Text)>,
        Query<(&SupplyCountText, &mut Text)>,
        Query<&mut Text, With<SelectionHudText>>,
        Query<(&PlayerStateText, &mut Text, &mut TextColor)>,
    )>,
    mut panels: Query<(&PlayerPanel, &mut BorderColor)>,
    reserved_rows: Query<(Entity, &ReservedRow)>,
    nobles_rows: Query<(Entity, &NoblesRow)>,
    status: Single<&mut Text, With<StatusText>>,
) {
    if !dirty.0 {
        return;
    }
    dirty.0 = false;

    if let Ok(mut t) = texts.p0().single_mut() {
        **t = format!(
            "TURN {}  /  PLAYER {}{}",
            turn.0 + 1,
            model.0.current_id() + 1,
            if model.0.end_triggered { "  /  FINAL" } else { "" }
        );
    }
    {
        let mut scores = texts.p1();
        for (m, mut text) in &mut scores {
            **text = format!("{} PTS", model.0.player(m.0).score(&model.0.card_store, &model.0.noble_store));
        }
    }
    {
        let mut colors = texts.p2();
        for (m, mut text) in &mut colors {
            let p = model.0.player(m.player);
            let bonus = p.bonus(&model.0.card_store);
            **text = format!("C {} / T {}", bonus.get(card_color_of(m.color)), p.token_count(m.color));
        }
    }
    {
        let mut golds = texts.p3();
        for (m, mut text) in &mut golds {
            **text = format!("T {}", model.0.player(m.0).token_count(GemColor::Gold));
        }
    }
    {
        let mut decks = texts.p4();
        for (m, mut text) in &mut decks {
            let remaining = model.0.decks.remaining(m.0);
            **text = if remaining == 0 { "EMPTY".into() } else { format!("{remaining:02}") };
        }
    }
    {
        let mut supplies = texts.p5();
        for (m, mut text) in &mut supplies {
            **text = format!("x{}", model.0.bank.tokens.get(m.0));
        }
    }
    {
        let _ = status;
    }
    {
        if let Ok(mut hud) = texts.p6().single_mut() {
            // selection HUD count — picker not queried here；由 refresh_selection_hud 独立刷新。
            **hud = "0/3".to_string();
        }
    }
    {
        let mut states = texts.p7();
        for (m, mut text, mut color) in &mut states {
            let active = m.0 == model.0.current_id();
            **text = if active { "ACTIVE".into() } else { "WAITING".into() };
            color.0 = if active { GOLD } else { MUTED };
        }
    }
    for (panel, mut border) in &mut panels {
        *border = BorderColor::all(if panel.0 == model.0.current_id() { GOLD } else { OUTLINE });
    }
    let _ = (&mut commands, &reserved_rows, &nobles_rows, &mut turn);
}

/// GemColor(普通) -> CardColor 反查（用于 bonus.get(CardColor)）。
fn card_color_of(g: GemColor) -> CardColor {
    match g {
        GemColor::White => CardColor::White,
        GemColor::Blue => CardColor::Blue,
        GemColor::Green => CardColor::Green,
        GemColor::Red => CardColor::Red,
        GemColor::Black => CardColor::Black,
        GemColor::Gold => CardColor::White, // 不应发生
    }
}

fn refresh_selection_hud(
    picker: Res<TokenPicker>,
    model: Res<BattleModel>,
    mut hud: Single<&mut Text, With<SelectionHudText>>,
) {
    ***hud = format!("{}/3", picker.selected.len());
    // 高亮已选 supply 按钮（边框）由 update_focus_visuals 统一处理颜色，此处只更新计数。
    let _ = model;
}

fn button_hover_effects(
    mut buttons: Query<(&Interaction, &mut UiTransform), Without<FlyAnimation>>,
) {
    for (interaction, mut transform) in &mut buttons {
        transform.scale = Vec2::splat(match *interaction {
            Interaction::Pressed => 0.98,
            Interaction::Hovered => 1.03,
            Interaction::None => 1.0,
        });
    }
}

fn update_focus_visuals(
    focus: Res<FocusCursor>,
    mut items: Query<(&Focusable, &mut BorderColor), Without<FlyAnimation>>,
) {
    if !focus.is_changed() {
        return;
    }
    for (item, mut border) in &mut items {
        *border = BorderColor::all(if item.zone == focus.zone {
            GOLD_BRIGHT
        } else {
            item.normal_border
        });
    }
}

fn responsive_battle_layout(
    window: Single<&Window, With<PrimaryWindow>>,
    mut ui_scale: ResMut<UiScale>,
) {
    ui_scale.0 = (window.height() / 720.0).clamp(1.0, 1.25);
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

    #[test]
    fn outcome_complete_yields_no_pending() {
        assert!(outcome_to_pending(ActionOutcome::Complete).is_none());
    }

    #[test]
    fn outcome_need_discard_maps() {
        let p = outcome_to_pending(ActionOutcome::NeedDiscardTokens { excess: 2 });
        assert!(matches!(p, Some(BattlePhase::AwaitDiscard { excess: 2 })));
    }

    #[test]
    fn outcome_need_choose_noble_maps() {
        let p = outcome_to_pending(ActionOutcome::NeedChooseNoble { candidates: vec![1, 2] });
        assert!(matches!(p, Some(BattlePhase::AwaitNobleChoice { candidates }) if candidates == vec![1, 2]));
    }

    #[test]
    fn outcome_need_final_discard_then_noble_maps_to_discard() {
        let p = outcome_to_pending(ActionOutcome::NeedFinalDiscardThenChooseNoble {
            excess: 1,
            candidates: vec![3],
        });
        assert!(matches!(p, Some(BattlePhase::AwaitDiscard { excess: 1 })));
    }
}
