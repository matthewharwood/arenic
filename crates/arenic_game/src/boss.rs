//! **Boss A** — a geometric, square-based pyramid token.
//!
//! Authored in Blender at true game scale (a `1.0`-unit base = **4 tiles across**,
//! the *boss* size from `_docs/UNITS_AND_SCALE.md` §1 — radius `0.5`, ≈ 75 px at
//! the default zoom) and exported to `assets/models/boss_a.glb`. This crate owns
//! only the *piece* — the marker component and how to spawn the model. Scene
//! dressing (camera, lights, ground) belongs to whoever stages it (the storybook
//! story today, the real arena later), so this stays free of any scene deps.

use std::f32::consts::FRAC_PI_3;

use bevy::prelude::*;

use crate::theme::Tint;

/// Marks a spawned Boss A.
#[derive(Component, Debug, Clone, Copy)]
#[component(immutable)]
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
pub fn boss_a(assets: &AssetServer) -> impl Bundle + use<> {
    (BossA, SceneRoot(assets.load(MODEL)))
}

/// One of the eight **hollow-light shells** — a dark glTF silhouette (`assets/models/
/// *.glb`, modeled per ARE-8…ARE-15). This crate owns only the dark shell + which
/// `.glb` it is; whoever stages it adds the glowing inner [`CoreMesh`]. The arena
/// each shell belongs to lives in [`crate::arena`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BossShell {
    Obelisk,          // Hunter / Labyrinth
    TruncatedCone,    // Alchemist / Crucible
    TorusHalo,        // Cardinal / Sanctum
    HexPrism,         // Warrior / Bastion
    TriangularPrism,  // Thief / Pawnshop
    CapsuleResonator, // Bard / Gala
    SteppedPyramid,   // Forager / Mountain
    HollowIcosphere,  // Merchant / Casino
}

impl BossShell {
    /// The glTF scene path for this shell (Bevy labels the first scene `#Scene0`).
    /// The ONE place a shell maps to its `.glb` — both binaries spawn from here.
    pub fn model(self) -> &'static str {
        match self {
            BossShell::Obelisk => "models/hollow_obelisk.glb#Scene0",
            BossShell::TruncatedCone => "models/truncated_cone.glb#Scene0",
            BossShell::TorusHalo => "models/torus_halo.glb#Scene0",
            BossShell::HexPrism => "models/hex_prism.glb#Scene0",
            BossShell::TriangularPrism => "models/triangular_prism.glb#Scene0",
            BossShell::CapsuleResonator => "models/capsule_resonator.glb#Scene0",
            BossShell::SteppedPyramid => "models/stepped_pyramid.glb#Scene0",
            BossShell::HollowIcosphere => "models/hollow_icosphere.glb#Scene0",
        }
    }

    /// A bundle that spawns this shell's dark glTF. Add a [`Transform`] at the call
    /// site (rotate `+90°` about X for the top-down board — UNITS_AND_SCALE §5).
    pub fn scene(self, assets: &AssetServer) -> impl Bundle + use<> {
        SceneRoot(assets.load(self.model()))
    }
}

/// The emissive inner-core primitive of a hollow boss (a Bevy primitive that glows
/// out through the shell's opening).
#[derive(Clone, Copy)]
pub enum CoreMesh {
    Cuboid(f32, f32, f32),
    Sphere(f32),
}

impl CoreMesh {
    pub fn to_mesh(self) -> Mesh {
        match self {
            CoreMesh::Cuboid(x, y, z) => Cuboid::new(x, y, z).into(),
            CoreMesh::Sphere(r) => Sphere::new(r).into(),
        }
    }
}

/// The signature light-telegraph motion of each boss core.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LightBehavior {
    /// Hunter — a blade of light sweeps around the four inner walls.
    ObeliskBlade,
    /// Alchemist — liquid light rises and falls inside the vessel.
    CauldronRise,
    /// Cardinal — the ring fills, resolves, then resets.
    HaloFill,
    /// Warrior — a face lights to show the blocked direction.
    PrismBlock,
    /// Thief — a narrow beam projects forward and back.
    WedgeBeam,
    /// Bard — the filament pulses on the beat.
    CapsulePulse,
    /// Forager — light grows upward from the central shaft.
    ZigguratGrow,
    /// Merchant — facets flicker semi-randomly (luck / volatility).
    GeodeShimmer,
}

