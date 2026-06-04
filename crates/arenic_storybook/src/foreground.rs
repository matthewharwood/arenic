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

use crate::layers::{Layer, LayerTag};
use crate::stage::StageCamera;
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
        // The skybox is an OPAQUE backdrop so the frosted-glass floor can refract
        // it (specular transmission samples the opaque scene). The foreground
        // blends over the scene.
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
        app.add_plugins(MaterialPlugin::<CloudFog>::default()).add_systems(
            Update,
            (
                attach_atmosphere,
                update_atmosphere,
                // The discrete drifting sky-swarm — repopulated per arena, animated
                // every frame, re-toned on theme change.
                respawn_swarm.run_if(resource_changed::<CurrentStory>),
                animate_swarm,
                retone_swarm,
            ),
        );
    }
}

fn lin(c: Color, a: f32) -> Vec4 {
    let l = c.to_linear();
    Vec4::new(l.red, l.green, l.blue, a)
}

/// One atmosphere voice: `(style, scale, drift_x, drift_y, coverage, vignette, speed)`.
type Voice = (f32, f32, f32, f32, f32, f32, f32);

/// Per-arena `(skybox, foreground)` voices.
///
/// HARMONY FRAMEWORK: the two planes are different instruments, not the same
/// effect at another scale. The SKYBOX is a continuous FIELD atmosphere (styles
/// banded/billows/vertical/spiral/hearth); the FOREGROUND is a discrete or
/// structured NEAR effect (embers/sweep/pulse/grain/streaks/ripple). They
/// contrast on effect TYPE (always field vs near), drift DIRECTION (perpendicular
/// or opposite), REGISTER (the foreground is finer), and TEMPO (different speed).
/// Across arenas both the skybox set and the foreground set stay distinct
/// (colour + style + direction). Style ids: banded(0), billows(1), vertical(2),
/// spiral(3), embers(4), sweep(5), pulse(6), hearth(7), grain(8), streaks(9),
/// ripple(10).
fn arena_voices(story: Option<StoryId>) -> (Voice, Voice) {
    match story {
        //                              SKYBOX (field)                              FOREGROUND (near — counterpoint)
        Some(StoryId::Hunter) => (
            (0.0, 3.0, 0.030, 0.000, 0.55, 0.40, 0.45), // banded bands drifting sideways
            (9.0, 5.0, 0.000, -0.060, 0.50, 0.0, 0.90), // fine streaks falling, faster (perp)
        ),
        Some(StoryId::Guildmaster) => (
            (7.0, 2.0, 0.008, 0.004, 0.58, 0.30, 0.40), // hearth haze breathing in place
            (4.0, 6.0, 0.000, 0.050, 0.50, 0.0, 0.50),  // warm motes rising (discrete)
        ),
        Some(StoryId::Cardinal) => (
            (1.0, 2.5, 0.012, 0.006, 0.50, 0.35, 0.35), // gold incense billows rolling
            (8.0, 9.0, 0.000, -0.050, 0.55, 0.0, 0.50), // fine gilt grain sifting down
        ),
        Some(StoryId::Forager) => (
            (2.0, 2.5, 0.000, 0.050, 0.58, 0.40, 1.00), // green spores rising (vertical)
            (8.0, 7.0, 0.060, 0.000, 0.50, 0.0, 0.70),  // wind-grain drifting sideways (perp)
        ),
        Some(StoryId::Warrior) => (
            (2.0, 2.2, 0.000, -0.040, 0.62, 0.45, 1.00), // ash lid pressing down
            (4.0, 5.0, 0.000, 0.060, 0.50, 0.0, 1.40),   // sparks rising fast (opposite)
        ),
        Some(StoryId::Thief) => (
            (0.0, 2.5, 0.000, 0.025, 0.50, 0.45, 0.40), // cyan bands, vertical drift
            (5.0, 2.5, 0.030, 0.000, 0.50, 0.0, 0.50),  // watch-band sweeping across (perp)
        ),
        Some(StoryId::Alchemist) => (
            (1.0, 2.5, -0.020, 0.015, 0.60, 0.40, 1.00), // lime smog rolling, two layers
            (10.0, 4.0, 0.000, 0.000, 0.50, 0.0, 0.80),  // bubble-ripples popping (radial)
        ),
        Some(StoryId::Merchant) => (
            (3.0, 2.2, 0.000, 0.000, 0.54, 0.45, 0.70), // gold-plum smoke swirling
            (9.0, 5.0, 0.040, -0.030, 0.50, 0.0, 0.90), // coin-streaks tumbling diagonally
        ),
        Some(StoryId::Bard) => (
            (1.0, 3.0, 0.015, 0.010, 0.54, 0.40, 0.50), // violet haze drifting
            (6.0, 3.0, 0.010, 0.010, 0.54, 0.0, 1.50),  // pink/cyan throb on the beat (rhythm)
        ),
        _ => (
            (1.0, 2.5, 0.010, 0.010, 0.50, 0.35, 0.60),
            (8.0, 7.0, -0.010, 0.000, 0.40, 0.0, 0.80),
        ),
    }
}

