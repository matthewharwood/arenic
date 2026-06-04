//! `arenic_storybook` — a standalone binary for building and exercising game
//! pieces in isolation, separate from the game itself.
//!
//! It depends on `arenic_game` (the shared lib) so stories can render and drive
//! real game components — e.g. dropping the Guildmaster on a canvas to play with
//! his abilities outside the full game loop. Run it with:
//!
//! ```sh
//! cargo run -p arenic_storybook
//! ```

mod arena;
mod foreground;
mod hollow;
mod layers;
mod stage;
mod stories;
mod storybook;
mod widgets;

use arenic_game::default_font::DefaultFontPlugin;
use arenic_game::interaction::InteractionPlugin;
use arenic_game::orbit::OrbitCameraPlugin;
use bevy::prelude::*;
use storybook::StorybookPlugin;

fn main() -> AppExit {
    App::new()
        .add_plugins(arenic_game::default_plugins())
        // Must come after DefaultPlugins so it overwrites Bevy's built-in default font.
        .add_plugins(DefaultFontPlugin)
        .add_plugins(InteractionPlugin)
        .add_plugins(OrbitCameraPlugin)
        .add_plugins(hollow::HollowLightPlugin)
        .add_plugins(foreground::ForegroundPlugin)
        .add_plugins(layers::LayersPlugin)
        .add_systems(Startup, spawn_camera)
        .add_plugins(StorybookPlugin)
        .run()
}

/// The single 2D camera the storybook UI renders through.
fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}
