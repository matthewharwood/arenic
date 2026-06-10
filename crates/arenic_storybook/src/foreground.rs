//! Per-arena **atmosphere** — a cloud / fog / vignette shader rendered on TWO
//! camera-attached planes to create dynamic depth layering:
//!
//! - a **skybox** plane FAR behind the arena floor (the main backdrop), and
//! - a **foreground** plane BETWEEN the camera and the floor (a near haze).
//!
//! The arena floor is drawn semi-transparent (see [`crate::stage`]), so the
//! skybox shows through the tiles — the boss floats on a translucent floor over
//! a drifting sky, with a subtle haze in front. All nine arenas share this; the
//! look (style, motion, colour) is per-arena and re-tones with the theme.

use arenic_game::atmosphere::{AtmospherePlugin, CloudFog, Plane, Voice, cloud_material};
use arenic_game::grid::board_center;
use arenic_game::swarm::{
    SwarmMember, SwarmSpec, animate_swarm, mote_mesh, swarm_amp, swarm_home, swarm_material,
};
use arenic_game::theme::ActiveTheme;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;

use crate::arena::{DEFAULT_FG, DEFAULT_SKY, spec};
use crate::layers::{Layer, OnLayer};
use crate::stage::StageCamera;
use crate::stories::StoryId;
use crate::storybook::CurrentStory;

/// Foreground plane: just in front of the camera (local `-Z`), covers the FOV.
const FG_DIST: f32 = 3.0;
const FG_W: f32 = 2.7;
const FG_H: f32 = 1.55;
/// Skybox plane: far behind the arena floor, large enough to fill the backdrop.
const SKY_DIST: f32 = 40.0;
const SKY_W: f32 = 30.0;
const SKY_H: f32 = 17.0;

/// One of the two atmosphere planes (skybox or foreground).
#[derive(Component, Clone, Copy)]
#[component(immutable)]
struct AtmospherePlane;

/// Handles to the two atmosphere materials, so per-arena params can be updated.
#[derive(Resource)]
struct CloudMats {
    sky: Handle<CloudFog>,
    fg: Handle<CloudFog>,
}

/// Adds the shared atmosphere pipeline ([`arenic_game::atmosphere`]), attaches the
/// skybox + foreground planes to the stage camera once it exists, and keeps their
/// per-arena params in sync.
pub struct ForegroundPlugin;

impl Plugin for ForegroundPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(AtmospherePlugin).add_systems(
            Update,
            (
                attach_atmosphere,
                // Re-shape the planes only when the story or theme actually changes.
                update_atmosphere.run_if(
                    resource_exists::<CloudMats>
                        .and(resource_changed::<CurrentStory>.or(resource_changed::<ActiveTheme>)),
                ),
                // The discrete drifting sky-swarm — repopulated per arena, animated
                // every frame, re-toned on theme change.
                respawn_swarm.run_if(resource_changed::<CurrentStory>),
                animate_swarm,
                retone_swarm
                    .run_if(resource_exists::<SwarmStyle>.and(resource_changed::<ActiveTheme>)),
            ),
        );
    }
}

/// The `(skybox, foreground)` voices for a story — the arena's pair from
/// [`crate::arena::spec`], or the generic [`DEFAULT_SKY`]/[`DEFAULT_FG`] for the
/// non-arena (design-token) pages. See [`crate::arena`] for the harmony framework.
fn arena_voices(story: Option<StoryId>) -> (Voice, Voice) {
    match story.and_then(spec) {
        Some(s) => (s.sky, s.fg),
        None => (DEFAULT_SKY, DEFAULT_FG),
    }
}

