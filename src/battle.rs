use std::time::{SystemTime, UNIX_EPOCH};

use bevy::{
    prelude::*,
    ui::{ColorStop, LinearGradient, UiScale, Val2},
    window::PrimaryWindow,
};

use crate::{
    AppState,
    game::{Card, GameSession, GemColor, LEVEL_COUNT, SLOTS_PER_LEVEL},
};

const INK: Color = Color::srgb(0.028, 0.033, 0.047);
const PANEL: Color = Color::srgb(0.072, 0.082, 0.108);
const CREAM: Color = Color::srgb(0.95, 0.91, 0.82);
const MUTED: Color = Color::srgb(0.55, 0.58, 0.64);
const GOLD: Color = Color::srgb(0.91, 0.68, 0.29);
const GOLD_BRIGHT: Color = Color::srgb(1.0, 0.82, 0.44);
const OUTLINE: Color = Color::srgba(1.0, 1.0, 1.0, 0.11);
const FOCUSABLE_COUNT: usize = 18;

pub struct BattlePlugin;

impl Plugin for BattlePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActionQueue>()
            .init_resource::<FocusCursor>()
            .init_resource::<AnimationCounts>()
            .init_resource::<UiDirty>()
            .add_systems(OnEnter(AppState::Battle), setup_battle)
            .add_systems(OnExit(AppState::Battle), cleanup_battle)
            .add_systems(
                Update,
                (
                    mouse_actions,
                    keyboard_actions,
                    apply_actions,
                    animate_flights,
                    animate_deals,
                    refresh_battle_ui,
                    update_end_button_state,
                    update_focus_visuals,
                    button_hover_effects,
                    responsive_battle_layout,
                )
                    .chain()
                    .run_if(in_state(AppState::Battle)),
            );
    }
}

#[derive(Resource)]
struct BattleModel(GameSession);

#[derive(Resource, Default)]
struct ActionQueue(Vec<BattleAction>);

#[derive(Resource, Default)]
struct FocusCursor {
    index: usize,
}

#[derive(Resource, Default)]
struct AnimationCounts {
    flying: usize,
    dealing: usize,
    reveal_turn_when_done: bool,
}

impl AnimationCounts {
    fn busy(&self) -> bool {
        self.flying + self.dealing > 0
    }
}

#[derive(Resource)]
struct UiDirty(bool);

impl Default for UiDirty {
    fn default() -> Self {
        Self(true)
    }
}

#[derive(Component)]
struct BattleScreen;

#[derive(Component)]
struct BattleRoot;

#[derive(Component, Clone, Copy)]
struct Focusable {
    index: usize,
    normal_border: Color,
}

#[derive(Component, Clone, Copy)]
enum BattleAction {
    TakeCard { level: usize, slot: usize },
    TakeToken(GemColor),
    EndTurn,
}

#[derive(Component)]
struct CardButton {
    level: usize,
    slot: usize,
}

#[derive(Component)]
struct CardSlot {
    level: usize,
    slot: usize,
}

#[derive(Component)]
struct PlayerPanel(usize);

#[derive(Component)]
struct PlayerScoreText(usize);

#[derive(Component)]
struct PlayerStateText(usize);

#[derive(Component)]
struct PlayerColorText {
    player: usize,
    color: GemColor,
}

#[derive(Component)]
struct DeckCountText(usize);

#[derive(Component)]
struct SupplyCountText(GemColor);

#[derive(Component)]
struct TurnText;

#[derive(Component)]
struct StatusText;

#[derive(Component)]
struct EndTurnButton;

#[derive(Component)]
struct EmptyMarketSlot {
    level: usize,
    slot: usize,
}

#[derive(Component)]
struct FlyAnimation {
    timer: Timer,
    target: Vec2,
}

#[derive(Component)]
struct DealAnimation {
    timer: Timer,
}

