use bevy::prelude::*;

use arenic_game::ui::menu_button;

use crate::states::AppState;

/// The title screen: "Arenic" centered above a "Start" button (-> Intro).
pub struct TitleScreenPlugin;

impl Plugin for TitleScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Title), setup);
    }
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    // Load the title font from `assets/fonts/`.
    let font = asset_server.load("fonts/Migra-Extrabold.ttf");

    // Full-screen column that centers the title and button row.
    let root = commands
        .spawn((
            DespawnOnExit(AppState::Title),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(40.0),
                ..default()
            },
        ))
        .id();

    let title = commands
        .spawn((
            Text::new("Arenic"),
            TextFont {
                font,
                font_size: 128.0,
                ..default()
            },
            TextColor(Color::WHITE),
        ))
        .id();

    let start = menu_button(&mut commands, "Start", 28.0);
    commands.entity(start).observe(
        |_: On<Pointer<Click>>, mut next: ResMut<NextState<AppState>>| {
            next.set(AppState::Intro);
        },
    );

    commands.entity(root).add_children(&[title, start]);
}
