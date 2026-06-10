//! Board geometry for the 3×3 arena grid (`_docs/UNITS_AND_SCALE.md`).
//!
//! One arena is `66 × 31` tiles of `0.25` units = `16.5 × 7.75` units; nine of them
//! tile a `3 × 3` grid, indexed `0..9` row-major. Rows grow in **−Y** (per §4.1), so
//! arena `i` sits at world offset `(col·16.5, −row·7.75, 0)`. Characters live as
//! children of their arena and hold **local** tile transforms, so the same tile
//! coordinate is correct in any arena.

use bevy::prelude::*;

/// Tiles across one arena.
pub const GRID_W: i32 = 66;
/// Tiles down one arena.
pub const GRID_H: i32 = 31;
/// Units per tile (`0.25 = 2⁻²`, exactly representable → drift-free snapping).
pub const TILE: f32 = 0.25;
/// One arena's width in units (`16.5`).
pub const ARENA_W: f32 = GRID_W as f32 * TILE;
/// One arena's height in units (`7.75`).
pub const ARENA_H: f32 = GRID_H as f32 * TILE;
/// The 3×3 grid holds nine arenas.
pub const ARENAS: usize = 9;
/// Largest valid tile column (`GRID_W - 1`).
pub const MAX_COL: i32 = GRID_W - 1;
/// Largest valid tile row (`GRID_H - 1`).
pub const MAX_ROW: i32 = GRID_H - 1;

/// World offset of arena `index` (`0..9`) in the 3×3 grid. Columns grow +X, rows
/// grow −Y, so the grid descends as the index increases (§4.1 / "Known caveats").
pub fn arena_offset(index: usize) -> Vec2 {
    let col = (index % 3) as f32;
    let row = (index / 3) as f32;
    Vec2::new(col * ARENA_W, -row * ARENA_H)
}

/// Local centre of a board — the midpoint of the tile-centre extents, `(8.125,
/// 3.75)`. The camera frames here; content is dropped here.
pub fn board_center() -> Vec2 {
    Vec2::new(
        (GRID_W - 1) as f32 * TILE * 0.5,
        (GRID_H - 1) as f32 * TILE * 0.5,
    )
}

/// Local world XY of tile `(col, row)`'s centre — the same mapping the dot grid
/// uses (`col·TILE, row·TILE`).
pub fn tile_to_world(col: i32, row: i32) -> Vec2 {
    Vec2::new(col as f32 * TILE, row as f32 * TILE)
}

/// A board character that steps **one tile per arrow-key press** ([`step_movers`];
/// holding a key does NOT repeat). `col`/`row` are its tile within its arena, so
/// its local `Transform` stays grid-snapped.
#[derive(Component)]
pub struct TileMover {
    pub col: i32,
    pub row: i32,
}

impl TileMover {
    pub fn new(col: i32, row: i32) -> Self {
        Self { col, row }
    }

    /// Steps this mover by `delta` tiles, clamped to the arena grid. `strict_add`
    /// so a sim overflow fails loud instead of silently wrapping (movement feeds
    /// record/replay).
    pub fn step(&mut self, transform: &mut Transform, delta: IVec2) {
        self.snap_to(
            transform,
            self.col.strict_add(delta.x),
            self.row.strict_add(delta.y),
        );
    }

    /// Parks this mover on tile `(col, row)` (clamped to the arena) and snaps the
    /// transform's local X/Y to its centre (local Z is preserved). The ONE place
    /// tile-snapping math lives — stepping, ghost snapping, and edge-walking all
    /// land here so the policy can never drift.
    pub fn snap_to(&mut self, transform: &mut Transform, col: i32, row: i32) {
        self.col = col.clamp(0, MAX_COL);
        self.row = row.clamp(0, MAX_ROW);
        let p = tile_to_world(self.col, self.row);
        transform.translation.x = p.x;
        transform.translation.y = p.y;
    }
}

/// The arrow-key step for this frame: ±1 tile per axis per **press** (`just_pressed`
/// fires once per press, so a held key doesn't repeat).
pub fn arrow_delta(keys: &ButtonInput<KeyCode>) -> IVec2 {
    IVec2::new(
        (keys.just_pressed(KeyCode::ArrowRight) as i32)
            .strict_sub(keys.just_pressed(KeyCode::ArrowLeft) as i32),
        (keys.just_pressed(KeyCode::ArrowUp) as i32)
            .strict_sub(keys.just_pressed(KeyCode::ArrowDown) as i32),
    )
}

/// `true` when any arrow key was just pressed this frame (gates [`step_movers`]).
pub fn arrow_pressed(keys: Res<ButtonInput<KeyCode>>) -> bool {
    arrow_delta(&keys) != IVec2::ZERO
}

/// Steps every [`TileMover`] one tile per arrow-key press, clamped to its arena's
/// grid. Gate registrations on [`arrow_pressed`] (as [`GridPlugin`] does) — the
/// body writes every mover's `Transform` unconditionally, even for a zero delta.
pub fn step_movers(
    keys: Res<ButtonInput<KeyCode>>,
    mut movers: Query<(&mut TileMover, &mut Transform)>,
) {
    let delta = arrow_delta(&keys);
    for (mut mover, mut transform) in &mut movers {
        mover.step(&mut transform, delta);
    }
}

/// Adds tile-step movement for every [`TileMover`].
pub struct GridPlugin;

impl Plugin for GridPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, step_movers.run_if(arrow_pressed));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arena_grid_is_3x3_descending_in_minus_y() {
        assert_eq!(arena_offset(0), Vec2::new(0.0, 0.0));
        // Index 1 = top row, middle column (the Guild House) — one arena right.
        assert_eq!(arena_offset(1), Vec2::new(ARENA_W, 0.0));
        // Index 4 = centre arena — middle column, second row (descends in −Y).
        assert_eq!(arena_offset(4), Vec2::new(ARENA_W, -ARENA_H));
        assert_eq!(arena_offset(8), Vec2::new(2.0 * ARENA_W, -2.0 * ARENA_H));
    }

    #[test]
    fn board_centre_is_the_grid_midpoint() {
        assert_eq!(board_center(), Vec2::new(8.125, 3.75));
    }
}
