use bevy::prelude::*;
use bevy::text::Font;

/// Embeds Archivo into the binary and installs it as Bevy's *default* font.
///
/// Bevy stores its built-in default font (FiraMono) at `AssetId::<Font>::default()`
/// — the handle every `TextFont` uses when no explicit `font` is set. We parse
/// Archivo synchronously and overwrite that slot, so all text defaults to Archivo
/// (our sans-serif) unless a component opts into a different font.
///
/// `Archivo-Variable.ttf` is a variable font; its default instance is Regular weight.
const ARCHIVO_TTF: &[u8] = include_bytes!("../assets/fonts/Archivo-Variable.ttf");

pub struct DefaultFontPlugin;

impl Plugin for DefaultFontPlugin {
    fn build(&self, app: &mut App) {
        // Bevy's `TextPlugin` (inside `DefaultPlugins`) inserts FiraMono at this id
        // during its own `build`. Add this plugin *after* `DefaultPlugins` so our
        // overwrite wins.
        let font = Font::try_from_bytes(ARCHIVO_TTF.to_vec())
            .expect("embedded Archivo-Variable.ttf failed to parse");
        let mut fonts = app.world_mut().resource_mut::<Assets<Font>>();
        fonts
            .insert(AssetId::<Font>::default(), font)
            .expect("failed to install Archivo as the default font");
    }
}
