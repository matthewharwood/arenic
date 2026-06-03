//! Storybook harness: a top navigation bar (with the sidebar collapse toggle on
//! its left) above a body that pairs an IDE-style sidebar (theme switcher + a
//! file-tree of stories) with a scrollable canvas that renders the selected
//! story. Collapsing the sidebar removes it from the body entirely; the nav-bar
//! toggle stays put so it can be reopened.
//!
//! The UI is a pure function of two resources — [`ActiveTheme`] and
//! [`CurrentStory`]. Changing either rebuilds the whole tree from scratch, so
//! the design tokens drive every pixel and theme-switching is automatic.

use arenic_game::icon::{Icon, icon};
use arenic_game::interaction::{Interactive, hidden_outline};
use arenic_game::orbit::OrbitCamera;
use arenic_game::theme::{ActiveTheme, Theme, ThemeId, scale};
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;

use crate::stories::{self, StoryId};
use crate::widgets::{self, label};

pub struct StorybookPlugin;

impl Plugin for StorybookPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActiveTheme>()
            .init_resource::<CurrentStory>()
            .init_resource::<SidebarCollapsed>()
            .add_systems(Startup, crate::stage::setup_stage)
            .add_systems(
                Update,
                (
                    rebuild.run_if(
                        resource_changed::<ActiveTheme>
                            .or(resource_changed::<CurrentStory>)
                            .or(resource_changed::<SidebarCollapsed>),
                    ),
                    scroll_canvas,
                ),
            );
    }
}

/// Sidebar width as a percentage of the window — fixed, never shrinks to fit
/// content (see `flex_shrink: 0.0` below). The canvas takes the remainder.
const SIDEBAR_WIDTH_PCT: f32 = 20.0;

/// The currently selected story (`None` = nothing picked yet).
#[derive(Resource, Default)]
struct CurrentStory(Option<StoryId>);

/// Whether the sidebar is collapsed (canvas takes the full window).
#[derive(Resource, Default)]
struct SidebarCollapsed(bool);

/// Root of the rebuildable UI tree.
#[derive(Component)]
struct UiRoot;

/// The left navigation pane (theme switcher + story tree).
#[derive(Component)]
pub struct Sidebar;

/// The main content pane that renders the selected story.
#[derive(Component)]
pub struct Canvas;

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
            ("components", StoryId::Components, Icon::Component),
        ],
    ),
    ("units", &[("guildmaster", StoryId::Guildmaster, Icon::User)]),
];

