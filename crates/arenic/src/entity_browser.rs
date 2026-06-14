//! The **entity browser** — the keyboard-first spawn palette
//! (`_docs/AUTHORING_UI.md` §3), built to the trees.software bar:
//!
//! - **Palette as data**: [`PALETTE`] is the amendable toolbar — path-first
//!   rows (`enemies/minions/token`); directories are derived from path
//!   segments, never stored. Adding an enemy type is one table row.
//! - **Virtual scrolling**: a fixed pool of row nodes; scrolling REBINDS text
//!   and tints — it never spawns or despawns rows. The window only recomputes
//!   when scroll escapes the overscan (the @pierre/trees early-out), so most
//!   scroll frames do nothing at all. Holds thousands of rows.
//! - **Keyboard grammar**: `E` focuses the panel (every world hotkey yields
//!   via the `no_modal` fold-in); `↓`/`↑` move, `→`/`←` expand/collapse,
//!   `Home`/`End` jump, ANY letter opens the type-to-filter seeded with it
//!   (normalized substring-on-path, expand-matches), `Enter` adds the focused
//!   palette entry as a new layer at the playhead, `Esc` closes the filter
//!   (restoring the pre-filter expansion snapshot) then the panel.
//! - **Drag to the board**: drag a palette row onto an arena tile to place a
//!   minion layer at that tile + the current tick; dropping off-board shakes
//!   the panel and places nothing.
//!
//! Author-feature only; nothing here ships.

use arenic_game::arena::Arena;
use arenic_game::default_font::MonoFont;
use arenic_game::grid::{ARENA_H, ARENA_W, ARENAS, GRID_H, GRID_W, TILE};
use bevy::color::Alpha;
use bevy::input::common_conditions::input_just_pressed;
use bevy::platform::collections::HashSet;
use bevy::prelude::*;

use crate::hud::TOP_BAR_PX;
use crate::intro_scene::CurrentArena;
use crate::layer_edit::LayerEditor;
use crate::states::{AppState, BrowserFocus};

const PANEL_W: f32 = 230.0;
const ROW_H: f32 = 24.0;
const PANEL_TOP: f32 = TOP_BAR_PX + 10.0;
const PANEL_BOTTOM: f32 = 196.0; // clears the dope sheet
const HEADER_H: f32 = 40.0;
/// Rows beyond the viewport mounted on each side (the trees overscan).
const OVERSCAN: usize = 10;
/// Pool size: a 720px window leaves ~450px of rows; overscan both sides.
const POOL: usize = 19 + 2 * OVERSCAN;

/// What activating a palette entry does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum AddKind {
    /// A new minion layer (spawn at the drop tile / arena centre).
    MinionToken,
    /// A new empty tile layer on top of the stack.
    TileLayer,
}

/// One spawnable thing — the AMENDABLE toolbar row. The `path` is the
/// identity: segments are the tree. New enemy archetypes land here (a
/// hot-reloaded RON manifest is the planned upgrade once archetypes multiply).
struct PaletteEntry {
    path: &'static str,
    label: &'static str,
    add: AddKind,
}

const PALETTE: &[PaletteEntry] = &[
    PaletteEntry {
        path: "enemies/minions/token",
        label: "Token Minion",
        add: AddKind::MinionToken,
    },
    PaletteEntry {
        path: "tiles/choreography-layer",
        label: "Tile Layer",
        add: AddKind::TileLayer,
    },
];

/// One row of the flat projection the panel renders.
#[derive(Clone, Debug, PartialEq)]
struct BrowserRow {
    path: String,
    label: String,
    depth: usize,
    kind: RowKind,
}

#[derive(Clone, Debug, PartialEq)]
enum RowKind {
    Dir { expanded: bool },
    Entry { palette: usize },
}

/// The browser's working state. The flat `rows` projection rebuilds only when
/// `expanded`/`filter` change — never on scroll or focus moves.
#[derive(Resource)]
pub(crate) struct BrowserState {
    rows: Vec<BrowserRow>,
    focused: usize,
    expanded: HashSet<String>,
    saved_expanded: Option<HashSet<String>>,
    filter: Option<String>,
    scroll_row: usize,
    /// A palette index being dragged toward the board.
    dragging: Option<usize>,
    /// Seconds left of the reject shake.
    shake: f32,
}