/// Per-behaviour `(emissive intensity, position offset, extra rotation)` at phase
/// `p` (seconds × tier speed). Pure function of time — deterministic, no RNG. The
/// shared motion both binaries' core-animation systems read.
pub fn light_offset(behavior: LightBehavior, p: f32) -> (f32, Vec3, Quat) {
    use LightBehavior::*;
    // Unit "rise" helper: a 0..1 sine.
    let s = |x: f32| 0.5 + 0.5 * x.sin();
    match behavior {
        ObeliskBlade => (
            0.6 + 0.5 * s(p * 3.0),
            Vec3::new(0.16 * p.cos(), 0.16 * p.sin(), 0.0),
            Quat::IDENTITY,
        ),
        CauldronRise => {
            let rise = 0.3 * (1.0 - (p * 0.8).cos());
            (0.45 + 0.9 * rise, Vec3::new(0.0, 0.0, rise), Quat::IDENTITY)
        }
        HaloFill => (
            0.2 + 0.9 * (p * 0.3).fract(),
            Vec3::ZERO,
            Quat::from_rotation_z(p),
        ),
        PrismBlock => {
            let a = (p * 0.6).floor() * FRAC_PI_3;
            (
                0.45 + 0.5 * s(p * 3.0),
                Vec3::new(0.12 * a.cos(), 0.12 * a.sin(), 0.0),
                Quat::IDENTITY,
            )
        }
        WedgeBeam => (
            0.5 + 0.6 * s(p * 3.0),
            Vec3::new(0.12 * s(p * 1.5), 0.0, 0.0),
            Quat::IDENTITY,
        ),
        CapsulePulse => {
            let beat = (p * 4.0).sin().max(0.0);
            (0.25 + 0.85 * beat * beat, Vec3::ZERO, Quat::IDENTITY)
        }
        ZigguratGrow => (
            0.4 + 0.6 * s(p * 0.8),
            Vec3::new(0.0, 0.0, 0.08 * p.sin()),
            Quat::IDENTITY,
        ),
        GeodeShimmer => (
            0.3 + 0.7 * s(p * 7.0) * s(p * 3.3 + 1.0),
            Vec3::new(0.02 * (p * 11.0).sin(), 0.02 * (p * 9.0).cos(), 0.0),
            Quat::IDENTITY,
        ),
    }
}

/// What sits at the centre of an arena.
#[derive(Clone, Copy)]
pub enum BossSpec {
    /// A hollow-light boss: a dark [`BossShell`] + an animated emissive [`CoreMesh`].
    Hollow {
        shell: BossShell,
        core: CoreMesh,
        core_z: f32,
        behavior: LightBehavior,
        color: Tint,
    },
    /// The Guild House home: a calm pyramid (reusing [`boss_a`]), no core.
    Home { color: Tint },
}

#[cfg(test)]
mod tests {
    use super::*;

    const EVERY_BEHAVIOR: [LightBehavior; 8] = [
        LightBehavior::ObeliskBlade,
        LightBehavior::CauldronRise,
        LightBehavior::HaloFill,
        LightBehavior::PrismBlock,
        LightBehavior::WedgeBeam,
        LightBehavior::CapsulePulse,
        LightBehavior::ZigguratGrow,
        LightBehavior::GeodeShimmer,
    ];

    #[test]
    fn core_intensity_is_never_negative() {
        // The emissive multiplier feeds bloom; a negative value would invert the glow.
        for (b, behavior) in EVERY_BEHAVIOR.into_iter().enumerate() {
            for i in 0..400 {
                let (intensity, _, _) = light_offset(behavior, i as f32 * 0.25);
                assert!(intensity >= 0.0, "behavior #{b} gave negative {intensity}");
            }
        }
    }
}
