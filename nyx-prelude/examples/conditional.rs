//! Probability & conditional sequencing — TidalCycles-style modifiers.
//!
//! A kick on every beat with 80% probability, hi-hats that reverse
//! every 4 bars. Demonstrates `.prob()` and `.every()` on top of the
//! existing `Sequence` API.
//!
//! Run: cargo run -p nyx-prelude --example conditional --release

use nyx_prelude::*;

fn main() {
    let mut clk = clock::clock(128.0);
    let mut kick = inst::kick();
    let mut hat = inst::hihat(false);

    // 4-on-the-floor with 20% degradation
    let kick_pat = Pattern::new(&[true, true, true, true]);
    let mut kick_seq = Sequence::new(kick_pat, 1.0).degrade(0.2).seed(42);

    // Sixteenth-note hats that reverse their pattern every 4 bars —
    // mostly even pulses (14/16), occasional rests.
    let hat_pat = Euclid::generate(14, 16);
    let mut hat_seq = Sequence::new(hat_pat, 0.25)
        .every(4, |p| p.reverse())
        .seed(99);

    let signal = move |ctx: &AudioContext| {
        let state = clk.tick(ctx);
        let k = kick_seq.tick(&state);
        let h = hat_seq.tick(&state);
        if k.triggered && k.value {
            kick.trigger();
        }
        if h.triggered && h.value {
            hat.trigger();
        }
        (kick.next(ctx) + hat.next(ctx) * 0.5) * 0.7
    };

    play(signal).unwrap();
}
