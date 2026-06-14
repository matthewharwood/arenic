//! **Author-mode window docking** for the dope sheet (`_docs/AUTHORING_UI.md`).
//!
//! In the plain game the sheet would overlay the bottom of the arena. Author
//! mode instead GROWS the OS window downward so the sheet sits in a strip
//! *below* the arena — nothing the arena draws is ever covered.
//!
//! Two cameras, disjoint viewports, one primary window:
//! - the arena's [`Camera3d`] is pinned to the top band (the design-resolution
//!   rect). Its render is a pure function of camera pose + FOV, so clipping it
//!   to the band is pixel-identical to the un-grown window. It also claims
//!   [`IsDefaultUiCamera`] so the HUD/modals/browser keep targeting it — a
//!   second primary-window camera would otherwise outrank it by `order`.
//! - a [`SheetCamera`] (`Camera2d`) owns the bottom strip and the docked sheet
//!   [targets][`UiTargetCamera`] it.
//!
//! Everything is sized off the live window: the window is grown **logically**
//! (via [`WindowResolution::set`], which respects the monitor scale factor, so
//! it stays normal-sized + crisp at any DPI), and the camera viewports are
//! derived in **physical** px from `scale_factor()` — correct on Retina and at
//! either [`ResolutionMode`]. The window height tracks the sheet's *effective*
//! height: `design + PANEL_H·s` while docked & expanded, `design` when
//! collapsed (`` ` ``) or popped out (`F2`). Author-feature only; nothing ships.

use bevy::camera::Viewport;
use bevy::prelude::*;
use bevy::ui::{IsDefaultUiCamera, UiTargetCamera};
use bevy::window::PrimaryWindow;

use super::{PANEL_H, SheetBody};
use crate::settings::ResolutionMode;
use crate::states::AppState;

/// The `Camera2d` that renders the docked sheet strip at the window bottom.
/// [`sync_dock`] sizes its viewport and lights it.
#[derive(Component)]
#[component(immutable)]
pub(crate) struct SheetCamera;

/// Spawns the strip camera on Intro entry — inactive, with a placeholder
/// viewport that [`sync_dock`] replaces once it can read the live window.
fn setup_strip_camera(mut commands: Commands) {
    commands.spawn((
        DespawnOnExit(AppState::Intro),
        SheetCamera,
        Camera2d,
        Camera {
            // Above the arena's 0, below a pop-out window's 2.
            order: 1,
            viewport: Some(Viewport {
                physical_position: UVec2::ZERO,
                physical_size: UVec2::ONE,
                depth: 0.0..1.0,
            }),
            // Clears just the strip (scissored to the viewport) so the panel's
            // 92%-opaque surface reads over solid dark, not last frame's pixels.
            clear_color: ClearColorConfig::Custom(Color::srgb(0.02, 0.02, 0.03)),
            is_active: false,
            ..default()
        },
    ));
}

/// Reconciles, every frame, the window height + the two cameras against the
/// sheet's dock state and the active resolution mode. Idempotent: each write is
/// guarded, so the steady state costs nothing.
fn sync_dock(
    mode: Res<ResolutionMode>,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
    arena: Single<
        (Entity, &mut Camera, Has<IsDefaultUiCamera>),
        (With<Camera3d>, Without<SheetCamera>),
    >,
    strip: Single<(Entity, &mut Camera), (With<SheetCamera>, Without<Camera3d>)>,
    sheet: Option<Single<(Entity, &Node, Has<ChildOf>, Has<UiTargetCamera>), With<SheetBody>>>,
    mut commands: Commands,
) {
    let s = mode.scale();
    let design = mode.logical(); // logical (1280·s, 720·s)
    let panel_logical = PANEL_H * s;

    // Docked AND expanded? Popped → it carries a `ChildOf` (its pop-out root);
    // collapsed → `display: none`.
    let docked_expanded = sheet
        .as_deref()
        .is_some_and(|&(_, node, popped, _)| !popped && node.display != Display::None);

    // Physical framebuffer as it stands NOW (the backend trails the resolution
    // struct by a frame). We size cameras against this realized size and gate
    // the strip on it, so a viewport never exceeds the render target.
    let sf = window.resolution.scale_factor();
    let phys_w = window.resolution.physical_width();
    let cur_phys_h = window.resolution.physical_height();
    let band_phys_h = ((design.y * sf).round() as u32).min(cur_phys_h);

    // Grow the window LOGICALLY by the visible sheet height (0 when collapsed /
    // popped). Takes effect next frame.
    let want_logical_h = design.y + if docked_expanded { panel_logical } else { 0.0 };
    if (window.resolution.height() - want_logical_h).abs() > 0.5
        || (window.resolution.width() - design.x).abs() > 0.5
    {
        window.resolution.set(design.x, want_logical_h);
    }

    // Arena: pinned to the top band + the default UI camera (the strip camera
    // shares the primary window and would otherwise win the "highest order"
    // default-UI tiebreak and steal the HUD).
    let (arena_entity, mut arena_cam, arena_is_default) = arena.into_inner();
    set_viewport(&mut arena_cam, UVec2::ZERO, UVec2::new(phys_w, band_phys_h));
    if !arena_is_default {
        commands.entity(arena_entity).insert(IsDefaultUiCamera);
    }

    // Strip: the remainder below the band, lit only when the realized window is
    // already tall enough (so its bottom-anchored viewport stays in bounds).
    let (strip_entity, mut strip_cam) = strip.into_inner();
    let strip_active = docked_expanded && cur_phys_h > band_phys_h;
    if strip_active {
        set_viewport(
            &mut strip_cam,
            UVec2::new(0, band_phys_h),
            UVec2::new(phys_w, cur_phys_h - band_phys_h),
        );
    }
    if strip_cam.is_active != strip_active {
        strip_cam.is_active = strip_active;
    }

    // Point the docked sheet at the strip camera; a popped sheet inherits its
    // pop-out window's camera, so it must NOT carry an own target.
    if let Some(&(sheet_entity, _, popped, has_target)) = sheet.as_deref() {
        if !popped && !has_target {
            commands
                .entity(sheet_entity)
                .insert(UiTargetCamera(strip_entity));
        } else if popped && has_target {
            commands.entity(sheet_entity).remove::<UiTargetCamera>();
        }
    }
}

/// Set-if-changed for a camera viewport (`Viewport` has no `PartialEq`).
fn set_viewport(cam: &mut Camera, position: UVec2, size: UVec2) {
    let changed = match &cam.viewport {
        Some(vp) => vp.physical_position != position || vp.physical_size != size,
        None => true,
    };
    if changed {
        cam.viewport = Some(Viewport {
            physical_position: position,
            physical_size: size,
            depth: 0.0..1.0,
        });
    }
}

/// Leaving the Intro shrinks the window back to the bare design size so the
/// Title/Settings screens aren't left with an empty strip (the strip camera +
/// sheet die with the state via `DespawnOnExit`).
fn reset_window(mode: Res<ResolutionMode>, mut window: Single<&mut Window, With<PrimaryWindow>>) {
    let design = mode.logical();
    if (window.resolution.height() - design.y).abs() > 0.5 {
        window.resolution.set(design.x, design.y);
    }
}

/// Wires the docking systems into the dope-sheet plugin.
pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Intro), setup_strip_camera)
        .add_systems(OnExit(AppState::Intro), reset_window)
        .add_systems(Update, sync_dock.run_if(in_state(AppState::Intro)));
}
