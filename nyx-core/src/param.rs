use crate::signal::{AudioContext, Signal};

/// A parameter that is either a fixed value or modulated by a `Signal`.
///
/// Every processor parameter (frequency, cutoff, gain, …) accepts `Param<S>`
/// so that `osc.lowpass(800.0)` and `osc.lowpass(lfo)` both compile.
pub enum Param<S: Signal> {
    Static(f32),
    Modulated(S),
}

impl<S: Signal> Param<S> {
    /// Resolve the current value: returns the static value unchanged,
    /// or pulls the next sample from the modulation source.
    pub fn next(&mut self, ctx: &AudioContext) -> f32 {
        match self {
            Param::Static(v) => *v,
            Param::Modulated(s) => s.next(ctx),
        }
    }
}

// --- Ergonomic conversions ---

impl From<f32> for Param<ConstSignal> {
    fn from(value: f32) -> Self {
        Param::Static(value)
    }
}

/// A trivial signal that always returns a constant value.
/// Used as the type parameter when converting from `f32`.
pub struct ConstSignal;

impl Signal for ConstSignal {
    fn next(&mut self, _ctx: &AudioContext) -> f32 {
        // Never actually called — `Param::Static` short-circuits.
        0.0
    }
}
