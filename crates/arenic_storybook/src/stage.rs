//! A reusable top-down 3D "stage" rendered into a texture that any 3D story
//! displays inside the canvas.
//!
//! It reproduces the reference framing from `_docs/UNITS_AND_SCALE.md`: a flat
//! board on the XY plane (a `66 × 31` dot grid with a border) viewed by a
//! near-orthographic perspective camera parked far back on `+Z` (vertical FOV
//! `π/8`, eye at `z = 24`, looking straight down `−Z`), lit by one soft-shadow
//! light. Bevy draws 3D through a `Camera3d`, so the scene is rendered off-screen
//! into an image the story shows as an `ImageNode`.
//!
//! **Scaffolding vs. content (ARE-6).** Everything reusable — the render target,
//! the §3 camera (with the orbit-unlock control), the light, the ground /
//! shadow-catcher board, and the theme-reactive materials — is built once at
//! startup and shared by every 3D story. A story supplies only its *content*:
//! the object(s) to drop on the board. There is a **single** stage; its content
//! is swapped to match the selected story (so the stories never fight over one
//! render target), and game pieces stay in `arenic_game` with no scene coupling.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_8};

use arenic_game::boss::{
    boss_a, capsule_resonator, hex_prism, hollow_icosphere, hollow_obelisk, stepped_pyramid,
    torus_halo, triangular_prism, truncated_cone,
};
use arenic_game::orbit::OrbitCamera;
use arenic_game::theme::{ActiveTheme, Theme};
use bevy::asset::RenderAssetUsages;
use bevy::camera::RenderTarget;
use bevy::color::Alpha;
use bevy::post_process::bloom::Bloom;
use bevy::light::ShadowFilteringMethod;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::render::view::Hdr;

use crate::hollow::{HollowLight, LightBehavior, spawn_hollow_boss};
use crate::layers::{Layer, LayerTag};
use crate::stories::StoryId;

/// A fixed near-black for every hollow shell — the doc's "preserve dark exterior
/// mass" rule. The glow (the inner core) carries the theme colour, not the shell.
fn shell_dark(_theme: &Theme) -> Color {
    Color::srgb(0.06, 0.06, 0.07)
}

/// Render-target resolution — 16:9, matching the doc's framebuffer.
const STAGE_W: u32 = 1024;
const STAGE_H: u32 = 576;

// Arena dimensions, straight from UNITS_AND_SCALE §1.
const GRID_W: i32 = 66;
const GRID_H: i32 = 31;
const TILE: f32 = 0.25;

/// The board centre — the midpoint of the tile-centre extents (`0..16.25`,
/// `0..7.5`). The camera frames here and content is dropped here.
fn board_center() -> Vec2 {
    Vec2::new(
        (GRID_W - 1) as f32 * TILE * 0.5, // 8.125
        (GRID_H - 1) as f32 * TILE * 0.5, // 3.75
    )
}

/// Builds the reusable 3D stage, swaps per-story content, and keeps both in sync
/// with the active theme.
pub struct StagePlugin;

impl Plugin for StagePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_stage).add_systems(
            Update,
            (
                // Re-populate the board whenever the selected story changes.
                swap_content.run_if(resource_changed::<crate::storybook::CurrentStory>),
                // Discover async-loaded glTF materials, then re-tone everything.
                collect_content_materials,
                retheme_stage,
                retheme_content,
            ),
        );
    }
}

/// Handle to the texture the stage renders into. Stories show it via `ImageNode`.
#[derive(Resource)]
pub struct Stage3d {
    pub image: Handle<Image>,
}

/// The themeable materials owned by the **stage** (board, dots, border). Colours
/// are mutated by handle — every mesh sharing a handle re-tones at once — from
/// the active theme. Content materials (which load async with glTF) are handled
/// separately by [`StageContent`] + [`ContentMaterials`].
#[derive(Resource)]
struct StageMaterials {
    board: Handle<StandardMaterial>,
    dots: Handle<StandardMaterial>,
    border: Handle<StandardMaterial>,
}