impl Default for BrowserState {
    fn default() -> Self {
        // Top-level groups start expanded — orientation beats minimalism.
        let expanded = PALETTE
            .iter()
            .filter_map(|entry| entry.path.split('/').next())
            .map(str::to_owned)
            .collect();
        Self {
            rows: Vec::new(),
            focused: 0,
            expanded,
            saved_expanded: None,
            filter: None,
            scroll_row: 0,
            dragging: None,
            shake: 0.0,
        }
    }
}

/// The pure window function — the virtualization heart, unit-tested: which
/// row range should be mounted for `scroll_row`, given `len` rows and the
/// previously mounted `current` range (early-out when still inside it).
fn window_range(scroll_row: usize, len: usize, current: Option<(usize, usize)>) -> (usize, usize) {
    let visible = POOL.strict_sub(2 * OVERSCAN);
    if let Some((start, end)) = current {
        // Still fully inside the mounted window (with overscan slack)?
        let inside = scroll_row >= start.strict_add(if start == 0 { 0 } else { OVERSCAN / 2 })
            && scroll_row.strict_add(visible) <= end.saturating_sub(OVERSCAN / 2)
            && end <= len;
        if inside {
            return (start, end);
        }
    }
    let start = scroll_row.saturating_sub(OVERSCAN);
    let end = start.strict_add(POOL).min(len);
    (start, end)
}

/// Marker components for the panel skeleton + pooled rows. `BrowserPanel`
/// is the host-agnostic panel ROOT the pop-out module re-parents.
#[derive(Component)]
#[component(immutable)]
pub(crate) struct BrowserPanel;
#[derive(Component)]
#[component(immutable)]
struct BrowserHeader;
#[derive(Component)]
#[component(immutable)]
struct PoolRow(usize);
#[derive(Component)]
#[component(immutable)]
struct PoolRowLabel(usize);

/// The mounted range, kept across frames for the early-out.
#[derive(Resource, Default)]
struct Mounted(Option<(usize, usize)>);

fn setup_browser(mut commands: Commands, mono: Res<MonoFont>, current: Res<CurrentArena>) {
    let theme = Arena::from_index(current.0)
        .expect("invariant: CurrentArena is a valid arena index")
        .theme()
        .palette();
    commands
        .spawn((
            DespawnOnExit(AppState::Intro),
            BrowserPanel,
            GlobalZIndex(7),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(PANEL_TOP),
                bottom: Val::Px(PANEL_BOTTOM),
                left: Val::Px(0.0),
                width: Val::Px(PANEL_W),
                flex_direction: FlexDirection::Column,
                display: Display::None, // closed until `E`
                ..default()
            },
            BackgroundColor(theme.surface_1().with_alpha(0.94)),
        ))
        .with_children(|panel| {
            panel.spawn((
                BrowserHeader,
                Text::new("ENTITIES"),
                TextFont {
                    font: mono.0.clone(),
                    font_size: 11.0,
                    ..default()
                },
                TextColor(theme.text_muted()),
                Node {
                    height: Val::Px(HEADER_H),
                    padding: UiRect::all(Val::Px(8.0)),
                    ..default()
                },
            ));
            // The fixed row pool — rebound, never respawned.
            for slot in 0..POOL {
                panel
                    .spawn((
                        PoolRow(slot),
                        Node {
                            height: Val::Px(ROW_H),
                            width: Val::Percent(100.0),
                            align_items: AlignItems::Center,
                            display: Display::None,
                            ..default()
                        },
                        BackgroundColor(Color::NONE),
                        Interaction::default(),
                    ))
                    .with_children(|row| {
                        row.spawn((
                            PoolRowLabel(slot),
                            Text::new(""),
                            TextFont {
                                font_size: 11.0,
                                ..default()
                            },
                            TextColor(theme.text_1()),
                        ));
                    });
            }
        });
}

