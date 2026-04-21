//! Karplus-Strong — a four-voice plucked Cmaj7 chord that rings out.
//!
//! Each `pluck()` is a single-shot voice that strikes once (at build
//! time) and decays naturally via its feedback-loop lowpass. Stacking
//! four of them sums to a shimmering plucked chord.
//!
//! Run: cargo run -p nyx-prelude --example pluck --release

use nyx_prelude::*;

fn main() {
    let chord = pluck(Note::C4.to_freq(), 0.996)
        .add(pluck(Note::from_midi(63).to_freq(), 0.996)) // Eb (minor)
        .add(pluck(Note::G4.to_freq(), 0.996))
        .add(pluck(Note::from_midi(70).to_freq(), 0.996)) // Bb (7th)
        .amp(0.25);

    play(chord).unwrap();
}
