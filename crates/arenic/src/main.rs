mod intro_scene;
mod states;
mod title_screen;

use arenic_game::default_font::DefaultFontPlugin;
use bevy::prelude::*;
use intro_scene::IntroScenePlugin;
use states::AppState;
use title_screen::TitleScreenPlugin;

fn main() -> AppExit {
    App::new()
        .add_plugins(arenic_game::default_plugins())
        // Must come after DefaultPlugins so it overwrites Bevy's built-in default font.
        .add_plugins(DefaultFontPlugin)
        .init_state::<AppState>()
        .add_systems(Startup, spawn_camera)
        .add_plugins(TitleScreenPlugin)
        .add_plugins(IntroScenePlugin)
        .run()
}

/// The single 2D camera, shared by every scene (scenes only own their UI).
fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}
