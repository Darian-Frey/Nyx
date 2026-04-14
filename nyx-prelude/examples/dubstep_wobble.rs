//! Dubstep wobble: LFO-modulated filter cutoff, BPM-synced at 140.
//!
//! Run: cargo run -p nyx-prelude --example dubstep_wobble --release

use nyx_prelude::*;

fn main() {
    // LFO at 2 Hz (≈ quarter notes at 140 BPM) sweeping 200–1700 Hz.
    let lfo = osc::sine(2.0).amp(750.0).offset(950.0);

    // Fat bass: detuned saws → resonant lowpass → soft clip for grit.
    let bass = osc::saw(55.0)
        .add(osc::saw(55.5).amp(0.7))
        .lowpass(lfo, 4.0)
        .soft_clip(1.8)
        .amp(0.3);

    play(bass).unwrap();
}
