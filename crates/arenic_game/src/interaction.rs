//! Themeable interactive states for buttons: **hover**, **active** (pressed) and
//! **focus**.
//!
//! Bevy maintains an [`Interaction`] on every `Button`, but does nothing visual
//! with it. Attach an [`Interactive`] (with an [`Outline`]) to a button and
//! [`InteractionPlugin`] drives the rest from theme tokens, so the states work
//! identically across every theme:
//!
//! - **hover** / **active** swap the background to colours you precompute from
//!   [`Theme::interactions`](crate::theme::Theme::interactions) (the source's
//!   `--hover-tint` / `--press-tint`);
//! - **focus** draws an outline in the theme's `--focus-ring`. Focus follows
//!   clicks, and <kbd>Tab</kbd> / <kbd>Shift+Tab</kbd> cycle between controls;
//! - **pointer cursor**: while any [`Interactive`] control is hovered (or
//!   pressed) the OS cursor becomes a pointer (hand) — the standard "this is
//!   clickable" affordance — and reverts to the default arrow otherwise;
//! - an optional neobrutalist hard shadow lifts on hover and collapses on press.

use bevy::prelude::*;
use bevy::ui::Interaction;
use bevy::window::{CursorIcon, PrimaryWindow, SystemCursorIcon};

/// The colours (and optional shadow) an [`InteractionPlugin`] applies to a
/// button across its states. Spawn it alongside `Button`, `BackgroundColor` and
/// an [`Outline`] (use [`hidden_outline`]).
#[derive(Component, Clone)]
pub struct Interactive {
    pub rest: Color,
    pub hover: Color,
    pub active: Color,
    pub focus_ring: Color,
    /// Optional neobrutalist hard shadow: `(color, resting offset px)`. Hover
    /// lifts (offset +2, content nudged up 1px); active presses (shadow
    /// collapses, content drops to where the shadow was). Requires the entity
    /// to also have a [`BoxShadow`] and a [`UiTransform`].
    pub shadow: Option<(Color, f32)>,
}

impl Interactive {
    /// A flat control (background tints + focus ring, no shadow).
    pub fn flat(rest: Color, hover: Color, active: Color, focus_ring: Color) -> Self {
        Self {
            rest,
            hover,
            active,
            focus_ring,
            shadow: None,
        }
    }

    /// Adds a neobrutalist hard shadow that lifts on hover / presses on active.
    pub fn with_shadow(mut self, color: Color, offset: f32) -> Self {
        self.shadow = Some((color, offset));
        self
    }
}

/// A resting (invisible) outline. Give every [`Interactive`] one so the focus
/// ring can toggle without structural component churn.
pub fn hidden_outline() -> Outline {
    Outline {
        width: Val::Px(0.0),
        offset: Val::Px(0.0),
        color: Color::NONE,
    }
}

/// Which [`Interactive`] currently has focus.
#[derive(Resource, Default)]
pub struct UiFocus(pub Option<Entity>);

pub struct InteractionPlugin;

impl Plugin for InteractionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiFocus>()
            .add_systems(Update, (focus_on_press, tab_focus, apply_states).chain())
            .add_systems(Update, pointer_cursor);
    }
}

/// Clicking a control focuses it.
#[allow(clippy::type_complexity)]
fn focus_on_press(
    mut focus: ResMut<UiFocus>,
    q: Query<(Entity, &Interaction), (Changed<Interaction>, With<Interactive>)>,
) {
    for (entity, interaction) in &q {
        if *interaction == Interaction::Pressed {
            focus.0 = Some(entity);
        }
    }
}

/// Tab / Shift+Tab move focus between controls (stable entity order).
fn tab_focus(
    keys: Res<ButtonInput<KeyCode>>,
    mut focus: ResMut<UiFocus>,
    q: Query<Entity, With<Interactive>>,
) {
    if !keys.just_pressed(KeyCode::Tab) {
        return;
    }
    let mut entities: Vec<Entity> = q.iter().collect();
    if entities.is_empty() {
        return;
    }
    entities.sort();
    let back = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let next = match focus.0.and_then(|f| entities.iter().position(|&e| e == f)) {
        Some(i) if back => (i + entities.len() - 1) % entities.len(),
        Some(i) => (i + 1) % entities.len(),
        None => 0,
    };
    focus.0 = Some(entities[next]);
}

/// Sets the OS cursor to a pointer while any [`Interactive`] control is hovered
/// or pressed, and back to the default arrow otherwise. The cursor is a
/// [`CursorIcon`] component on the primary window entity; we only re-insert it
/// when the desired icon actually changes, to avoid per-frame churn.
fn pointer_cursor(
    mut commands: Commands,
    controls: Query<&Interaction, With<Interactive>>,
    window: Single<(Entity, Option<&CursorIcon>), With<PrimaryWindow>>,
) {
    let hovering = controls.iter().any(|i| !matches!(i, Interaction::None));
    let desired = CursorIcon::System(if hovering {
        SystemCursorIcon::Pointer
    } else {
        SystemCursorIcon::Default
    });
    let (window, current) = window.into_inner();
    if current != Some(&desired) {
        commands.entity(window).insert(desired);
    }
}

/// Paints each control for its current interaction + focus state.
#[allow(clippy::type_complexity)]
fn apply_states(
    focus: Res<UiFocus>,
    mut q: Query<(
        Entity,
        &Interaction,
        &Interactive,
        &mut BackgroundColor,
        &mut Outline,
        Option<&mut BoxShadow>,
        Option<&mut UiTransform>,
    )>,
) {
    for (entity, interaction, it, mut bg, mut outline, shadow, transform) in &mut q {
        bg.0 = match interaction {
            Interaction::Pressed => it.active,
            Interaction::Hovered => it.hover,
            Interaction::None => it.rest,
        };

        if focus.0 == Some(entity) {
            outline.width = Val::Px(2.0);
            outline.offset = Val::Px(2.0);
            outline.color = it.focus_ring;
        } else {
            outline.color = Color::NONE;
        }

        if let Some((color, offset)) = it.shadow {
            let (shadow_offset, shift) = match interaction {
                Interaction::Hovered => (offset + 2.0, -1.0),
                Interaction::Pressed => (0.0, offset),
                Interaction::None => (offset, 0.0),
            };
            if let Some(mut shadow) = shadow {
                *shadow = BoxShadow::new(
                    color,
                    Val::Px(shadow_offset),
                    Val::Px(shadow_offset),
                    Val::Px(0.0),
                    Val::Px(0.0),
                );
            }
            if let Some(mut transform) = transform {
                transform.translation = Val2::px(shift, shift);
            }
        }
    }
}