/// Tears down and rebuilds the entire UI for the active theme + selected story.
fn rebuild(
    mut commands: Commands,
    assets: Res<AssetServer>,
    stage: Res<crate::stage::Stage3d>,
    active: Res<ActiveTheme>,
    current: Res<CurrentStory>,
    collapsed: Res<SidebarCollapsed>,
    orbit: Query<&OrbitCamera>,
    roots: Query<Entity, With<UiRoot>>,
) {
    for e in &roots {
        commands.entity(e).despawn();
    }
    let theme = active.palette();

    let root = commands
        .spawn((
            UiRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                ..default()
            },
            BackgroundColor(theme.surface_1()),
        ))
        .id();

    // Full-height sidebar on the left, spawned only when expanded — collapsing
    // leaves no rail behind.
    let sidebar = (!collapsed.0)
        .then(|| build_sidebar(&mut commands, &assets, &theme, active.0, current.0));

    // Right column: the navigation bar stacked above the canvas. It sits beside
    // the sidebar (not over it), so the nav starts at the sidebar's right edge.
    let right = commands
        .spawn(Node {
            flex_grow: 1.0,
            min_width: Val::Px(0.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .id();

    // Navigation bar with the collapse toggle at its left edge — always present
    // so a collapsed sidebar can be reopened.
    // Show the orbit toggle only for 3D stories; reflect the camera's lock state.
    let orbit_unlocked = current
        .0
        .filter(|s| s.has_3d_stage())
        .map(|_| orbit.iter().next().is_some_and(|c| c.unlocked));
    let topnav = build_topnav(&mut commands, &assets, &theme, collapsed.0, orbit_unlocked);

    // Scrollable canvas with an inner content column we render the story into.
    // `flex_grow` makes it fill the column below the nav; `min_height: 0` lets it
    // shrink so its content can clip/scroll rather than push the layout around.
    let canvas = commands
        .spawn((
            Canvas,
            ScrollArea,
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
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
    stories::render(&mut commands, content, &theme, current.0, &stage.image);

    commands.entity(right).add_children(&[topnav, canvas]);

    // Root holds the sidebar (when present) then the nav+canvas column.
    let mut root_children = Vec::with_capacity(2);
    root_children.extend(sidebar);
    root_children.push(right);
    commands.entity(root).add_children(&root_children);
}

/// The navigation bar: a strip above the canvas with the sidebar collapse/expand
/// toggle anchored at its left. The toggle reflects `collapsed` — showing the
/// "open" glyph (which expands) while collapsed, the "close" glyph otherwise.
fn build_topnav(
    commands: &mut Commands,
    assets: &AssetServer,
    theme: &Theme,
    collapsed: bool,
    orbit_unlocked: Option<bool>,
) -> Entity {
    let nav = commands
        .spawn((
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
            BackgroundColor(theme.surface_2()),
            BorderColor::all(theme.border_subtle()),
        ))
        .id();
    let (glyph, collapse) = if collapsed {
        (Icon::PanelLeftOpen, false)
    } else {
        (Icon::PanelLeftClose, true)
    };
    let toggle = collapse_toggle(commands, assets, theme, glyph, collapse);
    commands.entity(nav).add_children(&[toggle]);

    // 3D stories get an orbit (unlock camera) toggle pinned to the top-right.
    if let Some(unlocked) = orbit_unlocked {
        let spacer = commands
            .spawn(Node {
                flex_grow: 1.0,
                ..default()
            })
            .id();
        let orbit = orbit_toggle(commands, assets, theme, unlocked);
        commands.entity(nav).add_children(&[spacer, orbit]);
    }
    nav
}

/// The "unlock 3D camera" toggle (top-right of the canvas for 3D stories).
/// Clicking flips every [`OrbitCamera`] between locked (home pose) and unlocked
/// (mouse/trackpad orbit-pan-zoom); locking snaps back home.
fn orbit_toggle(commands: &mut Commands, assets: &AssetServer, theme: &Theme, unlocked: bool) -> Entity {
    let bg = if unlocked {
        theme.selected_tint()
    } else {
        Color::NONE
    };
    let fg = if unlocked {
        theme.brand()
    } else {
        theme.text_muted()
    };
    let button = commands
        .spawn((
            Button,
            Node {
                width: Val::Px(28.0),
                height: Val::Px(28.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(scale::radius::XS)),
                ..default()
            },
            BackgroundColor(bg),
            BorderColor::all(theme.border_subtle()),
        ))
        .id();
    let glyph = icon(commands, assets, Icon::Rotate3d, 18.0, fg);
    commands.entity(button).add_children(&[glyph]);
    make_interactive(commands, theme, button, bg);
    commands.entity(button).observe(
        |_: On<Pointer<Click>>, mut cams: Query<&mut OrbitCamera>| {
            for mut cam in &mut cams {
                cam.unlocked = !cam.unlocked;
                if !cam.unlocked {
                    cam.reset();
                }
            }
        },
    );
    button
}

/// The sidebar collapse/expand button — a tinted Lucide icon. `collapse` chooses
/// the target state.
fn collapse_toggle(
    commands: &mut Commands,
    assets: &AssetServer,
    theme: &Theme,
    glyph: Icon,
    collapse: bool,
) -> Entity {
    let button = commands
        .spawn((
            Button,
            Node {
                width: Val::Px(28.0),
                height: Val::Px(28.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(scale::radius::XS)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(theme.border_subtle()),
        ))
        .id();
    let glyph = icon(commands, assets, glyph, 18.0, theme.text_2());
    commands.entity(button).add_children(&[glyph]);
    make_interactive(commands, theme, button, Color::NONE);
    commands
        .entity(button)
        .observe(move |_: On<Pointer<Click>>, mut state: ResMut<SidebarCollapsed>| {
            state.0 = collapse;
        });
    button
}

fn build_sidebar(
    commands: &mut Commands,
    assets: &AssetServer,
    theme: &Theme,
    active: ThemeId,
    current: Option<StoryId>,
) -> Entity {
    let sidebar = commands
        .spawn((
            Node {
                // Fixed fraction of the window; never shrinks to fit content.
                width: Val::Percent(SIDEBAR_WIDTH_PCT),
                flex_shrink: 0.0,
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(scale::space::S)),
                row_gap: Val::Px(scale::space::XS2),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(theme.surface_2()),
            ScrollPosition::default(),
            ScrollArea,
            Sidebar,
        ))
        .id();

    // --- Header: palette icon + "Theme" title (collapse toggle lives in the nav) ---
    let header = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(scale::space::XS3),
            ..default()
        })
        .id();
    let palette = icon(commands, assets, Icon::Palette, 14.0, theme.text_muted());
    let theme_title = label(commands, "Theme", scale::font_size::F00, theme.text_muted());
    commands.entity(header).add_children(&[palette, theme_title]);

    // --- Theme switcher ---
    let switcher = widgets::wrap(commands, scale::space::XS3);
    let buttons: Vec<Entity> = ThemeId::ALL
        .iter()
        .map(|&id| theme_button(commands, theme, id, id == active))
        .collect();
    commands.entity(switcher).add_children(&buttons);

    // --- Stories header ---
    let stories_title = commands
        .spawn((
            Text::new("Stories"),
            TextFont {
                font_size: scale::font_size::F2,
                ..default()
            },
            TextColor(theme.text_1()),
            Node {
                margin: UiRect::top(Val::Px(scale::space::XS)),
                ..default()
            },
        ))
        .id();

    let chevron_down = Icon::ChevronDown.handle(assets);
    let chevron_right = Icon::ChevronRight.handle(assets);

    let mut rows = vec![header, switcher, stories_title];
    for (folder, leaves) in TREE {
        let (header, chevron) = folder_header(commands, assets, theme, folder);
        let leaf_rows: Vec<Entity> = leaves
            .iter()
            .map(|(name, story, leaf_icon)| {
                leaf_row(
                    commands,
                    assets,
                    theme,
                    name,
                    *story,
                    *leaf_icon,
                    current == Some(*story),
                )
            })
            .collect();
        let container = commands
            .spawn(Node {
                flex_direction: FlexDirection::Column,
                ..default()
            })
            .id();
        commands.entity(container).add_children(&leaf_rows);

        // Toggle the folder open/closed, swapping the chevron icon to match.
        let (down, right) = (chevron_down.clone(), chevron_right.clone());
        commands.entity(header).observe(
            move |_: On<Pointer<Click>>,
                  mut nodes: Query<&mut Node>,
                  mut images: Query<&mut ImageNode>| {
                let collapsed = nodes
                    .get(container)
                    .map(|n| n.display == Display::None)
                    .unwrap_or(false);
                if let Ok(mut node) = nodes.get_mut(container) {
                    node.display = if collapsed {
                        Display::Flex
                    } else {
                        Display::None
                    };
                }
                if let Ok(mut img) = images.get_mut(chevron) {
                    img.image = if collapsed { down.clone() } else { right.clone() };
                }
            },
        );
        rows.push(header);
        rows.push(container);
    }

    commands.entity(sidebar).add_children(&rows);
    sidebar
}

fn theme_button(commands: &mut Commands, theme: &Theme, id: ThemeId, selected: bool) -> Entity {
    let bg = if selected {
        theme.selected_tint()
    } else {
        Color::NONE
    };
    let fg = if selected {
        theme.text_1()
    } else {
        theme.text_muted()
    };
    let border = if selected {
        theme.border_strong()
    } else {
        theme.border_subtle()
    };
    let button = commands
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(scale::space::XS2), Val::Px(scale::space::XS3)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(scale::radius::PILL)),
                ..default()
            },
            BackgroundColor(bg),
            BorderColor::all(border),
        ))
        .id();
    let text = label(commands, id.label(), scale::font_size::F00, fg);
    commands.entity(button).add_children(&[text]);
    make_interactive(commands, theme, button, bg);
    commands
        .entity(button)
        .observe(move |_: On<Pointer<Click>>, mut active: ResMut<ActiveTheme>| {
            active.0 = id;
        });
    button
}

