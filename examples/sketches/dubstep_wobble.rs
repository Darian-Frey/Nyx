// Dubstep wobble: LFO on filter cutoff, BPM-synced.
//
// Run with: cargo run -p nyx-cli -- examples/sketches/dubstep_wobble.rs

use nyx_core::osc;
use nyx_core::{Signal, SignalExt};
use nyx_core::filter::FilterExt;
use nyx_core::AudioContext;

#[no_mangle]
pub fn nyx_sketch() -> Box<dyn Signal> {
    // Saw bass at ~55 Hz (A1)
    let bass = osc::saw(55.0);

    // LFO modulating filter cutoff: 200–2000 Hz at ~4 Hz wobble
    let lfo = osc::sine(4.0).amp(900.0).offset(1100.0);

    // Bass through resonant lowpass with wobbling cutoff
    bass.lowpass(lfo, 4.0).amp(0.4).boxed()
}
