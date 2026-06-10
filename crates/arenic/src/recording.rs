//! The recording state machine — R-key flows, intent capture, modal choices,
//! ghost visuals, and the REC/clock HUD strip (RULEBOOK → Record & Replay).
//!
//! Exactly one recording exists at a time, held in the global [`RecordingState`]
//! and [`DraftTimeline`] resources; the recording arena is always *the selected
//! character's arena*, queried on demand and never stored (RULEBOOK → Selected
//! Character Query Pattern). Every modal decision lands here in [`handle_choice`],
//! the single, idempotent place state transitions are applied.

use arenic_game::Boss;
use arenic_game::arena::Arena;
use arenic_game::default_font::MonoFont;
use arenic_game::encounter::{ActiveDifficulty, Difficulty};
use arenic_game::grid::TileMover;
use arenic_game::timeline::{
    Action, ArenaClock, ArenaTimeline, COUNTDOWN_TICKS, CYCLE_TICKS, Ghost, Recording,
    RecordingLibrary, TICKS_PER_SECOND, TimelineEvent, TimelineSet, fold, restart, snap_ghost,
    unfold,
};
use bevy::input::common_conditions::input_just_pressed;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;

use crate::hud::TOP_BAR_PX;
use crate::intro_scene::{CurrentArena, Selected};
use crate::modal::{
    ActiveModal, Choice, ModalChoice, ModalLatch, ModalRoot, no_modal, spawn_modal,
};
use crate::states::{AppState, not_tile_editing};

/// The one in-flight recording. `Countdown` holds the recording arena at tick 0
/// for 3 seconds; `Recording` captures until the player commits/discards or the
/// cycle fills.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum RecordingState {
    #[default]
    Idle,
    Countdown {
        ticks_left: u32,
    },
    Recording,
}

/// The draft staff being captured — empty unless a recording is in flight.
#[derive(Resource, Default)]
pub(crate) struct DraftTimeline {
    pub(crate) start: IVec2,
    pub(crate) events: Vec<TimelineEvent>,
}

/// The edge-walk step that interrupted a recording, stashed while the
/// *Like the recording?* modal is open. "Cancel & walk out" leaves it for
/// `travel::perform_pending_walk`; every other choice clears it.
#[derive(Resource)]
pub(crate) struct PendingWalk(pub(crate) IVec2);

/// Written once per Commit — the seam the author feature listens on to mirror
/// a boss's committed staff into a versioned score file. Builds without that
/// consumer (plain game, and wasm even with the feature) write it and nobody
/// reads it (the buffer just drains), so the fields look dead there.
#[derive(Message)]
#[cfg_attr(
    not(all(feature = "author", not(target_arch = "wasm32"))),
    allow(dead_code)
)]
pub(crate) struct RecordingCommitted {
    pub(crate) entity: Entity,
    pub(crate) arena: Arena,
    /// Stamped at COMMIT time — the mirror runs unordered against
    /// [`handle_choice`], so reading [`ActiveDifficulty`] there could see a
    /// post-commit `D` press.
    pub(crate) difficulty: Difficulty,
    pub(crate) recording: Recording,
}

/// Marks the blue ring child a [`Ghost`] wears (see [`ghost_ring_add`]).
#[derive(Component)]
#[component(immutable)]
struct GhostRing;

/// One element of the REC/clock HUD strip under the top bar.
#[derive(Component)]
#[component(immutable)]
enum RecHudPart {
    Clock,
    Rec,
    Countdown,
}

pub(crate) fn is_idle(state: Res<RecordingState>) -> bool {
    matches!(*state, RecordingState::Idle)
}

pub(crate) fn is_recording(state: Res<RecordingState>) -> bool {
    matches!(*state, RecordingState::Recording)
}

pub(crate) fn not_counting_down(state: Res<RecordingState>) -> bool {
    !matches!(*state, RecordingState::Countdown { .. })
}

/// `true` while no confirmed-but-unperformed edge-walk is in flight. World input
/// (R, Tab, abilities, L) freezes for the one frame between "Cancel & walk out"
/// and `travel::perform_pending_walk` — otherwise that input could re-arm a
/// recording or retarget `Selected`, stranding the walk (or applying it to the
/// wrong hero).
pub(crate) fn no_pending_walk(pending: Option<Res<PendingWalk>>) -> bool {
    pending.is_none()
}

