//! The **title screen**: a white, glass-veiled field of choreographed circles
//! with the sharp "Arenic" title + Start/Settings buttons on top.
//!
//! Three layers, back to front:
//! 1. a white "paper" 3D scene of ~40 dark circles that flow across a grid on
//!    the beat (the baked [`choreo`] score, locked to `arenic_theme.mp3`);
//! 2. the [`glass`] post-process — blur + FBM warp + grain — turning the circles
//!    into ethereal, distorted drop-shadows behind a sheet of glass;
//! 3. the UI (title + buttons), which composites *after* the post-process and
//!    so stays razor-sharp.
//!
//! Colours come from [`arenic_game::theme`]: the white paper + dark ink are the
//! LIGHT palette (the title's deliberate look), while the glass tint borrows the
//! ACTIVE theme's accent, so swapping `ActiveTheme` still tints the mood.

mod choreo;
mod glass;

use arenic_game::InteractionPlugin;
use arenic_game::action_bar::{ActionBarStyle, action_bar_row, spawn_ability_slot};
use arenic_game::default_font::MonoFont;
use arenic_game::theme::{ActiveTheme, ThemeId};
use bevy::camera::ScalingMode;
use bevy::color::LinearRgba;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;

use crate::states::AppState;
use glass::{GlassPlugin, GlassSettings};

/// The title screen plus its glass post-process and circle choreography.
pub struct TitleScreenPlugin;

impl Plugin for TitleScreenPlugin {
    fn build(&self, app: &mut App) {
        // InteractionPlugin drives the themed hover/press/focus + hand cursor on
        // the Start/Settings/Continue/Quit action buttons.
        app.add_plugins((GlassPlugin, InteractionPlugin))
            .add_systems(OnEnter(AppState::Title), setup)
            .add_systems(
                Update,
                (animate_glass, handle_number_keys).run_if(in_state(AppState::Title)),
            );
        choreo::add_playback_systems(app);
    }
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    active: Res<ActiveTheme>,
    mono: Res<MonoFont>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let paper = ThemeId::Light.palette();
    let bg = paper.surface_1(); // near-white
    let ink = paper.text_1(); // near-black title text
    // Circles are a lighter grey than the ink — mixed toward the paper so they
    // read as soft drop-shadows, not hard black dots.
    let circle = mix(ink, bg, 0.55);
    // Glass tint follows ActiveTheme; LINEAR space, since the post-process mixes
    // it into the (linear) sampled scene colour.
    let accent = active.palette().primary.to_linear();

    let (gw, gh) = choreo::grid_dims();

    // One orthographic 3D camera frames the whole grid (vertical extent = grid
    // height; 16:9 viewport → horizontal extent = grid width). It carries the
    // GlassSettings, so the post-process runs ONLY here.
    commands.spawn((
        DespawnOnExit(AppState::Title),
        Camera3d::default(),
        Projection::Orthographic(OrthographicProjection {
            // Show at least the whole grid, whatever the window aspect (the
            // white paper just extends past it on the off-axis).
            scaling_mode: ScalingMode::AutoMin {
                min_width: gw as f32,
                min_height: gh as f32,
            },
            near: -1000.0,
            far: 1000.0,
            ..OrthographicProjection::default_3d()
        }),
        Camera {
            clear_color: ClearColorConfig::Custom(bg),
            ..default()
        },
        // Crisp paper-white vs dark-ink: the LIGHT tokens are already
        // display-referred, so skip tonemapping (which would mute the contrast
        // before the glass blur reads it).
        Tonemapping::None,
        Transform::from_xyz(0.0, 0.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
        GlassSettings {
            // x = blur (texels), y = distort, z = noise scale, w = grain. Distort
            // must be comparable to the blur radius or the refraction is smeared
            // away; these are tuning defaults — adjust to taste.
            params: Vec4::new(9.0, 0.045, 3.0, 0.02),
            anim: Vec4::ZERO,
            tint: Vec4::new(accent.red, accent.green, accent.blue, 0.06),
        },
    ));

    choreo::setup_playback(&mut commands, &mut meshes, circle);

    // UI overlay (sharp, foremost) — themed to the LIGHT palette so dark text +
    // buttons read on the white paper.
    let font = asset_server.load("fonts/Migra-Extrabold.ttf");
    let root = commands
        .spawn((
            DespawnOnExit(AppState::Title),
            GlobalZIndex(10),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(40.0),
                ..default()
            },
            children![(
                Text::new("Arenic"),
                TextFont {
                    font,
                    font_size: 128.0,
                    ..default()
                },
                TextColor(ink),
            )],
        ))
        .id();

