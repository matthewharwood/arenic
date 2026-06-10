//! Dope sheet v2 — the KEYFRAME EDITING grammar (`_docs/AUTHORING_UI.md`
//! §2.4-2.7): selection, modal transforms, the snap invariant, ripple, ease
//! ops, and the inline interpolation panel. Everything edits the focused
//! arena's DRAFT [`ArenaStack`] (dirty → `W` publishes), and every key
//! position is tick-quantized — there is no sub-tick anything.
//!
//! Bindings (also in the `?`/`F1` overlay):
//! - click channel row = focus layer · click pill = select key (`Shift` adds)
//! - `Ctrl+K` column at playhead · `Ctrl+A`/`Alt+A` all/none
//! - `I` insert Scale key · `Shift+I` insert Opacity key (at playhead)
//! - `G` move (modal: drag-free, arrows ±1/±15, DIGITS = numeric entry,
//!   `Enter` commit, `Esc` cancel) · `Alt+←/→` nudge ±1 (`+Shift` ±15)
//! - `Shift+D` duplicate into move · `X`/`Del` delete
//! - `Shift+W` ripple (modal: every key at/after the playhead shifts)
//! - `F9` ease-in-out · `Shift+F9` ease-in · `Ctrl+Shift+F9` ease-out ·
//!   `H` hold · `T` cycle ease — the inline panel previews the curve
//! - `J`/`K` jump playhead to previous/next key · `Space` play/pause
//!
//! Deliberately deferred (tracked on the Linear ticket): box/time-range/
//! discontiguous selection, `S` scale, visual paste, slide, strip split,
//! markers/bookmarks, Blender keyframe tags, mouse pill dragging.

use arenic_game::arena::Arena;
use arenic_game::default_font::MonoFont;
use arenic_game::effect::{EaseKind, EffectKey, EffectKind, EffectTrack};
use arenic_game::layer::{ArenaStack, LayerId};
use arenic_game::timeline::{ArenaClock, ArenaTimeline, CYCLE_TICKS, Ghost};
use bevy::color::Alpha;
use bevy::math::curve::{Curve, EasingCurve};
use bevy::prelude::*;

use crate::author::seek_arena;
use crate::intro_scene::CurrentArena;
use crate::modal::no_modal;
use crate::recording::is_idle;
use crate::states::AppState;

use arenic_game::grid::TileMover;

/// One selected keyframe, identified structurally (tick re-targets after
/// moves — the selection follows the keys it moved).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct SelKey {
    pub layer: LayerId,
    pub kind: EffectKind,
    pub tick: u32,
}

/// The sheet's edit focus: a focused layer row + the selected key pills.
#[derive(Resource, Default)]
pub(super) struct SheetSelection {
    pub layer: Option<LayerId>,
    pub keys: Vec<SelKey>,
}

/// A snapshot of the tracks an in-flight modal op may need to restore.
type TrackSnapshot = Vec<(LayerId, EffectKind, Vec<EffectKey>)>;

/// The one in-flight modal transform (Blender grammar: Esc cancels, Enter
/// commits, input refines).
#[derive(Resource, Default)]
enum EditOp {
    #[default]
    Idle,
    /// `G` — move the selection by `delta` ticks (digits = numeric entry).
    Move {
        original: TrackSnapshot,
        base: Vec<SelKey>,
        delta: i64,
        digits: String,
    },
    /// `Shift+W` — ripple: every key at/after `origin` shifts by `delta`.
    Ripple {
        original: TrackSnapshot,
        origin: u32,
        delta: i64,
    },
}

/// Marks a channel row as focusable for `layer`.
#[derive(Component, Clone, Copy)]
#[component(immutable)]
pub(super) struct RowFocus(pub LayerId);

/// Marks a keyframe pill in the key area.
#[derive(Component, Clone, Copy)]
#[component(immutable)]
pub(super) struct KeyPill(pub SelKey);