/// Marks the stage camera so the retheme system can set its clear colour (and so
/// the foreground overlay can parent itself to it).
#[derive(Component)]
pub struct StageCamera;

/// Tags a story's content root dropped onto the stage. The stage (a) **despawns**
/// every `StageContent` when the story changes and (b) walks each root's
/// hierarchy — including its async-loaded glTF `StandardMaterial`s — and tints
/// them to follow the active theme, using `tint` to pick the token.
///
/// This is the generic generalisation of ARE-3's guildmaster-only `DiscRoot`:
/// any story can drop themed content with one tag.
#[derive(Component, Clone, Copy)]
pub struct StageContent {
    /// Resolves the content's base colour from the active theme, so it re-tones
    /// on every theme switch (e.g. `|t| t.brand()`).
    pub tint: fn(&Theme) -> Color,
}

/// The collected `StandardMaterial` handles of a [`StageContent`] root, recorded
/// once its glTF scene has finished spawning. Lives on the content entity, so it
/// despawns with the content on a story swap — no stale global state.
#[derive(Component)]
struct ContentMaterials(Vec<Handle<StandardMaterial>>);

/// Builds the reusable render-to-texture stage once at startup: the target
/// image, the §3 camera, one soft-shadow light, and the ground/board it all
/// frames. Content is added later by [`swap_content`].
fn setup_stage(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Vec2 { x: cx, y: cy } = board_center();

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
        // HDR + bloom so the hollow bosses' emissive inner cores glow (ARE-8…16).
        Hdr,
        Bloom::NATURAL,
        AmbientLight {
            brightness: 600.0,
            ..default()
        },
        StageCamera,
    ));

    // --- One light, angled so content drops a soft shadow on the board ---
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
        // "Liquid glass": a translucent (alpha-blended) sheet — the skybox AND the
        // under-floor swarm show through it, while it stays a LIT surface so it
        // still RECEIVES the bosses' contact shadows. A glossy `perceptual_roughness`
        // gives the wet glass sheen. (Screen-space specular transmission is
        // unreliable on this HDR render-to-texture camera, so we use blend.)
        perceptual_roughness: 0.4,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    commands.spawn((
        // A flat plane (not a box) so it reads as a single sheet of glass.
        Mesh3d(meshes.add(Rectangle::new(board_w, board_h))),
        MeshMaterial3d(board_mat.clone()),
        Transform::from_xyz(cx, cy, -0.02),
        LayerTag(Layer::Floor),
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
                LayerTag(Layer::Floor),
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
            LayerTag(Layer::Floor),
        ));
    }

    // Colours are left at their defaults; `retheme_stage` paints them from the
    // active theme every frame (writing only when they actually change).
    commands.insert_resource(StageMaterials {
        board: board_mat,
        dots: dot_mat,
        border: border_mat,
    });
}

