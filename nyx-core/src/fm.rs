//! FM (actually PM) operator — phase modulation of a sine carrier.
//!
//! "FM synthesis" as popularised by the Yamaha DX7 is really phase
//! modulation: the modulator signal is added to the carrier's phase
//! accumulator before the sine lookup. This produces the characteristic
//! metallic bell tones, electric pianos, brass, and tight basses that
//! define 80s synth sound.
//!
//! Formally, for each sample:
//!
//! ```text
//! output = sin(2π * (phase + index * modulator + feedback * last_output))
//! phase += carrier_freq / sample_rate
//! ```
//!
//! where `index` is the modulation depth (peak phase deviation in cycles
//! at the modulator's output amplitude). A modulation index of `0`
//! produces a pure sine carrier; `1` starts adding noticeable sidebands;
//! `3+` gives bells and bright timbres.
//!
//! # Construction
//!
//! ```ignore
//! use nyx_prelude::*;
//!
//! // Direct construction:
//! play(fm_op(440.0, osc::sine(880.0), 3.0)).unwrap();
//!
//! // Fluent from an existing sine:
//! play(osc::sine(440.0).fm(osc::sine(880.0), 3.0)).unwrap();
//! ```
//!
//! # Feedback
//!
//! Self-feedback routes the operator's own last output into its phase
//! modulator, producing sawtooth-like timbres as feedback approaches 1.0
//! (the DX7 "algorithm 1" feedback loop).
//!
//! ```ignore
//! fm_op(440.0, osc::sine(660.0), 2.0).feedback(0.5)
//! ```

use crate::param::{IntoParam, Param};
use crate::signal::{AudioContext, Signal};

/// FM (phase modulation) operator. One sine carrier with an arbitrary
/// modulator signal driving its phase.
///
/// Construct via [`fm_op`] or via `.fm()` on an `osc::sine(...)`.
pub struct FmOp<F: Signal, M: Signal, I: Signal> {
    carrier_freq: Param<F>,
    modulator: M,
    index: Param<I>,
    feedback: f32,
    phase: f32,
    last_out: f32,
}

/// Build an FM operator at `freq`, modulated by `modulator`, with the
/// given `index` of modulation.
///
/// All three parameters accept `f32` or any `Signal`.
pub fn fm_op<F, M, I>(freq: F, modulator: M, index: I) -> FmOp<F::Signal, M, I::Signal>
where
    F: IntoParam,
    M: Signal,
    I: IntoParam,
{
    FmOp {
        carrier_freq: freq.into_param(),
        modulator,
        index: index.into_param(),
        feedback: 0.0,
        phase: 0.0,
        last_out: 0.0,
    }
}

impl<F: Signal, M: Signal, I: Signal> FmOp<F, M, I> {
    /// Set the self-feedback amount. Values near `0` produce clean sines;
    /// higher values progressively add harmonics (approaching a sawtooth
    /// at `feedback ≈ 1`). Clamped to `[-1, 1]`.
    pub fn feedback(mut self, amount: f32) -> Self {
        self.feedback = amount.clamp(-1.0, 1.0);
        self
    }

    /// Internal constructor used by `Sine::fm()` to preserve the
    /// carrier's existing phase state.
    pub(crate) fn from_sine_parts(
        carrier_freq: Param<F>,
        modulator: M,
        index: Param<I>,
        phase: f32,
    ) -> Self {
        FmOp {
            carrier_freq,
            modulator,
            index,
            feedback: 0.0,
            phase,
            last_out: 0.0,
        }
    }
}

impl<F: Signal, M: Signal, I: Signal> Signal for FmOp<F, M, I> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let freq = self.carrier_freq.next(ctx);
        let mod_sample = self.modulator.next(ctx);
        let index = self.index.next(ctx);

        // Phase modulation: phase + index * modulator + feedback loop.
        let modulated = self.phase + index * mod_sample + self.feedback * self.last_out;
        let out = (modulated * std::f32::consts::TAU).sin();

        self.phase += freq / ctx.sample_rate;
        self.phase -= self.phase.floor();
        self.last_out = out;

        out
    }
}
