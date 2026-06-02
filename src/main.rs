mod default_font;
mod title_screen;

use bevy::prelude::*;
use default_font::DefaultFontPlugin;
use title_screen::TitleScreenPlugin;

fn main() -> AppExit {
    App::new()
        .add_plugins(DefaultPlugins)
        // Must come after DefaultPlugins so it overwrites Bevy's built-in default font.
        .add_plugins(DefaultFontPlugin)
        .add_plugins(TitleScreenPlugin)
        .run()
}
