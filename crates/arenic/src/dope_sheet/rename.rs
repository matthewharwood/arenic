//! Dope sheet: **right-click a layer row → context menu** (ARE-47). Two items:
//! `Rename` (the first) and `Delete`. `Rename` opens an inline field over the
//! row — `Enter` commits, `Esc` cancels, an empty/whitespace name is rejected
//! (the prior name stays); `Delete` removes the layer. Both run through the
//! reusable [`crate::layer_edit::LayerEditor`] (a SAVE event): the commit edits
//! the DRAFT stack only — the alias is on `Layer.name`, never `LayerId`,
//! bindings, order, recordings, or fold/replay — and `layer_dirty` already
//! counts it, so the dirty `●` lights, `W` persists to the next
//! `layers.vNNNN.ron`, and a reload shows it. Author-feature only.

use arenic_game::arena::Arena;
use arenic_game::default_font::MonoFont;
use arenic_game::layer::{ArenaStack, LayerId};
use bevy::input::ButtonState;
use bevy::input::common_conditions::input_just_pressed;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;

use super::edit::RowFocus;
use super::{CHANNEL_W, HEADER_H, ROW_H, RULER_H, SheetBody};
use crate::intro_scene::CurrentArena;
use crate::layer_edit::LayerEditor;
use crate::modal::no_modal;
use crate::states::{AppState, RenameFocus};

/// The in-flight rename, if any. The buffer is held HERE (not on `Layer.name`)
/// so `Esc` cancels without ever touching the stack.
#[derive(Resource, Default)]
enum Rename {
    #[default]
    Idle,
    Editing {
        layer: LayerId,
        buffer: String,
    },
}

/// The right-click context menu panel.
#[derive(Component)]
#[component(immutable)]
struct ContextMenu;

