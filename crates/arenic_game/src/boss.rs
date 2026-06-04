//! **Boss A** — a geometric, square-based pyramid token.
//!
//! Authored in Blender at true game scale (a `1.0`-unit base = **4 tiles across**,
//! the *boss* size from `_docs/UNITS_AND_SCALE.md` §1 — radius `0.5`, ≈ 75 px at
//! the default zoom) and exported to `assets/models/boss_a.glb`. This crate owns
//! only the *piece* — the marker component and how to spawn the model. Scene
//! dressing (camera, lights, ground) belongs to whoever stages it (the storybook
//! story today, the real arena later), so this stays free of any scene deps.

use bevy::prelude::*;

/// Marks a spawned Boss A.
#[derive(Component, Debug, Clone, Copy)]
pub struct BossA;

/// glTF scene path for the pyramid (Bevy labels the first scene `#Scene0`).
pub const MODEL: &str = "models/boss_a.glb#Scene0";

/// A bundle that spawns the Boss A pyramid **as authored**: already at true scale
/// (1 unit = 1 m), its base lying flat in the XZ plane with the apex pointing
/// `+Y` after glTF's Z-up → Y-up conversion — i.e. an upright pyramid in a Y-up
/// world. Add a [`Transform`] to position it.
///
/// For the top-down game board (board on XY, camera on `+Z`) rotate `+90°` about
/// X at the call site so the base lies on the board and the apex points at the
/// camera (UNITS_AND_SCALE §5) — the same orientation fix the flat tokens use.
pub fn boss_a(assets: &AssetServer) -> impl Bundle {
    (BossA, SceneRoot(assets.load(MODEL)))
}

/// Defines a **hollow-light shell** boss piece: a marker component + a spawn fn
/// loading its dark glTF shell. The shell is *only* the dark silhouette (modeled
/// in Blender per ARE-8…ARE-15); whoever stages it adds the glowing inner core
/// (e.g. the storybook's `hollow` module). Zero scene/camera deps, so the arena
/// binary can spawn the same pieces.
macro_rules! hollow_shell {
    ($marker:ident, $func:ident, $path:literal) => {
        #[derive(Component, Debug, Clone, Copy)]
        pub struct $marker;

        #[doc = concat!("Spawns the ", stringify!($marker), " hollow shell (dark glTF).")]
        pub fn $func(assets: &AssetServer) -> impl Bundle {
            ($marker, SceneRoot(assets.load(concat!($path, "#Scene0"))))
        }
    };
}

hollow_shell!(HollowObelisk, hollow_obelisk, "models/hollow_obelisk.glb"); // Hunter / Labyrinth
hollow_shell!(TruncatedCone, truncated_cone, "models/truncated_cone.glb"); // Alchemist / Crucible
hollow_shell!(TorusHalo, torus_halo, "models/torus_halo.glb"); // Cardinal / Sanctum
hollow_shell!(HexPrism, hex_prism, "models/hex_prism.glb"); // Warrior / Bastion
hollow_shell!(TriangularPrism, triangular_prism, "models/triangular_prism.glb"); // Thief / Pawnshop
hollow_shell!(CapsuleResonator, capsule_resonator, "models/capsule_resonator.glb"); // Bard / Gala
hollow_shell!(SteppedPyramid, stepped_pyramid, "models/stepped_pyramid.glb"); // Forager / Mountain
hollow_shell!(HollowIcosphere, hollow_icosphere, "models/hollow_icosphere.glb"); // Merchant / Casino
