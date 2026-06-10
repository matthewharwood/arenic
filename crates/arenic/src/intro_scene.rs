//! The game's **intro / overworld scene** — the 3D world the player first sees.
//!
//! Builds the full `3 × 3` arena grid (`_docs/UNITS_AND_SCALE.md`): nine themed
//! arenas, each a clean cell (a **skybox** cloud backdrop, a continuous liquid-glass
//! **floor**, and an illuminated **dot** per tile you can address + restate by
//! `(arena, col, row)`; no boss / flora / fauna). It starts **zoomed out** on
//! the overworld — the perspective camera frames all nine arenas (centred on arena 4
//! at `z = 72`), so the grid fills the 1280×720 window per the units doc — with
//! column **dividers** + a border on the current arena drawn as gizmos.
//!
//! **Controls:** `P` toggles overworld ↔ single-arena zoom (`z = 24`); `[` / `]`
//! select the previous / next arena (cyclic) — the camera follows when zoomed in,
//! but stays centred on the overworld when zoomed out (only the selection border
//! moves). The Guild House has **two guildmaster pucks**: `Tab` selects between them
//! (a glowing ring marks the selected one), arrow keys step the selected puck one
//! tile/press — stepping past an arena edge **edge-walks** into the adjacent arena
//! — `1` casts Holy Nova from it, `R` records its staff (see `crate::recording` +
//! RULEBOOK → Record & Replay), `L` lavas its tile (a demo of the
//! [`arenic_game::tile::TileBoard`] API). `Esc` → title.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_8};

use arenic_game::Boss;
use arenic_game::ability::{AbilityMeshes, AbilityPlugin, AbilitySfx, cast_holy_nova};
use arenic_game::arena::{Arena, PropSpec};
use arenic_game::atmosphere::{AtmospherePlugin, CloudFog, Plane, cloud_material};
use arenic_game::boss::{BossSpec, LightBehavior, boss_a, light_offset};
use arenic_game::grid::{
    ARENA_H, ARENA_W, ARENAS, GRID_H, GRID_W, TILE, TileMover, arena_offset, board_center,
    tile_to_world,
};
use arenic_game::guildmaster::guildmaster;
use arenic_game::swarm::{
    SWARM_TIME_SCALE, SwarmMember, SwarmSpec, mote_mesh, swarm_amp, swarm_home, swarm_material,
    swarm_offset,
};
use arenic_game::theme::Theme;
use arenic_game::tile::{ArenaTiles, Tile, TileBoard, TileKind, build_tile_materials};
use arenic_game::timeline::{
    Action, ArenaClock, ArenaTimeline, Ghost, RecordingLibrary, TimelineEvent,
};
use bevy::color::Alpha;
use bevy::input::common_conditions::input_just_pressed;
use bevy::light::{NotShadowCaster, NotShadowReceiver, ShadowFilteringMethod};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::view::Hdr;

use crate::hud::{BOTTOM_BAR_PX, TOP_BAR_PX};
use crate::modal::no_modal;
use crate::recording::{
    DraftTimeline, RecordingState, is_idle, no_pending_walk, not_counting_down,
};
use crate::states::{AppState, not_tile_editing};

/// The Guild House — arena index 1 (top row, middle column); home of the puck.
const GUILD_HOUSE: usize = 1;
/// Arena 4 (centre of the 3×3 grid) — the overworld zoom-out re-centres here.
const OVERWORLD: usize = 4;
/// The tile every combat boss rests on until a score moves it — the cell
/// nearest the arena centre.
const BOSS_COL: i32 = 33;
const BOSS_ROW: i32 = 15;
/// (zoomed-in z, zoomed-out z) camera distances — straight from the reference repo
/// (`ZOOM = (24, 72)`). Matching them keeps the HUD's local-space framing aligned
/// with the game's global gamespace; the HUD's side/top/bottom chrome masks the
/// arena edges, so neighbouring arenas read as framed rather than bleeding in.
const ZOOM_IN: f32 = 24.0;
const ZOOM_OUT: f32 = 72.0;

