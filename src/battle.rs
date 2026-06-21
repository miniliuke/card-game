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

fn setup_battle(_commands: Commands) {
    // 占位；后续 Task 填充
}

fn cleanup_battle(
    mut commands: Commands,
    screen: Single<Entity, With<BattleScreen>>,
    mut ui_scale: ResMut<UiScale>,
) {
    let _ = screen;
    let _ = ui_scale;
    commands.spawn_empty(); // 占位避免未用警告
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
}
