//! Title-screen choreography **playback**: loads the baked RON score (generated
//! by `cargo xtask choreograph`), spawns a soft dark circle per active agent,
//! and glides each along its tick-stamped path on a looping 2-minute clock —
//! never overlapping (the generator proved that). The circles are the only 3D
//! content; the [`super::glass`] post-process blurs them into ethereal,
//! distorted drop-shadows.
//!
//! The score is `include_str!`'d (baked into the binary) so it loads identically
//! on web — no filesystem, no async asset flicker. Regenerate + rebuild to
//! update it; the versioned `choreography.vNNNN.ron` is the rollback history.

use bevy::color::Alpha;
use bevy::prelude::*;
use serde::Deserialize;

use crate::states::AppState;

/// The baked score (a copy of the newest `choreography.vNNNN.ron`).
const SCORE: &str = include_str!("../../../../assets/title/choreography.latest.ron");

/// Radius of a circle in cell units (< 0.5 so neighbours never visually touch —
/// the no-overlap guarantee is at the cell level).
const CIRCLE_R: f32 = 0.4;
/// Sim ticks per second (the score's tick rate; matches the game clock).
const TICKS_PER_SECOND: f32 = 60.0;
/// Fade-in / fade-out ramp length in ticks (~0.6 s) so circles dissolve in and
/// out at the edges instead of popping.
const FADE_TICKS: f32 = 36.0;

// --- RON schema (Deserialize mirror of `xtask::score`) ----------------------

#[derive(Deserialize, Clone)]
pub(crate) struct Choreography {
    grid_w: u32,
    grid_h: u32,
    cycle_ticks: u32,
    agents: Vec<Agent>,
}

#[derive(Deserialize, Clone)]
struct Agent {
    spawn_tick: u32,
    despawn_tick: u32,
    start: [i32; 2],
    steps: Vec<Step>,
}

#[derive(Deserialize, Clone)]
struct Step {
    tick: u32,
    dx: i32,
    dy: i32,
}

// --- Resources / components -------------------------------------------------

/// The parsed score (inserted on title enter).
#[derive(Resource)]
pub(crate) struct TitleScore(Choreography);

/// The looping playback head, in fractional 60 Hz ticks.
#[derive(Resource)]
pub(crate) struct TitleClock(f32);

/// Shared circle mesh + the base grey colour. Each circle gets its OWN material
/// (cloned from this colour) so it can fade independently.
#[derive(Resource)]
pub(crate) struct CircleAssets {
    mesh: Handle<Mesh>,
    color: Color,
}

/// Tags a live circle with the score-agent it's playing.
#[derive(Component)]
struct CircleAgent(usize);

// --- Setup ------------------------------------------------------------------

/// Parses the baked score (called once on title enter).
pub(crate) fn parse_score() -> Choreography {
    ron::from_str::<Choreography>(SCORE).expect("baked title choreography RON is valid")
}

/// Inserts the playback resources + the shared circle assets. `color` is the
/// (opaque) grey circle colour; per-circle materials clone it and animate alpha.
pub(crate) fn setup_playback(commands: &mut Commands, meshes: &mut Assets<Mesh>, color: Color) {
    let mesh = meshes.add(Circle::new(CIRCLE_R));
    commands.insert_resource(TitleScore(parse_score()));
    commands.insert_resource(TitleClock(0.0));
    commands.insert_resource(CircleAssets { mesh, color });
}

// --- Playback ---------------------------------------------------------------

/// Advances the looping playback head.
fn advance_clock(time: Res<Time>, score: Res<TitleScore>, mut clock: ResMut<TitleClock>) {
    let cycle = score.0.cycle_ticks.max(1) as f32;
    clock.0 = (clock.0 + time.delta_secs() * TICKS_PER_SECOND).rem_euclid(cycle);
}

/// Cell centre → world position. Grid is 1 unit/cell, centred on the origin,
/// `y` up (so it matches the camera framing).
fn cell_to_world(cell: (f32, f32), gw: u32, gh: u32) -> Vec3 {
    Vec3::new(
        cell.0 + 0.5 - gw as f32 * 0.5,
        cell.1 + 0.5 - gh as f32 * 0.5,
        0.0,
    )
}

/// An agent's smoothly-interpolated world position at fractional tick `t`, or
/// `None` if it isn't alive then.
fn agent_pos(agent: &Agent, t: f32, gw: u32, gh: u32) -> Option<Vec3> {
    if t < agent.spawn_tick as f32 || t >= agent.despawn_tick as f32 {
        return None;
    }
    let mut cell = (agent.start[0], agent.start[1]);
    let mut prev_tick = agent.spawn_tick as f32;
    let mut next: Option<&Step> = None;
    for step in &agent.steps {
        if step.tick as f32 <= t {
            cell.0 += step.dx;
            cell.1 += step.dy;
            prev_tick = step.tick as f32;
        } else {
            next = Some(step);
            break;
        }
    }
    let here = (cell.0 as f32, cell.1 as f32);
    let world = match next {
        Some(step) => {
            let target = ((cell.0 + step.dx) as f32, (cell.1 + step.dy) as f32);
            let span = (step.tick as f32 - prev_tick).max(1.0);
            let f = ((t - prev_tick) / span).clamp(0.0, 1.0);
            let eased = f * f * (3.0 - 2.0 * f); // smoothstep glide
            cell_to_world(here, gw, gh).lerp(cell_to_world(target, gw, gh), eased)
        }
        None => cell_to_world(here, gw, gh),
    };
    Some(world)
}

