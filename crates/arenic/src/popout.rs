//! **Pop-out panels** (`_docs/AUTHORING_UI.md` §5) — the dope sheet and the
//! entity browser can each leave the main window for their own OS window and
//! come back, panels intact.
//!
//! Mechanics (all 0.18-verified): a pop-out is a spawned [`Window`] entity
//! (the entity IS the window), a `Camera2d` whose [`RenderTarget`] points at
//! it, and a full-window UI root carrying [`UiTargetCamera`]. "Moving" a
//! panel between windows is nothing but re-parenting its host-agnostic root —
//! per-window `Interaction` and `ui_picking` events keep working.
//!
//! - **`F2`** pops / docks the dope sheet · **`F3`** the entity browser.
//! - OS-closing a pop-out window RE-DOCKS its panel (nothing is lost) — the
//!   [`WindowClosed`] Message drives the cleanup.
//! - Exiting the Intro tears everything down via `DespawnOnExit` (the panel's
//!   own exit-despawn no-ops if its pop-out host already took it down).
//! - wasm: the entire author feature (this module included) is compiled out;
//!   a second `Window` on the web is just a second canvas webgl2 can't drive.
//!   Panels stay docked there by construction.
//!
//! Deferred (tracked on the ticket): titlebar dock buttons and true mouse
//! drag-between-windows (needs window-under-cursor tracking); the hotkeys
//! cover the affordance.

use bevy::camera::RenderTarget;
use bevy::input::common_conditions::input_just_pressed;
use bevy::prelude::*;
use bevy::ui::UiTargetCamera;
use bevy::window::{WindowClosed, WindowRef, WindowResolution};

use crate::dope_sheet::SheetBody;
use crate::entity_browser::BrowserPanel;
use crate::modal::no_modal;
use crate::states::AppState;

/// One panel's pop-out host (window + camera + UI root), if popped.
struct PopOut {
    window: Entity,
    camera: Entity,
    root: Entity,
}

/// Which panels are currently popped.
#[derive(Resource, Default)]
struct PopOuts {
    sheet: Option<PopOut>,
    browser: Option<PopOut>,
}

/// Spawns a pop-out host and re-parents `panel` into it.
fn pop_out(commands: &mut Commands, panel: Entity, title: &str, size: (u32, u32)) -> PopOut {
    let window = commands
        .spawn((
            Window {
                title: title.to_owned(),
                resolution: WindowResolution::new(size.0, size.1),
                ..default()
            },
            DespawnOnExit(AppState::Intro),
        ))
        .id();
    let camera = commands
        .spawn((
            Camera2d,
            Camera {
                order: 2,
                ..default()
            },
            RenderTarget::Window(WindowRef::Entity(window)),
            DespawnOnExit(AppState::Intro),
        ))
        .id();
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            UiTargetCamera(camera),
            DespawnOnExit(AppState::Intro),
        ))
        .id();
    commands.entity(panel).insert(ChildOf(root));
    PopOut {
        window,
        camera,
        root,
    }
}

/// Re-docks `panel` into the main window and tears the host down.
/// `window_alive` is false when the OS already closed it (entity despawned).
fn re_dock(commands: &mut Commands, panel: Option<Entity>, host: PopOut, window_alive: bool) {
    if let Some(panel) = panel {
        commands.entity(panel).remove::<ChildOf>();
    }
    if window_alive {
        commands.entity(host.window).despawn();
    }
    commands.entity(host.camera).despawn();
    commands.entity(host.root).despawn();
}

/// `F2` — pop / dock the dope sheet.
fn toggle_sheet(
    mut commands: Commands,
    mut popouts: ResMut<PopOuts>,
    panel: Single<Entity, With<SheetBody>>,
) {
    match popouts.sheet.take() {
        Some(host) => re_dock(&mut commands, Some(*panel), host, true),
        None => popouts.sheet = Some(pop_out(&mut commands, *panel, "Dope Sheet", (1280, 220))),
    }
}

/// `F3` — pop / dock the entity browser (popping also opens it visually; the
/// keyboard focus model is unchanged — `E` still owns it).
fn toggle_browser(
    mut commands: Commands,
    mut popouts: ResMut<PopOuts>,
    panel: Single<(Entity, &mut Node), With<BrowserPanel>>,
) {
    let (panel, mut node) = panel.into_inner();
    match popouts.browser.take() {
        Some(host) => re_dock(&mut commands, Some(panel), host, true),
        None => {
            node.display = Display::Flex;
            popouts.browser = Some(pop_out(&mut commands, panel, "Entities", (320, 560)));
        }
    }
}

/// OS-closed pop-out windows re-dock their panel — closing the window never
/// loses authoring UI.
fn handle_closed(
    mut commands: Commands,
    mut closed: MessageReader<WindowClosed>,
    mut popouts: ResMut<PopOuts>,
    sheet: Query<Entity, With<SheetBody>>,
    browser: Query<Entity, With<BrowserPanel>>,
) {
    for message in closed.read() {
        if popouts
            .sheet
            .as_ref()
            .is_some_and(|host| host.window == message.window)
        {
            let host = popouts.sheet.take().expect("invariant: checked above");
            re_dock(&mut commands, sheet.single().ok(), host, false);
        }
        if popouts
            .browser
            .as_ref()
            .is_some_and(|host| host.window == message.window)
        {
            let host = popouts.browser.take().expect("invariant: checked above");
            re_dock(&mut commands, browser.single().ok(), host, false);
        }
    }
}

/// State exit: forget the hosts (their entities die via `DespawnOnExit`).
fn reset_on_exit(mut popouts: ResMut<PopOuts>) {
    popouts.sheet = None;
    popouts.browser = None;
}

/// Native pop-out windows for the authoring panels.
pub(crate) struct PopOutPlugin;

impl Plugin for PopOutPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PopOuts>()
            .add_systems(OnExit(AppState::Intro), reset_on_exit)
            .add_systems(
                Update,
                (
                    toggle_sheet.run_if(input_just_pressed(KeyCode::F2).and(no_modal)),
                    toggle_browser.run_if(input_just_pressed(KeyCode::F3).and(no_modal)),
                    handle_closed.run_if(on_message::<WindowClosed>),
                )
                    .run_if(in_state(AppState::Intro)),
            );
    }
}
