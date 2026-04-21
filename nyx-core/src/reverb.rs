//! Freeverb — the classic public-domain reverb algorithm.
//!
//! Implementation of Jezar's Freeverb (2000): 8 parallel lowpass-feedback
//! comb filters feeding 4 series Schroeder all-pass filters, with a
//! stereo spread on the right channel. This is the most-copied reverb
//! in open-source audio software.
//!
//! # Example
//!
//! ```ignore
//! use nyx_prelude::*;
//!
//! // A pad with lush reverb
//! let pad = osc::sine(Note::C4.to_freq())
//!     .add(osc::sine(Note::E4.to_freq()))
//!     .add(osc::sine(Note::G4.to_freq()))
//!     .amp(0.15);
//!
//! let wet = pad.freeverb()
//!     .room_size(0.85)
//!     .damping(0.5)
//!     .wet(0.4);
//!
//! play(wet).unwrap();
//! ```
//!
//! # Architecture
//!
//! The reverb outputs genuine stereo via `next_stereo` (the Sprint 2
//! stereo refactor). `next` folds `L + R` for mono playback.
//!
//! All buffers are allocated once at construction (sized for 96 kHz
//! upper bound). Zero allocation per sample. Feedback and damping
//! parameters are clamped to safe ranges internally.

use crate::signal::{AudioContext, Signal};

// Original Freeverb comb lengths (samples @ 44.1 kHz).
const COMB_LENGTHS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];

// Original Freeverb allpass lengths (samples @ 44.1 kHz).
const ALLPASS_LENGTHS: [usize; 4] = [225, 556, 441, 341];

// Right-channel stereo spread (samples added to each comb/allpass).
const STEREO_SPREAD: usize = 23;

// Freeverb's magic constants.
const FIXED_GAIN: f32 = 0.015;
const ALLPASS_FEEDBACK: f32 = 0.5;
const ROOM_SCALE: f32 = 0.28;
const ROOM_OFFSET: f32 = 0.7;
const DAMP_SCALE: f32 = 0.4;

/// Upper-bound sample rate used to size buffers at construction.
const FREEVERB_MAX_SR: f32 = 96_000.0;

fn scaled_len(base: usize, sr: f32) -> usize {
    ((base as f32 * sr / 44_100.0).round() as usize).max(1)
}

fn max_buffer_size(base: usize) -> usize {
    scaled_len(base, FREEVERB_MAX_SR) + 1
}

/// Lowpass-feedback comb filter used in each Freeverb channel.
struct Comb {
    buffer: Box<[f32]>,
    active_len: usize,
    idx: usize,
    feedback: f32,
    damp: f32,
    filter_store: f32,
}

impl Comb {
    fn new(base_len: usize) -> Self {
        Comb {
            buffer: vec![0.0; max_buffer_size(base_len)].into_boxed_slice(),
            active_len: base_len,
            idx: 0,
            feedback: 0.5,
            damp: 0.5,
            filter_store: 0.0,
        }
    }

    fn set_active_len(&mut self, n: usize) {
        self.active_len = n.clamp(1, self.buffer.len());
    }

    #[inline(always)]
    fn process(&mut self, input: f32) -> f32 {
        let out = self.buffer[self.idx];
        // One-pole lowpass in the feedback path controls high-frequency
        // decay — "damping" in the Freeverb sense.
        self.filter_store = out * (1.0 - self.damp) + self.filter_store * self.damp;
        self.buffer[self.idx] = input + self.filter_store * self.feedback;
        self.idx = (self.idx + 1) % self.active_len;
        out
    }
}

/// Schroeder all-pass filter used in series after the comb bank.
struct Allpass {
    buffer: Box<[f32]>,
    active_len: usize,
    idx: usize,
}

impl Allpass {
    fn new(base_len: usize) -> Self {
        Allpass {
            buffer: vec![0.0; max_buffer_size(base_len)].into_boxed_slice(),
            active_len: base_len,
            idx: 0,
        }
    }

    fn set_active_len(&mut self, n: usize) {
        self.active_len = n.clamp(1, self.buffer.len());
    }

    #[inline(always)]
    fn process(&mut self, input: f32) -> f32 {
        let buf_out = self.buffer[self.idx];
        let out = -input + buf_out;
        self.buffer[self.idx] = input + buf_out * ALLPASS_FEEDBACK;
        self.idx = (self.idx + 1) % self.active_len;
        out
    }
}

/// Freeverb stereo reverb. Build via
/// [`SignalExt::freeverb`](crate::SignalExt::freeverb).
pub struct Freeverb<A: Signal> {
    source: A,
    combs_l: [Comb; 8],
    combs_r: [Comb; 8],
    allpasses_l: [Allpass; 4],
    allpasses_r: [Allpass; 4],
    room_size: f32,
    damping: f32,
    wet_mix: f32,
    width: f32,
    // Derived from wet + width, recomputed on change.
    wet1: f32,
    wet2: f32,
    initialised: bool,
}