fn voice_params(v: Voice, theme: &Theme, foreground: bool) -> CloudParams {
    let (style, scale, dx, dy, cov, vig, speed) = v;
    CloudParams {
        tint: lin(theme.base_300, cov),
        accent: lin(theme.primary, speed),
        vignette: lin(theme.base_300, vig),
        flags: Vec4::new(style, scale, dx, dy),
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
    if !current.is_changed() && !active.is_changed() {
        return;
    }
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

/// Board centre — the §1 midpoint of the 66×31 field (`0..16.25`, `0..7.5`).
fn board_center() -> Vec2 {
    Vec2::new(8.125, 3.75)
}

/// A small mote silhouette family, shared across arenas to keep one grammar.
#[derive(Clone, Copy)]
enum Mote {
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
enum SwarmMotion {
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

/// Per-arena `(motion, silhouette, count, scale)`. Colour is always the arena's
/// theme accent (`primary`), so it re-tones and stays per-arena distinct.
fn arena_swarm(story: StoryId) -> Option<(SwarmMotion, Mote, usize, f32)> {
    use Mote::{Bubble, Dart, Flake, Spark};
    use SwarmMotion::*;
    Some(match story {
        StoryId::Hunter => (Patrol, Dart, 12, 0.11), // scout darts patrol the sightlines
        StoryId::Guildmaster => (HearthRise, Spark, 14, 0.07), // warm hearth embers rising
        StoryId::Cardinal => (PendulumFall, Flake, 12, 0.10), // gilt-leaf pendulum descent
        StoryId::Forager => (GustDrift, Spark, 16, 0.06), // wind-borne spores
        StoryId::Warrior => (UpdraftChurn, Flake, 13, 0.09), // forge cinders churn up
        StoryId::Thief => (FurtiveDart, Dart, 10, 0.09), // furtive bats / coin-flakes
        StoryId::Alchemist => (OpposingDrift, Bubble, 14, 0.08), // bubbles up / droplets down
        StoryId::Merchant => (TumbleArc, Flake, 12, 0.10), // tumbling coins / die-pips
        StoryId::Bard => (BeatDrift, Flake, 16, 0.09), // confetti on the beat
        _ => return None,
    })
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
    let Some((motion, mote, count, scale)) = arena_swarm(story) else {
        return;
    };

    let theme = active.palette();
    let mesh = meshes.add(mote_mesh(mote, scale));
    let mat = materials.add(swarm_material(theme.primary));
    commands.insert_resource(SwarmStyle { mat: mat.clone() });

    let center = board_center();
    // A golden-angle spread keeps the ring even; `r` stays in the outer band so
    // motes ride the board edges + margin, never the central boss zone. `z = -1.4`
    // seats the whole swarm UNDER the floor (z = -0.02); with amp.z = 0.9 it rides
    // z ∈ [-2.3, -0.5] — under and around the ground, never through it.
    for i in 0..count {
        let a = i as f32 * 2.399_963_2; // golden angle
        let r = 0.65 + 0.4 * ((i as f32 + 0.5) / count as f32).sqrt();
        let home = Vec3::new(center.x + a.cos() * r * 7.5, center.y + a.sin() * r * 3.4, -1.4);
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
                amp: Vec3::new(1.4, 1.0, 0.9 * z_sign),
            },
            LayerTag(Layer::Swarm),
            NotShadowCaster,
            NotShadowReceiver,
        ));
    }
}

/// Drives every swarm member's drift + spin from its motion archetype.
fn animate_swarm(time: Res<Time>, mut swarm: Query<(&SwarmMember, &mut Transform)>) {
    let t = time.elapsed_secs() * 0.5; // global slow factor — non-distracting
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
    if !active.is_changed() {
        return;
    }
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
            Vec3::new(amp.x * (2.0 * s(0.35)).tanh(), amp.y * s(0.16), amp.z * 0.2 * s(0.5)),
            Quat::from_rotation_z(0.3 * s(0.4)),
        ),
        HearthRise => (
            Vec3::new(amp.x * 0.4 * s(0.3), amp.y * 0.3 * s(0.25), amp.z * saw(0.10)),
            Quat::IDENTITY,
        ),
        PendulumFall => (
            Vec3::new(amp.x * s(0.6), amp.y * 0.3 * s(0.4), amp.z * (1.0 - saw(0.08))),
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
            Vec3::new(amp.x * 0.4 * s(0.3), amp.y * 0.3 * s(0.27), amp.z * (0.5 + 0.5 * s(0.4))),
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
            Vec3::new(amp.x * 0.3 * s(0.2), amp.y * 0.3 * s(0.18), amp.z * saw(0.12)),
            Quat::IDENTITY,
        ),
        TumbleArc => (
            Vec3::new(amp.x * s(0.3), amp.y * 0.4 * s(0.25), amp.z * s(0.4)),
            Quat::from_axis_angle(Vec3::new(0.4, 1.0, 0.2).normalize(), t * 2.0 + phase),
        ),
        BeatDrift => {
            let beat = (t * 2.0 + phase).sin().max(0.0).powi(4);
            (
                Vec3::new(amp.x * s(0.2), amp.y * s(0.16), -amp.z * 0.3 * saw(0.1) + amp.z * 0.5 * beat),
                Quat::from_rotation_z(0.5 * s(0.35)),
            )
        }
    }
}