/// Spawns circles that should be alive and despawns ones that shouldn't — so the
/// looping clock naturally clears the field at the wrap and refills it.
fn reconcile_circles(
    mut commands: Commands,
    score: Res<TitleScore>,
    clock: Res<TitleClock>,
    assets: Res<CircleAssets>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    live: Query<(Entity, &CircleAgent)>,
) {
    let t = clock.0;
    let mut present = vec![false; score.0.agents.len()];
    for (_, agent) in &live {
        if let Some(slot) = present.get_mut(agent.0) {
            *slot = true;
        }
    }
    for (i, agent) in score.0.agents.iter().enumerate() {
        let alive = t >= agent.spawn_tick as f32 && t < agent.despawn_tick as f32;
        if alive && !present[i] {
            let pos = agent_pos(agent, t, score.0.grid_w, score.0.grid_h).unwrap_or(Vec3::ZERO);
            // Own material per circle, starting transparent — `animate_circles`
            // fades it in. Dropped on despawn, so materials don't accumulate.
            let material = materials.add(StandardMaterial {
                base_color: assets.color.with_alpha(0.0),
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                ..default()
            });
            commands.spawn((
                CircleAgent(i),
                Mesh3d(assets.mesh.clone()),
                MeshMaterial3d(material),
                Transform::from_translation(pos),
                DespawnOnExit(AppState::Title),
            ));
        }
    }
    if !live.is_empty() {
        for (entity, agent) in &live {
            let a = &score.0.agents[agent.0];
            let alive = t >= a.spawn_tick as f32 && t < a.despawn_tick as f32;
            if !alive {
                commands.entity(entity).despawn();
            }
        }
    }
}

/// Glides every live circle to its interpolated position AND fades it in on
/// spawn / out as it reaches its exit edge — so circles dissolve, never pop.
fn animate_circles(
    score: Res<TitleScore>,
    clock: Res<TitleClock>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut circles: Query<(
        &CircleAgent,
        &mut Transform,
        &MeshMaterial3d<StandardMaterial>,
    )>,
) {
    let t = clock.0;
    for (agent, mut transform, mat) in &mut circles {
        let a = &score.0.agents[agent.0];
        if let Some(pos) = agent_pos(a, t, score.0.grid_w, score.0.grid_h) {
            transform.translation = pos;
        }
        if let Some(material) = materials.get_mut(&mat.0) {
            material.base_color.set_alpha(fade(a, t));
        }
    }
}

/// Eased presence in `0..=1`: ramps up over the first [`FADE_TICKS`] of life and
/// down over the last, so a circle dissolves in/out instead of popping.
fn fade(agent: &Agent, t: f32) -> f32 {
    let fade_in = ((t - agent.spawn_tick as f32) / FADE_TICKS).clamp(0.0, 1.0);
    let fade_out = ((agent.despawn_tick as f32 - t) / FADE_TICKS).clamp(0.0, 1.0);
    fade_in.min(fade_out)
}

/// The grid the choreography plays on (cells), for the caller's camera framing.
pub(crate) fn grid_dims() -> (u32, u32) {
    let score = parse_score();
    (score.grid_w, score.grid_h)
}

/// Registers the playback systems (title state only).
pub(crate) fn add_playback_systems(app: &mut App) {
    app.add_systems(
        Update,
        (advance_clock, reconcile_circles, animate_circles)
            .chain()
            .run_if(in_state(AppState::Title)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// The baked score parses, has 40 agents, and — reconstructing every circle's
    /// CELL at every tick — never overlaps and stays in bounds. Locks the
    /// generator's guarantee into the gate and proves the runtime schema matches
    /// the file.
    #[test]
    fn baked_choreography_parses_and_never_overlaps() {
        let c = parse_score();
        assert_eq!(c.agents.len(), 40, "expected 40 choreographed circles");

        let cell_at = |a: &Agent, t: u32| -> Option<(i32, i32)> {
            if t < a.spawn_tick || t >= a.despawn_tick {
                return None;
            }
            let mut cell = (a.start[0], a.start[1]);
            for step in &a.steps {
                if step.tick <= t {
                    cell.0 += step.dx;
                    cell.1 += step.dy;
                } else {
                    break;
                }
            }
            Some(cell)
        };

        for t in 0..c.cycle_ticks {
            let mut seen: HashMap<(i32, i32), usize> = HashMap::new();
            for (i, agent) in c.agents.iter().enumerate() {
                let Some(cell) = cell_at(agent, t) else {
                    continue;
                };
                assert!(
                    cell.0 >= 0
                        && cell.0 < c.grid_w as i32
                        && cell.1 >= 0
                        && cell.1 < c.grid_h as i32,
                    "agent {i} out of bounds {cell:?} at tick {t}"
                );
                if let Some(&j) = seen.get(&cell) {
                    panic!("overlap: agents {i} and {j} share {cell:?} at tick {t}");
                }
                seen.insert(cell, i);
            }
        }
    }

    /// No blank title screen: the longest run of ticks with zero circles alive
    /// must stay tiny (the score starts on the first beat ≈ tick 8 and the tail
    /// lingerers reach the cycle end, so the only gap is the sub-beat lead-in).
    /// Guards against the "opens on a blank white sheet" regression.
    #[test]
    fn baked_choreography_has_no_blank_window() {
        const MAX_BLANK_TICKS: u32 = 30; // 0.5 s
        let c = parse_score();
        let alive_at = |t: u32| {
            c.agents
                .iter()
                .any(|a| t >= a.spawn_tick && t < a.despawn_tick)
        };
        let mut run = 0u32;
        let mut worst = 0u32;
        for t in 0..c.cycle_ticks {
            run = if alive_at(t) { 0 } else { run + 1 };
            worst = worst.max(run);
        }
        assert!(
            worst <= MAX_BLANK_TICKS,
            "blank window of {worst} ticks (> {MAX_BLANK_TICKS}) — the title would show an empty white screen"
        );
    }
}
