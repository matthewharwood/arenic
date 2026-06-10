//! Hero **travel**: arrow-key tile steps, edge-walking between adjacent arenas,
//! and the travel-adjacent modals (ghost break-out, the mid-recording interrupt,
//! replay-on-return). Split out of `intro_scene` along the movement fault line —
//! the scene owns what the world *is*; this module owns how the selected hero
//! *moves through it* (RULEBOOK → Travel: Leaving and Returning).

use arenic_game::Boss;
use arenic_game::arena::Arena;
use arenic_game::default_font::MonoFont;
use arenic_game::grid::{MAX_COL, MAX_ROW, TileMover, arrow_delta, arrow_pressed};
use arenic_game::timeline::{Action, ArenaClock, Ghost, RecordingLibrary, TimelineEvent};
use bevy::prelude::*;

use crate::intro_scene::{CurrentArena, Selected};
use crate::modal::{Choice, ModalLatch, no_modal, spawn_modal};
use crate::recording::{
    DraftTimeline, PendingWalk, RecordingState, is_idle, no_pending_walk, not_counting_down,
};
use crate::states::{AppState, not_tile_editing};

/// Moves the SELECTED puck one tile per arrow-key press. Only the selected puck
/// responds — the others hold position. Three special cases (RULEBOOK):
/// a selected **ghost** never moves — an arrow press opens the *Break out?* modal;
/// stepping past the arena edge **while recording** opens the *Like the
/// recording?* interrupt modal; the same step while idle **edge-walks** into the
/// adjacent arena. A possessed [`Boss`] never edge-walks at all — its arena IS
/// its world, so an edge step clamps exactly like the outer border.
fn move_selected(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<RecordingState>,
    mut latch: ResMut<ModalLatch>,
    mut draft: ResMut<DraftTimeline>,
    mono: Res<MonoFont>,
    selected: Single<
        (
            Entity,
            &mut TileMover,
            &mut Transform,
            &ChildOf,
            Has<Ghost>,
            Has<Boss>,
            &RecordingLibrary,
        ),
        With<Selected>,
    >,
    arena_roots: Query<(Entity, &Arena)>,
    mut clocks: Query<&mut ArenaClock>,
    mut current: ResMut<CurrentArena>,
) -> Result {
    let delta = arrow_delta(&keys);
    let (hero, mut mover, mut transform, child_of, is_ghost, is_boss, library) =
        selected.into_inner();
    let arena_entity = child_of.parent();
    let (_, arena) = arena_roots.get(arena_entity)?;
    let recording = matches!(*state, RecordingState::Recording);

    if is_ghost {
        // Arrows never drive a ghost — ask before pulling it out of the score.
        let mut clock = clocks.get_mut(arena_entity)?;
        let _ = spawn_modal(
            &mut commands,
            &mut latch,
            &mut clock,
            &arena.theme().palette(),
            &mono,
            "Break out of the recording?",
            "Take control unfolds this hero - the arena keeps playing",
            &[
                ("Take control", Choice::TakeControl),
                ("Restart arena", Choice::RestartArena),
                ("Cancel", Choice::Cancel),
            ],
            2,
        );
        return Ok(());
    }

    let target = IVec2::new(mover.col.strict_add(delta.x), mover.row.strict_add(delta.y));
    let inside = (0..=MAX_COL).contains(&target.x) && (0..=MAX_ROW).contains(&target.y);
    let crossing = if inside || is_boss {
        None
    } else {
        adjacent_entry(arena.index(), mover.col, mover.row, delta)
    };

    match crossing {
        None => {
            // In bounds — or clamped at the outer world border, exactly like
            // replay clamps. The step APPLIES, so it is what the draft records:
            // a blocked step (the interrupt arm below) never leaves a phantom
            // event in the committed staff.
            mover.step(&mut transform, delta);
            if recording {
                let tick = clocks.get(arena_entity)?.tick;
                draft.events.push(TimelineEvent {
                    tick,
                    action: Action::Move(delta),
                });
            }
        }
        Some(_) if recording => {
            // Mid-recording edge step into a real neighbour: confirm before
            // anything is lost. The PendingWalk only exists if the modal really
            // opened — a stranded one would suppress movement forever.
            let mut clock = clocks.get_mut(arena_entity)?;
            if spawn_modal(
                &mut commands,
                &mut latch,
                &mut clock,
                &arena.theme().palette(),
                &mono,
                "Like the recording?",
                "Moving out of the arena cancels it",
                &[
                    ("Continue recording", Choice::Cancel),
                    ("Cancel & walk out", Choice::DiscardAndWalk),
                    ("Commit & stay", Choice::Commit),
                ],
                0,
            ) {
                commands.insert_resource(PendingWalk(delta));
            }
        }
        Some(entry) => {
            edge_walk(
                &mut commands,
                &mut latch,
                &mono,
                hero,
                &mut mover,
                &mut transform,
                entry,
                library,
                &arena_roots,
                &mut clocks,
                &mut current,
            )?;
        }
    }
    Ok(())
}

