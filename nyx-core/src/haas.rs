//! Haas-effect stereo widener.
//!
//! The Haas effect creates stereo width from a mono source by delaying
//! one channel by a short time (~5–30 ms). The human auditory system
//! localises the sound toward the earlier-arriving channel while still
//! perceiving the full signal from both — so we hear width without a
//! sense of echo.
//!
//! This is the classic pop-mix widening trick for lead vocals, rhythm
//! guitars, pads — anything that feels flat in mono.
//!
//! # Example
//!
//! ```ignore
//! use nyx_prelude::*;
//!
//! // 15 ms delay on the right channel
//! let signal = osc::saw(220.0).haas(15.0);
//! play(signal).unwrap();
//! ```
//!
//! # Mono compatibility
//!
//! When summed to mono (left + right), a Haas-widened signal produces
//! a subtle comb filter because the two channels are a delayed copy of
//! the same source. The audible effect on mono playback is a mild
//! high-frequency dip. Usually inaudible at typical widths; exaggerate
//! by shortening the delay if you want a more audible comb.

use crate::signal::{AudioContext, Signal};

/// Which channel gets the delay. `Right` is the typical choice (lead
/// vocals, solo instruments); `Left` can feel "backwards" and is
/// occasionally used for contrast on already-panned parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HaasSide {
    /// Delay the left channel; right plays in time.
    Left,
    /// Delay the right channel; left plays in time.
    Right,
}

/// Haas-effect stereo widener. One channel is delayed by a fixed number
/// of samples (set at construction from the delay time in milliseconds).
pub struct Haas<A: Signal> {
    source: A,
    buffer: Box<[f32]>,
    write_idx: usize,
    delay_samples: usize,
    side: HaasSide,
}

impl<A: Signal> Haas<A> {
    pub(crate) fn new(source: A, delay_ms: f32, side: HaasSide) -> Self {
        // Use a conservative 96 kHz upper bound for buffer sizing so
        // the delay fits regardless of stream sample rate.
        let max_sr = 96_000.0_f32;
        let ms = delay_ms.clamp(0.1, 50.0);
        let max_samples = ((ms * 0.001 * max_sr).ceil() as usize + 1).max(2);
        let delay_samples = (ms * 0.001 * 44_100.0).round() as usize;
        Haas {
            source,
            buffer: vec![0.0; max_samples].into_boxed_slice(),
            write_idx: 0,
            delay_samples,
            side,
        }
    }
}

impl<A: Signal> Signal for Haas<A> {
    /// Mono fold — summing L+R folds the delay into a mild comb filter.
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let (l, r) = self.next_stereo(ctx);
        l + r
    }

    fn next_stereo(&mut self, ctx: &AudioContext) -> (f32, f32) {
        let input = self.source.next(ctx);
        let buf_len = self.buffer.len();
        let read_idx = (self.write_idx + buf_len - self.delay_samples.min(buf_len - 1)) % buf_len;
        let delayed = self.buffer[read_idx];
        self.buffer[self.write_idx] = input;
        self.write_idx = (self.write_idx + 1) % buf_len;
        match self.side {
            HaasSide::Left => (delayed, input),
            HaasSide::Right => (input, delayed),
        }
    }
}