// Per-arena cell layers, in LOCAL space (camera on +Z). The skybox + floor are
// sized to the arena footprint so the nine cells tile edge-to-edge.
const SKY_Z: f32 = -0.2;
/// Tile marker: a small **unlit** dot at each cell centre (no grid lines — the
/// continuous glass floor fills the cell). `0.012` radius matches the storybook's
/// subtle grid; unlit so it doesn't bloom. Sat just above the floor.
const TILE_Z: f32 = 0.02;
const DOT_R: f32 = 0.012;
/// Foreground haze plane Z — in front of the board content (toward the camera).
const FG_Z: f32 = 2.5;
/// HDR emissive scale for a boss core at full intensity — high enough to bloom.
const BOSS_EMISSIVE: f32 = 4.0;

/// Which arena is selected (the camera zoom-in target + the gizmo border). Starts
/// on the Guild House. `pub(crate)` so the HUD can read the focused arena.
#[derive(Resource)]
pub(crate) struct CurrentArena(pub(crate) usize);

/// On the camera while it frames the whole overworld (gates the overworld gizmos).
/// `pub(crate)` so the HUD knows whether to theme to an arena or the overworld.
#[derive(Component)]
pub(crate) struct ZoomOut;

/// A guildmaster puck's index in the Guild House — `Tab` cycles selection by it.
/// `pub(crate)` so the author feature can hand `Selected` back to puck 0 when a
/// boss is released.
#[derive(Component)]
#[component(immutable)]
pub(crate) struct Puck(pub(crate) usize);

/// Marks the currently-selected puck: it moves with the arrows, casts abilities, and
/// wears the selection ring. Exactly one puck has this at a time — recording
/// systems query it dynamically (RULEBOOK → Selected Character Query Pattern).
#[derive(Component)]
pub(crate) struct Selected;

/// The glowing selection halo; [`follow_selected`] parks it on the selected puck.
#[derive(Component)]
struct SelectionRing;

/// A boss's glowing inner core: its signature [`LightBehavior`], rest transform, and
/// the (baked) arena colour it glows. Set once at spawn (immutable); driven by
/// [`animate_boss_cores`].
#[derive(Component)]
#[component(immutable)]
struct BossCore {
    behavior: LightBehavior,
    rest: Transform,
    color: Color,
}

pub struct IntroScenePlugin;

impl Plugin for IntroScenePlugin {
    fn build(&self, app: &mut App) {
        // `AbilityPlugin` also preloads the shared `AbilitySfx` at startup.
        app.add_plugins(AtmospherePlugin)
            .add_plugins(AbilityPlugin)
            .insert_resource(CurrentArena(GUILD_HOUSE))
            .add_systems(OnEnter(AppState::Intro), setup_intro)
            .add_systems(
                Update,
                (
                    // World input gates off while a modal is open, a recording
                    // countdown holds the arena (RULEBOOK → Modal Controls), a
                    // confirmed edge-walk is one frame from landing, or the
                    // author tile editor owns the keys.
                    fire_holy_nova.run_if(
                        input_just_pressed(KeyCode::Digit1)
                            .or(input_just_pressed(KeyCode::Numpad1))
                            .and(no_modal)
                            .and(not_counting_down)
                            .and(no_pending_walk)
                            .and(not_tile_editing),
                    ),
                    toggle_zoom.run_if(input_just_pressed(KeyCode::KeyP)),
                    // Pagination yields to the tile editor too — its cursor and
                    // scrub target are pinned to the arena it opened on.
                    paginate.run_if(
                        input_just_pressed(KeyCode::BracketLeft)
                            .or(input_just_pressed(KeyCode::BracketRight))
                            .and(not_tile_editing),
                    ),
                    refocus_camera.run_if(resource_changed::<CurrentArena>),
                    // L mutates the board, which restart() can't rewind — idle only,
                    // so a committed staff never replays against unrecorded state.
                    kindle.run_if(
                        input_just_pressed(KeyCode::KeyL)
                            .and(no_modal)
                            .and(is_idle)
                            .and(no_pending_walk)
                            .and(not_tile_editing),
                    ),
                    // Tab is ignored while recording — the staff belongs to whoever
                    // started it (RULEBOOK → Recording Interruptions) — and while a
                    // pending walk could otherwise retarget Selected mid-handoff.
                    cycle_selection.run_if(
                        input_just_pressed(KeyCode::Tab)
                            .and(no_modal)
                            .and(is_idle)
                            .and(no_pending_walk)
                            .and(not_tile_editing),
                    ),
                    // Arrow movement + edge-walking live in `crate::travel`.
                    follow_selected,
                    animate_swarm,
                    animate_boss_cores,
                    draw_overworld_gizmos.run_if(any_with_component::<ZoomOut>),
                    back_to_title.run_if(
                        input_just_pressed(KeyCode::Escape)
                            .and(no_modal)
                            .and(is_idle),
                    ),
                )
                    .run_if(in_state(AppState::Intro)),
            );
    }
}

