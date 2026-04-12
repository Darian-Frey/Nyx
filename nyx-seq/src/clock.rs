//! BPM-based musical clock driven by `AudioContext.tick`.
//!
//! The clock converts absolute sample counts into musical time:
//! beats, bars, and fractional positions within a beat. BPM is
//! a `Param<S>` so tempo can be modulated by a signal.

use nyx_core::param::IntoParam;
use nyx_core::{AudioContext, Param, Signal};

/// A musical clock that tracks beat position from sample ticks.
///
/// The clock accumulates fractional beats each sample based on the
/// current BPM. This approach handles tempo changes smoothly —
/// including modulated BPM — without discontinuities.
pub struct Clock<S: Signal> {
    bpm: Param<S>,
    beats_per_bar: f32,
    beat_acc: f64, // accumulated beats (f64 for long-running precision)
}

/// Create a clock at the given BPM with 4 beats per bar.
pub fn clock<P: IntoParam>(bpm: P) -> Clock<P::Signal> {
    Clock {
        bpm: bpm.into_param(),
        beats_per_bar: 4.0,
        beat_acc: 0.0,
    }
}

impl<S: Signal> Clock<S> {
    /// Set the number of beats per bar (default 4).
    pub fn beats_per_bar(mut self, n: f32) -> Self {
        self.beats_per_bar = n;
        self
    }

    /// Advance the clock by one sample and return the current state.
    ///
    /// Call this once per sample in the audio callback. The returned
    /// `ClockState` gives you beat/bar positions for the *current* tick.
    pub fn tick(&mut self, ctx: &AudioContext) -> ClockState {
        let state = ClockState {
            beat: self.beat_acc as f32,
            bar: (self.beat_acc as f32) / self.beats_per_bar,
            phase_in_beat: (self.beat_acc % 1.0) as f32,
            phase_in_bar: ((self.beat_acc as f32) % self.beats_per_bar) / self.beats_per_bar,
            beats_per_bar: self.beats_per_bar,
        };

        // Advance accumulator by the fraction of a beat this sample represents.
        let bpm = self.bpm.next(ctx) as f64;
        let beats_per_sample = bpm / (60.0 * ctx.sample_rate as f64);
        self.beat_acc += beats_per_sample;

        state
    }

    /// Reset the clock to beat 0.
    pub fn reset(&mut self) {
        self.beat_acc = 0.0;
    }

    /// Snap a beat position to the nearest grid line.
    ///
    /// `grid` is in beats: 1.0 = quarter note, 0.5 = eighth note,
    /// 0.25 = sixteenth note, etc.
    pub fn snap(beat: f32, grid: f32) -> f32 {
        if grid <= 0.0 {
            return beat;
        }
        (beat / grid).round() * grid
    }
}

/// A snapshot of the clock's musical position at a given sample.
#[derive(Debug, Clone, Copy)]
pub struct ClockState {
    /// Total beats elapsed since the clock started (e.g. 4.5 = halfway through beat 5).
    pub beat: f32,
    /// Total bars elapsed (e.g. 1.125 = 1/8 of the way into bar 2).
    pub bar: f32,
    /// Fractional position within the current beat, in [0, 1).
    pub phase_in_beat: f32,
    /// Fractional position within the current bar, in [0, 1).
    pub phase_in_bar: f32,
    /// Beats per bar (for reference).
    pub beats_per_bar: f32,
}
