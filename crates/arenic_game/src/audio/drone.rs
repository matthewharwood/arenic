//! The **overworld drone** — the soothing low hum that *is* the overworld's
//! soundtrack, generated entirely from baked-in oscillators (no audio asset).
//!
//! Three detuned sine voices (a 55 Hz root, its octave, and a +0.7 Hz detuned
//! twin that beats slowly against it) breathe under a very slow LFO, over a
//! soft low-passed noise bed — the "almost white noise" that smooths the
//! distant arena sounds when the camera is far up. All DSP happens inside the
//! rodio [`Source`] iterator: pure Rust, allocation-free per sample, fixed
//! noise seed (deterministic), so it behaves identically on native and web.

use std::f32::consts::TAU;
use std::time::Duration;

use bevy::audio::{Decodable, Source};
use bevy::math::ops;
use bevy::prelude::*;

const SAMPLE_RATE: u32 = 44_100;
/// The oscillator stack: `(frequency Hz, gain)`. 55 Hz is A1; the 110.7 Hz
/// voice beats at ~0.7 Hz against the octave — the slow "alive" shimmer.
const VOICES: [(f32, f32); 3] = [(55.0, 0.5), (110.0, 0.3), (110.7, 0.2)];
/// One breath every ~14 seconds.
const LFO_HZ: f32 = 0.07;
/// How deep the breath swells the stack (±15%).
const LFO_DEPTH: f32 = 0.15;
/// Level of the low-passed noise bed relative to the stack.
const NOISE_GAIN: f32 = 0.08;
/// One-pole low-pass cutoff for the noise bed — dark enough to soothe.
const NOISE_CUTOFF_HZ: f32 = 400.0;
/// Headroom so the drone never competes with the spatial SFX riding over it.
const MASTER: f32 = 0.7;

/// The procedural drone asset. Add one with `Assets<DroneSource>::add` and
/// play it like any audio: `AudioPlayer(handle)` (the tuple form — `new()` is
/// hard-wired to file-backed `AudioSource`).
#[derive(Asset, TypePath, Default)]
pub struct DroneSource;

/// The infinite sample iterator behind [`DroneSource`]. Phase accumulators
/// wrap at `TAU` so precision never degrades over a long session.
pub struct DroneDecoder {
    phases: [f32; VOICES.len()],
    steps: [f32; VOICES.len()],
    lfo_phase: f32,
    lfo_step: f32,
    noise_state: u32,
    noise_lp: f32,
    noise_alpha: f32,
}

impl DroneDecoder {
    fn new() -> Self {
        let step = |hz: f32| TAU * hz / SAMPLE_RATE as f32;
        Self {
            phases: [0.0; VOICES.len()],
            steps: VOICES.map(|(hz, _)| step(hz)),
            lfo_phase: 0.0,
            lfo_step: step(LFO_HZ),
            // Fixed seed: the drone is bit-identical every run.
            noise_state: 0x9E37_79B9,
            noise_lp: 0.0,
            noise_alpha: 1.0 - ops::exp(-TAU * NOISE_CUTOFF_HZ / SAMPLE_RATE as f32),
        }
    }

    /// xorshift32 white noise in `[-1, 1]` — allocation-free, deterministic.
    fn white(&mut self) -> f32 {
        let mut x = self.noise_state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.noise_state = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

impl Iterator for DroneDecoder {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let mut stack = 0.0;
        for (phase, (step, (_, gain))) in self
            .phases
            .iter_mut()
            .zip(self.steps.iter().zip(VOICES.iter()))
        {
            *phase += *step;
            if *phase > TAU {
                *phase -= TAU;
            }
            stack += ops::sin(*phase) * gain;
        }
        self.lfo_phase += self.lfo_step;
        if self.lfo_phase > TAU {
            self.lfo_phase -= TAU;
        }
        let breath = 1.0 + LFO_DEPTH * ops::sin(self.lfo_phase);
        let white = self.white();
        self.noise_lp += self.noise_alpha * (white - self.noise_lp);
        let sample = (stack * breath + self.noise_lp * NOISE_GAIN) * MASTER;
        Some(sample.clamp(-1.0, 1.0))
    }
}

impl Source for DroneDecoder {
    fn current_frame_len(&self) -> Option<usize> {
        None // infinite
    }

    fn channels(&self) -> u16 {
        1
    }

    fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }

    fn total_duration(&self) -> Option<Duration> {
        None // loops forever
    }
}

impl Decodable for DroneSource {
    type DecoderItem = f32;
    type Decoder = DroneDecoder;

    fn decoder(&self) -> Self::Decoder {
        DroneDecoder::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two seconds of drone: every sample finite and inside [-1, 1], the
    /// stream deterministic across decoders, and actually audible (the RMS
    /// catches a future edit that silences a voice or blows the headroom).
    #[test]
    fn drone_is_bounded_audible_and_deterministic() {
        let two_secs = SAMPLE_RATE as usize * 2;
        let a: Vec<f32> = DroneSource.decoder().take(two_secs).collect();
        let b: Vec<f32> = DroneSource.decoder().take(two_secs).collect();
        assert_eq!(a, b, "fixed-seed drone must be deterministic");
        assert!(a.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
        let rms = (a.iter().map(|s| s * s).sum::<f32>() / a.len() as f32).sqrt();
        assert!(rms > 0.05, "drone too quiet to hear: rms {rms}");
        assert!(rms < MASTER, "drone eating all its headroom: rms {rms}");
    }
}
