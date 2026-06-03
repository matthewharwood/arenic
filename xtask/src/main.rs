//! Repo automation, run as `cargo xtask <command>`.
//!
//! The home for real build logic — so the repo needs no shell scripts and `just`
//! stays a thin verb layer. Add a new command as a `match` arm + a function.
//!
//! Commands:
//! - `icons` — regenerate the Lucide icon PNGs in `assets/icons/` (pure-Rust
//!   fetch + rasterize; no `curl`/`librsvg`).

use anyhow::{Context, Result, bail};
use tiny_skia::{Pixmap, Transform};
use usvg::{Options, Tree};

/// The Lucide icons we ship. Add a name here, then `cargo xtask icons`.
const ICONS: &[&str] = &[
    "panel-left-close",
    "panel-left-open",
    "chevron-right",
    "chevron-down",
    "folder",
    "folder-open",
    "palette",
    "swatch-book",
    "type",
    "ruler",
    "box",
    "layers",
    "component",
    "user",
    "rotate-3d",
];

const SRC: &str = "https://raw.githubusercontent.com/lucide-icons/lucide/main/icons";
/// Raster size in px (Lucide's 24px viewBox is upscaled to this).
const SIZE: u32 = 96;

fn main() -> Result<()> {
    match std::env::args().nth(1).as_deref() {
        Some("icons") => icons(),
        other => bail!("usage: cargo xtask icons  (got {other:?})"),
    }
}

/// Fetches every icon, recolours it white, rasterizes to a transparent PNG, and
/// writes it to `assets/icons/<name>.png`.
fn icons() -> Result<()> {
    let out = std::path::Path::new("assets/icons");
    std::fs::create_dir_all(out).context("create assets/icons")?;
    let agent = ureq::Agent::new_with_defaults();

    for name in ICONS {
        let png = rasterize(&agent, &format!("{SRC}/{name}.svg"))
            .with_context(|| format!("icon {name}"))?;
        std::fs::write(out.join(format!("{name}.png")), png)?;
        println!("icon {name:<18} -> assets/icons/{name}.png");
    }
    println!("done: {} icons", ICONS.len());
    Ok(())
}

/// Downloads one SVG, replaces `currentColor` with white (so `ImageNode.color`
/// can tint it), and renders it centered in a `SIZE`×`SIZE` transparent PNG.
fn rasterize(agent: &ureq::Agent, url: &str) -> Result<Vec<u8>> {
    let svg = agent
        .get(url)
        .call()
        .with_context(|| format!("GET {url}"))?
        .body_mut()
        .read_to_string()
        .context("read response body")?
        .replace("currentColor", "#ffffff");

    let tree = Tree::from_str(&svg, &Options::default()).context("parse SVG")?;

    // Aspect-preserving fit into the square, centred.
    let size = tree.size();
    let scale = (SIZE as f32 / size.width()).min(SIZE as f32 / size.height());
    let tx = (SIZE as f32 - size.width() * scale) / 2.0;
    let ty = (SIZE as f32 - size.height() * scale) / 2.0;
    let transform = Transform::from_scale(scale, scale).post_translate(tx, ty);

    // `Pixmap::new` zero-fills → fully transparent background.
    let mut pixmap = Pixmap::new(SIZE, SIZE).context("allocate pixmap")?;
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    pixmap.encode_png().context("encode PNG")
}
