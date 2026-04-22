//! Tape emulation: wow + flutter + tape EQ + asymmetric saturation.
//!
//! A one-shot "run it through tape" wrapper that combines three effects:
//!
//! 1. **Pitch modulation** via a short modulated delay line. Wow is a
//!    slow sine (~0.5 Hz), flutter is filtered noise (~6 Hz). Together
//!    they give the characteristic "played back on tape" wobble.
//! 2. **Tape EQ** — one-pole HP at 30 Hz removes DC/rumble, one-pole LP
//!    at 12 kHz simulates tape head loss.
//! 3. **Asymmetric soft-clip** (`tanh(drive·(x + bias)) − tanh(drive·bias)`)
//!    for even-harmonic tape colour.
//!
//! A single [`age`](Tape::age) knob sweeps from pristine (0.0) to
//! battered (1.0), scaling wow depth, flutter depth, and drive together.
//! Individual controls — [`wow`](Tape::wow), [`flutter`](Tape::flutter),
//! [`drive`](Tape::drive) — let you override any component.
//!
//! ```ignore
//! use nyx_core::{osc, SignalExt, TapeExt};
//!
//! let cassette = osc::saw_bl(220.0).amp(0.5).tape().age(0.6);
//! let pristine = osc::sine(440.0).tape().age(0.1);
//! let destroyed = osc::square_bl(110.0).amp(0.4).tape().age(1.0);
//! ```

use crate::signal::{AudioContext, Signal};

/// Pre-allocated delay buffer length. 50 ms at 96 kHz (worst-case SR).
/// At 44.1 kHz this gives ~108 ms of headroom — plenty for the 10 ms
/// base delay plus ±couple-ms wow modulation.
const TAPE_BUFFER_LEN: usize = 4800;

/// Base delay in seconds. The read head sits this far behind the write
/// head so the wow/flutter modulation can swing both ways without
/// underflowing.
const TAPE_BASE_DELAY_SECS: f32 = 0.010;

// Tape EQ corners.
const TAPE_PRE_HP_HZ: f32 = 30.0;
const TAPE_POST_LP_HZ: f32 = 12_000.0;
const TAPE_BIAS: f32 = 0.1;

// Default wow/flutter depths expressed as fraction of sample rate
// (so the pitch modulation is SR-independent).
const DEFAULT_WOW_RATE_HZ: f32 = 0.5;
const DEFAULT_FLUTTER_RATE_HZ: f32 = 6.0;
const DEFAULT_WOW_DEPTH_FRAC: f32 = 0.0008; // ~35 samples at 44.1 kHz
const DEFAULT_FLUTTER_DEPTH_FRAC: f32 = 0.0003; // ~13 samples at 44.1 kHz
const DEFAULT_AGE: f32 = 0.5;

/// Tape emulation wrapper. See the [module docs][crate::tape] for the
/// signal chain and defaults.
pub struct Tape<A: Signal> {
    source: A,

    // Circular delay buffer for pitch modulation.
    buffer: Box<[f32]>,
    write_idx: usize,

    // Wow LFO (sine).
    wow_phase: f32,
    wow_rate_hz: f32,
    wow_depth_frac: f32,

    // Flutter (filtered noise).
    flutter_noise_state: u32,
    flutter_lp_state: f32,
    flutter_rate_hz: f32,
    flutter_depth_frac: f32,

    // Tape EQ state.
    pre_lp_state: f32,
    post_lp_state: f32,

    // Cached sample-rate-dependent coefficients.
    pre_alpha: f32,
    post_alpha: f32,
    flutter_alpha: f32,
    base_delay_samples: f32,
    sr: f32,
    initialised: bool,

    drive: f32,
}

impl<A: Signal> Tape<A> {
    /// Master "age" knob — scales wow depth, flutter depth, and drive
    /// together. `0.0` = pristine (no modulation, no drive), `1.0` =
    /// battered (full modulation, heavy drive). Clamped to `[0, 1]`.
    ///
    /// Overrides any previously-set wow/flutter/drive values.
    pub fn age(mut self, amount: f32) -> Self {
        let a = amount.clamp(0.0, 1.0);
        self.wow_depth_frac = DEFAULT_WOW_DEPTH_FRAC * a;
        self.flutter_depth_frac = DEFAULT_FLUTTER_DEPTH_FRAC * a;
        // Drive goes from 1.0 (no colour) to 3.0 (heavy colour) linearly.
        self.drive = 1.0 + 2.0 * a;
        self
    }

    /// Set the wow LFO rate (Hz) and depth. Depth is a fraction of the
    /// sample rate: 0.0008 ≈ the default, perceptually subtle.
    pub fn wow(mut self, rate_hz: f32, depth: f32) -> Self {
        self.wow_rate_hz = rate_hz.max(0.0);
        self.wow_depth_frac = depth.max(0.0);
        self
    }

