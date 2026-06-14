//! Reusable ability pieces — exercised by the storybook's `abilities/*` stories.
//!
//! An ability is **VFX + SFX**: [`AbilityBurst`] is a self-contained expanding-
//! sphere effect (e.g. Holy Nova) — a unit sphere scaled from `start_radius` to
//! `end_radius` over `duration`, eased and fading out, then despawned — and
//! [`play_sfx`] fires its one-shot sound. Both are generic *pieces* (no class, no
//! timeline). Spawn [`holy_nova`] + [`play_sfx`] when an ability casts, and add
//! [`AbilityPlugin`] to drive the VFX and preload every ability's sound into
//! [`AbilitySfx`].

use bevy::audio::{AudioSource, DefaultSpatialScale};
use bevy::color::Alpha;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::audio::{AudioMix, SfxProfile, play_spatial_sfx};

/// The identity of one castable ability — THE shared data structure for hero
/// and boss abilities alike. A boss ability is built from the exact same pieces
/// as a hero ability (generally a longer / stronger / weaker variant of one),
/// so the two can never structurally drift: add a variant here and a
/// [`cast`] arm, and both heroes and the boss phase loadouts
/// ([`crate::encounter::loadout`]) can slot it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum AbilityId {
    HolyNova,
}

impl AbilityId {
    /// The short display name shown on the hotbar — one word so a slot's label
    /// never wraps. (The VFX is a "Holy Nova"; on the bar it reads just "Holy".)
    pub fn name(self) -> &'static str {
        match self {
            AbilityId::HolyNova => "Holy",
        }
    }

    /// How this ability SOUNDS across distance ([`SfxProfile`]): its base
    /// volume, and `reach` — how far it carries past the spatial falloff knee.
    /// This is where "different tones behave differently" lives; give a new
    /// ability its own row as it lands.
    pub fn sfx_profile(self) -> SfxProfile {
        match self {
            AbilityId::HolyNova => SfxProfile {
                volume: 1.0,
                reach: 1.0,
            },
        }
    }
}

/// Casts `ability` from `caster` — the ONE dispatch point shared by live input,
/// hero ghost playback, and boss phase loadouts, so a cast always means the
/// same thing no matter who triggered it. The sound emits FROM the caster
/// (spatial, against the camera-microphone) shaped by the ability's
/// [`AbilityId::sfx_profile`]. `aim` is the recorded tile-space direction of a
/// directed cast — radial abilities (Holy Nova) ignore it; beams consume it.
pub fn cast(
    commands: &mut Commands,
    ability: AbilityId,
    meshes: &AbilityMeshes,
    materials: &mut Assets<StandardMaterial>,
    sfx: &AbilitySfx,
    mix: &AudioMix,
    scale: &DefaultSpatialScale,
    aim: Option<IVec2>,
    caster: Entity,
) {
    let _ = aim; // no directed ability exists yet — the payload plumbs through
    match ability {
        AbilityId::HolyNova => cast_holy_nova(commands, meshes, materials, sfx, mix, scale, caster),
    }
}

/// An expanding, fading sphere burst. Driven to completion + despawn by
/// [`update_ability_bursts`]; create one with [`holy_nova`].
#[derive(Component)]
pub struct AbilityBurst {
    pub elapsed: f32,
    pub duration: f32,
    pub start_radius: f32,
    pub end_radius: f32,
    pub base_alpha: f32,
}

