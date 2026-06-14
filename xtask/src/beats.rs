//! `cargo xtask beats` — detect the title theme's tempo + beat grid and bake it
//! to `assets/title/beats.ron`. The choreography generator
//! (`cargo xtask choreograph`) reads it so the circles enter on the beat.

use anyhow::{Context, Result};

use crate::dsp;
use crate::score::Beats;

const THEME: &str = "assets/music/arenic_theme.mp3";
const OUT: &str = "assets/title/beats.ron";
const TICKS_PER_SECOND: f64 = 60.0;
/// Tempo search window (BPM). The prior in `dsp` centres confidence at 120.
const RANGE: (f64, f64) = (70.0, 180.0);

pub fn run() -> Result<()> {
    let path = std::path::Path::new(THEME);
    println!("decoding {THEME} …");
    let (mono, sr) = dsp::decode_mono(path)?;
    let duration = mono.len() as f64 / sr as f64;

    let (raw, fps) = dsp::onset_envelope(&mono, sr);
    let env = dsp::enhance(&raw, fps);

    let autocorr = dsp::estimate_autocorr(&env, fps, RANGE);
    let comb = dsp::estimate_comb(&env, fps, RANGE);
    let ioi = dsp::estimate_ioi(&env, fps, RANGE);
    let bpm = dsp::consensus(autocorr, comb, ioi);

    let times = dsp::beat_times(&env, fps, bpm);
    let beat_ticks: Vec<u32> = times
        .iter()
        .map(|t| (t * TICKS_PER_SECOND).round() as u32)
        .collect();
    let bar_ticks: Vec<u32> = beat_ticks.iter().step_by(4).copied().collect();

    let beats = Beats {
        bpm,
        duration_secs: duration,
        ticks_per_second: TICKS_PER_SECOND as u32,
        beat_ticks,
        bar_ticks,
    };

    std::fs::create_dir_all("assets/title").context("create assets/title")?;
    let ron = ron::ser::to_string_pretty(&beats, ron::ser::PrettyConfig::default())
        .context("serialize beats")?;
    std::fs::write(OUT, ron).with_context(|| format!("write {OUT}"))?;

    println!(
        "theme: {duration:.1}s  envelope {} frames @ {fps:.1} fps",
        env.len()
    );
    println!(
        "tempo  autocorr {autocorr:>5.1} | comb {comb:>5.1} | ioi {ioi:>5.1}  →  consensus {bpm:.1} BPM"
    );
    println!(
        "wrote {OUT}: {} beats, {} bars",
        beats.beat_ticks.len(),
        beats.bar_ticks.len()
    );
    Ok(())
}
