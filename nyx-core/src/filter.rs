//! Biquad filters — Transposed Direct Form II.
//!
//! Supports resonant low-pass and high-pass. All parameters accept
//! `IntoParam` so cutoff and Q can be modulated by signals.
//! Coefficient smoothing (one-pole) is mandatory to prevent clicks.

use crate::param::{IntoParam, Param};
use crate::signal::{AudioContext, Signal};

/// One-pole smoother for a single coefficient.
///
/// Converges to the target value exponentially. Default time constant
/// is ~5ms at 44100 Hz.
struct OnePoleSmoother {
    current: f32,
    coeff: f32, // smoothing coefficient in (0, 1)
}

impl OnePoleSmoother {
    fn new(initial: f32, time_ms: f32, sample_rate: f32) -> Self {
        Self {
            current: initial,
            coeff: Self::coeff_for(time_ms, sample_rate),
        }
    }

    fn coeff_for(time_ms: f32, sample_rate: f32) -> f32 {
        let samples = time_ms * 0.001 * sample_rate;
        if samples <= 0.0 {
            return 1.0;
        }
        1.0 - (-1.0 / samples).exp()
    }

    fn next(&mut self, target: f32) -> f32 {
        self.current += self.coeff * (target - self.current);
        self.current
    }
}

/// Biquad filter coefficients.
#[derive(Clone, Copy)]
struct BiquadCoeffs {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

impl BiquadCoeffs {
    fn lowpass(cutoff: f32, q: f32, sample_rate: f32) -> Self {
        let omega = std::f32::consts::TAU * cutoff / sample_rate;
        let sin_w = omega.sin();
        let cos_w = omega.cos();
        let alpha = sin_w / (2.0 * q);

        let a0 = 1.0 + alpha;
        let b0 = ((1.0 - cos_w) / 2.0) / a0;
        let b1 = (1.0 - cos_w) / a0;
        let b2 = b0;
        let a1 = (-2.0 * cos_w) / a0;
        let a2 = (1.0 - alpha) / a0;

        BiquadCoeffs { b0, b1, b2, a1, a2 }
    }

    fn highpass(cutoff: f32, q: f32, sample_rate: f32) -> Self {
        let omega = std::f32::consts::TAU * cutoff / sample_rate;
        let sin_w = omega.sin();
        let cos_w = omega.cos();
        let alpha = sin_w / (2.0 * q);

        let a0 = 1.0 + alpha;
        let b0 = ((1.0 + cos_w) / 2.0) / a0;
        let b1 = -(1.0 + cos_w) / a0;
        let b2 = b0;
        let a1 = (-2.0 * cos_w) / a0;
        let a2 = (1.0 - alpha) / a0;

        BiquadCoeffs { b0, b1, b2, a1, a2 }
    }
}

/// Filter mode.
#[derive(Clone, Copy)]
pub enum FilterMode {
    LowPass,
    HighPass,
}

/// Resonant biquad filter (Transposed Direct Form II) with coefficient smoothing.
pub struct Biquad<A: Signal, C: Signal, Q: Signal> {
    source: A,
    cutoff: Param<C>,
    q: Param<Q>,
    mode: FilterMode,
    // TDF-II state
    s1: f32,
    s2: f32,
    // Smoothers for each coefficient
    sm_b0: OnePoleSmoother,
    sm_b1: OnePoleSmoother,
    sm_b2: OnePoleSmoother,
    sm_a1: OnePoleSmoother,
    sm_a2: OnePoleSmoother,
    initialised: bool,
}

impl<A: Signal, C: Signal, Q: Signal> Signal for Biquad<A, C, Q> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let cutoff = self.cutoff.next(ctx);
        let q = self.q.next(ctx);

        let coeffs = match self.mode {
            FilterMode::LowPass => BiquadCoeffs::lowpass(cutoff, q, ctx.sample_rate),
            FilterMode::HighPass => BiquadCoeffs::highpass(cutoff, q, ctx.sample_rate),
        };

        // On first sample, snap smoothers to target (no ramp from zero).
        if !self.initialised {
            self.sm_b0 = OnePoleSmoother::new(coeffs.b0, 5.0, ctx.sample_rate);
            self.sm_b1 = OnePoleSmoother::new(coeffs.b1, 5.0, ctx.sample_rate);
            self.sm_b2 = OnePoleSmoother::new(coeffs.b2, 5.0, ctx.sample_rate);
            self.sm_a1 = OnePoleSmoother::new(coeffs.a1, 5.0, ctx.sample_rate);
            self.sm_a2 = OnePoleSmoother::new(coeffs.a2, 5.0, ctx.sample_rate);
            self.initialised = true;
        }

        let b0 = self.sm_b0.next(coeffs.b0);
        let b1 = self.sm_b1.next(coeffs.b1);
        let b2 = self.sm_b2.next(coeffs.b2);
        let a1 = self.sm_a1.next(coeffs.a1);
        let a2 = self.sm_a2.next(coeffs.a2);

        let input = self.source.next(ctx);

        // Transposed Direct Form II
        let output = b0 * input + self.s1;
        self.s1 = b1 * input - a1 * output + self.s2;
        self.s2 = b2 * input - a2 * output;

        output
    }
}

fn new_biquad<A: Signal, PC: IntoParam, PQ: IntoParam>(
    source: A,
    cutoff: PC,
    q: PQ,
    mode: FilterMode,
) -> Biquad<A, PC::Signal, PQ::Signal> {
    Biquad {
        source,
        cutoff: cutoff.into_param(),
        q: q.into_param(),
        mode,
        s1: 0.0,
        s2: 0.0,
        sm_b0: OnePoleSmoother::new(0.0, 5.0, 44100.0),
        sm_b1: OnePoleSmoother::new(0.0, 5.0, 44100.0),
        sm_b2: OnePoleSmoother::new(0.0, 5.0, 44100.0),
        sm_a1: OnePoleSmoother::new(0.0, 5.0, 44100.0),
        sm_a2: OnePoleSmoother::new(0.0, 5.0, 44100.0),
        initialised: false,
    }
}

/// Extension trait adding `.lowpass()` and `.highpass()` to all signals.
pub trait FilterExt: Signal + Sized {
    /// Apply a resonant low-pass filter.
    ///
    /// ```ignore
    /// osc::saw(220).lowpass(800.0, 0.707)   // static cutoff
    /// osc::saw(220).lowpass(lfo, 2.0)        // modulated cutoff
    /// ```
    fn lowpass<PC: IntoParam, PQ: IntoParam>(
        self,
        cutoff: PC,
        q: PQ,
    ) -> Biquad<Self, PC::Signal, PQ::Signal> {
        new_biquad(self, cutoff, q, FilterMode::LowPass)
    }

    /// Apply a resonant high-pass filter.
    fn highpass<PC: IntoParam, PQ: IntoParam>(
        self,
        cutoff: PC,
        q: PQ,
    ) -> Biquad<Self, PC::Signal, PQ::Signal> {
        new_biquad(self, cutoff, q, FilterMode::HighPass)
    }
}

impl<T: Signal + Sized> FilterExt for T {}
