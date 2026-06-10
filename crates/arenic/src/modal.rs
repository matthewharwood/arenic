//! Keyboard-first confirmation modals — TUI-style (RULEBOOK → "Modal Controls").
//!
//! One modal at a time: [`spawn_modal`] builds a full-screen overlay (dim scrim,
//! centred card, title + subtitle, a horizontal row of numbered options, and a
//! bracketed hint bar) and inserts the [`ActiveModal`] resource. Interaction
//! follows the Caves-of-Qud conventions: a digit activates its option instantly;
//! `←`/`→`/`Tab` move a visible focus highlight; `Enter`/`Space` confirm the
//! focused option; `Esc` always takes the [`Choice::Cancel`] option (never a
//! destructive one) and is inert when the modal has none. The caller pre-focuses
//! the *safe* option and pauses the affected arena's clock; every choice lands as
//! a [`ModalChoice`] message for `recording::handle_choice` — the one place that
//! closes the modal and applies the transition.

use arenic_game::default_font::MonoFont;
use arenic_game::theme::Theme;
use arenic_game::timeline::ArenaClock;
use bevy::prelude::*;

use crate::states::AppState;

/// Every decision a modal can resolve to, across all recording modals. `Cancel`
/// is the universal safe option (`Esc`); the rest are the state transitions.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Choice {
    /// Start (or restart) a recording for this arena — countdown begins.
    StartRecording,
    /// Fold the cached recording back into the arena and restart it.
    ReplayPrevious,
    /// Commit the draft (full or partial): cache, fold, become a ghost, restart.
    Commit,
    /// Throw the draft away; the character stays selected, the clock resumes.
    Discard,
    /// Throw the draft away, then perform the edge-walk that interrupted it.
    DiscardAndWalk,
    /// Break a ghost out: unfold its events; the arena keeps playing (no restart).
    TakeControl,
    /// Restart the arena at 0:00 — every ghost snaps to its start and stays folded.
    RestartArena,
    /// Close the modal and resume — the safe option every modal carries for `Esc`.
    Cancel,
}

/// A picked modal option, consumed by `recording::handle_choice`.
#[derive(Message)]
pub(crate) struct ModalChoice(pub(crate) Choice);

/// The open modal: its options (left → right), the focused index, and the two
/// backgrounds the focus highlight swaps between. `just_opened` swallows the
/// keypress that opened the modal so it can't also drive it.
#[derive(Resource)]
pub(crate) struct ActiveModal {
    pub(crate) options: Vec<(&'static str, Choice)>,
    pub(crate) focused: usize,
    rest: Color,
    focus: Color,
    just_opened: bool,
}

/// The overlay root (despawned when any choice lands).
#[derive(Component)]
pub(crate) struct ModalRoot;

/// One option button: its index in [`ActiveModal::options`].
#[derive(Component)]
#[component(immutable)]
struct ModalOption(usize);

/// The synchronous one-modal-at-a-time latch. [`ActiveModal`] itself lands via
/// deferred `Commands`, so two systems in one frame could both pass a
/// resource-existence check; this plain resource flips inside [`spawn_modal`]
/// (and back in `recording::handle_choice`), so the second opener in a frame is
/// gated out the moment the first one runs.
#[derive(Resource, Default)]
pub(crate) struct ModalLatch(pub(crate) bool);

/// `true` while no modal is open or opening — gates every world-input system.
pub(crate) fn no_modal(latch: Res<ModalLatch>) -> bool {
    !latch.0
}

/// Spawns the overlay, registers it as the [`ActiveModal`], and pauses `clock`
/// (the affected arena's) — a modal ALWAYS pauses exactly its arena, so the
/// pairing lives here, not in caller convention. `focused` is the index
/// pre-highlighted on open — always the safe option, never a destructive one.
/// Returns `false` (a no-op) if a modal is already open — callers must not
/// commit side effects (e.g. a `PendingWalk`) for a modal that never opened.
#[must_use]
pub(crate) fn spawn_modal(
    commands: &mut Commands,
    latch: &mut ModalLatch,
    clock: &mut ArenaClock,
    theme: &Theme,
    mono: &MonoFont,
    title: &str,
    subtitle: &str,
    options: &[(&'static str, Choice)],
    focused: usize,
) -> bool {
    if latch.0 {
        return false;
    }
    latch.0 = true;
    clock.paused = true;

    let rest = theme.surface_3();
    let (focus, _press) = theme.interactions(rest);
    commands.insert_resource(ActiveModal {
        options: options.to_vec(),
        focused: focused.min(options.len().saturating_sub(1)),
        rest,
        focus,
        just_opened: true,
    });

    // Scrim root: dims the scene, centres the card, sits above the HUD.
    let root = commands
        .spawn((
            ModalRoot,
            DespawnOnExit(AppState::Intro),
            GlobalZIndex(10),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(theme.surface_overlay()),
        ))
        .id();

    let card = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(14.0),
                padding: UiRect::axes(Val::Px(36.0), Val::Px(28.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(theme.surface_2()),
            ChildOf(root),
            children![
                (
                    Text::new(title),
                    TextFont {
                        font_size: 26.0,
                        ..default()
                    },
                    TextColor(theme.text_1()),
                ),
                (
                    Text::new(subtitle),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(theme.text_muted()),
                ),
            ],
        ))
        .id();

    // The horizontal option row: `[n]` hotkey chip + label per button.
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(12.0),
                margin: UiRect::top(Val::Px(8.0)),
                ..default()
            },
            ChildOf(card),
        ))
        .id();
    for (index, &(label, choice)) in options.iter().enumerate() {
        commands
            .spawn((
                Button,
                ModalOption(index),
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(8.0),
                    padding: UiRect::axes(Val::Px(18.0), Val::Px(12.0)),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(rest),
                ChildOf(row),
                children![
                    (
                        Text::new(format!("[{}]", index.strict_add(1))),
                        TextFont {
                            font: mono.0.clone(),
                            font_size: 13.0,
                            ..default()
                        },
                        TextColor(theme.text_muted()),
                    ),
                    (
                        Text::new(label),
                        TextFont {
                            font_size: 15.0,
                            ..default()
                        },
                        TextColor(theme.text_1()),
                    ),
                ],
            ))
            // Hover moves the focus highlight (keyboard and mouse share one focus).
            .observe(
                move |_: On<Pointer<Over>>, mut modal: ResMut<ActiveModal>| {
                    modal.focused = index;
                },
            )
            .observe(
                move |_: On<Pointer<Click>>, mut choices: MessageWriter<ModalChoice>| {
                    choices.write(ModalChoice(choice));
                },
            );
    }

