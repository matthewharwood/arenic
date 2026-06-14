//! The **Hotbar** — the player's action bar across the bottom of the screen.
//!
//! Four numbered ability slots (the `1`-`4` keys) plus a record control (`R`):
//! - A FILLED slot shows the ability's short name (e.g. "Holy") with its slot
//!   number tucked in the corner; an EMPTY slot is just the bordered box (no
//!   name, no number). Slots come straight from [`resolve_ability`], the same
//!   source the casts use, so the bar can never claim a slot the keys don't
//!   fire — today that's slot 1 (Holy) for a hero, the rest empty.
//! - The fifth control is the record button: `Record` while idle, `Stop` once
//!   armed; pressing `R` (or clicking it) runs the exact recording flow. When
//!   recording begins, the countdown timer slides in and ticks the 2-minute
//!   cycle down in `mm:ss:cs`.
//!
//! Everything themes to the focused arena (like the REC strip), and the literal
//! sizes below are 720-base px — `UiScale` scales them for the 1080 mode.

use arenic_game::Boss;
use arenic_game::arena::Arena;
use arenic_game::default_font::MonoFont;
use arenic_game::encounter::{ActiveDifficulty, resolve_ability};
use arenic_game::timeline::{ArenaClock, CYCLE_TICKS, TICKS_PER_SECOND};
use bevy::color::Alpha;
use bevy::prelude::*;

use crate::intro_scene::{CurrentArena, Selected};
use crate::recording::{RecordClicked, RecordingState};
use crate::states::AppState;

/// 720-base geometry (UiScale handles 1080). Buttons are 56×56, radius 12.
const SLOT: f32 = 56.0;
const RADIUS: f32 = 12.0;
const BORDER: f32 = 1.5;
const GAP: f32 = 8.0;
const TIMER_W: f32 = 100.0;
const TIMER_H: f32 = 44.0;
/// How far the timer slides in from (px), and how fast the reveal eases (1/s).
const TIMER_SLIDE: f32 = 14.0;
const REVEAL_SPEED: f32 = 7.0;

