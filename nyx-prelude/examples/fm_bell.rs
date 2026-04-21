//! FM bell — the classic DX7 bell sound.
//!
//! Two-operator FM with a 1:2 modulator ratio, decaying modulation
//! index (3.0 → 0.0 over 2 seconds), gated by an ADSR. Each trigger
//! lands on a different note from a C minor pentatonic scale.
//!
//! Demonstrates: `fm_op`, automation-driven modulation index via an
//! ADSR, carrier frequency updates through `OscParam` (RT-safe), and
//! scale-aware random melody generation.
//!
//! Run: cargo run -p nyx-prelude --example fm_bell --release

use nyx_prelude::*;

fn main() {
    // Atomic params for the carrier and modulator frequencies.
    // Each trigger writes a new pitch; the FM operator reads it
    // through an OscSignal with one-pole smoothing.
    let carrier_param = OscParam::new(Note::C4.to_freq());
    let mod_param = OscParam::new(Note::C4.to_freq() * 2.0);
    let index_param = OscParam::new(0.0);

    // Build the two-operator FM graph.
    let modulator = osc::sine(mod_param.signal(1.0));
    let op = fm_op(
        carrier_param.signal(1.0),
        modulator,
        index_param.signal(1.0),
    );

    // Note generator state, running on the audio thread alongside the
    // envelope that shapes both amplitude and modulation index.
    let mut clk = clock::clock(110.0);
    let scale = Scale::pentatonic_minor("C");
    let mut rng = seeded(0x8E11);
    let low = Note::from_midi(60);
    let high = Note::from_midi(84);
    let mut last_beat = -1_i32;
    let mut idx_env = envelope::adsr(0.001, 1.2, 0.0, 0.05);
    let mut amp_env = envelope::adsr(0.001, 0.05, 0.6, 1.8);

    // Carrier/mod writers live on the audio thread (read-only from
    // outside), so setting them here is fine.
    let carrier_writer = carrier_param.writer();
    let mod_writer = mod_param.writer();
    let index_writer = index_param.writer();

    let signal = op.amp(move |ctx: &nyx_prelude::AudioContext| {
        // Advance clock, trigger on each new beat.
        let state = clk.tick(ctx);
        let beat = state.beat as i32;
        if beat != last_beat {
            last_beat = beat;
            let note = rng.next_note_in(&scale, low, high);
            carrier_writer.set(note.to_freq());
            mod_writer.set(note.to_freq() * 2.0);
            idx_env.trigger();
            amp_env.trigger();
        }
        // The amp closure returns the amplitude for this sample:
        // AM(ctx) = amp_env(ctx), and separately updates the index
        // atomic so the FM operator reads a fresh value next sample.
        index_writer.set(idx_env.next(ctx) * 3.5);
        amp_env.next(ctx) * 0.4
    });

    play(signal).unwrap();
}
