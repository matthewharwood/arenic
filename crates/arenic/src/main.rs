// Author mode (`just author`) is a cargo feature that never ships: the plain
// game, release, and web builds compile none of it.
#[cfg(all(feature = "author", not(target_arch = "wasm32")))]
mod author;
mod hud;
mod intro_scene;
mod modal;
mod recording;
mod score_sync;
mod soundtrack;
mod states;
mod title_screen;
mod travel;

use arenic_game::GameAudioPlugin;
use arenic_game::default_font::DefaultFontPlugin;
use arenic_game::default_plugins;
use arenic_game::theme::ActiveTheme;
use arenic_game::timeline::TimelinePlugin;
use bevy::prelude::*;
use bevy::window::WindowResolution;
use hud::HudPlugin;
use intro_scene::IntroScenePlugin;
use modal::ModalPlugin;
use recording::RecordingPlugin;
use score_sync::ScoreSyncPlugin;
use soundtrack::SoundtrackPlugin;
use states::AppState;
use title_screen::TitleScreenPlugin;
use travel::TravelPlugin;

fn main() -> AppExit {
    let mut app = App::new();
    // Pin the framebuffer to 1280×720 PHYSICAL px (UNITS_AND_SCALE §2), so the
    // camera's ~75 px/unit (1 tile ≈ 19 px) holds regardless of monitor DPI.
    app.add_plugins(default_plugins().set(WindowPlugin {
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
    // The sheet-music sim: TimelinePlugin pins the 60 Hz fixed timestep and
    // replays every arena's master timeline; Modal + Recording layer the
    // keyboard-first capture/commit flows on top (`_docs/RULEBOOK.md`);
    // ScoreSync folds the versioned boss/tile score files in and tracks the
    // newest version at every cycle wrap.
    .add_plugins(TimelinePlugin)
    .add_plugins(ModalPlugin)
    .add_plugins(RecordingPlugin)
    .add_plugins(TravelPlugin)
    .add_plugins(ScoreSyncPlugin)
    .add_plugins(HudPlugin)
    // The camera is the microphone: spatial SFX + crossfading music. The
    // spatial knee sits at the zoomed-in camera height, so the focused arena
    // is full volume and the rest fall off by inverse-square distance.
    .add_plugins(GameAudioPlugin {
        reference_distance: intro_scene::ZOOM_IN,
    })
    .add_plugins(SoundtrackPlugin);
    #[cfg(all(feature = "author", not(target_arch = "wasm32")))]
    app.add_plugins(author::AuthorPlugin);
    app.run()
}
