use bevy::prelude::*;

/// The title screen: shows "Arenic" centered on screen in the Migra font.
pub struct TitleScreenPlugin;

impl Plugin for TitleScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup);
    }
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    // A 2D camera is required to see anything on screen.
    commands.spawn(Camera2d);

    // Load the title font from `assets/fonts/`.
    let font = asset_server.load("fonts/Migra-Extrabold.ttf");

    // A full-screen flex container that centers its child on both axes.
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        children![(
            Text::new("Arenic"),
            TextFont {
                font,
                font_size: 128.0,
                ..default()
            },
            TextColor(Color::WHITE),
        )],
    ));
}
