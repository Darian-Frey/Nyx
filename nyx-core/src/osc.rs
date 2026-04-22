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

impl<S: Signal> Sine<S> {
    /// Convert this sine into an FM (phase modulation) operator.
    ///
    /// The carrier frequency is preserved from this sine; `modulator`
    /// is an arbitrary signal whose output is added to the carrier's
    /// phase, scaled by `index`.
    ///
    /// ```ignore
    /// // DX7-style bell: 1:2 modulator ratio, index 3
    /// osc::sine(440.0).fm(osc::sine(880.0), 3.0)
    /// ```
    pub fn fm<M: Signal, I: IntoParam>(
        self,
        modulator: M,
        index: I,
    ) -> crate::fm::FmOp<S, M, I::Signal> {
        crate::fm::FmOp::from_sine_parts(self.freq, modulator, index.into_param(), self.phase)
    }
}

/// Sawtooth oscillator (naive, non-band-limited).
///
/// Produces ideal harmonics that fold back below Nyquist as aliasing.
/// This is the "raw" / chiptune / 8-bit character — for a clean
/// subtractive-synth sound, use [`saw_bl`] instead.
pub struct Saw<S: Signal> {
    phase: f32,
    freq: Param<S>,
}

/// Create a sawtooth oscillator at the given frequency (naive).
///
/// Use [`saw_bl`] for a band-limited version that removes aliasing.
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
///
/// Produces ideal harmonics that fold back as aliasing. For a clean
/// subtractive-synth sound, use [`square_bl`] instead.
pub struct Square<S: Signal> {
    phase: f32,
    freq: Param<S>,
}

/// Create a square wave oscillator at the given frequency (naive).
///
/// Use [`square_bl`] for a band-limited version that removes aliasing.
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

// ─── Band-limited (PolyBLEP) oscillators ────────────────────────────

/// Polynomial Band-Limited stEP correction.
///
/// Returns a value to subtract from (saw) or add/subtract from (square)
/// the naive waveform within ±`dt` samples of each discontinuity, where
/// `t` is the current phase in `[0, 1)` and `dt = freq / sample_rate`.
/// See Välimäki & Huovilainen, IEEE SPM 2007.
#[inline]
fn poly_blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let x = t / dt;
        2.0 * x - x * x - 1.0
    } else if t > 1.0 - dt {
        let x = (t - 1.0) / dt;
        x * x + 2.0 * x + 1.0
    } else {
        0.0
    }
}

/// Band-limited sawtooth via PolyBLEP.
///
/// Same timbre as [`Saw`] below ~1 kHz; above that, audibly free of the
/// inharmonic fold-back that a naive saw produces. Residual aliasing is
/// ~70 dB down — not transparent, but perceptually clean across the
/// full musical range.
pub struct SawBl<S: Signal> {
    phase: f32,
    freq: Param<S>,
}

/// Create a band-limited sawtooth oscillator at the given frequency.
///
/// ```ignore
/// osc::saw_bl(440.0)         // clean mid-register saw
/// osc::saw_bl(lfo_freq)      // frequency modulated, still clean
/// ```
pub fn saw_bl<P: IntoParam>(freq: P) -> SawBl<P::Signal> {
    SawBl {
        phase: 0.0,
        freq: freq.into_param(),
    }
}

impl<S: Signal> Signal for SawBl<S> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let freq = self.freq.next(ctx);
        // Clamp dt below Nyquist so the BLEP windows never overlap.
        let dt = (freq / ctx.sample_rate).abs().min(0.5);

        let naive = 2.0 * self.phase - 1.0;
        let out = naive - poly_blep(self.phase, dt);

        self.phase += freq / ctx.sample_rate;
        self.phase -= self.phase.floor();
        out
    }
}

/// Band-limited square wave via PolyBLEP.
///
/// Corrects both discontinuities per cycle (up-step at phase 0, down-
/// step at phase 0.5). Same caveats as [`SawBl`].
pub struct SquareBl<S: Signal> {
    phase: f32,
    freq: Param<S>,
}

/// Create a band-limited square wave oscillator at the given frequency.
pub fn square_bl<P: IntoParam>(freq: P) -> SquareBl<P::Signal> {
    SquareBl {
        phase: 0.0,
        freq: freq.into_param(),
    }
}

