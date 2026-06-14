//! Game audio — **the camera is the microphone**.
//!
//! Three layers, all bevy_audio (rodio), all web-safe (the spatial math is
//! pure-Rust source-side DSP, identical on wasm):
//!
//! - **Spatial SFX**: [`play_spatial_sfx`] spawns a one-shot as a child of its
//!   emitter, `spatial: true`, so rodio pans and attenuates it against the
//!   camera's [`SpatialListener`] ears. The inverse-square knee sits at
//!   [`GameAudioPlugin::reference_distance`] (the zoomed-in camera height), so
//!   the focused arena reads full volume, neighbours ~0.7 and panned, the far
//!   corner ~0.3, and from the overworld everything settles to a ~0.1 wash.
//!   Per-sound character comes from [`SfxProfile`]: `volume`, plus `reach`
//!   dividing the spatial scale so big abilities carry farther.
//! - **Music**: one [`DesiredMusic`] resource names what should be playing
//!   ([`MusicTarget`]); the engine crossfades looping sinks toward it
//!   (~1.2 s exponential approach) and despawns what faded out. The overworld
//!   target is the procedural [`DroneSource`] — oscillators, no asset. An arena
//!   [`MusicTarget::Track`] is LOCKED to that arena's 2-minute [`ArenaClock`]
//!   ([`sync_tracks`]): it drops in at the clock's position, fades out/in across
//!   the cycle boundary, and re-seeks on a wrap / `F5` restart / dope-sheet seek
//!   (paused while the clock is, so authoring scrubs don't drift).
//! - **Mix**: [`AudioMix`] is a tiny SFX bus — the overworld ducks it to
//!   [`OVERWORLD_SFX`] so the drone "smooths" the distant activity (bevy_audio
//!   has no filters; duck + mask is the honest substitute until Firewheel).
//!
//! Web notes: all gameplay audio starts after the Start click (autoplay-safe);
//! the title theme spawns on the Title screen and simply begins at the first
//! user interaction the browser accepts. Volume tweens land in ~46 ms steps on
//! wasm — invisible at these fade lengths.

mod drone;

pub use drone::DroneSource;

use std::time::Duration;

use bevy::audio::{
    AddAudioSource, AudioPlayer, AudioSink, AudioSinkPlayback, AudioSource, DefaultSpatialScale,
    GlobalVolume, PlaybackSettings, SpatialScale, Volume,
};
use bevy::prelude::*;

use crate::arena::Arena;
use crate::grid::ARENAS;
use crate::timeline::{ArenaClock, CYCLE_TICKS, TICKS_PER_SECOND};

/// Ear gap of the camera-microphone, world units ([`bevy::audio::SpatialListener::new`]).
/// Pan strength for distant sources is gap-invariant; this mostly shapes how
/// hard near sounds pan.
pub const EAR_GAP: f32 = 4.0;
/// SFX bus level while the overworld camera is up — the duck that lets the
/// drone smooth the distant activity.
pub const OVERWORLD_SFX: f32 = 0.45;
/// Time constant of a music crossfade (~95% done in three of these).
const MUSIC_FADE_TAU: f32 = 0.4;
/// Time constant of the SFX bus duck.
const MIX_TAU: f32 = 0.17;
/// A music sink below this is silent enough to despawn.
const SILENCE_FLOOR: f32 = 0.005;
/// Ticks of fade at each end of the 2-minute arena cue: the track fades OUT
/// over the last `CYCLE_FADE_TICKS` and back IN over the first, so the loop seam
/// at the cycle boundary is a soft envelope, not a hard cut (and it masks the
/// resync seek to tick 0). 90 ticks = 1.5 s.
const CYCLE_FADE_TICKS: u32 = 90;
/// A forward clock jump beyond this many ticks in one frame is a SEEK, not the
/// natural ~1-tick advance — so the music re-seeks to it. 30 ticks = 0.5 s,
/// comfortably above any frame-hitch catch-up yet far below a `,`/`.` scrub
/// (≥1 s) or a ruler drag.
const RESYNC_FORWARD: u32 = 30;

/// The title screen's theme.
const THEME_PATH: &str = "music/arenic_theme.mp3";

