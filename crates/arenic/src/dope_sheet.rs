//! The **dope sheet** — the layer panel of the authoring suite
//! (`_docs/AUTHORING_UI.md` §2, v1: structure). After-Effects-shaped, built to
//! exceed it: a bottom panel showing the focused arena's [`ArenaStack`] as
//! rows (top of stack first — the row that wins conflicts is the row you see
//! first), an adaptive ruler over the 7,200-tick cycle, a 1-px playhead with
//! click/drag scrubbing, and zoom/pan.
//!
//! - **Rows**: kind glyph + name + `M`/`S`/`L` (mute/solo/lock, Blender-NLA
//!   semantics) + a dirty `●`; recording layers render their take as a dimmed
//!   span, tile layers a segment per keyframe range.
//! - **Ruler**: unit ladder tick → ¼s → 1s → 5s → 15s → 1min; the largest unit
//!   spaced ≥ ~80 px gets `m:ss` labels (DM Mono), one rung below renders as
//!   minor ticks — no aliasing at any zoom. Boss-phase boundaries mark a thin
//!   accent lane.
//! - **Scrub**: press/drag in the ruler parks the arena at the pointer's tick
//!   via the same [`seek_arena`] the `,`/`.` keys use (the clock pauses for
//!   the drag and resumes after).
//! - **View**: wheel = pan · `Ctrl`+wheel = zoom about the cursor's tick ·
//!   `=`/`-` = zoom at playhead · `\` = toggle last-zoom ↔ full cycle ·
//!   `A` = frame all · `` ` `` = collapse the panel.
//!
//! Keyframe pills, selection, and editing land in v2 (ARE-42); this file owns
//! the structure they mount into. Author-feature only; nothing here ships.

use arenic_game::Boss;
use arenic_game::arena::Arena;
use arenic_game::default_font::MonoFont;
use arenic_game::encounter::PHASE_TICKS;
use arenic_game::grid::TileMover;
use arenic_game::layer::{ArenaStack, LayerId, LayerKind};
use arenic_game::theme::Theme;
use arenic_game::tile_script::TileScript;
use arenic_game::timeline::{
    ArenaClock, ArenaTimeline, CYCLE_TICKS, Ghost, RecordingLibrary, TICKS_PER_SECOND,
};
use bevy::input::common_conditions::input_just_pressed;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use bevy::color::Alpha;

use crate::author::{seek_arena, stacks_changed};
use crate::intro_scene::{CurrentArena, Selected};
use crate::modal::no_modal;
use crate::recording::fmt_tick;
use crate::score_sync::apply_stack_preview;
use crate::states::AppState;

/// Panel geometry — the window is pinned to 1280×720 physical px
/// (`UNITS_AND_SCALE` §2), so fixed columns are honest.
const PANEL_H: f32 = 188.0;
const HEADER_H: f32 = 22.0;
const RULER_H: f32 = 26.0;
const ROW_H: f32 = 24.0;
const CHANNEL_W: f32 = 200.0;
const KEY_W: f32 = 1280.0 - CHANNEL_W;
/// A labeled ruler unit needs at least this much horizontal room.
const LABEL_MIN_PX: f32 = 80.0;
/// Minor ticks thinner than this are dropped (anti-aliasing rule).
const MINOR_MIN_PX: f32 = 8.0;
/// The ruler's unit ladder, ticks (1 tick → 1 min), largest first.
const UNITS: [u32; 6] = [3600, 900, 300, 60, 15, 1];

/// The dope sheet's viewport over the timeline: leftmost visible tick and
/// zoom (ticks per pixel). Smooth values; everything SNAPS to ticks at the
/// interaction layer, never here.
#[derive(Resource)]
pub(crate) struct SheetView {
    pub origin: f32,
    pub ticks_per_px: f32,
    /// The view to return to when `\` toggles back from full-fit.
    last_zoom: Option<(f32, f32)>,
    collapsed: bool,
    /// The arena clock's paused state before a ruler-drag scrub began.
    scrub_resume: Option<bool>,
}

