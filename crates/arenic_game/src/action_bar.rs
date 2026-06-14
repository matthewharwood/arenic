//! The **action bar** widget — the player's row of ability slots plus the record
//! control that sits across the bottom of the screen in-game.
//!
//! This module owns only the *view*: the component markers, the geometry, and the
//! builder functions that spawn the boxes. The game (`arenic`) drives them with
//! live systems (filling slots from the loadout, animating the record timer); the
//! storybook (`arenic_storybook`) spawns a static showcase from the same builders
//! so the two never drift. Style is fully theme-driven and parameterised via
//! [`ActionBarStyle`], so the same boxes render as the game's subtle bar or the
//! storybook's primary/secondary/accent-bordered showcase.
//!
//! The literal sizes here are 720-base px (the game scales them with `UiScale`
//! for the 1080 mode).

use bevy::color::Alpha;
use bevy::prelude::*;
use bevy::text::Font;

use crate::interaction::{Interactive, hidden_outline};
use crate::theme::{Theme, scale};

/// 720-base geometry. Boxes are 56×56; the radius is supplied by the style.
pub const SLOT: f32 = 56.0;
pub const BORDER: f32 = 1.5;
pub const GAP: f32 = 8.0;
pub const TIMER_W: f32 = 100.0;
pub const TIMER_H: f32 = 44.0;
/// How far the timer slides in from (px), and how fast the reveal eases (1/s).
pub const TIMER_SLIDE: f32 = 14.0;
pub const REVEAL_SPEED: f32 = 7.0;

/// One of the four numbered ability slots, plus the role each text node plays.
#[derive(Component, Clone, Copy)]
#[component(immutable)]
pub struct SlotText {
    pub slot: u8,
    pub role: SlotRole,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SlotRole {
    Name,
    Number,
}

/// The bordered box of an ability slot (re-themed as focus moves).
#[derive(Component)]
#[component(immutable)]
pub struct AbilitySlot;

/// The record button box (its border colour tracks the recording state).
#[derive(Component)]
#[component(immutable)]
pub struct RecordButton;

/// A text node on the record control + which line it is.
#[derive(Component, Clone, Copy)]
#[component(immutable)]
pub enum RecordPart {
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
pub struct RecordTimer;

/// The countdown digits inside the pill (their alpha is the reveal).
#[derive(Component)]
#[component(immutable)]
pub struct TimerInk;

/// 0 → hidden, 1 → fully shown. The game's `animate_timer` eases it.
#[derive(Component)]
pub struct TimerReveal(pub f32);

/// The full appearance of an action bar — radius, per-box border/background, and
/// whether the boxes respond to hover/active/focus. Use [`ActionBarStyle::game`]
/// for the live bar; the storybook builds its own from the knob controls.
#[derive(Clone, Copy)]
pub struct ActionBarStyle {
    /// Corner radius for every box.
    pub radius: f32,
    /// Border colour for ability slots 1–4 (indexed by `slot - 1`).
    pub slot_borders: [Color; 4],
    /// Border colour for the record/stop button.
    pub record_border: Color,
    /// Text colour for the record label (`Record`/`Stop`).
    pub record_label: Color,
    /// Text colour for the record `R` hotkey hint.
    pub record_hotkey: Color,
    /// Resting background for every box (`Color::NONE` = the empty variant).
    pub background: Color,
    /// When set, every box becomes a themed [`Interactive`] button with
    /// hover/active background tints + a focus ring; otherwise boxes ignore
    /// picking (the in-game gutter lets clicks fall through).
    pub interactive: bool,
}

impl ActionBarStyle {
    /// The in-game bar: radius-M boxes with uniform subtle borders, a solid
    /// surface fill, and no interaction (the game's update systems re-theme the
    /// borders/text live; these are the rest state).
    pub fn game(theme: &Theme) -> Self {
        Self {
            radius: scale::radius::M,
            slot_borders: [theme.border_subtle(); 4],
            record_border: theme.border_muted(),
            record_label: theme.text_1(),
            record_hotkey: theme.text_muted(),
            background: theme.surface_3(),
            interactive: false,
        }
    }

