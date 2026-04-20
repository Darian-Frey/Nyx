//! Bitcrusher and sample-rate reducer — the two halves of classic
//! lo-fi/glitch distortion.
//!
//! Both effects are stateless in the "no DSP memory" sense: [`BitCrush`]
//! holds nothing between samples, and [`Downsample`] holds exactly one
//! latched sample plus a phase counter. Trivially real-time safe.
//!
//! The methods [`SignalExt::bitcrush`](crate::SignalExt::bitcrush),
//! [`SignalExt::downsample`](crate::SignalExt::downsample), and
//! [`SignalExt::crush`](crate::SignalExt::crush) are the usual entry
//! points.

use crate::signal::{AudioContext, Signal};

/// A signal wrapper that quantises its input to a reduced bit depth.
///
/// Produced by [`SignalExt::bitcrush`](crate::SignalExt::bitcrush).
pub struct BitCrush<S: Signal> {
    source: S,
    levels: f32,
}

impl<S: Signal> BitCrush<S> {
    pub(crate) fn new(source: S, bits: u32) -> Self {
        // 1 bit → 1 level, 2 bits → 3 levels, 8 bits → 255, 16 → 65535.
        // Clamp to at least 1 bit to avoid division by zero.
        let bits = bits.clamp(1, 24);
        let levels = (1u32 << bits) as f32 - 1.0;
        BitCrush { source, levels }
    }
}

impl<S: Signal> Signal for BitCrush<S> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let input = self.source.next(ctx);
        // Map [-1, 1] → [0, 1], quantise, map back.
        let unipolar = input.clamp(-1.0, 1.0) * 0.5 + 0.5;
        let quantised = (unipolar * self.levels).round() / self.levels;
        quantised * 2.0 - 1.0
    }
}

/// A signal wrapper that holds each input sample for multiple output
/// samples, reducing the effective sample rate.
///
/// `ratio` ∈ `(0, 1]`:
/// - `1.0` — identity (no reduction)
/// - `0.5` — each input sample is held for 2 output samples
/// - `0.25` — each input sample is held for 4 output samples
///
/// The upstream source is consumed at the full rate (so oscillator
/// phases etc. keep advancing correctly); only the *emitted* value is
/// held. This sample-and-hold behaviour produces the aliasing artefacts
/// that give the effect its character.
///
/// Produced by [`SignalExt::downsample`](crate::SignalExt::downsample).
pub struct Downsample<S: Signal> {
    source: S,
    ratio: f32,
    counter: f32,
    held: f32,
}

impl<S: Signal> Downsample<S> {
    pub(crate) fn new(source: S, ratio: f32) -> Self {
        let ratio = ratio.clamp(1e-6, 1.0);
        Downsample {
            source,
            ratio,
            counter: 0.0,
            held: 0.0,
        }
    }
}

impl<S: Signal> Signal for Downsample<S> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let input = self.source.next(ctx);
        // Latch whenever the counter runs out; then refill it by 1.0 and
        // subtract `ratio` for each output sample. At ratio=0.5 the
        // counter refills to 1.0 then drops to 0.5 (no re-latch) then
        // 0.0 (re-latch) — producing the pattern: latch, hold, latch,
        // hold, ...
        if self.counter <= 0.0 {
            self.held = input;
            self.counter += 1.0;
        }
        self.counter -= self.ratio;
        self.held
    }
}
