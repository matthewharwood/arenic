//! **Display settings** — the resolution mode, its persistence, and the
//! Settings screen reached from the title.
//!
//! One scale `s = design_height / 720` drives the whole game. The window's
//! LOGICAL size is `(1280·s, 720·s)` and [`UiScale`] is `s`, so the 3D arena
//! (its framing is aspect-driven, hence resolution-independent) is unchanged
//! while every `Val::Px` of UI scales together. We never set a scale-factor
//! override, so the monitor's pixel density is honoured natively (Retina-crisp)
//! and re-adapts if the window moves to a different display.

use bevy::prelude::*;
use bevy::ui::UiScale;
use bevy::window::PrimaryWindow;
use serde::{Deserialize, Serialize};

use arenic_game::action_bar::{ActionBarStyle, action_bar_row, spawn_ability_slot};
use arenic_game::default_font::MonoFont;
use arenic_game::theme::ThemeId;

use crate::states::AppState;

/// The base (720) design size; a mode scales it by [`ResolutionMode::scale`].
const BASE_W: f32 = 1280.0;
const BASE_H: f32 = 720.0;

/// The user-selectable display resolution. `Hd` is the historical default
/// (1280×720 logical); `FullHd` scales the whole game 1.5× (1920×1080 logical).
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub(crate) enum ResolutionMode {
    #[default]
    Hd,
    FullHd,
}

impl ResolutionMode {
    /// The UI + window scale this mode applies on top of the 720 base.
    pub(crate) fn scale(self) -> f32 {
        match self {
            ResolutionMode::Hd => 1.0,
            ResolutionMode::FullHd => 1.5,
        }
    }

    /// The window's LOGICAL size in this mode (16:9). Physical pixels follow
    /// the monitor's scale factor automatically.
    pub(crate) fn logical(self) -> Vec2 {
        Vec2::new(BASE_W, BASE_H) * self.scale()
    }

    fn label(self) -> &'static str {
        match self {
            ResolutionMode::Hd => "720p",
            ResolutionMode::FullHd => "1080p",
        }
    }
}

/// Persisted user settings (just the resolution today), stored as RON at the
/// working directory's root (native-only — web has no writable FS and a fixed
/// canvas).
#[derive(Serialize, Deserialize, Default)]
pub(crate) struct Settings {
    pub(crate) resolution: ResolutionMode,
}

#[cfg(not(target_arch = "wasm32"))]
const SETTINGS_PATH: &str = "settings.ron";

impl Settings {
    /// Loads `settings.ron` from the working directory (the workspace root
    /// under `cargo run` / `just author`). Any error — or the web target —
    /// yields the default (720).
    pub(crate) fn load() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        if let Ok(text) = std::fs::read_to_string(SETTINGS_PATH)
            && let Ok(settings) = ron::from_str(&text)
        {
            return settings;
        }
        Self::default()
    }

    /// Writes the settings back to disk (native only; best-effort).
    fn save(&self) {
        #[cfg(not(target_arch = "wasm32"))]
        match ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()) {
            Ok(text) => {
                if let Err(err) = std::fs::write(SETTINGS_PATH, text) {
                    warn!("settings: save failed: {err}");
                }
            }
            Err(err) => warn!("settings: serialize failed: {err}"),
        }
    }
}

/// Tags a resolution-picker button with the mode it selects.
#[derive(Component, Clone, Copy)]
#[component(immutable)]
struct ModeButton(ResolutionMode);

/// The live "Resolution: …" status line on the settings screen.
#[derive(Component)]
#[component(immutable)]
struct CurrentResLabel;

/// Applies the active [`ResolutionMode`] to the live window + UI scale. Runs
/// whenever the mode changes (startup + every Settings toggle) — but the mode
/// only ever changes on the Settings screen, so this never fights the author
/// dock (which owns the window height while in the Intro).
fn apply_resolution(
    mode: Res<ResolutionMode>,
    mut ui_scale: ResMut<UiScale>,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
) {
    let s = mode.scale();
    if ui_scale.0 != s {
        ui_scale.0 = s;
    }
    // Base (no dope-sheet) size; the author dock grows it in the Intro. `set`
    // is LOGICAL — it multiplies by the live scale factor, keeping the window
    // a normal on-screen size and crisp at any DPI.
    let size = mode.logical();
    if (window.resolution.width() - size.x).abs() > 0.5
        || (window.resolution.height() - size.y).abs() > 0.5
    {
        window.resolution.set(size.x, size.y);
    }
}

