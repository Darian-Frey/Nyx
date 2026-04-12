//! Oscillator primitives.
//!
//! All oscillators track phase as a normalised `f32` in [0, 1),
//! incremented by `freq / sample_rate` each sample. Frequency
//! accepts `Param<S>` so it can be modulated by another signal.

use crate::param::{IntoParam, Param};
use crate::signal::{AudioContext, Signal};

/// Sine oscillator.
pub struct Sine<S: Signal> {
    phase: f32,
    freq: Param<S>,
}

/// Create a sine oscillator at the given frequency.
///
/// ```ignore
/// osc::sine(440.0)           // fixed pitch
/// osc::sine(lfo)             // frequency modulated
/// ```
pub fn sine<P: IntoParam>(freq: P) -> Sine<P::Signal> {
    Sine {
        phase: 0.0,
        freq: freq.into_param(),
    }
}

impl<S: Signal> Signal for Sine<S> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let out = (self.phase * std::f32::consts::TAU).sin();
        let freq = self.freq.next(ctx);
        self.phase += freq / ctx.sample_rate;
        self.phase -= self.phase.floor();
        out
    }
}

/// Sawtooth oscillator (naive, non-band-limited).
pub struct Saw<S: Signal> {
    phase: f32,
    freq: Param<S>,
}

/// Create a sawtooth oscillator at the given frequency.
pub fn saw<P: IntoParam>(freq: P) -> Saw<P::Signal> {
    Saw {
        phase: 0.0,
        freq: freq.into_param(),
    }
}

impl<S: Signal> Signal for Saw<S> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        // Output in [-1, 1]: ramp from -1 to +1 over one period.
        let out = 2.0 * self.phase - 1.0;
        let freq = self.freq.next(ctx);
        self.phase += freq / ctx.sample_rate;
        self.phase -= self.phase.floor();
        out
    }
}

/// Square wave oscillator (naive, non-band-limited).
pub struct Square<S: Signal> {
    phase: f32,
    freq: Param<S>,
}

/// Create a square wave oscillator at the given frequency.
pub fn square<P: IntoParam>(freq: P) -> Square<P::Signal> {
    Square {
        phase: 0.0,
        freq: freq.into_param(),
    }
}

impl<S: Signal> Signal for Square<S> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let out = if self.phase < 0.5 { 1.0 } else { -1.0 };
        let freq = self.freq.next(ctx);
        self.phase += freq / ctx.sample_rate;
        self.phase -= self.phase.floor();
        out
    }
}

/// Triangle wave oscillator.
pub struct Triangle<S: Signal> {
    phase: f32,
    freq: Param<S>,
}

/// Create a triangle wave oscillator at the given frequency.
pub fn triangle<P: IntoParam>(freq: P) -> Triangle<P::Signal> {
    Triangle {
        phase: 0.0,
        freq: freq.into_param(),
    }
}

impl<S: Signal> Signal for Triangle<S> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        // Triangle: rises from -1 to +1 in first half, falls from +1 to -1 in second.
        let out = if self.phase < 0.5 {
            4.0 * self.phase - 1.0
        } else {
            3.0 - 4.0 * self.phase
        };
        let freq = self.freq.next(ctx);
        self.phase += freq / ctx.sample_rate;
        self.phase -= self.phase.floor();
        out
    }
}

/// Noise generators.
pub mod noise {
    use crate::signal::{AudioContext, Signal};

    /// White noise generator using a portable xorshift32 PRNG.
    ///
    /// Output is uniformly distributed in [-1, 1].
    pub struct White {
        state: u32,
    }

    /// Create a white noise generator with a given seed.
    pub fn white(seed: u32) -> White {
        // Avoid zero seed — xorshift degenerates.
        White {
            state: if seed == 0 { 1 } else { seed },
        }
    }

    impl Signal for White {
        fn next(&mut self, _ctx: &AudioContext) -> f32 {
            // xorshift32
            let mut x = self.state;
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            self.state = x;
            // Map u32 to [-1, 1]
            (x as f32 / u32::MAX as f32) * 2.0 - 1.0
        }
    }

    /// Pink noise generator using the Voss-McCartney algorithm (3-octave).
    ///
    /// Sums octave-band white noise sources for an approximate -3 dB/octave
    /// spectral slope.
    pub struct Pink {
        white_values: [f32; PINK_OCTAVES],
        counter: u32,
        state: u32,
    }

    const PINK_OCTAVES: usize = 12;

    /// Create a pink noise generator with a given seed.
    pub fn pink(seed: u32) -> Pink {
        Pink {
            white_values: [0.0; PINK_OCTAVES],
            counter: 0,
            state: if seed == 0 { 1 } else { seed },
        }
    }

    impl Pink {
        fn next_white(&mut self) -> f32 {
            let mut x = self.state;
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            self.state = x;
            (x as f32 / u32::MAX as f32) * 2.0 - 1.0
        }
    }

    impl Signal for Pink {
        fn next(&mut self, _ctx: &AudioContext) -> f32 {
            let changed = self.counter ^ self.counter.wrapping_add(1);
            self.counter = self.counter.wrapping_add(1);

            // Update octave bands where the corresponding bit flipped.
            for i in 0..PINK_OCTAVES {
                if changed & (1 << i) != 0 {
                    self.white_values[i] = self.next_white();
                }
            }

            let sum: f32 = self.white_values.iter().sum();
            // Normalise to roughly [-1, 1].
            sum / PINK_OCTAVES as f32
        }
    }
}