fn counting_down(state: Res<RecordingState>) -> bool {
    matches!(*state, RecordingState::Countdown { .. })
}

/// `m:ss` of a cycle tick — the clock format every HUD strip shares.
pub(crate) fn fmt_tick(tick: u32) -> String {
    let secs = tick / TICKS_PER_SECOND;
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// Holds `clock`/`timeline` at tick 0 (paused) and arms the draft — the 3-second
/// countdown begins. Callers snap the arena's other ghosts to their starts.
fn arm_countdown(
    clock: &mut ArenaClock,
    timeline: &mut ArenaTimeline,
    draft: &mut DraftTimeline,
    state: &mut RecordingState,
    hero_tile: IVec2,
) {
    clock.tick = 0;
    clock.paused = true;
    timeline.cursor = 0;
    draft.start = hero_tile;
    draft.events.clear();
    *state = RecordingState::Countdown {
        ticks_left: COUNTDOWN_TICKS,
    };
}

/// Folds `recording` for `hero` into the arena's master timeline, restarts the
/// arena, and re-poses the hero as a ghost at the recording's start — the ONE
/// commit/replay tail, so the two paths can't drift. The hero's `Ghost` marker
/// lands via commands (deferred), so it is snapped by hand here; the caller
/// snaps the arena's OTHER ghosts.
fn fold_as_ghost(
    commands: &mut Commands,
    hero: Entity,
    recording: &Recording,
    clock: &mut ArenaClock,
    timeline: &mut ArenaTimeline,
    mover: &mut TileMover,
    transform: &mut Transform,
) {
    fold(timeline, hero, recording);
    restart(clock, timeline);
    let ghost = Ghost {
        start: recording.start,
    };
    snap_ghost(&ghost, mover, transform);
    commands.entity(hero).insert(ghost);
}

/// Snaps every ghost parented to `arena` back to its recorded start tile —
/// `except` skips an entity whose `Ghost` marker is mid-insertion/removal
/// (commands defer), posed by hand by the caller. Shared by the recording
/// flows, the score sync, and the author tool's scrub/restart.
pub(crate) fn snap_arena_ghosts(
    arena: Entity,
    ghosts: &mut Query<(Entity, &Ghost, &ChildOf, &mut TileMover, &mut Transform)>,
    except: Option<Entity>,
) {
    for (entity, ghost, child_of, mut mover, mut transform) in ghosts.iter_mut() {
        if child_of.parent() == arena && except != Some(entity) {
            snap_ghost(ghost, &mut mover, &mut transform);
        }
    }
}

/// `R` — the context-sensitive record key (RULEBOOK → Controls). No modal when
/// there's no real choice: a fresh hero/arena pair goes straight to countdown.
fn handle_r_key(
    mut commands: Commands,
    mono: Res<MonoFont>,
    mut latch: ResMut<ModalLatch>,
    mut state: ResMut<RecordingState>,
    mut draft: ResMut<DraftTimeline>,
    mut heroes: ParamSet<(
        Query<(&ChildOf, Has<Ghost>, &RecordingLibrary, &TileMover), With<Selected>>,
        Query<(Entity, &Ghost, &ChildOf, &mut TileMover, &mut Transform)>,
    )>,
    arena_ids: Query<&Arena>,
    mut arenas: Query<(&Arena, &mut ArenaClock, &mut ArenaTimeline)>,
) -> Result {
    let (arena_entity, is_ghost, hero_tile, has_cache) = {
        let p0 = heroes.p0();
        let Ok((child_of, is_ghost, library, mover)) = p0.single() else {
            return Ok(());
        };
        let arena_entity = child_of.parent();
        let arena = *arena_ids.get(arena_entity)?;
        (
            arena_entity,
            is_ghost,
            IVec2::new(mover.col, mover.row),
            library.0.contains_key(&arena),
        )
    };
    let (arena, mut clock, mut timeline) = arenas.get_mut(arena_entity)?;
    let theme = arena.theme().palette();

    match *state {
        RecordingState::Countdown { .. } => {
            // R again aborts the countdown — nothing has been captured yet.
            *state = RecordingState::Idle;
            draft.events.clear();
            clock.paused = false;
        }
        RecordingState::Recording => {
            let _ = spawn_modal(
                &mut commands,
                &mut latch,
                &mut clock,
                &theme,
                &mono,
                "Like the recording?",
                "Commit folds this draft into the arena's timeline",
                &[
                    ("Commit", Choice::Commit),
                    ("Keep recording", Choice::Cancel),
                    ("Discard", Choice::Discard),
                ],
                1,
            );
        }
        RecordingState::Idle if is_ghost => {
            let _ = spawn_modal(
                &mut commands,
                &mut latch,
                &mut clock,
                &theme,
                &mono,
                "Record over this ghost?",
                "Record new unfolds it and starts the countdown",
                &[
                    ("Record new", Choice::StartRecording),
                    ("Cancel", Choice::Cancel),
                ],
                1,
            );
        }
        RecordingState::Idle if has_cache => {
            let _ = spawn_modal(
                &mut commands,
                &mut latch,
                &mut clock,
                &theme,
                &mono,
                "This hero has a staff here",
                "Record new replaces it - Replay folds the old one back in",
                &[
                    ("Record new", Choice::StartRecording),
                    ("Replay previous", Choice::ReplayPrevious),
                    ("Cancel", Choice::Cancel),
                ],
                2,
            );
        }
        RecordingState::Idle => {
            arm_countdown(&mut clock, &mut timeline, &mut draft, &mut state, hero_tile);
            snap_arena_ghosts(arena_entity, &mut heroes.p1(), None);
        }
    }
    Ok(())
}

/// Mirrors a HERO's ability slots 2-4 into the draft while recording, stamped
/// with the recording arena's current tick. Captured in `Update` so
/// `just_pressed` edges are never missed; the stamp quantizes the intent onto
/// the tick grid. Hero slots 2-4 are inert on both sides (no live cast, no
/// playback cast), so recording them silently keeps the staff faithful. A
/// possessed BOSS is skipped here entirely: every boss slot has a live effect
/// (the phase loadout), so `author::boss_cast_slots` captures cast + event
/// atomically — the committed staff can never contain an event the live take
/// didn't perform (or vice versa). Anything else with a LIVE effect records
/// atomically where it applies — movement in `travel::move_selected`, hero
/// slot 1 in `intro_scene::fire_holy_nova`.
fn capture_intent(
    keys: Res<ButtonInput<KeyCode>>,
    selected: Single<(&ChildOf, Has<Boss>), With<Selected>>,
    clocks: Query<&ArenaClock>,
    mut draft: ResMut<DraftTimeline>,
) -> Result {
    let (child_of, is_boss) = *selected;
    if is_boss {
        return Ok(());
    }
    let tick = clocks.get(child_of.parent())?.tick;
    const SLOTS: [(KeyCode, KeyCode, u8); 3] = [
        (KeyCode::Digit2, KeyCode::Numpad2, 2),
        (KeyCode::Digit3, KeyCode::Numpad3, 3),
        (KeyCode::Digit4, KeyCode::Numpad4, 4),
    ];
    for (digit, numpad, slot) in SLOTS {
        if keys.just_pressed(digit) || keys.just_pressed(numpad) {
            draft.events.push(TimelineEvent {
                tick,
                action: Action::Ability { slot, aim: None },
            });
        }
    }
    Ok(())
}

/// Counts the 3-second countdown down; at zero the recording arena's clock is
/// released and capture begins.
fn tick_countdown(
    mut state: ResMut<RecordingState>,
    selected: Single<&ChildOf, With<Selected>>,
    mut clocks: Query<&mut ArenaClock>,
) -> Result {
    let RecordingState::Countdown { ticks_left } = &mut *state else {
        return Ok(());
    };
    *ticks_left = ticks_left.strict_sub(1);
    if *ticks_left == 0 {
        clocks.get_mut(selected.parent())?.paused = false;
        *state = RecordingState::Recording;
    }
    Ok(())
}

/// When the recording arena's clock reaches the end of the cycle, pause it and
/// ask the player to commit or discard — the 2-minute staff is full.
fn auto_stop(
    mut commands: Commands,
    mono: Res<MonoFont>,
    mut latch: ResMut<ModalLatch>,
    selected: Single<&ChildOf, With<Selected>>,
    mut arenas: Query<(&Arena, &mut ArenaClock)>,
) -> Result {
    let (arena, mut clock) = arenas.get_mut(selected.parent())?;
    if clock.tick < CYCLE_TICKS.strict_sub(1) {
        return Ok(());
    }
    let _ = spawn_modal(
        &mut commands,
        &mut latch,
        &mut clock,
        &arena.theme().palette(),
        &mono,
        "Time's up - like the recording?",
        "The two-minute cycle is full",
        &[("Commit", Choice::Commit), ("Discard", Choice::Discard)],
        0,
    );
    Ok(())
}

/// Applies the choice made on ANY modal — the one place recording transitions
/// happen, so each is explicit and idempotent. Closes the modal first; every
/// branch then resumes, restarts, or re-arms the affected arena deliberately.
fn handle_choice(
    mut commands: Commands,
    mut choices: MessageReader<ModalChoice>,
    mut commits: MessageWriter<RecordingCommitted>,
    difficulty: Res<ActiveDifficulty>,
    mut state: ResMut<RecordingState>,
    mut draft: ResMut<DraftTimeline>,
    mut heroes: ParamSet<(
        Query<
            (
                Entity,
                &ChildOf,
                &mut RecordingLibrary,
                &mut TileMover,
                &mut Transform,
            ),
            With<Selected>,
        >,
        Query<(Entity, &Ghost, &ChildOf, &mut TileMover, &mut Transform)>,
    )>,
    mut arenas: Query<(&Arena, &mut ArenaClock, &mut ArenaTimeline)>,
    roots: Query<Entity, With<ModalRoot>>,
    mut latch: ResMut<ModalLatch>,
) -> Result {
    let Some(&ModalChoice(choice)) = choices.read().next() else {
        return Ok(());
    };
    choices.clear();
    if !latch.0 {
        // A mashed press that landed after the modal already closed — drop it.
        return Ok(());
    }
    latch.0 = false;

    // Close the modal; the branches below decide how the arena resumes.
    commands.remove_resource::<ActiveModal>();
    for root in &roots {
        commands.entity(root).despawn();
    }
    if choice != Choice::DiscardAndWalk {
        commands.remove_resource::<PendingWalk>();
    }

    let (hero, arena_entity, hero_tile) = {
        let p0 = heroes.p0();
        let Ok((hero, child_of, _, mover, _)) = p0.single() else {
            return Ok(());
        };
        (hero, child_of.parent(), IVec2::new(mover.col, mover.row))
    };

    match choice {
        Choice::Cancel => {
            // Resume exactly where the modal froze the arena.
            arenas.get_mut(arena_entity)?.1.paused = false;
        }
        Choice::Discard | Choice::DiscardAndWalk => {
            // Throw the draft away; the hero stays selected where it stands.
            // DiscardAndWalk leaves `PendingWalk` for the intro scene to perform.
            *state = RecordingState::Idle;
            draft.events.clear();
            arenas.get_mut(arena_entity)?.1.paused = false;
        }
        Choice::Commit => {
            *state = RecordingState::Idle;
            let recording = Recording {
                start: draft.start,
                events: draft.events.drain(..).collect(),
            };
            {
                let mut p0 = heroes.p0();
                let (_, _, mut library, mut mover, mut transform) = p0.single_mut()?;
                let (arena, mut clock, mut timeline) = arenas.get_mut(arena_entity)?;
                library.0.insert(*arena, recording.clone());
                fold_as_ghost(
                    &mut commands,
                    hero,
                    &recording,
                    &mut clock,
                    &mut timeline,
                    &mut mover,
                    &mut transform,
                );
                commits.write(RecordingCommitted {
                    entity: hero,
                    arena: *arena,
                    difficulty: difficulty.0,
                    recording,
                });
            }
            snap_arena_ghosts(arena_entity, &mut heroes.p1(), None);
        }
        Choice::ReplayPrevious => {
            {
                let mut p0 = heroes.p0();
                let (_, _, library, mut mover, mut transform) = p0.single_mut()?;
                let (arena, mut clock, mut timeline) = arenas.get_mut(arena_entity)?;
                let Some(recording) = library.0.get(arena) else {
                    // No cache after all (shouldn't happen) — at least resume.
                    clock.paused = false;
                    return Ok(());
                };
                fold_as_ghost(
                    &mut commands,
                    hero,
                    recording,
                    &mut clock,
                    &mut timeline,
                    &mut mover,
                    &mut transform,
                );
            }
            snap_arena_ghosts(arena_entity, &mut heroes.p1(), None);
        }
        Choice::StartRecording => {
            let (_, mut clock, mut timeline) = arenas.get_mut(arena_entity)?;
            if heroes.p1().get(hero).is_ok() {
                // Re-recording a ghost: pull its old events out first.
                unfold(&mut timeline, hero, clock.tick);
                commands.entity(hero).remove::<Ghost>();
            }
            arm_countdown(&mut clock, &mut timeline, &mut draft, &mut state, hero_tile);
            snap_arena_ghosts(arena_entity, &mut heroes.p1(), Some(hero));
        }
        Choice::TakeControl => {
            // Break-out: the events leave the score, the arena keeps playing
            // right where it left off, and the hero is free at its current tile.
            let (_, mut clock, mut timeline) = arenas.get_mut(arena_entity)?;
            unfold(&mut timeline, hero, clock.tick);
            clock.paused = false;
            commands.entity(hero).remove::<Ghost>();
        }
        Choice::RestartArena => {
            // "I made a mistake": rewind everything; the hero stays folded.
            let (_, mut clock, mut timeline) = arenas.get_mut(arena_entity)?;
            restart(&mut clock, &mut timeline);
            snap_arena_ghosts(arena_entity, &mut heroes.p1(), None);
        }
    }
    Ok(())
}

/// Dresses a fresh [`Ghost`] in its blue ring — the sibling of the white
/// selection halo (same `Annulus` recipe, ghost-blue, slightly wider so the two
/// read concentrically on a selected ghost). Parented to the wearer, so it
/// follows playback. Wearers differ in pose — a puck is X-rotated 90°, a boss
/// root is not — so the ring counter-rotates by the parent's inverse to lie
/// flat, parked just above the board (`z = 0.03` world) whatever the parent's
/// own height.
fn ghost_ring_add(
    add: On<Add, Ghost>,
    mut commands: Commands,
    transforms: Query<&Transform>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let (inverse, lift) = transforms
        .get(add.entity)
        .map(|t| (t.rotation.inverse(), 0.03 - t.translation.z))
        .unwrap_or((Quat::IDENTITY, 0.0));
    let blue = Color::srgb(0.35, 0.55, 1.0);
    let l = blue.to_linear();
    commands.spawn((
        GhostRing,
        Mesh3d(meshes.add(Annulus::new(0.32, 0.38))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: blue,
            emissive: LinearRgba::rgb(l.red, l.green, l.blue) * 2.5,
            ..default()
        })),
        Transform::from_translation(inverse * Vec3::new(0.0, 0.0, lift)).with_rotation(inverse),
        NotShadowCaster,
        NotShadowReceiver,
        ChildOf(add.entity),
    ));
}

