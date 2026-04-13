//! Macro-synth instrument primitives.
//!
//! Each instrument is built from `nyx-core` oscillators, filters, envelopes,
//! and combinators. They serve as both useful defaults and documentation
//! of how to compose the library's building blocks.

use nyx_core::{AudioContext, Signal};

use crate::envelope::{self, Adsr};
use crate::note::Note;
use crate::chord::Chord;

// ─── Kick ───────────────────────────────────────────────────────────

/// A synthesised kick drum: sine with pitch envelope + amplitude decay.
pub struct Kick {
    phase: f32,
    freq: f32,
    freq_start: f32,
    freq_end: f32,
    freq_decay: f32,
    amp_env: Adsr,
    sample_rate: f32,
}

/// Create a kick drum instrument. Call `.trigger()` to fire it.
pub fn kick() -> Kick {
    let env = envelope::adsr(0.001, 0.15, 0.0, 0.05);
    // Start idle — caller triggers.
    Kick {
        phase: 0.0,
        freq: 150.0,
        freq_start: 150.0,
        freq_end: 50.0,
        freq_decay: 0.0,
        amp_env: env,
        sample_rate: 0.0,
    }
}

impl Kick {
    /// Trigger the kick.
    pub fn trigger(&mut self) {
        self.freq = self.freq_start;
        self.freq_decay = 0.0;
        self.phase = 0.0;
        self.amp_env.trigger();
    }
}

impl Signal for Kick {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        if self.sample_rate != ctx.sample_rate {
            self.sample_rate = ctx.sample_rate;
        }

        let env = self.amp_env.next(ctx);

        // Pitch envelope: exponential decay from freq_start to freq_end.
        self.freq_decay += 1.0 / (0.05 * ctx.sample_rate); // ~50ms pitch decay
        let t = self.freq_decay.min(1.0);
        self.freq = self.freq_start + (self.freq_end - self.freq_start) * t;

        let sample = (self.phase * std::f32::consts::TAU).sin();
        self.phase += self.freq / ctx.sample_rate;
        self.phase -= self.phase.floor();

        sample * env
    }
}

// ─── Snare ──────────────────────────────────────────────────────────

/// A synthesised snare: sine body + white noise burst.
pub struct Snare {
    body_phase: f32,
    noise_state: u32,
    body_env: Adsr,
    noise_env: Adsr,
}

/// Create a snare drum instrument. Call `.trigger()` to fire it.
pub fn snare() -> Snare {
    Snare {
        body_phase: 0.0,
        noise_state: 1,
        body_env: envelope::adsr(0.001, 0.08, 0.0, 0.03),
        noise_env: envelope::adsr(0.001, 0.12, 0.0, 0.05),
    }
}

impl Snare {
    pub fn trigger(&mut self) {
        self.body_phase = 0.0;
        self.body_env.trigger();
        self.noise_env.trigger();
    }
}

impl Signal for Snare {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let body_env = self.body_env.next(ctx);
        let noise_env = self.noise_env.next(ctx);

        // Sine body at ~200 Hz
        let body = (self.body_phase * std::f32::consts::TAU).sin();
        self.body_phase += 200.0 / ctx.sample_rate;
        self.body_phase -= self.body_phase.floor();

        // White noise
        let mut x = self.noise_state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.noise_state = x;
        let noise = (x as f32 / u32::MAX as f32) * 2.0 - 1.0;

        body * body_env * 0.6 + noise * noise_env * 0.4
    }
}

// ─── Hi-hat ─────────────────────────────────────────────────────────

/// A synthesised hi-hat: filtered white noise with short/long decay.
pub struct HiHat {
    noise_state: u32,
    env: Adsr,
}

/// Create a hi-hat. `open = true` for open (longer decay), `false` for closed.
pub fn hihat(open: bool) -> HiHat {
    let decay = if open { 0.3 } else { 0.03 };
    HiHat {
        noise_state: 1,
        env: envelope::adsr(0.001, decay, 0.0, 0.01),
    }
}