fn folder_header(
    commands: &mut Commands,
    assets: &AssetServer,
    theme: &Theme,
    name: &str,
) -> (Entity, Entity) {
    // Lucide chevron (swapped down/right by the toggle observer) + a folder icon.
    let chevron = icon(commands, assets, Icon::ChevronDown, 14.0, theme.text_muted());
    let folder = icon(commands, assets, Icon::Folder, 16.0, theme.text_muted());
    let text = label(commands, name, scale::font_size::F0, theme.text_2());
    let header = commands
        .spawn((
            Button,
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(scale::space::XS3),
                padding: UiRect::axes(Val::Px(4.0), Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(scale::radius::XS2)),
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .id();
    commands.entity(header).add_children(&[chevron, folder, text]);
    make_interactive(commands, theme, header, Color::NONE);
    (header, chevron)
}

fn leaf_row(
    commands: &mut Commands,
    assets: &AssetServer,
    theme: &Theme,
    name: &str,
    story: StoryId,
    leaf_icon: Icon,
    selected: bool,
) -> Entity {
    let base = if selected {
        theme.selected_tint()
    } else {
        Color::NONE
    };
    let fg = if selected {
        theme.text_1()
    } else {
        theme.text_2()
    };
    let glyph = icon(commands, assets, leaf_icon, 16.0, fg);
    let text = label(commands, name, scale::font_size::F0, fg);
    let row = commands
        .spawn((
            Button,
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(scale::space::XS3),
                padding: UiRect {
                    left: Val::Px(scale::space::M),
                    right: Val::Px(4.0),
                    top: Val::Px(3.0),
                    bottom: Val::Px(3.0),
                },
                border_radius: BorderRadius::all(Val::Px(scale::radius::XS2)),
                ..default()
            },
            BackgroundColor(base),
        ))
        .id();
    commands.entity(row).add_children(&[glyph, text]);
    make_interactive(commands, theme, row, base);
    commands
        .entity(row)
        .observe(move |_: On<Pointer<Click>>, mut current: ResMut<CurrentStory>| {
            current.0 = Some(story);
        });
    row
}

/// Makes `entity` a themeable flat control: hover/active background tints plus a
/// focus ring, all derived from the theme and its resting colour `rest`.
fn make_interactive(commands: &mut Commands, theme: &Theme, entity: Entity, rest: Color) {
    let (hover, active) = theme.interactions(rest);
    commands.entity(entity).insert((
        Interactive::flat(rest, hover, active, theme.focus_ring()),
        hidden_outline(),
    ));
}

/// Scrolls whichever scroll areas exist by the mouse wheel delta — unless an
/// orbit camera is unlocked, in which case the wheel/trackpad drives the camera
/// instead (see `arenic_game::orbit`).
fn scroll_canvas(
    mut wheel: MessageReader<MouseWheel>,
    mut areas: Query<&mut ScrollPosition, With<ScrollArea>>,
    orbit: Query<&OrbitCamera>,
) {
    if orbit.iter().any(|c| c.unlocked) {
        return;
    }
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
