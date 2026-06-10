//! Record & Replay timelines — the "sheet music" system (`_docs/RULEBOOK.md`).
//!
//! Every character can record a 2-minute stream of intent events per arena (its
//! "staff" of sheet music, cached in its [`RecordingLibrary`]); committing folds
//! the staff into the arena's single master [`ArenaTimeline`] (the "score"),
//! which drives every folded [`Ghost`] as the arena's independent [`ArenaClock`]
//! ticks. The sim runs on the fixed timestep and counts ticks, never seconds;
//! events store *intent* (tile steps + ability slots) and transforms are derived
//! on playback (`_docs/code-review.md` → Determinism).
//!
//! Pure timeline surgery lives in [`fold`] / [`unfold`] (unit-tested, no World).
//! [`TimelinePlugin`] pins the fixed timestep to [`TICK_HZ`] and registers the
//! per-tick playback systems; the binaries layer recording/input on top.

use std::sync::Arc;

use bevy::platform::collections::HashMap;
use bevy::prelude::*;

use bevy::audio::DefaultSpatialScale;

use crate::ability::{AbilityMeshes, AbilitySfx, cast};
use crate::arena::Arena;
use crate::audio::AudioMix;
use crate::boss::Boss;
use crate::encounter::{ActiveDifficulty, resolve_ability};
use crate::grid::TileMover;

/// Sim ticks per second — the fixed-timestep rate everything below counts in.
pub const TICKS_PER_SECOND: u32 = 60;
/// [`TICKS_PER_SECOND`] as the rate [`TimelinePlugin`] pins `Time<Fixed>` to.
pub const TICK_HZ: f64 = TICKS_PER_SECOND as f64;
/// Ticks in one 2-minute arena cycle (const-evaluated: overflow fails the build).
pub const CYCLE_TICKS: u32 = 120 * TICKS_PER_SECOND;
/// Ticks in the 3-second pre-recording countdown.
pub const COUNTDOWN_TICKS: u32 = 3 * TICKS_PER_SECOND;

/// One recorded intent: what the player pressed, on which tick of the cycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimelineEvent {
    pub tick: u32,
    pub action: Action,
}

/// A recordable intent — a tile step or an ability slot (`1..=4`). On playback
/// a hero ghost casts only slot 1 (slots 2-4 are recorded but inert until more
/// hero abilities exist); a boss ghost resolves every slot through its phase
/// loadout ([`crate::encounter::resolve_ability`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Move(IVec2),
    Ability(u8),
}

/// One character's committed staff for one arena: the tile it stood on at tick 0
/// (`start` — every cycle replays from there) plus its tick-sorted events. A
/// partial commit is simply a shorter event list; the ghost plays it, then idles.
#[derive(Clone, PartialEq, Debug)]
pub struct Recording {
    pub start: IVec2,
    pub events: Arc<[TimelineEvent]>,
}

/// A character's per-arena library of recordings, keyed by [`Arena`] — its
/// staff per arena. Sim code only ever looks up by key; never iterate this for
/// simulation (HashMap order is nondeterministic).
#[derive(Component, Default)]
pub struct RecordingLibrary(pub HashMap<Arena, Recording>);

/// Present while a character is folded into its arena's master timeline: playback
/// drives it and direct input is ignored. `start` is the tile the ghost snaps to
/// whenever its arena restarts at tick 0. Set once per fold (immutable) — a ghost
/// changes by being unfolded and re-folded, never edited in place.
#[derive(Component)]
#[component(immutable)]
pub struct Ghost {
    pub start: IVec2,
}

/// One event of the master timeline: a [`TimelineEvent`] tagged with the ghost
/// entity it drives.
#[derive(Clone, Copy, Debug)]
pub struct GhostEvent {
    pub tick: u32,
    pub ghost: Entity,
    pub action: Action,
}

/// An arena's master timeline: every committed recording cloned, tagged with its
/// ghost, and merged into one tick-sorted stream, plus the playback cursor.
/// Sorting is stable for equal ticks (earlier-folded ghosts keep priority), so
/// playback order is deterministic within a session.
#[derive(Component, Default)]
pub struct ArenaTimeline {
    pub events: Vec<GhostEvent>,
    pub cursor: usize,
}

/// An arena's independent 2-minute clock, in ticks (`0..CYCLE_TICKS`). Paused
/// while a modal targets the arena or while a recording countdown holds it at 0.
#[derive(Component, Default)]
pub struct ArenaClock {
    pub tick: u32,
    pub paused: bool,
}

/// Replaces `ghost`'s events in the master timeline with `recording`'s and
/// resets the cursor — folding always accompanies an arena restart (the caller
/// zeroes the clock and snaps ghosts to their starts). Removing first makes
/// re-committing idempotent; the stable sort keeps earlier-folded ghosts first
/// within a tick.
pub fn fold(timeline: &mut ArenaTimeline, ghost: Entity, recording: &Recording) {
    timeline.events.retain(|e| e.ghost != ghost);
    timeline.events.extend(
        recording
            .events
            .iter()
            .map(|&TimelineEvent { tick, action }| GhostEvent {
                tick,
                ghost,
                action,
            }),
    );
    timeline.events.sort_by_key(|e| e.tick);
    timeline.cursor = 0;
}

