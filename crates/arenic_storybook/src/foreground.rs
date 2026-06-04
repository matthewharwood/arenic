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

use arenic_game::theme::{ActiveTheme, Theme};
use bevy::asset::Asset;
use bevy::color::Alpha;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::pbr::{Material, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

use crate::arena::{DEFAULT_FG, DEFAULT_SKY, SwarmSpec, Voice, spec};
use crate::layers::{Layer, LayerTag};
use crate::stage::{StageCamera, board_center};
use crate::stories::StoryId;
use crate::storybook::CurrentStory;

const SHADER: &str = "shaders/arena_fog.wgsl";

/// Foreground plane: just in front of the camera (local `-Z`), covers the FOV.
const FG_DIST: f32 = 3.0;
const FG_W: f32 = 2.7;
const FG_H: f32 = 1.55;
/// Skybox plane: far behind the arena floor, large enough to fill the backdrop.
const SKY_DIST: f32 = 40.0;
const SKY_W: f32 = 30.0;
const SKY_H: f32 = 17.0;

/// One of the two atmosphere planes (skybox or foreground).
#[derive(Component)]
struct AtmospherePlane;

/// Handles to the two atmosphere materials, so per-arena params can be updated.
#[derive(Resource)]
struct CloudMats {
    sky: Handle<CloudFog>,
    fg: Handle<CloudFog>,
}

/// Per-arena shape + colour for the atmosphere shader. One uniform buffer.
#[derive(Clone, Copy, Default, ShaderType)]
struct CloudParams {
    /// rgb = cloud/smoke colour, a = coverage.
    tint: Vec4,
    /// rgb = cloud glow (theme accent), a = animation speed.
    accent: Vec4,
    /// a = corner-vignette strength (rgb unused).
    vignette: Vec4,
    /// x = style, y = noise scale, z = drift.x, w = drift.y.
    flags: Vec4,
    /// x = 0 skybox (full) / 1 foreground (edge-framed); y = alpha scale.
    mode: Vec4,
}

/// The atmosphere cloud/fog/vignette material.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
struct CloudFog {
    #[uniform(0)]
    params: CloudParams,
}

impl Material for CloudFog {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        // The skybox is an OPAQUE backdrop — a solid sky the translucent
        // liquid-glass floor and the under-floor swarm read against. The
        // foreground BLENDS over the scene as a near haze.
        if self.params.mode.x > 0.5 {
            AlphaMode::Blend
        } else {
            AlphaMode::Opaque
        }
    }
}

/// Adds the atmosphere pipeline, attaches the skybox + foreground planes to the
/// stage camera once it exists, and keeps their per-arena params in sync.
pub struct ForegroundPlugin;

impl Plugin for ForegroundPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<CloudFog>::default())
            .add_systems(
                Update,
                (
                    attach_atmosphere,
                    // Re-shape the planes only when the story or theme actually changes.
                    update_atmosphere.run_if(
                        resource_changed::<CurrentStory>.or(resource_changed::<ActiveTheme>),
                    ),
                    // The discrete drifting sky-swarm — repopulated per arena, animated
                    // every frame, re-toned on theme change.
                    respawn_swarm.run_if(resource_changed::<CurrentStory>),
                    animate_swarm,
                    retone_swarm.run_if(resource_changed::<ActiveTheme>),
                ),
            );
    }
}