impl Default for SheetView {
    fn default() -> Self {
        Self {
            origin: 0.0,
            ticks_per_px: CYCLE_TICKS as f32 / KEY_W,
            last_zoom: None,
            collapsed: false,
            scrub_resume: None,
        }
    }
}

impl SheetView {
    fn clamp(&mut self) {
        let full = CYCLE_TICKS as f32 / KEY_W;
        // Max-in: 1 tick ≥ ~8 px; max-out: the whole cycle fits.
        self.ticks_per_px = self.ticks_per_px.clamp(1.0 / MINOR_MIN_PX, full);
        let span = KEY_W * self.ticks_per_px;
        self.origin = self.origin.clamp(0.0, (CYCLE_TICKS as f32 - span).max(0.0));
    }

    fn tick_at(&self, px: f32) -> u32 {
        (self.origin + px * self.ticks_per_px)
            .round()
            .clamp(0.0, (CYCLE_TICKS - 1) as f32) as u32
    }

    fn px_of(&self, tick: u32) -> f32 {
        (tick as f32 - self.origin) / self.ticks_per_px
    }
}

/// Marker components for the panel's mount points.
#[derive(Component)]
#[component(immutable)]
struct SheetBody;
#[derive(Component)]
#[component(immutable)]
struct SheetChannels;
#[derive(Component)]
#[component(immutable)]
struct SheetTracks;
#[derive(Component)]
#[component(immutable)]
struct SheetRuler;
#[derive(Component)]
#[component(immutable)]
struct Playhead;
#[derive(Component)]
#[component(immutable)]
struct SheetTitle;

