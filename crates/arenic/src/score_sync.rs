//! **Score sync** — files → world. Loads the newest published [`LayerStack`]
//! (`assets/encounters/<arena>/<difficulty>/layers.vNNNN.ron`, with the legacy
//! `boss.vNNNN.ron` + `tiles.vNNNN.ron` pair as a read-only fallback — see
//! `arenic_game::layer::read_stack`) and keeps the running world on it:
//!
//! - Every effective **recording layer** folds into the arena's master
//!   timeline ([`fold_stack`]) — the boss layer binds to the arena's [`Boss`]
//!   root and replays as a [`Ghost`]; minion layers bind once ARE-44 lands.
//! - The merged **tile schedule** ([`LayerStack::merged_tiles`], top layer
//!   wins) rides the arena root as a [`TileScript`];
//!   [`play_tile_scripts`] applies it to the board every tick.
//! - The loaded stack itself rides the root as an [`ArenaStack`] — the author
//!   tool edits its draft; this sync NEVER overwrites a dirty draft for the
//!   active difficulty.
//!
//! The disk check re-runs at every cycle wrap (and on difficulty swaps), so
//! the game and the author tool run **in tandem**: publish a new stack in the
//! author (`W`), and the game picks it up when its cycle restarts; roll back
//! by deleting the newest file. Filesystem access is native-only — the web
//! build ships none of it (a baked manifest is future work).

use arenic_game::Difficulty;
use arenic_game::arena::Arena;
use arenic_game::tile::{ArenaTiles, TileBoard};
use arenic_game::tile_script::{AppliedCell, TileScript, desired};
use arenic_game::timeline::{ArenaClock, TimelineSet};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;

use crate::states::AppState;

#[cfg(not(target_arch = "wasm32"))]
use arenic_game::Boss;
#[cfg(not(target_arch = "wasm32"))]
use arenic_game::encounter::{ActiveDifficulty, encounter_dir};
#[cfg(not(target_arch = "wasm32"))]
use arenic_game::grid::TileMover;
#[cfg(not(target_arch = "wasm32"))]
use arenic_game::layer::{ArenaStack, LayerKind, LayerStack, fold_stack, read_stack};
#[cfg(not(target_arch = "wasm32"))]
use arenic_game::timeline::{ArenaTimeline, Ghost, RecordingLibrary, restart, unfold};

#[cfg(not(target_arch = "wasm32"))]
use crate::intro_scene::Selected;
#[cfg(not(target_arch = "wasm32"))]
use crate::recording::{RecordingState, snap_arena_ghosts};

/// The stack version currently folded per arena (`None` = nothing published),
/// tagged with the difficulty it was loaded FOR. [`sync_scores`] diffs disk
/// against this ledger; the author's publish bumps it so a fresh `W` isn't
/// immediately re-folded at the wrap.
#[derive(Resource, Default)]
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub(crate) struct LoadedScores(pub(crate) HashMap<u8, Option<(Difficulty, u32)>>);

/// Re-entering the intro respawns every arena fresh, so the ledger must forget
/// the previous session — a stale entry would skip the re-fold.
#[cfg(not(target_arch = "wasm32"))]
fn reset_loaded(mut loaded: ResMut<LoadedScores>) {
    loaded.0.clear();
}

