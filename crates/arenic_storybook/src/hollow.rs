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

use std::f32::consts::FRAC_PI_2;

use arenic_game::boss::{LightBehavior, light_offset};
use arenic_game::theme::{ActiveTheme, Tint};
use bevy::input::common_conditions::input_just_pressed;
use bevy::prelude::*;

use crate::layers::{Layer, OnLayer};
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

/// A glowing inner core: its rest transform (anchor), behavior, and the theme
/// token its colour follows. Set once at spawn (immutable); driven by
/// [`animate_hollow_lights`].
#[derive(Component)]
#[component(immutable)]
pub struct HollowLight {
    pub behavior: LightBehavior,
    pub color: Tint,
    pub rest: Transform,
}

/// Adds the tier control + core animation. Registered alongside the stage.
pub struct HollowLightPlugin;

impl Plugin for HollowLightPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Tier>().add_systems(
            Update,
            (
                cycle_tier.run_if(input_just_pressed(KeyCode::KeyT)),
                animate_hollow_lights,
            ),
        );
    }
}

/// Drops a hollow boss onto the board: the dark shell (`arenic_game` glTF,
/// oriented per §5) plus a separate emissive inner `core_mesh` that
/// [`animate_hollow_lights`] drives. `core_rest` is relative to the board centre.
pub fn spawn_hollow_boss(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    center: Vec2,
    shell: impl Bundle,
    shell_tint: Tint,
    core_mesh: Mesh,
    core_rest: Transform,
    behavior: LightBehavior,
    color: Tint,
) -> Entity {
    // Dark shell — themed via `StageContent` like any other content piece.
    // §5: rotate +90° about X so the authored (Y-up) shell faces the camera.
    commands.spawn((
        shell,
        StageContent { tint: shell_tint },
        OnLayer(Layer::Boss),
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
            OnLayer(Layer::Boss),
        ))
        .id()
}

/// `T` cycles the difficulty tier (Normal → Heroic → Mythic → …).
fn cycle_tier(mut tier: ResMut<Tier>) {
    *tier = tier.next();
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
        let (intensity, offset, rot) = light_offset(core.behavior, p);
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

// The per-behavior motion (`light_offset`) + the difficulty-cycle test live in
// `arenic_game::boss` now; this module keeps only the storybook's animation system.
