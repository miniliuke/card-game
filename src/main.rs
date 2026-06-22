use bevy::{
    input_focus::InputFocus,
    prelude::*,
    render::{
        settings::{Backends, RenderCreation, WgpuSettings},
        RenderPlugin,
    },
    ui::{ColorStop, LinearGradient},
    window::{PrimaryWindow, WindowResolution},
};

mod battle;
mod rules;

#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum AppState {
    #[default]
    Menu,
    Battle,
}

const INK: Color = Color::srgb(0.035, 0.041, 0.059);
const PANEL_LIGHT: Color = Color::srgb(0.105, 0.119, 0.158);
const GOLD: Color = Color::srgb(0.91, 0.68, 0.29);
const GOLD_BRIGHT: Color = Color::srgb(1.0, 0.81, 0.43);
const CREAM: Color = Color::srgb(0.95, 0.91, 0.82);
const MUTED: Color = Color::srgb(0.54, 0.57, 0.64);
const BORDER: Color = Color::srgba(0.91, 0.68, 0.29, 0.28);

fn default_render_backends() -> Backends {
    #[cfg(target_os = "windows")]
    {
        Backends::DX12
    }

    #[cfg(not(target_os = "windows"))]
    {
        Backends::all()
    }
}

fn render_settings() -> WgpuSettings {
    WgpuSettings {
        // Vulkan swapchain acquisition is unstable with some Windows AMD
        // drivers. DX12 is the native, reliable default; WGPU_BACKEND still
        // allows an explicit override for diagnostics and other hardware.
        backends: Some(Backends::from_env().unwrap_or_else(default_render_backends)),
        ..default()
    }
}

fn main() {
    App::new()
        .insert_resource(ClearColor(INK))
        .init_resource::<InputFocus>()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Arcana Table".into(),
                        resolution: WindowResolution::new(1280, 720),
                        resizable: true,
                        ..default()
                    }),
                    ..default()
                })
                .set(RenderPlugin {
                    render_creation: RenderCreation::Automatic(render_settings()),
                    ..default()
                }),
        )
        .init_state::<AppState>()
        .add_plugins(battle::BattlePlugin)
        .add_systems(Startup, |mut commands: Commands| {
            commands.spawn(Camera2d);
        })
        .add_systems(OnEnter(AppState::Menu), setup_menu)
        .add_systems(OnExit(AppState::Menu), cleanup_menu)
        .add_systems(
            Update,
            (menu_interactions, responsive_layout).run_if(in_state(AppState::Menu)),
        )
        .run();
}

#[derive(Component)]
struct MenuScreen;

#[derive(Component)]
struct MainLayout;

#[derive(Component)]
struct ContentPanel;

#[derive(Component)]
struct HeroVisual;

#[derive(Component)]
struct MainTitle;

#[derive(Component)]
struct MenuStatus;

#[derive(Component, Clone, Copy)]
enum MenuAction {
    NewRun,
    Continue,
    Collection,
    Settings,
    Quit,
}

impl MenuAction {
    fn message(self) -> &'static str {
        match self {
            Self::NewRun => "New adventure selected - the road is waiting.",
            Self::Continue => "No saved journey yet. Begin a new adventure.",
            Self::Collection => "Collection selected - deck builder coming next.",
            Self::Settings => "Settings selected - options panel coming next.",
            Self::Quit => "Thanks for visiting the Arcana Table.",
        }
    }
}

#[derive(Component, Clone, Copy)]
enum ButtonStyle {
    Primary,
    Secondary,
    Quiet,
}

fn setup_menu(mut commands: Commands) {
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
                angle: 2.55,
                stops: vec![
                    ColorStop::new(Color::srgb(0.025, 0.03, 0.045), percent(0)),
                    ColorStop::new(Color::srgb(0.075, 0.062, 0.09), percent(55)),
                    ColorStop::new(Color::srgb(0.035, 0.065, 0.075), percent(100)),
                ],
                ..default()
            }),
            MenuScreen,
        ))
        .with_children(|root| {
            spawn_ambient_shapes(root);
            spawn_header(root);

            root.spawn((
                Node {
                    width: percent(100),
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::axes(px(64), px(20)),
                    column_gap: px(40),
                    ..default()
                },
                MainLayout,
            ))
            .with_children(|main| {
                spawn_content(main);
                spawn_card_showcase(main);
            });

            spawn_footer(root);
        });
}

fn cleanup_menu(mut commands: Commands, screen: Single<Entity, With<MenuScreen>>) {
    commands.entity(*screen).despawn();
}

fn spawn_ambient_shapes(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: px(440),
            height: px(440),
            right: px(-170),
            top: px(-220),
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BackgroundColor(Color::srgba(0.22, 0.48, 0.47, 0.08)),
    ));

    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: px(360),
            height: px(360),
            left: px(-210),
            bottom: px(-210),
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BackgroundColor(Color::srgba(0.85, 0.55, 0.24, 0.055)),
    ));
}

