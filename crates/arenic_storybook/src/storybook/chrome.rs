//! Storybook **chrome** — the view builders. Pure functions that assemble the
//! top-nav (collapse toggle + Layers/Theme dropdowns) and the sidebar story-tree
//! from a [`Theme`] + the current state, returning entities for the [`super`]
//! harness to parent into its hosts. Observers wired here mutate the harness state
//! resources (e.g. [`OpenMenu`], [`ActiveTheme`]); the harness then rebuilds the
//! affected region. No state or systems live here — only builders.

use arenic_game::icon::{Icon, icon};
use arenic_game::orbit::{OrbitCamera, OrbitUnlocked};
use arenic_game::theme::{ActiveTheme, Theme, ThemeId, scale};
use arenic_game::{Interactive, hidden_outline};
use bevy::prelude::*;
use bevy::ui::GlobalZIndex;

use crate::layers::{Layer, LayerVisibility};
use crate::stories::StoryId;
use crate::widgets::{self, label};

use super::{
    CurrentStory, NavMenu, OpenMenu, SIDEBAR_WIDTH_PCT, ScrollArea, Sidebar, SidebarCollapsed, TREE,
};

/// Builds the top-nav's item entities (collapse toggle, the Layers/Theme dropdowns,
/// and — for 3D stories — a spacer + tier readout + orbit toggle), in order.
/// Returned to [`super::rebuild_nav`], which parents them to the persistent nav
/// host. The collapse glyph reflects `collapsed`: the "open" glyph while collapsed.
pub(super) fn nav_items(
    commands: &mut Commands,
    assets: &AssetServer,
    theme: &Theme,
    collapsed: bool,
    open: Option<NavMenu>,
    active: ThemeId,
    layers: LayerVisibility,
    orbit_unlocked: Option<bool>,
    tier_label: Option<&str>,
) -> Vec<Entity> {
    let (glyph, collapse) = if collapsed {
        (Icon::PanelLeftOpen, false)
    } else {
        (Icon::PanelLeftClose, true)
    };
    let toggle = collapse_toggle(commands, assets, theme, glyph, collapse);

    // Layers (multiselect checkbox menu) + Theme (single-select) dropdowns. The
    // popover contents are built lazily (only for the open menu) so a closed menu
    // spawns nothing.
    let layers_menu = nav_menu(
        commands,
        assets,
        theme,
        "Layers",
        NavMenu::Layers,
        open == Some(NavMenu::Layers),
        |c| {
            Layer::ALL
                .iter()
                .map(|&(layer, name)| layer_checkbox(c, theme, layer, name, layers.get(layer)))
                .collect()
        },
    );
    let theme_menu = nav_menu(
        commands,
        assets,
        theme,
        "Theme",
        NavMenu::Theme,
        open == Some(NavMenu::Theme),
        |c| theme_popover_items(c, theme, active),
    );
    let mut items = vec![toggle, layers_menu, theme_menu];

    // 3D stories get a tier readout + an orbit (unlock camera) toggle, top-right.
    if let Some(unlocked) = orbit_unlocked {
        items.push(
            commands
                .spawn(Node {
                    flex_grow: 1.0,
                    ..default()
                })
                .id(),
        );
        if let Some(t) = tier_label {
            items.push(label(
                commands,
                &format!("Tier: {t}  [T]"),
                scale::font_size::F00,
                theme.text_muted(),
            ));
        }
        items.push(orbit_toggle(commands, assets, theme, unlocked));
    }
    items
}

