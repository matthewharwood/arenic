use bevy::prelude::*;

use crate::theme::Theme;

/// Spawns a clickable text button and returns its entity so the caller can
/// attach an `On<Pointer<Click>>` observer and parent it where needed.
///
/// Colours come from `theme`'s tokens (`surface_3` rest, `interactions` hover,
/// `text_1` label). Hover feedback is wired up here via `Over`/`Out` observers;
/// the click behaviour is left to the caller since it varies per button.
pub fn menu_button(commands: &mut Commands, theme: &Theme, label: &str, font_size: f32) -> Entity {
    let rest = theme.surface_3();
    let (hover, _press) = theme.interactions(rest);
    commands
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(28.0), Val::Px(12.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rest),
            children![(
                Text::new(label),
                TextFont {
                    font_size,
                    ..default()
                },
                TextColor(theme.text_1()),
            )],
        ))
        .observe(
            move |ev: On<Pointer<Over>>, mut q: Query<&mut BackgroundColor>| {
                if let Ok(mut bg) = q.get_mut(ev.entity) {
                    bg.0 = hover;
                }
            },
        )
        .observe(
            move |ev: On<Pointer<Out>>, mut q: Query<&mut BackgroundColor>| {
                if let Ok(mut bg) = q.get_mut(ev.entity) {
                    bg.0 = rest;
                }
            },
        )
        .id()
}
