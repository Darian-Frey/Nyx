//! Sampler: render a synthesised kick to a buffer, play it back at
//! different pitches on a beat grid.
//!
//! This example demonstrates the full lifecycle: synthesize audio,
//! capture it into a `Sample::from_buffer`, then trigger playback
//! voices on clock beats with shifting pitch.
//!
//! Run: cargo run -p nyx-prelude --example sampler --release

use nyx_prelude::*;

fn main() {
    // 1. Render a one-shot kick to an in-memory buffer.
    let mut kick_source = inst::kick();
    kick_source.trigger();
    let kick_buf = render_to_buffer(&mut kick_source, 0.4, 44100.0);
    let kick = Sample::from_buffer(kick_buf, 44100.0).unwrap();

    // 2. Build one sampler voice per beat slot.
    let mut voices: Vec<Sampler<ConstSignal>> =
        (0..4).map(|_| Sampler::new(kick.clone())).collect();

    // Pitch per step (semitones relative to native): 0, +7, 0, +5
    let pitches = [1.0_f32, 1.5, 1.0, 1.335];

    // 3. Beat-grid trigger using the clock.
    let mut clk = clock::clock(120.0);
    let mut last_beat = -1_i32;

    let signal = move |ctx: &AudioContext| {
        let state = clk.tick(ctx);
        let beat = state.beat as i32;
        if beat != last_beat {
            last_beat = beat;
            let slot = (beat.rem_euclid(4)) as usize;
            // Rebuild the voice with the new pitch — Sampler is cheap to
            // re-create since data is Arc-shared.
            voices[slot] = Sampler::new(kick.clone()).pitch(pitches[slot]);
            voices[slot].trigger();
        }
        // Mix all voices
        voices.iter_mut().map(|v| v.next(ctx)).sum::<f32>() * 0.7
    };

    play(signal).unwrap();
}
