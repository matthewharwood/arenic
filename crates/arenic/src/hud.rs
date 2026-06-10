//! The game's global **HUD** — themed top / side / bottom chrome overlaying the 3D
//! intro scene.
//!
//! The chrome colours follow whatever is **focused**: zoomed in on an arena, the HUD
//! wears that arena's theme; on the overworld it uses the system default (dark). The
//! HUD spawns no camera of its own — its nodes render on the intro scene's `Camera3d`
//! (the only camera in the Intro state), drawn as UI over the 3D scene.

use arenic_game::arena::Arena;
use arenic_game::default_font::MonoFont;
use arenic_game::theme::{ThemeId, Tint};
use bevy::prelude::*;

use crate::intro_scene::{CurrentArena, ZoomOut};
use crate::states::AppState;

/// What the HUD is themed to: the focused arena, or the overworld default. Set by
/// [`update_focus`], consumed by [`retheme_hud`].
#[derive(Resource, Default, PartialEq)]
enum Focus {
    #[default]
    Overworld,
    Arena(Arena),
}

impl Focus {
    fn theme(&self) -> ThemeId {
        match self {
            Focus::Overworld => overworld_theme(),
            Focus::Arena(arena) => arena.theme(),
        }
    }
    fn title(&self) -> &str {
        match self {
            Focus::Overworld => "Overworld",
            Focus::Arena(arena) => arena.name(),
        }
    }
}

/// The overworld's theme — the "system default". (OS light/dark detection, e.g. the
/// `dark-light` crate, would choose here; we default to Dark for now.)
fn overworld_theme() -> ThemeId {
    ThemeId::Dark
}

/// HUD bar heights (px). The intro camera reads these to lift the arena into the
/// band BETWEEN them (see `intro_scene::UI_VOFFSET_PER_Z`) — kept here so the two
/// can't drift apart.
pub(crate) const TOP_BAR_PX: f32 = 35.0;
pub(crate) const BOTTOM_BAR_PX: f32 = 95.0;

/// Tags a node whose `BackgroundColor` follows a theme token (recoloured on focus).
#[derive(Component)]
#[component(immutable)]
struct ThemedBg(Tint);
/// Tags a node whose `TextColor` follows a theme token.
#[derive(Component)]
#[component(immutable)]
struct ThemedText(Tint);
/// The top-bar title text (its string is the focused arena's name).
#[derive(Component)]
struct HudTitle;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Focus>()
            .add_systems(OnEnter(AppState::Intro), setup_hud)
            .add_systems(
                Update,
                (update_focus, retheme_hud.run_if(resource_changed::<Focus>))
                    .run_if(in_state(AppState::Intro)),
            );
    }
}

/// Tracks the focused arena: zoomed in → that arena's theme + name; zoomed out → the
/// overworld default. Writes [`Focus`] only when it actually changes (so the recolour
/// runs on focus changes, not every frame).
fn update_focus(
    current: Res<CurrentArena>,
    zoomed_out: Query<(), (With<Camera3d>, With<ZoomOut>)>,
    mut focus: ResMut<Focus>,
) {
    let new = if zoomed_out.is_empty() {
        Arena::from_index(current.0).map_or(Focus::Overworld, Focus::Arena)
    } else {
        Focus::Overworld
    };
    focus.set_if_neq(new);
}

/// Builds the HUD: the top / side / bottom chrome, coloured to the current [`Focus`].
fn setup_hud(mut commands: Commands, focus: Res<Focus>, mono: Res<MonoFont>) {
    let t = focus.theme().palette();
    let bg = |tok: Tint| (BackgroundColor(tok(&t)), ThemedBg(tok));
    let txt = |s: &str, size: f32, tok: Tint| {
        (
            Text::new(s),
            TextFont {
                font_size: size,
                ..default()
            },
            TextColor(tok(&t)),
            ThemedText(tok),
        )
    };
    // Digit-bearing labels (hotkey numbers) render in DM Mono so digits align.
    let mono_txt = |s: &str, size: f32, tok: Tint| {
        (
            Text::new(s),
            TextFont {
                font: mono.0.clone(),
                font_size: size,
                ..default()
            },
            TextColor(tok(&t)),
            ThemedText(tok),
        )
    };
    // The side chrome is a thin 22px accent strip (no content yet).
    let strip = || Node {
        width: Val::Px(22.0),
        height: Val::Percent(100.0),
        flex_shrink: 0.0,
        ..default()
    };
    let action = |label: &str| {
        (
            Node {
                padding: UiRect::axes(Val::Px(14.0), Val::Px(7.0)),
                align_items: AlignItems::Center,
                ..default()
            },
            bg(|x| x.surface_3()),
            children![txt(label, 14.0, |x| x.text_1())],
        )
    };

    commands.spawn((
        DespawnOnExit(AppState::Intro),
        // Render on the intro Camera3d (the only camera in this state).
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            ..default()
        },
        children![
            // --- Top bar (35px): focused-arena name + the control hints ---
            (
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(TOP_BAR_PX),
                    flex_shrink: 0.0,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::horizontal(Val::Px(16.0)),
                    ..default()
                },
                bg(|x| x.surface_2()),
                children![
                    (txt(focus.title(), 16.0, |x| x.text_1()), HudTitle),
                    mono_txt(
                        "P zoom   [ ] arena   \u{2190}\u{2191}\u{2193}\u{2192} move   1 Nova   L lava   Esc title",
                        12.0,
                        |x| x.text_muted(),
                    ),
                ],
            ),
            // --- Middle: thin left strip | (3D shows through) | thin right strip ---
            (
                Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Row,
                    ..default()
                },
                children![
                    (strip(), bg(|x| x.surface_2())),
                    (Node { flex_grow: 1.0, ..default() },),
                    (strip(), bg(|x| x.surface_2())),
                ],
            ),
            // --- Bottom bar (95px): action buttons ---
            (
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(BOTTOM_BAR_PX),
                    flex_shrink: 0.0,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    column_gap: Val::Px(12.0),
                    ..default()
                },
                bg(|x| x.surface_2()),
                children![
                    action("Rotate"),
                    action("Roster"),
                    action("Loot"),
                    action("Auction"),
                    action("Craft"),
                ],
            ),
        ],
    ));
}

/// Recolours every themed HUD node + updates the title when [`Focus`] changes.
fn retheme_hud(
    focus: Res<Focus>,
    mut bgs: Query<(&ThemedBg, &mut BackgroundColor)>,
    mut texts: Query<(&ThemedText, &mut TextColor)>,
    mut title: Query<&mut Text, With<HudTitle>>,
) {
    let t = focus.theme().palette();
    for (tok, mut color) in &mut bgs {
        color.0 = (tok.0)(&t);
    }
    for (tok, mut color) in &mut texts {
        color.0 = (tok.0)(&t);
    }
    if let Ok(mut text) = title.single_mut() {
        text.0 = focus.title().to_string();
    }
}