/// Where stepping `delta` from `(col, row)` lands when it crosses the edge of
/// arena `index`: `(adjacent arena, entry col, entry row)` on the opposite edge,
/// or `None` at the outer border of the 3×3 world (movement clamps; no wrap).
fn adjacent_entry(index: usize, col: i32, row: i32, delta: IVec2) -> Option<(usize, i32, i32)> {
    let target = IVec2::new(col.strict_add(delta.x), row.strict_add(delta.y));
    let (grid_col, grid_row) = (index % 3, index / 3);
    if target.x > MAX_COL && grid_col < 2 {
        Some((index.strict_add(1), 0, target.y.clamp(0, MAX_ROW)))
    } else if target.x < 0 && grid_col > 0 {
        Some((index.strict_sub(1), MAX_COL, target.y.clamp(0, MAX_ROW)))
    } else if target.y > MAX_ROW && grid_row > 0 {
        // Within an arena +row is up; the arena above sits at index − 3.
        Some((index.strict_sub(3), target.x.clamp(0, MAX_COL), 0))
    } else if target.y < 0 && grid_row < 2 {
        Some((index.strict_add(3), target.x.clamp(0, MAX_COL), MAX_ROW))
    } else {
        None
    }
}

/// Re-parents the hero into the adjacent arena at `entry` (from
/// [`adjacent_entry`]), refocuses [`CurrentArena`] (the camera follows when
/// zoomed in; the ring catches up via `intro_scene::follow_selected`), and — if
/// the hero has a staff cached for the new arena — asks whether to fold it back
/// in (RULEBOOK → Travel: Leaving and Returning).
fn edge_walk(
    commands: &mut Commands,
    latch: &mut ModalLatch,
    mono: &MonoFont,
    hero: Entity,
    mover: &mut TileMover,
    transform: &mut Transform,
    entry: (usize, i32, i32),
    library: &RecordingLibrary,
    arena_roots: &Query<(Entity, &Arena)>,
    clocks: &mut Query<&mut ArenaClock>,
    current: &mut CurrentArena,
) -> Result {
    let (to_index, col, row) = entry;
    let (to_arena, arena) = arena_roots
        .iter()
        .find(|(_, a)| a.index() == to_index)
        .ok_or("invariant: adjacent_entry returned an arena index with no spawned root")?;
    mover.snap_to(transform, col, row);
    commands.entity(hero).insert(ChildOf(to_arena));
    current.0 = to_index;

    // Returning with a staff for this arena? Offer to fold it back in.
    if library.0.contains_key(arena) {
        let mut clock = clocks.get_mut(to_arena)?;
        let _ = spawn_modal(
            commands,
            latch,
            &mut clock,
            &arena.theme().palette(),
            mono,
            "Replay this hero's staff here?",
            "Replay folds the cached recording back in and restarts the arena",
            &[
                ("Replay previous", Choice::ReplayPrevious),
                ("Continue without", Choice::Cancel),
            ],
            1,
        );
    }
    Ok(())
}

