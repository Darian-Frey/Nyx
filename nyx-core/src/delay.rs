//! Delay line with feedback.
//!
//! The foundational effect for echo, chorus, flanger, ping-pong,
//! Karplus-Strong, and comb filtering. A single allocation at
//! construction time (main thread, before `play()`); zero allocation
//! per sample in the audio callback.
//!
//! All three parameters — time, feedback, and mix — accept `IntoParam`,
//! so they can be modulated by LFOs or other signals. Each is smoothed
//! by a one-pole filter (~5 ms) to prevent zipper noise on rapid change.
//!
//! # Example
//!
//! ```ignore
//! use nyx_prelude::*;
//!
//! // A saw bass with a 3/8-beat echo, 40% feedback, 30% wet
//! let signal = osc::saw(220.0)
//!     .delay(0.375)
//!     .feedback(0.4)
//!     .mix(0.3);
//! ```
//!
//! # Why not `const N: usize`?
//!
//! Buffer length in samples depends on sample rate, which isn't known
//! at compile time. Const-generic delay types would either hardcode a
//! sample rate or force users to propagate the const through every
//! downstream combinator. `Box<[f32]>` allocated once at construction
//! (main thread, before the stream starts) is the standard approach
//! used by `fundsp`, `Tone.js`, and DAW plugins.
//!
//! # Maximum delay time
//!
//! `.delay(secs)` allocates a buffer sized for `secs` at the highest
//! sample rate we expect to ever see ([`DELAY_MAX_SR`] = 96 kHz). If you
//! intend to modulate the delay time beyond the initial value, call
//! `.max_time(secs)` with the upper bound. Delay times that exceed the
//! buffer capacity are silently clamped.

use crate::param::{ConstSignal, IntoParam, Param};
use crate::signal::{AudioContext, Signal};

/// Conservative upper bound on sample rate used to size the delay
/// buffer. Covers 44.1, 48, 88.2, and 96 kHz streams.
pub const DELAY_MAX_SR: f32 = 96_000.0;

/// Maximum permitted feedback amount. Values > 1.0 produce infinite
/// gain; this clamp keeps the delay bounded even under misuse.
pub const MAX_FEEDBACK: f32 = 0.95;

/// One-pole parameter smoother time constant (approx 5 ms).
const SMOOTH_TIME_SECS: f32 = 0.005;

/// A delay line with feedback and wet/dry mix.
///
/// Four type parameters carry the source type plus the signal types
/// used for each modulatable parameter. You'll almost never need to
/// write these out explicitly — the type system infers everything from
/// the builder chain.
pub struct Delay<S: Signal, PT: Signal, PF: Signal, PM: Signal> {
    input: S,
    buffer: Box<[f32]>,
    write_idx: usize,
    time_param: Param<PT>,
    feedback_param: Param<PF>,
    mix_param: Param<PM>,
    smoothed_time_samples: f32,
    smoothed_feedback: f32,
    smoothed_mix: f32,
    smooth_coeff: f32,
    initialised: bool,
}

/// Construct a new delay line. Use via [`SignalExt::delay`](crate::SignalExt::delay).
pub(crate) fn new_delay<S: Signal>(
    input: S,
    time_secs: f32,
) -> Delay<S, ConstSignal, ConstSignal, ConstSignal> {
    let time_secs = time_secs.max(0.0);
    let max_samples = buffer_samples_for(time_secs);
    Delay {
        input,
        buffer: vec![0.0; max_samples].into_boxed_slice(),
        write_idx: 0,
        time_param: Param::Static(time_secs),
        feedback_param: Param::Static(0.0),
        mix_param: Param::Static(0.5),
        smoothed_time_samples: 0.0,
        smoothed_feedback: 0.0,
        smoothed_mix: 0.5,
        smooth_coeff: 0.0,
        initialised: false,
    }
}

fn buffer_samples_for(time_secs: f32) -> usize {
    // +1 so read interpolation always has two valid neighbours.
    // Minimum 2 so an empty delay is still a valid ring buffer.
    ((time_secs * DELAY_MAX_SR).ceil() as usize + 1).max(2)
}

impl<S: Signal, PT: Signal, PF: Signal, PM: Signal> Delay<S, PT, PF, PM> {
    /// Reallocate the internal buffer to support delay times up to `secs`.
    ///
    /// Use when you intend to modulate delay time beyond the value passed
    /// to `.delay()`. Call on the main thread before playback — this
    /// allocates.
    pub fn max_time(mut self, secs: f32) -> Self {
        let new_samples = buffer_samples_for(secs.max(0.0));
        if new_samples > self.buffer.len() {
            let mut new_buf = vec![0.0; new_samples].into_boxed_slice();
            // Preserve existing buffer contents at their current indices.
            for (i, &v) in self.buffer.iter().enumerate() {
                new_buf[i] = v;
            }
            self.buffer = new_buf;
        }
        self
    }