fn lin(c: Color, a: f32) -> Vec4 {
    let l = c.to_linear();
    Vec4::new(l.red, l.green, l.blue, a)
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

/// Packs one [`Voice`] into the shader uniform, tinted by the active theme.
fn voice_params(v: Voice, theme: &Theme, foreground: bool) -> CloudParams {
    CloudParams {
        tint: lin(theme.base_300, v.coverage),
        accent: lin(theme.primary, v.speed),
        vignette: lin(theme.base_300, v.vignette),
        flags: Vec4::new(v.style.as_f32(), v.scale, v.drift.x, v.drift.y),
        // Foreground is edge-framed + subtle; the skybox is full + the main layer.
        mode: if foreground {
            Vec4::new(1.0, 0.5, 0.0, 0.0)
        } else {
            Vec4::new(0.0, 1.0, 0.0, 0.0)
        },
    }
}

/// Spawns the skybox + foreground planes as children of the stage camera the
/// frame it appears, seeding params from the current story + theme.
fn attach_atmosphere(
    mut commands: Commands,
    camera: Query<Entity, Added<StageCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<CloudFog>>,
    current: Res<CurrentStory>,
    active: Res<ActiveTheme>,
) {
    let Ok(camera) = camera.single() else {
        return;
    };
    let theme = active.palette();
    let (sky_v, fg_v) = arena_voices(current.0);
    let sky = mats.add(CloudFog {
        params: voice_params(sky_v, &theme, false),
    });
    let fg = mats.add(CloudFog {
        params: voice_params(fg_v, &theme, true),
    });

    // Skybox: far behind the floor (local -Z), large, fills the backdrop.
    commands.spawn((
        Mesh3d(meshes.add(Rectangle::new(SKY_W, SKY_H))),
        MeshMaterial3d(sky.clone()),
        Transform::from_xyz(0.0, 0.0, -SKY_DIST),
        AtmospherePlane,
        LayerTag(Layer::Skybox),
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
        LayerTag(Layer::Foreground),
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
    handles: Option<Res<CloudMats>>,
    mut mats: ResMut<Assets<CloudFog>>,
) {
    let Some(handles) = handles else {
        return;
    };
    let theme = active.palette();
    let (sky_v, fg_v) = arena_voices(current.0);
    if let Some(m) = mats.get_mut(&handles.sky) {
        m.params = voice_params(sky_v, &theme, false);
    }
    if let Some(m) = mats.get_mut(&handles.fg) {
        m.params = voice_params(fg_v, &theme, true);
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

/// A small mote silhouette family, shared across arenas to keep one grammar.
#[derive(Clone, Copy)]
pub(crate) enum Mote {
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
pub(crate) enum SwarmMotion {
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

/// One drifting swarm instance. `home` is its sky anchor; `amp` is its drift
/// amplitude (the `z` sign also picks rise-vs-fall for [`SwarmMotion::OpposingDrift`]).
#[derive(Component)]
struct SwarmMember {
    motion: SwarmMotion,
    phase: f32,
    home: Vec3,
    amp: Vec3,
}

/// The shared swarm material, so a theme switch can re-tone the whole swarm at once.
#[derive(Resource)]
struct SwarmStyle {
    mat: Handle<StandardMaterial>,
}

/// The shared, translucent, lightly-emissive material for a swarm tinted `c`.
fn swarm_material(c: Color) -> StandardMaterial {
    let l = c.to_linear();
    StandardMaterial {
        base_color: c.with_alpha(0.55),
        emissive: LinearRgba::rgb(l.red, l.green, l.blue) * 1.6,
        alpha_mode: AlphaMode::Blend,
        ..default()
    }
}

/// A small mote mesh for the given silhouette family at `s` units.
fn mote_mesh(mote: Mote, s: f32) -> Mesh {
    match mote {
        Mote::Spark => Sphere::new(s * 0.5).into(),
        Mote::Bubble => Sphere::new(s * 0.6).into(),
        Mote::Flake => Cuboid::new(s, s, s * 0.12).into(),
        Mote::Dart => Cuboid::new(s * 0.3, s * 1.6, s * 0.1).into(),
    }
}

/// Swarm geometry — the single place the "under the floor, never through it"
/// invariant is defined. The ring half-axes sit a little inside the board
/// half-extents (~8.1 × 3.75) so motes hug the rim. `SWARM_Z` seats the swarm
/// below the floor plane (z = -0.02); since the motion never exceeds `±SWARM_AMP.z`,
/// the swarm rides z ∈ [SWARM_Z - amp, SWARM_Z + amp] = [-2.3, -0.5], always under it.
const SWARM_RING_X: f32 = 7.5;
const SWARM_RING_Y: f32 = 3.4;
const SWARM_Z: f32 = -1.4;
const SWARM_AMP: Vec3 = Vec3::new(1.4, 1.0, 0.9);
/// Global slow factor on swarm time — keeps the drift calm and non-distracting.
const SWARM_TIME_SCALE: f32 = 0.5;

/// The liquid-glass floor plane (see `stage.rs` board). The swarm lives under it.
const FLOOR_PLANE_Z: f32 = -0.02;
/// Compile-time invariant: the swarm's highest reach stays below the floor, so it
/// goes *around and under* the ground but never *through* it. A motion can't exceed
/// `±SWARM_AMP.z`, so the ceiling is `SWARM_Z + SWARM_AMP.z`.
const _: () = assert!(SWARM_Z + SWARM_AMP.z < FLOOR_PLANE_Z);

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
    // A golden-angle spread keeps the ring even; `r` stays in the outer band so
    // motes ride the board edges + margin, never the central boss zone (see the
    // SWARM_* consts above for the under-floor invariant).
    for i in 0..count {
        let a = i as f32 * 2.399_963_2; // golden angle
        let r = 0.65 + 0.4 * ((i as f32 + 0.5) / count as f32).sqrt();
        let home = Vec3::new(
            center.x + a.cos() * r * SWARM_RING_X,
            center.y + a.sin() * r * SWARM_RING_Y,
            SWARM_Z,
        );
        // OpposingDrift: half the bubbles rise, half the droplets fall.
        let z_sign = if matches!(motion, SwarmMotion::OpposingDrift) && i % 2 == 1 {
            -1.0
        } else {
            1.0
        };
        commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(mat.clone()),
            Transform::from_translation(home),
            SwarmMember {
                motion,
                phase: i as f32 * 0.7,
                home,
                amp: Vec3::new(SWARM_AMP.x, SWARM_AMP.y, SWARM_AMP.z * z_sign),
            },
            LayerTag(Layer::Swarm),
            NotShadowCaster,
            NotShadowReceiver,
        ));
    }
}

/// Drives every swarm member's drift + spin from its motion archetype.
fn animate_swarm(time: Res<Time>, mut swarm: Query<(&SwarmMember, &mut Transform)>) {
    let t = time.elapsed_secs() * SWARM_TIME_SCALE;
    for (m, mut tf) in &mut swarm {
        let (offset, rot) = swarm_offset(m.motion, m.phase, t, m.amp);
        tf.translation = m.home + offset;
        tf.rotation = rot;
    }
}

/// Re-tones the shared swarm material when the theme changes.
fn retone_swarm(
    active: Res<ActiveTheme>,
    style: Option<Res<SwarmStyle>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(style) = style else {
        return;
    };
    if let Some(m) = materials.get_mut(&style.mat) {
        *m = swarm_material(active.palette().primary);
    }
}

/// Per-motion `(drift offset, rotation)` at phase `p` and time `t`. Pure function
/// of time — deterministic, no RNG; the per-index `phase` staggers the swarm.
fn swarm_offset(motion: SwarmMotion, phase: f32, t: f32, amp: Vec3) -> (Vec3, Quat) {
    use SwarmMotion::*;
    let s = |k: f32| (t * k + phase).sin();
    // 0..1 sawtooth (rise-and-reset), for looping vertical drifts.
    let saw = |k: f32| (t * k + phase).rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU;
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
            // amp.z sign (set per index) decides rise vs fall.
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
    fn swarm_never_rises_through_the_floor() {
        // The resting-ceiling bound (SWARM_Z + SWARM_AMP.z < floor) is a
        // compile-time invariant (see FLOOR_PLANE_Z assertion above). Here we also
        // sample every archetype over time + phase, so a future motion tweak that
        // pushes a mote above its amplitude bound is caught too.
        for (m, motion) in EVERY_MOTION.into_iter().enumerate() {
            for i in 0..200 {
                let t = i as f32 * 0.31;
                let phase = m as f32 * 0.7;
                let (offset, _) = swarm_offset(motion, phase, t, SWARM_AMP);
                let z = SWARM_Z + offset.z;
                assert!(
                    z < FLOOR_PLANE_Z,
                    "motion #{m} rose to z = {z} (floor is {FLOOR_PLANE_Z})"
                );
            }
        }
    }

    #[test]
    fn voice_mode_flags_distinguish_skybox_from_foreground() {
        let theme = arenic_game::theme::ThemeId::Dark.palette();
        assert_eq!(
            voice_params(DEFAULT_SKY, &theme, false).mode,
            Vec4::new(0.0, 1.0, 0.0, 0.0),
            "skybox is opaque/full",
        );
        assert_eq!(
            voice_params(DEFAULT_FG, &theme, true).mode,
            Vec4::new(1.0, 0.5, 0.0, 0.0),
            "foreground is edge-framed/half-alpha",
        );
    }
}
