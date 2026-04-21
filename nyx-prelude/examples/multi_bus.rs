//! Bus architecture — drum bus, harmony bus, reverb send, master bus.
//!
//! Demonstrates the `Bus` mixer: group signals, apply shared processing
//! (compression on drums, reverb on harmony), then fold everything into
//! a soft-clipped master bus. This is how a DAW mix tree maps to Nyx:
//!
//! ```
//!   kick ──┐
//!   snare ─┼── drum_bus ─ compress ───────┐
//!   hat ───┘                              │
//!                                         ├── master ─ soft_clip ─> out
//!   pad ──── harmony_bus ─ freeverb ──────┘
//!   bass ── (dry) ────────────────────────┘
//! ```
//!
//! Run: cargo run -p nyx-prelude --example multi_bus --release

use nyx_prelude::*;
use std::f32::consts::TAU;

const BPM: f32 = 110.0;

/// Self-triggering kick at `bpm`.
fn kick_at_bpm(bpm: f32) -> impl Signal {
    let mut phase = 0.0_f32;
    let mut beat_samples: u64 = 0;
    move |ctx: &AudioContext| -> f32 {
        if beat_samples == 0 {
            beat_samples = (60.0 / bpm * ctx.sample_rate) as u64;
        }
        let beat_pos = ctx.tick % beat_samples;
        let t = beat_pos as f32 / ctx.sample_rate;
        let env = (-t * 32.0).exp();
        let freq = 50.0 + 80.0 * env * env;
        phase += freq / ctx.sample_rate;
        phase -= phase.floor();
        (phase * TAU).sin() * env * 0.9
    }
}

/// Simple off-beat closed-hat tick.
fn hat_at_bpm(bpm: f32) -> impl Signal {
    let mut rng_state: u32 = 0xA55A;
    let mut beat_samples: u64 = 0;
    move |ctx: &AudioContext| -> f32 {
        if beat_samples == 0 {
            beat_samples = (60.0 / bpm * ctx.sample_rate) as u64;
        }
        let half = beat_samples / 2;
        let beat_pos = ctx.tick % beat_samples;
        if beat_pos < half {
            return 0.0;
        }
        let t = (beat_pos - half) as f32 / ctx.sample_rate;
        let env = (-t * 120.0).exp();
        rng_state = rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        let n = (rng_state >> 16) as f32 / 32768.0 - 1.0;
        n * env * 0.2
    }
}

/// Snare on beats 2 & 4 (half-period offset from kick).
fn snare_at_bpm(bpm: f32) -> impl Signal {
    let mut rng_state: u32 = 0x1234;
    let mut tone_phase = 0.0_f32;
    let mut bar_samples: u64 = 0;
    move |ctx: &AudioContext| -> f32 {
        if bar_samples == 0 {
            // 4-beat bar.
            bar_samples = (60.0 / bpm * 4.0 * ctx.sample_rate) as u64;
        }
        let beat_samples = bar_samples / 4;
        let pos = ctx.tick % bar_samples;
        let beat = pos / beat_samples;
        // Fire on beats 1 and 3 (0-indexed: 1, 3).
        if beat != 1 && beat != 3 {
            return 0.0;
        }
        let local = pos % beat_samples;
        let t = local as f32 / ctx.sample_rate;
        let env = (-t * 25.0).exp();
        rng_state = rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        let noise = (rng_state >> 16) as f32 / 32768.0 - 1.0;
        tone_phase += 200.0 / ctx.sample_rate;
        tone_phase -= tone_phase.floor();
        let tone = (tone_phase * TAU).sin();
        (noise * 0.6 + tone * 0.4) * env * 0.5
    }
}

fn main() {
    // ─── Drum bus: kick + snare + hat, bus-compressed ───────────────
    let drums = Bus::with_capacity(3)
        .add(kick_at_bpm(BPM))
        .add(snare_at_bpm(BPM))
        .add(hat_at_bpm(BPM))
        .gain(0.85)
        .compress(-10.0, 4.0)
        .attack_ms(5.0)
        .release_ms(80.0)
        .makeup_db(2.0);

    // ─── Harmony bus: 3 detuned saws (Am triad), shared reverb ──────
    let pad = Bus::with_capacity(3)
        .add(osc::saw(Note::from_midi(57).to_freq())) // A3
        .add(osc::saw(Note::C4.to_freq()))
        .add(osc::saw(Note::E4.to_freq()))
        .gain(0.08)
        .lowpass(1800.0, 0.7)
        .freeverb()
        .room_size(0.85)
        .damping(0.4)
        .wet(0.45);

    // ─── Bass: dry, untouched by bus effects ────────────────────────
    let bass = osc::saw(Note::from_midi(33).to_freq()) // A1
        .lowpass(500.0, 1.0)
        .amp(0.22);

    // ─── Master bus: everything sums, soft-clipped at the output ────
    let master = Bus::with_capacity(3)
        .add(drums)
        .add(pad)
        .add(bass)
        .soft_clip(1.1);

    play(master).unwrap();
}
