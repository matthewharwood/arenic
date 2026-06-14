//! Pure-Rust DSP for `cargo xtask beats`.
//!
//! Pipeline: decode mp3 → mono f64 (symphonia) → a **spectral-flux onset
//! envelope** (hand-rolled radix-2 FFT, no extra deps) → **three independent
//! tempo estimators** (autocorrelation, comb-filter, inter-onset-interval
//! histogram). Their consensus is our confidence: we can't ear-check the BPM,
//! so agreement across methods stands in for it. Then a beat-phase alignment
//! turns the tempo into actual beat times.

use std::collections::HashMap;
use std::f64::consts::PI;
use std::path::Path;

use anyhow::{Context, Result, bail};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// STFT window (power of two for the radix-2 FFT).
const WIN: usize = 2048;
/// STFT hop — frame rate is `sample_rate / HOP` (~86 fps at 44.1 kHz).
const HOP: usize = 512;

/// Decodes an mp3 to mono f64 samples + its sample rate.
pub fn decode_mono(path: &Path) -> Result<(Vec<f64>, u32)> {
    let file = std::fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("mp3");
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .context("probe mp3 container")?;
    let mut format = probed.format;
    let track = format.default_track().context("no default audio track")?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .context("make mp3 decoder")?;

    let mut sample_rate = track.codec_params.sample_rate.unwrap_or(44_100);
    let mut mono: Vec<f64> = Vec::new();
    let mut buf: Option<SampleBuffer<f32>> = None;

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymError::IoError(_)) => break, // clean EOF
            Err(e) => return Err(e).context("read packet"),
        };
        if packet.track_id() != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = *decoded.spec();
                sample_rate = spec.rate;
                if buf.is_none() {
                    buf = Some(SampleBuffer::<f32>::new(decoded.capacity() as u64, spec));
                }
                let sb = buf.as_mut().expect("just created");
                sb.copy_interleaved_ref(decoded);
                let channels = spec.channels.count().max(1);
                for frame in sb.samples().chunks(channels) {
                    let sum: f32 = frame.iter().copied().sum();
                    mono.push((sum / channels as f32) as f64);
                }
            }
            Err(SymError::DecodeError(_)) => continue, // skip a corrupt frame
            Err(SymError::IoError(_)) => break,
            Err(e) => return Err(e).context("decode packet"),
        }
    }
    if mono.is_empty() {
        bail!("decoded no samples from {}", path.display());
    }
    Ok((mono, sample_rate))
}

/// In-place iterative radix-2 Cooley–Tukey forward FFT. `re`/`im` length must be
/// a power of two.
fn fft(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    debug_assert!(n.is_power_of_two());
    // Bit-reversal permutation.
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    // Butterflies.
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * PI / len as f64;
        let (wl_re, wl_im) = (ang.cos(), ang.sin());
        let half = len / 2;
        let mut base = 0;
        while base < n {
            let (mut w_re, mut w_im) = (1.0f64, 0.0f64);
            for k in 0..half {
                let a = base + k;
                let b = base + k + half;
                let t_re = re[b] * w_re - im[b] * w_im;
                let t_im = re[b] * w_im + im[b] * w_re;
                re[b] = re[a] - t_re;
                im[b] = im[a] - t_im;
                re[a] += t_re;
                im[a] += t_im;
                let nw_re = w_re * wl_re - w_im * wl_im;
                w_im = w_re * wl_im + w_im * wl_re;
                w_re = nw_re;
            }
            base += len;
        }
        len <<= 1;
    }
}

/// Spectral-flux onset envelope (sum of positive magnitude changes per frame)
/// + the envelope's frame rate in frames/second.
pub fn onset_envelope(mono: &[f64], sample_rate: u32) -> (Vec<f64>, f64) {
    // Hann window (= sin²).
    let hann: Vec<f64> = (0..WIN)
        .map(|i| {
            let s = (PI * i as f64 / (WIN as f64 - 1.0)).sin();
            s * s
        })
        .collect();
    let frames = if mono.len() >= WIN {
        (mono.len() - WIN) / HOP + 1
    } else {
        0
    };
    let mut env = Vec::with_capacity(frames);
    let mut prev = vec![0.0f64; WIN / 2 + 1];
    let mut re = vec![0.0f64; WIN];
    let mut im = vec![0.0f64; WIN];
    for f in 0..frames {
        let start = f * HOP;
        for i in 0..WIN {
            re[i] = mono[start + i] * hann[i];
            im[i] = 0.0;
        }
        fft(&mut re, &mut im);
        let mut flux = 0.0;
        for k in 0..=WIN / 2 {
            let mag = (re[k] * re[k] + im[k] * im[k]).sqrt();
            let d = mag - prev[k];
            if d > 0.0 {
                flux += d;
            }
            prev[k] = mag;
        }
        env.push(flux);
    }
    let fps = sample_rate as f64 / HOP as f64;
    (env, fps)
}

/// Peak-emphasis: subtract a ~0.15 s moving average and half-wave rectify, so
/// the envelope is near-zero between onsets and spikes on them.
pub fn enhance(env: &[f64], fps: f64) -> Vec<f64> {
    let w = ((fps * 0.15).round() as usize).max(1) | 1;
    let half = w / 2;
    let mut out = vec![0.0; env.len()];
    for i in 0..env.len() {
        let lo = i.saturating_sub(half);
        let hi = (i + half + 1).min(env.len());
        let mean = env[lo..hi].iter().sum::<f64>() / (hi - lo) as f64;
        out[i] = (env[i] - mean).max(0.0);
    }
    out
}