/// The inline interpolation panel (Rive-style: docked, no mode switch).
#[derive(Component)]
#[component(immutable)]
struct EasePanel;
#[derive(Component)]
#[component(immutable)]
struct EasePanelText;
#[derive(Component)]
#[component(immutable)]
struct EaseCurveBar(usize);

/// The `?` overlay.
#[derive(Component)]
#[component(immutable)]
struct HelpOverlay;

const CURVE_BARS: usize = 24;
const EASE_CYCLE: [EaseKind; 8] = [
    EaseKind::Linear,
    EaseKind::SineInOut,
    EaseKind::CubicInOut,
    EaseKind::ExpoOut,
    EaseKind::BackOut,
    EaseKind::ElasticOut,
    EaseKind::BounceOut,
    EaseKind::Hold,
];

fn track_mut(stack: &mut ArenaStack, layer: LayerId, kind: EffectKind) -> Option<&mut EffectTrack> {
    stack
        .stack
        .layer_mut(layer)?
        .effects
        .iter_mut()
        .find(|track| track.kind == kind)
}

/// Sorts + auto-merges (same tick: LAST wins — the design's collision rule).
fn normalize(track: &mut EffectTrack) {
    track.keys.sort_by_key(|key| key.tick);
    track.keys.reverse();
    track.keys.dedup_by_key(|key| key.tick);
    track.keys.reverse();
}

/// Snapshots every track the selection touches (for Esc-restore).
fn snapshot(stack: &ArenaStack, keys: &[SelKey]) -> TrackSnapshot {
    let mut tracks: TrackSnapshot = Vec::new();
    for key in keys {
        if tracks
            .iter()
            .any(|(layer, kind, _)| *layer == key.layer && *kind == key.kind)
        {
            continue;
        }
        if let Some(layer) = stack.stack.layer(key.layer)
            && let Some(track) = layer.effects.iter().find(|t| t.kind == key.kind)
        {
            tracks.push((key.layer, key.kind, track.keys.clone()));
        }
    }
    tracks
}

/// Every key of `stack` matching the optional layer/tick filters — the
/// imperative collector behind column/all selection and the ripple scope.
fn collect_keys(stack: &ArenaStack, layer: Option<LayerId>, tick: Option<u32>) -> Vec<SelKey> {
    let mut picked = Vec::new();
    for candidate in &stack.stack.layers {
        if layer.is_some_and(|id| candidate.id != id) {
            continue;
        }
        for track in &candidate.effects {
            for key in &track.keys {
                if tick.is_some_and(|t| key.tick != t) {
                    continue;
                }
                picked.push(SelKey {
                    layer: candidate.id,
                    kind: track.kind,
                    tick: key.tick,
                });
            }
        }
    }
    picked
}

fn restore(stack: &mut ArenaStack, original: &TrackSnapshot) {
    for (layer, kind, keys) in original {
        if let Some(track) = track_mut(stack, *layer, *kind) {
            track.keys = keys.clone();
        }
    }
}

/// Applies `delta` to `base` keys against the `original` snapshot — the move
/// preview: original keys minus the moved ones, plus the moved ones at their
/// shifted (tick-clamped) homes.
fn apply_move(stack: &mut ArenaStack, original: &TrackSnapshot, base: &[SelKey], delta: i64) {
    for (layer, kind, keys) in original {
        let moved: Vec<u32> = base
            .iter()
            .filter(|key| key.layer == *layer && key.kind == *kind)
            .map(|key| key.tick)
            .collect();
        let Some(track) = track_mut(stack, *layer, *kind) else {
            continue;
        };
        track.keys = keys
            .iter()
            .map(|key| {
                if moved.contains(&key.tick) {
                    EffectKey {
                        tick: shift_tick(key.tick, delta),
                        ..*key
                    }
                } else {
                    *key
                }
            })
            .collect();
        normalize(track);
    }
}

/// The tick-quantization invariant: every shift lands on a clamped tick.
fn shift_tick(tick: u32, delta: i64) -> u32 {
    (tick as i64)
        .strict_add(delta)
        .clamp(0, (CYCLE_TICKS.strict_sub(1)) as i64) as u32
}