/// Re-populates the stage with the selected story's content. Despawns whatever
/// was there, then spawns the new story's piece (from `arenic_game`) at the board
/// centre, tagged [`StageContent`] so it themes and can be swapped out next time.
///
/// This is the **single content-injection point**: a new 3D story is one match
/// arm here plus its tree leaf — no camera/light/shadow/RTT/orbit/theming is ever
/// duplicated. Non-3D stories simply leave the board empty.
fn swap_content(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    current: Res<crate::storybook::CurrentStory>,
    existing: Query<Entity, Or<(With<StageContent>, With<HollowLight>)>>,
    mut cams: Query<&mut OrbitCamera>,
) {
    for entity in &existing {
        commands.entity(entity).despawn();
    }

    // Leaving the 3D stage (no story, or a non-3D one) relocks the camera. The
    // orbit-unlock toggle only exists on 3D stories, so a camera left unlocked
    // would otherwise strand canvas scrolling with no visible control to relock.
    if !current.0.is_some_and(StoryId::has_3d_stage) {
        for mut cam in &mut cams {
            cam.unlocked = false;
            cam.reset();
        }
    }

    let Some(story) = current.0 else {
        return;
    };
    let center = board_center();
    // UNITS_AND_SCALE §5: rotate +90° about X so the authored (Y-up) token faces
    // the camera — the disc lies flat, the pyramid's apex points at the camera.
    let face_camera = Quat::from_rotation_x(FRAC_PI_2);

    // A hollow boss: dark shell (`arenic_game`) + emissive inner core (animated).
    // `hb!(shell, shell-tint, core-mesh, rest-z, behavior, light-colour)`.
    macro_rules! hb {
        ($shell:expr, $core:expr, $z:expr, $behavior:expr, $color:expr $(,)?) => {{
            spawn_hollow_boss(
                &mut commands,
                &mut meshes,
                &mut materials,
                center,
                $shell,
                shell_dark,
                $core,
                Transform::from_xyz(0.0, 0.0, $z),
                $behavior,
                $color,
            );
        }};
    }

    match story {
        StoryId::Guildmaster => {
            // The Guild House home: a calm pyramid (reusing boss_a) on the warm
            // Coffee theme — the guild's safe hearth, not a true boss.
            commands.spawn((
                boss_a(&assets),
                StageContent {
                    tint: |t: &Theme| t.brand(),
                },
                LayerTag(Layer::Boss),
                Transform::from_xyz(center.x, center.y, 0.02).with_rotation(face_camera),
            ));
        }
        // --- The eight hollow-light arena bosses (ARE-8…ARE-15) ---
        StoryId::Hunter => hb!(
            hollow_obelisk(&assets),
            Cuboid::new(0.06, 0.06, 1.6).into(),
            1.0,
            LightBehavior::ObeliskBlade,
            |t: &Theme| t.brand(),
        ),
        StoryId::Alchemist => hb!(
            truncated_cone(&assets),
            Cuboid::new(0.5, 0.5, 0.06).into(),
            0.12,
            LightBehavior::CauldronRise,
            |t: &Theme| t.success,
        ),
        StoryId::Cardinal => hb!(
            torus_halo(&assets),
            Cuboid::new(0.55, 0.55, 0.04).into(),
            0.06,
            LightBehavior::HaloFill,
            |t: &Theme| t.warning,
        ),
        StoryId::Warrior => hb!(
            hex_prism(&assets),
            Sphere::new(0.2).into(),
            0.22,
            LightBehavior::PrismBlock,
            |t: &Theme| t.error,
        ),
        StoryId::Thief => hb!(
            triangular_prism(&assets),
            Cuboid::new(0.5, 0.12, 0.12).into(),
            0.16,
            LightBehavior::WedgeBeam,
            |t: &Theme| t.secondary,
        ),
        StoryId::Bard => hb!(
            capsule_resonator(&assets),
            Cuboid::new(0.7, 0.05, 0.05).into(),
            0.16,
            LightBehavior::CapsulePulse,
            |t: &Theme| t.accent,
        ),
        StoryId::Forager => hb!(
            stepped_pyramid(&assets),
            Cuboid::new(0.1, 0.1, 0.9).into(),
            0.5,
            LightBehavior::ZigguratGrow,
            |t: &Theme| t.success,
        ),
        StoryId::Merchant => hb!(
            hollow_icosphere(&assets),
            Sphere::new(0.22).into(),
            0.3,
            LightBehavior::GeodeShimmer,
            |t: &Theme| t.warning,
        ),
        _ => {}
    }

    // Ambient flora/fauna props for arena stories — a toggle-ready scene layer
    // (placeholders; real meshes later). Re-toned and despawned like any content.
    spawn_arena_props(&mut commands, &mut meshes, &mut materials, center, story);
}