    /// The menu style shared by the title + settings screens: transparent square
    /// boxes (radius L) carrying only the theme's subtle border, with the
    /// hover/press wash + focus ring (and hand cursor) of an [`Interactive`]
    /// button. Pass the LIGHT palette for the deliberate light look; the numbered
    /// corner badge each box gets from its slot index doubles as the keyboard hint.
    pub fn menu(theme: &Theme) -> Self {
        Self {
            radius: scale::radius::L,
            slot_borders: [theme.border_subtle(); 4],
            record_border: theme.border_muted(),
            record_label: theme.text_1(),
            record_hotkey: theme.text_muted(),
            background: Color::NONE,
            interactive: true,
        }
    }
}

/// Adds the hover/active/focus behaviour (or, when not interactive, makes the box
/// transparent to picking like the in-game gutter).
fn apply_interaction(commands: &mut Commands, theme: &Theme, id: Entity, style: &ActionBarStyle) {
    if style.interactive {
        let (hover, active) = theme.interactions(style.background);
        commands.entity(id).insert((
            Button,
            Interactive::flat(style.background, hover, active, theme.focus_ring()),
            hidden_outline(),
        ));
    } else {
        commands.entity(id).insert(Pickable::IGNORE);
    }
}

fn mono(font: &Handle<Font>, size: f32) -> TextFont {
    TextFont {
        font: font.clone(),
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

/// The square box shared by every slot + the record control.
fn slot_box(radius: f32) -> Node {
    Node {
        width: Val::Px(SLOT),
        height: Val::Px(SLOT),
        border: UiRect::all(Val::Px(BORDER)),
        border_radius: BorderRadius::all(Val::Px(radius)),
        flex_direction: FlexDirection::Column,
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    }
}

/// The centred row that holds the bar's boxes. Callers own the *outer* container
/// (the game positions it absolutely over the screen; the storybook drops it in a
/// content column), so this is only the inner flow.
pub fn action_bar_row() -> Node {
    Node {
        flex_direction: FlexDirection::Row,
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        column_gap: Val::Px(GAP),
        ..default()
    }
}

/// Spawns one ability slot (a [`SlotText::Name`] + a corner [`SlotText::Number`])
/// as a child of `parent` and returns it. `name` of `Some` fills + shows the slot
/// (the static storybook view); `None` leaves it empty + hidden for the game's
/// `update_slots` to fill from the live loadout.
pub fn spawn_ability_slot(
    commands: &mut Commands,
    parent: Entity,
    theme: &Theme,
    font: &Handle<Font>,
    slot: u8,
    style: &ActionBarStyle,
    name: Option<&str>,
) -> Entity {
    let border = style.slot_borders[(slot as usize - 1) % style.slot_borders.len()];
    let visibility = if name.is_some() {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    let id = commands
        .spawn((
            AbilitySlot,
            ChildOf(parent),
            slot_box(style.radius),
            BackgroundColor(style.background),
            BorderColor::all(border),
        ))
        .with_children(|box_| {
            box_.spawn((
                SlotText {
                    slot,
                    role: SlotRole::Name,
                },
                Text::new(name.unwrap_or("")),
                sans(11.0),
                TextColor(theme.text_1()),
                visibility,
            ));
            box_.spawn((
                SlotText {
                    slot,
                    role: SlotRole::Number,
                },
                Text::new(name.map_or(String::new(), |_| slot.to_string())),
                mono(font, 9.0),
                TextColor(theme.primary),
                corner(),
                visibility,
            ));
        })
        .id();
    apply_interaction(commands, theme, id, style);
    id
}

/// Spawns the record/stop button as a child of `parent` and returns it (the game
/// attaches its click observer to the returned entity). `with_timer` adds the
/// slide-in countdown pill the game animates; the storybook omits it.
pub fn spawn_record_button(
    commands: &mut Commands,
    parent: Entity,
    theme: &Theme,
    font: &Handle<Font>,
    style: &ActionBarStyle,
    label: &str,
    with_timer: bool,
) -> Entity {
    let radius = style.radius;
    let id = commands
        .spawn((
            RecordButton,
            Button,
            ChildOf(parent),
            slot_box(radius),
            BackgroundColor(style.background),
            BorderColor::all(style.record_border),
        ))
        .with_children(|btn| {
            btn.spawn((
                RecordPart::Label,
                Text::new(label),
                sans(12.0),
                TextColor(style.record_label),
            ));
            btn.spawn((
                RecordPart::Hotkey,
                Text::new("R"),
                mono(font, 9.0),
                TextColor(style.record_hotkey),
                corner(),
            ));
            if with_timer {
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
                        border_radius: BorderRadius::all(Val::Px(radius)),
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
                        mono(font, 16.0),
                        TextColor(theme.error.with_alpha(0.0)),
                    ));
                });
            }
        })
        .id();
    // The record button is always a real `Button` (the game observes its clicks),
    // so it never takes `Pickable::IGNORE` — only the hover/active/focus dressing.
    if style.interactive {
        let (hover, active) = theme.interactions(style.background);
        commands.entity(id).insert((
            Interactive::flat(style.background, hover, active, theme.focus_ring()),
            hidden_outline(),
        ));
    }
    id
}