/// Per-arena soundtrack files under `assets/music/` — fill in a line as a
/// track lands (see `assets/music/README.md`). `None` means a zoomed-in visit
/// plays no music: the spatial SFX carry the scene.
fn arena_track(arena: Arena) -> Option<&'static str> {
    match arena {
        Arena::Guildmaster => Some("music/guildmaster_guildhouse.mp3"),
        Arena::Warrior => Some("music/warrior_bastion.mp3"),
        Arena::Merchant => Some("music/merchant_casino.mp3"),
        _ => None,
    }
}

/// What the music engine should be playing. The binaries' drivers decide;
/// the engine crossfades toward it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MusicTarget {
    /// The title screen's Arenic theme.
    Theme,
    /// The arena's own soundtrack (silence if it has none yet).
    Track(Arena),
    /// The overworld's procedural oscillator drone.
    Drone,
    /// Fade everything out.
    Silence,
}

/// The one knob a driver turns: set it (`set_if_neq`) and the engine handles
/// spawn/crossfade/despawn.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub struct DesiredMusic(pub MusicTarget);

impl Default for DesiredMusic {
    fn default() -> Self {
        Self(MusicTarget::Silence)
    }
}

/// The SFX bus: one-shots multiply their volume by `sfx` at spawn; `tend_mix`
/// eases `sfx` toward `sfx_target` (the zoom driver writes the target). Spawn
/// time only — a one-shot already in flight keeps its volume (they live
/// ~0.6 s; the duck settles in ~0.5 s, so the overlap is inaudible).
#[derive(Resource)]
pub struct AudioMix {
    pub sfx: f32,
    pub sfx_target: f32,
}

impl Default for AudioMix {
    fn default() -> Self {
        Self {
            sfx: 1.0,
            sfx_target: 1.0,
        }
    }
}

/// How one sound carries: `volume` scales its level; `reach` divides its
/// spatial scale, so `reach > 1` is heard farther (a boss roar) and
/// `reach < 1` stays local (a tick). The "different tones behave differently"
/// hook — see [`crate::ability::AbilityId::sfx_profile`].
#[derive(Clone, Copy, Debug)]
pub struct SfxProfile {
    pub volume: f32,
    pub reach: f32,
}

/// Plays `sound` as a spatial one-shot riding `emitter` (spawned as its child,
/// so it tracks the emitter while it plays and despawns itself when done).
/// The sound also DIES with its emitter (despawn is recursive) — fine here,
/// because every caster outlives a one-shot and a scene exit SHOULD cut audio;
/// don't ride an emitter that despawns mid-sound.
pub fn play_spatial_sfx(
    commands: &mut Commands,
    sound: Handle<AudioSource>,
    emitter: Entity,
    profile: SfxProfile,
    mix: &AudioMix,
    scale: &DefaultSpatialScale,
) {
    commands.spawn((
        AudioPlayer::new(sound),
        PlaybackSettings::DESPAWN
            .with_spatial(true)
            .with_volume(Volume::Linear(profile.volume * mix.sfx))
            .with_spatial_scale(SpatialScale(scale.0.0 / profile.reach)),
        Transform::default(),
        ChildOf(emitter),
    ));
}

/// Plays a one-shot with no position — UI feedback, not world sound.
pub fn play_sfx(commands: &mut Commands, sound: Handle<AudioSource>) {
    commands.spawn((AudioPlayer::new(sound), PlaybackSettings::DESPAWN));
}

/// The loaded music handles: the theme, the per-arena track table (sparse —
/// only paths that exist are loaded, so the web build never 404s), and the
/// procedural drone.
#[derive(Resource)]
struct MusicAssets {
    theme: Handle<AudioSource>,
    tracks: [Option<Handle<AudioSource>>; ARENAS],
    drone: Handle<DroneSource>,
}

/// One live looping music sink and the target it represents. At most two
/// exist at once (the one fading in and the one fading out).
#[derive(Component)]
#[component(immutable)]
struct MusicChannel(MusicTarget);

/// Clock-sync state on an arena [`MusicChannel`] (only `Track` channels carry
/// it): the tick it last observed — so [`sync_tracks`] can spot a
/// wrap/restart/seek discontinuity — and the cycle-end volume envelope that
/// [`tend_music`] folds in.
#[derive(Component)]
struct TrackSync {
    prev_tick: u32,
    envelope: f32,
}