/// World-Y nudge PER UNIT of camera-z that lifts the arena into the band BETWEEN the
/// HUD bars. The bars (top `35`, bottom `95`) leave the view `(95−35)/2 = 30px` low;
/// the screen shift is constant at any zoom, so the WORLD shift scales with z:
/// `NDC × tan(fov/2)` where `NDC = (bottom−top)/window_h` and the vertical FOV is
/// `π/8` (so `tan(π/16) ≈ 0.198_912`).
const UI_VOFFSET_PER_Z: f32 = ((BOTTOM_BAR_PX - TOP_BAR_PX) / 720.0) * 0.198_912_4;

/// The camera pose framing arena `index` at distance `zoom`: parked straight back on
/// +Z over the arena's centre, looking down −Z, lifted so the arena centres in the
/// visible band between the HUD's top + bottom bars.
fn camera_pose(index: usize, zoom: f32) -> Transform {
    let mut c = arena_offset(index) + board_center();
    c.y -= zoom * UI_VOFFSET_PER_Z;
    Transform::from_xyz(c.x, c.y, zoom).looking_at(Vec3::new(c.x, c.y, 0.0), Vec3::Y)
}

/// Builds the overworld: the 9-arena grid, the guildmaster puck, the camera
/// (starting zoomed out), and the light.
fn setup_intro(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut clouds: ResMut<Assets<CloudFog>>,
) {
    let center = board_center();

    // --- Camera: start framing the whole overworld (zoomed out, centred on arena 4) ---
    commands.spawn((
        DespawnOnExit(AppState::Intro),
        ZoomOut,
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: FRAC_PI_8, // 22.5° vertical FOV
            near: 0.05,
            far: 150.0,
            ..default()
        }),
        Camera {
            // The only camera in the Intro state — it renders the 3D scene AND the
            // HUD UI (no separate UI camera, so nothing fights over the framebuffer).
            clear_color: ClearColorConfig::Custom(Color::srgb(0.02, 0.02, 0.03)),
            ..default()
        },
        Hdr,
        Bloom::NATURAL,
        ShadowFilteringMethod::Gaussian,
        AmbientLight {
            brightness: 600.0,
            ..default()
        },
        camera_pose(OVERWORLD, ZOOM_OUT),
    ));

    // --- One directional light (global) so floors catch a soft shadow ---
    commands.spawn((
        DespawnOnExit(AppState::Intro),
        DirectionalLight {
            illuminance: 6000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(3.0, 3.0, 8.0).looking_at(Vec3::ZERO, Vec3::Z),
    ));

    // --- The 3×3 battleground: one Arena per index, children hold LOCAL transforms ---
    let battleground = commands
        .spawn((
            DespawnOnExit(AppState::Intro),
            Transform::default(),
            Visibility::default(),
        ))
        .id();

    // Shared illuminated dot materials (themed per arena) + lava, plus the dot mesh +
    // the O(1) (arena, col, row) → entity lookup, all built as the grid spawns.
    let tints = Arena::ALL.map(|a| a.theme().palette().text_muted());
    let tile_materials = build_tile_materials(&mut materials, tints);
    let mut tile_lookup = ArenaTiles::default();
    let tile_mesh = meshes.add(Circle::new(DOT_R));

    for (index, &arena_id) in Arena::ALL.iter().enumerate() {
        let spec = arena_id.spec();
        let theme = spec.theme.palette();
        let offset = arena_offset(index);
        let arena = commands
            .spawn((
                // The arena root carries its identity, its 2-minute clock, and
                // the master timeline its ghosts replay (RULEBOOK → sheet music).
                arena_id,
                ArenaClock::default(),
                ArenaTimeline::default(),
                Transform::from_xyz(offset.x, offset.y, 0.0),
                Visibility::default(),
                ChildOf(battleground),
            ))
            .id();

        // Skybox — the themed cloud backdrop, sized to the cell so cells tile cleanly.
        commands.spawn((
            Mesh3d(meshes.add(Rectangle::new(ARENA_W, ARENA_H))),
            MeshMaterial3d(clouds.add(cloud_material(spec.sky, &theme, Plane::Skybox))),
            Transform::from_xyz(center.x, center.y, SKY_Z),
            NotShadowCaster,
            NotShadowReceiver,
            ChildOf(arena),
        ));

        // Foreground — a themed near haze in front of the board (the 6th layer).
        commands.spawn((
            Mesh3d(meshes.add(Rectangle::new(ARENA_W, ARENA_H))),
            MeshMaterial3d(clouds.add(cloud_material(spec.fg, &theme, Plane::Foreground))),
            Transform::from_xyz(center.x, center.y, FG_Z),
            NotShadowCaster,
            NotShadowReceiver,
            ChildOf(arena),
        ));

        // Floor — ONE continuous liquid-glass plane per arena (no seams, no grid
        // lines), translucent so the skybox reads through it.
        commands.spawn((
            Mesh3d(meshes.add(Rectangle::new(ARENA_W, ARENA_H))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: theme.surface_2().with_alpha(0.5),
                perceptual_roughness: 0.4,
                alpha_mode: AlphaMode::Blend,
                ..default()
            })),
            Transform::from_xyz(center.x, center.y, 0.0),
            ChildOf(arena),
        ));

        // Tile dots — a small illuminated dot at EVERY cell centre (66×31). Each shares
        // its arena's `Normal` glow material (instanced + cheap) and is recorded in the
        // lookup so `TileBoard` can flip it (e.g. to lava) by (arena, col, row).
        for col in 0..GRID_W {
            for row in 0..GRID_H {
                let p = tile_to_world(col, row);
                let tile = commands
                    .spawn((
                        Mesh3d(tile_mesh.clone()),
                        MeshMaterial3d(tile_materials.of(index, TileKind::Normal)),
                        Tile {
                            arena: index as u8,
                            col: col as u8,
                            row: row as u8,
                            kind: TileKind::Normal,
                        },
                        Transform::from_xyz(p.x, p.y, TILE_Z),
                        NotShadowCaster,
                        NotShadowReceiver,
                        ChildOf(arena),
                    ))
                    .id();
                tile_lookup.insert(index, col as usize, row as usize, tile);
            }
        }

        // Swarm (fauna), boss, and props — the per-arena content, baked to this
        // arena's theme (the game shows nine themes at once, so no live re-toning).
        spawn_swarm(
            &mut commands,
            &mut meshes,
            &mut materials,
            arena,
            center,
            spec.swarm,
            theme.primary,
        );
        spawn_boss(
            &mut commands,
            &assets,
            &mut meshes,
            &mut materials,
            arena,
            center,
            spec.boss,
            &theme,
        );
        spawn_props(
            &mut commands,
            &mut meshes,
            &mut materials,
            arena,
            center,
            spec.props,
            &theme,
        );

        // The Guild House holds TWO guildmaster pucks (children of the arena, so they
        // move in local tile space, §4.1). `Tab` selects between them; the first starts
        // selected. A glowing ring (below) marks the selection.
        if index == GUILD_HOUSE {
            for (i, (col, row)) in [(30, 15), (36, 15)].into_iter().enumerate() {
                let p = tile_to_world(col, row);
                let mut puck = commands.spawn((
                    guildmaster(&assets),
                    Puck(i),
                    TileMover::new(col, row),
                    RecordingLibrary::default(),
                    Transform::from_xyz(p.x, p.y, 0.05)
                        .with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
                    ChildOf(arena),
                ));
                if i == 0 {
                    puck.insert(Selected);
                }
            }
            // The selection halo — a glowing ring `follow_selected` parks on the
            // selected puck each frame (starts on puck 0).
            let start = tile_to_world(30, 15);
            commands.spawn((
                SelectionRing,
                Mesh3d(meshes.add(Annulus::new(0.24, 0.30))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::WHITE,
                    emissive: LinearRgba::rgb(1.0, 1.0, 1.0) * 2.5,
                    ..default()
                })),
                Transform::from_xyz(start.x, start.y, 0.03),
                NotShadowCaster,
                NotShadowReceiver,
                ChildOf(arena),
            ));
        }
    }

    // Hand the materials + lookup to the world so `TileBoard` can flip tile states.
    commands.insert_resource(tile_materials);
    commands.insert_resource(tile_lookup);
}

