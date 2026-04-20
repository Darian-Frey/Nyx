//! WAV export — render a 10-second sketch to disk.
//!
//! Writes `track.wav` in the current directory. 16-bit mono at 44.1 kHz.
//!
//! Run: cargo run -p nyx-prelude --example wav_export --release

use nyx_prelude::*;

fn main() {
    let lfo = osc::sine(0.5).amp(400.0).offset(800.0);
    let signal = osc::saw(110.0)
        .lowpass(lfo, 0.707)
        .amp(0.3);

    render_to_wav(signal, 10.0, 44100.0, "track.wav").unwrap();
    println!("wrote track.wav (10s, 44.1 kHz, 16-bit mono)");
}