    // Bracketed hint bar, Qud-style — built from the ACTUAL options so it never
    // advertises a key that does nothing. Digits + arrows belong to the modal
    // while it's open (world input is gated off).
    let mut hint = format!(
        "[1-{}] choose   [\u{2190}/\u{2192}] focus   [enter/space] confirm",
        options.len()
    );
    if options.iter().any(|(_, c)| *c == Choice::Cancel) {
        hint.push_str("   [esc] cancel");
    }
    commands.spawn((
        Text::new(hint),
        TextFont {
            font: mono.0.clone(),
            font_size: 12.0,
            ..default()
        },
        TextColor(theme.text_muted()),
        ChildOf(card),
    ));
    true
}

/// Drives the modal from the keyboard: digits activate instantly, `←`/`→`/`Tab`
/// move focus (wrapping), `Enter`/`Space` confirm, `Esc` takes the Cancel option.
fn modal_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut modal: ResMut<ActiveModal>,
    mut choices: MessageWriter<ModalChoice>,
) {
    if modal.just_opened {
        // Swallow the press that opened the modal (e.g. the arrow key that
        // triggered a break-out prompt) so it can't also drive the modal.
        modal.just_opened = false;
        return;
    }
    let count = modal.options.len();

    // Digits (top row + numpad) pick an option instantly. All nine are wired so
    // the `[1-N] choose` hint can never advertise a dead key.
    const DIGITS: [(KeyCode, KeyCode); 9] = [
        (KeyCode::Digit1, KeyCode::Numpad1),
        (KeyCode::Digit2, KeyCode::Numpad2),
        (KeyCode::Digit3, KeyCode::Numpad3),
        (KeyCode::Digit4, KeyCode::Numpad4),
        (KeyCode::Digit5, KeyCode::Numpad5),
        (KeyCode::Digit6, KeyCode::Numpad6),
        (KeyCode::Digit7, KeyCode::Numpad7),
        (KeyCode::Digit8, KeyCode::Numpad8),
        (KeyCode::Digit9, KeyCode::Numpad9),
    ];
    for (index, &(digit, numpad)) in DIGITS.iter().take(count).enumerate() {
        if keys.just_pressed(digit) || keys.just_pressed(numpad) {
            choices.write(ModalChoice(modal.options[index].1));
            return;
        }
    }

    if keys.just_pressed(KeyCode::Enter)
        || keys.just_pressed(KeyCode::NumpadEnter)
        || keys.just_pressed(KeyCode::Space)
    {
        choices.write(ModalChoice(modal.options[modal.focused].1));
        return;
    }
    if keys.just_pressed(KeyCode::Escape)
        && let Some(&(_, cancel)) = modal.options.iter().find(|(_, c)| *c == Choice::Cancel)
    {
        choices.write(ModalChoice(cancel));
        return;
    }

    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let tab = keys.just_pressed(KeyCode::Tab);
    if keys.just_pressed(KeyCode::ArrowRight) || (tab && !shift) {
        modal.focused = modal.focused.strict_add(1) % count;
    }
    if keys.just_pressed(KeyCode::ArrowLeft) || (tab && shift) {
        modal.focused = modal.focused.strict_add(count).strict_sub(1) % count;
    }
}

/// Paints the focus highlight: the focused option wears the hover tint.
fn sync_modal_focus(
    modal: Res<ActiveModal>,
    mut options: Query<(&ModalOption, &mut BackgroundColor)>,
) {
    for (option, mut bg) in &mut options {
        bg.0 = if option.0 == modal.focused {
            modal.focus
        } else {
            modal.rest
        };
    }
}

pub(crate) struct ModalPlugin;

impl Plugin for ModalPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ModalChoice>()
            .init_resource::<ModalLatch>()
            .add_systems(
                Update,
                (
                    modal_keys,
                    // Self-contained condition: survives a refactor that drops
                    // the outer existence guard (resource_changed alone would
                    // fail param validation when the resource is absent).
                    sync_modal_focus.run_if(resource_exists_and_changed::<ActiveModal>),
                )
                    .chain()
                    .run_if(resource_exists::<ActiveModal>)
                    .run_if(in_state(AppState::Intro)),
            );
    }
}
