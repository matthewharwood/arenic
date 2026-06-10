//! The per-arena **sky-swarm** — a low ring of drifting motes that rides the board
//! rim *under* the translucent floor (seen dimly through the liquid glass), giving
//! each arena its signature fauna motion.
//!
//! This module owns the shared **types + math**: the [`Mote`] silhouettes, the
//! per-arena [`SwarmMotion`] archetypes, the [`SwarmMember`] component, and the pure
//! placement/motion functions ([`swarm_home`], [`swarm_amp`], [`swarm_offset`],
//! [`mote_mesh`], [`swarm_material`]). Each binary keeps its own spawn + animate
//! systems (the storybook respawns per story + re-tones on theme change; the game
//! spawns all nine arenas once with baked colours) — both built on these.

use std::f32::consts::TAU;

use bevy::color::Alpha;
use bevy::prelude::*;

/// A small mote silhouette family, shared across arenas to keep one grammar.
#[derive(Clone, Copy)]
pub enum Mote {
    /// Round glow-mote (ember, spore).
    Spark,
    /// Flat quad (gilt-leaf, ash flake, coin, confetti).
    Flake,
    /// Elongated sliver (scout dart, bat).
    Dart,
    /// Soft round bubble (alchemical vapor).
    Bubble,
}

/// The per-arena drift archetype — the distinct motion that separates arenas.
#[derive(Clone, Copy)]
pub enum SwarmMotion {
    /// Hunter — straight glides along the corridors with sharp turns.
    Patrol,
    /// Guildmaster — gentle warm hearth-updraft (calmest of all).
    HearthRise,
    /// Cardinal — gilt-leaf sifting down in a near-vertical pendulum sway.
    PendulumFall,
    /// Forager — wind-borne spores wandering with gusts.
    GustDrift,
    /// Warrior — forge cinders stalling, flipping and rising on the heat.
    UpdraftChurn,
    /// Thief — furtive dart-pause-dart, never lingering.
    FurtiveDart,
    /// Alchemist — bubbles crawl up while droplets sag down (opposing).
    OpposingDrift,
    /// Merchant — coins tumbling end-over-end in tight mechanical arcs.
    TumbleArc,
    /// Bard — confetti drifting, with a synchronized on-beat upward pluck.
    BeatDrift,
}

/// The sky-swarm for an arena: its drift archetype, mote silhouette, count, size.
#[derive(Clone, Copy)]
pub struct SwarmSpec {
    pub motion: SwarmMotion,
    pub mote: Mote,
    pub count: usize,
    pub scale: f32,
}

/// One drifting swarm instance. `home` is its sky anchor; `amp` is its drift
/// amplitude (the `z` sign also picks rise-vs-fall for [`SwarmMotion::OpposingDrift`]).
/// Set once at spawn (immutable) — the animate systems read it and write `Transform`.
#[derive(Component, Clone, Copy)]
#[component(immutable)]
pub struct SwarmMember {
    pub motion: SwarmMotion,
    pub phase: f32,
    pub home: Vec3,
    pub amp: Vec3,
}

/// Swarm geometry — the "under the floor, never through it" grammar. The ring
/// half-axes sit a little inside the board half-extents (~8.1 × 3.75) so motes hug
/// the rim. `SWARM_Z` seats the swarm well below any floor plane; since no motion
/// exceeds `±SWARM_AMP.z`, the swarm rides `z ∈ [-2.3, -0.5]`.
const SWARM_RING_X: f32 = 7.5;
const SWARM_RING_Y: f32 = 3.4;
const SWARM_Z: f32 = -1.4;
const SWARM_AMP: Vec3 = Vec3::new(1.4, 1.0, 0.9);
/// Global slow factor on swarm time — keeps the drift calm and non-distracting.
pub const SWARM_TIME_SCALE: f32 = 0.5;
/// Compile-time invariant: the swarm's highest reach (`SWARM_Z + SWARM_AMP.z = -0.5`)
/// stays well below every floor plane (game `z = 0`, storybook `z = -0.02`), so the
/// swarm goes *around and under* the ground, never *through* it.
const _: () = assert!(SWARM_Z + SWARM_AMP.z < -0.05);

/// The sky-anchor of mote `i` of `count`, in a golden-angle ring around `center`.
pub fn swarm_home(i: usize, count: usize, center: Vec2) -> Vec3 {
    let a = i as f32 * 2.399_963_2; // golden angle — even spread
    let r = 0.65 + 0.4 * ((i as f32 + 0.5) / count as f32).sqrt();
    Vec3::new(
        center.x + a.cos() * r * SWARM_RING_X,
        center.y + a.sin() * r * SWARM_RING_Y,
        SWARM_Z,
    )
}

/// The drift amplitude of mote `i` (the `OpposingDrift` z-sign alternates per index).
pub fn swarm_amp(motion: SwarmMotion, i: usize) -> Vec3 {
    let z_sign = if matches!(motion, SwarmMotion::OpposingDrift) && i % 2 == 1 {
        -1.0
    } else {
        1.0
    };
    Vec3::new(SWARM_AMP.x, SWARM_AMP.y, SWARM_AMP.z * z_sign)
}

