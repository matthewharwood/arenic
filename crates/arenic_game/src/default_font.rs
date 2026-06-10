use bevy::prelude::*;
use bevy::text::Font;

/// Embeds **DM Sans Medium** and installs it as Bevy's *default* font, and embeds
/// **DM Mono Medium** as [`MonoFont`] for numeric UI text.
///
/// Bevy stores its built-in default font at `AssetId::<Font>::default()` — the handle
/// every `TextFont` uses when no explicit `font` is set. We overwrite that slot with
/// DM Sans Medium, so **all UI text defaults to DM Sans Medium** unless a component
/// opts into a different font. Digit-bearing UI text should instead use the monospace
/// [`MonoFont`] so columns of numbers line up (see CLAUDE.md "Fonts").
///
/// Both are compile-time embeds of the `assets/fonts/*.ttf` files, independent of the
/// runtime asset root. `CARGO_MANIFEST_DIR` points at this crate (`crates/arenic_game`),
/// so we climb two levels to reach the workspace-root `assets/`.
const DM_SANS_MEDIUM: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/fonts/DMSans-Medium.ttf"
));
const DM_MONO_MEDIUM: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/fonts/DMMono-Medium.ttf"
));

/// The font for **numeric** UI text — DM Mono (Medium). Grab the handle to render
/// digits monospaced: `TextFont { font: mono.0.clone(), ..default() }`.
#[derive(Resource, Clone)]
pub struct MonoFont(pub Handle<Font>);

pub struct DefaultFontPlugin;

impl Plugin for DefaultFontPlugin {
    fn build(&self, app: &mut App) {
        // Bevy's `TextPlugin` (inside `DefaultPlugins`) inserts FiraMono at the default
        // id during its own `build`. Add this plugin *after* `DefaultPlugins` so our
        // overwrite wins. Both fonts parse synchronously from the embedded bytes.
        let sans = Font::try_from_bytes(DM_SANS_MEDIUM.to_vec())
            .expect("embedded DMSans-Medium.ttf failed to parse");
        let mono = Font::try_from_bytes(DM_MONO_MEDIUM.to_vec())
            .expect("embedded DMMono-Medium.ttf failed to parse");
        let mono_handle = {
            let mut fonts = app.world_mut().resource_mut::<Assets<Font>>();
            fonts
                .insert(AssetId::<Font>::default(), sans)
                .expect("failed to install DM Sans Medium as the default font");
            fonts.add(mono)
        };
        app.insert_resource(MonoFont(mono_handle));
    }
}
