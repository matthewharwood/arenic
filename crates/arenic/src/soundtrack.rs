//! The **soundtrack driver** — maps app state to what the music engine plays
//! (`arenic_game::audio`); the engine handles the crossfades.
//!
//! - **Title screen** → the Arenic theme. (Web caveat: the browser holds audio
//!   until the first user interaction, so on wasm the theme begins at the
//!   first click/keypress rather than on load — the Start click at the
//!   latest. Native plays immediately.)
//! - **Intro, zoomed in** → that arena's track ([`MusicTarget::Track`];
//!   arenas without one play nothing — the spatial SFX carry the scene).
//!   Pagination and edge-walks crossfade between tracks via [`CurrentArena`].
//! - **Intro, overworld** → the procedural oscillator drone, and the SFX bus
//!   ducks to [`OVERWORLD_SFX`] so the nine arenas' distant activity reads as
//!   a smoothed wash under it.
//!
//! Future note (per the design discussion): arena tracks could instead become
//! SPATIAL emitters parked on their arena roots, so the overworld hears all
//! nine in the distance — deliberately not done yet; with nine simultaneous
//! themes it needs musical strategy (shared key/tempo) before it stops being
//! cacophony. The seam is `MusicTarget`: a `SpatialTracks` variant would slot
//! into the same engine.

use arenic_game::arena::Arena;
use arenic_game::audio::{AudioMix, DesiredMusic, MusicTarget, OVERWORLD_SFX};
use bevy::prelude::*;

use crate::intro_scene::{CurrentArena, ZoomOut};
use crate::states::AppState;

/// Entering the title (first boot, or Esc from the game) fades the theme in.
fn title_theme(mut desired: ResMut<DesiredMusic>) {
    desired.set_if_neq(DesiredMusic(MusicTarget::Theme));
}

/// Keeps the music target + SFX duck matched to the camera: overworld plays
/// the drone over ducked SFX; zoomed in plays the focused arena's track at
/// full SFX. Cheap enough to run every Intro frame; `set_if_neq` keeps the
/// engine's change detection quiet when nothing moved. The `Single` camera
/// param deliberately skips the system on state-transition frames (camera not
/// yet/no longer alive) — the previous target simply keeps playing.
fn retarget(
    current: Res<CurrentArena>,
    zoomed_out: Single<Has<ZoomOut>, With<Camera3d>>,
    mut desired: ResMut<DesiredMusic>,
    mut mix: ResMut<AudioMix>,
) {
    let target = if *zoomed_out {
        MusicTarget::Drone
    } else {
        let arena =
            Arena::from_index(current.0).expect("invariant: CurrentArena is a valid arena index");
        MusicTarget::Track(arena)
    };
    desired.set_if_neq(DesiredMusic(target));
    let sfx_target = if *zoomed_out { OVERWORLD_SFX } else { 1.0 };
    if mix.sfx_target != sfx_target {
        mix.sfx_target = sfx_target;
    }
}

/// Drives [`DesiredMusic`] / the SFX duck from the app state. The engine
/// itself ships in [`arenic_game::audio::GameAudioPlugin`].
pub(crate) struct SoundtrackPlugin;

impl Plugin for SoundtrackPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Title), title_theme)
            .add_systems(Update, retarget.run_if(in_state(AppState::Intro)));
    }
}
