//! Non-linear waveshapers with distinct tonal characters.
//!
//! Three named saturations, each voiced for a classic analog sound:
//!
//! - [`TapeSat`] — asymmetric `tanh` soft-clip sandwiched between a DC-removing
//!   HP and a 12 kHz LP. The asymmetry (`bias ≈ 0.1`) introduces the
//!   even-harmonic colour that real tape recorders produce, while the HF
//!   rolloff mimics tape head loss. Drive-compensated output.
//! - [`TubeSat`] — asymmetric polynomial waveshaper. `y = x − k·x²` adds
//!   second-harmonic "tube warmth"; a following `y − y³/3` softens peaks.
//!   A 15 kHz LP smooths residual harmonics; a DC-blocking HP keeps the
//!   polynomial's DC component out of the signal path.
//! - [`DiodeClip`] — algebraic soft-clip `y = x / (1 + |x·drive|)`. Sharper
//!   knee than `tanh`, classic diode/pedal character. No filtering.
//!
//! Unlike the generic [`SoftClip`](crate::SoftClip) (which is just
//! `tanh(drive·x)`), these three modules are tuned for specific
//! sounds — reach for them when you want "tape" or "tube" or "fuzz,"
//! not a generic waveshaper.
//!
//! ```ignore
//! use nyx_core::{osc, SaturationExt, SignalExt};
//!
//! let warm = osc::saw_bl(220.0).amp(0.6).tape_sat(2.0);
//! let bright = osc::saw_bl(220.0).amp(0.6).tube_sat(3.0);
//! let fuzz = osc::saw_bl(220.0).amp(0.6).diode_clip(8.0);
//! ```

use crate::param::{IntoParam, Param};
use crate::signal::{AudioContext, Signal};

/// One-pole filter coefficient: `1 - exp(-2π · cutoff / sr)`.
///
/// Used for HP/LP states inside the saturators. At cutoff ≪ sr the
/// approximation is indistinguishable from a biquad; it's chosen here
/// because it is branch-free, one multiply per sample, and stable
/// under arbitrary parameter modulation.
#[inline]
fn one_pole_alpha(cutoff: f32, sr: f32) -> f32 {
    1.0 - (-std::f32::consts::TAU * cutoff / sr).exp()
}

// ─── TapeSat ─────────────────────────────────────────────────────────

/// Asymmetric `tanh` saturation with tape pre/de-emphasis.
///
/// Signal chain: one-pole HP @ 30 Hz → asymmetric soft-clip
/// (`tanh(drive·(x + bias)) − tanh(drive·bias)`) → one-pole LP @ 12 kHz
/// → gain compensation by `1/√drive`.
pub struct TapeSat<A: Signal, D: Signal> {
    source: A,
    drive: Param<D>,
    bias: f32,
    pre_lp_state: f32,
    post_lp_state: f32,
    pre_alpha: f32,
    post_alpha: f32,
    sr: f32,
    initialised: bool,
}

const TAPE_PRE_HP_HZ: f32 = 30.0;
const TAPE_POST_LP_HZ: f32 = 12_000.0;
const TAPE_BIAS: f32 = 0.1;

/// Default drive compensation exponent. Real tape loudness grows sub-linearly
/// with input level; `1/√drive` keeps perceived volume roughly constant
/// as `drive` sweeps from 1 → 10.
#[inline]
fn tape_gain_comp(drive: f32) -> f32 {
    if drive > 0.0 {
        drive.sqrt().recip()
    } else {
        1.0
    }
}

impl<A: Signal, D: Signal> Signal for TapeSat<A, D> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        if !self.initialised || self.sr != ctx.sample_rate {
            self.sr = ctx.sample_rate;
            self.pre_alpha = one_pole_alpha(TAPE_PRE_HP_HZ, ctx.sample_rate);
            self.post_alpha = one_pole_alpha(TAPE_POST_LP_HZ, ctx.sample_rate);
            self.initialised = true;
        }

        let x = self.source.next(ctx);
        let drive = self.drive.next(ctx).max(0.0);

        // One-pole HP: subtract a slowly-tracking LP of the input.
        self.pre_lp_state += self.pre_alpha * (x - self.pre_lp_state);
        let hp = x - self.pre_lp_state;

        // Asymmetric soft-clip. Subtracting `tanh(drive·bias)` removes the
        // DC offset the bias would otherwise introduce.
        let biased = drive * (hp + self.bias);
        let y = biased.tanh() - (drive * self.bias).tanh();

        // One-pole LP to simulate tape HF head-loss.
        self.post_lp_state += self.post_alpha * (y - self.post_lp_state);

        self.post_lp_state * tape_gain_comp(drive)
    }
}

