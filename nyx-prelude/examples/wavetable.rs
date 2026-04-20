//! Wavetable: user-drawn waveforms + interpolation.
//!
//! Two wavetable voices mixed together:
//!   - A custom "supersaw-lite" (sum of three detuned saws in one table)
//!   - A pure sine sub an octave lower
//!
//! Both share the same pair of Wavetable Arcs across their voices —
//! building fat polyphonic sounds costs essentially nothing after the
//! initial table allocation.
//!
//! Run: cargo run -p nyx-prelude --example wavetable --release

use nyx_prelude::*;

fn main() {
    // A "supersaw-lite" wavetable: three detuned saws summed in one table.
    let supersaw = Wavetable::from_fn(4096, |t| {
        let saw = |phase: f32| 2.0 * phase.fract() - 1.0;
        let a = saw(t);
        let b = saw(t * 1.003);
        let c = saw(t * 0.997);
        (a + b + c) * 0.33
    });

    let sub = Wavetable::sine(2048);

    // Build a wobbly filter cutoff
    let lfo = osc::sine(0.25).amp(1500.0).offset(2000.0);

    // Root note at A2 (110 Hz), sub one octave below.
    let lead = supersaw.freq(110.0).svf_lp(lfo, 2.5).amp(0.35);
    let sub_voice = sub.freq(55.0).amp(0.25);

    let mix = lead.add(sub_voice).soft_clip(1.2);

    play(mix).unwrap();
}
