mod title_screen;

use bevy::prelude::*;
use title_screen::TitleScreenPlugin;

fn main() -> AppExit {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(TitleScreenPlugin)
        .run()
}
