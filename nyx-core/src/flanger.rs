//! Flanger — modulated short delay with feedback.
//!
//! Flanging is what you get when you make a chorus with a much shorter
//! base delay (0.5–10 ms) and crank the feedback up. The interference
//! between dry and wet produces a characteristic swooping comb-filter
//! sweep — the classic jet-plane "whoosh" of the 70s and 80s.
//!
//! Nyx's flanger produces real stereo via two LFOs offset by 180°
//! (same pattern as [`Chorus`](crate::chorus::Chorus)).
//!
//! # Example
//!
//! ```ignore
//! use nyx_prelude::*;
//!
//! // Heavy flange on a saw bass
//! let bass = osc::saw(110.0)
//!     .flanger(0.3, 2.0)    // 0.3 Hz, 2 ms depth
//!     .feedback(0.7);        // heavy swirl
//! play(bass).unwrap();
//! ```

use crate::signal::{AudioContext, Signal};

/// Max sample rate supported for buffer sizing.
const FLANGER_MAX_SR: f32 = 96_000.0;
/// Maximum delay time we ever need.
const FLANGER_MAX_MS: f32 = 20.0;
/// Feedback clamp (same as Delay) to prevent infinite gain.
const MAX_FEEDBACK: f32 = 0.95;

/// Stereo flanger — modulated short delay with feedback.
///
/// Construct via [`SignalExt::flanger`](crate::SignalExt::flanger).
pub struct Flanger<A: Signal> {
    source: A,
    buffer: Box<[f32]>,
    write_idx: usize,
    rate_hz: f32,
    depth_ms: f32,
    base_ms: f32,
    feedback: f32,
    mix: f32,
    lfo_phase: f32,
    last_wet_l: f32,
    last_wet_r: f32,
}

impl<A: Signal> Flanger<A> {
    pub(crate) fn new(source: A, rate_hz: f32, depth_ms: f32) -> Self {
        let max_samples = ((FLANGER_MAX_MS * 0.001 * FLANGER_MAX_SR).ceil() as usize).max(2);
        Flanger {
            source,
            buffer: vec![0.0; max_samples].into_boxed_slice(),
            write_idx: 0,
            rate_hz: rate_hz.max(0.01),
            depth_ms: depth_ms.clamp(0.1, 10.0),
            base_ms: 2.5,
            feedback: 0.0,
            mix: 0.5,
            lfo_phase: 0.0,
            last_wet_l: 0.0,
            last_wet_r: 0.0,
        }
    }

    /// Override the base (minimum) delay in ms. Typical flangers use
    /// 1–5 ms. Default `2.5`.
    pub fn base_delay(mut self, ms: f32) -> Self {
        self.base_ms = ms.clamp(0.1, 15.0);
        self
    }

    /// Self-feedback, clamped to `[0.0, 0.95]`. Higher = more swirl.
    /// Default `0.0`.
    pub fn feedback(mut self, amount: f32) -> Self {
        self.feedback = amount.clamp(0.0, MAX_FEEDBACK);
        self
    }

    /// Wet/dry mix. `0.0` = dry only, `1.0` = wet only. Default `0.5`.
    pub fn mix(mut self, wet: f32) -> Self {
        self.mix = wet.clamp(0.0, 1.0);
        self
    }

    fn read_interpolated(&self, delay_samples: f32) -> f32 {
        let len = self.buffer.len();
        let d = delay_samples.clamp(0.0, (len - 1) as f32);
        let read_pos = self.write_idx as f32 - d;
        let read_pos = if read_pos < 0.0 {
            read_pos + len as f32
        } else {
            read_pos
        };
        let idx0 = (read_pos.floor() as usize) % len;
        let idx1 = (idx0 + 1) % len;
        let frac = read_pos - read_pos.floor();
        self.buffer[idx0] * (1.0 - frac) + self.buffer[idx1] * frac
    }

    fn tick(&mut self, ctx: &AudioContext) -> (f32, f32, f32) {
        let lfo_l = (self.lfo_phase * std::f32::consts::TAU).sin();
        let lfo_r = ((self.lfo_phase + 0.5) * std::f32::consts::TAU).sin();
        self.lfo_phase += self.rate_hz / ctx.sample_rate;
        self.lfo_phase -= self.lfo_phase.floor();

        let base_samples = self.base_ms * 0.001 * ctx.sample_rate;
        let depth_samples = self.depth_ms * 0.001 * ctx.sample_rate;
        // Unipolar LFO modulation — never goes below base delay (would
        // cause negative delay). `lfo * 0.5 + 0.5` gives [0, 1].
        let d_l = base_samples + (lfo_l * 0.5 + 0.5) * depth_samples;
        let d_r = base_samples + (lfo_r * 0.5 + 0.5) * depth_samples;

        let input = self.source.next(ctx);
        let wet_l = self.read_interpolated(d_l);
        let wet_r = self.read_interpolated(d_r);

        // Write input + feedback from averaged previous wet output.
        let fb_signal = (self.last_wet_l + self.last_wet_r) * 0.5 * self.feedback;
        self.buffer[self.write_idx] = input + fb_signal;
        self.write_idx = (self.write_idx + 1) % self.buffer.len();

        self.last_wet_l = wet_l;
        self.last_wet_r = wet_r;

        (input, wet_l, wet_r)
    }
}

impl<A: Signal> Signal for Flanger<A> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let (dry, wet_l, wet_r) = self.tick(ctx);
        let wet = (wet_l + wet_r) * 0.5;
        dry * (1.0 - self.mix) + wet * self.mix
    }

    fn next_stereo(&mut self, ctx: &AudioContext) -> (f32, f32) {
        let (dry, wet_l, wet_r) = self.tick(ctx);
        let dry_m = 1.0 - self.mix;
        (
            dry * dry_m + wet_l * self.mix,
            dry * dry_m + wet_r * self.mix,
        )
    }
}
