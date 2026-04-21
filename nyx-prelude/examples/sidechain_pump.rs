//! Sidechain compression — the classic trance / house "pumping bass".
//!
//! A 128 BPM four-on-the-floor kick drives both the audible drum layer
//! *and* a sidechain detector on a sustained sub-bass chord. Every beat,
//! the kick ducks the bass hard, then releases it back up over ~150 ms —
//! the sonic inhale-exhale that defines the genre.
//!
//! Toggle `SIDECHAIN_ON` to hear the difference: with the effect off, the
//! mix is a muddy pile; with it on, the bass breathes around the kick.
//!
//! Run: cargo run -p nyx-prelude --example sidechain_pump --release

use nyx_prelude::*;
use std::f32::consts::TAU;

const BPM: f32 = 128.0;
const SIDECHAIN_ON: bool = true;

/// A self-triggering kick drum: one thump per beat at `bpm`. The pitch
/// drops from ~130 Hz to 50 Hz over ~30 ms for the classic thump.
fn kick_at_bpm(bpm: f32) -> impl Signal {
    let mut phase = 0.0_f32;
    let mut beat_samples: u64 = 0;
    move |ctx: &AudioContext| -> f32 {
        if beat_samples == 0 {
            beat_samples = (60.0 / bpm * ctx.sample_rate) as u64;
        }
        let beat_pos = ctx.tick % beat_samples;
        let t = beat_pos as f32 / ctx.sample_rate;
        let env = (-t * 30.0).exp();
        let freq = 50.0 + 80.0 * env * env;
        phase += freq / ctx.sample_rate;
        phase -= phase.floor();
        (phase * TAU).sin() * env
    }
}

fn main() {
    // Bass: sub sine + gentle saw for harmonics, low-passed.
    //   A1 (55 Hz) + octave E2 for a fat sub.
    let bass = osc::sine(55.0)
        .add(osc::sine(82.4).amp(0.5))
        .add(osc::saw(55.0).amp(0.2))
        .lowpass(500.0, 0.9)
        .amp(0.45);

    // Optional sidechain ducking: -30 dB threshold, 10:1 ratio, fast
    // attack, trance-length release. Kick trigger is an independent
    // copy of the kick generator (silent, used only for detection).
    let bass_mix: Box<dyn Signal> = if SIDECHAIN_ON {
        bass.sidechain(kick_at_bpm(BPM), -30.0, 10.0)
            .attack_ms(1.0)
            .release_ms(160.0)
            .makeup_db(2.0)
            .boxed()
    } else {
        bass.boxed()
    };

    // Audible kick layer (separate instance).
    let kick = kick_at_bpm(BPM).amp(0.9);

    // Gentle closed-hat layer on the off-beats for trance feel.
    let mut hat_phase = 0.0_f32;
    let mut hat_beat_samples: u64 = 0;
    let hat = move |ctx: &AudioContext| -> f32 {
        if hat_beat_samples == 0 {
            hat_beat_samples = (60.0 / BPM * ctx.sample_rate) as u64;
        }
        let half = hat_beat_samples / 2;
        let beat_pos = ctx.tick % hat_beat_samples;
        // Trigger on the off-beat (between kicks).
        let t = if beat_pos < half {
            return 0.0;
        } else {
            (beat_pos - half) as f32 / ctx.sample_rate
        };
        let env = (-t * 80.0).exp();
        hat_phase = (hat_phase * 1.1 + 0.37) % 1.0; // cheap noise
        (hat_phase - 0.5) * 2.0 * env * 0.15
    };

    let mix = kick.add(bass_mix).add(hat).soft_clip(1.1);
    play(mix).unwrap();
}
