//! Render the Tron: Legacy-style cue to a WAV file.
//!
//! The signal is defined in [`nyx_prelude::demos::tron`] so the browser
//! demo and the offline renderer share a single musical source of truth.
//! See that module for the full structural breakdown.
//!
//! Run: cargo run -p nyx-prelude --example tron_wav --release

use nyx_prelude::*;

fn main() {
    const SAMPLE_RATE: f32 = 44100.0;
    const DURATION_SECS: f32 = 90.0;

    println!(
        "nyx: rendering Tron-style cue — 120 BPM, D minor, 45 bars ({:.1} s)...",
        DURATION_SECS
    );
    let out = "target/tron.wav";
    render_to_wav(demos::tron(), DURATION_SECS, SAMPLE_RATE, out).unwrap();
    println!(
        "nyx: wrote {} ({:.1} s, {} Hz, 16-bit mono)",
        out, DURATION_SECS, SAMPLE_RATE as i32
    );
}