    /// Set the delay time in seconds (static `f32` or a modulating `Signal`).
    pub fn time<P: IntoParam>(self, secs: P) -> Delay<S, P::Signal, PF, PM> {
        Delay {
            input: self.input,
            buffer: self.buffer,
            write_idx: self.write_idx,
            time_param: secs.into_param(),
            feedback_param: self.feedback_param,
            mix_param: self.mix_param,
            smoothed_time_samples: self.smoothed_time_samples,
            smoothed_feedback: self.smoothed_feedback,
            smoothed_mix: self.smoothed_mix,
            smooth_coeff: self.smooth_coeff,
            initialised: self.initialised,
        }
    }

    /// Set the feedback amount. Internally clamped to `[0.0, 0.95]`.
    pub fn feedback<P: IntoParam>(self, amount: P) -> Delay<S, PT, P::Signal, PM> {
        Delay {
            input: self.input,
            buffer: self.buffer,
            write_idx: self.write_idx,
            time_param: self.time_param,
            feedback_param: amount.into_param(),
            mix_param: self.mix_param,
            smoothed_time_samples: self.smoothed_time_samples,
            smoothed_feedback: self.smoothed_feedback,
            smoothed_mix: self.smoothed_mix,
            smooth_coeff: self.smooth_coeff,
            initialised: self.initialised,
        }
    }

    /// Set the wet/dry mix: `0.0` = all dry (bypass), `1.0` = all wet.
    pub fn mix<P: IntoParam>(self, wet: P) -> Delay<S, PT, PF, P::Signal> {
        Delay {
            input: self.input,
            buffer: self.buffer,
            write_idx: self.write_idx,
            time_param: self.time_param,
            feedback_param: self.feedback_param,
            mix_param: wet.into_param(),
            smoothed_time_samples: self.smoothed_time_samples,
            smoothed_feedback: self.smoothed_feedback,
            smoothed_mix: self.smoothed_mix,
            smooth_coeff: self.smooth_coeff,
            initialised: self.initialised,
        }
    }

    /// Linear read from the ring buffer at a fractional offset.
    fn read_interpolated(&self, delay_samples: f32) -> f32 {
        let buf_len = self.buffer.len();
        let buf_len_f = buf_len as f32;
        // Clamp to a legal range: at least 0 (no delay) and at most
        // buf_len - 1 (one sample shy of wrapping onto the write head).
        let delay_samples = delay_samples.clamp(0.0, buf_len_f - 1.0);
        let read_pos = self.write_idx as f32 - delay_samples;
        let read_pos = if read_pos < 0.0 {
            read_pos + buf_len_f
        } else {
            read_pos
        };
        let idx0 = read_pos.floor() as usize % buf_len;
        let idx1 = (idx0 + 1) % buf_len;
        let frac = read_pos - read_pos.floor();
        self.buffer[idx0] * (1.0 - frac) + self.buffer[idx1] * frac
    }
}

impl<S: Signal, PT: Signal, PF: Signal, PM: Signal> Signal for Delay<S, PT, PF, PM> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        // Lazy init of smoothing state once we know the stream sample rate.
        if !self.initialised {
            let tau_samples = SMOOTH_TIME_SECS * ctx.sample_rate;
            self.smooth_coeff = if tau_samples > 0.0 {
                1.0 - (-1.0 / tau_samples).exp()
            } else {
                1.0
            };
            self.smoothed_time_samples = self.time_param.next(ctx) * ctx.sample_rate;
            self.smoothed_feedback = self.feedback_param.next(ctx).clamp(0.0, MAX_FEEDBACK);
            self.smoothed_mix = self.mix_param.next(ctx).clamp(0.0, 1.0);
            self.initialised = true;
        }

        // Smooth each parameter toward its target.
        let target_time_samples = self.time_param.next(ctx) * ctx.sample_rate;
        self.smoothed_time_samples +=
            self.smooth_coeff * (target_time_samples - self.smoothed_time_samples);
        let target_fb = self.feedback_param.next(ctx).clamp(0.0, MAX_FEEDBACK);
        self.smoothed_feedback += self.smooth_coeff * (target_fb - self.smoothed_feedback);
        let target_mix = self.mix_param.next(ctx).clamp(0.0, 1.0);
        self.smoothed_mix += self.smooth_coeff * (target_mix - self.smoothed_mix);

        let input = self.input.next(ctx);
        let delayed = self.read_interpolated(self.smoothed_time_samples);
        let to_write = input + delayed * self.smoothed_feedback;
        self.buffer[self.write_idx] = to_write;
        self.write_idx = (self.write_idx + 1) % self.buffer.len();

        input * (1.0 - self.smoothed_mix) + delayed * self.smoothed_mix
    }
}
