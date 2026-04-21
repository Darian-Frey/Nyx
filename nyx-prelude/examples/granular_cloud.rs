//! Granular synthesis — a slow-evolving cloud stretched from a short
//! synthesised pad.
//!
//! A 3-second Cm7 pad (C2 – Eb2 – G2 – Bb2, saw-based, lowpassed) is
//! rendered once into a `Sample`. The granular engine then reads slowly
//! across it with jittered position, pitch, and pan, producing the
//! classic "time-stretched drone" texture. A gentle reverb tail glues
//! the grains into a single, evolving sound.
//!
//! The `position` parameter drifts across the source over the duration
//! of playback so the cloud's harmonic content shifts naturally.
//!
//! Run: cargo run -p nyx-prelude --example granular_cloud --release

use nyx_prelude::*;

fn main() {
    // ── 1. Bake a short pad into a Sample ──────────────────────────
    //
    // Four low saws, lowpassed, mixed and soft-clipped. Three seconds of
    // raw material that the granulator will reassemble into a minutes-
    // long drone via overlapping jittered grains.
    let mut pad = osc::saw(Note::from_midi(36).to_freq()) // C2
        .add(osc::saw(Note::from_midi(39).to_freq())) // Eb2
        .add(osc::saw(Note::from_midi(43).to_freq())) // G2
        .add(osc::saw(Note::from_midi(46).to_freq())) // Bb2
        .lowpass(900.0, 0.8)
        .amp(0.3)
        .soft_clip(1.0);
    let buf = render_to_buffer(&mut pad, 3.0, 44100.0);
    let source = Sample::from_buffer(buf, 44100.0).unwrap();

    // ── 2. Build the granular cloud ────────────────────────────────
    //
    // Wide position jitter + gentle pitch wobble + full stereo spread
    // converts the short source into a continuously-evolving texture.
    let cloud = Granular::new(source)
        .grain_size(0.12) // 120 ms grains — long and smooth
        .density(45.0) // lots of overlap → seamless cloud
        .position(0.5)
        .position_jitter(0.35) // ±35 % of sample length
        .pitch(1.0)
        .pitch_jitter(0.025) // ±2.5 % pitch wobble = gentle chorus
        .pan_spread(1.0) // full stereo field
        .amp(0.25)
        .amp_jitter(0.2)
        .seed(424242);

    // Reverb tail binds the grains into a single drone.
    let out = cloud
        .freeverb()
        .room_size(0.9)
        .damping(0.4)
        .wet(0.35)
        .soft_clip(1.1);

    play(out).unwrap();
}