/// Removes the blue ring when its ghost breaks out / records anew.
fn ghost_ring_remove(
    remove: On<Remove, Ghost>,
    mut commands: Commands,
    rings: Query<(Entity, &ChildOf), With<GhostRing>>,
) {
    for (ring, child_of) in &rings {
        if child_of.parent() == remove.entity {
            commands.entity(ring).despawn();
        }
    }
}

/// The thin strip under the top bar: `REC` while recording, the selected arena's
/// clock as `m:ss`, and the countdown digit. Spawned in the current arena's
/// tokens; [`update_rec_hud`] re-tokens as focus moves.
fn setup_rec_hud(mut commands: Commands, mono: Res<MonoFont>, current: Res<CurrentArena>) {
    let theme = Arena::from_index(current.0)
        .expect("invariant: CurrentArena is a valid arena index")
        .theme()
        .palette();
    let mono_text = |size: f32| TextFont {
        font: mono.0.clone(),
        font_size: size,
        ..default()
    };
    commands.spawn((
        DespawnOnExit(AppState::Intro),
        GlobalZIndex(5),
        Pickable::IGNORE,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(TOP_BAR_PX + 8.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            ..default()
        },
        children![
            (
                RecHudPart::Rec,
                Text::new("REC"),
                mono_text(14.0),
                TextColor(theme.error),
                Visibility::Hidden,
            ),
            (
                RecHudPart::Clock,
                Text::new("0:00"),
                mono_text(14.0),
                TextColor(theme.text_1()),
            ),
            (
                RecHudPart::Countdown,
                Text::new(""),
                mono_text(22.0),
                TextColor(theme.warning),
                Visibility::Hidden,
            ),
        ],
    ));
}

