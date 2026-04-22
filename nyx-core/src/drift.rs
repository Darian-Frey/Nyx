//! Analog-oscillator drift.
//!
//! Produces a slow random wander around **1.0**, intended to multiply
//! an oscillator's frequency parameter so its pitch inherits the
//! slight instability that makes analog VCOs sound alive.
//!
//! The output is `2^(cents / 1200)` where `cents` is a smoothed
//! random walk bounded by `±amount_cents`. At construction the
//! internal target updates `rate_hz` times per second — typical
//! values: 0.1–1.0 Hz, a few cents of depth.
//!
//! ```ignore
//! use nyx_core::{osc, drift::drift, SignalExt};
//!
//! // A subtle-drift saw around 440 Hz.
//! let freq = drift(4.0, 0.3).amp(440.0);
//! let osc = osc::saw_bl(freq);
//! ```
//!
//! The output is centred at 1.0 so `.amp(base_freq)` yields a
//! `base_freq ± small` frequency signal. Use `.seed(...)` for
//! reproducible drift (important for live-diff reloads and tests).

use crate::signal::{AudioContext, Signal};

/// Default one-pole smoother time constant. 100 ms makes target jumps
/// feel like gentle analog wander rather than stepped noise.
const DRIFT_SMOOTH_TAU_SECS: f32 = 0.1;

/// Seeded xorshift32 default. Non-zero so the PRNG doesn't degenerate.
const DRIFT_DEFAULT_SEED: u32 = 0xC0FFEE12;

/// Signal producing a slow random wander in frequency multiplier form.
pub struct Drift {
    /// Current smoothed wander, in cents.
    state: f32,
    /// Current target wander, in cents.
    target: f32,
    /// Half-range of the random target, in cents.
    amount_cents: f32,
    /// How often (Hz) to pick a new target.
    rate_hz: f32,
    /// Samples elapsed since the last target pick.
    samples_since_pick: u32,
    /// Cached sample rate and derived per-sample coefficients.
    samples_between_picks: u32,
    smooth_alpha: f32,
    sr: f32,
    initialised: bool,
    /// xorshift32 PRNG state.
    rng_state: u32,
}

/// Create a drift signal.
///
/// - `amount_cents` — half-range of the wander (e.g. `5.0` ⇒ ±5 cents).
/// - `rate_hz` — how often to pick a new target (e.g. `0.3`).
///
/// Output is a frequency multiplier centred on 1.0. Pair with
/// `.amp(base_freq_hz)` to turn it into a frequency signal.
pub fn drift(amount_cents: f32, rate_hz: f32) -> Drift {
    Drift {
        state: 0.0,
        target: 0.0,
        amount_cents: amount_cents.max(0.0),
        rate_hz: rate_hz.max(0.0),
        samples_since_pick: 0,
        samples_between_picks: 0,
        smooth_alpha: 0.0,
        sr: 0.0,
        initialised: false,
        rng_state: DRIFT_DEFAULT_SEED,
    }
}

impl Drift {
    /// Set the PRNG seed for reproducible wander. `0` is auto-mapped
    /// to `1` so the xorshift state never degenerates.
    pub fn seed(mut self, seed: u32) -> Self {
        self.rng_state = if seed == 0 { 1 } else { seed };
        self
    }

    #[inline]
    fn next_uniform_signed(&mut self) -> f32 {
        let mut x = self.rng_state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng_state = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

impl Signal for Drift {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        if !self.initialised || self.sr != ctx.sample_rate {
            self.sr = ctx.sample_rate;
            // At least 1 sample between picks to avoid div-by-zero and
            // to guarantee the target update path runs on some cadence.
            self.samples_between_picks = (ctx.sample_rate / self.rate_hz.max(0.001))
                .max(1.0)
                .min(u32::MAX as f32) as u32;
            self.smooth_alpha = 1.0 - (-1.0 / (DRIFT_SMOOTH_TAU_SECS * ctx.sample_rate)).exp();
            // Pick an initial target immediately so drift starts moving
            // from sample 0 — otherwise at low `rate_hz` we'd sit at
            // the zero initial state for tens of thousands of samples.
            self.target = self.next_uniform_signed() * self.amount_cents;
            self.samples_since_pick = 0;
            self.initialised = true;
        }

        // Pick a new target periodically.
        if self.samples_since_pick >= self.samples_between_picks {
            self.target = self.next_uniform_signed() * self.amount_cents;
            self.samples_since_pick = 0;
        }
        self.samples_since_pick = self.samples_since_pick.saturating_add(1);

        // Smooth current state toward target.
        self.state += self.smooth_alpha * (self.target - self.state);

        // Convert cents to frequency multiplier: 2^(cents / 1200).
        (self.state / 1200.0 * std::f32::consts::LN_2).exp()
    }
}
