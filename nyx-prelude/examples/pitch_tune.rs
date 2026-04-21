//! Real-time pitch detection — YIN locked onto a slowly-sweeping sine.
//!
//! The audio source is a sine oscillator whose frequency drifts across
//! two octaves every 20 seconds. A `PitchTracker` taps it and publishes
//! the detected fundamental to a `PitchHandle`. The main thread polls
//! the handle ten times per second and prints both the true and
//! detected frequency (and the YIN clarity score) so you can see the
//! tracker locking on.
//!
//! Run: cargo run -p nyx-prelude --example pitch_tune --release
//!
//! Press Enter to stop.

use nyx_prelude::*;
use std::io::{self, Write};
use std::thread;
use std::time::Duration;

fn main() {
    // Sweep frequency: 110 Hz → 880 Hz → 110 Hz, 20-second cycle.
    //   t is seconds-from-start (Automation seeds `t` automatically).
    //   Use an absolute-value triangle so the sweep is monotonic up then down.
    let sweep = automation::automation(|t| {
        let cycle = (t % 20.0) / 20.0; // 0..1
        let tri = if cycle < 0.5 {
            cycle * 2.0 // 0..1
        } else {
            2.0 - cycle * 2.0 // 1..0
        };
        // Exponential sweep across 3 octaves sounds more musical than linear.
        110.0 * 2.0_f32.powf(tri * 3.0)
    });

    // Oscillator with the modulated frequency.
    let src = osc::sine(sweep).amp(0.3);

    // Tap it with a pitch tracker.
    let (tapped, pitch) = src.pitch(PitchConfig {
        frame_size: 2048,
        hop_size: 512,
        threshold: 0.12,
        min_freq: 80.0,
        max_freq: 2000.0,
    });

    let _engine = play_async(tapped).expect("audio engine failed to start");
    println!("nyx pitch tracker — sine sweeping 110–880 Hz");
    println!("press Enter to stop\n");
    println!("{:>7}   {:>7}   {:>5}", "true", "detected", "clarity");
    println!("{:>7}   {:>7}   {:>5}", "-------", "--------", "-----");

    // Poll and print on a background thread.
    let polling = thread::spawn(move || {
        let start = std::time::Instant::now();
        loop {
            let elapsed = start.elapsed().as_secs_f32();
            let cycle = (elapsed % 20.0) / 20.0;
            let tri = if cycle < 0.5 { cycle * 2.0 } else { 2.0 - cycle * 2.0 };
            let true_f = 110.0 * 2.0_f32.powf(tri * 3.0);
            let (detected, clarity) = pitch.read();
            let _ = writeln!(
                io::stdout(),
                "{true_f:>7.1}   {detected:>7.1}   {clarity:>5.2}"
            );
            thread::sleep(Duration::from_millis(100));
        }
    });

    let mut buf = String::new();
    let _ = io::stdin().read_line(&mut buf);
    drop(polling); // detached; audio engine stops when `_engine` drops
}
