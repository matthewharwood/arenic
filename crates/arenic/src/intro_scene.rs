use bevy::prelude::*;

use arenic_game::ui::menu_button;

use crate::states::AppState;

/// Scaffold for the game's intro sequence (entered from the title's "Start").
/// Currently just a placeholder with a way back to the title.
pub struct IntroScenePlugin;

impl Plugin for IntroScenePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Intro), setup);
    }
}

fn setup(mut commands: Commands) {
    let root = commands
        .spawn((
            DespawnOnExit(AppState::Intro),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(32.0),
                ..default()
            },
        ))
        .id();

    let heading = commands
        .spawn((
            Text::new("Intro"),
            TextFont {
                font_size: 72.0,
                ..default()
            },
            TextColor(Color::WHITE),
        ))
        .id();

    let back = menu_button(&mut commands, "Back", 28.0);
    commands.entity(back).observe(
        |_: On<Pointer<Click>>, mut next: ResMut<NextState<AppState>>| {
            next.set(AppState::Title);
        },
    );

    commands.entity(root).add_children(&[heading, back]);
}
