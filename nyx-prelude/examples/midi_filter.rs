//! MIDI CC → filter cutoff. CC1 (mod wheel) sweeps the cutoff from 100 Hz
//! to 8 kHz (exponential). Saw bass runs continuously underneath.
//!
//! Requires the `midi` feature:
//!   cargo run -p nyx-prelude --example midi_filter --features midi --release

use nyx_prelude::*;

#[cfg(feature = "midi")]
fn main() {
    let cc_map = CcMap::new();
    let writer = cc_map.writer();
    writer.set(1, 64); // initial mid-sweep value

    let (mut rx, _conn) = match midi::open_midi_input() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("nyx: {e} — plug in a MIDI controller and try again");
            return;
        }
    };

    // Smoothed CC → exponential cutoff: 100 Hz at 0, 8000 Hz at 1.
    let mut cc = cc_map.signal(1, 5.0);
    let cutoff = move |ctx: &AudioContext| {
        let x = cc.next(ctx);
        100.0 * 80.0_f32.powf(x)
    };

    let voice = osc::saw(55.0)
        .add(osc::saw(55.3).amp(0.5))
        .lowpass(cutoff, 2.5)
        .amp(0.3);

    let _engine = play_async(voice).unwrap();

    println!("nyx: listening for MIDI CC1 — move your mod wheel. Ctrl+C to quit.");
    loop {
        for event in rx.drain() {
            if let MidiEvent::ControlChange { cc, value, .. } = event {
                writer.set(cc, value);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[cfg(not(feature = "midi"))]
fn main() {
    eprintln!("This example requires the `midi` feature.");
    eprintln!("Run: cargo run -p nyx-prelude --example midi_filter --features midi --release");
}
