//! Huovilainen non-linear 4-pole ladder lowpass filter.
//!
//! The canonical "analog synth lowpass" sound: four cascaded one-pole
//! lowpass stages with global feedback through a saturating `tanh`.
//! Every stage has its own `tanh` applied to both its input and state,
//! which is what gives the filter its characteristic non-linear warmth
//! and the resonance "thinning" at high `k` that defines Moog-style
//! filters.
//!
//! - **Rolloff**: `-24 dB/oct`.
//! - **Resonance** `0.0 .. ≈ 1.2`. At `resonance ≈ 1.0` the filter
//!   self-oscillates at the cutoff frequency — the classic acid-bass
//!   squeal.
//! - **DC gain drops with resonance** — the canonical Moog behaviour.
//!   Approximate DC gain ≈ `1 / (1 + 4·resonance)`: at resonance=0
//!   it's unity, at resonance=0.5 it's ⅓, at resonance=1 it's ⅕. To
//!   compensate, scale input or add `.amp(1.0 + 4.0 * resonance)`
//!   upstream.
//! - **Cost**: 9 `tanh` per sample. At 44.1 kHz this is ≈ 400 k
//!   evaluations/s, comfortable on any modern CPU.
//!
//! The implementation uses the "unit-delay-corrected" form (not ZDF):
//! the feedback reads the previous sample's output. Tuning error is
//! negligible below `sr / 4` and unnoticeable in musical use.
//!
//! ```ignore
//! use nyx_core::{osc, LadderExt, SignalExt};
//!
//! // Acid bass: squelchy saw through a resonant ladder.
//! let lfo = osc::sine(0.3).amp(400.0).offset(800.0);
//! let acid = osc::saw_bl(55.0).amp(0.6).ladder_lp(lfo, 1.05);
//! ```
//!
//! References:
//! - Stilson & Smith, *Analyzing the Moog VCF*, CCRMA 1996.
//! - Huovilainen, *Nonlinear Digital Implementation of the Moog Ladder
//!   Filter*, DAFx 2004.

use crate::param::{IntoParam, Param};
use crate::signal::{AudioContext, Signal};

/// 4-pole non-linear ladder lowpass.
///
/// Wraps any [`Signal`] and filters it with modulatable `cutoff` /
/// `resonance` params. See the [module docs][crate::ladder] for the
/// algorithm and references.
pub struct Ladder<A: Signal, C: Signal, R: Signal> {
    source: A,
    cutoff: Param<C>,
    resonance: Param<R>,
    s1: f32,
    s2: f32,
    s3: f32,
    s4: f32,
    // Previous sample's output, used for the feedback term.
    last_out: f32,
}

// Minimum cutoff clamp — one-pole integrator below ~10 Hz starts to
// struggle with f32 precision. Also avoids `g = 0` degenerate case.
const LADDER_MIN_CUTOFF_HZ: f32 = 20.0;
// Maximum cutoff clamp as fraction of sample rate. Beyond this the
// one-pole approximation breaks down and the filter loses resonance
// character (and can become unstable at extreme k).
const LADDER_MAX_CUTOFF_FRAC: f32 = 0.45;
// Resonance clamp. 1.0 is self-oscillation threshold; 1.2 gives a
// loud, still-stable squeal. Higher values saturate musically but
// can spike on transients.
const LADDER_MAX_RESONANCE: f32 = 1.2;

impl<A: Signal, C: Signal, R: Signal> Signal for Ladder<A, C, R> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let input = self.source.next(ctx);

        // Clamp cutoff and resonance to safe ranges.
        let cutoff = self.cutoff.next(ctx).clamp(
            LADDER_MIN_CUTOFF_HZ,
            ctx.sample_rate * LADDER_MAX_CUTOFF_FRAC,
        );
        let resonance = self.resonance.next(ctx).clamp(0.0, LADDER_MAX_RESONANCE);

        // One-pole integrator coefficient.
        let g = 1.0 - (-std::f32::consts::TAU * cutoff / ctx.sample_rate).exp();
        // Feedback gain. k = 4 puts the loop at unity at DC → self-osc.
        let k = 4.0 * resonance;

        // Feedback: subtract a saturated copy of the previous output.
        // `tanh` on the feedback is what keeps the loop bounded when
        // self-oscillating.
        let u = input - k * self.last_out.tanh();

        // Four cascaded one-pole LPs, each with tanh on input and state.
        // The per-stage tanh is the source of Moog-style non-linear
        // "thickening" at high resonance.
        let t_u = u.tanh();
        let t1 = self.s1.tanh();
        let t2 = self.s2.tanh();
        let t3 = self.s3.tanh();
        let t4 = self.s4.tanh();

        self.s1 += g * (t_u - t1);
        self.s2 += g * (t1 - t2);
        self.s3 += g * (t2 - t3);
        self.s4 += g * (t3 - t4);

        self.last_out = self.s4;
        // Output saturation: tanh keeps the signal bounded through
        // self-oscillation transients, where s4 can briefly exceed ±1.
        self.s4.tanh()
    }
}

/// Adds `.ladder_lp(cutoff, resonance)` to every [`Signal`].
pub trait LadderExt: Signal + Sized {
    /// Apply a non-linear Moog-style 4-pole lowpass filter.
    ///
    /// - `cutoff` — in Hz. Clamped to `[20, sr * 0.45]`.
    /// - `resonance` — `0.0 .. 1.2`. Self-oscillates at `≥ 1.0`.
    ///
    /// Both parameters are modulatable — pass an `f32` for a static
    /// setting or a [`Signal`] for audio-rate sweep.
    fn ladder_lp<C: IntoParam, R: IntoParam>(
        self,
        cutoff: C,
        resonance: R,
    ) -> Ladder<Self, C::Signal, R::Signal> {
        Ladder {
            source: self,
            cutoff: cutoff.into_param(),
            resonance: resonance.into_param(),
            s1: 0.0,
            s2: 0.0,
            s3: 0.0,
            s4: 0.0,
            last_out: 0.0,
        }
    }
}

impl<T: Signal + Sized> LadderExt for T {}