/// Spawns an arena's sky-swarm — `count` motes in a low ring under the floor, all
/// sharing one baked-colour material; driven by [`animate_swarm`].
fn spawn_swarm(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    arena: Entity,
    center: Vec2,
    swarm: SwarmSpec,
    color: Color,
) {
    let mesh = meshes.add(mote_mesh(swarm.mote, swarm.scale));
    let mat = materials.add(swarm_material(color));
    for i in 0..swarm.count {
        let home = swarm_home(i, swarm.count, center);
        commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(mat.clone()),
            Transform::from_translation(home),
            SwarmMember {
                motion: swarm.motion,
                phase: i as f32 * 0.7,
                home,
                amp: swarm_amp(swarm.motion, i),
            },
            NotShadowCaster,
            NotShadowReceiver,
            ChildOf(arena),
        ));
    }
}

/// Spawns an arena's boss: the Guild House home pyramid, or — for combat arenas
/// — a **movable boss root** (a [`Boss`] + `TileMover`, recorded and replayed
/// exactly like a hero) wearing the dark hollow shell + emissive core (driven
/// by [`animate_boss_cores`]) as children. §5: rotate the shell +90° about X so
/// the authored (Y-up) glTF faces the camera; the root itself stays unrotated
/// so the core's light offsets keep their board-space axes.
fn spawn_boss(
    commands: &mut Commands,
    assets: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    arena: Entity,
    center: Vec2,
    boss: BossSpec,
    theme: &Theme,
) {
    let face_camera = Quat::from_rotation_x(FRAC_PI_2);
    match boss {
        BossSpec::Home { .. } => {
            let at_center =
                Transform::from_xyz(center.x, center.y, 0.02).with_rotation(face_camera);
            commands.spawn((boss_a(assets), at_center, ChildOf(arena)));
        }
        BossSpec::Hollow {
            shell,
            core,
            core_z,
            behavior,
            color,
        } => {
            let baked = color(theme);
            let l = baked.to_linear();
            // The core's rest pose is LOCAL to the boss root now.
            let rest = Transform::from_xyz(0.0, 0.0, core_z);
            let start = tile_to_world(BOSS_COL, BOSS_ROW);
            commands.spawn((
                Boss,
                TileMover::new(BOSS_COL, BOSS_ROW),
                RecordingLibrary::default(),
                Transform::from_xyz(start.x, start.y, 0.0),
                Visibility::default(),
                ChildOf(arena),
                children![
                    // Dark shell (its authored glTF material is already near-black).
                    (
                        shell.scene(assets),
                        Transform::from_xyz(0.0, 0.0, 0.02).with_rotation(face_camera),
                    ),
                    // Emissive core — baked to this arena's colour, animated.
                    (
                        Mesh3d(meshes.add(core.to_mesh())),
                        MeshMaterial3d(materials.add(StandardMaterial {
                            base_color: Color::srgb(0.02, 0.02, 0.03),
                            emissive: LinearRgba::rgb(l.red, l.green, l.blue) * BOSS_EMISSIVE,
                            perceptual_roughness: 1.0,
                            ..default()
                        })),
                        rest,
                        BossCore {
                            behavior,
                            rest,
                            color: baked,
                        },
                    ),
                ],
            ));
        }
    }
}