/// Channel-row clicks focus a layer; pill clicks select keys (`Shift` adds).
fn select_on_click(
    keys_in: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<SheetSelection>,
    rows: Query<(&RowFocus, &Interaction), Changed<Interaction>>,
    pills: Query<(&KeyPill, &Interaction), Changed<Interaction>>,
) {
    let shift = keys_in.pressed(KeyCode::ShiftLeft) || keys_in.pressed(KeyCode::ShiftRight);
    for (row, interaction) in &rows {
        if *interaction == Interaction::Pressed {
            selection.layer = Some(row.0);
            if !shift {
                selection.keys.clear();
            }
        }
    }
    for (pill, interaction) in &pills {
        if *interaction != Interaction::Pressed {
            continue;
        }
        selection.layer = Some(pill.0.layer);
        if shift {
            if !selection.keys.contains(&pill.0) {
                selection.keys.push(pill.0);
            }
        } else {
            selection.keys = vec![pill.0];
        }
    }
}

/// The non-modal edit keys: insert, delete, duplicate, nudge, column/all
/// select, ease ops, J/K jumps, Space transport, and modal-op STARTS.
fn edit_keys(
    keys_in: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<SheetSelection>,
    mut op: ResMut<EditOp>,
    current: Res<CurrentArena>,
    mut stacks: Query<(&Arena, &mut ArenaStack)>,
    mut arenas: Query<(Entity, &Arena, &mut ArenaClock, &mut ArenaTimeline)>,
    mut ghosts: Query<(Entity, &Ghost, &ChildOf, &mut TileMover, &mut Transform)>,
) {
    if !matches!(*op, EditOp::Idle) {
        return; // the modal handler owns the keyboard
    }
    let shift = keys_in.pressed(KeyCode::ShiftLeft) || keys_in.pressed(KeyCode::ShiftRight);
    let ctrl = keys_in.pressed(KeyCode::ControlLeft) || keys_in.pressed(KeyCode::ControlRight);
    let alt = keys_in.pressed(KeyCode::AltLeft) || keys_in.pressed(KeyCode::AltRight);
    let Some((_, mut stack)) = stacks
        .iter_mut()
        .find(|(arena, _)| arena.index() == current.0)
    else {
        return;
    };
    let playhead = arenas
        .iter()
        .find(|(_, arena, ..)| arena.index() == current.0)
        .map_or(0, |(_, _, clock, _)| clock.tick);

    // --- Selection ---------------------------------------------------------
    if ctrl && keys_in.just_pressed(KeyCode::KeyK) {
        // Column at the playhead, across every layer's tracks.
        selection.keys = collect_keys(&stack, None, Some(playhead));
        return;
    }
    if ctrl && keys_in.just_pressed(KeyCode::KeyA) {
        selection.keys = collect_keys(&stack, None, None);
        return;
    }
    if alt && keys_in.just_pressed(KeyCode::KeyA) {
        selection.keys.clear();
        return;
    }

    // --- Insert (I = Scale, Shift+I = Opacity) at the playhead --------------
    if keys_in.just_pressed(KeyCode::KeyI) {
        let Some(layer_id) = selection.layer else {
            warn!("focus a layer row first (click its name)");
            return;
        };
        let kind = if shift {
            EffectKind::Opacity
        } else {
            EffectKind::Scale
        };
        if let Some(layer) = stack.stack.layer_mut(layer_id) {
            let track = match layer.effects.iter_mut().find(|t| t.kind == kind) {
                Some(track) => track,
                None => {
                    layer.effects.push(EffectTrack {
                        kind,
                        keys: Vec::new(),
                    });
                    layer
                        .effects
                        .last_mut()
                        .expect("invariant: just pushed a track")
                }
            };
            let value = track.sample(playhead).unwrap_or(1.0);
            track.keys.push(EffectKey {
                tick: playhead,
                value,
                ease: EaseKind::default(),
            });
            normalize(track);
            selection.keys = vec![SelKey {
                layer: layer_id,
                kind,
                tick: playhead,
            }];
        }
        return;
    }

    if selection.keys.is_empty() {
        // Transport works selection-free.
        transport_keys(
            &keys_in,
            &selection,
            &stack,
            &current,
            &mut arenas,
            &mut ghosts,
        );
        return;
    }

    // --- Delete / duplicate / nudge -----------------------------------------
    if keys_in.just_pressed(KeyCode::KeyX) || keys_in.just_pressed(KeyCode::Delete) {
        for key in selection.keys.drain(..) {
            if let Some(track) = track_mut(&mut stack, key.layer, key.kind) {
                track.keys.retain(|k| k.tick != key.tick);
            }
        }
        return;
    }
    if shift && keys_in.just_pressed(KeyCode::KeyD) {
        // Duplicate in place, then drop straight into move-mode.
        let original = snapshot(&stack, &selection.keys);
        for key in &selection.keys {
            if let Some(track) = track_mut(&mut stack, key.layer, key.kind)
                && let Some(found) = track.keys.iter().find(|k| k.tick == key.tick).copied()
            {
                track.keys.push(found);
            }
        }
        *op = EditOp::Move {
            original,
            base: selection.keys.clone(),
            delta: 0,
            digits: String::new(),
        };
        return;
    }
    if alt
        && (keys_in.just_pressed(KeyCode::ArrowLeft) || keys_in.just_pressed(KeyCode::ArrowRight))
    {
        let step = if shift { 15 } else { 1 };
        let delta = if keys_in.just_pressed(KeyCode::ArrowRight) {
            step
        } else {
            -step
        };
        let original = snapshot(&stack, &selection.keys);
        apply_move(&mut stack, &original, &selection.keys.clone(), delta);
        for key in &mut selection.keys {
            key.tick = shift_tick(key.tick, delta);
        }
        return;
    }

    // --- Ease ops ------------------------------------------------------------
    let set_ease = |stack: &mut ArenaStack, ease: EaseKind| {
        for key in &selection.keys {
            if let Some(track) = track_mut(stack, key.layer, key.kind)
                && let Some(found) = track.keys.iter_mut().find(|k| k.tick == key.tick)
            {
                found.ease = ease;
            }
        }
    };
    if keys_in.just_pressed(KeyCode::F9) {
        let ease = if ctrl && shift {
            EaseKind::SineOut
        } else if shift {
            EaseKind::SineIn
        } else {
            EaseKind::SineInOut
        };
        set_ease(&mut stack, ease);
        return;
    }
    if keys_in.just_pressed(KeyCode::KeyH) {
        set_ease(&mut stack, EaseKind::Hold);
        return;
    }
    if keys_in.just_pressed(KeyCode::KeyT) {
        // Cycle the ease palette (the inline panel previews the result).
        let current_ease = selection.keys.first().and_then(|key| {
            stack.stack.layer(key.layer).and_then(|layer| {
                layer
                    .effects
                    .iter()
                    .find(|t| t.kind == key.kind)
                    .and_then(|t| t.keys.iter().find(|k| k.tick == key.tick))
                    .map(|k| k.ease)
            })
        });
        let at = current_ease
            .and_then(|ease| EASE_CYCLE.iter().position(|&e| e == ease))
            .unwrap_or(EASE_CYCLE.len().strict_sub(1));
        set_ease(&mut stack, EASE_CYCLE[at.strict_add(1) % EASE_CYCLE.len()]);
        return;
    }

    // --- Modal starts ---------------------------------------------------------
    if keys_in.just_pressed(KeyCode::KeyG) {
        *op = EditOp::Move {
            original: snapshot(&stack, &selection.keys),
            base: selection.keys.clone(),
            delta: 0,
            digits: String::new(),
        };
        return;
    }
    if shift && keys_in.just_pressed(KeyCode::KeyW) {
        // Ripple: snapshot EVERY track of the focused layer (or all layers).
        let scope: Vec<SelKey> = collect_keys(&stack, selection.layer, None);
        *op = EditOp::Ripple {
            original: snapshot(&stack, &scope),
            origin: playhead,
            delta: 0,
        };
        return;
    }

    transport_keys(
        &keys_in,
        &selection,
        &stack,
        &current,
        &mut arenas,
        &mut ghosts,
    );
}