/// A mute/solo/lock toggle on a layer row.
#[derive(Component, Clone, Copy)]
#[component(immutable)]
struct RowToggle {
    layer: LayerId,
    kind: ToggleKind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ToggleKind {
    Mute,
    Solo,
    Lock,
}

/// Asks the preview to re-fold an arena after a stack edit that changes what
/// is effective (mute/solo, layer add/remove). Handled by [`refold_preview`];
/// written here and by the author's layer-creation keys.
#[derive(Message)]
pub(crate) struct RefoldPreview {
    pub(crate) arena: Entity,
}

/// Spawns the panel skeleton once per Intro entry; rows/ruler content are
/// rebuilt by [`rebuild_sheet`] whenever their inputs change.
fn setup_sheet(mut commands: Commands, mono: Res<MonoFont>, current: Res<CurrentArena>) {
    let theme = Arena::from_index(current.0)
        .expect("invariant: CurrentArena is a valid arena index")
        .theme()
        .palette();
    let mono_text = |size: f32| TextFont {
        font: mono.0.clone(),
        font_size: size,
        ..default()
    };
    commands
        .spawn((
            DespawnOnExit(AppState::Intro),
            GlobalZIndex(7),
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(0.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Px(PANEL_H),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(theme.surface_1().with_alpha(0.92)),
            SheetBody,
        ))
        .with_children(|panel| {
            // Header strip: title + hints.
            panel
                .spawn((Node {
                    height: Val::Px(HEADER_H),
                    width: Val::Percent(100.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::horizontal(Val::Px(8.0)),
                    ..default()
                },))
                .with_children(|header| {
                    header.spawn((
                        SheetTitle,
                        Text::new("DOPE SHEET"),
                        mono_text(11.0),
                        TextColor(theme.text_muted()),
                    ));
                    header.spawn((
                        Text::new("wheel pan · ctrl+wheel zoom · = - \\ A · ` collapse"),
                        mono_text(10.0),
                        TextColor(theme.text_muted()),
                    ));
                });
            // Ruler row: spacer over the channel column + the ruler itself.
            panel
                .spawn((Node {
                    height: Val::Px(RULER_H),
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    ..default()
                },))
                .with_children(|row| {
                    row.spawn((Node {
                        width: Val::Px(CHANNEL_W),
                        ..default()
                    },));
                    row.spawn((
                        SheetRuler,
                        RelativeCursorPosition::default(),
                        Interaction::default(),
                        Node {
                            width: Val::Px(KEY_W),
                            height: Val::Px(RULER_H),
                            overflow: Overflow::clip(),
                            ..default()
                        },
                        BackgroundColor(theme.surface_2().with_alpha(0.9)),
                    ));
                });
            // Body: channel column + key area. The playhead is a SIBLING
            // overlay (not a child of the tracks), so row rebuilds — which
            // purge the tracks' children — never despawn it.
            panel
                .spawn((Node {
                    flex_grow: 1.0,
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    ..default()
                },))
                .with_children(|body| {
                    body.spawn((
                        SheetChannels,
                        Node {
                            width: Val::Px(CHANNEL_W),
                            flex_direction: FlexDirection::Column,
                            overflow: Overflow::scroll_y(),
                            ..default()
                        },
                        ScrollPosition::default(),
                    ));
                    body.spawn((
                        SheetTracks,
                        RelativeCursorPosition::default(),
                        Interaction::default(),
                        Node {
                            width: Val::Px(KEY_W),
                            flex_direction: FlexDirection::Column,
                            overflow: Overflow::clip(),
                            ..default()
                        },
                    ));
                    body.spawn((
                        Playhead,
                        Node {
                            position_type: PositionType::Absolute,
                            top: Val::Px(0.0),
                            left: Val::Px(CHANNEL_W),
                            width: Val::Px(1.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        BackgroundColor(theme.warning),
                        GlobalZIndex(8),
                    ));
                });
        });
}

/// One row's strip content for the key area.
fn spawn_strip_segments(
    strip: &mut ChildSpawnerCommands,
    view: &SheetView,
    kind: &LayerKind,
    color: Color,
) {
    let mut segment = |from: u32, to: u32, alpha: f32| {
        let left = view.px_of(from);
        let right = view.px_of(to);
        if right < 0.0 || left > KEY_W {
            return;
        }
        let left = left.max(0.0);
        let width = (right.min(KEY_W) - left).max(1.5);
        strip.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(left),
                top: Val::Px(5.0),
                width: Val::Px(width),
                height: Val::Px(ROW_H - 10.0),
                ..default()
            },
            BackgroundColor(color.with_alpha(alpha)),
        ));
    };
    match kind {
        LayerKind::Boss(recording)
        | LayerKind::Minion(arenic_game::layer::MinionLayer { recording, .. }) => {
            let first = recording.events.first().map_or(0, |e| e.tick);
            let last = recording.events.last().map_or(first, |e| e.tick);
            segment(first, last.max(first.strict_add(1)), 0.35);
        }
        LayerKind::Tiles(keyframes) => {
            // Cap the per-row node count; the v2 heat-strip replaces this.
            for keyframe in keyframes.iter().take(200) {
                segment(keyframe.from, keyframe.to, 0.55);
            }
        }
    }
}

/// Rebuilds rows + ruler whenever the focused stack, the focused arena, or
/// the view changes. At this scale (≤ a dozen rows, ~100 nodes) a rebuild is
/// cheaper than diffing; v2's editing layer mounts into these rows.
fn rebuild_sheet(
    mut commands: Commands,
    mono: Res<MonoFont>,
    current: Res<CurrentArena>,
    view: Res<SheetView>,
    stacks: Query<(&Arena, &ArenaStack)>,
    channels: Single<Entity, With<SheetChannels>>,
    tracks: Single<Entity, With<SheetTracks>>,
    ruler: Single<Entity, With<SheetRuler>>,
    title: Single<&mut Text, With<SheetTitle>>,
) {
    let arena = Arena::from_index(current.0).expect("invariant: CurrentArena is valid");
    let theme = arena.theme().palette();
    let mono_text = |size: f32| TextFont {
        font: mono.0.clone(),
        font_size: size,
        ..default()
    };
    let stack = stacks
        .iter()
        .find(|(a, _)| a.index() == current.0)
        .map(|(_, s)| s);

    let mut title_text = title.into_inner();
    let line = format!("DOPE SHEET · {}", arena.slug());
    if title_text.0 != line {
        title_text.0 = line;
    }

    // --- Channel rows + strips (top of stack first) -------------------------
    commands.entity(*channels).despawn_related::<Children>();
    commands.entity(*tracks).despawn_related::<Children>();

    let Some(stack) = stack else {
        commands.entity(*channels).with_children(|col| {
            col.spawn((
                Text::new("no stack — record (R) or paint (T·Space)"),
                mono_text(10.0),
                TextColor(theme.text_muted()),
                Node {
                    margin: UiRect::all(Val::Px(8.0)),
                    ..default()
                },
            ));
        });
        rebuild_ruler(&mut commands, &mono, &view, &theme, *ruler);
        return;
    };

    for layer in stack.stack.layers.iter().rev() {
        let glyph = match &layer.kind {
            LayerKind::Boss(_) => "B",
            LayerKind::Minion(_) => "M",
            LayerKind::Tiles(_) => "T",
        };
        let dirty = stack.layer_dirty(layer.id);
        let row_alpha = if layer.muted { 0.35 } else { 1.0 };
        let toggle = |kind: ToggleKind, on: bool, label: &str| {
            (
                RowToggle {
                    layer: layer.id,
                    kind,
                },
                Button,
                Text::new(label),
                mono_text(10.0),
                TextColor(if on {
                    theme.warning
                } else {
                    theme.text_muted()
                }),
                Node {
                    margin: UiRect::horizontal(Val::Px(3.0)),
                    ..default()
                },
            )
        };
        commands.entity(*channels).with_children(|col| {
            col.spawn((
                Node {
                    height: Val::Px(ROW_H),
                    width: Val::Percent(100.0),
                    align_items: AlignItems::Center,
                    padding: UiRect::horizontal(Val::Px(6.0)),
                    column_gap: Val::Px(4.0),
                    ..default()
                },
                BackgroundColor(theme.surface_2().with_alpha(0.4 * row_alpha)),
            ))
            .with_children(|row| {
                row.spawn((
                    Text::new(glyph),
                    mono_text(10.0),
                    TextColor(theme.primary.with_alpha(row_alpha)),
                ));
                row.spawn((
                    Text::new(layer.name.clone()),
                    TextFont {
                        font_size: 11.0,
                        ..default()
                    },
                    TextColor(theme.text_1().with_alpha(row_alpha)),
                    Node {
                        flex_grow: 1.0,
                        overflow: Overflow::clip(),
                        ..default()
                    },
                ));
                if dirty {
                    row.spawn((Text::new("●"), mono_text(9.0), TextColor(theme.warning)));
                }
                row.spawn(toggle(ToggleKind::Mute, layer.muted, "M"));
                row.spawn(toggle(ToggleKind::Solo, layer.solo, "S"));
                row.spawn(toggle(ToggleKind::Lock, layer.locked, "L"));
            });
        });
        commands.entity(*tracks).with_children(|tracks| {
            tracks
                .spawn((
                    Node {
                        height: Val::Px(ROW_H),
                        width: Val::Percent(100.0),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(theme.surface_2().with_alpha(0.15)),
                ))
                .with_children(|strip| {
                    spawn_strip_segments(
                        strip,
                        &view,
                        &layer.kind,
                        theme.primary.with_alpha(row_alpha),
                    );
                });
        });
    }

    rebuild_ruler(&mut commands, &mono, &view, &theme, *ruler);
}

/// The adaptive ruler: labeled majors at the largest unit spaced ≥ 80 px,
/// minors one rung below, phase boundaries in the accent colour.
fn rebuild_ruler(
    commands: &mut Commands,
    mono: &MonoFont,
    view: &SheetView,
    theme: &Theme,
    ruler: Entity,
) {
    commands.entity(ruler).despawn_related::<Children>();
    let major = UNITS
        .into_iter()
        .find(|&unit| unit as f32 / view.ticks_per_px >= LABEL_MIN_PX)
        .unwrap_or(1);
    let minor = UNITS
        .into_iter()
        .find(|&unit| unit < major && unit as f32 / view.ticks_per_px >= MINOR_MIN_PX);

    let span = KEY_W * view.ticks_per_px;
    let end = (view.origin + span).min(CYCLE_TICKS as f32) as u32;
    let mut marks: Vec<(u32, bool)> = Vec::new();
    let first_major = (view.origin as u32).div_ceil(major).strict_mul(major);
    let mut tick = first_major;
    while tick <= end {
        marks.push((tick, true));
        tick = tick.strict_add(major);
    }
    if let Some(minor) = minor {
        let first = (view.origin as u32).div_ceil(minor).strict_mul(minor);
        let mut tick = first;
        while tick <= end {
            if tick % major != 0 {
                marks.push((tick, false));
            }
            tick = tick.strict_add(minor);
        }
    }

    commands.entity(ruler).with_children(|ruler| {
        for (tick, labeled) in marks {
            let x = view.px_of(tick);
            ruler.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(x),
                    bottom: Val::Px(0.0),
                    width: Val::Px(1.0),
                    height: Val::Px(if labeled { 10.0 } else { 5.0 }),
                    ..default()
                },
                BackgroundColor(if labeled {
                    theme.text_muted()
                } else {
                    theme.border_subtle()
                }),
            ));
            if labeled {
                let label = if major >= TICKS_PER_SECOND {
                    fmt_tick(tick)
                } else {
                    format!("{}+{:02}", fmt_tick(tick), tick % TICKS_PER_SECOND)
                };
                ruler.spawn((
                    Text::new(label),
                    TextFont {
                        font: mono.0.clone(),
                        font_size: 9.0,
                        ..default()
                    },
                    TextColor(theme.text_muted()),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(x + 3.0),
                        top: Val::Px(2.0),
                        ..default()
                    },
                ));
            }
        }
        // Boss-phase boundaries — the marker lane.
        for phase in 1..3u32 {
            let tick = PHASE_TICKS.strict_mul(phase);
            let x = view.px_of(tick);
            if (0.0..=KEY_W).contains(&x) {
                ruler.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(x),
                        top: Val::Px(0.0),
                        width: Val::Px(2.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(theme.primary.with_alpha(0.7)),
                ));
            }
        }
    });
}

