//! Reusable ability pieces — exercised by the storybook's `abilities/*` stories.
//!
//! An ability is **VFX + SFX**: [`AbilityBurst`] is a self-contained expanding-
//! sphere effect (e.g. Holy Nova) — a unit sphere scaled from `start_radius` to
//! `end_radius` over `duration`, eased and fading out, then despawned — and
//! [`play_sfx`] fires its one-shot sound. Both are generic *pieces* (no class, no
//! timeline). Spawn [`holy_nova`] + [`play_sfx`] when an ability casts, and add
//! [`AbilityPlugin`] to drive the VFX and preload every ability's sound into
//! [`AbilitySfx`].

use bevy::audio::AudioSource;
use bevy::color::Alpha;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;

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
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    color: Color,
) -> impl Bundle + use<> {
    const START: f32 = 0.3;
    const BASE_ALPHA: f32 = 0.4;
    let l = color.to_linear();
    let mesh = meshes.add(Sphere::new(1.0));
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
        Mesh3d(mesh),
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

/// Plays a one-shot ability sound that cleans itself up when finished. Fire it
/// alongside the VFX when an ability casts (the sound is its own entity, so it
/// plays to completion even after the VFX despawns).
pub fn play_sfx(commands: &mut Commands, sound: Handle<AudioSource>) {
    commands.spawn((AudioPlayer::new(sound), PlaybackSettings::DESPAWN));
}

/// Every ability's preloaded sound — one handle per ability (an ability is VFX +
/// SFX). Loaded once at startup by [`AbilityPlugin`] so the first cast has no
/// hitch; add a field here when you add an ability.
#[derive(Resource)]
pub struct AbilitySfx {
    pub holy_nova: Handle<AudioSource>,
}

/// Preloads every ability's sound effect up front.
fn load_sfx(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(AbilitySfx {
        holy_nova: assets.load("abilities/holy_nova.ogg"),
    });
}

/// Registers the ability-VFX systems and preloads [`AbilitySfx`].
pub struct AbilityPlugin;

impl Plugin for AbilityPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_sfx)
            .add_systems(Update, update_ability_bursts);
    }
}
