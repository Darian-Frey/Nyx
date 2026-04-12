//! Per-sample inspection: `.inspect()` calls a closure on each sample
//! while staying on the audio thread.
//!
//! Useful for debugging, logging peak levels, or driving custom meters
//! without the overhead of a ring buffer.

use crate::signal::{AudioContext, Signal};

/// A signal wrapper that calls a closure on each sample.
pub struct Inspect<A: Signal, F: FnMut(f32, &AudioContext) + Send> {
    source: A,
    func: F,
}

impl<A: Signal, F: FnMut(f32, &AudioContext) + Send> Signal for Inspect<A, F> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let sample = self.source.next(ctx);
        (self.func)(sample, ctx);
        sample
    }
}

/// Extension trait adding `.inspect()` to all signals.
pub trait InspectExt: Signal + Sized {
    /// Call a closure on each sample without modifying the signal.
    ///
    /// The closure receives `(sample_value, &AudioContext)` and runs
    /// on the audio thread. **Do not allocate or block inside it.**
    ///
    /// ```ignore
    /// let mut peak = 0.0_f32;
    /// signal.inspect(move |s, _ctx| peak = peak.max(s.abs()))
    /// ```
    fn inspect<F: FnMut(f32, &AudioContext) + Send>(self, func: F) -> Inspect<Self, F> {
        Inspect { source: self, func }
    }
}

impl<T: Signal + Sized> InspectExt for T {}