/// Spawns the skybox + foreground planes as children of the stage camera the
/// frame it appears, seeding params from the current story + theme.
fn attach_atmosphere(
    mut commands: Commands,
    camera: Single<Entity, Added<StageCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<CloudFog>>,
    current: Res<CurrentStory>,
    active: Res<ActiveTheme>,
) {
    let camera = *camera;
    let theme = active.palette();
    let (sky_v, fg_v) = arena_voices(current.0);
    let sky = mats.add(cloud_material(sky_v, &theme, Plane::Skybox));
    let fg = mats.add(cloud_material(fg_v, &theme, Plane::Foreground));

    // Skybox: far behind the floor (local -Z), large, fills the backdrop.
    commands.spawn((
        Mesh3d(meshes.add(Rectangle::new(SKY_W, SKY_H))),
        MeshMaterial3d(sky.clone()),
        Transform::from_xyz(0.0, 0.0, -SKY_DIST),
        AtmospherePlane,
        OnLayer(Layer::Skybox),
        NotShadowCaster,
        NotShadowReceiver,
        ChildOf(camera),
    ));
    // Foreground: just in front of the camera.
    commands.spawn((
        Mesh3d(meshes.add(Rectangle::new(FG_W, FG_H))),
        MeshMaterial3d(fg.clone()),
        Transform::from_xyz(0.0, 0.0, -FG_DIST),
        AtmospherePlane,
        OnLayer(Layer::Foreground),
        NotShadowCaster,
        NotShadowReceiver,
        ChildOf(camera),
    ));
    commands.insert_resource(CloudMats { sky, fg });
}

/// Reshapes/re-tones both planes when the story or theme changes (the clouds
/// animate themselves from `globals.time`, so this only runs on a real change).
fn update_atmosphere(
    current: Res<CurrentStory>,
    active: Res<ActiveTheme>,
    handles: Res<CloudMats>,
    mut mats: ResMut<Assets<CloudFog>>,
) {
    let theme = active.palette();
    let (sky_v, fg_v) = arena_voices(current.0);
    if let Some(m) = mats.get_mut(&handles.sky) {
        *m = cloud_material(sky_v, &theme, Plane::Skybox);
    }
    if let Some(m) = mats.get_mut(&handles.fg) {
        *m = cloud_material(fg_v, &theme, Plane::Foreground);
    }
}

// ===========================================================================
// Foreground sky-swarm — the discrete drifting "fauna/probe" layer.
//
// A small, sparse cloud of translucent emissive motes drifting in a low ring
// UNDER the liquid-glass floor — they ride z ∈ [-2.3, -0.5], always BELOW the
// floor plane (z = -0.02), so they go *around and under* the ground but never
// THROUGH it. Seen glowing softly up through the translucent floor (and biased to
// the board's outer ring so the boss reads clearly above them).
//
// One grammar (shared silhouette families + the dark-mass/hollow-light/bloom
// look); per-arena DISTANCE is carried by the motion vector, cadence and the
// arena theme colour. Mirrors the data-driven hollow-light pattern.
// ===========================================================================

/// The shared swarm material, so a theme switch can re-tone the whole swarm at once.
#[derive(Resource)]
struct SwarmStyle {
    mat: Handle<StandardMaterial>,
}

/// Despawns the old swarm and spawns the selected arena's, in a low outer ring
/// UNDER the board (seen through the liquid glass). A no-op for non-arena stories.
fn respawn_swarm(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    current: Res<CurrentStory>,
    active: Res<ActiveTheme>,
    existing: Query<Entity, With<SwarmMember>>,
) {
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    let Some(story) = current.0 else {
        return;
    };
    let Some(SwarmSpec {
        motion,
        mote,
        count,
        scale,
    }) = spec(story).map(|s| s.swarm)
    else {
        return;
    };

    let theme = active.palette();
    let mesh = meshes.add(mote_mesh(mote, scale));
    let mat = materials.add(swarm_material(theme.primary));
    commands.insert_resource(SwarmStyle { mat: mat.clone() });

    let center = board_center();
    // The ring placement + per-index amplitude are the shared swarm grammar.
    for i in 0..count {
        let home = swarm_home(i, count, center);
        commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(mat.clone()),
            Transform::from_translation(home),
            SwarmMember {
                motion,
                phase: i as f32 * 0.7,
                home,
                amp: swarm_amp(motion, i),
            },
            OnLayer(Layer::Swarm),
            NotShadowCaster,
            NotShadowReceiver,
        ));
    }
}

/// Re-tones the shared swarm material when the theme changes.
fn retone_swarm(
    active: Res<ActiveTheme>,
    style: Res<SwarmStyle>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if let Some(m) = materials.get_mut(&style.mat) {
        *m = swarm_material(active.palette().primary);
    }
}

// The cloud material (`CloudFog` / `cloud_material`), the per-motion drift math
// (`swarm_offset`), and the shared `animate_swarm` driver (with their tests) now
// live in `arenic_game::{atmosphere, swarm}`, shared with the game.