/// Performs the edge-walk the player confirmed from the interrupt modal
/// ("Cancel & walk out") — the draft was discarded by `recording::handle_choice`;
/// the stashed step happens now.
fn perform_pending_walk(
    mut commands: Commands,
    pending: Res<PendingWalk>,
    mut latch: ResMut<ModalLatch>,
    mono: Res<MonoFont>,
    selected: Single<
        (
            Entity,
            &mut TileMover,
            &mut Transform,
            &ChildOf,
            &RecordingLibrary,
        ),
        With<Selected>,
    >,
    arena_roots: Query<(Entity, &Arena)>,
    mut clocks: Query<&mut ArenaClock>,
    mut current: ResMut<CurrentArena>,
) -> Result {
    let delta = pending.0;
    commands.remove_resource::<PendingWalk>();
    let (hero, mut mover, mut transform, child_of, library) = selected.into_inner();
    let (_, arena) = arena_roots.get(child_of.parent())?;
    let entry = adjacent_entry(arena.index(), mover.col, mover.row, delta)
        .ok_or("invariant: PendingWalk held a step that no longer crosses an arena edge")?;
    edge_walk(
        &mut commands,
        &mut latch,
        &mono,
        hero,
        &mut mover,
        &mut transform,
        entry,
        library,
        &arena_roots,
        &mut clocks,
        &mut current,
    )
}

/// Adds selected-hero movement + edge-walking to the intro scene.
pub(crate) struct TravelPlugin;

impl Plugin for TravelPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                // A live PendingWalk suppresses arrow movement for the one frame
                // perform_pending_walk needs — otherwise both could move the hero
                // in the same frame off a stale ChildOf. The author tile editor
                // borrows the arrows for its cursor while open.
                move_selected.run_if(
                    arrow_pressed
                        .and(no_modal)
                        .and(not_counting_down)
                        .and(no_pending_walk)
                        .and(not_tile_editing),
                ),
                perform_pending_walk
                    .run_if(resource_exists::<PendingWalk>.and(no_modal).and(is_idle)),
            )
                .run_if(in_state(AppState::Intro)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_walks_enter_the_adjacent_arena_on_the_opposite_edge() {
        // Guild House (1) → right past the last column → Sanctum (2), col 0.
        assert_eq!(
            adjacent_entry(1, MAX_COL, 15, IVec2::new(1, 0)),
            Some((2, 0, 15))
        );
        // Guild House (1) → left past column 0 → Labyrinth (0), MAX_COL.
        assert_eq!(
            adjacent_entry(1, 0, 15, IVec2::new(-1, 0)),
            Some((0, MAX_COL, 15))
        );
        // Bastion (4) → up past the top row → Guild House (1), row 0.
        assert_eq!(
            adjacent_entry(4, 30, MAX_ROW, IVec2::new(0, 1)),
            Some((1, 30, 0))
        );
        // Guild House (1) → down past row 0 → Bastion (4), MAX_ROW.
        assert_eq!(
            adjacent_entry(1, 30, 0, IVec2::new(0, -1)),
            Some((4, 30, MAX_ROW))
        );
    }

    #[test]
    fn the_outer_border_does_not_wrap() {
        assert_eq!(adjacent_entry(0, 0, 15, IVec2::new(-1, 0)), None);
        assert_eq!(adjacent_entry(1, 30, MAX_ROW, IVec2::new(0, 1)), None);
        assert_eq!(adjacent_entry(8, MAX_COL, 15, IVec2::new(1, 0)), None);
        assert_eq!(adjacent_entry(7, 30, 0, IVec2::new(0, -1)), None);
    }

    #[test]
    fn steps_inside_the_arena_are_not_edge_walks() {
        assert_eq!(adjacent_entry(4, 10, 10, IVec2::new(1, 0)), None);
    }
}