    /// Set the flutter rate (Hz) and depth (fraction of sample rate).
    pub fn flutter(mut self, rate_hz: f32, depth: f32) -> Self {
        self.flutter_rate_hz = rate_hz.max(0.0);
        self.flutter_depth_frac = depth.max(0.0);
        self
    }

    /// Set the tape saturation drive. `1.0` ≈ transparent, `3.0` =
    /// heavy tape colour.
    pub fn drive(mut self, amount: f32) -> Self {
        self.drive = amount.max(0.0);
        self
    }

    #[inline]
    fn next_noise(&mut self) -> f32 {
        // xorshift32.
        let mut x = self.flutter_noise_state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.flutter_noise_state = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

impl<A: Signal> Signal for Tape<A> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        if !self.initialised || self.sr != ctx.sample_rate {
            self.sr = ctx.sample_rate;
            self.pre_alpha = one_pole_alpha(TAPE_PRE_HP_HZ, ctx.sample_rate);
            self.post_alpha = one_pole_alpha(TAPE_POST_LP_HZ, ctx.sample_rate);
            self.flutter_alpha = one_pole_alpha(self.flutter_rate_hz.max(0.01), ctx.sample_rate);
            self.base_delay_samples = TAPE_BASE_DELAY_SECS * ctx.sample_rate;
            self.initialised = true;
        }

        let input = self.source.next(ctx);

        // 1. Write input to the circular buffer.
        self.buffer[self.write_idx] = input;
        self.write_idx = (self.write_idx + 1) % self.buffer.len();

        // 2. Compute wow + flutter modulation in samples.
        let wow_mod =
            (self.wow_phase * std::f32::consts::TAU).sin() * self.wow_depth_frac * ctx.sample_rate;
        self.wow_phase += self.wow_rate_hz / ctx.sample_rate;
        self.wow_phase -= self.wow_phase.floor();

        let noise = self.next_noise();
        self.flutter_lp_state += self.flutter_alpha * (noise - self.flutter_lp_state);
        let flutter_mod = self.flutter_lp_state * self.flutter_depth_frac * ctx.sample_rate;

        let delay_samples = self.base_delay_samples + wow_mod + flutter_mod;

        // 3. Read delayed sample with linear interpolation.
        let len = self.buffer.len() as f32;
        // Position relative to write head (always negative in this form).
        let mut read_pos = self.write_idx as f32 - delay_samples;
        while read_pos < 0.0 {
            read_pos += len;
        }
        while read_pos >= len {
            read_pos -= len;
        }
        let i0 = read_pos.floor() as usize;
        let i1 = (i0 + 1) % self.buffer.len();
        let frac = read_pos - read_pos.floor();
        let delayed = self.buffer[i0] * (1.0 - frac) + self.buffer[i1] * frac;

        // 4. Tape EQ + asymmetric saturation.
        // HP (subtract slow LP) to kill rumble.
        self.pre_lp_state += self.pre_alpha * (delayed - self.pre_lp_state);
        let hp = delayed - self.pre_lp_state;

        // Asymmetric tanh soft-clip with bias for even harmonics.
        let drive = self.drive;
        let biased = drive * (hp + TAPE_BIAS);
        let saturated = biased.tanh() - (drive * TAPE_BIAS).tanh();

        // Post-LP to simulate tape HF head loss.
        self.post_lp_state += self.post_alpha * (saturated - self.post_lp_state);
        self.post_lp_state
    }
}

#[inline]
fn one_pole_alpha(cutoff: f32, sr: f32) -> f32 {
    1.0 - (-std::f32::consts::TAU * cutoff / sr).exp()
}

/// Adds `.tape()` to every [`Signal`].
pub trait TapeExt: Signal + Sized {
    /// Wrap the signal in a tape emulator with mid-age defaults
    /// (age=0.5). Chain `.age(...)`, `.drive(...)`, `.wow(...)` or
    /// `.flutter(...)` afterwards to customise.
    fn tape(self) -> Tape<Self> {
        let defaults_age = DEFAULT_AGE;
        Tape {
            source: self,
            buffer: vec![0.0_f32; TAPE_BUFFER_LEN].into_boxed_slice(),
            write_idx: 0,
            wow_phase: 0.0,
            wow_rate_hz: DEFAULT_WOW_RATE_HZ,
            wow_depth_frac: DEFAULT_WOW_DEPTH_FRAC * defaults_age,
            flutter_noise_state: 0x517C_B0FE,
            flutter_lp_state: 0.0,
            flutter_rate_hz: DEFAULT_FLUTTER_RATE_HZ,
            flutter_depth_frac: DEFAULT_FLUTTER_DEPTH_FRAC * defaults_age,
            pre_lp_state: 0.0,
            post_lp_state: 0.0,
            pre_alpha: 0.0,
            post_alpha: 0.0,
            flutter_alpha: 0.0,
            base_delay_samples: 0.0,
            sr: 0.0,
            initialised: false,
            drive: 1.0 + 2.0 * defaults_age,
        }
    }
}

impl<T: Signal + Sized> TapeExt for T {}
