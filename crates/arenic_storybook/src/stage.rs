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

use arenic_game::arena::Prim;
use arenic_game::boss::{BossShell, BossSpec, LightBehavior, boss_a};
use arenic_game::grid::{
    ARENA_H, ARENA_W, GRID_H, GRID_W, GridPlugin, TILE, TileMover, board_center, tile_to_world,
};
use arenic_game::guildmaster::guildmaster;
use arenic_game::orbit::{OrbitCamera, OrbitUnlocked};
use arenic_game::theme::{ActiveTheme, Theme, Tint};
use bevy::asset::RenderAssetUsages;
use bevy::camera::RenderTarget;
use bevy::color::Alpha;
use bevy::light::ShadowFilteringMethod;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::render::view::Hdr;

use crate::abilities::Player;
use crate::arena::spec;
use crate::hollow::{HollowLight, spawn_hollow_boss};
use crate::layers::{Layer, OnLayer};
use crate::stories::StoryId;
use crate::storybook::CurrentStory;

/// A fixed near-black for every hollow shell — the doc's "preserve dark exterior
/// mass" rule. The glow (the inner core) carries the theme colour, not the shell.
fn shell_dark(_theme: &Theme) -> Color {
    Color::srgb(0.06, 0.06, 0.07)
}

/// Render-target resolution — 16:9, matching the doc's framebuffer.
const STAGE_W: u32 = 1024;
const STAGE_H: u32 = 576;

/// Builds the reusable 3D stage, swaps per-story content, and keeps both in sync
/// with the active theme. Board geometry (`arenic_game::grid`) is the shared
/// source of truth; [`GridPlugin`] steps any board puck a tile per arrow press.
pub struct StagePlugin;

