//! Vinyl character — crackle impulses and hiss floor.
//!
//! These complete the lo-fi palette for anything that wants to sit in
//! the "old medium" aesthetic without needing a full tape chain.
//!
//! - [`crackle`] — a sparse random impulse stream filtered through a
//!   2 kHz resonator, producing the characteristic clicks and pops of
//!   dusty vinyl. `intensity` in `[0, 1]` controls both how often
//!   clicks fire and how loud they are.
//! - [`hiss`] — pink noise scaled to a given dB-FS level. Typical
//!   vinyl/tape noise floors sit between `−70` and `−50 dB`.
//!
//! ```ignore
//! use nyx_core::{osc, vinyl, SignalExt};
//!
//! // Mix vinyl ambience under a pad.
//! let pad = osc::saw_bl(220.0).amp(0.4)
//!     .add(vinyl::crackle(0.35))
//!     .add(vinyl::hiss(-58.0));
//! ```

use crate::osc::noise;
use crate::signal::{AudioContext, Signal, SignalExt};

/// Target centre frequency of the click resonator. 2 kHz sits in the
/// "pen tip on paper" register and reads as a click rather than a
/// transient.
const CRACKLE_FREQ_HZ: f32 = 2000.0;
/// Resonator pole radius. Closer to 1.0 = longer ring. 0.93 gives a
/// click tail of ≈ 10 ms at 44.1 kHz.
const CRACKLE_DECAY_R: f32 = 0.93;
/// Fire-rate scaling factor. With intensity=1.0 this gives ~4 clicks
/// per second on average — "well-loved" vinyl.
const CRACKLE_RATE_SCALE: f32 = 0.0001;
/// Impulse amplitude scaling. With intensity=1.0 a click peaks near
/// ±1.0 after the resonator's peak gain.
const CRACKLE_IMPULSE_GAIN: f32 = 0.3;

/// xorshift32 seeds. Kept distinct so simultaneously-active crackle
/// and hiss streams don't produce correlated output.
const CRACKLE_SEED: u32 = 0x71E1_7EA1;
const HISS_SEED: u32 = 0x141F_A5E1;

/// Random impulses through a 2 kHz resonator — vinyl-style clicks.
pub struct VinylCrackle {
    rng_state: u32,
    probability: f32,
    impulse_gain: f32,
    // Resonator state: y[n] = a1·y[n-1] + a2·y[n-2] + b0·x[n]
    y1: f32,
    y2: f32,
    a1: f32,
    a2: f32,
    b0: f32,
    sr: f32,
    initialised: bool,
}

/// Create a crackle source. `intensity ∈ [0, 1]`: `0` is silent,
/// `1.0` is "well-loved vinyl" (several clicks per second).
pub fn crackle(intensity: f32) -> VinylCrackle {
    let i = intensity.clamp(0.0, 1.0);
    VinylCrackle {
        rng_state: CRACKLE_SEED,
        probability: i * CRACKLE_RATE_SCALE,
        impulse_gain: i * CRACKLE_IMPULSE_GAIN,
        y1: 0.0,
        y2: 0.0,
        a1: 0.0,
        a2: 0.0,
        b0: 0.0,
        sr: 0.0,
        initialised: false,
    }
}

impl VinylCrackle {
    /// Set the PRNG seed for reproducible crackle patterns.
    pub fn seed(mut self, seed: u32) -> Self {
        self.rng_state = if seed == 0 { 1 } else { seed };
        self
    }

    #[inline]
    fn next_u32(&mut self) -> u32 {
        let mut x = self.rng_state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng_state = x;
        x
    }
}

impl Signal for VinylCrackle {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        if !self.initialised || self.sr != ctx.sample_rate {
            self.sr = ctx.sample_rate;
            let w0 = std::f32::consts::TAU * CRACKLE_FREQ_HZ / ctx.sample_rate;
            self.a1 = 2.0 * CRACKLE_DECAY_R * w0.cos();
            self.a2 = -(CRACKLE_DECAY_R * CRACKLE_DECAY_R);
            // (1 − r) input scaling keeps peak gain close to unity.
            self.b0 = 1.0 - CRACKLE_DECAY_R;
            self.initialised = true;
        }

        // Fire an impulse with `probability` chance. Using integer
        // comparison keeps this branch-friendly and allocation-free.
        let roll = self.next_u32();
        let threshold = (self.probability * u32::MAX as f32) as u32;
        let impulse = if roll < threshold {
            // Random polarity / amplitude.
            let amp = (self.next_u32() as f32 / u32::MAX as f32) * 2.0 - 1.0;
            amp * self.impulse_gain
        } else {
            0.0
        };

        // Two-pole resonator.
        let y = self.a1 * self.y1 + self.a2 * self.y2 + self.b0 * impulse;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

/// Pink-noise hiss floor at the given level in dBFS.
///
/// Typical values: `-70 dB` (subtle), `-55 dB` (noticeable), `-40 dB`
/// (very dusty). Uses a dedicated seed so a hiss mixed alongside
/// [`crackle`] doesn't correlate with it.
pub fn hiss(level_db: f32) -> impl Signal {
    let scale = 10.0_f32.powf(level_db / 20.0);
    noise::pink(HISS_SEED).amp(scale)
}
