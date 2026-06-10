//! Runtime for the `abilities/*` stories: a neutral [`Player`] puck and the
//! key-triggered ability bursts you fire at the boss in an arena test scene. The
//! VFX itself is a reusable `arenic_game::ability` piece; the scene (player puck +
//! target boss) is staged by [`crate::stage`].

use arenic_game::AbilityId;
use arenic_game::ability::{AbilityMeshes, AbilityPlugin, AbilitySfx, holy_nova};
use arenic_game::audio::{AudioMix, play_spatial_sfx};
use arenic_game::theme::ActiveTheme;
use bevy::audio::{DefaultSpatialScale, SpatialScale};
use bevy::input::common_conditions::input_just_pressed;
use bevy::prelude::*;

/// The stage camera's home orbit radius (`crate::stage`) — the spatial-audio
/// knee sits there, matching the game's zoomed-in tuning.
const STAGE_LISTENER_DISTANCE: f32 = 24.0;

/// The player token in an ability-test scene — a neutral puck, NOT a class. The
/// fire systems target every `Player`, so a story with no puck is inert.
#[derive(Component)]
pub struct Player;

/// Adds the ability-VFX systems (the shared plugin also preloads [`AbilitySfx`])
/// + the storybook key-bindings that fire them.
pub struct AbilitiesPlugin;

impl Plugin for AbilitiesPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(AbilityPlugin)
            // Casts emit spatially against the stage camera's listener; the
            // storybook has no music engine, just the SFX bus defaults.
            .init_resource::<AudioMix>()
            .insert_resource(DefaultSpatialScale(SpatialScale::new(
                1.0 / STAGE_LISTENER_DISTANCE,
            )))
            .add_systems(
                Update,
                fire_holy_nova.run_if(
                    input_just_pressed(KeyCode::Digit1).or(input_just_pressed(KeyCode::Numpad1)),
                ),
            );
    }
}

/// `1` (or numpad `1`) fires a Holy Nova from every [`Player`] puck on the board:
/// the burst VFX from each puck + the ability's sound once. A no-op on any story
/// without a player, so it's inert outside the ability scene.
fn fire_holy_nova(
    mut commands: Commands,
    players: Query<Entity, With<Player>>,
    meshes: Res<AbilityMeshes>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    active: Res<ActiveTheme>,
    sfx: Res<AbilitySfx>,
    mix: Res<AudioMix>,
    scale: Res<DefaultSpatialScale>,
) {
    // Holy gold that re-tones with the theme (`warning` is the warm token).
    let color = active.palette().warning;
    let mut fired = None;
    for player in &players {
        let burst = holy_nova(meshes.sphere.clone(), &mut materials, color);
        // Spawn as a child of the puck so it bursts from the player.
        commands.spawn((burst, ChildOf(player)));
        fired.get_or_insert(player);
    }
    // One sound per cast (not per puck), emitted spatially from the first
    // firing puck, and only if something actually fired.
    if let Some(player) = fired {
        play_spatial_sfx(
            &mut commands,
            sfx.holy_nova.clone(),
            player,
            AbilityId::HolyNova.sfx_profile(),
            &mix,
            &scale,
        );
    }
}