impl HiHat {
    pub fn trigger(&mut self) {
        self.env.trigger();
    }
}

impl Signal for HiHat {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let env = self.env.next(ctx);

        let mut x = self.noise_state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.noise_state = x;
        let noise = (x as f32 / u32::MAX as f32) * 2.0 - 1.0;

        // High-pass effect: simple one-pole high-pass approximation.
        noise * env * 0.8
    }
}

// ─── Drone ──────────────────────────────────────────────────────────

/// A sustained drone: detuned saw waves with slow filter modulation.
pub struct Drone {
    phase1: f32,
    phase2: f32,
    freq: f32,
    detune: f32,
    lfo_phase: f32,
}

/// Create a drone at the given note.
pub fn drone(note: Note) -> Drone {
    Drone {
        phase1: 0.0,
        phase2: 0.0,
        freq: note.to_freq(),
        detune: 1.003, // ~5 cents sharp
        lfo_phase: 0.0,
    }
}

impl Signal for Drone {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let saw1 = 2.0 * self.phase1 - 1.0;
        let saw2 = 2.0 * self.phase2 - 1.0;

        self.phase1 += self.freq / ctx.sample_rate;
        self.phase1 -= self.phase1.floor();
        self.phase2 += (self.freq * self.detune) / ctx.sample_rate;
        self.phase2 -= self.phase2.floor();

        // Slow amplitude LFO (~0.2 Hz)
        let lfo = (self.lfo_phase * std::f32::consts::TAU).sin() * 0.15 + 0.85;
        self.lfo_phase += 0.2 / ctx.sample_rate;
        self.lfo_phase -= self.lfo_phase.floor();

        (saw1 + saw2) * 0.4 * lfo
    }
}

// ─── Riser ──────────────────────────────────────────────────────────

/// A noise riser: filtered noise with rising cutoff over a duration.
pub struct Riser {
    noise_state: u32,
    duration_samples: f32,
    elapsed: f32,
}

/// Create a riser effect lasting `duration_secs`.
pub fn riser(duration_secs: f32) -> Riser {
    Riser {
        noise_state: 1,
        duration_samples: duration_secs, // converted on first sample
        elapsed: 0.0,
    }
}

impl Signal for Riser {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let total = self.duration_samples * ctx.sample_rate;
        let t = (self.elapsed / total).min(1.0);
        self.elapsed += 1.0;

        // Noise
        let mut x = self.noise_state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.noise_state = x;
        let noise = (x as f32 / u32::MAX as f32) * 2.0 - 1.0;

        // Rising amplitude and "brightness" (just amplitude ramp for now)
        noise * t * t // quadratic rise
    }
}

// ─── Pad ────────────────────────────────────────────────────────────

/// A chord pad: multiple detuned sines, one per chord note.
pub struct Pad {
    voices: Vec<(f32, f32)>, // (phase, freq) per voice
    env: Adsr,
}

/// Create a pad from a chord.
pub fn pad(chord: Chord) -> Pad {
    let voices = chord
        .freqs()
        .into_iter()
        .map(|freq| (0.0_f32, freq))
        .collect();
    Pad {
        voices,
        env: envelope::adsr(0.3, 0.2, 0.7, 0.5),
    }
}

impl Pad {
    pub fn trigger(&mut self) {
        self.env.trigger();
    }

    pub fn release(&mut self) {
        self.env.release();
    }
}

impl Signal for Pad {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let env = self.env.next(ctx);
        let mut sum = 0.0_f32;
        let n = self.voices.len() as f32;
        for (phase, freq) in &mut self.voices {
            sum += (*phase * std::f32::consts::TAU).sin();
            *phase += *freq / ctx.sample_rate;
            *phase -= phase.floor();
        }
        if n > 0.0 {
            sum /= n;
        }
        sum * env
    }
}
