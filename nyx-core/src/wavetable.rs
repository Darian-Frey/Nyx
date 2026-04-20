//! Wavetable oscillator — user-drawn or preset waveforms with linear
//! interpolation.
//!
//! A [`Wavetable`] holds one period of a waveform as `Arc<[f32]>` —
//! cheap to clone across many voices. A [`WavetableOsc`] reads through
//! the table at a rate determined by its frequency, interpolating
//! between adjacent table slots.
//!
//! # Example
//!
//! ```ignore
//! use nyx_prelude::*;
//!
//! // Preset sine table
//! let sine_table = Wavetable::sine(2048);
//! play(sine_table.freq(440.0)).unwrap();
//!
//! // User-drawn saw-plus-square hybrid
//! let custom = Wavetable::from_fn(2048, |t| {
//!     let saw = 2.0 * t - 1.0;
//!     let sq = if t < 0.5 { 1.0 } else { -1.0 };
//!     0.5 * saw + 0.5 * sq
//! });
//! play(custom.freq(220.0)).unwrap();
//! ```
//!
//! # Table sharing
//!
//! `Wavetable` is `Clone`-cheap (refcount bump only). Build a table
//! once on the main thread, then clone it into as many oscillator
//! voices as you need. The same lifetime caveat as [`Sample`] applies:
//! keep at least one `Wavetable` reference alive for as long as any
//! oscillator cloned from it is playing.
//!
//! [`Sample`]: crate::Sample

use std::sync::Arc;

use crate::param::{IntoParam, Param};
use crate::signal::{AudioContext, Signal};

/// Immutable shared wavetable data.
#[derive(Clone)]
pub struct Wavetable {
    table: Arc<[f32]>,
}

impl Wavetable {
    /// Build a wavetable from a slice of samples. Must be non-empty.
    pub fn new(data: &[f32]) -> Self {
        assert!(!data.is_empty(), "wavetable must have at least one sample");
        Wavetable {
            table: data.to_vec().into_boxed_slice().into(),
        }
    }

    /// Build from an owned `Vec<f32>`.
    pub fn from_vec(data: Vec<f32>) -> Self {
        assert!(!data.is_empty(), "wavetable must have at least one sample");
        Wavetable {
            table: data.into_boxed_slice().into(),
        }
    }

    /// Build from a function `f(t)` evaluated at `size` evenly spaced
    /// points with `t` in `[0, 1)`.
    ///
    /// ```ignore
    /// Wavetable::from_fn(2048, |t| (t * std::f32::consts::TAU).sin())
    /// ```
    pub fn from_fn<F: Fn(f32) -> f32>(size: usize, f: F) -> Self {
        assert!(size > 0, "wavetable size must be > 0");
        let data: Vec<f32> = (0..size).map(|i| f(i as f32 / size as f32)).collect();
        Self::from_vec(data)
    }

    /// A pure-sine wavetable at the given size.
    pub fn sine(size: usize) -> Self {
        Self::from_fn(size, |t| (t * std::f32::consts::TAU).sin())
    }

    /// A naive (non-band-limited) sawtooth wavetable.
    pub fn saw(size: usize) -> Self {
        Self::from_fn(size, |t| 2.0 * t - 1.0)
    }

    /// A naive (non-band-limited) square wavetable.
    pub fn square(size: usize) -> Self {
        Self::from_fn(size, |t| if t < 0.5 { 1.0 } else { -1.0 })
    }

    /// A triangle wavetable.
    pub fn triangle(size: usize) -> Self {
        Self::from_fn(size, |t| {
            if t < 0.5 {
                4.0 * t - 1.0
            } else {
                3.0 - 4.0 * t
            }
        })
    }

    /// Build an oscillator reading through this wavetable at the given
    /// frequency. Accepts `f32` or any `Signal` for modulated pitch.
    ///
    /// ```ignore
    /// let table = Wavetable::sine(2048);
    /// let osc = table.freq(440.0);
    /// ```
    pub fn freq<P: IntoParam>(&self, freq: P) -> WavetableOsc<P::Signal> {
        WavetableOsc {
            table: Arc::clone(&self.table),
            freq: freq.into_param(),
            phase: 0.0,
        }
    }

    /// Number of samples in the table.
    pub fn len(&self) -> usize {
        self.table.len()
    }

    /// Whether the table is empty (always `false` because constructors
    /// reject empty tables; provided for Clippy satisfaction).
    pub fn is_empty(&self) -> bool {
        self.table.is_empty()
    }
}

/// A wavetable-reading oscillator voice.
pub struct WavetableOsc<S: Signal> {
    table: Arc<[f32]>,
    freq: Param<S>,
    phase: f32,
}

impl<S: Signal> Signal for WavetableOsc<S> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let freq = self.freq.next(ctx);
        let len = self.table.len();
        // phase in [0,1) → fractional index into the table.
        let pos = self.phase * len as f32;
        let idx0 = (pos as usize) % len;
        let idx1 = (idx0 + 1) % len;
        let frac = pos - pos.floor();
        let out = self.table[idx0] * (1.0 - frac) + self.table[idx1] * frac;

        self.phase += freq / ctx.sample_rate;
        self.phase -= self.phase.floor();

        out
    }
}
