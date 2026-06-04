//! Hollow-light primitives — the shared substrate for the arena boss props
//! (ARE-8…ARE-16).
//!
//! Each boss is a **dark hollow shell** (a Blender glTF, loaded from
//! `arenic_game::boss`) plus a separate **emissive inner core** (a Bevy
//! primitive) that glows out through the shell's opening. With **HDR + bloom** on
//! the stage camera (see [`crate::stage`]) the core blooms while the shell stays
//! dark — so the *void* glows, not the surface (the core visual rule).
//!
//! [`animate_hollow_lights`] drives each core with its boss's signature
//! [`LightBehavior`] telegraph, scaled by the [`Tier`] (Normal / Heroic / Mythic,
//! cycled with the `T` key). [`spawn_hollow_boss`] is the one helper a story arm
//! calls to drop a shell + animated core onto the board.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_3};

use arenic_game::theme::{ActiveTheme, Theme};
use bevy::prelude::*;

use crate::layers::{Layer, LayerTag};
use crate::stage::StageContent;

/// HDR emissive scale for a core at full intensity — high enough to bloom.
const EMISSIVE: f32 = 4.0;

/// Difficulty tier — the same shape, faster/brighter light at each step (the
/// doc's Normal / Heroic / Mythic). Cycled in the storybook with the `T` key.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tier {
    #[default]
    Normal,
    Heroic,
    Mythic,
}

impl Tier {
    pub fn label(self) -> &'static str {
        match self {
            Tier::Normal => "Normal",
            Tier::Heroic => "Heroic",
            Tier::Mythic => "Mythic",
        }
    }
    fn speed(self) -> f32 {
        match self {
            Tier::Normal => 1.0,
            Tier::Heroic => 1.7,
            Tier::Mythic => 2.6,
        }
    }
    fn intensity(self) -> f32 {
        match self {
            Tier::Normal => 1.0,
            Tier::Heroic => 1.4,
            Tier::Mythic => 1.9,
        }
    }
    fn next(self) -> Tier {
        match self {
            Tier::Normal => Tier::Heroic,
            Tier::Heroic => Tier::Mythic,
            Tier::Mythic => Tier::Normal,
        }
    }
}

/// The signature light-telegraph motion of each boss primitive.
#[derive(Clone, Copy)]
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

/// A glowing inner core: its rest transform (anchor), behavior, and the theme
/// token its colour follows. Driven by [`animate_hollow_lights`].
#[derive(Component)]
pub struct HollowLight {
    pub behavior: LightBehavior,
    pub color: fn(&Theme) -> Color,
    pub rest: Transform,
}

/// Adds the tier control + core animation. Registered alongside the stage.
pub struct HollowLightPlugin;

impl Plugin for HollowLightPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Tier>()
            .add_systems(Update, (cycle_tier, animate_hollow_lights));
    }
}

/// Drops a hollow boss onto the board: the dark shell (`arenic_game` glTF,
/// oriented per §5) plus a separate emissive inner `core_mesh` that
/// [`animate_hollow_lights`] drives. `core_rest` is relative to the board centre.
#[allow(clippy::too_many_arguments)]
pub fn spawn_hollow_boss(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    center: Vec2,
    shell: impl Bundle,
    shell_tint: fn(&Theme) -> Color,
    core_mesh: Mesh,
    core_rest: Transform,
    behavior: LightBehavior,
    color: fn(&Theme) -> Color,
) -> Entity {
    // Dark shell — themed via `StageContent` like any other content piece.
    // §5: rotate +90° about X so the authored (Y-up) shell faces the camera.
    commands.spawn((
        shell,
        StageContent { tint: shell_tint },
        LayerTag(Layer::Boss),
        Transform::from_xyz(center.x, center.y, 0.02)
            .with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
    ));

    // Emissive core — a world-space primitive, animated. Tagged `HollowLight`
    // (not `StageContent`) so it's owned entirely by the animation system.
    let mut rest = core_rest;
    rest.translation += Vec3::new(center.x, center.y, 0.0);
    let mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.02, 0.02, 0.03),
        emissive: LinearRgba::BLACK,
        perceptual_roughness: 1.0,
        ..default()
    });
    commands
        .spawn((
            Mesh3d(meshes.add(core_mesh)),
            MeshMaterial3d(mat),
            rest,
            HollowLight {
                behavior,
                color,
                rest,
            },
            LayerTag(Layer::Boss),
        ))
        .id()
}

/// `T` cycles the difficulty tier (Normal → Heroic → Mythic → …).
fn cycle_tier(keys: Res<ButtonInput<KeyCode>>, mut tier: ResMut<Tier>) {
    if keys.just_pressed(KeyCode::KeyT) {
        *tier = tier.next();
    }
}

/// Animates every [`HollowLight`] core: a per-behavior motion + emissive pulse,
/// scaled by the active [`Tier`] and tinted by the active theme.
fn animate_hollow_lights(
    time: Res<Time>,
    tier: Res<Tier>,
    active: Res<ActiveTheme>,
    mut cores: Query<(
        &HollowLight,
        &mut Transform,
        &MeshMaterial3d<StandardMaterial>,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let theme = active.palette();
    let p = time.elapsed_secs() * tier.speed();
    let gain = tier.intensity();
    for (core, mut tf, mat) in &mut cores {
        let (intensity, offset, rot) = animate(core.behavior, p);
        tf.translation = core.rest.translation + offset;
        tf.rotation = core.rest.rotation * rot;
        tf.scale = core.rest.scale;
        if let Some(m) = materials.get_mut(&mat.0) {
            let lin = (core.color)(&theme).to_linear();
            m.emissive =
                LinearRgba::rgb(lin.red, lin.green, lin.blue) * (intensity * gain * EMISSIVE);
        }
    }
}

/// Per-behavior `(emissive intensity, position offset, extra rotation)` at phase
/// `p` (seconds × tier speed). Pure function of time — deterministic, no RNG.
fn animate(behavior: LightBehavior, p: f32) -> (f32, Vec3, Quat) {
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
        // The emissive multiplier feeds bloom; a negative value would invert the
        // glow. Every telegraph must stay >= 0 across its whole cycle.
        for (b, behavior) in EVERY_BEHAVIOR.into_iter().enumerate() {
            for i in 0..400 {
                let p = i as f32 * 0.25;
                let (intensity, _, _) = animate(behavior, p);
                assert!(
                    intensity >= 0.0,
                    "behavior #{b} gave negative intensity {intensity}"
                );
            }
        }
    }
}