/// A **Holy Nova**: a translucent, emissive sphere of light that bursts outward
/// from a small to a large radius and dissipates. `color` tints the glow (it blooms
/// via the stage's HDR + bloom). Centre it on the caster by spawning it as their
/// child: `commands.spawn((holy_nova(..), ChildOf(caster)))`.
pub fn holy_nova(
    sphere: Handle<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    color: Color,
) -> impl Bundle + use<> {
    const START: f32 = 0.3;
    const BASE_ALPHA: f32 = 0.4;
    let l = color.to_linear();
    let material = materials.add(StandardMaterial {
        base_color: color.with_alpha(BASE_ALPHA),
        emissive: LinearRgba::rgb(l.red, l.green, l.blue) * 5.0,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    (
        AbilityBurst {
            elapsed: 0.0,
            duration: 0.6,
            start_radius: START,
            end_radius: 3.0,
            base_alpha: BASE_ALPHA,
        },
        Mesh3d(sphere),
        MeshMaterial3d(material),
        Transform::from_scale(Vec3::splat(START)),
        NotShadowCaster,
        NotShadowReceiver,
    )
}

/// Expands each [`AbilityBurst`] (exponential-out), fades it as it grows, and
/// despawns it once its `duration` elapses. A pure function of `time`.
pub fn update_ability_bursts(
    mut commands: Commands,
    time: Res<Time>,
    mut bursts: Query<(
        Entity,
        &mut Transform,
        &mut AbilityBurst,
        &MeshMaterial3d<StandardMaterial>,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, mut transform, mut burst, material) in &mut bursts {
        burst.elapsed += time.delta_secs();
        let t = (burst.elapsed / burst.duration).clamp(0.0, 1.0);
        // Exponential-out: a quick burst that eases into its final radius.
        let eased = if t >= 1.0 {
            1.0
        } else {
            1.0 - 2f32.powf(-10.0 * t)
        };
        let radius = burst.start_radius + (burst.end_radius - burst.start_radius) * eased;
        transform.scale = Vec3::splat(radius);
        // Dissipate: alpha falls to zero as it reaches full radius.
        if let Some(m) = materials.get_mut(&material.0) {
            m.base_color.set_alpha(burst.base_alpha * (1.0 - t));
        }
        if burst.elapsed >= burst.duration {
            commands.entity(entity).despawn();
        }
    }
}

/// Casts Holy Nova from `caster`: the burst VFX parented to them plus its
/// sound, emitted spatially FROM the caster. Shared by the GAME's live input
/// (`1`), ghost playback, and boss loadout playback, so live take and replay
/// can never drift. (The storybook assembles its own theme-tinted variant
/// from [`holy_nova`] + [`play_spatial_sfx`] directly.)
pub fn cast_holy_nova(
    commands: &mut Commands,
    meshes: &AbilityMeshes,
    materials: &mut Assets<StandardMaterial>,
    sfx: &AbilitySfx,
    mix: &AudioMix,
    scale: &DefaultSpatialScale,
    caster: Entity,
) {
    let holy_gold = Color::srgb(1.0, 0.9, 0.55);
    let burst = holy_nova(meshes.sphere.clone(), materials, holy_gold);
    commands.spawn((burst, ChildOf(caster)));
    play_spatial_sfx(
        commands,
        sfx.holy_nova.clone(),
        caster,
        AbilityId::HolyNova.sfx_profile(),
        mix,
        scale,
    );
}

/// Every ability's preloaded sound — one handle per ability (an ability is VFX +
/// SFX). Loaded once at startup by [`AbilityPlugin`] so the first cast has no
/// hitch; add a field here when you add an ability.
#[derive(Resource)]
pub struct AbilitySfx {
    pub holy_nova: Handle<AudioSource>,
}

/// Shared ability meshes, built once at startup — every Holy Nova clones ONE
/// unit-sphere handle instead of allocating a fresh mesh asset per cast (which
/// would churn `Assets<Mesh>` forever under ghost playback).
#[derive(Resource)]
pub struct AbilityMeshes {
    pub sphere: Handle<Mesh>,
}

/// Preloads every ability's sound effect + shared meshes up front.
fn load_assets(mut commands: Commands, assets: Res<AssetServer>, mut meshes: ResMut<Assets<Mesh>>) {
    commands.insert_resource(AbilitySfx {
        holy_nova: assets.load("abilities/holy_nova.ogg"),
    });
    commands.insert_resource(AbilityMeshes {
        sphere: meshes.add(Sphere::new(1.0)),
    });
}

/// Registers the ability-VFX systems and preloads [`AbilitySfx`] + [`AbilityMeshes`].
pub struct AbilityPlugin;

impl Plugin for AbilityPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_assets)
            .add_systems(Update, update_ability_bursts);
    }
}
