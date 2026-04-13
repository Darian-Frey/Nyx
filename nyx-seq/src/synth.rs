//! SubSynth: a configurable synthesizer template.
//!
//! Architecture: oscillator → filter → ADSR → gain.
//! Parameters are stored in a `SynthPatch` which is serialisable via serde.
//!
//! **Note:** `dyn Signal` is not serialisable. Only the `SynthPatch` config
//! is saved/loaded. The actual signal chain is rebuilt from the patch.

use nyx_core::{AudioContext, Signal};
use crate::envelope::{self, Adsr};
use serde::{Deserialize, Serialize};

/// Oscillator waveform selection for the SubSynth.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum OscShape {
    Sine,
    Saw,
    Square,
    Triangle,
}

/// Filter type selection for the SubSynth.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum FilterType {
    LowPass,
    HighPass,
    Bypass,
}

/// A serialisable synth patch. All parameters needed to reconstruct a SubSynth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynthPatch {
    pub name: String,
    pub osc_shape: OscShape,
    pub frequency: f32,
    pub filter_type: FilterType,
    pub filter_cutoff: f32,
    pub filter_q: f32,
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
    pub gain: f32,
}

impl Default for SynthPatch {
    fn default() -> Self {
        Self {
            name: "Init".to_string(),
            osc_shape: OscShape::Saw,
            frequency: 440.0,
            filter_type: FilterType::LowPass,
            filter_cutoff: 2000.0,
            filter_q: 0.707,
            attack: 0.01,
            decay: 0.1,
            sustain: 0.7,
            release: 0.3,
            gain: 0.8,
        }
    }
}

impl SynthPatch {
    /// Save this patch to a TOML file.
    pub fn save(&self, path: &str) -> Result<(), PatchError> {
        let toml_str = toml::to_string_pretty(self)
            .map_err(|e| PatchError::Serialize(e.to_string()))?;
        std::fs::write(path, toml_str)
            .map_err(|e| PatchError::Io(e.to_string()))
    }

    /// Load a patch from a TOML file.
    pub fn load(path: &str) -> Result<Self, PatchError> {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| PatchError::Io(e.to_string()))?;
        toml::from_str(&contents)
            .map_err(|e| PatchError::Deserialize(e.to_string()))
    }

    /// Build a `SubSynth` from this patch.
    pub fn build(&self) -> SubSynth {
        SubSynth::from_patch(self.clone())
    }
}

/// Errors from patch save/load.
#[derive(Debug)]
pub enum PatchError {
    Serialize(String),
    Deserialize(String),
    Io(String),
}

impl std::fmt::Display for PatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PatchError::Serialize(e) => write!(f, "serialization error: {e}"),
            PatchError::Deserialize(e) => write!(f, "deserialization error: {e}"),
            PatchError::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for PatchError {}

/// A subtractive synthesizer: oscillator → biquad filter → ADSR envelope → gain.
///
/// Built from a `SynthPatch`. Call `.trigger()` to start and `.release()` to stop.
pub struct SubSynth {
    patch: SynthPatch,
    osc_phase: f32,
    env: Adsr,
    // Biquad TDF-II state
    s1: f32,
    s2: f32,
}

impl SubSynth {
    /// Create a SubSynth from a patch.
    pub fn from_patch(patch: SynthPatch) -> Self {
        let env = envelope::adsr(patch.attack, patch.decay, patch.sustain, patch.release);
        SubSynth {
            patch,
            osc_phase: 0.0,
            env,
            s1: 0.0,
            s2: 0.0,
        }
    }

    /// Trigger the synth (note-on).
    pub fn trigger(&mut self) {
        self.env.trigger();
    }

    /// Release the synth (note-off).
    pub fn release(&mut self) {
        self.env.release();
    }

    /// Set the oscillator frequency (e.g. for note changes).
    pub fn set_frequency(&mut self, freq: f32) {
        self.patch.frequency = freq;
    }

    /// Get the current patch.
    pub fn patch(&self) -> &SynthPatch {
        &self.patch
    }

    fn osc_sample(&mut self, ctx: &AudioContext) -> f32 {
        let out = match self.patch.osc_shape {
            OscShape::Sine => (self.osc_phase * std::f32::consts::TAU).sin(),
            OscShape::Saw => 2.0 * self.osc_phase - 1.0,
            OscShape::Square => {
                if self.osc_phase < 0.5 { 1.0 } else { -1.0 }
            }
            OscShape::Triangle => {
                if self.osc_phase < 0.5 {
                    4.0 * self.osc_phase - 1.0
                } else {
                    3.0 - 4.0 * self.osc_phase
                }
            }
        };
        self.osc_phase += self.patch.frequency / ctx.sample_rate;
        self.osc_phase -= self.osc_phase.floor();
        out
    }

    fn filter_sample(&mut self, input: f32, ctx: &AudioContext) -> f32 {
        match self.patch.filter_type {
            FilterType::Bypass => input,
            FilterType::LowPass | FilterType::HighPass => {
                let omega = std::f32::consts::TAU * self.patch.filter_cutoff / ctx.sample_rate;
                let sin_w = omega.sin();
                let cos_w = omega.cos();
                let alpha = sin_w / (2.0 * self.patch.filter_q);

                let (b0, b1, b2, a1, a2) = match self.patch.filter_type {
                    FilterType::LowPass => {
                        let a0 = 1.0 + alpha;
                        (
                            ((1.0 - cos_w) / 2.0) / a0,
                            (1.0 - cos_w) / a0,
                            ((1.0 - cos_w) / 2.0) / a0,
                            (-2.0 * cos_w) / a0,
                            (1.0 - alpha) / a0,
                        )
                    }
                    FilterType::HighPass => {
                        let a0 = 1.0 + alpha;
                        (
                            ((1.0 + cos_w) / 2.0) / a0,
                            -(1.0 + cos_w) / a0,
                            ((1.0 + cos_w) / 2.0) / a0,
                            (-2.0 * cos_w) / a0,
                            (1.0 - alpha) / a0,
                        )
                    }
                    FilterType::Bypass => unreachable!(),
                };

                // TDF-II
                let output = b0 * input + self.s1;
                self.s1 = b1 * input - a1 * output + self.s2;
                self.s2 = b2 * input - a2 * output;
                output
            }
        }
    }
}

impl Signal for SubSynth {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let osc = self.osc_sample(ctx);
        let filtered = self.filter_sample(osc, ctx);
        let env = self.env.next(ctx);
        filtered * env * self.patch.gain
    }
}
