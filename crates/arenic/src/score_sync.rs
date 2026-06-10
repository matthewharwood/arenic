//! **Score sync** — files → world. Loads the newest versioned encounter files
//! (`assets/encounters/<arena>/<difficulty>/boss.vNNNN.ron` / `tiles.vNNNN.ron`,
//! see `arenic_game::encounter`) and keeps the running world on them:
//!
//! - The newest **boss score** folds into its arena's master timeline as a
//!   [`Ghost`] on the arena's [`Boss`] root, so the boss replays its authored
//!   take every cycle exactly like a hero ghost.
//! - The newest **tile script** rides the arena root as a [`TileScript`];
//!   [`play_tile_scripts`] applies it to the board every tick.
//!
//! The disk check re-runs at every cycle wrap (and on difficulty swaps), so the
//! game and the author tool can run **in tandem**: commit a new version in the
//! author, and the game picks it up when its cycle restarts; roll back by
//! deleting the newest file. Filesystem access is native-only — the web build
//! ships none of it (a baked manifest is future work).

use arenic_game::Difficulty;
use arenic_game::arena::Arena;
use arenic_game::tile::{ArenaTiles, TileBoard, TileKind};
use arenic_game::tile_script::{TileScript, desired};
use arenic_game::timeline::{ArenaClock, TimelineSet};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;

use crate::states::AppState;

#[cfg(not(target_arch = "wasm32"))]
use arenic_game::Boss;
#[cfg(not(target_arch = "wasm32"))]
use arenic_game::encounter::{
    ActiveDifficulty, BOSS_PREFIX, BossScoreFile, encounter_dir, latest_version, read_ron,
};
#[cfg(not(target_arch = "wasm32"))]
use arenic_game::grid::TileMover;
#[cfg(not(target_arch = "wasm32"))]
use arenic_game::tile_script::{TILES_PREFIX, TileScriptFile};
#[cfg(not(target_arch = "wasm32"))]
use arenic_game::timeline::{
    ArenaTimeline, Ghost, RecordingLibrary, fold, restart, snap_ghost, unfold,
};

/// The score versions currently folded into the world, keyed by arena index
/// (`None` = no file). [`sync_scores`] diffs disk against this ledger; the
/// author feature bumps it when it writes, so a fresh commit isn't immediately
/// re-folded at the wrap.
#[derive(Resource, Default)]
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub(crate) struct LoadedScores(pub(crate) HashMap<u8, ScoreVersions>);

/// One arena's loaded `(boss, tiles)` file versions, each tagged with the
/// difficulty it was loaded FOR — two difficulties can both sit at `v1`, so a
/// bare version number could not tell "same file" from "same number on the
/// other difficulty's timeline".
#[derive(Clone, Copy, Default)]
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub(crate) struct ScoreVersions {
    pub(crate) boss: Option<(Difficulty, u32)>,
    pub(crate) tiles: Option<(Difficulty, u32)>,
}

/// Re-entering the intro respawns every arena fresh, so the ledger must forget
/// the previous session — a stale entry would skip the re-fold.
#[cfg(not(target_arch = "wasm32"))]
fn reset_loaded(mut loaded: ResMut<LoadedScores>) {
    loaded.0.clear();
}

