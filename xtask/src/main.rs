//! Repo automation, run as `cargo xtask <command>`.
//!
//! The home for real build logic — so the repo needs no shell scripts and `just`
//! stays a thin verb layer. Add a new command as a module + a `match` arm.
//!
//! Commands:
//! - `icons`       — regenerate the Lucide icon PNGs in `assets/icons/`.
//! - `beats`       — detect the title theme's tempo + beat grid → `assets/title/beats.ron`.
//! - `choreograph` — generate the title-screen circle choreography → `assets/title/choreography.vNNNN.ron`.

mod beats;
mod choreograph;
mod dsp;
mod icons;
mod score;

use anyhow::{Result, bail};

fn main() -> Result<()> {
    match std::env::args().nth(1).as_deref() {
        Some("icons") => icons::run(),
        Some("beats") => beats::run(),
        Some("choreograph") => choreograph::run(),
        other => bail!("usage: cargo xtask <icons|beats|choreograph>  (got {other:?})"),
    }
}
