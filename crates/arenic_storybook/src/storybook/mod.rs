//! Storybook **harness** — the engine that drives the UI as a pure function of
//! state (see `README.md` for the architecture). It owns the UI state (resources),
//! the persistent skeleton (a top-nav host + a body host), the two region-rebuild
//! systems that refill those hosts when their inputs change, the story registry
//! ([`TREE`]), and scroll. The view builders (the nav dropdowns + sidebar tree)
//! live in the sibling [`chrome`] module; this module decides *when* to rebuild
//! each region and calls into `chrome` to build it.
//!
//! `UI = f(state)`: changing [`ActiveTheme`]/[`CurrentStory`]/[`SidebarCollapsed`]
//! refills the body (the expensive part); those plus [`OpenMenu`], the tier, and
//! [`crate::layers::LayerVisibility`] refill the cheap nav. An open multiselect
//! menu re-renders *still open* because its state lives in the [`OpenMenu`]
//! resource, not in the despawned tree.

mod chrome;

use arenic_game::default_font::MonoFont;
use arenic_game::icon::Icon;
use arenic_game::orbit::{OrbitCamera, OrbitUnlocked};
use arenic_game::theme::{ActiveTheme, scale};
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;

use crate::hollow::Tier;
use crate::layers::LayerVisibility;
use crate::stage::{Stage3d, StagePlugin};
use crate::stories::{self, ActionBarKnobs, StoryId};

pub struct StorybookPlugin;

impl Plugin for StorybookPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActiveTheme>()
            .init_resource::<CurrentStory>()
            .init_resource::<SidebarCollapsed>()
            .init_resource::<OpenMenu>()
            .init_resource::<ActionBarKnobs>()
            .add_plugins(StagePlugin)
            .add_systems(Startup, setup_shell)
            .add_systems(
                Update,
                (
                    // The BODY (sidebar + story canvas) is the expensive part — it is
                    // refilled ONLY when its content actually changes (story / theme /
                    // collapse), so a layer toggle or a menu-open never re-renders the
                    // story content. That full teardown was the click flicker.
                    rebuild_body.run_if(
                        resource_changed::<ActiveTheme>
                            .or(resource_changed::<CurrentStory>)
                            .or(resource_changed::<SidebarCollapsed>)
                            // Flipping an Action Bar knob re-renders the story body.
                            .or(resource_changed::<ActionBarKnobs>),
                    ),
                    // The NAV is a handful of nodes and reflects everything, so it can
                    // refill on any of these without touching the body.
                    rebuild_nav.run_if(
                        resource_changed::<ActiveTheme>
                            .or(resource_changed::<CurrentStory>)
                            .or(resource_changed::<SidebarCollapsed>)
                            .or(resource_changed::<OpenMenu>)
                            .or(resource_changed::<Tier>)
                            .or(resource_changed::<LayerVisibility>),
                    ),
                    scroll_canvas.run_if(orbit_locked),
                ),
            );
    }
}

/// Sidebar width as a percentage of the window — fixed, never shrinks to fit
/// content (see `flex_shrink: 0.0` below). The canvas takes the remainder.
const SIDEBAR_WIDTH_PCT: f32 = 20.0;

/// The currently selected story (`None` = nothing picked yet). Read by
/// [`crate::stage`] to swap the 3D stage's content to match.
#[derive(Resource, Default)]
pub(crate) struct CurrentStory(pub(crate) Option<StoryId>);

/// Whether the sidebar is collapsed (canvas takes the full window).
#[derive(Resource, Default)]
struct SidebarCollapsed(bool);

/// A top-nav dropdown menu.
#[derive(Clone, Copy, PartialEq, Eq)]
enum NavMenu {
    Layers,
    Theme,
}

/// Which top-nav dropdown is open (`None` = all closed). Held in a resource so it
/// survives the from-scratch rebuild — a multiselect menu stays open while you
/// tick checkboxes inside it.
#[derive(Resource, Default)]
struct OpenMenu(Option<NavMenu>);