/// Drives the strip from the selected hero's arena: its clock, the REC flag, and
/// the countdown digit, themed to that arena's palette.
fn update_rec_hud(
    state: Res<RecordingState>,
    selected: Single<&ChildOf, With<Selected>>,
    arenas: Query<(&Arena, &ArenaClock)>,
    mut parts: Query<(&RecHudPart, &mut Text, &mut TextColor, &mut Visibility)>,
) -> Result {
    let (arena, clock) = arenas.get(selected.parent())?;
    let theme = arena.theme().palette();
    // Write-if-changed throughout: an unconditional `text.0 =` would mark the
    // Text changed and re-shape its glyphs every frame for a string that only
    // changes once a second.
    let set_text = |text: &mut Mut<Text>, s: String| {
        if text.0 != s {
            text.0 = s;
        }
    };
    let set_color = |color: &mut Mut<TextColor>, c: Color| {
        if color.0 != c {
            color.0 = c;
        }
    };
    let set_visible = |visibility: &mut Mut<Visibility>, on: bool| {
        let v = if on {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if **visibility != v {
            **visibility = v;
        }
    };
    for (part, mut text, mut color, mut visibility) in &mut parts {
        match part {
            RecHudPart::Clock => {
                set_text(&mut text, fmt_tick(clock.tick));
                set_color(&mut color, theme.text_1());
            }
            RecHudPart::Rec => {
                set_color(&mut color, theme.error);
                set_visible(&mut visibility, matches!(*state, RecordingState::Recording));
            }
            RecHudPart::Countdown => {
                set_color(&mut color, theme.warning);
                if let RecordingState::Countdown { ticks_left } = *state {
                    set_text(&mut text, ticks_left.div_ceil(TICKS_PER_SECOND).to_string());
                    set_visible(&mut visibility, true);
                } else {
                    set_visible(&mut visibility, false);
                }
            }
        }
    }
    Ok(())
}

/// Leaving the intro scene mid-recording drops everything cleanly (the entities
/// despawn via `DespawnOnExit`; the globals reset here).
fn reset_on_exit(
    mut commands: Commands,
    mut state: ResMut<RecordingState>,
    mut draft: ResMut<DraftTimeline>,
    mut latch: ResMut<ModalLatch>,
) {
    *state = RecordingState::Idle;
    draft.events.clear();
    latch.0 = false;
    commands.remove_resource::<ActiveModal>();
    commands.remove_resource::<PendingWalk>();
}

pub(crate) struct RecordingPlugin;

impl Plugin for RecordingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RecordingState>()
            .init_resource::<DraftTimeline>()
            .add_message::<RecordingCommitted>()
            .add_observer(ghost_ring_add)
            .add_observer(ghost_ring_remove)
            .add_systems(OnEnter(AppState::Intro), setup_rec_hud)
            .add_systems(OnExit(AppState::Intro), reset_on_exit)
            .add_systems(
                Update,
                (
                    handle_r_key.run_if(
                        input_just_pressed(KeyCode::KeyR)
                            .and(no_modal)
                            .and(no_pending_walk)
                            .and(not_tile_editing),
                    ),
                    capture_intent.run_if(is_recording.and(no_modal)),
                    // Gated on a pending message: the heavy ParamSet (and its
                    // mutable access) is skipped entirely on choice-less frames.
                    handle_choice.run_if(on_message::<ModalChoice>),
                    update_rec_hud,
                )
                    .run_if(in_state(AppState::Intro)),
            )
            .add_systems(
                FixedUpdate,
                (
                    tick_countdown.run_if(counting_down),
                    auto_stop.run_if(is_recording),
                )
                    .after(TimelineSet)
                    .run_if(no_modal.and(in_state(AppState::Intro))),
            );
    }
}