/// Spawns an arena's three ambient flora/fauna props, baked to the arena theme.
fn spawn_props(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    arena: Entity,
    center: Vec2,
    props: [PropSpec; 3],
    theme: &Theme,
) {
    for prop in props {
        let (mesh, z_rest, stand) = prop.prim.to_mesh();
        let mut transform =
            Transform::from_xyz(center.x + prop.offset.x, center.y + prop.offset.y, z_rest);
        if stand {
            transform.rotation = Quat::from_rotation_x(FRAC_PI_2);
        }
        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: (prop.tint)(theme),
                perceptual_roughness: 0.8,
                ..default()
            })),
            transform,
            ChildOf(arena),
        ));
    }
}

/// Drives every [`SwarmMember`]'s drift + spin from its motion archetype.
fn animate_swarm(time: Res<Time>, mut swarm: Query<(&SwarmMember, &mut Transform)>) {
    let t = time.elapsed_secs() * SWARM_TIME_SCALE;
    for (m, mut tf) in &mut swarm {
        let (offset, rot) = swarm_offset(m.motion, m.phase, t, m.amp);
        tf.translation = m.home + offset;
        tf.rotation = rot;
    }
}

/// Drives every [`BossCore`]: its signature motion + an emissive pulse in the baked
/// arena colour (no theme lookup — the colour was resolved at spawn).
fn animate_boss_cores(
    time: Res<Time>,
    mut cores: Query<(&BossCore, &mut Transform, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let p = time.elapsed_secs();
    for (core, mut tf, mat) in &mut cores {
        let (intensity, offset, rot) = light_offset(core.behavior, p);
        tf.translation = core.rest.translation + offset;
        tf.rotation = core.rest.rotation * rot;
        if let Some(m) = materials.get_mut(&mat.0) {
            let l = core.color.to_linear();
            m.emissive = LinearRgba::rgb(l.red, l.green, l.blue) * (intensity * BOSS_EMISSIVE);
        }
    }
}

/// Demo of the tile-state API: `L` turns the tile under the puck to lava (move with
/// the arrows first). Your game logic flips tiles the same way: `board.set(arena,
/// col, row, TileKind::Lava)`.
fn kindle(
    puck: Single<(&TileMover, &ChildOf), With<Selected>>,
    arenas: Query<&Arena>,
    mut board: TileBoard,
) -> Result {
    let (mover, child_of) = *puck;
    let arena = arenas.get(child_of.parent())?;
    board.set(
        arena.index(),
        mover.col as usize,
        mover.row as usize,
        TileKind::Lava,
    );
    Ok(())
}

/// `Tab` moves the selection to the next guildmaster puck (cyclic, by [`Puck`]
/// index). Focus follows selection: [`CurrentArena`] jumps to the new puck's
/// arena (the zoomed-in camera + HUD re-theme), and the ring catches up via
/// [`follow_selected`] — the same mechanism an edge-walk uses.
fn cycle_selection(
    mut commands: Commands,
    selected: Single<(Entity, &Puck), With<Selected>>,
    pucks: Query<(Entity, &Puck, &ChildOf)>,
    arenas: Query<&Arena>,
    mut current: ResMut<CurrentArena>,
) -> Result {
    let (current_puck, here) = *selected;
    let count = pucks.iter().count();
    debug_assert!(count > 0, "invariant: the Selected puck is in `pucks`");
    let next = here.0.strict_add(1) % count;
    if let Some((entity, _, child_of)) = pucks.iter().find(|(_, p, _)| p.0 == next) {
        commands.entity(current_puck).remove::<Selected>();
        commands.entity(entity).insert(Selected);
        current.0 = arenas.get(child_of.parent())?.index();
    }
    Ok(())
}

/// Parks the selection ring on the selected puck (its local X/Y), so the halo
/// tracks `Tab`, arrow-key movement, AND arena changes: if the puck lives in a
/// different arena than the ring (Tab across arenas, or an edge-walk), the ring
/// re-parents first so the local copy lands in the right space. The ring keeps
/// its own Z.
fn follow_selected(
    mut commands: Commands,
    puck: Single<(&Transform, &ChildOf), (With<Selected>, Without<SelectionRing>)>,
    ring: Single<(Entity, &mut Transform, &ChildOf), With<SelectionRing>>,
) {
    let (puck_transform, puck_parent) = *puck;
    let (entity, mut transform, ring_parent) = ring.into_inner();
    if ring_parent.parent() != puck_parent.parent() {
        commands
            .entity(entity)
            .insert(ChildOf(puck_parent.parent()));
    }
    // Write-if-changed: an unconditional write would re-dirty the ring's
    // Transform (and its propagation) every frame the puck stands still.
    let target = puck_transform.translation.truncate();
    if transform.translation.truncate() != target {
        transform.translation.x = target.x;
        transform.translation.y = target.y;
    }
}

/// `P` toggles the camera between the overworld (all 9 arenas) and a zoom on the
/// current arena.
fn toggle_zoom(
    current: Res<CurrentArena>,
    camera: Single<(Entity, &mut Transform, Has<ZoomOut>), With<Camera3d>>,
    mut commands: Commands,
) {
    let (entity, mut transform, zoomed_out) = camera.into_inner();
    if zoomed_out {
        *transform = camera_pose(current.0, ZOOM_IN);
        commands.entity(entity).remove::<ZoomOut>();
    } else {
        *transform = camera_pose(OVERWORLD, ZOOM_OUT);
        commands.entity(entity).insert(ZoomOut);
    }
}

/// `[` / `]` select the previous / next arena (cyclic). [`refocus_camera`] follows
/// when zoomed in; zoomed out, only the selection border
/// (see [`draw_overworld_gizmos`]) moves.
fn paginate(keys: Res<ButtonInput<KeyCode>>, mut current: ResMut<CurrentArena>) {
    let dir = (keys.just_pressed(KeyCode::BracketRight) as i32)
        .strict_sub(keys.just_pressed(KeyCode::BracketLeft) as i32);
    if dir == 0 {
        return;
    }
    current.0 = (current.0 as i32).strict_add(dir).rem_euclid(ARENAS as i32) as usize;
}

/// Re-poses the zoomed-in camera whenever [`CurrentArena`] changes (pagination or
/// an edge-walk). Zoomed out ([`ZoomOut`] present) the `Single` filter finds no
/// camera and the system is skipped — the overworld stays centred.
fn refocus_camera(
    current: Res<CurrentArena>,
    mut camera: Single<&mut Transform, (With<Camera3d>, Without<ZoomOut>)>,
) {
    **camera = camera_pose(current.0, ZOOM_IN);
}

/// When zoomed out, draws the column dividers + a border around the current arena.
fn draw_overworld_gizmos(mut gizmos: Gizmos, current: Res<CurrentArena>) {
    let center = board_center();
    let top = center.y + ARENA_H * 0.5;
    let bottom = arena_offset(6).y + center.y - ARENA_H * 0.5;
    // Two vertical dividers at the seams between the three columns.
    for c in 1..=2 {
        let x = c as f32 * ARENA_W - TILE * 0.5;
        gizmos.line(
            Vec3::new(x, top, 0.5),
            Vec3::new(x, bottom, 0.5),
            Color::srgb(0.45, 0.45, 0.5),
        );
    }
    // A white border around the current arena.
    let c = arena_offset(current.0) + center;
    for i in 0..3 {
        let pad = i as f32 * 0.06;
        gizmos.rect(
            Isometry3d::from_translation(Vec3::new(c.x, c.y, 0.6)),
            Vec2::new(ARENA_W + pad, ARENA_H + pad),
            Color::WHITE,
        );
    }
}

/// `1` (or numpad `1`) casts Holy Nova from the puck: the burst VFX + its sound.
/// A selected ghost never casts live — its recorded casts play back instead.
/// While recording, the cast lands in the draft HERE, atomically with the live
/// effect (like movement in `travel::move_selected`), so the committed staff and
/// the rehearsed take can never disagree about a cast.
fn fire_holy_nova(
    mut commands: Commands,
    state: Res<RecordingState>,
    mut draft: ResMut<DraftTimeline>,
    player: Single<(Entity, &ChildOf), (With<Selected>, Without<Ghost>)>,
    clocks: Query<&ArenaClock>,
    meshes: Res<AbilityMeshes>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    sfx: Res<AbilitySfx>,
) -> Result {
    let (player, child_of) = *player;
    cast_holy_nova(&mut commands, &meshes, &mut materials, &sfx, player);
    if matches!(*state, RecordingState::Recording) {
        let tick = clocks.get(child_of.parent())?.tick;
        draft.events.push(TimelineEvent {
            tick,
            action: Action::Ability(1),
        });
    }
    Ok(())
}

/// `Esc` returns to the title — only while idle with no modal open (inside a
/// modal, Esc is the cancel key; mid-recording it would destroy the draft).
fn back_to_title(mut next: ResMut<NextState<AppState>>) {
    next.set(AppState::Title);
}
