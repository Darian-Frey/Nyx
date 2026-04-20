//! Lo-fi: bitcrush + downsample for that 80s sampler grit.
//!
//! A filtered saw with a slow wobble, then crushed to 6-bit depth and
//! downsampled to quarter rate (≈11 kHz). Sounds like it was recorded
//! to a cassette and played back through a phone.
//!
//! Run: cargo run -p nyx-prelude --example lofi --release

use nyx_prelude::*;

fn main() {
    let lfo = osc::sine(0.3).amp(500.0).offset(900.0);
    let signal = osc::saw(110.0)
        .lowpass(lfo, 0.707)
        .crush(6, 0.25)  // 6-bit, quarter-rate
        .amp(0.3);

    play(signal).unwrap();
}