/// A small mote mesh for the given silhouette family at `s` units.
pub fn mote_mesh(mote: Mote, s: f32) -> Mesh {
    match mote {
        Mote::Spark => Sphere::new(s * 0.5).into(),
        Mote::Bubble => Sphere::new(s * 0.6).into(),
        Mote::Flake => Cuboid::new(s, s, s * 0.12).into(),
        Mote::Dart => Cuboid::new(s * 0.3, s * 1.6, s * 0.1).into(),
    }
}

/// The shared, translucent, lightly-emissive material for a swarm tinted `c`.
pub fn swarm_material(c: Color) -> StandardMaterial {
    let l = c.to_linear();
    StandardMaterial {
        base_color: c.with_alpha(0.55),
        emissive: LinearRgba::rgb(l.red, l.green, l.blue) * 1.6,
        alpha_mode: AlphaMode::Blend,
        ..default()
    }
}

/// Per-motion `(drift offset, rotation)` at phase `p` and time `t`. Pure function of
/// time — deterministic, no RNG; the per-index `phase` staggers the swarm.
pub fn swarm_offset(motion: SwarmMotion, phase: f32, t: f32, amp: Vec3) -> (Vec3, Quat) {
    use SwarmMotion::*;
    let s = |k: f32| (t * k + phase).sin();
    // 0..1 sawtooth (rise-and-reset), for looping vertical drifts.
    let saw = |k: f32| (t * k + phase).rem_euclid(TAU) / TAU;
    match motion {
        Patrol => (
            Vec3::new(
                amp.x * (2.0 * s(0.35)).tanh(),
                amp.y * s(0.16),
                amp.z * 0.2 * s(0.5),
            ),
            Quat::from_rotation_z(0.3 * s(0.4)),
        ),
        HearthRise => (
            Vec3::new(
                amp.x * 0.4 * s(0.3),
                amp.y * 0.3 * s(0.25),
                amp.z * saw(0.10),
            ),
            Quat::IDENTITY,
        ),
        PendulumFall => (
            Vec3::new(
                amp.x * s(0.6),
                amp.y * 0.3 * s(0.4),
                amp.z * (1.0 - saw(0.08)),
            ),
            Quat::from_rotation_y(0.6 * s(0.6)),
        ),
        GustDrift => (
            Vec3::new(
                amp.x * (s(0.2) + 0.4 * (t * 0.07 + phase * 3.0).sin()),
                amp.y * s(0.17),
                amp.z * 0.3 * s(0.13),
            ),
            Quat::from_rotation_z(0.5 * s(0.3)),
        ),
        UpdraftChurn => (
            Vec3::new(
                amp.x * 0.4 * s(0.3),
                amp.y * 0.3 * s(0.27),
                amp.z * (0.5 + 0.5 * s(0.4)),
            ),
            Quat::from_rotation_x(0.8 * s(0.5)),
        ),
        FurtiveDart => (
            Vec3::new(
                amp.x * (3.0 * s(0.6)).tanh(),
                amp.y * (3.0 * (t * 0.5 + phase + 1.0).sin()).tanh(),
                amp.z * 0.2 * s(0.7),
            ),
            Quat::from_rotation_z(0.4 * s(0.8)),
        ),
        OpposingDrift => (
            Vec3::new(
                amp.x * 0.3 * s(0.2),
                amp.y * 0.3 * s(0.18),
                amp.z * saw(0.12),
            ),
            Quat::IDENTITY,
        ),
        TumbleArc => (
            Vec3::new(amp.x * s(0.3), amp.y * 0.4 * s(0.25), amp.z * s(0.4)),
            Quat::from_axis_angle(Vec3::new(0.4, 1.0, 0.2).normalize(), t * 2.0 + phase),
        ),
        BeatDrift => {
            let beat = (t * 2.0 + phase).sin().max(0.0).powi(4);
            (
                Vec3::new(
                    amp.x * s(0.2),
                    amp.y * s(0.16),
                    -amp.z * 0.3 * saw(0.1) + amp.z * 0.5 * beat,
                ),
                Quat::from_rotation_z(0.5 * s(0.35)),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EVERY_MOTION: [SwarmMotion; 9] = [
        SwarmMotion::Patrol,
        SwarmMotion::HearthRise,
        SwarmMotion::PendulumFall,
        SwarmMotion::GustDrift,
        SwarmMotion::UpdraftChurn,
        SwarmMotion::FurtiveDart,
        SwarmMotion::OpposingDrift,
        SwarmMotion::TumbleArc,
        SwarmMotion::BeatDrift,
    ];

    #[test]
    fn swarm_never_rises_above_minus_005() {
        // Beyond the compile-time resting-ceiling assert, sample every archetype over
        // time + phase so a future motion tweak that breaches the floor is caught.
        for (m, motion) in EVERY_MOTION.into_iter().enumerate() {
            for i in 0..200 {
                let (offset, _) = swarm_offset(motion, m as f32 * 0.7, i as f32 * 0.31, SWARM_AMP);
                let z = SWARM_Z + offset.z;
                assert!(z < -0.05, "motion #{m} rose to z = {z}");
            }
        }
    }
}