/// Rebuilds the flat projection from the palette + expansion + filter. The
/// trees rules: filter = normalized substring on the FULL path, expand-matches
/// mode (every match's ancestors render expanded).
fn rebuild_projection(state: &mut BrowserState) {
    let mut rows: Vec<BrowserRow> = Vec::new();
    let filter = state.filter.as_deref().map(str::to_lowercase);
    let mut seen_dirs: HashSet<String> = HashSet::default();
    for (index, entry) in PALETTE.iter().enumerate() {
        if let Some(filter) = &filter
            && !entry.path.to_lowercase().contains(filter.as_str())
        {
            continue;
        }
        let segments: Vec<&str> = entry.path.split('/').collect();
        let mut prefix = String::new();
        let mut visible = true;
        for (depth, segment) in segments.iter().enumerate() {
            let leaf = depth == segments.len().strict_sub(1);
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(segment);
            if leaf {
                if visible {
                    rows.push(BrowserRow {
                        path: entry.path.to_owned(),
                        label: entry.label.to_owned(),
                        depth,
                        kind: RowKind::Entry { palette: index },
                    });
                }
            } else {
                // Filtered views render matches' ancestors as expanded.
                let expanded = filter.is_some() || state.expanded.contains(&prefix);
                if visible && seen_dirs.insert(prefix.clone()) {
                    rows.push(BrowserRow {
                        path: prefix.clone(),
                        label: (*segment).to_owned(),
                        depth,
                        kind: RowKind::Dir { expanded },
                    });
                }
                visible = visible && expanded;
            }
        }
    }
    state.rows = rows;
    state.focused = state.focused.min(state.rows.len().saturating_sub(1));
}

