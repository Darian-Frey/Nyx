//! Stereo panning + Haas widener — demonstrates the Sprint 2 stereo
//! refactor. The engine now writes real left/right channels instead
//! of duplicating mono.
//!
//! Two layers:
//!   - A saw bass panned by a slow LFO, sweeping L↔R over 4 seconds
//!   - A pluck layer widened with a 15 ms Haas delay for width
//!
//! Run: cargo run -p nyx-prelude --example stereo_sweep --release
//!
//! Use stereo headphones or speakers to hear the effect.

use nyx_prelude::*;

fn main() {
    // 0.25 Hz LFO sweeps pan position between -1 (hard left) and +1.
    let pan_lfo = osc::sine(0.25);

    // Saw bass, panned by the LFO.
    let bass = osc::saw(55.0).lowpass(800.0, 1.5).amp(0.35).pan(pan_lfo);

    // Pluck chord stacked with Haas widening for width.
    let plucks = pluck(Note::C4.to_freq(), 0.995)
        .add(pluck(Note::from_midi(63).to_freq(), 0.995)) // Eb4
        .add(pluck(Note::G4.to_freq(), 0.995))
        .amp(0.15)
        .haas(18.0);

    play(bass.add(plucks)).unwrap();
}