fn setup_battle(mut commands: Commands) {
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(1, |duration| duration.as_nanos() as u64);
    let session = GameSession::new(2, seed);
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
                spawn_market(main, &session);
                spawn_player_panel(main, 1);
            });
            spawn_footer(root);
        });
    commands.insert_resource(BattleModel(session));
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
        bar.spawn(Node {
            align_items: AlignItems::Center,
            column_gap: px(13),
            ..default()
        })
        .with_children(|brand| {
            brand
                .spawn((
                    Node {
                        width: px(30),
                        height: px(36),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        border: UiRect::all(px(1)),
                        border_radius: BorderRadius::all(px(6)),
                        ..default()
                    },
                    BorderColor::all(GOLD),
                    BackgroundColor(Color::srgba(0.91, 0.68, 0.29, 0.1)),
                    UiTransform::from_rotation(Rot2::degrees(-7.0)),
                ))
                .with_children(|icon| {
                    icon.spawn((
                        Text::new("*"),
                        TextFont {
                            font_size: 15.0,
                            ..default()
                        },
                        TextColor(GOLD_BRIGHT),
                    ));
                });
            brand.spawn((
                Text::new("ARCANA TABLE  /  MARKET"),
                TextFont {
                    font_size: 15.0,
                    ..default()
                },
                TextColor(CREAM),
            ));
        });
        bar.spawn((
            Text::new("ROUND 01  /  PLAYER 1 TURN"),
            TextFont {
                font_size: 12.0,
                ..default()
            },
            TextColor(GOLD),
            TurnText,
        ));
    });
}

fn spawn_player_panel(parent: &mut ChildSpawnerCommands, player: usize) {
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
                        TextFont {
                            font_size: 15.0,
                            ..default()
                        },
                        TextColor(CREAM),
                    ));
                    header.spawn((
                        Text::new("0 PTS"),
                        TextFont {
                            font_size: 11.0,
                            ..default()
                        },
                        TextColor(GOLD),
                        PlayerScoreText(player),
                    ));
                });
            panel.spawn((
                Text::new(if player == 0 {
                    "ACTIVE PLAYER"
                } else {
                    "WAITING"
                }),
                TextFont {
                    font_size: 9.0,
                    ..default()
                },
                TextColor(if player == 0 { GOLD } else { MUTED }),
                PlayerStateText(player),
            ));
            for color in GemColor::ALL {
                spawn_player_color_row(panel, player, color);
            }
            panel.spawn(Node {
                flex_grow: 1.0,
                ..default()
            });
            panel.spawn((
                Text::new("PERMANENT CARDS / TOKENS"),
                TextFont {
                    font_size: 8.0,
                    ..default()
                },
                TextColor(MUTED.with_alpha(0.7)),
            ));
        });
}

fn spawn_player_color_row(parent: &mut ChildSpawnerCommands, player: usize, color: GemColor) {
    parent
        .spawn((
            Node {
                width: percent(100),
                min_height: px(48),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::axes(px(10), px(7)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(8)),
                ..default()
            },
            BackgroundColor(gem_color(color).with_alpha(0.13)),
            BorderColor::all(gem_color(color).with_alpha(0.38)),
        ))
        .with_children(|row| {
            row.spawn(Node {
                align_items: AlignItems::Center,
                column_gap: px(8),
                ..default()
            })
            .with_children(|label| {
                label.spawn((
                    Node {
                        width: px(18),
                        height: px(18),
                        border_radius: BorderRadius::MAX,
                        border: UiRect::all(px(1)),
                        ..default()
                    },
                    BackgroundColor(gem_color(color)),
                    BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.28)),
                ));
                label.spawn((
                    Text::new(color_name(color)),
                    TextFont {
                        font_size: 10.0,
                        ..default()
                    },
                    TextColor(CREAM),
                ));
            });
            row.spawn((
                Text::new("C 0  /  T 0"),
                TextFont {
                    font_size: 10.0,
                    ..default()
                },
                TextColor(CREAM),
                PlayerColorText { player, color },
            ));
        });
}