/// `J`/`K` jump the playhead to the previous/next key (focused layer, else
/// all); `Space` toggles the focused arena's clock.
fn transport_keys(
    keys_in: &ButtonInput<KeyCode>,
    selection: &SheetSelection,
    stack: &ArenaStack,
    current: &CurrentArena,
    arenas: &mut Query<(Entity, &Arena, &mut ArenaClock, &mut ArenaTimeline)>,
    ghosts: &mut Query<(Entity, &Ghost, &ChildOf, &mut TileMover, &mut Transform)>,
) {
    let Some((arena_entity, _, mut clock, mut timeline)) = arenas
        .iter_mut()
        .find(|(_, arena, ..)| arena.index() == current.0)
    else {
        return;
    };
    if keys_in.just_pressed(KeyCode::Space) {
        clock.paused = !clock.paused;
        return;
    }
    let prev = keys_in.just_pressed(KeyCode::KeyJ);
    let next = keys_in.just_pressed(KeyCode::KeyK);
    if !(prev || next) {
        return;
    }
    let mut ticks: Vec<u32> = stack
        .stack
        .layers
        .iter()
        .filter(|layer| selection.layer.is_none_or(|id| layer.id == id))
        .flat_map(|layer| {
            layer
                .effects
                .iter()
                .flat_map(|t| t.keys.iter().map(|k| k.tick))
        })
        .collect();
    ticks.sort_unstable();
    ticks.dedup();
    let target = if next {
        ticks.iter().copied().find(|&t| t > clock.tick)
    } else {
        ticks.iter().rev().copied().find(|&t| t < clock.tick)
    };
    if let Some(target) = target {
        let was_paused = clock.paused;
        seek_arena(arena_entity, target, &mut clock, &mut timeline, ghosts);
        clock.paused = was_paused || true; // jumping parks the playhead
    }
}

