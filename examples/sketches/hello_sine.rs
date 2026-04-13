// A minimal Nyx sketch: a sine wave at 440 Hz.
//
// Run with: cargo run -p nyx-cli -- examples/sketches/hello_sine.rs
// Edit this file while it's playing to hear changes instantly!

use nyx_core::osc;
use nyx_core::SignalExt;
use nyx_core::Signal;

#[unsafe(no_mangle)]
pub fn nyx_sketch() -> Box<dyn Signal> {
    osc::sine(440.0).amp(0.3).boxed()
}