fn spawn_market(parent: &mut ChildSpawnerCommands, session: &GameSession) {
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
                        TextFont {
                            font_size: 19.0,
                            ..default()
                        },
                        TextColor(CREAM),
                    ));
                    title.spawn((
                        Text::new("Choose freely / refill at end of turn"),
                        TextFont {
                            font_size: 9.0,
                            ..default()
                        },
                        TextColor(MUTED),
                    ));
                });

            for level in (0..LEVEL_COUNT).rev() {
                spawn_market_row(market, level, session);
            }

            spawn_token_supply(market);
        });
}

fn spawn_market_row(parent: &mut ChildSpawnerCommands, level: usize, session: &GameSession) {
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
            row.spawn((
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
                BorderColor::all(GOLD.with_alpha(0.32 + level as f32 * 0.12)),
            ))
            .with_children(|deck| {
                deck.spawn((
                    Text::new(format!("TIER {}", level + 1)),
                    TextFont {
                        font_size: 9.0,
                        ..default()
                    },
                    TextColor(GOLD),
                ));
                deck.spawn((
                    Text::new("08"),
                    TextFont {
                        font_size: 21.0,
                        ..default()
                    },
                    TextColor(CREAM),
                    DeckCountText(level),
                ));
                deck.spawn((
                    Text::new("DECK"),
                    TextFont {
                        font_size: 8.0,
                        ..default()
                    },
                    TextColor(MUTED),
                ));
            });

            for slot in 0..SLOTS_PER_LEVEL {
                let focus = (LEVEL_COUNT - 1 - level) * SLOTS_PER_LEVEL + slot;
                row.spawn((card_slot_node(), CardSlot { level, slot }))
                    .with_children(|slot_parent| {
                        if let Some(card) = session.visible_card(level, slot) {
                            spawn_card_button(slot_parent, card, level, slot, focus, false);
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
    card: &Card,
    level: usize,
    slot: usize,
    focus: usize,
    dealing: bool,
) {
    let mut entity = parent.spawn((
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
                ColorStop::new(gem_color(card.bonus).with_alpha(0.26), percent(0)),
                ColorStop::new(PANEL, percent(58)),
                ColorStop::new(Color::srgb(0.035, 0.039, 0.055), percent(100)),
            ],
            ..default()
        }),
        BorderColor::all(gem_color(card.bonus).with_alpha(0.68)),
        UiTransform::default(),
        BoxShadow(vec![ShadowStyle {
            color: Color::srgba(0.0, 0.0, 0.0, 0.28),
            x_offset: px(0),
            y_offset: px(7),
            spread_radius: px(0),
            blur_radius: px(13),
        }]),
        CardButton { level, slot },
        BattleAction::TakeCard { level, slot },
        Focusable {
            index: focus,
            normal_border: gem_color(card.bonus).with_alpha(0.68),
        },
    ));
    if dealing {
        entity.insert(DealAnimation {
            timer: Timer::from_seconds(0.34, TimerMode::Once),
        });
    }
    entity.with_children(|face| {
        face.spawn(Node {
            width: percent(100),
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        })
        .with_children(|top| {
            top.spawn((
                Text::new(format!("T{}", card.level)),
                TextFont {
                    font_size: 9.0,
                    ..default()
                },
                TextColor(GOLD),
            ));
            top.spawn((
                Text::new(format!("{} PTS", card.points)),
                TextFont {
                    font_size: 9.0,
                    ..default()
                },
                TextColor(CREAM),
            ));
        });
        face.spawn((
            Text::new(&card.name),
            TextFont {
                font_size: 12.0,
                ..default()
            },
            TextColor(CREAM),
            TextLayout::new_with_justify(Justify::Center),
        ));
        face.spawn(Node {
            width: percent(100),
            justify_content: JustifyContent::Center,
            column_gap: px(3),
            ..default()
        })
        .with_children(|costs| {
            for color in GemColor::ALL {
                let amount = card.costs[color.index()];
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
                        BackgroundColor(gem_color(color).with_alpha(if amount == 0 {
                            0.18
                        } else {
                            0.72
                        })),
                        BorderColor::all(gem_color(color).with_alpha(0.85)),
                    ))
                    .with_children(|dot| {
                        dot.spawn((
                            Text::new(amount.to_string()),
                            TextFont {
                                font_size: 8.0,
                                ..default()
                            },
                            TextColor(if matches!(color, GemColor::White) {
                                INK
                            } else {
                                CREAM
                            }),
                        ));
                    });
            }
        });
    });
}