/// The in-flight modal op: arrows/digits refine, Enter commits, Esc restores.
fn modal_op(
    keys_in: Res<ButtonInput<KeyCode>>,
    mut op: ResMut<EditOp>,
    mut selection: ResMut<SheetSelection>,
    current: Res<CurrentArena>,
    mut stacks: Query<(&Arena, &mut ArenaStack)>,
) {
    if matches!(*op, EditOp::Idle) {
        return;
    }
    let Some((_, mut stack)) = stacks
        .iter_mut()
        .find(|(arena, _)| arena.index() == current.0)
    else {
        return;
    };
    let shift = keys_in.pressed(KeyCode::ShiftLeft) || keys_in.pressed(KeyCode::ShiftRight);
    let step: i64 = if shift { 15 } else { 1 };
    let mut nudge: i64 = 0;
    if keys_in.just_pressed(KeyCode::ArrowRight) {
        nudge = step;
    }
    if keys_in.just_pressed(KeyCode::ArrowLeft) {
        nudge = -step;
    }
    let digit = keys_in.get_just_pressed().find_map(|key| match key {
        KeyCode::Digit0 => Some('0'),
        KeyCode::Digit1 => Some('1'),
        KeyCode::Digit2 => Some('2'),
        KeyCode::Digit3 => Some('3'),
        KeyCode::Digit4 => Some('4'),
        KeyCode::Digit5 => Some('5'),
        KeyCode::Digit6 => Some('6'),
        KeyCode::Digit7 => Some('7'),
        KeyCode::Digit8 => Some('8'),
        KeyCode::Digit9 => Some('9'),
        KeyCode::Minus => Some('-'),
        _ => None,
    });
    let cancel = keys_in.just_pressed(KeyCode::Escape);
    let commit = keys_in.just_pressed(KeyCode::Enter);

    match &mut *op {
        EditOp::Idle => {}
        EditOp::Move {
            original,
            base,
            delta,
            digits,
        } => {
            if cancel {
                restore(&mut stack, original);
                *op = EditOp::Idle;
                return;
            }
            if let Some(c) = digit {
                // A sign is only legal as the first character.
                if c != '-' || digits.is_empty() {
                    digits.push(c);
                }
                *delta = digits.parse().unwrap_or(*delta);
            }
            *delta = delta.strict_add(nudge);
            apply_move(&mut stack, original, base, *delta);
            if commit {
                let moved: Vec<SelKey> = base
                    .iter()
                    .map(|key| SelKey {
                        tick: shift_tick(key.tick, *delta),
                        ..*key
                    })
                    .collect();
                selection.keys = moved;
                *op = EditOp::Idle;
            }
        }
        EditOp::Ripple {
            original,
            origin,
            delta,
        } => {
            if cancel {
                restore(&mut stack, original);
                *op = EditOp::Idle;
                return;
            }
            if let Some(c) = digit {
                if c.is_ascii_digit() {
                    *delta = delta
                        .strict_mul(10)
                        .strict_add((c as u8 - b'0') as i64 * delta.signum().max(1));
                } else {
                    *delta = delta.strict_mul(-1);
                }
            }
            *delta = delta.strict_add(nudge);
            // Re-derive from the snapshot: keys at/after origin shift.
            for (layer, kind, keys) in original.iter() {
                if let Some(track) = track_mut(&mut stack, *layer, *kind) {
                    track.keys = keys
                        .iter()
                        .map(|key| {
                            if key.tick >= *origin {
                                EffectKey {
                                    tick: shift_tick(key.tick, *delta),
                                    ..*key
                                }
                            } else {
                                *key
                            }
                        })
                        .collect();
                    normalize(track);
                }
            }
            if commit {
                *op = EditOp::Idle;
            }
        }
    }
}