impl<S: Signal> Signal for SquareBl<S> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let freq = self.freq.next(ctx);
        let dt = (freq / ctx.sample_rate).abs().min(0.5);

        let naive = if self.phase < 0.5 { 1.0 } else { -1.0 };
        let mut out = naive;
        // Up-step at phase 0.
        out += poly_blep(self.phase, dt);
        // Down-step at phase 0.5 — shift the phase by 0.5 so the kernel
        // sees the discontinuity at its origin.
        let shifted = (self.phase + 0.5).fract();
        out -= poly_blep(shifted, dt);

        self.phase += freq / ctx.sample_rate;
        self.phase -= self.phase.floor();
        out
    }
}

/// Band-limited pulse wave with modulatable duty cycle (PWM).
///
/// At `width = 0.5` this is identical to [`SquareBl`]. As `width`
/// moves away from `0.5` the on/off ratio changes, producing the
/// "thick" varying timbre of PWM synths (the Juno-60 voice being
/// the canonical example). Both discontinuities — the up-step at
/// `phase = 0` and the down-step at `phase = width` — get PolyBLEP
/// corrections.
///
/// `width` is clamped to `[0.05, 0.95]` so the up and down edges
/// never collide and each PolyBLEP window has room to spread.
pub struct PwmBl<S: Signal, W: Signal> {
    phase: f32,
    freq: Param<S>,
    width: Param<W>,
}

/// Create a band-limited PWM oscillator.
///
/// ```ignore
/// // Juno-style pad voice with slow PWM sweep.
/// let lfo = osc::sine(0.3).amp(0.15).offset(0.5);
/// let v = osc::pwm_bl(220.0, lfo);
/// ```
pub fn pwm_bl<FP: IntoParam, WP: IntoParam>(freq: FP, width: WP) -> PwmBl<FP::Signal, WP::Signal> {
    PwmBl {
        phase: 0.0,
        freq: freq.into_param(),
        width: width.into_param(),
    }
}

impl<S: Signal, W: Signal> Signal for PwmBl<S, W> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let freq = self.freq.next(ctx);
        let width = self.width.next(ctx).clamp(0.05, 0.95);
        let dt = (freq / ctx.sample_rate).abs().min(0.5);

        let naive = if self.phase < width { 1.0 } else { -1.0 };
        let mut out = naive;
        // Up-step at phase 0.
        out += poly_blep(self.phase, dt);
        // Down-step at phase = width — shift so the kernel sees it at 0.
        let shifted = (self.phase - width + 1.0).fract();
        out -= poly_blep(shifted, dt);

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

    /// Pink noise generator using the Paul Kellett filter (musicdsp.org).
    ///
    /// Drives a bank of five parallel one-pole filters plus two
    /// direct-path terms from a shared white source. Produces a
    /// smoother approximation of the −3 dB/octave slope than the
    /// previous Voss-McCartney implementation, with constant cost per
    /// sample (no branching on octave-counter bits) and fewer state
    /// variables.
    ///
    /// # Reference
    /// Paul Kellett filter — <https://www.musicdsp.org/en/latest/Filters/76-pink-noise-filter.html>
    pub struct Pink {
        state: u32,
        b0: f32,
        b1: f32,
        b2: f32,
        b3: f32,
        b4: f32,
        last_white: f32,
    }

    /// Create a pink noise generator with a given seed.
    pub fn pink(seed: u32) -> Pink {
        Pink {
            state: if seed == 0 { 1 } else { seed },
            b0: 0.0,
            b1: 0.0,
            b2: 0.0,
            b3: 0.0,
            b4: 0.0,
            last_white: 0.0,
        }
    }

    /// Empirical scaling to bring RMS to ~0.7, matching `White`.
    const PINK_KELLETT_SCALE: f32 = 0.11;

    impl Pink {
        #[inline]
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
            let w = self.next_white();
            self.b0 = 0.99886 * self.b0 + w * 0.055_517_9;
            self.b1 = 0.99332 * self.b1 + w * 0.075_075_9;
            self.b2 = 0.96900 * self.b2 + w * 0.153_852;
            self.b3 = 0.86650 * self.b3 + w * 0.310_485_6;
            self.b4 = 0.55000 * self.b4 + w * 0.532_952_2;
            let b6_now = w * 0.115926;
            let pink = self.b0 + self.b1 + self.b2 + self.b3 + self.b4 + b6_now + self.last_white;
            self.last_white = b6_now;
            pink * PINK_KELLETT_SCALE
        }
    }
}