fn spawn_empty_slot(parent: &mut ChildSpawnerCommands, level: usize, slot: usize) {
    parent
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                min_height: px(126),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(9)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.14)),
            BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.07)),
            EmptyMarketSlot { level, slot },
        ))
        .with_children(|empty| {
            empty.spawn((
                Text::new("DECK EMPTY"),
                TextFont {
                    font_size: 9.0,
                    ..default()
                },
                TextColor(MUTED.with_alpha(0.65)),
            ));
        });
}

fn spawn_token_supply(parent: &mut ChildSpawnerCommands) {
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
                TextFont {
                    font_size: 9.0,
                    ..default()
                },
                TextColor(MUTED),
                Node {
                    width: px(58),
                    ..default()
                },
            ));
            for (offset, color) in GemColor::ALL.into_iter().enumerate() {
                supply
                    .spawn((
                        Button,
                        Node {
                            flex_grow: 1.0,
                            height: px(58),
                            min_width: px(64),
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
                        BattleAction::TakeToken(color),
                        Focusable {
                            index: 12 + offset,
                            normal_border: gem_color(color).with_alpha(0.55),
                        },
                    ))
                    .with_children(|token| {
                        token
                            .spawn((
                                Node {
                                    width: px(32),
                                    height: px(32),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::Center,
                                    border_radius: BorderRadius::MAX,
                                    border: UiRect::all(px(2)),
                                    ..default()
                                },
                                BackgroundColor(gem_color(color)),
                                BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.34)),
                                BoxShadow(vec![ShadowStyle {
                                    color: Color::srgba(0.0, 0.0, 0.0, 0.35),
                                    x_offset: px(0),
                                    y_offset: px(4),
                                    spread_radius: px(0),
                                    blur_radius: px(7),
                                }]),
                            ))
                            .with_children(|coin| {
                                coin.spawn((
                                    Text::new(color_short(color)),
                                    TextFont {
                                        font_size: 9.0,
                                        ..default()
                                    },
                                    TextColor(if matches!(color, GemColor::White) {
                                        INK
                                    } else {
                                        CREAM
                                    }),
                                ));
                            });
                        token.spawn((
                            Text::new("x4"),
                            TextFont {
                                font_size: 12.0,
                                ..default()
                            },
                            TextColor(CREAM),
                            SupplyCountText(color),
                        ));
                    });
            }
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
            Text::new("ARROWS MOVE  /  ENTER SELECT  /  TAB CYCLE  /  E END TURN  /  ESC MENU"),
            TextFont {
                font_size: 9.0,
                ..default()
            },
            TextColor(MUTED),
        ));
        footer.spawn((
            Text::new("Choose a card or currency."),
            TextFont {
                font_size: 10.0,
                ..default()
            },
            TextColor(GOLD),
            StatusText,
        ));
        footer
            .spawn((
                Button,
                Node {
                    width: px(132),
                    height: px(34),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(7)),
                    ..default()
                },
                BackgroundColor(GOLD.with_alpha(0.92)),
                BorderColor::all(GOLD_BRIGHT),
                UiTransform::default(),
                BattleAction::EndTurn,
                Focusable {
                    index: 17,
                    normal_border: GOLD_BRIGHT,
                },
                EndTurnButton,
            ))
            .with_children(|button| {
                button.spawn((
                    Text::new("END TURN  [E]"),
                    TextFont {
                        font_size: 10.0,
                        ..default()
                    },
                    TextColor(INK),
                ));
            });
    });
}

