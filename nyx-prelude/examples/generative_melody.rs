//! Generative melody: Euclidean rhythm + scale-snapped random notes + seeded RNG.
//!
//! Run: cargo run -p nyx-prelude --example generative_melody --release

use nyx_prelude::*;

fn main() {
    let mut clk = clock::clock(110.0);
    let mut rng = seeded(42);
    let scale = Scale::pentatonic("A");

    // Pre-roll 16 scale-degree indices into a pattern, triggered by Euclid(5, 8).
    let rhythm = Euclid::generate(5, 8);
    let mut rhythm_seq = Sequence::new(rhythm, 0.25);

    let low = Note::from_midi(57); // A3
    let high = Note::from_midi(81); // A5

    let mut synth = SynthPatch::default().build();
    synth.set_frequency(low.to_freq());

    let signal = move |ctx: &AudioContext| {
        let state = clk.tick(ctx);
        let ev = rhythm_seq.tick(&state);
        if ev.triggered && ev.value {
            let note = rng.next_note_in(&scale, low, high);
            synth.set_frequency(note.to_freq());
            synth.trigger();
        }
        synth.next(ctx) * 0.4
    };

    play(signal).unwrap();
}