/// The cycle-end volume envelope from an arena tick: fades in over the first
/// [`CYCLE_FADE_TICKS`], full through the middle, fades out over the last —
/// deterministic, so the boundary fade can never drift from the clock.
fn cycle_envelope(tick: u32) -> f32 {
    if tick < CYCLE_FADE_TICKS {
        tick as f32 / CYCLE_FADE_TICKS as f32
    } else if tick >= CYCLE_TICKS.saturating_sub(CYCLE_FADE_TICKS) {
        CYCLE_TICKS.saturating_sub(tick) as f32 / CYCLE_FADE_TICKS as f32
    } else {
        1.0
    }
}

/// The arena playback position (seconds) for a clock tick.
fn tick_seconds(tick: u32) -> Duration {
    Duration::from_secs_f32(tick as f32 / TICKS_PER_SECOND as f32)
}

/// A music sink's full-on level, per target — the drone sits a little lower
/// so the spatial wash stays legible through it.
fn target_level(target: MusicTarget) -> f32 {
    match target {
        MusicTarget::Theme => 0.6,
        MusicTarget::Track(_) => 0.55,
        MusicTarget::Drone => 0.5,
        MusicTarget::Silence => 0.0,
    }
}

fn load_music(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut drones: ResMut<Assets<DroneSource>>,
) {
    commands.insert_resource(MusicAssets {
        theme: assets.load(THEME_PATH),
        tracks: Arena::ALL.map(|arena| arena_track(arena).map(|path| assets.load(path))),
        drone: drones.add(DroneSource),
    });
}

/// Spawns the sink for the desired target when none exists (silent;
/// `tend_music` fades it in). A still-fading-out sink for the same target is
/// simply re-tended back up, so rapid retargeting never stacks sinks. Runs
/// EVERY frame (a ≤2-row query + an early return) rather than on change
/// detection: a despawn command for a dying channel is deferred, so a
/// change-gated spawn could see the corpse, skip, and leave the target
/// permanently silent — self-healing beats edge-triggered here.
fn apply_music(
    mut commands: Commands,
    desired: Res<DesiredMusic>,
    assets: Res<MusicAssets>,
    channels: Query<&MusicChannel>,
    clocks: Query<(&Arena, &ArenaClock)>,
) {
    let target = desired.0;
    if channels.iter().any(|channel| channel.0 == target) {
        return;
    }
    let settings = PlaybackSettings::LOOP.with_volume(Volume::SILENT);
    match target {
        MusicTarget::Silence => {}
        MusicTarget::Theme => {
            commands.spawn((
                AudioPlayer::new(assets.theme.clone()),
                settings,
                MusicChannel(target),
            ));
        }
        MusicTarget::Track(arena) => {
            // No file yet -> nothing spawns; the outgoing track fades to silence.
            if let Some(track) = &assets.tracks[arena.index()] {
                // Drop in at the arena clock's position, so entering mid-cycle
                // begins the cue at the matching timestamp (tick 0 -> the start).
                let tick = clocks
                    .iter()
                    .find(|(candidate, _)| **candidate == arena)
                    .map_or(0, |(_, clock)| clock.tick);
                commands.spawn((
                    AudioPlayer::new(track.clone()),
                    settings.with_start_position(tick_seconds(tick)),
                    MusicChannel(target),
                    TrackSync {
                        prev_tick: tick,
                        envelope: cycle_envelope(tick),
                    },
                ));
            }
        }
        MusicTarget::Drone => {
            // ONCE, not LOOP: the drone never ends, and `Loop` would wrap it
            // in rodio's `repeat_infinite`, whose pinned `Buffered` head
            // retains every sample ever played (~10 MB/min, forever).
            commands.spawn((
                AudioPlayer(assets.drone.clone()),
                PlaybackSettings::ONCE.with_volume(Volume::SILENT),
                MusicChannel(target),
            ));
        }
    }
}