fn spawn_header(root: &mut ChildSpawnerCommands) {
    root.spawn(Node {
        width: percent(100),
        height: px(82),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::SpaceBetween,
        padding: UiRect::axes(px(40), px(0)),
        border: UiRect::bottom(px(1)),
        ..default()
    })
    .insert(BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.07)))
    .with_children(|header| {
        header
            .spawn(Node {
                align_items: AlignItems::Center,
                column_gap: px(14),
                ..default()
            })
            .with_children(|brand| {
                brand
                    .spawn((
                        Node {
                            width: px(34),
                            height: px(42),
                            border: UiRect::all(px(1)),
                            border_radius: BorderRadius::all(px(7)),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.91, 0.68, 0.29, 0.10)),
                        BorderColor::all(GOLD),
                        UiTransform::from_rotation(Rot2::degrees(-8.0)),
                    ))
                    .with_children(|card| {
                        card.spawn((
                            Text::new("*"),
                            TextFont {
                                font_size: 18.0,
                                ..default()
                            },
                            TextColor(GOLD_BRIGHT),
                        ));
                    });

                brand.spawn((
                    Text::new("ARCANA TABLE"),
                    TextFont {
                        font_size: 19.0,
                        ..default()
                    },
                    TextColor(CREAM),
                    TextLayout::new_with_justify(Justify::Center),
                ));
            });

        header
            .spawn(Node {
                align_items: AlignItems::Center,
                column_gap: px(12),
                ..default()
            })
            .with_children(|meta| {
                meta.spawn((
                    Node {
                        padding: UiRect::axes(px(12), px(7)),
                        border: UiRect::all(px(1)),
                        border_radius: BorderRadius::MAX,
                        ..default()
                    },
                    BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.10)),
                    BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.035)),
                ))
                .with_children(|chip| {
                    chip.spawn((
                        Text::new("SEASON 01  /  1,240  *"),
                        TextFont {
                            font_size: 12.0,
                            ..default()
                        },
                        TextColor(MUTED),
                    ));
                });
            });
    });
}

fn spawn_content(main: &mut ChildSpawnerCommands) {
    main.spawn((
        Node {
            width: percent(54),
            max_width: px(620),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexStart,
            ..default()
        },
        ContentPanel,
    ))
    .with_children(|content| {
        content.spawn((
            Text::new("A DECK-BUILDING ADVENTURE"),
            TextFont {
                font_size: 12.0,
                ..default()
            },
            TextColor(GOLD),
        ));

        content.spawn((
            Text::new("PLAY THE HAND\nFATE DEALS YOU."),
            TextFont {
                font_size: 56.0,
                ..default()
            },
            TextColor(CREAM),
            TextLayout::new_with_justify(Justify::Left),
            Node {
                margin: UiRect::vertical(px(18)),
                ..default()
            },
            MainTitle,
        ));

        content.spawn((
            Text::new("Build an impossible deck, outwit ancient rivals, and wager everything on one final draw."),
            TextFont {
                font_size: 17.0,
                ..default()
            },
            TextColor(MUTED),
            TextLayout::new(Justify::Left, LineBreak::WordBoundary),
            Node {
                max_width: px(520),
                margin: UiRect::bottom(px(30)),
                ..default()
            },
        ));

        content
            .spawn(Node {
                width: percent(100),
                max_width: px(490),
                flex_direction: FlexDirection::Column,
                row_gap: px(11),
                ..default()
            })
            .with_children(|buttons| {
                spawn_menu_button(buttons, "NEW ADVENTURE", "01", MenuAction::NewRun, ButtonStyle::Primary);
                spawn_menu_button(buttons, "CONTINUE", "02", MenuAction::Continue, ButtonStyle::Secondary);

                buttons
                    .spawn(Node {
                        width: percent(100),
                        column_gap: px(10),
                        ..default()
                    })
                    .with_children(|small| {
                        spawn_menu_button(small, "COLLECTION", "", MenuAction::Collection, ButtonStyle::Quiet);
                        spawn_menu_button(small, "SETTINGS", "", MenuAction::Settings, ButtonStyle::Quiet);
                        spawn_menu_button(small, "QUIT", "", MenuAction::Quit, ButtonStyle::Quiet);
                    });
            });

        content.spawn((
            Text::new("Choose an action to begin."),
            TextFont {
                font_size: 12.0,
                ..default()
            },
            TextColor(Color::srgba(0.95, 0.91, 0.82, 0.48)),
            Node {
                margin: UiRect::top(px(18)),
                ..default()
            },
            MenuStatus,
        ));
    });
}

