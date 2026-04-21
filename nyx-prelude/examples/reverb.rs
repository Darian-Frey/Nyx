//! Lush Freeverb reverb on a slowly-evolving pad chord.
//!
//! Three detuned sine voices tuned to Cm7, run through a big room
//! (85% size) with moderate damping. The reverb outputs genuine stereo
//! via `next_stereo`, audible through stereo headphones or speakers.
//!
//! Run: cargo run -p nyx-prelude --example reverb --release

use nyx_prelude::*;

fn main() {
    // Cm7 chord: C4, Eb4, G4, Bb4. Slight detuning on each voice.
    let chord = osc::sine(Note::C4.to_freq())
        .add(osc::sine(Note::C4.to_freq() * 1.0007)) // detuned C
        .add(osc::sine(Note::from_midi(63).to_freq())) // Eb4
        .add(osc::sine(Note::G4.to_freq()))
        .add(osc::sine(Note::from_midi(70).to_freq())) // Bb4
        .amp(0.08);

    // Slow amplitude LFO for gentle swelling.
    let swell = osc::sine(0.2).amp(0.4).offset(0.6);

    let wet = chord
        .amp(swell)
        .freeverb()
        .room_size(0.88)
        .damping(0.55)
        .width(1.0)
        .wet(0.6);

    play(wet).unwrap();
}
