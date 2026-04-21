//! Time-travel automation: `signal.follow(|t| expr)`.
//!
//! An automation curve is a `Signal` whose output is determined by a
//! user-supplied function of time (in seconds). This enables expressive
//! parameter sweeps without allocating breakpoint arrays.

use nyx_core::{AudioContext, Signal};

/// An automation curve driven by a closure over time in seconds.
///
/// Created via `automation(closure)` or the `.follow()` extension method.
pub struct Automation<F: FnMut(f32) -> f32 + Send> {
    func: F,
}

/// Create an automation signal from a time-to-value function.
///
/// The closure receives elapsed time in seconds and returns a value.
///
/// ```ignore
/// // Linear ramp from 0 to 1 over 2 seconds
/// let ramp = automation::automation(|t| (t / 2.0).min(1.0));
///
/// // Sine LFO at 0.5 Hz
/// let lfo = automation::automation(|t| (t * std::f32::consts::TAU * 0.5).sin());
/// ```
pub fn automation<F: FnMut(f32) -> f32 + Send>(func: F) -> Automation<F> {
    Automation { func }
}

impl<F: FnMut(f32) -> f32 + Send> Signal for Automation<F> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let time_secs = ctx.tick as f32 / ctx.sample_rate;
        (self.func)(time_secs)
    }
}

/// A signal whose output is the source signal transformed by an automation function.
///
/// Created via `.follow()`.
pub struct Follow<A: Signal, F: FnMut(f32) -> f32 + Send> {
    source: A,
    func: F,
}

impl<A: Signal, F: FnMut(f32) -> f32 + Send> Signal for Follow<A, F> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let time_secs = ctx.tick as f32 / ctx.sample_rate;
        let automation_value = (self.func)(time_secs);
        self.source.next(ctx) * automation_value
    }
}

/// Extension trait adding `.follow()` to all signals.
pub trait AutomationExt: Signal + Sized {
    /// Multiply this signal's output by a time-based automation curve.
    ///
    /// ```ignore
    /// // Fade in over 2 seconds
    /// osc::sine(440).follow(|t| (t / 2.0).min(1.0))
    /// ```
    fn follow<F: FnMut(f32) -> f32 + Send>(self, func: F) -> Follow<Self, F> {
        Follow { source: self, func }
    }
}

impl<T: Signal + Sized> AutomationExt for T {}
