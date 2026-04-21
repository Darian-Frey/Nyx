//! Chorus — modulated short delay with stereo spread.
//!
//! A chorus effect runs the input through a short delay (typically
//! 15–30 ms) whose length is modulated by a slow LFO (0.1–3 Hz). The
//! result mimics multiple voices of the same instrument playing
//! slightly out of sync — the classic "thicker than one voice" sound.
//!
//! Nyx's chorus produces real stereo via two LFOs offset by 180° —
//! when the left-channel delay is long, the right-channel delay is
//! short. Summing to mono is safe (mild comb filtering).
//!
//! # Example
//!
//! ```ignore
//! use nyx_prelude::*;
//!
//! // Classic 0.5 Hz / 3 ms depth chorus on a pad
//! let pad = osc::saw(220.0).amp(0.3).chorus(0.5, 3.0);
//! play(pad).unwrap();
//! ```

use crate::signal::{AudioContext, Signal};

/// Max sample rate supported for buffer sizing.
const CHORUS_MAX_SR: f32 = 96_000.0;
/// Maximum delay time we ever need (base + depth headroom).
const CHORUS_MAX_MS: f32 = 80.0;

/// Stereo chorus — modulated delay with 180°-offset LFOs on L and R.
///
/// Construct via [`SignalExt::chorus`](crate::SignalExt::chorus).
pub struct Chorus<A: Signal> {
    source: A,
    buffer: Box<[f32]>,
    write_idx: usize,
    rate_hz: f32,
    depth_ms: f32,
    base_ms: f32,
    mix: f32,
    lfo_phase: f32,
}

impl<A: Signal> Chorus<A> {
    pub(crate) fn new(source: A, rate_hz: f32, depth_ms: f32) -> Self {
        let max_samples = ((CHORUS_MAX_MS * 0.001 * CHORUS_MAX_SR).ceil() as usize).max(2);
        Chorus {
            source,
            buffer: vec![0.0; max_samples].into_boxed_slice(),
            write_idx: 0,
            rate_hz: rate_hz.max(0.01),
            depth_ms: depth_ms.clamp(0.1, 30.0),
            base_ms: 20.0,
            mix: 0.5,
            lfo_phase: 0.0,
        }
    }

    /// Override the base delay (default 20 ms, typical 15–30 ms).
    pub fn base_delay(mut self, ms: f32) -> Self {
        self.base_ms = ms.clamp(1.0, 50.0);
        self
    }

    /// Wet/dry mix. `0.0` = dry only, `1.0` = wet only. Default `0.5`.
    pub fn mix(mut self, wet: f32) -> Self {
        self.mix = wet.clamp(0.0, 1.0);
        self
    }

    /// Sample the delay line at a fractional position (linear interp).
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

    /// Compute both channels' delayed samples and write the new input
    /// into the ring buffer. Shared work between `next` and `next_stereo`.
    fn tick(&mut self, ctx: &AudioContext) -> (f32, f32, f32) {
        // LFO advance.
        let lfo_l = (self.lfo_phase * std::f32::consts::TAU).sin();
        // Right channel 180° offset — when L is peak, R is trough.
        let lfo_r = ((self.lfo_phase + 0.5) * std::f32::consts::TAU).sin();
        self.lfo_phase += self.rate_hz / ctx.sample_rate;
        self.lfo_phase -= self.lfo_phase.floor();

        // Modulated delay times in samples.
        let base_samples = self.base_ms * 0.001 * ctx.sample_rate;
        let depth_samples = self.depth_ms * 0.001 * ctx.sample_rate;
        let d_l = base_samples + lfo_l * depth_samples;
        let d_r = base_samples + lfo_r * depth_samples;

        let input = self.source.next(ctx);
        let wet_l = self.read_interpolated(d_l);
        let wet_r = self.read_interpolated(d_r);

        // Write the new input to the buffer. No feedback for chorus.
        self.buffer[self.write_idx] = input;
        self.write_idx = (self.write_idx + 1) % self.buffer.len();

        (input, wet_l, wet_r)
    }
}

impl<A: Signal> Signal for Chorus<A> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let (dry, wet_l, wet_r) = self.tick(ctx);
        // Mono: average the two wet taps.
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