/// The inline interpolation panel: selection summary + a 24-bar curve
/// preview of the FIRST selected key's segment — edits land via T/H/F9
/// without ever leaving the sheet (the anti-graph-editor).
fn update_ease_panel(
    mut commands: Commands,
    mono: Res<MonoFont>,
    selection: Res<SheetSelection>,
    current: Res<CurrentArena>,
    stacks: Query<(&Arena, &ArenaStack)>,
    panel: Option<Single<(Entity, &mut Visibility), With<EasePanel>>>,
    mut text: Query<&mut Text, With<EasePanelText>>,
    mut bars: Query<(&EaseCurveBar, &mut Node)>,
) {
    let theme = Arena::from_index(current.0)
        .expect("invariant: CurrentArena is a valid arena index")
        .theme()
        .palette();
    let Some(panel) = panel else {
        // First run: build the panel skeleton (hidden).
        commands
            .spawn((
                EasePanel,
                DespawnOnExit(AppState::Intro),
                GlobalZIndex(9),
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(10.0),
                    bottom: Val::Px(196.0),
                    width: Val::Px(220.0),
                    height: Val::Px(74.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(theme.surface_1().with_alpha(0.95)),
                Visibility::Hidden,
            ))
            .with_children(|panel| {
                panel.spawn((
                    EasePanelText,
                    Text::new(""),
                    TextFont {
                        font: mono.0.clone(),
                        font_size: 10.0,
                        ..default()
                    },
                    TextColor(theme.text_1()),
                ));
                panel
                    .spawn((Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::FlexEnd,
                        column_gap: Val::Px(1.0),
                        height: Val::Px(34.0),
                        margin: UiRect::top(Val::Px(4.0)),
                        ..default()
                    },))
                    .with_children(|row| {
                        for bar in 0..CURVE_BARS {
                            row.spawn((
                                EaseCurveBar(bar),
                                Node {
                                    width: Val::Px(7.0),
                                    height: Val::Px(2.0),
                                    ..default()
                                },
                                BackgroundColor(theme.primary.with_alpha(0.85)),
                            ));
                        }
                    });
            });
        return;
    };
    let (_, mut visibility) = panel.into_inner();
    let first = selection.keys.first().copied();
    let Some(sel) = first else {
        if *visibility != Visibility::Hidden {
            *visibility = Visibility::Hidden;
        }
        return;
    };
    let ease = stacks
        .iter()
        .find(|(arena, _)| arena.index() == current.0)
        .and_then(|(_, stack)| stack.stack.layer(sel.layer))
        .and_then(|layer| layer.effects.iter().find(|t| t.kind == sel.kind))
        .and_then(|track| track.keys.iter().find(|k| k.tick == sel.tick))
        .map(|key| key.ease);
    let Some(ease) = ease else {
        if *visibility != Visibility::Hidden {
            *visibility = Visibility::Hidden;
        }
        return;
    };
    if *visibility != Visibility::Inherited {
        *visibility = Visibility::Inherited;
    }
    if let Ok(mut text) = text.single_mut() {
        let line = format!(
            "{} key{} · {:?} · T cycle · H hold · F9 ease",
            selection.keys.len(),
            if selection.keys.len() == 1 { "" } else { "s" },
            ease,
        );
        if text.0 != line {
            text.0 = line;
        }
    }
    let curve = EasingCurve::new(0.0f32, 1.0, ease.function());
    for (bar, mut node) in &mut bars {
        let t = bar.0 as f32 / (CURVE_BARS.strict_sub(1)) as f32;
        let value = curve.sample_clamped(t).clamp(-0.35, 1.35);
        let height = 2.0 + (value + 0.35) / 1.7 * 30.0;
        if node.height != Val::Px(height) {
            node.height = Val::Px(height);
        }
    }
}

