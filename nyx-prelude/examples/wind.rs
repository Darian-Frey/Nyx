//! Wind: pink noise shaped by a slow random LFO on gain, filtered to soften.
//!
//! Run: cargo run -p nyx-prelude --example wind --release

use nyx_prelude::*;

fn main() {
    // Slow swell: sine LFO at 0.15 Hz modulates gain between 0.1 and 0.7.
    let swell = osc::sine(0.15).amp(0.3).offset(0.4);

    // Pink noise → lowpass (warmth) → gain swell.
    let wind = osc::noise::pink(7).lowpass(1800.0, 0.5).amp(swell);

    play(wind).unwrap();
}