/// Parks the 1-px playhead at the focused arena's clock every frame. The
/// playhead lives in the BODY row, so its x is offset by the channel column.
fn update_playhead(
    current: Res<CurrentArena>,
    view: Res<SheetView>,
    arenas: Query<(&Arena, &ArenaClock)>,
    playhead: Single<(&mut Node, &mut Visibility), With<Playhead>>,
) {
    let Some((_, clock)) = arenas.iter().find(|(a, _)| a.index() == current.0) else {
        return;
    };
    let x = view.px_of(clock.tick);
    let (mut node, mut visibility) = playhead.into_inner();
    let visible = (0.0..=KEY_W).contains(&x);
    let want = if visible {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if *visibility != want {
        *visibility = want;
    }
    let left = CHANNEL_W + x;
    if let Val::Px(current_x) = node.left
        && (current_x - left).abs() < 0.5
    {
        return;
    }
    node.left = Val::Px(left);
}

/// Ruler press/drag → scrub. The arena pauses for the drag and resumes after;
/// the seek itself is byte-identical to the `,`/`.` keys ([`seek_arena`]).
fn scrub_on_ruler(
    mut view: ResMut<SheetView>,
    interactions: Query<(&Interaction, &RelativeCursorPosition), With<SheetRuler>>,
    buttons: Res<ButtonInput<MouseButton>>,
    current: Res<CurrentArena>,
    mut arenas: Query<(Entity, &Arena, &mut ArenaClock, &mut ArenaTimeline)>,
    mut ghosts: Query<(Entity, &Ghost, &ChildOf, &mut TileMover, &mut Transform)>,
) {
    let Ok((interaction, cursor)) = interactions.single() else {
        return;
    };
    let Some((arena_entity, _, mut clock, mut timeline)) = arenas
        .iter_mut()
        .find(|(_, arena, ..)| arena.index() == current.0)
    else {
        return;
    };
    let pressing = *interaction == Interaction::Pressed && buttons.pressed(MouseButton::Left);
    match (pressing, view.scrub_resume) {
        (true, resume) => {
            if resume.is_none() {
                view.scrub_resume = Some(clock.paused);
                clock.paused = true;
            }
            let Some(norm) = cursor.normalized else {
                return;
            };
            // RelativeCursorPosition: (0,0) = node centre.
            let px = (norm.x + 0.5).clamp(0.0, 1.0) * KEY_W;
            let target = view.tick_at(px);
            if clock.tick != target {
                seek_arena(arena_entity, target, &mut clock, &mut timeline, &mut ghosts);
            }
        }
        (false, Some(resume)) => {
            clock.paused = resume;
            view.scrub_resume = None;
        }
        (false, None) => {}
    }
}

/// Wheel over the key area: pan; with `Ctrl`, zoom about the cursor's tick
/// (the tick under the pointer stays stationary — the AC's invariant).
fn wheel_on_tracks(
    scroll: On<Pointer<Scroll>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut view: ResMut<SheetView>,
    cursors: Query<&RelativeCursorPosition, With<SheetTracks>>,
) {
    let cursor_px = cursors
        .single()
        .ok()
        .and_then(|c| c.normalized)
        .map_or(KEY_W * 0.5, |norm| (norm.x + 0.5).clamp(0.0, 1.0) * KEY_W);
    if keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight) {
        let anchor = view.origin + cursor_px * view.ticks_per_px;
        let factor = if scroll.y > 0.0 { 0.85 } else { 1.18 };
        view.ticks_per_px *= factor;
        view.clamp();
        view.origin = anchor - cursor_px * view.ticks_per_px;
    } else {
        view.origin += -scroll.y * 40.0 * view.ticks_per_px;
    }
    view.clamp();
}

