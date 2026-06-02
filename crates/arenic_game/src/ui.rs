use bevy::prelude::*;

const NORMAL: Color = Color::srgb(0.15, 0.15, 0.18);
const HOVERED: Color = Color::srgb(0.25, 0.25, 0.30);

/// Spawns a clickable text button and returns its entity so the caller can
/// attach an `On<Pointer<Click>>` observer and parent it where needed.
///
/// Hover feedback is wired up here via `Over`/`Out` observers; the click
/// behaviour is left to the caller since it varies per button.
pub fn menu_button(commands: &mut Commands, label: &str, font_size: f32) -> Entity {
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
            BackgroundColor(NORMAL),
            children![(
                Text::new(label),
                TextFont {
                    font_size,
                    ..default()
                },
                TextColor(Color::WHITE),
            )],
        ))
        .observe(|ev: On<Pointer<Over>>, mut q: Query<&mut BackgroundColor>| {
            if let Ok(mut bg) = q.get_mut(ev.entity) {
                bg.0 = HOVERED;
            }
        })
        .observe(|ev: On<Pointer<Out>>, mut q: Query<&mut BackgroundColor>| {
            if let Ok(mut bg) = q.get_mut(ev.entity) {
                bg.0 = NORMAL;
            }
        })
        .id()
}