/// One of the four numbered ability slots, plus the role each text node plays.
#[derive(Component, Clone, Copy)]
#[component(immutable)]
struct SlotText {
    slot: u8,
    role: SlotRole,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SlotRole {
    Name,
    Number,
}

/// The bordered box of an ability slot (re-themed as focus moves).
#[derive(Component)]
#[component(immutable)]
struct AbilitySlot;

/// The record button box (its border colour tracks the recording state).
#[derive(Component)]
#[component(immutable)]
struct RecordButton;

/// A text node on the record control + which line it is.
#[derive(Component, Clone, Copy)]
#[component(immutable)]
enum RecordPart {
    /// `Record` / `Stop`.
    Label,
    /// The `R` hotkey hint.
    Hotkey,
    /// The `mm:ss:cs` countdown string.
    Timer,
}

/// The countdown pill — slides + fades in when recording starts.
#[derive(Component)]
#[component(immutable)]
struct RecordTimer;

/// The countdown digits inside the pill (their alpha is the reveal).
#[derive(Component)]
#[component(immutable)]
struct TimerInk;

/// 0 → hidden, 1 → fully shown. [`animate_timer`] eases it toward the target.
#[derive(Component)]
struct TimerReveal(f32);

fn mono(mono: &MonoFont, size: f32) -> TextFont {
    TextFont {
        font: mono.0.clone(),
        font_size: size,
        ..default()
    }
}

fn sans(size: f32) -> TextFont {
    TextFont {
        font_size: size,
        ..default()
    }
}

/// A small corner badge node (the slot number / the `R` hotkey), bottom-right.
fn corner() -> Node {
    Node {
        position_type: PositionType::Absolute,
        bottom: Val::Px(4.0),
        right: Val::Px(5.0),
        ..default()
    }
}

/// Builds the bar once per Intro entry; the update systems fill + re-theme it.
fn setup_hotbar(mut commands: Commands, mono_font: Res<MonoFont>, current: Res<CurrentArena>) {
    let theme = Arena::from_index(current.0)
        .expect("invariant: CurrentArena is a valid arena index")
        .theme()
        .palette();

    let root = commands
        .spawn((
            DespawnOnExit(AppState::Intro),
            GlobalZIndex(6),
            // Transparent gutter — only the buttons should catch clicks.
            Pickable::IGNORE,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(11.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                column_gap: Val::Px(GAP),
                ..default()
            },
        ))
        .id();

    // --- Four ability slots ------------------------------------------------
    for slot in 1..=4u8 {
        commands
            .spawn((
                AbilitySlot,
                Pickable::IGNORE,
                ChildOf(root),
                slot_box(),
                BackgroundColor(theme.surface_3()),
                BorderColor::all(theme.border_subtle()),
            ))
            .with_children(|box_| {
                box_.spawn((
                    SlotText {
                        slot,
                        role: SlotRole::Name,
                    },
                    Text::new(""),
                    sans(11.0),
                    TextColor(theme.text_1()),
                    Visibility::Hidden,
                ));
                box_.spawn((
                    SlotText {
                        slot,
                        role: SlotRole::Number,
                    },
                    Text::new(""),
                    mono(&mono_font, 9.0),
                    TextColor(theme.primary),
                    corner(),
                    Visibility::Hidden,
                ));
            });
    }

    // --- Record control + timer -------------------------------------------
    commands
        .spawn((
            RecordButton,
            Button,
            ChildOf(root),
            slot_box(),
            BackgroundColor(theme.surface_3()),
            BorderColor::all(theme.border_muted()),
        ))
        .observe(record_click)
        .with_children(|btn| {
            btn.spawn((
                RecordPart::Label,
                Text::new("Record"),
                sans(12.0),
                TextColor(theme.text_1()),
            ));
            btn.spawn((
                RecordPart::Hotkey,
                Text::new("R"),
                mono(&mono_font, 9.0),
                TextColor(theme.text_muted()),
                corner(),
            ));
            // The pill is parented to the button but absolutely placed to its
            // right, so revealing it never shifts the slot row.
            btn.spawn((
                RecordTimer,
                TimerReveal(0.0),
                Pickable::IGNORE,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(100.0),
                    top: Val::Px((SLOT - TIMER_H) * 0.5),
                    margin: UiRect::left(Val::Px(GAP)),
                    width: Val::Px(TIMER_W),
                    height: Val::Px(TIMER_H),
                    border: UiRect::all(Val::Px(BORDER)),
                    border_radius: BorderRadius::all(Val::Px(RADIUS)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(theme.surface_3().with_alpha(0.0)),
                BorderColor::all(theme.error.with_alpha(0.0)),
            ))
            .with_children(|pill| {
                pill.spawn((
                    RecordPart::Timer,
                    TimerInk,
                    Text::new("02:00:00"),
                    mono(&mono_font, 16.0),
                    TextColor(theme.error.with_alpha(0.0)),
                ));
            });
        });
}

fn slot_box() -> Node {
    Node {
        width: Val::Px(SLOT),
        height: Val::Px(SLOT),
        border: UiRect::all(Val::Px(BORDER)),
        border_radius: BorderRadius::all(Val::Px(RADIUS)),
        flex_direction: FlexDirection::Column,
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    }
}

/// Clicking the record button drives the same flow as the `R` key
/// (`recording::handle_r_key` reads the latch next).
fn record_click(_: On<Pointer<Click>>, mut clicked: ResMut<RecordClicked>) {
    clicked.0 = true;
}

/// Write-if-changed text (an unconditional `text.0 =` re-shapes glyphs every
/// frame for a string that rarely changes).
fn set_text(text: &mut Text, value: &str) {
    if text.0 != value {
        text.0.clear();
        text.0.push_str(value);
    }
}

fn set_color(color: &mut TextColor, value: Color) {
    if color.0 != value {
        color.0 = value;
    }
}

fn set_visible(visibility: &mut Visibility, on: bool) {
    let want = if on {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if *visibility != want {
        *visibility = want;
    }
}

/// Fills each slot from the selected hero's resolved loadout + re-themes it.
/// Cheap enough (≤4 slots, write-if-changed) to run every frame, so it tracks
/// `Tab`/possession/pagination without a change-condition web.
fn update_slots(
    current: Res<CurrentArena>,
    difficulty: Res<ActiveDifficulty>,
    selected: Option<Single<(Has<Boss>, &ChildOf), With<Selected>>>,
    clocks: Query<&ArenaClock>,
    mut texts: Query<(&SlotText, &mut Text, &mut TextColor, &mut Visibility)>,
    mut borders: Query<&mut BorderColor, With<AbilitySlot>>,
) {
    let arena = Arena::from_index(current.0).expect("invariant: CurrentArena is valid");
    let theme = arena.theme().palette();
    // Abilities are bound to the SELECTED character — nothing selected (an empty
    // arena) means every slot is empty, not a defaulted hero loadout.
    let selected_info = selected.map(|sel| {
        let (is_boss, child_of) = *sel;
        (is_boss, clocks.get(child_of.parent()).map_or(0, |c| c.tick))
    });

    for (slot_text, mut text, mut color, mut visibility) in &mut texts {
        let ability = selected_info.and_then(|(is_boss, tick)| {
            resolve_ability(is_boss, arena, difficulty.0, slot_text.slot, tick)
        });
        match slot_text.role {
            SlotRole::Name => {
                set_text(&mut text, ability.map_or("", |a| a.name()));
                set_color(&mut color, theme.text_1());
            }
            SlotRole::Number => {
                set_text(
                    &mut text,
                    &ability.map_or(String::new(), |_| slot_text.slot.to_string()),
                );
                set_color(&mut color, theme.primary);
            }
        }
        set_visible(&mut visibility, ability.is_some());
    }
    let border = theme.border_subtle();
    for mut border_color in &mut borders {
        if border_color.top != border {
            *border_color = BorderColor::all(border);
        }
    }
}

/// Drives the record button's label/colour from [`RecordingState`] and writes
/// the countdown string while recording; [`animate_timer`] owns the reveal.
fn update_record(
    state: Res<RecordingState>,
    current: Res<CurrentArena>,
    selected: Option<Single<&ChildOf, With<Selected>>>,
    clocks: Query<&ArenaClock>,
    mut parts: Query<(&RecordPart, &mut Text, &mut TextColor)>,
    mut border: Single<&mut BorderColor, With<RecordButton>>,
) {
    let theme = Arena::from_index(current.0)
        .expect("invariant: CurrentArena is valid")
        .theme()
        .palette();
    let recording = matches!(*state, RecordingState::Recording);
    let (label, accent) = match *state {
        RecordingState::Idle => ("Record", theme.text_1()),
        RecordingState::Countdown { .. } => ("Stop", theme.warning),
        RecordingState::Recording => ("Stop", theme.error),
    };
    let remaining = if recording {
        let tick = selected
            .and_then(|sel| clocks.get(sel.parent()).ok())
            .map_or(0, |c| c.tick);
        fmt_remaining(CYCLE_TICKS.saturating_sub(tick))
    } else {
        String::new()
    };

    for (part, mut text, mut color) in &mut parts {
        match part {
            RecordPart::Label => {
                set_text(&mut text, label);
                set_color(&mut color, accent);
            }
            RecordPart::Hotkey => {
                set_color(
                    &mut color,
                    if matches!(*state, RecordingState::Idle) {
                        theme.text_muted()
                    } else {
                        accent
                    },
                );
            }
            // Only the STRING here; `animate_timer` owns the timer's colour.
            RecordPart::Timer if recording => set_text(&mut text, &remaining),
            RecordPart::Timer => {}
        }
    }

    let border_color = match *state {
        RecordingState::Idle => theme.border_muted(),
        RecordingState::Countdown { .. } => theme.warning,
        RecordingState::Recording => theme.error,
    };
    if border.top != border_color {
        **border = BorderColor::all(border_color);
    }
}

/// Eases the timer pill in (while recording) / out, applying the reveal to its
/// alpha + a small slide so it "animates in".
fn animate_timer(
    time: Res<Time>,
    state: Res<RecordingState>,
    current: Res<CurrentArena>,
    pill: Single<
        (
            &mut TimerReveal,
            &mut Node,
            &mut BorderColor,
            &mut BackgroundColor,
        ),
        With<RecordTimer>,
    >,
    mut timer_color: Single<&mut TextColor, With<TimerInk>>,
) {
    let theme = Arena::from_index(current.0)
        .expect("invariant: CurrentArena is valid")
        .theme()
        .palette();
    let target = if matches!(*state, RecordingState::Recording) {
        1.0
    } else {
        0.0
    };
    let (mut reveal, mut node, mut border, mut background) = pill.into_inner();
    // Critically-damped-ish lerp toward the target.
    let t = (REVEAL_SPEED * time.delta_secs()).min(1.0);
    reveal.0 += (target - reveal.0) * t;
    let p = reveal.0.clamp(0.0, 1.0);

    node.margin = UiRect::left(Val::Px(GAP + (1.0 - p) * TIMER_SLIDE));
    let edge = theme.error.with_alpha(p);
    if border.top != edge {
        *border = BorderColor::all(edge);
    }
    background.0 = theme.surface_3().with_alpha(p * 0.85);
    timer_color.0 = edge;
}

/// `m m : s s : c c` — the 2-minute cycle COUNTING DOWN (`remaining` ticks).
fn fmt_remaining(remaining: u32) -> String {
    let whole = remaining / TICKS_PER_SECOND;
    let centis = (remaining % TICKS_PER_SECOND) * 100 / TICKS_PER_SECOND;
    format!("{:02}:{:02}:{:02}", whole / 60, whole % 60, centis)
}

/// The bottom action bar: ability slots + the record control.
pub(crate) struct HotbarPlugin;

impl Plugin for HotbarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Intro), setup_hotbar)
            .add_systems(
                Update,
                (update_slots, update_record, animate_timer).run_if(in_state(AppState::Intro)),
            );
    }
}
