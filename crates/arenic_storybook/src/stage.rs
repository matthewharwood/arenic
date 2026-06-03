//! A top-down arena "stage" rendered into a texture that the `guildmaster`
//! story displays inside the canvas.
//!
//! It reproduces the reference framing from `_docs/UNITS_AND_SCALE.md`: a flat
//! board on the XY plane (a `66 × 31` dot grid with a border) viewed by a
//! near-orthographic perspective camera parked far back on `+Z` (vertical FOV
//! `π/8`, eye at `z = 24`, looking straight down `−Z`). The Guildmaster sits at
//! the centre as a single `0.25`-unit token, lit by one light.
//!
//! Per ARE-3 the camera, light and board all live here (in the storybook); only
//! the Guildmaster piece comes from `arenic_game`. Bevy draws 3D through a
//! `Camera3d`, so the scene is rendered off-screen into an image the story shows
//! as an `ImageNode`.

use std::f32::consts::FRAC_PI_8;

use arenic_game::guildmaster::guildmaster;
use arenic_game::orbit::OrbitCamera;
use bevy::asset::RenderAssetUsages;
use bevy::camera::RenderTarget;
use bevy::light::ShadowFilteringMethod;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};

/// Render-target resolution — 16:9, matching the doc's framebuffer.
const STAGE_W: u32 = 1024;
const STAGE_H: u32 = 576;

// Arena dimensions, straight from UNITS_AND_SCALE §1.
const GRID_W: i32 = 66;
const GRID_H: i32 = 31;
const TILE: f32 = 0.25;

/// Handle to the texture the stage renders into.
#[derive(Resource)]
pub struct Stage3d {
    pub image: Handle<Image>,
}

/// Builds the render-to-texture arena once at startup.
pub fn setup_stage(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    assets: Res<AssetServer>,
) {
    // Board centre = midpoint of the tile-centre extents (0..16.25, 0..7.5).
    let cx = (GRID_W - 1) as f32 * TILE * 0.5; // 8.125
    let cy = (GRID_H - 1) as f32 * TILE * 0.5; // 3.75

    // --- Render-target texture ---
    let size = Extent3d {
        width: STAGE_W,
        height: STAGE_H,
        depth_or_array_layers: 1,
    };
    let mut image = Image::new_fill(
        size,
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING
        | TextureUsages::COPY_DST
        | TextureUsages::RENDER_ATTACHMENT;
    let image = images.add(image);
    commands.insert_resource(Stage3d {
        image: image.clone(),
    });

    // --- Near-orthographic top-down camera (UNITS_AND_SCALE §3) ---
    // Driven by OrbitCamera so the story's "unlock" button can orbit/pan/zoom it;
    // home pose (yaw=pitch=0, radius=24) is exactly the §3 top-down view.
    let orbit = OrbitCamera::new(Vec3::new(cx, cy, 0.0), 0.0, 0.0, 24.0);
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: FRAC_PI_8, // 22.5° vertical
            near: 0.05,
            far: 150.0,
            ..default()
        }),
        Camera {
            order: -1,
            clear_color: ClearColorConfig::Custom(Color::srgb(0.90, 0.90, 0.93)),
            ..default()
        },
        RenderTarget::Image(image.into()),
        orbit.transform(),
        orbit,
        ShadowFilteringMethod::Gaussian,
        AmbientLight {
            brightness: 600.0,
            ..default()
        },
    ));

    // --- One light, angled so the token drops a soft shadow on the board ---
    commands.spawn((
        DirectionalLight {
            illuminance: 6000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(cx + 3.0, cy + 3.0, 8.0).looking_at(Vec3::new(cx, cy, 0.0), Vec3::Z),
    ));

    // --- Board surface (catches the shadow; the light backdrop of the grid) ---
    let board_w = GRID_W as f32 * TILE + 0.7;
    let board_h = GRID_H as f32 * TILE + 0.7;
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(board_w, board_h, 0.02))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.94, 0.94, 0.96),
            perceptual_roughness: 0.95,
            ..default()
        })),
        Transform::from_xyz(cx, cy, -0.02),
    ));

    // --- Dot grid: a small disc at every tile centre (Circle faces +Z) ---
    let dot_mesh = meshes.add(Circle::new(0.012));
    let dot_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.62, 0.62, 0.68),
        unlit: true,
        ..default()
    });
    for col in 0..GRID_W {
        for row in 0..GRID_H {
            commands.spawn((
                Mesh3d(dot_mesh.clone()),
                MeshMaterial3d(dot_mat.clone()),
                Transform::from_xyz(col as f32 * TILE, row as f32 * TILE, 0.001),
            ));
        }
    }

    // --- Rounded-ish border: four thin bars framing the grid ---
    let border_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.20, 0.20, 0.26),
        unlit: true,
        ..default()
    });
    let min_x = -0.25;
    let max_x = (GRID_W - 1) as f32 * TILE + 0.25;
    let min_y = -0.25;
    let max_y = (GRID_H - 1) as f32 * TILE + 0.25;
    let span_x = max_x - min_x;
    let span_y = max_y - min_y;
    let bar = 0.035;
    let z = 0.004;
    for (x, y, w, h) in [
        (cx, min_y, span_x + bar, bar), // bottom
        (cx, max_y, span_x + bar, bar), // top
        (min_x, cy, bar, span_y + bar), // left
        (max_x, cy, bar, span_y + bar), // right
    ] {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(w, h, 0.012))),
            MeshMaterial3d(border_mat.clone()),
            Transform::from_xyz(x, y, z),
        ));
    }

    // --- The Guildmaster at board centre, one tile wide, facing the camera ---
    // (UNITS_AND_SCALE §5: rotate +90° about X so the disc faces +Z.)
    commands.spawn((
        guildmaster(&assets),
        Transform::from_xyz(cx, cy, 0.02)
            .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
    ));
}