/// A context-menu item — the layer it acts on + which action it runs.
#[derive(Component, Clone, Copy)]
#[component(immutable)]
struct MenuItem {
    layer: LayerId,
    action: MenuAction,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MenuAction {
    Rename,
    Delete,
}

/// The inline rename field (a bordered text node over the layer's row).
#[derive(Component)]
#[component(immutable)]
struct RenameField;

/// A row's top edge within [`SheetBody`] from its position in the displayed
/// stack (top-of-stack first, mirroring [`super::rebuild_sheet`]).
fn row_top(stack: &ArenaStack, layer: LayerId) -> Option<f32> {
    stack
        .stack
        .layers
        .iter()
        .rev()
        .position(|candidate| candidate.id == layer)
        .map(|index| HEADER_H + RULER_H + index as f32 * ROW_H)
}

/// Right-click a layer row → open the one-item context menu anchored to that
/// row. Gated by `no_modal`, so it never fires while editing (RenameFocus folds
/// in) or over another modal.
fn open_menu(
    mut commands: Commands,
    mono: Res<MonoFont>,
    current: Res<CurrentArena>,
    rows: Query<(&RowFocus, &Interaction)>,
    stacks: Query<(&Arena, &ArenaStack)>,
    body: Single<Entity, With<SheetBody>>,
    menus: Query<Entity, With<ContextMenu>>,
) {
    let Some(layer) = rows
        .iter()
        .find(|(_, interaction)| **interaction != Interaction::None)
        .map(|(row, _)| row.0)
    else {
        return; // right-clicked off any row
    };
    let Some((_, stack)) = stacks.iter().find(|(arena, _)| arena.index() == current.0) else {
        return;
    };
    let Some(top) = row_top(stack, layer) else {
        return;
    };
    let theme = Arena::from_index(current.0)
        .expect("invariant: CurrentArena is a valid arena index")
        .theme()
        .palette();
    // Replace any open menu.
    for menu in &menus {
        commands.entity(menu).despawn();
    }
    commands
        .spawn((
            ContextMenu,
            ChildOf(*body),
            GlobalZIndex(11),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(8.0),
                top: Val::Px(top + ROW_H),
                width: Val::Px(120.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(theme.surface_2()),
            BorderColor::all(theme.border_strong()),
        ))
        .with_children(|menu| {
            for (action, label, color) in [
                (MenuAction::Rename, "Rename", theme.text_1()),
                (MenuAction::Delete, "Delete", theme.error),
            ] {
                menu.spawn((
                    MenuItem { layer, action },
                    Button,
                    Interaction::default(),
                    Node {
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
                        width: Val::Percent(100.0),
                        ..default()
                    },
                    Text::new(label),
                    TextFont {
                        font: mono.0.clone(),
                        font_size: 11.0,
                        ..default()
                    },
                    TextColor(color),
                ));
            }
        });
}

/// Runs the clicked menu item: `Rename` starts the inline edit (seeded with the
/// live name + keyboard focus); `Delete` removes the layer through the reusable
/// [`LayerEditor`] (a save event).
fn menu_choose(
    mut commands: Commands,
    items: Query<(&MenuItem, &Interaction), Changed<Interaction>>,
    menus: Query<Entity, With<ContextMenu>>,
    mut rename: ResMut<Rename>,
    current: Res<CurrentArena>,
    mut editor: LayerEditor,
) {
    for (item, interaction) in &items {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match item.action {
            MenuAction::Rename => {
                let name = editor.layer_name(current.0, item.layer).unwrap_or_default();
                *rename = Rename::Editing {
                    layer: item.layer,
                    buffer: name,
                };
                commands.insert_resource(RenameFocus);
            }
            MenuAction::Delete => editor.delete(current.0, item.layer),
        }
        for menu in &menus {
            commands.entity(menu).despawn();
        }
    }
}

/// Dismisses the menu on `Esc` or a click elsewhere (the `Rename` click is
/// handled by [`menu_choose`] first, which leaves nothing to dismiss).
fn dismiss_menu(
    mut commands: Commands,
    rename: Res<Rename>,
    menus: Query<Entity, With<ContextMenu>>,
) {
    if matches!(*rename, Rename::Editing { .. }) {
        return;
    }
    for menu in &menus {
        commands.entity(menu).despawn();
    }
}

/// The rename field's keyboard grammar. Always drains [`KeyboardInput`] (so an
/// edit never replays history), but only mutates the buffer while editing.
fn rename_input(
    mut messages: MessageReader<KeyboardInput>,
    mut commands: Commands,
    mut rename: ResMut<Rename>,
    current: Res<CurrentArena>,
    mut editor: LayerEditor,
) {
    let pressed: Vec<Key> = messages
        .read()
        .filter(|input| input.state == ButtonState::Pressed)
        .map(|input| input.logical_key.clone())
        .collect();
    let Rename::Editing { layer, buffer } = &mut *rename else {
        return; // drained the buffer above; nothing to type into
    };
    let mut finish: Option<bool> = None;
    for key in &pressed {
        match key {
            Key::Escape => finish = Some(false),
            Key::Enter => finish = Some(true),
            Key::Backspace => {
                buffer.pop();
            }
            Key::Space => buffer.push(' '),
            Key::Character(text) => buffer.push_str(text.as_str()),
            _ => {}
        }
    }
    let Some(commit) = finish else {
        return;
    };
    if commit {
        let trimmed = buffer.trim();
        // Empty / whitespace-only is rejected — the prior name stays. A real
        // name commits through the reusable CRUD (the SAVE event).
        if !trimmed.is_empty() {
            editor.rename(current.0, *layer, trimmed);
        }
    }
    *rename = Rename::Idle;
    commands.remove_resource::<RenameFocus>();
}

/// Spawns / updates / despawns the inline field to mirror the [`Rename`] state.
fn sync_field(
    mut commands: Commands,
    mono: Res<MonoFont>,
    current: Res<CurrentArena>,
    rename: Res<Rename>,
    body: Single<Entity, With<SheetBody>>,
    stacks: Query<(&Arena, &ArenaStack)>,
    field: Option<Single<(Entity, &mut Text), With<RenameField>>>,
) {
    let Rename::Editing { layer, buffer } = &*rename else {
        if let Some(field) = field {
            commands.entity(field.into_inner().0).despawn();
        }
        return;
    };
    let line = format!("{buffer}_");
    if let Some(field) = field {
        let (_, mut text) = field.into_inner();
        if text.0 != line {
            text.0 = line;
        }
        return;
    }
    let top = stacks
        .iter()
        .find(|(arena, _)| arena.index() == current.0)
        .and_then(|(_, stack)| row_top(stack, *layer))
        .unwrap_or(HEADER_H + RULER_H);
    let theme = Arena::from_index(current.0)
        .expect("invariant: CurrentArena is a valid arena index")
        .theme()
        .palette();
    commands.spawn((
        RenameField,
        ChildOf(*body),
        GlobalZIndex(11),
        Text::new(line),
        TextFont {
            font: mono.0.clone(),
            font_size: 11.0,
            ..default()
        },
        TextColor(theme.text_1()),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(6.0),
            top: Val::Px(top),
            width: Val::Px(CHANNEL_W - 12.0),
            height: Val::Px(ROW_H - 2.0),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(3.0)),
            align_items: AlignItems::Center,
            padding: UiRect::horizontal(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(theme.surface_3()),
        BorderColor::all(theme.warning),
    ));
}

/// Wires the rename grammar into the dope-sheet plugin.
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Rename>().add_systems(
        Update,
        (
            open_menu.run_if(input_just_pressed(MouseButton::Right).and(no_modal)),
            menu_choose,
            dismiss_menu.run_if(
                input_just_pressed(MouseButton::Left).or(input_just_pressed(KeyCode::Escape)),
            ),
            rename_input,
            sync_field,
        )
            .run_if(in_state(AppState::Intro)),
    );
}