/// Builds the settings screen: a heading, the live resolution line, the
/// 720p/1080p pickers, and Back — all the LIGHT-theme numbered action boxes the
/// title uses (`ActionBarStyle::menu`), so the corner number on each doubles as
/// its `1`/`2`/`3` keyboard hint (see [`handle_number_keys`]).
fn setup_settings_screen(mut commands: Commands, mono: Res<MonoFont>, mode: Res<ResolutionMode>) {
    // Match the title's deliberate light look (LIGHT palette, not ActiveTheme).
    let palette = ThemeId::Light.palette();

    // Each top screen owns one camera (so the UI never fights a sibling). Clear
    // to the light paper so the whole panel reads on white like the title.
    commands.spawn((
        Camera2d,
        Camera {
            clear_color: ClearColorConfig::Custom(palette.surface_1()),
            ..default()
        },
        DespawnOnExit(AppState::Settings),
    ));

    let root = commands
        .spawn((
            DespawnOnExit(AppState::Settings),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(28.0),
                ..default()
            },
            children![
                (
                    Text::new("Settings"),
                    TextFont {
                        font_size: 64.0,
                        ..default()
                    },
                    TextColor(palette.text_1()),
                ),
                (
                    CurrentResLabel,
                    Text::new(format!("Resolution: {}", mode.label())),
                    // Digit-bearing → DM Mono (CLAUDE.md numeric-text rule).
                    TextFont {
                        font: mono.0.clone(),
                        font_size: 22.0,
                        ..default()
                    },
                    TextColor(palette.text_muted()),
                ),
            ],
        ))
        .id();

    let style = ActionBarStyle::menu(&palette);

    // The two resolution pickers (1, 2), side by side.
    let row = commands.spawn((ChildOf(root), action_bar_row())).id();
    let hd = spawn_ability_slot(
        &mut commands,
        row,
        &palette,
        &mono.0,
        1,
        &style,
        Some("720p"),
    );
    commands
        .entity(hd)
        .insert(ModeButton(ResolutionMode::Hd))
        .observe(pick_mode);
    let fhd = spawn_ability_slot(
        &mut commands,
        row,
        &palette,
        &mono.0,
        2,
        &style,
        Some("1080p"),
    );
    commands
        .entity(fhd)
        .insert(ModeButton(ResolutionMode::FullHd))
        .observe(pick_mode);

    // Back (3) on its own row below the pickers.
    let back = spawn_ability_slot(
        &mut commands,
        root,
        &palette,
        &mono.0,
        3,
        &style,
        Some("Back"),
    );
    commands.entity(back).observe(
        |_: On<Pointer<Click>>, mut next: ResMut<NextState<AppState>>| {
            next.set(AppState::Title);
        },
    );
}

/// Adopts `picked` as the resolution and persists it (no-op if unchanged, so the
/// change-detection-gated [`apply_resolution`] / [`refresh_res_label`] only fire
/// on a real switch). The live window + UI resize follow.
fn select_mode(mode: &mut ResMut<ResolutionMode>, picked: ResolutionMode) {
    if **mode != picked {
        **mode = picked;
        Settings { resolution: picked }.save();
    }
}

/// A resolution picker was clicked: adopt its mode.
fn pick_mode(
    click: On<Pointer<Click>>,
    picks: Query<&ModeButton>,
    mut mode: ResMut<ResolutionMode>,
) {
    let Ok(&ModeButton(picked)) = picks.get(click.entity) else {
        return;
    };
    select_mode(&mut mode, picked);
}

/// `1` / `2` pick 720p / 1080p and `3` goes Back to the title — the numbered
/// hint on each action box. Numpad digits work too.
fn handle_number_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut mode: ResMut<ResolutionMode>,
    mut next: ResMut<NextState<AppState>>,
) {
    if keys.just_pressed(KeyCode::Digit1) || keys.just_pressed(KeyCode::Numpad1) {
        select_mode(&mut mode, ResolutionMode::Hd);
    } else if keys.just_pressed(KeyCode::Digit2) || keys.just_pressed(KeyCode::Numpad2) {
        select_mode(&mut mode, ResolutionMode::FullHd);
    } else if keys.just_pressed(KeyCode::Digit3) || keys.just_pressed(KeyCode::Numpad3) {
        next.set(AppState::Title);
    }
}

/// Keeps the "Resolution: …" line in step with the live mode.
fn refresh_res_label(
    mode: Res<ResolutionMode>,
    mut label: Single<&mut Text, With<CurrentResLabel>>,
) {
    let line = format!("Resolution: {}", mode.label());
    if label.0 != line {
        label.0 = line;
    }
}

/// Display settings: live resolution apply + the Settings screen. Always
/// compiled (this is a shipping feature, not author-only).
pub(crate) struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            apply_resolution.run_if(resource_changed::<ResolutionMode>),
        )
        .add_systems(OnEnter(AppState::Settings), setup_settings_screen)
        .add_systems(
            Update,
            (
                handle_number_keys,
                refresh_res_label.run_if(resource_changed::<ResolutionMode>),
            )
                .run_if(in_state(AppState::Settings)),
        );
    }
}
