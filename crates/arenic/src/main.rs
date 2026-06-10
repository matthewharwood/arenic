mod hud;
mod intro_scene;
mod states;
mod title_screen;

use arenic_game::default_font::DefaultFontPlugin;
use arenic_game::default_plugins;
use arenic_game::theme::ActiveTheme;
use bevy::prelude::*;
use bevy::window::WindowResolution;
use hud::HudPlugin;
use intro_scene::IntroScenePlugin;
use states::AppState;
use title_screen::TitleScreenPlugin;

fn main() -> AppExit {
    App::new()
        // Pin the framebuffer to 1280×720 PHYSICAL px (UNITS_AND_SCALE §2), so the
        // camera's ~75 px/unit (1 tile ≈ 19 px) holds regardless of monitor DPI.
        .add_plugins(default_plugins().set(WindowPlugin {
            primary_window: Some(Window {
                resolution: WindowResolution::new(1280, 720),
                title: "Arenic".into(),
                ..default()
            }),
            ..default()
        }))
        // Must come after DefaultPlugins so it overwrites Bevy's built-in default font.
        .add_plugins(DefaultFontPlugin)
        .init_state::<AppState>()
        .init_resource::<ActiveTheme>()
        .add_plugins(TitleScreenPlugin)
        .add_plugins(IntroScenePlugin)
        .add_plugins(HudPlugin)
        .run()
}