/// Checks the disk for a newer published stack and swaps it in. Runs cheaply:
/// only on an arena's first sight, at its cycle wrap (`tick == 0`, unpaused —
/// a countdown hold or modal freeze is not a wrap), or when the active
/// difficulty changes (each difficulty owns its own timelines).
#[cfg(not(target_arch = "wasm32"))]
fn sync_scores(
    mut commands: Commands,
    difficulty: Res<ActiveDifficulty>,
    recording: Res<RecordingState>,
    mut loaded: ResMut<LoadedScores>,
    mut arenas: Query<(
        Entity,
        &Arena,
        &mut ArenaClock,
        &mut ArenaTimeline,
        Option<&mut TileScript>,
        Option<&ArenaStack>,
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
    mut board: TileBoard,
) {
    for (arena_entity, arena_id, mut clock, mut timeline, tile_script, arena_stack) in &mut arenas {
        let index = arena_id.index() as u8;
        let fresh = !loaded.0.contains_key(&index);
        if !(fresh || difficulty.is_changed() || (clock.tick == 0 && !clock.paused)) {
            continue;
        }
        let ledger = *loaded.0.entry(index).or_default();

        // The author's unsaved draft for THIS difficulty is never the sync's
        // to destroy — publish (`W`) re-baselines it and bumps the ledger.
        // Orphan drafts left on another difficulty's stack lose the swap.
        if arena_stack.is_some_and(|stack| stack.dirty())
            && ledger.is_none_or(|(d, _)| d == difficulty.0)
        {
            warn!(
                "{}: unsaved layer edits — press W to publish them",
                arena_id.name()
            );
            continue;
        }

        let dir = encounter_dir(*arena_id, difficulty.0);
        let boss = {
            let p0 = movers.p0();
            p0.iter()
                .find(|(_, child_of, ..)| child_of.parent() == arena_entity)
                .map(|(entity, .., has_ghost, possessed)| (entity, has_ghost, possessed))
        };

        // Parse + validate BEFORE touching the world. A same-difficulty newer
        // version failing keeps the previous stack playing (delete the bad
        // file to roll back); another difficulty's stack must NOT survive a
        // swap, so a failed read there unfolds instead.
        let incoming = match read_stack(&dir, *arena_id, difficulty.0) {
            Ok(Some((version, stack))) => Some(Some((version, stack))),
            Ok(None) => Some(None),
            Err(err) => {
                warn!("{}: unreadable encounter stack: {err}", arena_id.name());
                ledger
                    .is_some_and(|(d, _)| d != difficulty.0)
                    .then_some(None)
            }
        };
        let Some(swap) = incoming else {
            continue;
        };
        let want = swap.as_ref().map(|(version, _)| (difficulty.0, *version));

        // Re-fold when the version moved — or when the loaded stack has a
        // boss layer but its ghost is gone (a discarded re-record left the
        // boss unfolded; the wrap puts the committed take back). The rescue
        // stands down while the boss is possessed (`Selected`) or a recording
        // is in flight: a break-out or a re-record's countdown unfolds the
        // boss INTENTIONALLY. Releasing the boss (`B`) re-arms it.
        let stack_has_boss = arena_stack.is_some_and(|s| {
            s.stack
                .effective()
                .any(|layer| matches!(layer.kind, LayerKind::Boss(_)))
        });
        let rescue = boss.is_some_and(|(_, has_ghost, possessed)| {
            stack_has_boss && !has_ghost && !possessed && matches!(*recording, RecordingState::Idle)
        });
        if ledger == want && !rescue {
            continue;
        }

        // --- Revert what the outgoing stack held -----------------------------
        if let Some(mut script) = tile_script {
            for (&(col, row), cell) in &script.applied {
                board.set(index as usize, col as usize, row as usize, cell.prior);
            }
            script.applied.clear();
            script.keyframes.clear();
        }
        if let Some((boss_entity, ..)) = boss {
            unfold(&mut timeline, boss_entity, clock.tick);
        }

        match swap {
            Some((version, stack)) => {
                apply_stack(
                    &mut commands,
                    arena_entity,
                    *arena_id,
                    &stack,
                    boss.map(|(entity, ..)| entity),
                    &mut clock,
                    &mut timeline,
                    &mut movers,
                );
                commands
                    .entity(arena_entity)
                    .insert(ArenaStack::loaded(stack, version));
                loaded.0.insert(index, Some((difficulty.0, version)));
                info!("{}: encounter stack v{version} folded", arena_id.name());
            }
            None => {
                // Nothing published — the boss returns to a static piece, and
                // its cached staff goes too ("Replay previous" must never
                // resurrect a take this difficulty disowned).
                if let Some((boss_entity, ..)) = boss {
                    commands.entity(boss_entity).remove::<Ghost>();
                    if let Ok((.., mut library, _, _)) = movers.p0().get_mut(boss_entity) {
                        library.0.remove(arena_id);
                    }
                }
                restart(&mut clock, &mut timeline);
                snap_arena_ghosts(
                    arena_entity,
                    &mut movers.p1(),
                    boss.map(|(entity, ..)| entity),
                );
                commands
                    .entity(arena_entity)
                    .remove::<(TileScript, ArenaStack)>();
                loaded.0.insert(index, None);
                info!("{}: encounter stack removed — unfolded", arena_id.name());
            }
        }
    }
}

/// Folds `stack` into the world: recording layers bind (boss layer → the
/// arena's boss root; minion layers skip until ARE-44 spawns them), the
/// merged tile schedule becomes the live [`TileScript`], and the arena
/// restarts so playback begins at tick 0.
#[cfg(not(target_arch = "wasm32"))]
fn apply_stack(
    commands: &mut Commands,
    arena_entity: Entity,
    arena_id: Arena,
    stack: &LayerStack,
    boss: Option<Entity>,
    clock: &mut ArenaClock,
    timeline: &mut ArenaTimeline,
    movers: &mut ParamSet<(
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
) {
    // Bind the FIRST effective boss layer to the arena's boss root.
    let boss_layer = stack.effective().find_map(|layer| match &layer.kind {
        LayerKind::Boss(recording) => Some((layer.id, recording.clone())),
        _ => None,
    });
    let bindings: Vec<_> = match (boss, &boss_layer) {
        (Some(entity), Some((id, _))) => vec![(*id, entity)],
        _ => Vec::new(),
    };
    fold_stack(stack, &bindings, timeline);
    restart(clock, timeline);
    snap_arena_ghosts(arena_entity, &mut movers.p1(), boss);

    if let Some(boss_entity) = boss {
        match boss_layer {
            Some((_, recording)) => {
                // The boss's Ghost may be brand new (commands defer), so it is
                // posed by hand; caching the take makes `R → Replay previous`
                // work after a discard, exactly like a hero's staff.
                if let Ok((_, _, mut mover, mut transform, mut library, _, _)) =
                    movers.p0().get_mut(boss_entity)
                {
                    mover.snap_to(&mut transform, recording.start.x, recording.start.y);
                    library.0.insert(arena_id, recording.clone());
                }
                commands.entity(boss_entity).insert(Ghost {
                    start: recording.start,
                });
            }
            None => {
                commands.entity(boss_entity).remove::<Ghost>();
                if let Ok((.., mut library, _, _)) = movers.p0().get_mut(boss_entity) {
                    library.0.remove(&arena_id);
                }
            }
        }
    }

    commands.entity(arena_entity).insert(TileScript {
        keyframes: stack.merged_tiles(),
        applied: default(),
    });
}

/// Applies every arena's [`TileScript`] to the board at its clock's tick: the
/// scripted cells take their keyframed kind; cells the script lets go of revert
/// to whatever they wore before the script claimed them (a manually-flipped
/// tile is not the script's to undo). Runs even while a clock is paused, so the
/// author tool's scrubbing previews instantly; an unchanged diff is a no-op.
fn play_tile_scripts(
    mut arenas: Query<(&Arena, &ArenaClock, &mut TileScript)>,
    mut board: TileBoard,
) {
    for (arena, clock, mut script) in &mut arenas {
        let want = desired(&script.keyframes, clock.tick);
        // Fast path: the script already holds exactly the wanted cells.
        if want.len() == script.applied.len()
            && want
                .iter()
                .all(|(cell, kind)| script.applied.get(cell).is_some_and(|c| c.kind == *kind))
        {
            continue;
        }
        let index = arena.index();
        // Released cells go back to their pre-script kind.
        script.applied.retain(|&(col, row), cell| {
            let keep = want.contains_key(&(col, row));
            if !keep {
                board.set(index, col as usize, row as usize, cell.prior);
            }
            keep
        });
        for (&cell, &kind) in &want {
            let prior = match script.applied.get(&cell) {
                Some(applied) if applied.kind == kind => continue,
                Some(applied) => applied.prior,
                None => board
                    .kind(index, cell.0 as usize, cell.1 as usize)
                    .unwrap_or_default(),
            };
            board.set(index, cell.0 as usize, cell.1 as usize, kind);
            script.applied.insert(cell, AppliedCell { kind, prior });
        }
    }
}

/// File → world sync for the published layer stacks, plus tile-schedule
/// playback. Native loads from disk; on wasm only the (inert) playback systems
/// ship.
pub(crate) struct ScoreSyncPlugin;

impl Plugin for ScoreSyncPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LoadedScores>().add_systems(
            FixedUpdate,
            play_tile_scripts
                .after(TimelineSet)
                .run_if(in_state(AppState::Intro).and(resource_exists::<ArenaTiles>)),
        );
        #[cfg(not(target_arch = "wasm32"))]
        app.add_systems(OnEnter(AppState::Intro), reset_loaded)
            .add_systems(
                FixedUpdate,
                sync_scores
                    .before(TimelineSet)
                    .run_if(in_state(AppState::Intro).and(resource_exists::<ArenaTiles>)),
            );
    }
}