/// Checks the disk for newer score versions and swaps them in. Runs cheaply:
/// only on an arena's first sight, at its cycle wrap (`tick == 0`, unpaused —
/// a countdown hold or modal freeze is not a wrap), or when the active
/// difficulty changes (each difficulty owns its own timelines).
#[cfg(not(target_arch = "wasm32"))]
fn sync_scores(
    mut commands: Commands,
    difficulty: Res<ActiveDifficulty>,
    mut loaded: ResMut<LoadedScores>,
    mut arenas: Query<(
        Entity,
        &Arena,
        &mut ArenaClock,
        &mut ArenaTimeline,
        Option<&mut TileScript>,
    )>,
    mut movers: ParamSet<(
        Query<
            (
                Entity,
                &ChildOf,
                &mut TileMover,
                &mut Transform,
                &mut RecordingLibrary,
            ),
            With<Boss>,
        >,
        Query<(Entity, &Ghost, &ChildOf, &mut TileMover, &mut Transform)>,
    )>,
    mut board: TileBoard,
) {
    for (arena_entity, arena_id, mut clock, mut timeline, tile_script) in &mut arenas {
        let index = arena_id.index() as u8;
        let fresh = !loaded.0.contains_key(&index);
        if !(fresh || difficulty.is_changed() || (clock.tick == 0 && !clock.paused)) {
            continue;
        }
        let dir = encounter_dir(*arena_id, difficulty.0);

        // --- Boss score: fold the newest version; unfold when rolled away ---
        let latest = latest_version(&dir, BOSS_PREFIX);
        let want = latest.as_ref().map(|&(v, _)| (difficulty.0, v));
        let boss = {
            let p0 = movers.p0();
            p0.iter()
                .find(|(_, child_of, ..)| child_of.parent() == arena_entity)
                .map(|(entity, ..)| entity)
        };
        if let Some(boss) = boss
            && loaded.0.entry(index).or_default().boss != want
        {
            match &latest {
                Some((version, path)) => match read_ron::<BossScoreFile>(path) {
                    Ok(file) => {
                        let recording = file.recording();
                        fold(&mut timeline, boss, &recording);
                        restart(&mut clock, &mut timeline);
                        // The arena's other ghosts rewind to their starts; the
                        // boss's Ghost may be brand new (commands defer), so it
                        // is posed by hand. Caching the take in the boss's
                        // library makes `R → Replay previous` work after a
                        // discard, exactly like a hero's staff.
                        snap_others(&mut movers.p1(), arena_entity, boss);
                        if let Ok((_, _, mut mover, mut transform, mut library)) =
                            movers.p0().get_mut(boss)
                        {
                            mover.snap_to(&mut transform, recording.start.x, recording.start.y);
                            library.0.insert(index, recording.clone());
                        }
                        commands.entity(boss).insert(Ghost {
                            start: recording.start,
                        });
                        loaded.0.entry(index).or_default().boss = Some((difficulty.0, *version));
                        info!(
                            "{}: boss score v{version} folded ({})",
                            arena_id.name(),
                            path.display()
                        );
                    }
                    Err(err) => warn!(
                        "{}: unreadable boss score {}: {err}",
                        arena_id.name(),
                        path.display()
                    ),
                },
                None => {
                    // Every version deleted — the boss returns to a static piece.
                    unfold(&mut timeline, boss, clock.tick);
                    commands.entity(boss).remove::<Ghost>();
                    restart(&mut clock, &mut timeline);
                    snap_others(&mut movers.p1(), arena_entity, boss);
                    loaded.0.entry(index).or_default().boss = None;
                    info!("{}: boss score removed — boss unfolded", arena_id.name());
                }
            }
        }

        // --- Tile script: swap in the newest version, reverting the old one ---
        let latest = latest_version(&dir, TILES_PREFIX);
        let want = latest.as_ref().map(|&(v, _)| (difficulty.0, v));
        if loaded.0.entry(index).or_default().tiles != want {
            // Whatever the outgoing script held away from Normal reverts first.
            if let Some(mut script) = tile_script {
                for &(col, row) in script.applied.keys() {
                    board.set(index as usize, col as usize, row as usize, TileKind::Normal);
                }
                script.applied.clear();
                script.keyframes.clear();
            }
            match &latest {
                Some((version, path)) => match read_ron::<TileScriptFile>(path) {
                    Ok(file) => {
                        commands.entity(arena_entity).insert(TileScript {
                            keyframes: file.keyframes,
                            applied: default(),
                        });
                        loaded.0.entry(index).or_default().tiles = Some((difficulty.0, *version));
                        info!(
                            "{}: tile script v{version} loaded ({})",
                            arena_id.name(),
                            path.display()
                        );
                    }
                    Err(err) => warn!(
                        "{}: unreadable tile script {}: {err}",
                        arena_id.name(),
                        path.display()
                    ),
                },
                None => {
                    commands.entity(arena_entity).remove::<TileScript>();
                    loaded.0.entry(index).or_default().tiles = None;
                }
            }
        }
    }
}

/// Snaps every ghost of `arena` except `boss` back to its recorded start (the
/// boss is posed by hand, since its `Ghost` may still be in the command queue).
#[cfg(not(target_arch = "wasm32"))]
fn snap_others(
    ghosts: &mut Query<(Entity, &Ghost, &ChildOf, &mut TileMover, &mut Transform)>,
    arena: Entity,
    boss: Entity,
) {
    for (entity, ghost, child_of, mut mover, mut transform) in ghosts.iter_mut() {
        if child_of.parent() == arena && entity != boss {
            snap_ghost(ghost, &mut mover, &mut transform);
        }
    }
}

/// Applies every arena's [`TileScript`] to the board at its clock's tick: the
/// scripted cells take their keyframed kind; cells the script let go of revert
/// to `Normal` (and ONLY those — a manually-flipped tile is not the script's to
/// undo). Runs even while a clock is paused, so the author tool's scrubbing
/// previews instantly; an unchanged board diff is a no-op.
fn play_tile_scripts(
    mut arenas: Query<(&Arena, &ArenaClock, &mut TileScript)>,
    mut board: TileBoard,
) {
    for (arena, clock, mut script) in &mut arenas {
        let want = desired(&script.keyframes, clock.tick);
        if want == script.applied {
            continue;
        }
        for &(col, row) in script.applied.keys() {
            if !want.contains_key(&(col, row)) {
                board.set(arena.index(), col as usize, row as usize, TileKind::Normal);
            }
        }
        for (&(col, row), &kind) in &want {
            board.set(arena.index(), col as usize, row as usize, kind);
        }
        script.applied = want;
    }
}

/// File → world sync for the versioned encounter scores, plus tile-script
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