fn mouse_actions(
    mut interactions: Query<(&Interaction, &BattleAction, &Focusable), Changed<Interaction>>,
    mut queue: ResMut<ActionQueue>,
    mut focus: ResMut<FocusCursor>,
) {
    for (interaction, action, focusable) in &mut interactions {
        if !matches!(*interaction, Interaction::None) {
            focus.index = focusable.index;
        }
        if matches!(*interaction, Interaction::Pressed) {
            queue.0.push(*action);
        }
    }
}

fn keyboard_actions(
    keys: Res<ButtonInput<KeyCode>>,
    focusables: Query<(&Focusable, &BattleAction)>,
    mut focus: ResMut<FocusCursor>,
    mut queue: ResMut<ActionQueue>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::Menu);
        return;
    }

    if keys.just_pressed(KeyCode::Tab) {
        let backwards = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
        focus.index = if backwards {
            (focus.index + FOCUSABLE_COUNT - 1) % FOCUSABLE_COUNT
        } else {
            (focus.index + 1) % FOCUSABLE_COUNT
        };
    }

    if focus.index < 12 {
        let row = focus.index / 4;
        let column = focus.index % 4;
        if keys.just_pressed(KeyCode::ArrowLeft) {
            focus.index = row * 4 + column.saturating_sub(1);
        }
        if keys.just_pressed(KeyCode::ArrowRight) {
            focus.index = row * 4 + (column + 1).min(3);
        }
        if keys.just_pressed(KeyCode::ArrowUp) {
            focus.index = row.saturating_sub(1) * 4 + column;
        }
        if keys.just_pressed(KeyCode::ArrowDown) {
            focus.index = (row + 1).min(2) * 4 + column;
        }
    }

    if keys.just_pressed(KeyCode::Enter)
        && let Some((_, action)) = focusables
            .iter()
            .find(|(item, _)| item.index == focus.index)
    {
        queue.0.push(*action);
    }
    if keys.just_pressed(KeyCode::KeyE) {
        queue.0.push(BattleAction::EndTurn);
    }
}

fn apply_actions(
    mut commands: Commands,
    mut queue: ResMut<ActionQueue>,
    mut model: ResMut<BattleModel>,
    mut animations: ResMut<AnimationCounts>,
    root: Single<Entity, With<BattleRoot>>,
    card_buttons: Query<(Entity, &CardButton)>,
    slots: Query<(Entity, &CardSlot)>,
    empty_slots: Query<&EmptyMarketSlot>,
    mut status: Single<&mut Text, With<StatusText>>,
    mut dirty: ResMut<UiDirty>,
) {
    let actions = std::mem::take(&mut queue.0);
    for action in actions {
        match action {
            BattleAction::TakeCard { level, slot } if animations.dealing == 0 => {
                let player = model.0.active_player();
                if model.0.take_card(level, slot).is_ok()
                    && let Some((entity, _)) = card_buttons
                        .iter()
                        .find(|(_, button)| button.level == level && button.slot == slot)
                {
                    commands
                        .entity(entity)
                        .remove::<Button>()
                        .insert(FlyAnimation {
                            timer: Timer::from_seconds(0.45, TimerMode::Once),
                            target: Vec2::new(if player == 0 { -520.0 } else { 520.0 }, 150.0),
                        });
                    animations.flying += 1;
                    ***status = format!("Card claimed by Player {}.", player + 1);
                }
            }
            BattleAction::TakeToken(color) if animations.dealing == 0 => {
                let player = model.0.active_player();
                if model.0.take_token(color).is_ok() {
                    commands.entity(*root).with_children(|overlay| {
                        overlay
                            .spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    width: px(38),
                                    height: px(38),
                                    left: percent(50),
                                    bottom: px(65),
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
                                    target: Vec2::new(
                                        if player == 0 { -500.0 } else { 500.0 },
                                        -175.0,
                                    ),
                                },
                            ))
                            .with_children(|coin| {
                                coin.spawn((
                                    Text::new(color_short(color)),
                                    TextFont {
                                        font_size: 10.0,
                                        ..default()
                                    },
                                    TextColor(if matches!(color, GemColor::White) {
                                        INK
                                    } else {
                                        CREAM
                                    }),
                                ));
                            });
                    });
                    animations.flying += 1;
                    ***status = format!("{} currency claimed.", color_name(color));
                } else {
                    ***status = format!("No {} currency remains.", color_name(color));
                }
            }
            BattleAction::EndTurn if !animations.busy() => {
                let dealt = model.0.end_turn();
                for replacement in &dealt {
                    if let Some((slot_entity, _)) = slots.iter().find(|(_, target)| {
                        target.level == replacement.level && target.slot == replacement.slot
                    }) {
                        let focus = (LEVEL_COUNT - 1 - replacement.level) * SLOTS_PER_LEVEL
                            + replacement.slot;
                        commands.entity(slot_entity).with_children(|parent| {
                            spawn_card_button(
                                parent,
                                &replacement.card,
                                replacement.level,
                                replacement.slot,
                                focus,
                                true,
                            );
                        });
                    }
                }
                animations.dealing += dealt.len();
                animations.reveal_turn_when_done = !dealt.is_empty();
                if dealt.is_empty() {
                    dirty.0 = true;
                    ***status = format!("Player {} turn.", model.0.active_player() + 1);
                } else {
                    ***status = "Dealing new cards...".to_string();
                }

                for (slot_entity, slot_data) in &slots {
                    let placeholder_exists = empty_slots.iter().any(|empty| {
                        empty.level == slot_data.level && empty.slot == slot_data.slot
                    });
                    if model
                        .0
                        .visible_card(slot_data.level, slot_data.slot)
                        .is_none()
                        && !placeholder_exists
                    {
                        commands.entity(slot_entity).with_children(|parent| {
                            spawn_empty_slot(parent, slot_data.level, slot_data.slot);
                        });
                    }
                }
            }
            _ => {}
        }
    }
}

