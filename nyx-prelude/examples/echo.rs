//! Echo: a plucked synth into a 3/8-beat delay with 50% feedback.
//!
//! Demonstrates the `.delay().feedback().mix()` builder chain. The
//! SubSynth retriggers every bar; the delay turns each pluck into a
//! cascade of echoes.
//!
//! Run: cargo run -p nyx-prelude --example echo --release

use nyx_prelude::*;

fn main() {
    let mut clk = clock::clock(120.0);

    // Bright square-wave pluck with short envelope
    let patch = SynthPatch {
        osc_shape: OscShape::Square,
        filter_cutoff: 3000.0,
        filter_q: 2.0,
        attack: 0.001,
        decay: 0.15,
        sustain: 0.0,
        release: 0.01,
        gain: 0.35,
        ..Default::default()
    };
    let mut lead = patch.build();

    // 3/8-beat delay at 120 BPM = 750 ms — half-time echo feel
    let mut last_beat = -1i32;
    let voice = move |ctx: &AudioContext| {
        let state = clk.tick(ctx);
        let beat = state.beat as i32;
        if beat != last_beat {
            last_beat = beat;
            // Walking bassline on every beat: A3, E4, A3, G3
            let notes = [
                Note::from_midi(57), // A3
                Note::E4,
                Note::from_midi(57), // A3
                Note::from_midi(55), // G3
            ];
            let note = notes[(beat.rem_euclid(4)) as usize];
            lead.set_frequency(note.to_freq());
            lead.trigger();
        }
        lead.next(ctx)
    };

    let signal = voice.delay(0.75).feedback(0.5).mix(0.45);

    play(signal).unwrap();
}
