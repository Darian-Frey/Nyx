//! Karplus-Strong plucked string synthesis.
//!
//! The canonical DSP teaching example — a noise burst circulating
//! through a delay line with a gentle one-pole lowpass in the feedback
//! path. The delay length sets the pitch (`sample_rate / freq`), and
//! the lowpass progressively loses high frequencies each pass, giving
//! the characteristic decay envelope of a plucked string.
//!
//! # Example
//!
//! ```ignore
//! use nyx_prelude::*;
//!
//! // 440 Hz plucked string, long sustain (decay close to 1 = long).
//! play(pluck(440.0, 0.99)).unwrap();
//! ```
//!
//! # Parameters
//!
//! - `freq` — pitch in Hz. Must be positive; minimum internally clamped
//!   to 20 Hz to keep the delay buffer a reasonable size.
//! - `decay` — feedback coefficient in `[0.0, 1.0]`. `0.99` gives a
//!   long sustain; `0.9` gives a short pluck. `1.0` would never decay;
//!   values close to it may sustain for many seconds.
//!
//! # Single-shot behaviour
//!
//! `Pluck` is self-triggering: the noise burst is loaded when the
//! signal is constructed, and the string rings from there. Dropping a
//! `Pluck` instance ends the note. For repeated strikes, either build
//! a fresh `Pluck` per note-on event or (in future versions) wrap one
//! in a `VoicePool` to recycle instances.
//!
//! # Why this earns its own function
//!
//! A user could wire this up from `osc::noise::white` + `.delay()` +
//! `.lowpass()`. `pluck()` exists because:
//!
//! - The `sample_rate / freq` → delay-length conversion is a trap for
//!   newcomers.
//! - The algorithm is the canonical first example in every DSP textbook;
//!   its absence from the palette is surprising.
//! - A one-line demo (`play(pluck(440.0, 0.99))`) sells the library.

use crate::delay::DELAY_MAX_SR;
use crate::signal::{AudioContext, Signal};

/// Classic Karplus-Strong plucked-string voice.
///
/// Construct via [`pluck`].
pub struct Pluck {
    buffer: Box<[f32]>,
    active_len: usize,
    read_idx: usize,
    freq: f32,
    decay: f32,
    seed: u32,
    initialised: bool,
}

/// Create a new plucked-string voice at the given frequency and decay.
///
/// `freq` is in Hz; `decay` is a feedback coefficient in `[0, 1]`.
pub fn pluck(freq: f32, decay: f32) -> Pluck {
    // Allocate for the worst case: the delay length needed at the
    // highest sample rate we support. Actual length at runtime will
    // usually be smaller.
    let freq = freq.max(20.0);
    let max_samples = (DELAY_MAX_SR / freq).ceil() as usize + 1;

    // Derive a reproducible noise seed from the arguments so that two
    // `pluck(440.0, 0.99)` calls produce the same initial burst.
    let seed = freq.to_bits() ^ decay.to_bits().rotate_left(13);

    Pluck {
        buffer: vec![0.0; max_samples].into_boxed_slice(),
        active_len: 0,
        read_idx: 0,
        freq,
        decay: decay.clamp(0.0, 1.0),
        seed: if seed == 0 { 1 } else { seed },
        initialised: false,
    }
}

impl Pluck {
    /// Fill the active region of the buffer with white noise.
    fn strike(&mut self, sample_rate: f32) {
        let n = ((sample_rate / self.freq).round() as usize)
            .clamp(2, self.buffer.len());
        self.active_len = n;
        self.read_idx = 0;
        let mut state = self.seed;
        for i in 0..n {
            // xorshift32 for portable deterministic noise
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            self.buffer[i] = (state as f32 / u32::MAX as f32) * 2.0 - 1.0;
        }
    }
}

impl Signal for Pluck {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        if !self.initialised {
            self.strike(ctx.sample_rate);
            self.initialised = true;
        }

        let out = self.buffer[self.read_idx];
        let prev_idx =
            (self.read_idx + self.active_len - 1) % self.active_len;
        let prev = self.buffer[prev_idx];

        // One-pole lowpass (simple average) × decay in the feedback path.
        // High frequencies lose energy faster than low — the natural
        // string decay shape falls out of this.
        let filtered = 0.5 * (out + prev) * self.decay;
        self.buffer[self.read_idx] = filtered;

        self.read_idx = (self.read_idx + 1) % self.active_len;
        out
    }
}
