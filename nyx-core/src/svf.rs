//! State-Variable Filter (SVF) — Andy Simper ZDF topology.
//!
//! A single SVF core computes low-pass, band-pass, high-pass, and
//! notch outputs simultaneously from two state variables. Nyx exposes
//! four builder methods that pick which output mode you want:
//!
//! - [`FilterExt::svf_lp`](crate::FilterExt::svf_lp) — low-pass
//! - [`FilterExt::svf_hp`](crate::FilterExt::svf_hp) — high-pass
//! - [`FilterExt::svf_bp`](crate::FilterExt::svf_bp) — band-pass
//! - [`FilterExt::svf_notch`](crate::FilterExt::svf_notch) — notch (band-reject)
//!
//! # Why SVF alongside biquad?
//!
//! Biquad filters ([`Biquad`](crate::Biquad)) are the textbook choice
//! for static filtering. But when `cutoff` or `Q` modulates rapidly,
//! biquads need external coefficient smoothing to avoid clicks (which
//! Nyx applies at a ~5 ms time constant) — and the smoothing itself
//! limits how fast the filter can track fast LFOs.
//!
//! The zero-delay-feedback (ZDF) SVF reformulates the difference
//! equations so that per-sample parameter changes behave correctly
//! without smoothing. You can sweep the cutoff at audio rate with no
//! zipper, which makes the SVF the go-to filter for:
//!
//! - Wobble/growl basses with fast LFOs
//! - Vocal-style formant sweeps
//! - FM-modulated filter cutoffs
//! - Bandpass / notch effects (modes biquad doesn't expose today)
//!
//! The topology is Andy Simper's "Linear Trapezoidal State Variable
//! Filter" (2013), widely used in modern soft synths (Surge XT, Vital,
//! etc.).

use crate::param::Param;
use crate::signal::{AudioContext, Signal};

/// Output mode for an [`Svf`] filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SvfMode {
    /// Attenuates frequencies above cutoff.
    LowPass,
    /// Attenuates frequencies below cutoff.
    HighPass,
    /// Passes a narrow band around cutoff, attenuating the rest.
    BandPass,
    /// Attenuates a narrow band around cutoff, passing the rest.
    Notch,
}

/// Zero-delay-feedback state-variable filter.
///
/// Construct via the methods on [`FilterExt`](crate::FilterExt):
/// `.svf_lp()`, `.svf_hp()`, `.svf_bp()`, `.svf_notch()`.
pub struct Svf<S: Signal, C: Signal, Q: Signal> {
    source: S,
    cutoff: Param<C>,
    q: Param<Q>,
    mode: SvfMode,
    ic1: f32, // state variable 1 (bandpass integrator)
    ic2: f32, // state variable 2 (lowpass integrator)
}

impl<S: Signal, C: Signal, Q: Signal> Svf<S, C, Q> {
    pub(crate) fn new(source: S, cutoff: Param<C>, q: Param<Q>, mode: SvfMode) -> Self {
        Svf {
            source,
            cutoff,
            q,
            mode,
            ic1: 0.0,
            ic2: 0.0,
        }
    }
}

impl<S: Signal, C: Signal, Q: Signal> Signal for Svf<S, C, Q> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        // Sanitise inputs. Cutoff clamped to [20 Hz, Nyquist × 0.45]
        // (leaving a margin below Nyquist for numerical stability).
        let cutoff = self.cutoff.next(ctx).clamp(20.0, ctx.sample_rate * 0.45);
        let q = self.q.next(ctx).max(0.5);

        // Precompute ZDF coefficients — cheap trig per sample, but the
        // whole point of SVF over biquad is being able to do this.
        let g = (std::f32::consts::PI * cutoff / ctx.sample_rate).tan();
        let k = 1.0 / q;
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;

        let input = self.source.next(ctx);

        // Trapezoidal integrator update
        let v3 = input - self.ic2;
        let v1 = a1 * self.ic1 + a2 * v3;
        let v2 = self.ic2 + a2 * self.ic1 + a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;

        match self.mode {
            SvfMode::LowPass => v2,
            SvfMode::HighPass => input - k * v1 - v2,
            SvfMode::BandPass => v1,
            SvfMode::Notch => input - k * v1,
        }
    }
}
