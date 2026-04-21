//! Granular synthesis — plays many short "grains" from a source
//! [`Sample`](crate::sample::Sample) with jittered position, pitch, pan,
//! and amplitude to create clouds, drones, stretches, and smears.
//!
//! A `Granular` engine owns a fixed pool of grain voices (default 64)
//! allocated once at construction. A scheduler spawns new grains at the
//! configured density; each grain reads from the source with linear
//! interpolation, windowed by a Hann envelope, and panned to its own
//! position in the stereo field. Output is genuinely stereo — the
//! mono fold (`next`) sums the two channels, preserving energy.
//!
//! ```ignore
//! use nyx_prelude::*;
//!
//! let pad = Sample::load("pad.wav")?;
//! let cloud = Granular::new(pad)
//!     .grain_size(0.08)       // 80 ms
//!     .density(40.0)          // 40 grains/sec
//!     .position(0.4)          // read around 40% into the sample
//!     .position_jitter(0.15)
//!     .pitch_jitter(0.03)     // ±3 % pitch wobble
//!     .pan_spread(1.0);
//! play(cloud).unwrap();
//! ```
//!
//! **Real-time safety.** The grain pool and all scratch are allocated
//! once (heap) at construction time. No allocation happens in the
//! audio callback. The scheduler uses a local xorshift32 PRNG; no
//! syscalls, no locks, no I/O.
//!
//! **Voice stealing.** If the density × grain_size exceeds the voice
//! count, new grains are dropped rather than cutting off active ones.
//! Increase the pool with [`Granular::with_voices`] if you need more
//! overlap.

use std::sync::Arc;

use crate::sample::Sample;
use crate::signal::{AudioContext, Signal};

const DEFAULT_VOICES: usize = 64;

/// A single grain voice.
#[derive(Clone, Copy, Default)]
struct Grain {
    active: bool,
    /// Absolute index into the source buffer (float, for interpolation).
    source_pos: f32,
    /// Per-sample increment in source-index space. Combines the user's
    /// pitch multiplier, any per-grain pitch jitter, and the source/
    /// stream sample-rate ratio.
    source_inc: f32,
    /// Progress through the grain envelope, `0.0..1.0`.
    env_pos: f32,
    /// Per-sample envelope increment = `1 / grain_length_samples`.
    env_inc: f32,
    /// Amplitude for this grain.
    amp: f32,
    /// Pan position, `-1.0` (hard left) to `+1.0` (hard right).
    pan: f32,
}

/// Granular synthesis engine. Build with [`Granular::new`] and chain
/// builder methods.
pub struct Granular {
    data: Arc<[f32]>,
    sample_sr: f32,
    grains: Box<[Grain]>,

    grain_size_s: f32,
    density_hz: f32,
    position: f32,
    position_jitter: f32,
    pitch: f32,
    pitch_jitter: f32,
    pan_spread: f32,
    amp: f32,
    amp_jitter: f32,

    trigger_phase: f32,
    rng: u32,
}

impl Granular {
    /// Create a granular engine from a loaded sample, using the default
    /// 64-voice pool.
    pub fn new(sample: Sample) -> Self {
        Self::with_voices(sample, DEFAULT_VOICES)
    }

    /// Create a granular engine with an explicit voice-pool size.
    pub fn with_voices(sample: Sample, voices: usize) -> Self {
        let voices = voices.max(1);
        let grains: Vec<Grain> = vec![Grain::default(); voices];
        Self {
            data: sample.data_arc(),
            sample_sr: sample.sample_rate(),
            grains: grains.into_boxed_slice(),
            grain_size_s: 0.05,
            density_hz: 30.0,
            position: 0.5,
            position_jitter: 0.05,
            pitch: 1.0,
            pitch_jitter: 0.0,
            pan_spread: 0.5,
            amp: 0.8,
            amp_jitter: 0.0,
            trigger_phase: 0.0,
            rng: 0xA53C_9CDF,
        }
    }

    /// Grain length in seconds. Clamped to `>= 1 ms`. Default `0.05` (50 ms).
    pub fn grain_size(mut self, secs: f32) -> Self {
        self.grain_size_s = secs.max(0.001);
        self
    }

    /// Spawn rate in grains per second. `0.0` stops all new grains.
    /// Default `30.0`.
    pub fn density(mut self, hz: f32) -> Self {
        self.density_hz = hz.max(0.0);
        self
    }

    /// Read position in the source, as a fraction `0.0..1.0` of the
    /// sample length. Default `0.5` (middle).
    pub fn position(mut self, pos: f32) -> Self {
        self.position = pos.clamp(0.0, 1.0);
        self
    }

    /// Random spread around `position`, expressed as a fraction of the
    /// sample length. A value of `0.1` places grains within ±10 % of the
    /// set position. Default `0.05`.
    pub fn position_jitter(mut self, amount: f32) -> Self {
        self.position_jitter = amount.max(0.0);
        self
    }

    /// Playback rate multiplier (`1.0` = native pitch, `2.0` = octave up).
    /// Default `1.0`.
    pub fn pitch(mut self, rate: f32) -> Self {
        self.pitch = rate.max(0.0);
        self
    }

