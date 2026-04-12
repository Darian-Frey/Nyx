//! Trigger-based ADSR envelope generator.
//!
//! The envelope is a `Signal` that outputs a value in [0, 1] based on
//! its current stage. It responds to `trigger()` and `release()` calls
//! from the main thread (or voice pool logic).

use nyx_core::{AudioContext, Signal};

/// ADSR envelope stages.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Stage {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

/// ADSR envelope generator.
///
/// Times are in seconds. Sustain is a level (0.0–1.0), not a time.
pub struct Adsr {
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
    stage: Stage,
    level: f32,
    // Precomputed rates (per-sample increments), set on trigger/release.
    attack_rate: f32,
    decay_rate: f32,
    release_rate: f32,
    sample_rate: f32,
}

/// Create an ADSR envelope with the given parameters.
///
/// - `attack`: time in seconds to ramp from 0 to 1
/// - `decay`: time in seconds to ramp from 1 to sustain level
/// - `sustain`: level to hold at (0.0–1.0)
/// - `release`: time in seconds to ramp from sustain to 0
pub fn adsr(attack: f32, decay: f32, sustain: f32, release: f32) -> Adsr {
    Adsr {
        attack,
        decay,
        sustain: sustain.clamp(0.0, 1.0),
        release,
        stage: Stage::Idle,
        level: 0.0,
        attack_rate: 0.0,
        decay_rate: 0.0,
        release_rate: 0.0,
        sample_rate: 0.0,
    }
}

impl Adsr {
    /// Trigger the envelope (note-on). Starts the attack stage.
    pub fn trigger(&mut self) {
        self.stage = Stage::Attack;
    }

    /// Release the envelope (note-off). Starts the release stage
    /// from the current level.
    pub fn release(&mut self) {
        if self.stage != Stage::Idle {
            self.stage = Stage::Release;
        }
    }

    /// Returns the current envelope stage.
    pub fn stage(&self) -> Stage {
        self.stage
    }

    /// Returns true if the envelope has finished (idle).
    pub fn is_idle(&self) -> bool {
        self.stage == Stage::Idle
    }

    fn compute_rates(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        let sr = sample_rate;
        self.attack_rate = if self.attack > 0.0 {
            1.0 / (self.attack * sr)
        } else {
            1.0 // instant
        };
        self.decay_rate = if self.decay > 0.0 {
            (1.0 - self.sustain) / (self.decay * sr)
        } else {
            1.0
        };
        self.release_rate = if self.release > 0.0 {
            self.sustain / (self.release * sr)
        } else {
            1.0
        };
    }
}

impl Signal for Adsr {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        // Recompute rates if sample rate changed (or first call).
        if ctx.sample_rate != self.sample_rate {
            self.compute_rates(ctx.sample_rate);
        }

        match self.stage {
            Stage::Idle => {
                self.level = 0.0;
            }
            Stage::Attack => {
                self.level += self.attack_rate;
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.stage = Stage::Decay;
                }
            }
            Stage::Decay => {
                self.level -= self.decay_rate;
                if self.level <= self.sustain {
                    self.level = self.sustain;
                    self.stage = Stage::Sustain;
                }
            }
            Stage::Sustain => {
                self.level = self.sustain;
            }
            Stage::Release => {
                self.level -= self.release_rate;
                if self.level <= 0.0 {
                    self.level = 0.0;
                    self.stage = Stage::Idle;
                }
            }
        }

        self.level
    }
}