/// Locks the focused arena's track to its [`ArenaClock`]: pauses the sink while
/// the clock is paused (scrubbing/editing), re-seeks it on any non-natural tick
/// jump — a wrap, an `F5` restart, a dope-sheet/ruler seek — and refreshes the
/// cycle-end envelope. Those seeks land inaudibly: at a boundary the envelope
/// has already faded the track near-silent, and while scrubbing the clock (hence
/// the sink) is paused. Only the desired channel is synced; an outgoing track
/// just fades out free-running. Steady play needs no seeks — sink and clock both
/// advance at real time, so they stay in step from the `start_position` on.
fn sync_tracks(
    desired: Res<DesiredMusic>,
    clocks: Query<(&Arena, &ArenaClock)>,
    mut channels: Query<(&MusicChannel, &AudioSink, &mut TrackSync)>,
) {
    let MusicTarget::Track(arena) = desired.0 else {
        return;
    };
    let Some((_, clock)) = clocks.iter().find(|(candidate, _)| **candidate == arena) else {
        return;
    };
    let tick = clock.tick;
    for (channel, sink, mut sync) in &mut channels {
        if channel.0 != desired.0 {
            continue; // an outgoing track fades out free-running
        }
        if clock.paused {
            if !sink.is_paused() {
                sink.pause();
            }
        } else if sink.is_paused() {
            sink.play();
        }
        let jumped = tick < sync.prev_tick || tick > sync.prev_tick.saturating_add(RESYNC_FORWARD);
        if jumped {
            let _ = sink.try_seek(tick_seconds(tick));
        }
        sync.envelope = cycle_envelope(tick);
        sync.prev_tick = tick;
    }
}

/// Eases every music sink toward its place in the crossfade — full level for
/// the desired target, silence for the rest — and despawns what finished
/// fading out. Exponential approach: smooth, stateless, web-step tolerant.
/// The goal is multiplied by [`GlobalVolume`]: bevy bakes it in only at sink
/// creation, so an absolute `set_volume` here would silently exempt music
/// from any future master-volume control while SFX still obeyed it.
fn tend_music(
    mut commands: Commands,
    time: Res<Time>,
    desired: Res<DesiredMusic>,
    global: Res<GlobalVolume>,
    mut channels: Query<(Entity, &MusicChannel, &mut AudioSink, Option<&TrackSync>)>,
) {
    let factor = (time.delta_secs() / MUSIC_FADE_TAU).min(1.0);
    for (entity, channel, mut sink, sync) in &mut channels {
        let wanted = channel.0 == desired.0;
        // A Track channel's cycle-end envelope folds into the crossfade goal, so
        // the boundary fade and the arena-switch crossfade share one volume.
        let envelope = sync.map_or(1.0, |sync| sync.envelope);
        let level = if wanted {
            target_level(channel.0) * envelope
        } else {
            0.0
        };
        let goal = Volume::Linear(level) * global.volume;
        let next = sink.volume().fade_towards(goal, factor);
        sink.set_volume(next);
        if !wanted && next.to_linear() < SILENCE_FLOOR {
            commands.entity(entity).despawn();
        }
    }
}

/// Eases the SFX bus toward its target (the zoom driver's duck).
fn tend_mix(time: Res<Time>, mut mix: ResMut<AudioMix>) {
    if (mix.sfx - mix.sfx_target).abs() < 1e-3 {
        return;
    }
    let factor = (time.delta_secs() / MIX_TAU).min(1.0);
    mix.sfx += (mix.sfx_target - mix.sfx) * factor;
}

/// The audio engine. The binary that adds it owns the listener (a
/// [`bevy::audio::SpatialListener`] on its camera) and a driver that writes
/// [`DesiredMusic`] / [`AudioMix::sfx_target`]; everything else lives here.
pub struct GameAudioPlugin {
    /// Camera-to-board distance where spatial gain reaches full — the
    /// inverse-square knee. The game passes its zoomed-in camera height.
    pub reference_distance: f32,
}

impl Plugin for GameAudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_source::<DroneSource>()
            .init_resource::<AudioMix>()
            .init_resource::<DesiredMusic>()
            .insert_resource(DefaultSpatialScale(SpatialScale::new(
                1.0 / self.reference_distance,
            )))
            .add_systems(Startup, load_music)
            // Chained so a frame reads: spawn what's missing, then fade. The
            // binaries' drivers write DesiredMusic in plain Update; with
            // apply_music self-healing, driver-vs-engine order can't strand
            // a target.
            .add_systems(
                Update,
                (
                    apply_music.run_if(resource_exists::<MusicAssets>),
                    // Lock the focused arena's track to its clock, then fade.
                    sync_tracks,
                    tend_music,
                    tend_mix,
                )
                    .chain(),
            );
    }
}