fn animate_flights(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut FlyAnimation, &mut UiTransform)>,
    mut animations: ResMut<AnimationCounts>,
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
            animations.flying = animations.flying.saturating_sub(1);
            dirty.0 = true;
        }
    }
}

fn animate_deals(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut DealAnimation, &mut UiTransform)>,
    mut animations: ResMut<AnimationCounts>,
    mut dirty: ResMut<UiDirty>,
    mut status: Single<&mut Text, With<StatusText>>,
    model: Res<BattleModel>,
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
            animations.dealing = animations.dealing.saturating_sub(1);
        }
    }
    if animations.dealing == 0 && animations.reveal_turn_when_done {
        animations.reveal_turn_when_done = false;
        dirty.0 = true;
        ***status = format!("Player {} turn.", model.0.active_player() + 1);
    }
}

#[allow(clippy::too_many_arguments)]
fn refresh_battle_ui(
    model: Res<BattleModel>,
    mut dirty: ResMut<UiDirty>,
    mut texts: ParamSet<(
        Query<&mut Text, With<TurnText>>,
        Query<(&PlayerScoreText, &mut Text)>,
        Query<(&PlayerColorText, &mut Text)>,
        Query<(&DeckCountText, &mut Text)>,
        Query<(&SupplyCountText, &mut Text)>,
        Query<(&PlayerStateText, &mut Text, &mut TextColor)>,
    )>,
    mut panels: Query<(&PlayerPanel, &mut BorderColor)>,
) {
    if !dirty.0 {
        return;
    }
    dirty.0 = false;
    if let Ok(mut turn) = texts.p0().single_mut() {
        **turn = format!(
            "ROUND {:02}  /  PLAYER {} TURN",
            model.0.round(),
            model.0.active_player() + 1
        );
    }
    {
        let mut scores = texts.p1();
        for (marker, mut text) in &mut scores {
            **text = format!("{} PTS", model.0.player(marker.0).score());
        }
    }
    {
        let mut colors = texts.p2();
        for (marker, mut text) in &mut colors {
            let player = model.0.player(marker.player);
            **text = format!(
                "C {}  /  T {}",
                player.card_count(marker.color),
                player.token_count(marker.color)
            );
        }
    }
    {
        let mut decks = texts.p3();
        for (marker, mut text) in &mut decks {
            let remaining = model.0.deck_remaining(marker.0);
            **text = if remaining == 0 {
                "EMPTY".into()
            } else {
                format!("{remaining:02}")
            };
        }
    }
    {
        let mut supplies = texts.p4();
        for (marker, mut text) in &mut supplies {
            **text = format!("x{}", model.0.token_supply(marker.0));
        }
    }
    {
        let mut states = texts.p5();
        for (marker, mut text, mut color) in &mut states {
            let active = marker.0 == model.0.active_player();
            **text = if active {
                "ACTIVE PLAYER".into()
            } else {
                "WAITING".into()
            };
            color.0 = if active { GOLD } else { MUTED };
        }
    }
    for (panel, mut border) in &mut panels {
        *border = BorderColor::all(if panel.0 == model.0.active_player() {
            GOLD
        } else {
            OUTLINE
        });
    }
}

