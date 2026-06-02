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

pub mod default_font;
pub mod ui;
