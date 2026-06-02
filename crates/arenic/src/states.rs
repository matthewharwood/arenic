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