/// A top-nav dropdown: a trigger button that toggles [`OpenMenu`], plus — when
/// this menu is open — an absolutely-positioned popover below it holding `items`.
/// The popover is a **sibling** of the trigger (both under `container`), so clicks
/// inside it don't bubble back to the trigger and close the menu; a checkbox toggle
/// rebuilds the tree with the menu still flagged open.
fn nav_menu(
    commands: &mut Commands,
    assets: &AssetServer,
    theme: &Theme,
    title: &str,
    menu: NavMenu,
    is_open: bool,
    // Built ONLY when the menu is open — a closed menu must spawn nothing, or the
    // unparented items would render as orphan root nodes in the window's corner.
    build_items: impl FnOnce(&mut Commands) -> Vec<Entity>,
) -> Entity {
    // A column so the absolute popover anchors directly under the trigger.
    let container = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .id();

    let (bg, fg, border) = if is_open {
        (theme.selected_tint(), theme.text_1(), theme.border_strong())
    } else {
        (Color::NONE, theme.text_2(), Color::NONE)
    };
    let trigger = commands
        .spawn((
            Button,
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(scale::space::XS3),
                padding: UiRect::axes(Val::Px(scale::space::XS), Val::Px(scale::space::XS3)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(scale::radius::XS)),
                ..default()
            },
            BackgroundColor(bg),
            BorderColor::all(border),
        ))
        .id();
    let text = label(commands, title, scale::font_size::F1, fg);
    let chevron = icon(commands, assets, Icon::ChevronDown, 14.0, fg);
    commands.entity(trigger).add_children(&[text, chevron]);
    make_interactive(commands, theme, trigger, bg);
    commands
        .entity(trigger)
        .observe(move |_: On<Pointer<Click>>, mut open: ResMut<OpenMenu>| {
            // Toggle: clicking the open menu's trigger closes it.
            open.0 = if open.0 == Some(menu) {
                None
            } else {
                Some(menu)
            };
        });

    let mut children = vec![trigger];
    if is_open {
        let items = build_items(commands);
        let popover = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Percent(100.0),
                    left: Val::Px(0.0),
                    margin: UiRect::top(Val::Px(scale::space::XS3)),
                    width: Val::Px(220.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(scale::space::XS3),
                    padding: UiRect::all(Val::Px(scale::space::XS)),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(scale::radius::S)),
                    ..default()
                },
                BackgroundColor(theme.surface_3()),
                BorderColor::all(theme.border_strong()),
                // Float above the canvas (a sibling node that's spawned later).
                GlobalZIndex(100),
            ))
            .id();
        commands.entity(popover).add_children(&items);
        children.push(popover);
    }
    commands.entity(container).add_children(&children);
    container
}

/// The Theme dropdown's contents: the Game and Extra theme pills, each a labelled
/// wrapped row. Selecting a pill sets the theme and closes the menu (single-select).
fn theme_popover_items(commands: &mut Commands, theme: &Theme, active: ThemeId) -> Vec<Entity> {
    let mut items = Vec::new();
    for (name, ids) in [("Game", &ThemeId::GAME[..]), ("Extra", &ThemeId::EXTRA[..])] {
        items.push(label(
            commands,
            name,
            scale::font_size::F00,
            theme.text_muted(),
        ));
        let wrap = widgets::wrap(commands, scale::space::XS3);
        let buttons: Vec<Entity> = ids
            .iter()
            .map(|&id| theme_button(commands, theme, id, id == active))
            .collect();
        commands.entity(wrap).add_children(&buttons);
        items.push(wrap);
    }
    items
}