fn update_end_button_state(
    animations: Res<AnimationCounts>,
    mut end_button: Single<(&mut BackgroundColor, &mut BorderColor), With<EndTurnButton>>,
) {
    end_button.0.0 = if animations.busy() {
        GOLD.with_alpha(0.28)
    } else {
        GOLD.with_alpha(0.92)
    };
    *end_button.1 = BorderColor::all(if animations.busy() {
        OUTLINE
    } else {
        GOLD_BRIGHT
    });
}

fn update_focus_visuals(
    focus: Res<FocusCursor>,
    mut items: Query<(&Focusable, &mut BorderColor), Without<FlyAnimation>>,
) {
    if !focus.is_changed() {
        return;
    }
    for (item, mut border) in &mut items {
        *border = BorderColor::all(if item.index == focus.index {
            GOLD_BRIGHT
        } else {
            item.normal_border
        });
    }
}

fn button_hover_effects(
    mut buttons: Query<
        (&Interaction, &mut UiTransform),
        (
            Changed<Interaction>,
            Without<FlyAnimation>,
            Without<DealAnimation>,
        ),
    >,
) {
    for (interaction, mut transform) in &mut buttons {
        transform.scale = Vec2::splat(match *interaction {
            Interaction::Pressed => 0.98,
            Interaction::Hovered => 1.03,
            Interaction::None => 1.0,
        });
    }
}

fn responsive_battle_layout(
    window: Single<&Window, With<PrimaryWindow>>,
    mut ui_scale: ResMut<UiScale>,
) {
    ui_scale.0 = (window.height() / 720.0).clamp(1.0, 1.25);
}

fn cleanup_battle(
    mut commands: Commands,
    screen: Single<Entity, With<BattleScreen>>,
    mut ui_scale: ResMut<UiScale>,
) {
    commands.entity(*screen).despawn();
    commands.remove_resource::<BattleModel>();
    ui_scale.0 = 1.0;
}

fn color_name(color: GemColor) -> &'static str {
    match color {
        GemColor::White => "WHITE",
        GemColor::Blue => "BLUE",
        GemColor::Green => "GREEN",
        GemColor::Red => "RED",
        GemColor::Black => "BLACK",
    }
}

fn color_short(color: GemColor) -> &'static str {
    match color {
        GemColor::White => "W",
        GemColor::Blue => "U",
        GemColor::Green => "G",
        GemColor::Red => "R",
        GemColor::Black => "B",
    }
}

fn gem_color(color: GemColor) -> Color {
    match color {
        GemColor::White => Color::srgb(0.88, 0.86, 0.78),
        GemColor::Blue => Color::srgb(0.20, 0.47, 0.78),
        GemColor::Green => Color::srgb(0.22, 0.61, 0.43),
        GemColor::Red => Color::srgb(0.78, 0.25, 0.24),
        GemColor::Black => Color::srgb(0.12, 0.13, 0.17),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_slot_size_does_not_depend_on_its_contents() {
        let slot = card_slot_node();

        assert_eq!(slot.flex_basis, percent(0));
    }
}
