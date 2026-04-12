//! Dynamics processors: gain and peak limiter.
//!
//! Hard clip and soft clip (tanh) are already available as combinators
//! via `SignalExt::clip()` and `SignalExt::soft_clip()`.

use crate::param::{IntoParam, Param};
use crate::signal::{AudioContext, Signal};

/// Gain processor — multiplies signal by a (possibly modulated) amount.
///
/// This is functionally the same as `.amp()`, provided here as a named
/// struct for clarity in more complex signal graphs.
pub struct Gain<A: Signal, S: Signal> {
    source: A,
    gain: Param<S>,
}

/// Create a gain processor.
pub fn gain<A: Signal, P: IntoParam>(source: A, amount: P) -> Gain<A, P::Signal> {
    Gain {
        source,
        gain: amount.into_param(),
    }
}

impl<A: Signal, S: Signal> Signal for Gain<A, S> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        self.source.next(ctx) * self.gain.next(ctx)
    }
}

/// Peak limiter with attack/release envelope following.
///
/// Uses a simple feed-forward design: measures the peak level, applies
/// gain reduction when the signal exceeds the threshold, and smooths
/// the gain envelope to avoid clicks.
pub struct PeakLimiter<A: Signal> {
    source: A,
    threshold: f32,
    attack_coeff: f32,
    release_coeff: f32,
    envelope: f32,
}

/// Create a peak limiter.
///
/// - `threshold`: maximum output level (e.g. 1.0 for 0 dBFS)
/// - `attack_ms`: how quickly the limiter engages (typ. 0.1–1.0 ms)
/// - `release_ms`: how quickly the limiter releases (typ. 50–200 ms)
/// - `sample_rate`: needed for coefficient calculation
pub fn peak_limiter<A: Signal>(
    source: A,
    threshold: f32,
    attack_ms: f32,
    release_ms: f32,
    sample_rate: f32,
) -> PeakLimiter<A> {
    PeakLimiter {
        source,
        threshold,
        attack_coeff: coeff(attack_ms, sample_rate),
        release_coeff: coeff(release_ms, sample_rate),
        envelope: 0.0,
    }
}

fn coeff(time_ms: f32, sample_rate: f32) -> f32 {
    let samples = time_ms * 0.001 * sample_rate;
    if samples <= 0.0 {
        return 1.0;
    }
    1.0 - (-1.0 / samples).exp()
}

impl<A: Signal> Signal for PeakLimiter<A> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let input = self.source.next(ctx);
        let level = input.abs();

        // Envelope follower with asymmetric attack/release.
        let c = if level > self.envelope {
            self.attack_coeff
        } else {
            self.release_coeff
        };
        self.envelope += c * (level - self.envelope);

        // Compute gain reduction.
        if self.envelope > self.threshold {
            input * (self.threshold / self.envelope)
        } else {
            input
        }
    }
}
