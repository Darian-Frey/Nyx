//! SVF band-pass sweep — an audio-rate cutoff modulation that the
//! state-variable filter can track without clicks (the biquad would
//! need coefficient smoothing for the same effect).
//!
//! Pink noise fed through a band-pass whose cutoff sweeps 200 Hz → 8 kHz
//! via a 0.5 Hz sine LFO. High Q makes each sweep sound like a resonant
//! formant.
//!
//! Run: cargo run -p nyx-prelude --example svf_sweep --release

use nyx_prelude::*;

fn main() {
    // LFO sweeps cutoff exponentially between 200 Hz and 8 kHz.
    let lfo = osc::sine(0.5).amp(3900.0).offset(4100.0);

    let signal = osc::noise::pink(42)
        .svf_bp(lfo, 8.0)    // narrow resonant band-pass
        .amp(0.4);

    play(signal).unwrap();
}
