//! Hot-swap crossfade engine.
//!
//! Manages the transition between an old signal chain and a new one
//! with a smooth crossfade to prevent clicks and pops.

use crate::signal::{AudioContext, Signal};

/// A signal wrapper that crossfades from one signal to another.
///
/// When a new signal is loaded, the old one fades out while the new
/// one fades in over `crossfade_samples` samples.
pub struct HotSwap {
    current: Box<dyn Signal>,
    incoming: Option<Box<dyn Signal>>,
    fade_pos: usize,
    fade_len: usize,
}

impl HotSwap {
    /// Create a new hot-swap engine with an initial signal.
    ///
    /// `crossfade_ms` is the crossfade duration in milliseconds.
    /// `sample_rate` is used to convert ms to samples.
    pub fn new(initial: Box<dyn Signal>, crossfade_ms: f32, sample_rate: f32) -> Self {
        HotSwap {
            current: initial,
            incoming: None,
            fade_pos: 0,
            fade_len: (crossfade_ms * 0.001 * sample_rate) as usize,
        }
    }

    /// Load a new signal. The old signal will fade out while the
    /// new one fades in.
    pub fn swap(&mut self, new_signal: Box<dyn Signal>) {
        // If we're already mid-crossfade, snap to the incoming and start a new fade.
        if let Some(incoming) = self.incoming.take() {
            self.current = incoming;
        }
        self.incoming = Some(new_signal);
        self.fade_pos = 0;
    }

    /// Returns `true` if a crossfade is currently in progress.
    pub fn is_crossfading(&self) -> bool {
        self.incoming.is_some()
    }
}

impl Signal for HotSwap {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        if let Some(ref mut incoming) = self.incoming {
            let old_sample = self.current.next(ctx);
            let new_sample = incoming.next(ctx);

            self.fade_pos += 1;
            let t = if self.fade_len > 0 {
                (self.fade_pos as f32 / self.fade_len as f32).min(1.0)
            } else {
                1.0
            };

            let mixed = old_sample * (1.0 - t) + new_sample * t;

            // Crossfade complete — swap in the new signal.
            if self.fade_pos >= self.fade_len {
                self.current = self.incoming.take().unwrap();
                self.fade_pos = 0;
            }

            mixed
        } else {
            self.current.next(ctx)
        }
    }
}