/// Removes every event belonging to `ghost` and resyncs the cursor so playback
/// continues seamlessly from `tick` (pass the arena clock's current tick). The
/// arena does NOT restart — that's break-out semantics; restart paths overwrite
/// the cursor anyway via [`restart`].
pub fn unfold(timeline: &mut ArenaTimeline, ghost: Entity, tick: u32) {
    timeline.events.retain(|e| e.ghost != ghost);
    timeline.cursor = timeline.events.partition_point(|e| e.tick < tick);
}

/// Restarts an arena's playback: clock to tick 0, cursor to the top, unpaused.
/// Callers also snap each of the arena's ghosts to its start ([`snap_ghost`]).
pub fn restart(clock: &mut ArenaClock, timeline: &mut ArenaTimeline) {
    clock.tick = 0;
    clock.paused = false;
    timeline.cursor = 0;
}

/// Parks a ghost on its recorded start tile (the pose every cycle replays from).
pub fn snap_ghost(ghost: &Ghost, mover: &mut TileMover, transform: &mut Transform) {
    mover.snap_to(transform, ghost.start.x, ghost.start.y);
}

/// The next clock value after one tick — wraps at the cycle boundary.
fn advance(tick: u32) -> u32 {
    tick.strict_add(1) % CYCLE_TICKS
}

/// The `FixedUpdate` playback phase — binaries order their recording systems
/// `.after(TimelineSet)` so they observe this tick's clocks.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimelineSet;

/// Applies every master-timeline event whose tick has arrived: `Move` steps the
/// ghost's [`TileMover`] (clamped — recorded moves never edge-walk) and
/// `Ability(slot)` resolves through [`resolve_ability`] — a hero ghost casts
/// Holy Nova on slot 1 (2-4 are inert today); a [`Boss`] ghost reads the slot
/// from its phase loadout for the active difficulty. Runs before
/// [`tick_clocks`], so an event plays exactly while `clock.tick` equals its
/// recorded tick.
fn play_timelines(
    mut commands: Commands,
    difficulty: Res<ActiveDifficulty>,
    mut arenas: Query<(&Arena, &ArenaClock, &mut ArenaTimeline)>,
    mut ghosts: Query<(&mut TileMover, &mut Transform, Has<Boss>), With<Ghost>>,
    meshes: Res<AbilityMeshes>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    sfx: Res<AbilitySfx>,
    mix: Res<AudioMix>,
    scale: Res<DefaultSpatialScale>,
) {
    for (arena, clock, mut timeline) in &mut arenas {
        if clock.paused {
            continue;
        }
        while let Some(&GhostEvent {
            tick,
            ghost,
            action,
        }) = timeline.events.get(timeline.cursor)
        {
            if tick > clock.tick {
                break;
            }
            timeline.cursor = timeline.cursor.strict_add(1);
            match action {
                Action::Move(delta) => {
                    if let Ok((mut mover, mut transform, _)) = ghosts.get_mut(ghost) {
                        mover.step(&mut transform, delta);
                    }
                }
                Action::Ability(slot) => {
                    let is_boss = ghosts.get(ghost).is_ok_and(|(_, _, boss)| boss);
                    // Resolve at the event's RECORDED tick, so an ability keeps
                    // the phase it was authored in even if playback ever lags.
                    if let Some(id) = resolve_ability(is_boss, *arena, difficulty.0, slot, tick) {
                        cast(
                            &mut commands,
                            id,
                            &meshes,
                            &mut materials,
                            &sfx,
                            &mix,
                            &scale,
                            ghost,
                        );
                    }
                }
            }
        }
    }
}

/// The events already played at `tick` (every event stamped `<= tick`) and the
/// playback cursor that goes with them — the scrub math behind the author
/// tool's timeline seek. Callers snap the arena's ghosts to their starts,
/// re-apply the returned window's `Move` events (abilities are transient VFX,
/// skipped), then store the cursor and set the clock to `tick`.
pub fn seek_window(events: &[GhostEvent], tick: u32) -> (&[GhostEvent], usize) {
    let upto = events.partition_point(|e| e.tick <= tick);
    (&events[..upto], upto)
}

/// Advances every unpaused [`ArenaClock`] one tick; on wrap, rewinds playback
/// and snaps the arena's ghosts back to their recorded starts.
fn tick_clocks(
    mut arenas: Query<(Entity, &mut ArenaClock, &mut ArenaTimeline)>,
    mut ghosts: Query<(&Ghost, &ChildOf, &mut TileMover, &mut Transform)>,
) {
    for (arena, mut clock, mut timeline) in &mut arenas {
        if clock.paused {
            continue;
        }
        clock.tick = advance(clock.tick);
        if clock.tick == 0 {
            timeline.cursor = 0;
            for (ghost, child_of, mut mover, mut transform) in &mut ghosts {
                if child_of.parent() == arena {
                    snap_ghost(ghost, &mut mover, &mut transform);
                }
            }
        }
    }
}