/// `?` (Shift+/) or `F1` — toggle the bindings overlay.
fn toggle_help(
    mut commands: Commands,
    mono: Res<MonoFont>,
    current: Res<CurrentArena>,
    existing: Option<Single<Entity, With<HelpOverlay>>>,
) {
    if let Some(existing) = existing {
        commands.entity(*existing).despawn();
        return;
    }
    let theme = Arena::from_index(current.0)
        .expect("invariant: CurrentArena is a valid arena index")
        .theme()
        .palette();
    commands.spawn((
        HelpOverlay,
        DespawnOnExit(AppState::Intro),
        GlobalZIndex(20),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(10.0),
            top: Val::Px(45.0),
            padding: UiRect::all(Val::Px(10.0)),
            ..default()
        },
        BackgroundColor(theme.surface_1().with_alpha(0.97)),
        children![(
            Text::new(
                "DOPE SHEET\n\
                 click row = focus · click pill = select (+Shift)\n\
                 Ctrl+K column · Ctrl+A all · Alt+A none\n\
                 I / Shift+I  insert Scale / Opacity key\n\
                 G move (digits = numeric, Enter/Esc)\n\
                 Alt+arrows nudge ±1/±15 · Shift+D dup · X delete\n\
                 Shift+W ripple from playhead\n\
                 F9 family ease · H hold · T cycle ease\n\
                 J/K prev/next key · Space play/pause\n\
                 wheel pan · Ctrl+wheel zoom · = - \\ A · ` collapse\n\
                 E entities · M minion · W publish · F1/? this"
            ),
            TextFont {
                font: mono.0.clone(),
                font_size: 10.0,
                ..default()
            },
            TextColor(theme.text_1()),
        )],
    ));
}

fn help_pressed(keys: Res<ButtonInput<KeyCode>>) -> bool {
    keys.just_pressed(KeyCode::F1)
        || (keys.just_pressed(KeyCode::Slash)
            && (keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight)))
}

fn op_active(op: Res<EditOp>) -> bool {
    !matches!(*op, EditOp::Idle)
}

pub(super) struct EditPlugin;

impl Plugin for EditPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SheetSelection>()
            .init_resource::<EditOp>()
            .add_systems(
                Update,
                (
                    select_on_click,
                    edit_keys.run_if(no_modal.and(is_idle).and(not(op_active))),
                    modal_op.run_if(op_active),
                    update_ease_panel,
                    toggle_help.run_if(help_pressed.and(no_modal)),
                )
                    .run_if(in_state(AppState::Intro)),
            );
    }
}
