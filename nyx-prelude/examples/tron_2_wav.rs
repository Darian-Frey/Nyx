//! Render the darker F-Phrygian Tron-style cue to a WAV file.
//!
//! See [`nyx_prelude::demos::tron_2`] for the musical breakdown.
//!
//! Run: cargo run -p nyx-prelude --example tron_2_wav --release

use nyx_prelude::*;

fn main() {
    const SAMPLE_RATE: f32 = 44100.0;
    const DURATION_SECS: f32 = 88.9;

    println!(
        "nyx: rendering Tron_2 — 108 BPM, F Phrygian, 40 bars ({:.1} s)...",
        DURATION_SECS
    );
    let out = "target/tron_2.wav";
    render_to_wav(demos::tron_2(), DURATION_SECS, SAMPLE_RATE, out).unwrap();
    println!(
        "nyx: wrote {} ({:.1} s, {} Hz, 16-bit mono)",
        out, DURATION_SECS, SAMPLE_RATE as i32
    );
}