/// `=` / `-` zoom about the playhead; `\` toggles last-zoom ↔ full cycle;
/// `A` frames the whole cycle.
fn view_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut view: ResMut<SheetView>,
    current: Res<CurrentArena>,
    arenas: Query<(&Arena, &ArenaClock)>,
) {
    let playhead = arenas
        .iter()
        .find(|(a, _)| a.index() == current.0)
        .map_or(0.0, |(_, clock)| clock.tick as f32);
    let mut zoom_at_playhead = |view: &mut SheetView, factor: f32| {
        let px = view.px_of(playhead as u32).clamp(0.0, KEY_W);
        view.ticks_per_px *= factor;
        view.clamp();
        view.origin = playhead - px * view.ticks_per_px;
        view.clamp();
    };
    if keys.just_pressed(KeyCode::Equal) {
        zoom_at_playhead(&mut view, 0.7);
    }
    if keys.just_pressed(KeyCode::Minus) {
        zoom_at_playhead(&mut view, 1.43);
    }
    if keys.just_pressed(KeyCode::Backslash) {
        let full = CYCLE_TICKS as f32 / KEY_W;
        if (view.ticks_per_px - full).abs() < f32::EPSILON {
            if let Some((origin, tpp)) = view.last_zoom {
                view.origin = origin;
                view.ticks_per_px = tpp;
            }
        } else {
            view.last_zoom = Some((view.origin, view.ticks_per_px));
            view.origin = 0.0;
            view.ticks_per_px = full;
        }
        view.clamp();
    }
    if keys.just_pressed(KeyCode::KeyA) {
        view.origin = 0.0;
        view.ticks_per_px = CYCLE_TICKS as f32 / KEY_W;
    }
}