// ─── TubeSat ─────────────────────────────────────────────────────────

/// Asymmetric polynomial waveshaper with tube-like even-harmonic emphasis.
///
/// Chain: polynomial `y = x − k·x² − (x − k·x²)³ / 3`
/// → DC-blocking one-pole HP @ 15 Hz → one-pole LP @ 15 kHz.
pub struct TubeSat<A: Signal, D: Signal> {
    source: A,
    drive: Param<D>,
    k: f32,
    lp_state: f32,
    dc_lp_state: f32,
    lp_alpha: f32,
    dc_alpha: f32,
    sr: f32,
    initialised: bool,
}

const TUBE_LP_HZ: f32 = 15_000.0;
const TUBE_DC_HP_HZ: f32 = 15.0;
const TUBE_K: f32 = 0.2;

impl<A: Signal, D: Signal> Signal for TubeSat<A, D> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        if !self.initialised || self.sr != ctx.sample_rate {
            self.sr = ctx.sample_rate;
            self.lp_alpha = one_pole_alpha(TUBE_LP_HZ, ctx.sample_rate);
            self.dc_alpha = one_pole_alpha(TUBE_DC_HP_HZ, ctx.sample_rate);
            self.initialised = true;
        }

        // Pre-limit with `tanh` so the polynomial never sees |x| > 1. The
        // `y − y³/3` term diverges above |y| ≈ 1.5 — the `tanh` keeps the
        // input inside the polynomial's valid operating range regardless
        // of how much drive or input amplitude the caller throws at us.
        let x = (self.source.next(ctx) * self.drive.next(ctx).max(0.0)).tanh();

        // Even-harmonic emphasis (x²) then odd-order soft compression.
        let a = x - self.k * x * x;
        let shaped = a - (a * a * a) / 3.0;

        // DC-blocking HP (x² injects DC whenever x has non-zero RMS).
        self.dc_lp_state += self.dc_alpha * (shaped - self.dc_lp_state);
        let dc_free = shaped - self.dc_lp_state;

        // Post LP.
        self.lp_state += self.lp_alpha * (dc_free - self.lp_state);
        self.lp_state
    }
}

// ─── DiodeClip ───────────────────────────────────────────────────────

/// Algebraic soft-clipper — sharper knee than `tanh`.
///
/// `y = x / (1 + |x · drive|)`. Cheap (one divide, one abs, no `exp`)
/// and produces the hard-edged character of diode/transistor clipping.
/// No filtering — pure waveshaper; combine with your own EQ as needed.
pub struct DiodeClip<A: Signal, D: Signal> {
    source: A,
    drive: Param<D>,
}

impl<A: Signal, D: Signal> Signal for DiodeClip<A, D> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let x = self.source.next(ctx);
        let drive = self.drive.next(ctx).max(0.0);
        let driven = x * drive;
        driven / (1.0 + driven.abs())
    }
}

// ─── Extension trait ─────────────────────────────────────────────────

/// Adds `.tape_sat()`, `.tube_sat()`, `.diode_clip()` to every `Signal`.
pub trait SaturationExt: Signal + Sized {
    /// Tape-style asymmetric saturation with HF rolloff. `drive` of 1.0
    /// is near-transparent; 2.0 is audible colour; 10.0 is heavy.
    fn tape_sat<D: IntoParam>(self, drive: D) -> TapeSat<Self, D::Signal> {
        TapeSat {
            source: self,
            drive: drive.into_param(),
            bias: TAPE_BIAS,
            pre_lp_state: 0.0,
            post_lp_state: 0.0,
            pre_alpha: 0.0,
            post_alpha: 0.0,
            sr: 0.0,
            initialised: false,
        }
    }

    /// Tube-style polynomial saturation with even-harmonic emphasis.
    fn tube_sat<D: IntoParam>(self, drive: D) -> TubeSat<Self, D::Signal> {
        TubeSat {
            source: self,
            drive: drive.into_param(),
            k: TUBE_K,
            lp_state: 0.0,
            dc_lp_state: 0.0,
            lp_alpha: 0.0,
            dc_alpha: 0.0,
            sr: 0.0,
            initialised: false,
        }
    }

    /// Diode-style algebraic soft-clip. Sharp knee, pure waveshaper.
    fn diode_clip<D: IntoParam>(self, drive: D) -> DiodeClip<Self, D::Signal> {
        DiodeClip {
            source: self,
            drive: drive.into_param(),
        }
    }
}

impl<T: Signal + Sized> SaturationExt for T {}
