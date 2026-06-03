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
use arenic_game::theme::ActiveTheme;
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

/// Builds the 3D stage and keeps it in sync with the active theme.
pub struct StagePlugin;

impl Plugin for StagePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_stage)
            .add_systems(Update, (collect_disc_materials, retheme_stage).chain());
    }
}

/// Handle to the texture the stage renders into.
#[derive(Resource)]
pub struct Stage3d {
    pub image: Handle<Image>,
}

/// The themeable materials in the stage. Colours are mutated by handle (every
/// mesh sharing a handle re-tones at once) from the active theme. The disc's
/// glTF materials aren't known until its scene loads, so they're filled in later
/// by [`collect_disc_materials`].
#[derive(Resource)]
struct StageMaterials {
    board: Handle<StandardMaterial>,
    dots: Handle<StandardMaterial>,
    border: Handle<StandardMaterial>,
    disc: Vec<Handle<StandardMaterial>>,
}

/// Marks the stage camera so the retheme system can set its clear colour.
#[derive(Component)]
struct StageCamera;

/// Marks the spawned Guildmaster so its glTF materials can be discovered.
#[derive(Component)]
struct DiscRoot;

/// Builds the render-to-texture arena once at startup.
fn setup_stage(
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
        StageCamera,
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
    let board_mat = materials.add(StandardMaterial {
        perceptual_roughness: 0.95,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(board_w, board_h, 0.02))),
        MeshMaterial3d(board_mat.clone()),
        Transform::from_xyz(cx, cy, -0.02),
    ));

    // --- Dot grid: a small disc at every tile centre (Circle faces +Z) ---
    let dot_mesh = meshes.add(Circle::new(0.012));
    let dot_mat = materials.add(StandardMaterial {
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
        DiscRoot,
        Transform::from_xyz(cx, cy, 0.02)
            .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
    ));

    // Colours are left at their defaults; `retheme_stage` paints them from the
    // active theme every frame (writing only when they actually change).
    commands.insert_resource(StageMaterials {
        board: board_mat,
        dots: dot_mat,
        border: border_mat,
        disc: Vec::new(),
    });
}

/// Once the Guildmaster's glTF scene has spawned, records its material handles
/// so the retheme system can tint the disc. Runs until it finds them.
fn collect_disc_materials(
    roots: Query<Entity, With<DiscRoot>>,
    children: Query<&Children>,
    mesh_mats: Query<&MeshMaterial3d<StandardMaterial>>,
    stage: Option<ResMut<StageMaterials>>,
) {
    let (Ok(root), Some(mut stage)) = (roots.single(), stage) else {
        return;
    };
    if !stage.disc.is_empty() {
        return; // already collected
    }
    // Depth-first walk of the spawned scene hierarchy.
    let mut found = Vec::new();
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        if let Ok(mat) = mesh_mats.get(entity) {
            found.push(mat.0.clone());
        }
        if let Ok(kids) = children.get(entity) {
            stack.extend(kids.iter());
        }
    }
    if !found.is_empty() {
        stage.disc = found;
    }
}

/// Paints the stage from the active theme: clear colour, board, dots, border and
/// disc. Idempotent — only writes a material/camera when its colour differs, so
/// it costs nothing between theme switches and self-applies once the disc loads.
fn retheme_stage(
    active: Res<ActiveTheme>,
    stage: Option<Res<StageMaterials>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut camera: Query<&mut Camera, With<StageCamera>>,
) {
    let Some(stage) = stage else {
        return;
    };
    let theme = active.palette();

    set_base_color(&mut materials, &stage.board, theme.surface_2());
    set_base_color(&mut materials, &stage.dots, theme.text_muted());
    set_base_color(&mut materials, &stage.border, theme.border_bold());
    for handle in &stage.disc {
        set_base_color(&mut materials, handle, theme.brand());
    }

    if let Ok(mut camera) = camera.single_mut() {
        let clear = ClearColorConfig::Custom(theme.surface_1());
        if !matches!(&camera.clear_color, ClearColorConfig::Custom(c) if *c == theme.surface_1()) {
            camera.clear_color = clear;
        }
    }
}

/// Sets a material's base colour only if it differs. The immutable `get` check
/// first means `get_mut` (which forces a GPU re-upload) runs only on real
/// changes — so this is free to call every frame.
fn set_base_color(
    materials: &mut Assets<StandardMaterial>,
    handle: &Handle<StandardMaterial>,
    color: Color,
) {
    if !materials.get(handle).is_some_and(|m| m.base_color != color) {
        return;
    }
    if let Some(material) = materials.get_mut(handle) {
        material.base_color = color;
    }
}