/// Rebinds the pooled rows to the current window — text, indent, tints. Runs
/// only when the state changed; the window itself early-outs when scroll is
/// still inside the mounted range.
fn rebind_pool(
    state: Res<BrowserState>,
    current: Res<CurrentArena>,
    mut mounted: ResMut<Mounted>,
    mut rows: Query<(&PoolRow, &mut Node, &mut BackgroundColor)>,
    mut labels: Query<(&PoolRowLabel, &mut Text, &mut TextColor)>,
) {
    let theme = Arena::from_index(current.0)
        .expect("invariant: CurrentArena is a valid arena index")
        .theme()
        .palette();
    let (start, end) = window_range(state.scroll_row, state.rows.len(), mounted.0);
    mounted.0 = Some((start, end));
    for (slot, mut node, mut background) in &mut rows {
        let index = start.strict_add(slot.0);
        let visible = index < end && index < state.rows.len();
        let want = if visible {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != want {
            node.display = want;
        }
        if !visible {
            continue;
        }
        let row = &state.rows[index];
        node.padding.left = Val::Px(8.0 + row.depth as f32 * 14.0);
        let focused = index == state.focused;
        let tint = if focused {
            theme.surface_2().with_alpha(0.9)
        } else {
            Color::NONE
        };
        if background.0 != tint {
            background.0 = tint;
        }
    }
    for (slot, mut text, mut color) in &mut labels {
        let index = start.strict_add(slot.0);
        if index >= end || index >= state.rows.len() {
            continue;
        }
        let row = &state.rows[index];
        let line = match &row.kind {
            RowKind::Dir { expanded } => {
                format!("{} {}", if *expanded { "▾" } else { "▸" }, row.label)
            }
            RowKind::Entry { .. } => row.label.clone(),
        };
        if text.0 != line {
            text.0 = line;
        }
        let want = if matches!(row.kind, RowKind::Dir { .. }) {
            theme.text_muted()
        } else {
            theme.text_1()
        };
        if color.0 != want {
            color.0 = want;
        }
    }
}

/// `E` — open + focus the browser (world keys yield via the `no_modal`
/// fold-in). Inside, `Esc` closes filter → panel.
fn open_browser(
    mut commands: Commands,
    mut state: ResMut<BrowserState>,
    mut panel: Single<&mut Node, With<BrowserPanel>>,
) {
    commands.insert_resource(BrowserFocus);
    panel.display = Display::Flex;
    rebuild_projection(&mut state);
}

fn close_browser(commands: &mut Commands, state: &mut BrowserState, panel: &mut Node) {
    commands.remove_resource::<BrowserFocus>();
    panel.display = Display::None;
    state.filter = None;
    if let Some(saved) = state.saved_expanded.take() {
        state.expanded = saved;
    }
}

/// The whole keyboard grammar while the browser is focused.
fn browser_keys(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<BrowserState>,
    mut panel: Single<&mut Node, With<BrowserPanel>>,
    mut header: Single<&mut Text, With<BrowserHeader>>,
    current: Res<CurrentArena>,
    mut editor: LayerEditor,
) {
    // Esc: close the filter first, then the panel (layered, trees-style).
    if keys.just_pressed(KeyCode::Escape) {
        if state.filter.is_some() {
            state.filter = None;
            if let Some(saved) = state.saved_expanded.take() {
                state.expanded = saved;
            }
            rebuild_projection(&mut state);
        } else {
            close_browser(&mut commands, &mut state, &mut panel);
        }
        return;
    }
    // Navigation.
    if keys.just_pressed(KeyCode::ArrowDown) {
        state.focused = state
            .focused
            .strict_add(1)
            .min(state.rows.len().saturating_sub(1));
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        state.focused = state.focused.saturating_sub(1);
    }
    if keys.just_pressed(KeyCode::Home) {
        state.focused = 0;
    }
    if keys.just_pressed(KeyCode::End) {
        state.focused = state.rows.len().saturating_sub(1);
    }
    // Keep the focused row in view (scroll the minimum distance).
    let visible = POOL.strict_sub(2 * OVERSCAN);
    if state.focused < state.scroll_row {
        state.scroll_row = state.focused;
    } else if state.focused >= state.scroll_row.strict_add(visible) {
        state.scroll_row = state.focused.strict_sub(visible).strict_add(1);
    }
    // Expand / collapse.
    let focused_row = state.rows.get(state.focused).cloned();
    if let Some(row) = &focused_row {
        if keys.just_pressed(KeyCode::ArrowRight)
            && let RowKind::Dir { expanded } = row.kind
        {
            if expanded {
                state.focused = state
                    .focused
                    .strict_add(1)
                    .min(state.rows.len().saturating_sub(1));
            } else {
                state.expanded.insert(row.path.clone());
                rebuild_projection(&mut state);
            }
        }
        if keys.just_pressed(KeyCode::ArrowLeft) {
            match row.kind {
                RowKind::Dir { expanded: true } => {
                    state.expanded.remove(&row.path);
                    rebuild_projection(&mut state);
                }
                _ => {
                    // Jump to the parent directory row.
                    if let Some(parent) = row.path.rsplit_once('/').map(|(p, _)| p.to_owned())
                        && let Some(at) = state.rows.iter().position(|r| r.path == parent)
                    {
                        state.focused = at;
                    }
                }
            }
        }
        // Enter: activate a palette entry at the playhead.
        if keys.just_pressed(KeyCode::Enter)
            && let RowKind::Entry { palette } = row.kind
        {
            add_entry(palette, None, current.0, &mut editor);
        }
    }
    // Type-to-filter: any letter/digit seeds or extends it; Backspace edits.
    let typed: String = keys
        .get_just_pressed()
        .filter_map(|key| key_char(*key))
        .collect();
    if !typed.is_empty() {
        if state.filter.is_none() {
            state.saved_expanded = Some(state.expanded.clone());
        }
        let filter = state.filter.get_or_insert_default();
        filter.push_str(&typed);
        state.focused = 0;
        rebuild_projection(&mut state);
    }
    if keys.just_pressed(KeyCode::Backspace)
        && let Some(filter) = &mut state.filter
    {
        filter.pop();
        if filter.is_empty() {
            state.filter = None;
            if let Some(saved) = state.saved_expanded.take() {
                state.expanded = saved;
            }
        }
        rebuild_projection(&mut state);
    }
    // Header doubles as the filter readout.
    let line = match &state.filter {
        Some(filter) => format!("ENTITIES / {filter}_"),
        None => "ENTITIES".to_string(),
    };
    if header.0 != line {
        header.0 = line;
    }
}

/// Adds the palette entry as a new layer on the focused arena: minions spawn
/// at `tile` (or the arena centre) at the playhead tick; tile layers go on
/// top of the stack.
fn add_entry(palette: usize, tile: Option<IVec2>, arena: usize, editor: &mut LayerEditor) {
    let name = Arena::from_index(arena).map_or("?", |arena| arena.name());
    match PALETTE[palette].add {
        AddKind::MinionToken => {
            let tile = tile.unwrap_or(IVec2::new(30, 12));
            editor.create_minion(arena, tile);
            info!("{name}: minion layer placed at {tile}");
        }
        AddKind::TileLayer => {
            editor.create_tile_layer(arena);
            info!("{name}: tile layer added");
        }
    }
}

/// The character a key contributes to the filter (letters + digits — the
/// trees "any letter opens search" rule).
fn key_char(key: KeyCode) -> Option<char> {
    use KeyCode::*;
    Some(match key {
        KeyA => 'a',
        KeyB => 'b',
        KeyC => 'c',
        KeyD => 'd',
        KeyE => 'e',
        KeyF => 'f',
        KeyG => 'g',
        KeyH => 'h',
        KeyI => 'i',
        KeyJ => 'j',
        KeyK => 'k',
        KeyL => 'l',
        KeyM => 'm',
        KeyN => 'n',
        KeyO => 'o',
        KeyP => 'p',
        KeyQ => 'q',
        KeyR => 'r',
        KeyS => 's',
        KeyT => 't',
        KeyU => 'u',
        KeyV => 'v',
        KeyW => 'w',
        KeyX => 'x',
        KeyY => 'y',
        KeyZ => 'z',
        Digit0 => '0',
        Digit1 => '1',
        Digit2 => '2',
        Digit3 => '3',
        Digit4 => '4',
        Digit5 => '5',
        Digit6 => '6',
        Digit7 => '7',
        Digit8 => '8',
        Digit9 => '9',
        Minus => '-',
        Slash => '/',
        _ => return None,
    })
}

/// Mouse: click focuses/toggles a row; drag starts a board placement.
fn row_mouse(
    mut state: ResMut<BrowserState>,
    mounted: Res<Mounted>,
    rows: Query<(&PoolRow, &Interaction), Changed<Interaction>>,
) {
    let Some((start, _)) = mounted.0 else {
        return;
    };
    for (slot, interaction) in &rows {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let index = start.strict_add(slot.0);
        if index >= state.rows.len() {
            continue;
        }
        state.focused = index;
        match state.rows[index].kind.clone() {
            RowKind::Dir { expanded } => {
                let path = state.rows[index].path.clone();
                if expanded {
                    state.expanded.remove(&path);
                } else {
                    state.expanded.insert(path);
                }
                rebuild_projection(&mut state);
            }
            RowKind::Entry { palette } => {
                // Pressing an entry arms a drag; releasing over the board
                // places it (drop_on_board), releasing elsewhere shakes.
                state.dragging = Some(palette);
            }
        }
    }
}

/// On release: if a drag was armed, raycast the cursor onto the board plane —
/// a hit places the entry at that tile + the playhead; a miss shakes.
fn drop_on_board(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    mut state: ResMut<BrowserState>,
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera3d>>,
    arena_roots: Query<(&Arena, &GlobalTransform)>,
    current: Res<CurrentArena>,
    mut editor: LayerEditor,
) {
    if !buttons.just_released(MouseButton::Left) {
        return;
    }
    let Some(palette) = state.dragging.take() else {
        return;
    };
    let placed = (|| {
        let cursor = window.cursor_position()?;
        let (camera, camera_transform) = *camera;
        let ray = camera.viewport_to_world(camera_transform, cursor).ok()?;
        // Intersect the board plane (world z = 0).
        let t = -ray.origin.z / ray.direction.z;
        if !t.is_finite() || t < 0.0 {
            return None;
        }
        let hit = ray.origin + *ray.direction * t;
        // Which arena cell is that? (columns +X, rows -Y, local tile grid.)
        let arena_col = (hit.x / ARENA_W).floor();
        let arena_row = (-hit.y / ARENA_H).floor();
        if !(0.0..3.0).contains(&arena_col) || !(0.0..3.0).contains(&arena_row) {
            return None;
        }
        let index = (arena_row as usize)
            .strict_mul(3)
            .strict_add(arena_col as usize);
        if index >= ARENAS {
            return None;
        }
        let local_x = hit.x - arena_col * ARENA_W;
        let local_y = hit.y + arena_row * ARENA_H;
        let col = (local_x / TILE).round() as i32;
        let row = (local_y / TILE).round() as i32;
        if !(0..GRID_W).contains(&col) || !(0..GRID_H).contains(&row) {
            return None;
        }
        let _ = arena_roots; // arena transforms are grid-derived; index math suffices
        Some((index, IVec2::new(col, row)))
    })();
    match placed {
        Some((arena_index, tile)) => {
            // Placement targets the arena under the cursor: refocus it + add
            // straight onto it (the editor takes the arena explicitly, so the
            // deferred CurrentArena swap doesn't matter).
            if arena_index != current.0 {
                commands.insert_resource(CurrentArena(arena_index));
            }
            add_entry(palette, Some(tile), arena_index, &mut editor);
        }
        None => {
            state.shake = 0.25;
            warn!("drop outside the board — nothing placed");
        }
    }
}

/// The reject shake: wiggle the panel for a quarter second.
fn shake_panel(
    time: Res<Time>,
    mut state: ResMut<BrowserState>,
    mut panel: Single<&mut Node, With<BrowserPanel>>,
) {
    if state.shake <= 0.0 {
        if panel.left != Val::Px(0.0) {
            panel.left = Val::Px(0.0);
        }
        return;
    }
    state.shake = (state.shake - time.delta_secs()).max(0.0);
    let phase = (state.shake * 60.0).sin() * 4.0;
    panel.left = Val::Px(phase);
}

fn browser_focused(focus: Option<Res<BrowserFocus>>) -> bool {
    focus.is_some()
}

/// The entity browser. Registered by the author plugin.
pub(crate) struct EntityBrowserPlugin;

impl Plugin for EntityBrowserPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BrowserState>()
            .init_resource::<Mounted>()
            .add_systems(OnEnter(AppState::Intro), setup_browser)
            .add_systems(
                Update,
                (
                    // `E` opens — only while the world owns the keys.
                    open_browser.run_if(
                        input_just_pressed(KeyCode::KeyE)
                            .and(crate::modal::no_modal)
                            .and(crate::recording::is_idle)
                            .and(crate::states::not_tile_editing),
                    ),
                    (browser_keys, row_mouse, drop_on_board).run_if(browser_focused),
                    rebind_pool.run_if(
                        resource_changed::<BrowserState>.or(resource_changed::<CurrentArena>),
                    ),
                    shake_panel,
                )
                    .run_if(in_state(AppState::Intro)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The early-out: scrolling inside the mounted window changes nothing —
    /// the zero-allocation invariant the virtualization rests on. A 1,000-row
    /// palette never mounts more than POOL rows.
    #[test]
    fn window_early_outs_inside_and_caps_at_pool() {
        let len = 1_000;
        let first = window_range(0, len, None);
        assert_eq!(first, (0, POOL));
        // A small scroll stays inside the mounted window: identical range.
        assert_eq!(window_range(2, len, Some(first)), first);
        // Escaping the overscan recomputes around the new scroll row.
        let far = window_range(500, len, Some(first));
        assert_eq!(far, (490, 490 + POOL));
        assert!(far.1 - far.0 <= POOL);
        // The tail clamps to len.
        let tail = window_range(995, len, Some(far));
        assert_eq!(tail.1, len);
    }
}
