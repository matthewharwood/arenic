use bevy::prelude::*;

use arenic_game::theme::ActiveTheme;
use arenic_game::ui::menu_button;

use crate::states::AppState;

/// The title screen: "Arenic" centered above the "Start" (-> Intro) and
/// "Settings" (-> Settings) buttons.
pub struct TitleScreenPlugin;

impl Plugin for TitleScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Title), setup);
    }
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>, theme: Res<ActiveTheme>) {
    // The Title state owns its 2D camera (each state owns exactly one camera, so the
    // UI never fights a sibling camera over the framebuffer).
    commands.spawn((Camera2d, DespawnOnExit(AppState::Title)));

    // Load the title font from `assets/fonts/`.
    let font = asset_server.load("fonts/Migra-Extrabold.ttf");
    let palette = theme.palette();

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
            children![(
                Text::new("Arenic"),
                TextFont {
                    font,
                    font_size: 128.0,
                    ..default()
                },
                TextColor(palette.text_1()),
            )],
        ))
        .id();

    let start = menu_button(&mut commands, &palette, "Start", 28.0);
    commands.entity(start).observe(
        |_: On<Pointer<Click>>, mut next: ResMut<NextState<AppState>>| {
            next.set(AppState::Intro);
        },
    );

    let settings = menu_button(&mut commands, &palette, "Settings", 28.0);
    commands.entity(settings).observe(
        |_: On<Pointer<Click>>, mut next: ResMut<NextState<AppState>>| {
            next.set(AppState::Settings);
        },
    );

    commands.entity(root).add_children(&[start, settings]);
}
