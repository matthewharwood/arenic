use bevy::{

    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    prelude::*,
    window::{WindowTheme},
};
use bevy::window::CursorIcon; 

fn main() {
    App::new()
    .add_plugins((
        DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "ARENIC".into(),
                name: Some("ARENIC".into()),
                window_theme: Some(WindowTheme::Light),
                transparent: false,
                ..default()
            }),
            ..default()
        }),
        LogDiagnosticsPlugin::default(),
        FrameTimeDiagnosticsPlugin,
    ))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                button_system,
                player_movement,
                spawn_player_shot,
                move_player_shot,
                despawn_player_shot,
                spawn_enemy,
                move_enemy,
            ),
        )
        .run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera2dBundle::default());
    commands.spawn((
        SpriteBundle {
            texture: asset_server.load("rect.png"),
            ..default()
        },
        Player,
    ));

    // Call button setup
    start_button_setup(commands, asset_server);
}

#[derive(Component)]
struct Lifetime {
    timer: Timer,
}

#[derive(Component)]
struct Player;

#[derive(Component)]
struct Enemy;

#[derive(Component)]
struct PlayerShot;

fn player_movement(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut query: Query<&mut Transform, With<Player>>,
    time: Res<Time>,
) {
    for mut transform in query.iter_mut() {
        let mut direction = Vec3::ZERO;
        if keyboard_input.pressed(KeyCode::ArrowLeft) {
            direction.x -= 1.0;
        }
        if keyboard_input.pressed(KeyCode::ArrowRight) {
            direction.x += 1.0;
        }
        if keyboard_input.pressed(KeyCode::ArrowUp) {
            direction.y += 1.0;
        }
        if keyboard_input.pressed(KeyCode::ArrowDown) {
            direction.y -= 1.0;
        }
        let movement_speed = 300.0;
        transform.translation += direction.normalize_or_zero() * movement_speed * time.delta_seconds();
    }
}

fn spawn_player_shot(
    mut commands: Commands,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    player_query: Query<&Transform, With<Player>>,
    asset_server: Res<AssetServer>,
) {
    if keyboard_input.just_pressed(KeyCode::Space) {
        if let Ok(player_transform) = player_query.get_single() {
            let shot_position = player_transform.translation + Vec3::new(0.0, 10.0, 0.0);

            commands.spawn((
                SpriteBundle {
                    texture: asset_server.load("bullet.png"),
                    transform: Transform::from_translation(shot_position),
                    ..Default::default()
                },
                PlayerShot,
                Lifetime {
                    timer: Timer::from_seconds(1.0, TimerMode::Once),
                },
            ));

            println!("FIRE"); // Log when the shot is spawned
        }
    }
}

fn move_player_shot(
    time: Res<Time>,
    mut query: Query<&mut Transform, With<PlayerShot>>,
) {
    for mut transform in query.iter_mut() {
        transform.translation.y += 1000.0 * time.delta_seconds(); // Adjust speed as needed
    }
}

fn despawn_player_shot(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Lifetime), With<PlayerShot>>,
) {
    for (entity, mut lifetime) in query.iter_mut() {
        lifetime.timer.tick(time.delta());
        if lifetime.timer.finished() {
            commands.entity(entity).despawn();
        }
    }
}

fn spawn_enemy(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        SpriteBundle {
            texture: asset_server.load("enemy.png"),
            transform: Transform::from_translation(Vec3::ZERO),
            ..Default::default()
        },
        Enemy,
    ));
}
fn move_enemy(time: Res<Time>, mut query: Query<&mut Transform, With<Enemy>>) {
    for mut transform in query.iter_mut() {
        transform.translation.y -= 100.0 * time.delta_seconds();
    }
}

fn button_system(
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
    mut windows: Query<&mut Window>,
) {
    let mut window = windows.single_mut();
    
    for (interaction, mut color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                *color = Color::srgb(0.9, 0.9, 0.9).into();
            }
            Interaction::Hovered => {
                *color = Color::srgb(0.941, 0.914, 0.914).into();
            }
            Interaction::None => {
                *color = Color::srgba(1.0, 1.0, 1.0, 0.0).into();
            }
        }

        // Update cursor icon based on interaction
        window.cursor.icon = match *interaction {
            Interaction::Hovered => CursorIcon::Pointer,
            _ => CursorIcon::Default,
        };
    }
}

fn start_button_setup(mut commands: Commands, asset_server: Res<AssetServer>) {
       commands
           .spawn(ButtonBundle {
               style: Style {
                   width: Val::Px(128.0),
                   height: Val::Px(72.0),
                   margin: UiRect::all(Val::Auto),
                   justify_content: JustifyContent::Center,
                   align_items: AlignItems::Center,
                   border: UiRect::all(Val::Px(1.0)),
                   ..default()
               },
            //    background_color: Color::srgb(1.0, 1.0, 1.0).into(),
               border_color: Color::srgb(1.0, 1.0, 0.0).into(),
               ..default()
           })
           .with_children(|parent| {
               parent.spawn(TextBundle::from_section(
                   "Start",
                   TextStyle {
                       font: asset_server.load("fonts/Migra-Extralight.ttf"),
                       font_size: 32.0,
                       color: Color::srgb(0.0, 0.0, 0.0),
                   },
               ));
           });
}