/// The "unlock 3D camera" toggle (top-right of the canvas for 3D stories).
/// Clicking flips every [`OrbitCamera`]'s [`OrbitUnlocked`] marker between locked
/// (home pose) and unlocked (mouse/trackpad orbit-pan-zoom); locking snaps back home.
fn orbit_toggle(
    commands: &mut Commands,
    assets: &AssetServer,
    theme: &Theme,
    unlocked: bool,
) -> Entity {
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
        |_: On<Pointer<Click>>,
         mut commands: Commands,
         mut cams: Query<(Entity, &mut OrbitCamera, Has<OrbitUnlocked>)>| {
            for (entity, mut cam, unlocked) in &mut cams {
                if unlocked {
                    commands.entity(entity).remove::<OrbitUnlocked>();
                    cam.reset();
                } else {
                    commands.entity(entity).insert(OrbitUnlocked);
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
    commands.entity(button).observe(
        move |_: On<Pointer<Click>>, mut state: ResMut<SidebarCollapsed>| {
            state.0 = collapse;
        },
    );
    button
}

pub(super) fn build_sidebar(
    commands: &mut Commands,
    assets: &AssetServer,
    theme: &Theme,
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

    // The sidebar is now just the story tree (theme + layers live in the top nav).
    let stories_title = commands
        .spawn((
            Text::new("Stories"),
            TextFont {
                font_size: scale::font_size::F2,
                ..default()
            },
            TextColor(theme.text_1()),
        ))
        .id();

    let chevron_down = Icon::ChevronDown.handle(assets);
    let chevron_right = Icon::ChevronRight.handle(assets);

    let mut rows = vec![stories_title];
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
                    .is_ok_and(|n| n.display == Display::None);
                if let Ok(mut node) = nodes.get_mut(container) {
                    node.display = if collapsed {
                        Display::Flex
                    } else {
                        Display::None
                    };
                }
                if let Ok(mut img) = images.get_mut(chevron) {
                    img.image = if collapsed {
                        down.clone()
                    } else {
                        right.clone()
                    };
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
    commands.entity(button).observe(
        move |_: On<Pointer<Click>>,
              mut active: ResMut<ActiveTheme>,
              mut open: ResMut<OpenMenu>| {
            active.0 = id;
            open.0 = None; // single-select: picking a theme closes the menu
        },
    );
    button
}

/// A small checkbox row for a 3D-stage layer: a filled square when the layer is
/// visible, empty when hidden, plus its label. Clicking toggles the layer (which
/// re-renders the stage via the shared `LayerVisibility` resource).
fn layer_checkbox(
    commands: &mut Commands,
    theme: &Theme,
    layer: Layer,
    name: &str,
    checked: bool,
) -> Entity {
    let fill = if checked { theme.brand() } else { Color::NONE };
    let square = commands
        .spawn((
            Node {
                width: Val::Px(13.0),
                height: Val::Px(13.0),
                flex_shrink: 0.0,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(scale::radius::XS2)),
                ..default()
            },
            BackgroundColor(fill),
            BorderColor::all(theme.border_bold()),
        ))
        .id();
    let text = label(commands, name, scale::font_size::F0, theme.text_2());
    let row = commands
        .spawn((
            Button,
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(scale::space::XS3),
                padding: UiRect::axes(Val::Px(scale::space::XS), Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(scale::radius::XS2)),
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .id();
    commands.entity(row).add_children(&[square, text]);
    make_interactive(commands, theme, row, Color::NONE);
    commands.entity(row).observe(
        move |_: On<Pointer<Click>>, mut vis: ResMut<LayerVisibility>| {
            vis.toggle(layer);
        },
    );
    row
}

fn folder_header(
    commands: &mut Commands,
    assets: &AssetServer,
    theme: &Theme,
    name: &str,
) -> (Entity, Entity) {
    // Lucide chevron (swapped down/right by the toggle observer) + a folder icon.
    let chevron = icon(
        commands,
        assets,
        Icon::ChevronDown,
        14.0,
        theme.text_muted(),
    );
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
    commands
        .entity(header)
        .add_children(&[chevron, folder, text]);
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
    commands.entity(row).observe(
        move |_: On<Pointer<Click>>,
              mut current: ResMut<CurrentStory>,
              mut active: ResMut<ActiveTheme>| {
            // Guard so re-clicking the active leaf doesn't re-fire the change
            // and needlessly despawn/reload the 3D content (a glTF reload flash).
            if current.0 != Some(story) {
                current.0 = Some(story);
            }
            // Each arena opens in its matched theme, so the boss reads
            // on-theme. Non-arena stories leave the theme as-is.
            if let Some(t) = story.arena_theme()
                && active.0 != t
            {
                active.0 = t;
            }
        },
    );
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
