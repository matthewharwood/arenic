use bevy::prelude::*;

/// Storybook harness: a place to build and navigate between small, isolated game
/// pieces ("stories") outside the main game loop. This is its own binary, so the
/// whole app *is* the storybook — the UI is spawned once at startup.
///
/// Layout is a classic two-pane: a left sidebar with a collapsible file-tree of
/// stories (mirroring an IDE project panel), and a main content pane that
/// renders the selected one.
pub struct StorybookPlugin;

impl Plugin for StorybookPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup);
    }
}

/// Marker for the content pane, so the story-selection observers can clear and
/// repopulate it without touching the sidebar.
#[derive(Component)]
pub struct StoryCanvas;

/// The story tree: each folder holds a list of leaf stories. Blank for now —
/// selecting a leaf just renders its title.
const TREE: &[(&str, &[&str])] = &[
    ("units", &["guildmaster"]),
    ("styles", &["typography"]),
];

const SIDEBAR_BG: Color = Color::srgb(0.10, 0.10, 0.12);
const CANVAS_BG: Color = Color::srgb(0.06, 0.06, 0.07);
const ROW_HOVER: Color = Color::srgb(0.16, 0.16, 0.20);
const ROW_IDLE: Color = Color::NONE;
const TREE_TEXT: Color = Color::srgb(0.80, 0.80, 0.84);
const CHEVRON: Color = Color::srgb(0.55, 0.55, 0.60);
/// Tree row font, sized to read like an IDE project panel.
const TREE_FONT: f32 = 14.0;

/// Disclosure-chevron orientations. The marker is a top+right border corner
/// (vertex pointing up-right at identity); these rotations point that vertex
/// down (expanded) or right (collapsed).
fn chevron_down() -> Rot2 {
    Rot2::degrees(135.0)
}
fn chevron_right() -> Rot2 {
    Rot2::degrees(45.0)
}

fn setup(mut commands: Commands) {
    // Root: full-screen horizontal split.
    let root = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            ..default()
        })
        .id();

    // Left sidebar: a "Stories" header and the story tree.
    let sidebar = commands
        .spawn((
            Node {
                width: Val::Px(260.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(16.0)),
                row_gap: Val::Px(4.0),
                ..default()
            },
            BackgroundColor(SIDEBAR_BG),
        ))
        .id();

    let sidebar_title = commands
        .spawn((
            Text::new("Stories"),
            TextFont {
                font_size: 24.0,
                ..default()
            },
            TextColor(Color::WHITE),
            Node {
                margin: UiRect::bottom(Val::Px(12.0)),
                ..default()
            },
        ))
        .id();

    let mut rows = vec![sidebar_title];
    for (folder, leaves) in TREE {
        let (header, chevron) = folder_header(&mut commands, folder);

        let leaf_rows: Vec<Entity> = leaves.iter().map(|l| leaf_row(&mut commands, l)).collect();
        let container = commands
            .spawn(Node {
                flex_direction: FlexDirection::Column,
                ..default()
            })
            .id();
        commands.entity(container).add_children(&leaf_rows);

        // Clicking the folder toggles its children and flips the chevron.
        // Capture the related entities so the toggle is robust no matter which
        // descendant the click actually lands on.
        commands.entity(header).observe(
            move |_: On<Pointer<Click>>,
                  mut nodes: Query<&mut Node>,
                  mut transforms: Query<&mut UiTransform>| {
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
                if let Ok(mut transform) = transforms.get_mut(chevron) {
                    transform.rotation = if collapsed { chevron_down() } else { chevron_right() };
                }
            },
        );

        rows.push(header);
        rows.push(container);
    }

    commands.entity(sidebar).add_children(&rows);

    // Right pane: where a selected story renders.
    let canvas = commands
        .spawn((
            StoryCanvas,
            Node {
                flex_grow: 1.0,
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(CANVAS_BG),
            children![(
                Text::new("Select a story"),
                TextFont {
                    font_size: 24.0,
                    ..default()
                },
                TextColor(Color::srgb(0.4, 0.4, 0.45)),
            )],
        ))
        .id();

    commands.entity(root).add_children(&[sidebar, canvas]);
}

/// A collapsible folder row: a chevron + the folder name. Returns the row entity
/// (for the caller to attach the toggle observer) and the chevron entity (so the
/// toggle can rotate it between the down/right orientations).
fn folder_header(commands: &mut Commands, name: &str) -> (Entity, Entity) {
    // Archivo (our default font) has no triangle glyphs, so the disclosure
    // marker is *drawn*: a small square with two adjacent borders forms a
    // corner ("⌐"), and rotating it points the corner like a chevron. Starts
    // expanded → pointing down.
    let chevron = commands
        .spawn((
            Node {
                width: Val::Px(6.0),
                height: Val::Px(6.0),
                border: UiRect {
                    top: Val::Px(1.5),
                    right: Val::Px(1.5),
                    ..default()
                },
                ..default()
            },
            BorderColor {
                top: CHEVRON,
                right: CHEVRON,
                bottom: Color::NONE,
                left: Color::NONE,
            },
            UiTransform {
                rotation: chevron_down(),
                ..default()
            },
        ))
        .id();

    let label = commands
        .spawn((
            Text::new(name),
            TextFont {
                font_size: TREE_FONT,
                ..default()
            },
            TextColor(TREE_TEXT),
        ))
        .id();

    let header = commands
        .spawn((
            Button,
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(4.0), Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(ROW_IDLE),
        ))
        .id();
    commands.entity(header).add_children(&[chevron, label]);
    add_hover(commands, header);

    (header, chevron)
}

/// An indented leaf row. Clicking it clears the canvas and renders the story
/// (blank for now — just its title).
fn leaf_row(commands: &mut Commands, name: &str) -> Entity {
    let label = commands
        .spawn((
            Text::new(name),
            TextFont {
                font_size: TREE_FONT,
                ..default()
            },
            TextColor(TREE_TEXT),
        ))
        .id();

    let row = commands
        .spawn((
            Button,
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                padding: UiRect {
                    left: Val::Px(26.0),
                    right: Val::Px(4.0),
                    top: Val::Px(3.0),
                    bottom: Val::Px(3.0),
                },
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(ROW_IDLE),
        ))
        .id();
    commands.entity(row).add_children(&[label]);
    add_hover(commands, row);

    let story = name.to_string();
    commands.entity(row).observe(
        move |_: On<Pointer<Click>>,
              mut commands: Commands,
              canvas: Single<Entity, With<StoryCanvas>>| {
            let canvas = *canvas;
            commands.entity(canvas).despawn_related::<Children>();
            let title = commands
                .spawn((
                    Text::new(story.clone()),
                    TextFont {
                        font_size: 32.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ))
                .id();
            commands.entity(canvas).add_children(&[title]);
        },
    );

    row
}

/// Adds IDE-style hover highlighting to a tree row. The row entity is captured
/// so the highlight applies to the row itself, not whichever child the pointer
/// happened to be over.
fn add_hover(commands: &mut Commands, row: Entity) {
    commands
        .entity(row)
        .observe(
            move |_: On<Pointer<Over>>, mut q: Query<&mut BackgroundColor>| {
                if let Ok(mut bg) = q.get_mut(row) {
                    bg.0 = ROW_HOVER;
                }
            },
        )
        .observe(
            move |_: On<Pointer<Out>>, mut q: Query<&mut BackgroundColor>| {
                if let Ok(mut bg) = q.get_mut(row) {
                    bg.0 = ROW_IDLE;
                }
            },
        );
}
