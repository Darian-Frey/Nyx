//! Chorus + Flanger showcase.
//!
//! Two layers:
//!   - A saw pad through chorus at 0.4 Hz / 4 ms depth — classic
//!     thickening that makes a mono source sound like an ensemble.
//!   - A saw bass through a heavy flanger (0.3 Hz, 2 ms, 70% feedback)
//!     — the jet-plane whoosh sweeping through the low end.
//!
//! Run: cargo run -p nyx-prelude --example chorus_flanger --release
//!
//! Use stereo speakers or headphones — both effects output real stereo.

use nyx_prelude::*;

fn main() {
    // Lush chorused pad (A3, C4, E4 — A minor triad)
    let pad = osc::saw(Note::from_midi(57).to_freq()) // A3
        .add(osc::saw(Note::C4.to_freq()))
        .add(osc::saw(Note::E4.to_freq()))
        .amp(0.1)
        .chorus(0.4, 4.0)
        .mix(0.55)
        .base_delay(22.0);

    // Heavy flanger bass
    let bass = osc::saw(Note::from_midi(33).to_freq()) // A1
        .lowpass(700.0, 1.2)
        .amp(0.35)
        .flanger(0.3, 2.0)
        .feedback(0.7)
        .mix(0.5);

    let mix = pad.add(bass).soft_clip(1.2);
    play(mix).unwrap();
}