fn spawn_menu_button(
    parent: &mut ChildSpawnerCommands,
    label: &'static str,
    index: &'static str,
    action: MenuAction,
    style: ButtonStyle,
) {
    let (height, background, border, flex_grow) = match style {
        ButtonStyle::Primary => (58.0, Color::srgba(0.91, 0.68, 0.29, 0.96), GOLD, 0.0),
        ButtonStyle::Secondary => (52.0, PANEL_LIGHT, BORDER, 0.0),
        ButtonStyle::Quiet => (
            42.0,
            Color::srgba(1.0, 1.0, 1.0, 0.035),
            Color::srgba(1.0, 1.0, 1.0, 0.09),
            1.0,
        ),
    };
    let text_color = if matches!(style, ButtonStyle::Primary) {
        INK
    } else {
        CREAM
    };

    parent
        .spawn((
            Button,
            Node {
                width: if flex_grow == 0.0 {
                    percent(100)
                } else {
                    auto()
                },
                height: px(height),
                flex_grow,
                min_width: px(92),
                padding: UiRect::axes(px(18), px(0)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(8)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
            BackgroundColor(background),
            BorderColor::all(border),
            MenuActionTag(action),
            style,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                TextFont {
                    font_size: if matches!(style, ButtonStyle::Quiet) {
                        11.0
                    } else {
                        14.0
                    },
                    ..default()
                },
                TextColor(text_color),
            ));
            if !index.is_empty() {
                button.spawn((
                    Text::new(index),
                    TextFont {
                        font_size: 11.0,
                        ..default()
                    },
                    TextColor(text_color.with_alpha(0.55)),
                ));
            }
        });
}

#[derive(Component, Clone, Copy)]
struct MenuActionTag(MenuAction);

fn spawn_card_showcase(main: &mut ChildSpawnerCommands) {
    main.spawn((
        Node {
            width: percent(42),
            height: percent(100),
            min_height: px(430),
            position_type: PositionType::Relative,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        HeroVisual,
    ))
    .with_children(|stage| {
        spawn_card(
            stage,
            -85.0,
            20.0,
            -13.0,
            "VII",
            "THE\nORACLE",
            Color::srgb(0.17, 0.25, 0.28),
        );
        spawn_card(
            stage,
            84.0,
            24.0,
            13.0,
            "III",
            "THE\nEMBER",
            Color::srgb(0.31, 0.16, 0.15),
        );
        spawn_card(
            stage,
            0.0,
            -5.0,
            0.0,
            "XI",
            "THE\nCROWN",
            Color::srgb(0.12, 0.13, 0.20),
        );
    });
}

fn spawn_card(
    stage: &mut ChildSpawnerCommands,
    x: f32,
    y: f32,
    rotation: f32,
    number: &'static str,
    name: &'static str,
    color: Color,
) {
    stage
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: px(190),
                height: px(280),
                left: percent(50),
                top: percent(50),
                margin: UiRect {
                    left: px(x - 95.0),
                    top: px(y - 140.0),
                    ..default()
                },
                padding: UiRect::all(px(10)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(13)),
                ..default()
            },
            BackgroundColor(color),
            BorderColor::all(GOLD.with_alpha(0.72)),
            UiTransform::from_rotation(Rot2::degrees(rotation)),
            BoxShadow(vec![ShadowStyle {
                color: Color::srgba(0.0, 0.0, 0.0, 0.45),
                x_offset: px(0),
                y_offset: px(20),
                spread_radius: px(0),
                blur_radius: px(32),
            }]),
        ))
        .with_children(|card| {
            card.spawn((
                Node {
                    width: percent(100),
                    height: percent(100),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::all(px(12)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(8)),
                    ..default()
                },
                BorderColor::all(GOLD.with_alpha(0.42)),
                BackgroundGradient::from(LinearGradient {
                    angle: 2.2,
                    stops: vec![
                        ColorStop::new(Color::srgba(1.0, 1.0, 1.0, 0.05), percent(0)),
                        ColorStop::new(Color::srgba(0.0, 0.0, 0.0, 0.10), percent(100)),
                    ],
                    ..default()
                }),
            ))
            .with_children(|inside| {
                inside.spawn((
                    Text::new(number),
                    TextFont {
                        font_size: 11.0,
                        ..default()
                    },
                    TextColor(GOLD),
                ));

                inside
                    .spawn((
                        Node {
                            width: px(92),
                            height: px(92),
                            border: UiRect::all(px(1)),
                            border_radius: BorderRadius::MAX,
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                        BorderColor::all(GOLD.with_alpha(0.5)),
                        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.13)),
                    ))
                    .with_children(|sigil| {
                        sigil.spawn((
                            Text::new("*"),
                            TextFont {
                                font_size: 42.0,
                                ..default()
                            },
                            TextColor(GOLD_BRIGHT),
                            TextShadow {
                                offset: Vec2::new(0.0, 2.0),
                                color: Color::srgba(0.0, 0.0, 0.0, 0.45),
                            },
                        ));
                    });

                inside.spawn((
                    Text::new(name),
                    TextFont {
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(CREAM),
                    TextLayout::new_with_justify(Justify::Center),
                ));
            });
        });
}