impl Plugin for StagePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(GridPlugin)
            .add_systems(Startup, setup_stage)
            .add_systems(
                Update,
                (
                    // Re-populate the board whenever the selected story changes.
                    swap_content.run_if(resource_changed::<CurrentStory>),
                    // Discover async-loaded glTF materials — clones each into a private
                    // instance and paints it once, so the systems below need only run
                    // on a real theme switch (not every frame).
                    collect_content_materials,
                    retheme_stage.run_if(
                        resource_exists::<StageMaterials>.and(resource_changed::<ActiveTheme>),
                    ),
                    retheme_content.run_if(resource_changed::<ActiveTheme>),
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
#[component(immutable)]
pub struct StageContent {
    /// Resolves the content's base colour from the active theme, so it re-tones
    /// on every theme switch (e.g. `|t| t.brand()`).
    pub tint: Tint,
}

/// The collected `StandardMaterial` handles of a [`StageContent`] root, recorded
/// once its glTF scene has finished spawning. Lives on the content entity, so it
/// despawns with the content on a story swap — no stale global state.
#[derive(Component)]
#[component(immutable)]
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
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
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
    let board_w = ARENA_W + 0.7;
    let board_h = ARENA_H + 0.7;
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
        OnLayer(Layer::Floor),
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
                OnLayer(Layer::Floor),
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
            OnLayer(Layer::Floor),
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
    current: Res<CurrentStory>,
    existing: Query<Entity, Or<(With<StageContent>, With<HollowLight>)>>,
    mut cams: Query<(Entity, &mut OrbitCamera)>,
) {
    for entity in &existing {
        commands.entity(entity).despawn();
    }

    // Leaving the 3D stage (no story, or a non-3D one) relocks the camera. The
    // orbit-unlock toggle only exists on 3D stories, so a camera left unlocked
    // would otherwise strand canvas scrolling with no visible control to relock.
    if !current.0.is_some_and(StoryId::has_3d_stage) {
        for (entity, mut cam) in &mut cams {
            cam.reset();
            // The marker removal only applies at the command flush, so an
            // unordered `orbit_input` later this same frame can still drift the
            // camera — re-assert the home pose at the flush that relocks it.
            commands.entity(entity).remove::<OrbitUnlocked>().queue(
                |mut cam_entity: EntityWorldMut| {
                    if let Some(mut cam) = cam_entity.get_mut::<OrbitCamera>() {
                        cam.reset();
                    }
                },
            );
        }
    }

    let Some(story) = current.0 else {
        return;
    };
    let center = board_center();
    // The abilities test scene isn't an arena — it gets a player puck + a target boss.
    if story == StoryId::HolyNova {
        spawn_ability_scene(&mut commands, &mut meshes, &mut materials, &assets, center);
        return;
    }
    // The arena's whole identity (boss + props) comes from the one ArenaSpec table.
    let Some(arena) = spec(story) else {
        return;
    };
    spawn_boss(
        &mut commands,
        &mut meshes,
        &mut materials,
        &assets,
        center,
        &arena.boss,
    );
    // Ambient flora/fauna props — a toggle-ready scene layer (placeholders for
    // ARE-18…26). Re-toned and despawned like any content.
    for prop in &arena.props {
        spawn_prop(
            &mut commands,
            &mut meshes,
            &mut materials,
            center,
            prop.prim,
            prop.offset,
            prop.tint,
        );
    }
}

/// Spawns an arena's boss from its [`BossSpec`]: a hollow shell + animated core,
/// or the Guild House home pyramid (no core). The shell→glTF-loader mapping lives
/// here — the ONE place a new shell is wired; which shell each arena wears lives in
/// the [`crate::arena`] spec.
fn spawn_boss(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    assets: &AssetServer,
    center: Vec2,
    boss: &BossSpec,
) {
    // §5: rotate +90° about X so the authored (Y-up) token faces the camera.
    let face_camera = Quat::from_rotation_x(FRAC_PI_2);
    match *boss {
        BossSpec::Home { color } => {
            // The Guild House: a calm pyramid (reusing boss_a), the safe hearth.
            commands.spawn((
                boss_a(assets),
                StageContent { tint: color },
                OnLayer(Layer::Boss),
                Transform::from_xyz(center.x, center.y, 0.02).with_rotation(face_camera),
            ));
        }
        BossSpec::Hollow {
            shell,
            core,
            core_z,
            behavior,
            color,
        } => {
            let core_mesh = core.to_mesh();
            let rest = Transform::from_xyz(0.0, 0.0, core_z);
            // One call — the shell → glTF mapping now lives in `BossShell::scene`.
            spawn_hollow_boss(
                commands,
                meshes,
                materials,
                center,
                shell.scene(assets),
                shell_dark,
                core_mesh,
                rest,
                behavior,
                color,
            );
        }
    }
}

/// The `abilities/*` test scene: a neutral player puck at centre-front + a glowing
/// target boss set back from it. The ability itself fires from [`crate::abilities`]
/// on a key press; this just stages something to cast at.
fn spawn_ability_scene(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    assets: &AssetServer,
    center: Vec2,
) {
    // A target to fire at — a glowing torus (no class), set back from centre.
    spawn_hollow_boss(
        commands,
        meshes,
        materials,
        Vec2::new(center.x, center.y + 1.6),
        BossShell::TorusHalo.scene(assets),
        shell_dark,
        Cuboid::new(0.55, 0.55, 0.04).into(),
        Transform::from_xyz(0.0, 0.0, 0.06),
        LightBehavior::HaloFill,
        |t| t.warning,
    );

    // The controllable player — a guildmaster puck near board centre.
    spawn_guildmaster_puck(commands, assets, 32, 10);
}

/// Spawns a **guildmaster puck** — the flat `guildmaster.glb` disc — onto the board
/// at tile `(col, row)`. It's the controllable player: it casts abilities (tagged
/// [`crate::abilities::Player`]) and steps a tile per arrow-key press (tagged
/// [`TileMover`], driven by [`GridPlugin`]). Any story that wants a controllable
/// guildmaster calls this.
fn spawn_guildmaster_puck(commands: &mut Commands, assets: &AssetServer, col: i32, row: i32) {
    let p = tile_to_world(col, row);
    commands.spawn((
        // `guildmaster()` brings the disc + the `Guildmaster` marker; we add the
        // player/mover roles so any guildmaster puck is castable + controllable.
        guildmaster(assets),
        Player,
        TileMover::new(col, row),
        StageContent {
            tint: |t| t.text_1(),
        },
        // Lies flat facing the camera (§5); §5 +90°X for the authored Y-up disc.
        Transform::from_xyz(p.x, p.y, 0.02).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
    ));
}

/// Once a [`StageContent`] root's glTF scene has spawned and its materials have
/// loaded, **clones** each material into a private instance (re-pointing the mesh
/// at the clone) and paints it from the content's tint, then records the clone
/// handles on the root for later re-toning. Cloning is what keeps one story from
/// mutating the shared, cached glTF material another instance/story relies on.
///
/// The `Without<ContentMaterials>` filter retries each frame until the async load
/// fills in the hierarchy; we only commit once *every* discovered mesh material is
/// loaded (so a partially-loaded scene is retried, never half-recorded).
fn collect_content_materials(
    mut commands: Commands,
    active: Res<ActiveTheme>,
    roots: Query<(Entity, &StageContent), Without<ContentMaterials>>,
    children: Query<&Children>,
    mesh_mats: Query<&MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let theme = active.palette();
    for (root, content) in &roots {
        let color = (content.tint)(&theme);
        // Depth-first walk of the spawned scene hierarchy.
        let mut meshes_seen = 0usize;
        let mut owned = Vec::new();
        let mut stack = vec![root];
        while let Some(entity) = stack.pop() {
            if let Ok(mat) = mesh_mats.get(entity) {
                meshes_seen += 1;
                if let Some(src) = materials.get(&mat.0) {
                    let mut copy = src.clone();
                    copy.base_color = color; // paint once, up front
                    let handle = materials.add(copy);
                    commands
                        .entity(entity)
                        .insert(MeshMaterial3d(handle.clone()));
                    owned.push(handle);
                }
            }
            if let Ok(kids) = children.get(entity) {
                stack.extend(kids.iter());
            }
        }
        // Commit only when the whole scene's materials are present and cloned.
        if meshes_seen > 0 && owned.len() == meshes_seen {
            commands.entity(root).insert(ContentMaterials(owned));
        }
    }
}

/// Paints the **stage** from the active theme: clear colour, board, dots, border.
/// Idempotent — only writes a material/camera when its colour differs, so it
/// costs nothing between theme switches.
fn retheme_stage(
    active: Res<ActiveTheme>,
    stage: Res<StageMaterials>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut camera: Query<&mut Camera, With<StageCamera>>,
) {
    let theme = active.palette();

    // Translucent so the skybox + under-floor swarm read through the liquid glass.
    set_base_color(
        &mut materials,
        &stage.board,
        theme.surface_2().with_alpha(0.5),
    );
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
    tint: Tint,
) {
    let (mesh, z_rest, stand) = prim.to_mesh();
    let mesh = meshes.add(mesh);
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
        OnLayer(Layer::Props),
    ));
}

// `board_center` (and its midpoint test) lives in `arenic_game::grid`, shared
// with the game.