/// Once a [`StageContent`] root's glTF scene has spawned, records its material
/// handles (as a component on the root) so the retheme system can tint it. The
/// `Without<ContentMaterials>` filter makes this run once per content instance,
/// retrying each frame until the async scene load fills in the hierarchy.
fn collect_content_materials(
    mut commands: Commands,
    roots: Query<Entity, (With<StageContent>, Without<ContentMaterials>)>,
    children: Query<&Children>,
    mesh_mats: Query<&MeshMaterial3d<StandardMaterial>>,
) {
    for root in &roots {
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
            commands.entity(root).insert(ContentMaterials(found));
        }
    }
}

/// Paints the **stage** from the active theme: clear colour, board, dots, border.
/// Idempotent — only writes a material/camera when its colour differs, so it
/// costs nothing between theme switches.
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

    // Translucent so the skybox + under-floor swarm read through the liquid glass.
    set_base_color(&mut materials, &stage.board, theme.surface_2().with_alpha(0.5));
    set_base_color(&mut materials, &stage.dots, theme.text_muted());
    set_base_color(&mut materials, &stage.border, theme.border_bold());

    if let Ok(mut camera) = camera.single_mut()
        && !matches!(&camera.clear_color, ClearColorConfig::Custom(c) if *c == theme.surface_1())
    {
        camera.clear_color = ClearColorConfig::Custom(theme.surface_1());
    }
}