fn spawn_footer(root: &mut ChildSpawnerCommands) {
    root.spawn(Node {
        width: percent(100),
        height: px(48),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::SpaceBetween,
        padding: UiRect::axes(px(40), px(0)),
        ..default()
    })
    .with_children(|footer| {
        footer.spawn((
            Text::new("v0.1.0  /  BEVY 0.18"),
            TextFont {
                font_size: 10.0,
                ..default()
            },
            TextColor(Color::srgba(0.54, 0.57, 0.64, 0.65)),
        ));
        footer.spawn((
            Text::new("MOUSE  SELECT   /   ESC  BACK"),
            TextFont {
                font_size: 10.0,
                ..default()
            },
            TextColor(Color::srgba(0.54, 0.57, 0.64, 0.65)),
        ));
    });
}

fn menu_interactions(
    mut input_focus: ResMut<InputFocus>,
    mut next_state: ResMut<NextState<AppState>>,
    mut buttons: Query<
        (
            Entity,
            &Interaction,
            &MenuActionTag,
            &ButtonStyle,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        Changed<Interaction>,
    >,
    mut status: Single<(&mut Text, &mut TextColor), With<MenuStatus>>,
) {
    for (entity, interaction, action, style, mut background, mut border) in &mut buttons {
        let (normal, hovered, pressed) = match style {
            ButtonStyle::Primary => (
                Color::srgba(0.91, 0.68, 0.29, 0.96),
                GOLD_BRIGHT,
                Color::srgb(0.82, 0.57, 0.18),
            ),
            ButtonStyle::Secondary => (
                PANEL_LIGHT,
                Color::srgb(0.14, 0.16, 0.21),
                Color::srgb(0.08, 0.09, 0.12),
            ),
            ButtonStyle::Quiet => (
                Color::srgba(1.0, 1.0, 1.0, 0.035),
                Color::srgba(1.0, 1.0, 1.0, 0.08),
                Color::srgba(0.0, 0.0, 0.0, 0.18),
            ),
        };

        match *interaction {
            Interaction::Pressed => {
                input_focus.set(entity);
                background.0 = pressed;
                *border = BorderColor::all(GOLD_BRIGHT);
                **status.0 = action.0.message().to_string();
                status.1 .0 = GOLD;
                if matches!(action.0, MenuAction::NewRun) {
                    next_state.set(AppState::Battle);
                }
            }
            Interaction::Hovered => {
                input_focus.set(entity);
                background.0 = hovered;
                *border = BorderColor::all(GOLD.with_alpha(0.8));
            }
            Interaction::None => {
                background.0 = normal;
                *border = BorderColor::all(match style {
                    ButtonStyle::Primary => GOLD,
                    ButtonStyle::Secondary => BORDER,
                    ButtonStyle::Quiet => Color::srgba(1.0, 1.0, 1.0, 0.09),
                });
            }
        }
    }
}

fn responsive_layout(
    window: Single<&Window, With<PrimaryWindow>>,
    mut main: Single<&mut Node, (With<MainLayout>, Without<ContentPanel>, Without<HeroVisual>)>,
    mut content: Single<&mut Node, (With<ContentPanel>, Without<MainLayout>, Without<HeroVisual>)>,
    mut visual: Single<&mut Node, (With<HeroVisual>, Without<MainLayout>, Without<ContentPanel>)>,
    mut title: Single<&mut TextFont, With<MainTitle>>,
) {
    let compact = window.width() < 900.0 || window.height() < 620.0;

    if compact {
        main.flex_direction = FlexDirection::Column;
        main.justify_content = JustifyContent::Center;
        main.padding = UiRect::axes(px(34), px(18));
        content.width = percent(100);
        content.max_width = px(650);
        visual.display = Display::None;
        title.font_size = if window.width() < 580.0 { 38.0 } else { 48.0 };
    } else {
        main.flex_direction = FlexDirection::Row;
        main.justify_content = JustifyContent::SpaceBetween;
        main.padding = UiRect::axes(px(64), px(20));
        content.width = percent(54);
        content.max_width = px(620);
        visual.display = Display::Flex;
        title.font_size = 56.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::render::settings::Backends;

    #[test]
    #[cfg(target_os = "windows")]
    fn windows_defaults_to_dx12_rendering() {
        assert_eq!(default_render_backends(), Backends::DX12);
    }
}
