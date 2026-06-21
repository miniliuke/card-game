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

    commands.insert_resource(BattleModel(state));
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
        ));
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