/// `` ` `` — collapse/expand the panel.
fn toggle_collapsed(mut view: ResMut<SheetView>, mut body: Single<&mut Node, With<SheetBody>>) {
    view.collapsed = !view.collapsed;
    body.display = if view.collapsed {
        Display::None
    } else {
        Display::Flex
    };
}

/// Click handling for the M/S/L row toggles: flips the layer field on the
/// focused arena's draft stack; mute/solo re-fold the preview.
fn row_toggles(
    mut presses: Query<(&Interaction, &RowToggle), Changed<Interaction>>,
    current: Res<CurrentArena>,
    mut stacks: Query<(Entity, &Arena, &mut ArenaStack)>,
    mut refold: MessageWriter<RefoldPreview>,
) {
    for (interaction, toggle) in &mut presses {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some((arena_entity, _, mut stack)) = stacks
            .iter_mut()
            .find(|(_, arena, _)| arena.index() == current.0)
        else {
            continue;
        };
        let Some(layer) = stack.stack.layer_mut(toggle.layer) else {
            continue;
        };
        match toggle.kind {
            ToggleKind::Mute => layer.muted = !layer.muted,
            ToggleKind::Solo => layer.solo = !layer.solo,
            ToggleKind::Lock => layer.locked = !layer.locked,
        }
        if toggle.kind != ToggleKind::Lock {
            refold.write(RefoldPreview {
                arena: arena_entity,
            });
        }
    }
}

