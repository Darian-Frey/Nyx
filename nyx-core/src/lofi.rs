//! One-call lo-fi preset wrappers.
//!
//! Each method composes primitives from earlier sonic-character
//! modules ([`tape`](crate::tape), [`crush`](crate::crush),
//! [`filter`](crate::filter), [`osc::noise`](crate::osc::noise)) into
//! a finished aesthetic. Keep them one-liners so the resulting chain
//! is an IDE-readable "did what I expected" — if you want to tune a
//! single component, drop back to the underlying builders.
//!
//! ```ignore
//! use nyx_core::{osc, LofiExt, SignalExt};
//!
//! // Anything through a cassette deck.
//! let drums = osc::saw_bl(110.0).amp(0.5).cassette();
//!
//! // Beats under a dusty boom-bap veneer.
//! let loop_ = osc::saw_bl(220.0).amp(0.5).lofi_hiphop();
//!
//! // Heavier degradation — wobbly tape, aggressive HF loss.
//! let haunted = osc::sine(440.0).vhs();
//! ```
//!
//! These are opinionated defaults. For anything that needs custom
//! wow/flutter depth, saturation drive, crush bit-depth, or hiss
//! level, compose `.tape()`, `.bitcrush()`, and noise mixing
//! directly.

use crate::filter::FilterExt;
use crate::osc::noise;
use crate::signal::{Signal, SignalExt};
use crate::tape::TapeExt;

// Hiss levels expressed as peak amplitude. Pink noise has RMS roughly
// equal to the peak scale — these values are calibrated by ear to sit
// in the noise floor without drowning the source.
const HISS_CASSETTE: f32 = 0.015;
const HISS_LOFI_HIPHOP: f32 = 0.012;

/// Deterministic seeds per preset so the hiss texture is reproducible
/// across runs (important for live-diff reload and golden tests).
const SEED_CASSETTE: u32 = 0xCA55;
const SEED_LOFI_HIPHOP: u32 = 0xB007;

/// Adds preset lo-fi wrappers to every [`Signal`].
pub trait LofiExt: Signal + Sized {
    /// Classic cassette chain: tape wow/flutter/EQ/saturation + mild
    /// bit-crush + pink-noise hiss floor. Default `tape().age(0.5)`.
    fn cassette(self) -> impl Signal {
        let hiss = noise::pink(SEED_CASSETTE).amp(HISS_CASSETTE);
        self.tape().add(hiss).bitcrush(10)
    }

    /// Boom-bap character: slightly-aged tape, dusty HF rolloff at
    /// 4 kHz, a whisper of hiss. Use for drum loops and pad textures.
    fn lofi_hiphop(self) -> impl Signal {
        let hiss = noise::pink(SEED_LOFI_HIPHOP).amp(HISS_LOFI_HIPHOP);
        self.tape().age(0.7).lowpass(4_000.0, 0.707).add(hiss)
    }

    /// VHS-deck character: heavy wow, aggressive HF loss at 2.5 kHz,
    /// full drive. Makes anything feel like it was rescued from a
    /// second-hand tape.
    fn vhs(self) -> impl Signal {
        self.tape().age(1.0).lowpass(2_500.0, 0.707)
    }
}

impl<T: Signal + Sized> LofiExt for T {}
