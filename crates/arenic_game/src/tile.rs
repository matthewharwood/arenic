//! Board **tiles + their state**. Every cell of every arena is an entity carrying a
//! [`Tile`] (its `arena/col/row` + a [`TileKind`] state). [`ArenaTiles`] is an O(1)
//! `(arena, col, row) → Entity` lookup, and the [`TileBoard`] system param flips any
//! tile's state — which just swaps its **shared, per-state** material handle, so the
//! 18k-tile grid stays cheap (no per-tile materials, no respawns):
//!
//! ```
//! # use arenic_game::tile::{TileBoard, TileKind};
//! fn erupt(mut board: TileBoard) {
//!     board.set(0, 1, 1, TileKind::Lava); // Hunter's arena, tile (col 1, row 1) → lava
//! }
//! ```
//!
//! The game builds the materials with [`build_tile_materials`], spawns a tile per
//! cell (each with a [`Tile`] + the shared `Normal` material), and records every
//! entity into [`ArenaTiles`].

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::grid::{ARENAS, GRID_H, GRID_W};

/// The state of one board tile. Add variants here + a material in [`TileMaterials`].
/// Serde because tile-choreography keyframes ([`crate::tile_script`]) name kinds
/// in their RON score files.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum TileKind {
    /// A plain illuminated dot.
    #[default]
    Normal,
    /// A glowing lava tile.
    Lava,
}

/// A board tile: which arena/cell it is, and its current [`TileKind`].
#[derive(Component, Clone, Copy)]
pub struct Tile {
    pub arena: u8,
    pub col: u8,
    pub row: u8,
    pub kind: TileKind,
}

const W: usize = GRID_W as usize;
const H: usize = GRID_H as usize;
// const-eval: a bad grid constant fails the build instead of wrapping.
const TILE_COUNT: usize = ARENAS * W * H;

/// O(1) lookup of a tile entity by `(arena, col, row)`, built as the grid spawns.
#[derive(Resource)]
pub struct ArenaTiles {
    entities: Vec<Entity>, // len = TILE_COUNT, row-minor within an arena
}

impl Default for ArenaTiles {
    fn default() -> Self {
        Self {
            entities: vec![Entity::PLACEHOLDER; TILE_COUNT],
        }
    }
}

impl ArenaTiles {
    /// `strict_*` so a bad grid constant fails loud rather than wrapping (grid
    /// indices feed the future record/replay).
    fn index(arena: usize, col: usize, row: usize) -> Option<usize> {
        (arena < ARENAS && col < W && row < H).then_some(
            arena
                .strict_mul(W)
                .strict_add(col)
                .strict_mul(H)
                .strict_add(row),
        )
    }

    /// Record the entity for tile `(arena, col, row)` (called once per tile at spawn).
    pub fn insert(&mut self, arena: usize, col: usize, row: usize, entity: Entity) {
        if let Some(i) = Self::index(arena, col, row) {
            self.entities[i] = entity;
        }
    }

    /// The tile entity at `(arena, col, row)`, or `None` if out of range / unset.
    pub fn get(&self, arena: usize, col: usize, row: usize) -> Option<Entity> {
        let e = *self.entities.get(Self::index(arena, col, row)?)?;
        (e != Entity::PLACEHOLDER).then_some(e)
    }
}

/// The **shared** material for each [`TileKind`]: one themed glow material per arena
/// for `Normal`, one universal material for every other state. A tile changing state
/// just re-points at one of these — never a new material.
#[derive(Resource)]
pub struct TileMaterials {
    normal: [Handle<StandardMaterial>; ARENAS],
    lava: Handle<StandardMaterial>,
}

impl TileMaterials {
    /// The material handle for `(arena, kind)`.
    pub fn of(&self, arena: usize, kind: TileKind) -> Handle<StandardMaterial> {
        match kind {
            TileKind::Normal => self.normal[arena.min(ARENAS - 1)].clone(),
            TileKind::Lava => self.lava.clone(),
        }
    }
}

/// Builds the shared tile materials: a softly **illuminated** `Normal` dot per arena
/// (glowing in that arena's `tints[i]`), plus the universal lava one.
pub fn build_tile_materials(
    materials: &mut Assets<StandardMaterial>,
    tints: [Color; ARENAS],
) -> TileMaterials {
    TileMaterials {
        normal: tints.map(|c| materials.add(glow(c))),
        lava: materials.add(lava()),
    }
}

/// A `Normal` tile material — a small **unlit** dot (flat, no lighting, no emissive),
/// so the grid reads as subtle texture instead of blooming hotspots under HDR+bloom
/// (the storybook's dots are unlit + dim; emissive dots blow out the whole board).
fn glow(color: Color) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        unlit: true,
        ..default()
    }
}

/// A glowing lava tile — emissive so it blooms under the stage's HDR + bloom.
fn lava() -> StandardMaterial {
    let c = Color::srgb(1.0, 0.32, 0.05);
    let l = c.to_linear();
    StandardMaterial {
        base_color: c,
        emissive: LinearRgba::rgb(l.red, l.green, l.blue) * 4.0,
        perceptual_roughness: 0.6,
        ..default()
    }
}

/// Read/flip board tiles by `(arena, col, row)`. Setting a tile's state swaps its
/// shared material; a no-op if the tile doesn't exist or is already that state.
#[derive(SystemParam)]
pub struct TileBoard<'w, 's> {
    lookup: Res<'w, ArenaTiles>,
    materials: Res<'w, TileMaterials>,
    tiles: Query<
        'w,
        's,
        (
            &'static mut Tile,
            &'static mut MeshMaterial3d<StandardMaterial>,
        ),
    >,
}

impl TileBoard<'_, '_> {
    /// Set tile `(arena, col, row)` to `kind`.
    pub fn set(&mut self, arena: usize, col: usize, row: usize, kind: TileKind) {
        let Some(entity) = self.lookup.get(arena, col, row) else {
            return;
        };
        let Ok((mut tile, mut material)) = self.tiles.get_mut(entity) else {
            return;
        };
        if tile.kind != kind {
            tile.kind = kind;
            material.0 = self.materials.of(arena, kind);
        }
    }

    /// The current state of tile `(arena, col, row)`, if it exists.
    pub fn kind(&self, arena: usize, col: usize, row: usize) -> Option<TileKind> {
        let entity = self.lookup.get(arena, col, row)?;
        self.tiles.get(entity).ok().map(|(tile, _)| tile.kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_index_is_unique_and_bounded() {
        // Distinct cells map to distinct slots; out-of-range maps to None.
        assert_ne!(
            ArenaTiles::index(0, 1, 1),
            ArenaTiles::index(1, 1, 1),
            "different arenas must not collide"
        );
        assert_eq!(ArenaTiles::index(0, 0, 0), Some(0));
        assert_eq!(ArenaTiles::index(ARENAS, 0, 0), None);
        assert_eq!(ArenaTiles::index(0, W, 0), None);
        assert_eq!(ArenaTiles::index(0, 0, H), None);
        assert_eq!(
            ArenaTiles::index(ARENAS - 1, W - 1, H - 1),
            Some(ARENAS * W * H - 1)
        );
    }
}