    /// Fractional pitch deviation per grain. `0.02` gives ±2 % pitch
    /// variation — a gentle chorus. Default `0.0`.
    pub fn pitch_jitter(mut self, amount: f32) -> Self {
        self.pitch_jitter = amount.max(0.0);
        self
    }

    /// Stereo spread. `0.0` = all grains centre-panned, `1.0` = grains
    /// distributed from hard-left to hard-right. Default `0.5`.
    pub fn pan_spread(mut self, amount: f32) -> Self {
        self.pan_spread = amount.clamp(0.0, 1.0);
        self
    }

    /// Base per-grain amplitude. Grains overlap and sum; keep this
    /// below 1.0 to leave headroom. Default `0.8`.
    pub fn amp(mut self, gain: f32) -> Self {
        self.amp = gain.max(0.0);
        self
    }

    /// Fractional per-grain amplitude variation. Default `0.0`.
    pub fn amp_jitter(mut self, amount: f32) -> Self {
        self.amp_jitter = amount.clamp(0.0, 1.0);
        self
    }

    /// Seed the internal PRNG for reproducible grain patterns.
    pub fn seed(mut self, seed: u32) -> Self {
        self.rng = seed.max(1);
        self
    }

    // ── internal helpers ────────────────────────────────────────────

    #[inline]
    fn next_u32(&mut self) -> u32 {
        // xorshift32
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        // Guard against zero state.
        if x == 0 {
            x = 0xCAFE_BABE;
        }
        self.rng = x;
        x
    }

    #[inline]
    fn next_bipolar(&mut self) -> f32 {
        // (-1, 1)
        (self.next_u32() as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    /// Find a free grain slot (or return None if all are busy).
    fn find_slot(&self) -> Option<usize> {
        self.grains.iter().position(|g| !g.active)
    }

    fn spawn_grain(&mut self, sr: f32) {
        let Some(idx) = self.find_slot() else {
            return;
        };

        let len_samples = self.grain_size_s * sr;
        if len_samples < 2.0 {
            return;
        }

        let data_len = self.data.len();
        if data_len < 4 {
            return;
        }

        // Randomised read position.
        let pos_rand = self.next_bipolar() * self.position_jitter;
        let pos_frac = (self.position + pos_rand).clamp(0.0, 1.0);
        let abs_pos = pos_frac * (data_len as f32 - 2.0);

        // Randomised playback rate.
        let pitch_rand = self.next_bipolar() * self.pitch_jitter;
        let rate = (self.pitch * (1.0 + pitch_rand)).max(0.0);

        // Randomised amplitude.
        let amp_rand = self.next_bipolar() * self.amp_jitter;
        let amp = (self.amp * (1.0 + amp_rand)).max(0.0);

        // Randomised pan.
        let pan = self.next_bipolar() * self.pan_spread;

        let g = &mut self.grains[idx];
        g.active = true;
        g.source_pos = abs_pos;
        g.source_inc = rate * self.sample_sr / sr;
        g.env_pos = 0.0;
        g.env_inc = 1.0 / len_samples;
        g.amp = amp;
        g.pan = pan;
    }

    /// Core render loop shared between mono and stereo paths.
    fn render(&mut self, sr: f32) -> (f32, f32) {
        // Scheduler: spawn grains per density.
        let trigger_inc = self.density_hz / sr;
        self.trigger_phase += trigger_inc;
        while self.trigger_phase >= 1.0 {
            self.trigger_phase -= 1.0;
            self.spawn_grain(sr);
        }

        let mut out_l = 0.0_f32;
        let mut out_r = 0.0_f32;
        let len = self.data.len();

        for g in self.grains.iter_mut() {
            if !g.active {
                continue;
            }

            let idx = g.source_pos;
            if !idx.is_finite() || idx < 0.0 || idx as usize + 1 >= len {
                g.active = false;
                continue;
            }

            // Linear interpolation into the source.
            let i = idx as usize;
            let frac = idx - i as f32;
            let s = self.data[i] * (1.0 - frac) + self.data[i + 1] * frac;

            // Hann envelope: 0.5 · (1 − cos(2πt)).
            let w = 0.5 - 0.5 * (g.env_pos * std::f32::consts::TAU).cos();

            let sample = s * w * g.amp;

            // Simple linear pan law, matching crate::signal::Pan conventions.
            let l_gain = (1.0 - g.pan) * 0.5;
            let r_gain = (1.0 + g.pan) * 0.5;
            out_l += sample * l_gain;
            out_r += sample * r_gain;

            // Advance.
            g.source_pos += g.source_inc;
            g.env_pos += g.env_inc;
            if g.env_pos >= 1.0 {
                g.active = false;
            }
        }

        (out_l, out_r)
    }
}

impl Signal for Granular {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let (l, r) = self.render(ctx.sample_rate);
        l + r
    }

    fn next_stereo(&mut self, ctx: &AudioContext) -> (f32, f32) {
        self.render(ctx.sample_rate)
    }
}
