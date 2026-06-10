//! Shared game code for arenic.
//!
//! This crate holds the reusable pieces of the game — components, abilities,
//! widgets, and engine setup — that are consumed by *both* runnable binaries:
//!
//! - `arenic` — the real game.
//! - `arenic_storybook` — an isolated harness for building and exercising game
//!   pieces (e.g. dropping the Guildmaster on a canvas to test his abilities)
//!   outside the full game loop.
//!
//! The dependency only ever points one way: the binaries depend on this crate,
//! never the reverse. So anything a binary needs to share lives here and is
//! `pub`, rather than being duplicated.

pub mod ability;
pub mod arena;
pub mod atmosphere;
pub mod boss;
pub mod default_font;
pub mod grid;
pub mod guildmaster;
pub mod icon;
pub mod interaction;
pub mod orbit;
pub mod swarm;
pub mod theme;
pub mod tile;
pub mod ui;

pub use interaction::{InteractionPlugin, Interactive, UiFocus, hidden_outline};

use bevy::app::PluginGroupBuilder;
use bevy::asset::AssetMetaCheck;
use bevy::prelude::*;

/// `DefaultPlugins` tuned for both native and web hosting.
///
/// `AssetMetaCheck::Never` stops Bevy requesting `*.meta` sidecar files, which
/// 404 on static hosts like GitHub Pages (we ship none). On web, Bevy fetches
/// assets with a *relative* path (`assets/…`), so a build served from a subpath
/// (e.g. `/arenic/app/`) resolves them correctly with no `file_path` override.
///
/// Add binary-specific plugins (e.g. [`default_font::DefaultFontPlugin`]) *after*
/// this group.
pub fn default_plugins() -> PluginGroupBuilder {
    DefaultPlugins.set(AssetPlugin {
        meta_check: AssetMetaCheck::Never,
        ..default()
    })
}
