use bevy::prelude::*;

/// Top-level screens of the app. Each scene owns one variant and spawns its UI
/// on `OnEnter`, tagging entities with `DespawnOnExit(..)` so they clean up on
/// transition. Navigation is just `NextState::set(..)`.
#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
pub enum AppState {
    /// Landing screen: the "Arenic" title plus the Start button.
    #[default]
    Title,
    /// Placeholder for the game's intro (entered via "Start").
    Intro,
}

/// Present while the author feature's tile editor owns the keyboard — world
/// input (arrows, R, Tab, 1, L, pagination) yields to the editor's cursor and
/// scrubbing. Only the `author` feature ever inserts this, so in shipping
/// builds [`not_tile_editing`] is constant `true` and the gates are inert.
#[derive(Resource)]
pub(crate) struct TileEditMode;

/// `true` while the tile editor is closed — the gate world-input systems wear.
pub(crate) fn not_tile_editing(mode: Option<Res<TileEditMode>>) -> bool {
    mode.is_none()
}