/// Pins the fixed timestep to [`TICK_HZ`] and drives every arena's clock +
/// master-timeline playback. Add [`crate::ability::AbilityPlugin`] alongside it
/// (playback casts need the preloaded [`AbilitySfx`]).
pub struct TimelinePlugin;

impl Plugin for TimelinePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_hz(TICK_HZ))
            .init_resource::<ActiveDifficulty>()
            // Playback casts read the SFX bus; idempotent with GameAudioPlugin.
            .init_resource::<AudioMix>()
            .add_systems(
                FixedUpdate,
                (
                    play_timelines.run_if(
                        resource_exists::<AbilitySfx>.and(resource_exists::<AbilityMeshes>),
                    ),
                    tick_clocks,
                )
                    .chain()
                    .in_set(TimelineSet),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(start: (i32, i32), ticks: &[u32]) -> Recording {
        Recording {
            start: IVec2::new(start.0, start.1),
            events: ticks
                .iter()
                .map(|&tick| TimelineEvent {
                    tick,
                    action: Action::Move(IVec2::X),
                })
                .collect(),
        }
    }

    fn two_ghosts() -> (Entity, Entity) {
        let mut world = World::new();
        (world.spawn_empty().id(), world.spawn_empty().id())
    }

    #[test]
    fn fold_keeps_tick_order_and_is_stable_for_equal_ticks() {
        let (a, b) = two_ghosts();
        let mut tl = ArenaTimeline::default();
        fold(&mut tl, a, &rec((0, 0), &[5, 10]));
        fold(&mut tl, b, &rec((1, 1), &[5, 7]));
        let ticks: Vec<u32> = tl.events.iter().map(|e| e.tick).collect();
        assert_eq!(ticks, [5, 5, 7, 10]);
        // Stable: at tick 5 the earlier-folded ghost keeps priority.
        assert_eq!(tl.events[0].ghost, a);
        assert_eq!(tl.events[1].ghost, b);
    }

    #[test]
    fn refolding_the_same_ghost_is_idempotent() {
        let (a, b) = two_ghosts();
        let mut tl = ArenaTimeline::default();
        let staff = rec((0, 0), &[5, 10]);
        fold(&mut tl, a, &staff);
        fold(&mut tl, b, &rec((1, 1), &[7]));
        fold(&mut tl, a, &staff);
        assert_eq!(tl.events.len(), 3);
        assert_eq!(tl.events.iter().filter(|e| e.ghost == a).count(), 2);
        assert_eq!(tl.cursor, 0);
    }

    #[test]
    fn unfold_removes_exactly_one_ghost_and_resyncs_the_cursor() {
        let (a, b) = two_ghosts();
        let mut tl = ArenaTimeline::default();
        fold(&mut tl, a, &rec((0, 0), &[5, 10]));
        fold(&mut tl, b, &rec((1, 1), &[7]));
        // Played through tick 8: events 5 and 7 already applied.
        tl.cursor = 2;
        unfold(&mut tl, a, 8);
        assert_eq!(tl.events.len(), 1);
        assert_eq!(tl.events[0].ghost, b);
        // b's tick-7 event stays behind the cursor — playback continues, no replay.
        assert_eq!(tl.cursor, 1);
    }

    #[test]
    fn clock_wraps_to_zero_at_the_cycle_boundary() {
        assert_eq!(advance(0), 1);
        assert_eq!(advance(CYCLE_TICKS - 2), CYCLE_TICKS - 1);
        assert_eq!(advance(CYCLE_TICKS - 1), 0);
    }

    #[test]
    fn seek_window_includes_the_target_tick_and_syncs_the_cursor() {
        let (a, b) = two_ghosts();
        let mut tl = ArenaTimeline::default();
        fold(&mut tl, a, &rec((0, 0), &[0, 3, 7]));
        fold(&mut tl, b, &rec((1, 1), &[3]));
        // Seek to 3: events at 0 and BOTH at 3 have played; cursor sits past them.
        let (window, cursor) = seek_window(&tl.events, 3);
        assert_eq!(window.iter().map(|e| e.tick).collect::<Vec<_>>(), [0, 3, 3]);
        assert_eq!(cursor, 3);
        // Seek to 2: only tick 0; seeking past the end replays everything.
        assert_eq!(seek_window(&tl.events, 2).1, 1);
        assert_eq!(seek_window(&tl.events, 9999).1, tl.events.len());
        // Seek to 0 still plays tick-0 events (state AT a tick includes its events).
        assert_eq!(seek_window(&tl.events, 0).1, 1);
    }

    #[test]
    fn partial_recording_plays_through_then_idles() {
        let (a, _) = two_ghosts();
        let mut tl = ArenaTimeline::default();
        fold(&mut tl, a, &rec((0, 0), &[0, 3]));
        // Walk the cursor the way play_timelines does, far past the last event.
        for tick in 0..10u32 {
            while let Some(e) = tl.events.get(tl.cursor) {
                if e.tick > tick {
                    break;
                }
                tl.cursor = tl.cursor.strict_add(1);
            }
        }
        assert_eq!(tl.cursor, tl.events.len());
    }
}