/// Paints each piece of **content** from the active theme, using its own `tint`
/// token. Self-applies once a content root's glTF materials have been collected.
fn retheme_content(
    active: Res<ActiveTheme>,
    content: Query<(&StageContent, &ContentMaterials)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let theme = active.palette();
    for (content, mats) in &content {
        let color = (content.tint)(&theme);
        for handle in &mats.0 {
            set_base_color(&mut materials, handle, color);
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

/// Marks an ambient flora/fauna prop on the stage — a toggle-ready scene layer.
/// Props are tinted on-theme via [`StageContent`] and despawned on story swap
/// like any content. Low-poly placeholders; real Blender meshes replace them later.
#[derive(Component)]
pub struct FloraFauna;

/// A low-poly placeholder primitive for one flora/fauna prop.
#[derive(Clone, Copy)]
enum Prim {
    Cuboid(f32, f32, f32),
    Sphere(f32),
    Cylinder(f32, f32),
    Cone(f32, f32),
    Capsule(f32, f32),
}

/// Spawns one ambient prop resting on the board at `center + offset`, tinted
/// on-theme. Upright forms (cylinder/cone/capsule) are stood up along `+Z`; the
/// cuboid's `z` is its height; a sphere rests on its radius.
fn spawn_prop(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    center: Vec2,
    prim: Prim,
    offset: Vec2,
    tint: fn(&Theme) -> Color,
) {
    let (mesh, z_rest, stand) = match prim {
        Prim::Cuboid(x, y, h) => (meshes.add(Cuboid::new(x, y, h)), h * 0.5, false),
        Prim::Sphere(r) => (meshes.add(Sphere::new(r)), r, false),
        Prim::Cylinder(r, h) => (meshes.add(Cylinder::new(r, h)), h * 0.5, true),
        Prim::Cone(r, h) => (
            meshes.add(Cone {
                radius: r,
                height: h,
            }),
            h * 0.5,
            true,
        ),
        Prim::Capsule(r, h) => (meshes.add(Capsule3d::new(r, h)), h * 0.5 + r, true),
    };
    let mut transform = Transform::from_xyz(center.x + offset.x, center.y + offset.y, z_rest);
    if stand {
        transform.rotation = Quat::from_rotation_x(FRAC_PI_2);
    }
    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(materials.add(StandardMaterial {
            perceptual_roughness: 0.8,
            ..default()
        })),
        transform,
        StageContent { tint },
        FloraFauna,
        LayerTag(Layer::Props),
    ));
}

/// Spawns an arena story's three on-theme flora/fauna props near the walls
/// (placeholders for ARE-18…26). A no-op for non-arena stories.
fn spawn_arena_props(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    center: Vec2,
    story: StoryId,
) {
    use Prim::{Capsule, Cone, Cuboid, Cylinder, Sphere};
    type Tint = fn(&Theme) -> Color;
    // (primitive, dx, dy from board centre, on-theme tint token)
    let props: &[(Prim, f32, f32, Tint)] = match story {
        StoryId::Hunter => &[
            (Cuboid(0.55, 0.22, 0.4), -6.6, -2.4, |t: &Theme| t.surface_3()),
            (Cylinder(0.07, 0.55), 6.8, -2.7, |t: &Theme| t.primary),
            (Capsule(0.13, 0.3), 6.2, 2.9, |t: &Theme| t.text_muted()),
        ],
        StoryId::Alchemist => &[
            (Cylinder(0.3, 0.06), -6.8, 2.1, |t: &Theme| t.primary),
            (Sphere(0.16), 6.2, -2.7, |t: &Theme| t.surface_3()),
            (Cylinder(0.05, 0.5), 6.6, 2.9, |t: &Theme| t.secondary),
        ],
        StoryId::Cardinal => &[
            (Cone(0.22, 0.5), 6.8, -2.4, |t: &Theme| t.primary),
            (Cylinder(0.06, 0.55), -6.6, 2.6, |t: &Theme| t.surface_3()),
            (Sphere(0.16), -5.9, -2.8, |t: &Theme| t.warning),
        ],
        StoryId::Warrior => &[
            (Cone(0.16, 0.55), 6.8, 2.4, |t: &Theme| t.surface_3()),
            (Sphere(0.16), -6.2, -2.9, |t: &Theme| t.primary),
            (Cylinder(0.3, 0.06), -6.6, 2.7, |t: &Theme| t.warning),
        ],
        StoryId::Thief => &[
            (Sphere(0.28), 6.4, 2.9, |t: &Theme| t.primary),
            (Cone(0.16, 0.3), -6.7, -2.6, |t: &Theme| t.surface_3()),
            (Capsule(0.05, 0.5), -6.9, 2.1, |t: &Theme| t.secondary),
        ],
        StoryId::Bard => &[
            (Cuboid(0.42, 0.28, 0.06), -6.2, 2.9, |t: &Theme| t.primary),
            (Cylinder(0.26, 0.12), 6.6, -2.6, |t: &Theme| t.secondary),
            (Capsule(0.09, 0.3), -6.8, -2.2, |t: &Theme| t.surface_3()),
        ],
        StoryId::Forager => &[
            (Sphere(0.28), -6.6, 3.0, |t: &Theme| t.surface_3()),
            (Sphere(0.17), 6.8, -2.6, |t: &Theme| t.warning),
            (Cone(0.1, 0.5), -5.8, -2.8, |t: &Theme| t.accent),
        ],
        StoryId::Merchant => &[
            (Sphere(0.28), 6.6, -2.4, |t: &Theme| t.warning),
            (Cylinder(0.11, 0.55), -6.4, 2.9, |t: &Theme| t.surface_3()),
            (Cuboid(0.22, 0.22, 0.22), -0.4, -3.1, |t: &Theme| t.accent),
        ],
        StoryId::Guildmaster => &[
            // The home's warm hearth, a barrel, and a lantern post (safe + cosy).
            (Cuboid(0.45, 0.3, 0.32), -6.6, -2.5, |t: &Theme| t.primary),
            (Cylinder(0.16, 0.3), 6.6, -2.6, |t: &Theme| t.surface_3()),
            (Capsule(0.05, 0.4), 6.4, 2.7, |t: &Theme| t.warning),
        ],
        _ => &[],
    };
    for &(prim, dx, dy, tint) in props {
        spawn_prop(commands, meshes, materials, center, prim, Vec2::new(dx, dy), tint);
    }
}
