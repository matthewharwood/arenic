// Author mode (`just author`) is a cargo feature that never ships: the plain
// game, release, and web builds compile none of it.
#[cfg(all(feature = "author", not(target_arch = "wasm32")))]
mod author;
#[cfg(all(feature = "author", not(target_arch = "wasm32")))]
mod dope_sheet;
#[cfg(all(feature = "author", not(target_arch = "wasm32")))]
mod entity_browser;
mod hotbar;
mod hud;
mod intro_scene;
#[cfg(all(feature = "author", not(target_arch = "wasm32")))]
mod layer_edit;
mod modal;
#[cfg(all(feature = "author", not(target_arch = "wasm32")))]
mod popout;
mod recording;
mod score_sync;
mod settings;
mod soundtrack;
mod states;
mod title;
mod travel;

use arenic_game::GameAudioPlugin;
use arenic_game::default_font::DefaultFontPlugin;
use arenic_game::default_plugins;
use arenic_game::effect::EffectPlugin;
use arenic_game::theme::ActiveTheme;
use arenic_game::timeline::TimelinePlugin;
use bevy::prelude::*;
use bevy::ui::UiScale;
use bevy::window::WindowResolution;
use hotbar::HotbarPlugin;
use hud::HudPlugin;
use intro_scene::IntroScenePlugin;
use modal::ModalPlugin;
use recording::RecordingPlugin;
use score_sync::ScoreSyncPlugin;
use settings::{Settings, SettingsPlugin};
use soundtrack::SoundtrackPlugin;
use states::AppState;
use title::TitleScreenPlugin;
use travel::TravelPlugin;

fn main() -> AppExit {
    // Load the persisted display settings BEFORE the window is created so it
    // opens at the chosen resolution (720 default). `logical()` sizes the
    // window in LOGICAL px; physical pixels follow the monitor's scale factor,
    // so it's a normal on-screen size and crisp at any DPI (UNITS_AND_SCALE §2,
    // `settings`).
    let settings = Settings::load();
    let win = settings.resolution.logical();

    let mut app = App::new();
    app.add_plugins(default_plugins().set(WindowPlugin {
        primary_window: Some(Window {
            resolution: WindowResolution::new(win.x as u32, win.y as u32),
            title: "Arenic".into(),
            ..default()
        }),
        ..default()
    }))
    // Must come after DefaultPlugins so it overwrites Bevy's built-in default font.
    .add_plugins(DefaultFontPlugin)
    .init_state::<AppState>()
    .init_resource::<ActiveTheme>()
    // Display resolution: seed the mode + UI scale from the saved settings, then
    // let SettingsPlugin apply live changes (`s = height/720` scales all UI).
    .insert_resource(settings.resolution)
    .insert_resource(UiScale(settings.resolution.scale()))
    .add_plugins(SettingsPlugin)
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
    // Non-destructive layer effects (scale/opacity tracks) — published
    // stacks animate in the game exactly as previewed in the author.
    .add_plugins(EffectPlugin)
    .add_plugins(HudPlugin)
    // The bottom action bar: four ability slots + the record/stop control.
    .add_plugins(HotbarPlugin)
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