/// The persistent UI root, spawned once. Holds [`NavHost`] + [`BodyHost`].
#[derive(Component)]
struct UiRoot;

/// The persistent full-width top-nav host. Its children are refilled by
/// [`rebuild_nav`]; the host entity itself never despawns, so a nav refresh never
/// tears down the body.
#[derive(Component)]
struct NavHost;

/// The persistent body host (sidebar + canvas). Its children are refilled by
/// [`rebuild_body`].
#[derive(Component)]
struct BodyHost;

/// The left navigation pane (the story tree; theme + layers live in the top nav).
#[derive(Component)]
struct Sidebar;

/// The main content pane that renders the selected story.
#[derive(Component)]
struct Canvas;

/// Marks a scrollable viewport so the wheel handler can drive its
/// [`ScrollPosition`]. Both the [`Sidebar`] and the [`Canvas`] carry it.
#[derive(Component)]
struct ScrollArea;

/// A leaf in the story tree: display name, the story it opens, and its icon.
type Leaf = (&'static str, StoryId, Icon);
/// A folder in the story tree: display name and its leaves.
type Folder = (&'static str, &'static [Leaf]);

/// The story tree shown in the sidebar.
const TREE: &[Folder] = &[
    (
        "styles",
        &[
            ("colors", StoryId::Colors, Icon::SwatchBook),
            ("typography", StoryId::Typography, Icon::Type),
            ("spacing", StoryId::Spacing, Icon::Ruler),
            ("radii", StoryId::Radii, Icon::Box),
            ("elevation", StoryId::Elevation, Icon::Layers),
        ],
    ),
    (
        "components",
        &[
            ("buttons", StoryId::Buttons, Icon::Component),
            ("action bar", StoryId::ActionBar, Icon::Layers),
        ],
    ),
    (
        "units",
        &[
            // The nine arenas, in 3×3 grid order: class (arena).
            ("hunter", StoryId::Hunter, Icon::Crosshair), // Labyrinth
            ("guildmaster", StoryId::Guildmaster, Icon::Pyramid), // Guild House (home)
            ("cardinal", StoryId::Cardinal, Icon::Donut), // Sanctum
            ("forager", StoryId::Forager, Icon::Mountain), // Mountain
            ("warrior", StoryId::Warrior, Icon::Hexagon), // Bastion
            ("thief", StoryId::Thief, Icon::Triangle),    // Pawnshop
            ("alchemist", StoryId::Alchemist, Icon::FlaskConical), // Crucible
            ("merchant", StoryId::Merchant, Icon::Gem),   // Casino
            ("bard", StoryId::Bard, Icon::Music),         // Gala
        ],
    ),
    (
        "abilities",
        &[("holy nova", StoryId::HolyNova, Icon::Sparkles)],
    ),
];

/// Builds the persistent UI skeleton ONCE: a full-width top-nav host above a body
/// host (sidebar + canvas). The two hosts are refilled independently — so a cheap
/// change (a layer toggle, a menu-open) never tears down the expensive story body.
fn setup_shell(mut commands: Commands) {
    commands.spawn((
        UiRoot,
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            ..default()
        },
        children![
            (
                NavHost,
                Node {
                    width: Val::Percent(100.0),
                    flex_shrink: 0.0,
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(scale::space::XS2),
                    padding: UiRect::axes(Val::Px(scale::space::XS), Val::Px(scale::space::XS3)),
                    border: UiRect::bottom(Val::Px(1.0)),
                    ..default()
                },
            ),
            (
                BodyHost,
                Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    min_height: Val::Px(0.0),
                    flex_direction: FlexDirection::Row,
                    ..default()
                },
            ),
        ],
    ));
}

