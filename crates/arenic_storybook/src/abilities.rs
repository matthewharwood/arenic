//! Runtime for the `abilities/*` stories: a neutral [`Player`] puck and the
//! key-triggered ability bursts you fire at the boss in an arena test scene. The
//! VFX itself is a reusable `arenic_game::ability` piece; the scene (player puck +
//! target boss) is staged by [`crate::stage`].

use arenic_game::ability::{AbilityPlugin, AbilitySfx, holy_nova, play_sfx};
use arenic_game::theme::ActiveTheme;
use bevy::input::common_conditions::input_just_pressed;
use bevy::prelude::*;

/// The player token in an ability-test scene — a neutral puck, NOT a class. The
/// fire systems target every `Player`, so a story with no puck is inert.
#[derive(Component)]
pub struct Player;

/// Adds the ability-VFX systems (the shared plugin also preloads [`AbilitySfx`])
/// + the storybook key-bindings that fire them.
pub struct AbilitiesPlugin;

impl Plugin for AbilitiesPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(AbilityPlugin).add_systems(
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
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    active: Res<ActiveTheme>,
    sfx: Res<AbilitySfx>,
) {
    // Holy gold that re-tones with the theme (`warning` is the warm token).
    let color = active.palette().warning;
    let mut fired = false;
    for player in &players {
        let burst = holy_nova(&mut meshes, &mut materials, color);
        // Spawn as a child of the puck so it bursts from the player.
        commands.spawn((burst, ChildOf(player)));
        fired = true;
    }
    // One sound per cast (not per puck), and only if something actually fired.
    if fired {
        play_sfx(&mut commands, sfx.holy_nova.clone());
    }
}