/// A gentle log-Gaussian tempo prior centred on 120 BPM (≈0.9 octave σ) —
/// nudges the estimators away from half/double-time octave errors.
fn tempo_prior(bpm: f64) -> f64 {
    (-0.5 * ((bpm / 120.0).ln() / 0.9).powi(2)).exp()
}

/// Estimator 1 — autocorrelation of the onset envelope, weighted by the prior.
pub fn estimate_autocorr(env: &[f64], fps: f64, range: (f64, f64)) -> f64 {
    let (min_bpm, max_bpm) = range;
    let min_lag = (60.0 * fps / max_bpm).floor() as usize;
    let max_lag = (60.0 * fps / min_bpm).ceil() as usize;
    let mut best = (f64::MIN, min_lag.max(1));
    for lag in min_lag.max(1)..=max_lag {
        let mut acc = 0.0;
        for i in lag..env.len() {
            acc += env[i] * env[i - lag];
        }
        let bpm = 60.0 * fps / lag as f64;
        let score = acc * tempo_prior(bpm);
        if score > best.0 {
            best = (score, lag);
        }
    }
    60.0 * fps / best.1 as f64
}

/// Estimator 2 — comb filter: like autocorrelation but summing the first three
/// harmonics of each lag, which sharpens the true period against its sub-octaves.
pub fn estimate_comb(env: &[f64], fps: f64, range: (f64, f64)) -> f64 {
    let (min_bpm, max_bpm) = range;
    let min_lag = (60.0 * fps / max_bpm).floor() as usize;
    let max_lag = (60.0 * fps / min_bpm).ceil() as usize;
    let mut best = (f64::MIN, min_lag.max(1));
    for lag in min_lag.max(1)..=max_lag {
        let mut acc = 0.0;
        for h in 1..=3usize {
            let l = lag * h;
            for i in l..env.len() {
                acc += env[i] * env[i - l];
            }
        }
        let bpm = 60.0 * fps / lag as f64;
        let score = acc * tempo_prior(bpm);
        if score > best.0 {
            best = (score, lag);
        }
    }
    60.0 * fps / best.1 as f64
}

/// Estimator 3 — inter-onset-interval histogram: pick onset peaks, histogram the
/// gaps between them (folded into the BPM range), take the mode.
pub fn estimate_ioi(env: &[f64], fps: f64, range: (f64, f64)) -> f64 {
    let mean = env.iter().sum::<f64>() / env.len().max(1) as f64;
    let var = env.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / env.len().max(1) as f64;
    let thr = mean + var.sqrt();
    let mut peaks = Vec::new();
    for i in 1..env.len().saturating_sub(1) {
        if env[i] > thr && env[i] >= env[i - 1] && env[i] > env[i + 1] {
            peaks.push(i);
        }
    }
    let (min_bpm, max_bpm) = range;
    let mut hist: HashMap<u32, f64> = HashMap::new();
    for w in peaks.windows(2) {
        let d = (w[1] - w[0]) as f64;
        if d <= 0.0 {
            continue;
        }
        let mut bpm = 60.0 * fps / d;
        while bpm > max_bpm {
            bpm /= 2.0;
        }
        while bpm < min_bpm {
            bpm *= 2.0;
        }
        *hist.entry(bpm.round() as u32).or_default() += 1.0;
    }
    hist.into_iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).expect("finite"))
        .map(|(bpm, _)| bpm as f64)
        .unwrap_or(120.0)
}

/// Folds the three estimates into one octave and averages whichever agree
/// (within 3 BPM); falls back to the autocorrelation estimate `a` (the most
/// reliable) when they don't.
pub fn consensus(a: f64, comb: f64, ioi: f64) -> f64 {
    let fold = |mut x: f64| {
        while x > a * 1.4 {
            x /= 2.0;
        }
        while x < a / 1.4 {
            x *= 2.0;
        }
        x
    };
    let comb = fold(comb);
    let ioi = fold(ioi);
    let close = |x: f64, y: f64| (x - y).abs() <= 3.0;
    if close(a, comb) && close(a, ioi) {
        (a + comb + ioi) / 3.0
    } else if close(a, comb) {
        (a + comb) / 2.0
    } else if close(a, ioi) {
        (a + ioi) / 2.0
    } else if close(comb, ioi) {
        (comb + ioi) / 2.0
    } else {
        a
    }
}

/// Beat times (seconds) for `bpm`: pick the phase that best aligns a pulse train
/// to onset energy, then walk the (fractional) period across the song so it
/// can't drift over two minutes.
pub fn beat_times(env: &[f64], fps: f64, bpm: f64) -> Vec<f64> {
    let period = (60.0 * fps / bpm).max(1.0);
    let p = period.round() as usize;
    let mut best = (f64::MIN, 0usize);
    for phase in 0..p {
        let mut acc = 0.0;
        let mut i = phase;
        while i < env.len() {
            acc += env[i];
            i += p;
        }
        if acc > best.0 {
            best = (acc, phase);
        }
    }
    let mut times = Vec::new();
    let mut t = best.1 as f64;
    while (t as usize) < env.len() {
        times.push(t / fps);
        t += period;
    }
    times
}