/// Refills the top-nav host: collapse toggle, the Layers/Theme dropdowns, and —
/// for 3D stories — a tier readout + orbit toggle. Cheap (a handful of nodes), so
/// it can run on any nav-affecting change WITHOUT re-rendering the story body.
fn rebuild_nav(
    mut commands: Commands,
    assets: Res<AssetServer>,
    active: Res<ActiveTheme>,
    current: Res<CurrentStory>,
    collapsed: Res<SidebarCollapsed>,
    open: Res<OpenMenu>,
    tier: Res<Tier>,
    layers: Res<LayerVisibility>,
    orbit: Query<Has<OrbitUnlocked>, With<OrbitCamera>>,
    host: Single<Entity, With<NavHost>>,
) {
    let host = *host;
    commands.entity(host).despawn_related::<Children>();
    let theme = active.palette();
    commands.entity(host).insert((
        BackgroundColor(theme.surface_2()),
        BorderColor::all(theme.border_subtle()),
    ));

    // The orbit + tier controls show only for 3D stories.
    let orbit_unlocked = current
        .0
        .filter(|s| s.has_3d_stage())
        .map(|_| orbit.iter().next().unwrap_or(false));
    let tier_label = current.0.filter(|s| s.has_3d_stage()).map(|_| tier.label());

    let items = chrome::nav_items(
        &mut commands,
        &assets,
        &theme,
        collapsed.0,
        open.0,
        active.0,
        *layers,
        orbit_unlocked,
        tier_label,
    );
    commands.entity(host).add_children(&items);
}

/// Refills the body host: the sidebar (when expanded) + the scrollable canvas that
/// renders the selected story. The expensive rebuild — runs only when the story,
/// theme, or collapse state changes, never on a layer toggle or menu-open.
fn rebuild_body(
    mut commands: Commands,
    assets: Res<AssetServer>,
    stage: Res<Stage3d>,
    active: Res<ActiveTheme>,
    mono: Res<MonoFont>,
    current: Res<CurrentStory>,
    collapsed: Res<SidebarCollapsed>,
    knobs: Res<ActionBarKnobs>,
    host: Single<Entity, With<BodyHost>>,
) {
    let host = *host;
    commands.entity(host).despawn_related::<Children>();
    let theme = active.palette();

    // Full-height sidebar on the left, spawned only when expanded.
    let sidebar =
        (!collapsed.0).then(|| chrome::build_sidebar(&mut commands, &assets, &theme, current.0));

    // Scrollable canvas filling the rest of the row; `min_width: 0` lets it shrink
    // beside the sidebar, `min_height: 0` lets its content scroll rather than push.
    let canvas = commands
        .spawn((
            Canvas,
            ScrollArea,
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                height: Val::Percent(100.0),
                min_height: Val::Px(0.0),
                padding: UiRect::all(Val::Px(scale::space::L)),
                flex_direction: FlexDirection::Column,
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(theme.surface_1()),
            ScrollPosition::default(),
        ))
        .id();
    let content = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(scale::space::M),
            ..default()
        })
        .id();
    commands.entity(canvas).add_children(&[content]);
    stories::render(
        &mut commands,
        content,
        &theme,
        &mono.0,
        current.0,
        &stage.image,
        *knobs,
    );

    let mut children = Vec::with_capacity(2);
    children.extend(sidebar);
    children.push(canvas);
    commands.entity(host).add_children(&children);
}

/// Scrolls whichever scroll areas exist by the mouse wheel delta — gated off
/// while an orbit camera is unlocked, in which case the wheel/trackpad drives
/// the camera instead (see `arenic_game::orbit`).
fn scroll_canvas(
    mut wheel: MessageReader<MouseWheel>,
    mut areas: Query<&mut ScrollPosition, With<ScrollArea>>,
) {
    let mut dy = 0.0;
    for ev in wheel.read() {
        dy += match ev.unit {
            MouseScrollUnit::Line => ev.y * 24.0,
            MouseScrollUnit::Pixel => ev.y,
        };
    }
    if dy == 0.0 {
        return;
    }
    for mut pos in &mut areas {
        pos.0.y = (pos.0.y - dy).max(0.0);
    }
}

/// Run condition for [`scroll_canvas`]: the wheel scrolls the UI only while no
/// orbit camera is unlocked.
fn orbit_locked(orbit: Query<(), (With<OrbitCamera>, With<OrbitUnlocked>)>) -> bool {
    orbit.is_empty()
}