    // Start (1) + Settings (2) as numbered action-slot boxes, built from the
    // shared `action_bar` builders (the exact square 56×56 boxes the storybook
    // shows) so the title never drifts from the in-game bar. The corner number
    // each box gets from its slot index doubles as the keyboard hint (see
    // `handle_number_keys`). Light palette + interactive (themed hover/press/focus
    // + hand cursor).
    // Transparent square boxes with just the LIGHT border floating over the
    // glass (the hover/press wash shows through) — the shared title/settings look.
    let style = ActionBarStyle::menu(&paper);
    let row = commands
        .spawn((
            DespawnOnExit(AppState::Title),
            ChildOf(root),
            action_bar_row(),
        ))
        .id();

    let start = spawn_ability_slot(
        &mut commands,
        row,
        &paper,
        &mono.0,
        1,
        &style,
        Some("Start"),
    );
    commands.entity(start).observe(
        |_: On<Pointer<Click>>, mut next: ResMut<NextState<AppState>>| {
            next.set(AppState::Intro);
        },
    );

    let settings = spawn_ability_slot(
        &mut commands,
        row,
        &paper,
        &mono.0,
        2,
        &style,
        Some("Settings"),
    );
    commands.entity(settings).observe(
        |_: On<Pointer<Click>>, mut next: ResMut<NextState<AppState>>| {
            next.set(AppState::Settings);
        },
    );

    // Continue (3) — resume a saved game. Inert for now (no save system yet), so
    // it spawns with no click observer; `handle_number_keys` likewise ignores 3.
    spawn_ability_slot(
        &mut commands,
        row,
        &paper,
        &mono.0,
        3,
        &style,
        Some("Continue"),
    );

    // Quit (4) — exit the app (closes the window).
    let quit = spawn_ability_slot(&mut commands, row, &paper, &mono.0, 4, &style, Some("Quit"));
    commands
        .entity(quit)
        .observe(|_: On<Pointer<Click>>, mut exit: MessageWriter<AppExit>| {
            exit.write(AppExit::Success);
        });
}

/// `1` starts the game, `2` opens settings, `4` quits — the numbered hint on
/// each action-slot box (`3`/Continue is inert until there's a save to resume).
/// Numpad digits work too.
fn handle_number_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut next: ResMut<NextState<AppState>>,
    mut exit: MessageWriter<AppExit>,
) {
    if keys.just_pressed(KeyCode::Digit4) || keys.just_pressed(KeyCode::Numpad4) {
        exit.write(AppExit::Success);
    } else if keys.just_pressed(KeyCode::Digit1) || keys.just_pressed(KeyCode::Numpad1) {
        next.set(AppState::Intro);
    } else if keys.just_pressed(KeyCode::Digit2) || keys.just_pressed(KeyCode::Numpad2) {
        next.set(AppState::Settings);
    }
}

/// Linear-space lerp between two theme colours (the lighter-grey circle ink).
fn mix(a: Color, b: Color, t: f32) -> Color {
    let a = a.to_linear();
    let b = b.to_linear();
    Color::LinearRgba(LinearRgba {
        red: a.red + (b.red - a.red) * t,
        green: a.green + (b.green - a.green) * t,
        blue: a.blue + (b.blue - a.blue) * t,
        alpha: 1.0,
    })
}

/// Drives the glass FBM drift from elapsed time.
fn animate_glass(time: Res<Time>, mut settings: Query<&mut GlassSettings>) {
    for mut setting in &mut settings {
        setting.anim.x = time.elapsed_secs();
    }
}