/// Re-folds an arena's preview after mute/solo changed what is effective —
/// the same world-application the score sync uses, so preview and replay can
/// never disagree. Restarts the arena (a coherent cursor needs tick 0).
fn refold_preview(
    mut requests: MessageReader<RefoldPreview>,
    mut commands: Commands,
    mut arenas: Query<(
        Entity,
        &Arena,
        &mut ArenaClock,
        &mut ArenaTimeline,
        Option<&mut TileScript>,
        &ArenaStack,
    )>,
    mut movers: ParamSet<(
        Query<
            (
                Entity,
                &ChildOf,
                &mut TileMover,
                &mut Transform,
                &mut RecordingLibrary,
                Has<Ghost>,
                Has<Selected>,
            ),
            With<Boss>,
        >,
        Query<(Entity, &Ghost, &ChildOf, &mut TileMover, &mut Transform)>,
    )>,
    mut minions: Query<
        (
            Entity,
            &arenic_game::layer::LayerBinding,
            &ChildOf,
            &mut TileMover,
            &mut Transform,
        ),
        With<arenic_game::layer::Minion>,
    >,
) {
    for request in requests.read() {
        let Ok((arena_entity, arena_id, mut clock, mut timeline, tile_script, arena_stack)) =
            arenas.get_mut(request.arena)
        else {
            continue;
        };
        let stack = arena_stack.stack.clone();
        let boss = {
            let p0 = movers.p0();
            p0.iter()
                .find(|(_, child_of, ..)| child_of.parent() == arena_entity)
                .map(|(entity, ..)| entity)
        };
        if let Some(mut script) = tile_script {
            script.keyframes.clear();
        }
        if let Some(boss_entity) = boss {
            arenic_game::timeline::unfold(&mut timeline, boss_entity, clock.tick);
        }
        apply_stack_preview(
            &mut commands,
            arena_entity,
            *arena_id,
            &stack,
            boss,
            &mut clock,
            &mut timeline,
            &mut movers,
            &mut minions,
        );
    }
}

/// The dope sheet panel. Registered by the author plugin; v2 (ARE-42) mounts
/// keyframe editing into the structure built here.
pub(crate) struct DopeSheetPlugin;

impl Plugin for DopeSheetPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SheetView>()
            .add_message::<RefoldPreview>()
            .add_systems(OnEnter(AppState::Intro), setup_sheet)
            .add_systems(
                Update,
                (
                    attach_wheel_observer,
                    rebuild_sheet.run_if(
                        stacks_changed
                            .or(resource_changed::<CurrentArena>)
                            .or(resource_changed::<SheetView>),
                    ),
                    update_playhead,
                    scrub_on_ruler,
                    view_keys.run_if(no_modal),
                    toggle_collapsed.run_if(input_just_pressed(KeyCode::Backquote)),
                    row_toggles,
                    refold_preview.run_if(on_message::<RefoldPreview>),
                )
                    .run_if(in_state(AppState::Intro)),
            );
    }
}

/// Wires the wheel observer onto the tracks node once it exists (observers
/// can't be attached inside `children![..]` spawns).
fn attach_wheel_observer(mut commands: Commands, tracks: Query<Entity, Added<SheetTracks>>) {
    for entity in &tracks {
        commands.entity(entity).observe(wheel_on_tracks);
    }
}