impl<A: Signal> Freeverb<A> {
    pub(crate) fn new(source: A) -> Self {
        let combs_l: [Comb; 8] = std::array::from_fn(|i| Comb::new(COMB_LENGTHS[i]));
        let combs_r: [Comb; 8] =
            std::array::from_fn(|i| Comb::new(COMB_LENGTHS[i] + STEREO_SPREAD));
        let allpasses_l: [Allpass; 4] = std::array::from_fn(|i| Allpass::new(ALLPASS_LENGTHS[i]));
        let allpasses_r: [Allpass; 4] =
            std::array::from_fn(|i| Allpass::new(ALLPASS_LENGTHS[i] + STEREO_SPREAD));

        let mut rv = Freeverb {
            source,
            combs_l,
            combs_r,
            allpasses_l,
            allpasses_r,
            room_size: 0.5,
            damping: 0.5,
            wet_mix: 0.3,
            width: 1.0,
            wet1: 0.0,
            wet2: 0.0,
            initialised: false,
        };
        rv.recompute_comb_params();
        rv.recompute_wet();
        rv
    }

    /// Room size in `[0, 1]`. Higher values give a longer decay tail.
    pub fn room_size(mut self, r: f32) -> Self {
        self.room_size = r.clamp(0.0, 1.0);
        self.recompute_comb_params();
        self
    }

    /// High-frequency damping in `[0, 1]`. Higher values absorb highs
    /// faster, giving a warmer/darker tail.
    pub fn damping(mut self, d: f32) -> Self {
        self.damping = d.clamp(0.0, 1.0);
        self.recompute_comb_params();
        self
    }

    /// Wet/dry mix in `[0, 1]`. `0` = dry input only, `1` = pure reverb.
    pub fn wet(mut self, w: f32) -> Self {
        self.wet_mix = w.clamp(0.0, 1.0);
        self.recompute_wet();
        self
    }

    /// Stereo width in `[0, 1]`. `1` = full stereo spread, `0` = mono
    /// reverb (both channels identical).
    pub fn width(mut self, w: f32) -> Self {
        self.width = w.clamp(0.0, 1.0);
        self.recompute_wet();
        self
    }

    fn recompute_comb_params(&mut self) {
        let feedback = self.room_size * ROOM_SCALE + ROOM_OFFSET;
        let damp = self.damping * DAMP_SCALE;
        for c in self.combs_l.iter_mut().chain(self.combs_r.iter_mut()) {
            c.feedback = feedback;
            c.damp = damp;
        }
    }

    fn recompute_wet(&mut self) {
        self.wet1 = self.wet_mix * (self.width / 2.0 + 0.5);
        self.wet2 = self.wet_mix * ((1.0 - self.width) / 2.0);
    }

    fn initialise_for_sample_rate(&mut self, sr: f32) {
        for (i, c) in self.combs_l.iter_mut().enumerate() {
            c.set_active_len(scaled_len(COMB_LENGTHS[i], sr));
        }
        for (i, c) in self.combs_r.iter_mut().enumerate() {
            c.set_active_len(scaled_len(COMB_LENGTHS[i] + STEREO_SPREAD, sr));
        }
        for (i, a) in self.allpasses_l.iter_mut().enumerate() {
            a.set_active_len(scaled_len(ALLPASS_LENGTHS[i], sr));
        }
        for (i, a) in self.allpasses_r.iter_mut().enumerate() {
            a.set_active_len(scaled_len(ALLPASS_LENGTHS[i] + STEREO_SPREAD, sr));
        }
        self.initialised = true;
    }

    #[inline(always)]
    fn process_stereo(&mut self, input: f32) -> (f32, f32) {
        let driven = input * FIXED_GAIN;

        // Parallel comb banks.
        let mut wet_l = 0.0_f32;
        let mut wet_r = 0.0_f32;
        for c in self.combs_l.iter_mut() {
            wet_l += c.process(driven);
        }
        for c in self.combs_r.iter_mut() {
            wet_r += c.process(driven);
        }

        // Series allpass chains.
        for a in self.allpasses_l.iter_mut() {
            wet_l = a.process(wet_l);
        }
        for a in self.allpasses_r.iter_mut() {
            wet_r = a.process(wet_r);
        }

        // Stereo spread: width cross-mixes the two channels.
        let out_l = wet_l * self.wet1 + wet_r * self.wet2;
        let out_r = wet_r * self.wet1 + wet_l * self.wet2;
        (out_l, out_r)
    }
}

impl<A: Signal> Signal for Freeverb<A> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        if !self.initialised {
            self.initialise_for_sample_rate(ctx.sample_rate);
        }
        let dry = self.source.next(ctx);
        let (wet_l, wet_r) = self.process_stereo(dry);
        // Mono output: average wet channels so wet=0 recovers the dry
        // signal exactly (no doubling) and mono playback of the reverb
        // tail is energy-preserving.
        let wet_mono = (wet_l + wet_r) * 0.5;
        let dry_mix = 1.0 - self.wet_mix;
        dry * dry_mix + wet_mono
    }

    fn next_stereo(&mut self, ctx: &AudioContext) -> (f32, f32) {
        if !self.initialised {
            self.initialise_for_sample_rate(ctx.sample_rate);
        }
        let dry = self.source.next(ctx);
        let (wet_l, wet_r) = self.process_stereo(dry);
        // Stereo output: dry goes to both channels unattenuated; wet
        // is already cross-mixed for the stereo spread.
        let dry_mix = 1.0 - self.wet_mix;
        (dry * dry_mix + wet_l, dry * dry_mix + wet_r)
    }
}
