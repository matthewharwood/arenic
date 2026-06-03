//! The **Guildmaster** — a flat disc token.
//!
//! Authored in Blender at true game scale (a `0.25`-unit-diameter disc = one
//! tile, per `_docs/UNITS_AND_SCALE.md` §1) and exported to
//! `assets/models/guildmaster.glb`. This crate owns only the *piece* — the
//! marker component and how to spawn the model. Scene dressing (camera, lights,
//! ground) belongs to whoever stages it (e.g. the storybook story).

use bevy::prelude::*;

/// Marks a spawned Guildmaster.
#[derive(Component, Debug, Clone, Copy)]
pub struct Guildmaster;

/// glTF scene path for the disc (Bevy labels the first scene `#Scene0`).
pub const MODEL: &str = "models/guildmaster.glb#Scene0";

/// A bundle that spawns the Guildmaster disc **as authored**: already at true
/// scale (1 unit = 1 m), lying flat in the XZ plane (normal `+Y`) after glTF's
/// Z-up → Y-up conversion — i.e. resting on the ground. Add a [`Transform`] to
/// position it.
///
/// For the top-down game board (board on XY, camera on `+Z`) rotate `+90°`
/// about X at the call site so the disc faces the camera (UNITS_AND_SCALE §5);
/// a 3D diorama like the storybook uses it as-is.
pub fn guildmaster(assets: &AssetServer) -> impl Bundle {
    (Guildmaster, SceneRoot(assets.load(MODEL)))
}
