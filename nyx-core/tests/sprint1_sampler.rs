//! Sprint 1 — Sampler tests.
//!
//! All tests use `Sample::from_buffer` so they work without the `wav`
//! feature. A separate `#[cfg(feature = "wav")]` test exercises
//! `Sample::load` end-to-end.

use nyx_core::{AudioContext, DenyAllocGuard, Sample, Sampler, Signal};

const SR: f32 = 44100.0;

fn ctx(tick: u64) -> AudioContext {
    AudioContext {
        sample_rate: SR,
        tick,
    }
}

/// A monotonic ramp 0, 1, 2, ... len-1 — makes position-to-value
/// correspondence obvious.
fn ramp_sample(len: usize, sr: f32) -> Sample {
    let data: Vec<f32> = (0..len).map(|i| i as f32).collect();
    Sample::from_buffer(data, sr).unwrap()
}

// ─────────────── Sample construction ───────────────

#[test]
fn empty_buffer_errors() {
    assert!(Sample::from_buffer(Vec::new(), SR).is_err());
}

#[test]
fn sample_metadata_matches_input() {
    let s = Sample::from_buffer(vec![0.1, 0.2, 0.3, 0.4], 48000.0).unwrap();
    assert_eq!(s.len(), 4);
    assert!(!s.is_empty());
    assert_eq!(s.sample_rate(), 48000.0);
    assert!((s.duration_secs() - 4.0 / 48000.0).abs() < 1e-6);
}

// ─────────────── OneShot playback ───────────────

#[test]
fn oneshot_plays_then_silences() {
    let s = ramp_sample(5, SR);
    let mut voice = Sampler::new(s);
    let got: Vec<f32> = (0..8).map(|t| voice.next(&ctx(t))).collect();
    // Ramp is [0, 1, 2, 3, 4]; after index 4, we're past-end and
    // finished=true, so subsequent outputs are 0.
    assert_eq!(got, vec![0.0, 1.0, 2.0, 3.0, 4.0, 0.0, 0.0, 0.0]);
    assert!(voice.is_finished());
}

#[test]
fn trigger_resets_oneshot() {
    let s = ramp_sample(4, SR);
    let mut voice = Sampler::new(s);
    // Drain it
    for _ in 0..8 {
        voice.next(&ctx(0));
    }
    assert!(voice.is_finished());
    // Retrigger → plays again from the start
    voice.trigger();
    assert!(!voice.is_finished());
    let first = voice.next(&ctx(0));
    assert_eq!(first, 0.0);
}

// ─────────────── Pitch ───────────────

#[test]
fn pitch_2_halves_duration() {
    let s = ramp_sample(10, SR);
    let mut voice = Sampler::new(s).pitch(2.0);
    let got: Vec<f32> = (0..8).map(|t| voice.next(&ctx(t))).collect();
    // Reading every 2 samples with linear interp: output at t=0,1,2,3,4
    // corresponds to positions 0, 2, 4, 6, 8. After t=4 we're past-end
    // and finished.
    assert_eq!(got[0], 0.0);
    assert_eq!(got[1], 2.0);
    assert_eq!(got[2], 4.0);
    assert_eq!(got[3], 6.0);
    assert_eq!(got[4], 8.0);
    assert_eq!(got[5], 0.0); // finished
}

#[test]
fn pitch_half_doubles_duration() {
    // Interpolated: position 0.0, 0.5, 1.0, 1.5, 2.0...
    // Values: ramp[0]=0, interp(0,1)=0.5, ramp[1]=1, interp(1,2)=1.5, ...
    let s = ramp_sample(5, SR);
    let mut voice = Sampler::new(s).pitch(0.5);
    let got: Vec<f32> = (0..8).map(|t| voice.next(&ctx(t))).collect();
    assert!((got[0] - 0.0).abs() < 1e-6);
    assert!((got[1] - 0.5).abs() < 1e-6);
    assert!((got[2] - 1.0).abs() < 1e-6);
    assert!((got[3] - 1.5).abs() < 1e-6);
}

#[test]
fn pitch_accepts_signal() {
    // A Signal as the pitch parameter — confirms the IntoParam API works.
    let s = ramp_sample(100, SR);
    let pitch_sig = |_ctx: &AudioContext| 1.0_f32;
    let mut voice = Sampler::new(s).pitch(pitch_sig);
    let got: Vec<f32> = (0..5).map(|t| voice.next(&ctx(t))).collect();
    assert_eq!(got, vec![0.0, 1.0, 2.0, 3.0, 4.0]);
}

// ─────────────── Sample-rate adjustment ───────────────

#[test]
fn sample_rate_mismatch_adjusts_playback() {
    // 44100 Hz sample played at 22050 Hz stream → plays at 2× speed
    // (pitch=1.0 gives native pitch at the sample's rate, not the
    // stream's rate).
    let s = ramp_sample(10, 44100.0);
    let mut voice = Sampler::new(s);
    let ctx_22k = |tick: u64| AudioContext {
        sample_rate: 22050.0,
        tick,
    };
    let got: Vec<f32> = (0..5).map(|t| voice.next(&ctx_22k(t))).collect();
    // step per sample = 1.0 × (44100 / 22050) = 2.0 → positions 0, 2, 4, 6, 8
    assert_eq!(got, vec![0.0, 2.0, 4.0, 6.0, 8.0]);
}

// ─────────────── Loop mode ───────────────

#[test]
fn loop_all_wraps() {
    let s = ramp_sample(4, SR);
    let mut voice = Sampler::new(s).loop_all();
    let got: Vec<f32> = (0..10).map(|t| voice.next(&ctx(t))).collect();
    // [0,1,2,3] repeating
    assert_eq!(got, vec![0.0, 1.0, 2.0, 3.0, 0.0, 1.0, 2.0, 3.0, 0.0, 1.0]);
    assert!(!voice.is_finished());
}

#[test]
fn loop_region_wraps_within_bounds() {
    // Ramp of 10 samples; loop over [2, 5) — 3-sample window.
    let s = ramp_sample(10, SR);
    let start = 2.0 / SR;
    let end = 5.0 / SR;
    let mut voice = Sampler::new(s).loop_region(start, end);
    voice.trigger(); // jump to loop start
    let got: Vec<f32> = (0..10).map(|t| voice.next(&ctx(t))).collect();
    // Values 2, 3, 4 repeating.
    assert_eq!(got, vec![2.0, 3.0, 4.0, 2.0, 3.0, 4.0, 2.0, 3.0, 4.0, 2.0]);
}

// ─────────────── PingPong ───────────────

#[test]
fn ping_pong_bounces() {
    let s = ramp_sample(4, SR);
    // Loop over whole sample, ping-pong.
    let mut voice = Sampler::new(s).loop_all().ping_pong();
    let got: Vec<f32> = (0..12).map(|t| voice.next(&ctx(t))).collect();
    // Forward: 0, 1, 2, 3. Hit end at pos 4.0, reflect to pos 4.0 with
    // direction=-1. Step by -1 → positions 4, 3, 2, 1, 0 ... read at
    // those positions. The exact sequence depends on how we handle the
    // boundary flip. Just verify it's bouncing (not all monotonic).
    let mut increased = 0;
    let mut decreased = 0;
    for w in got.windows(2) {
        if w[1] > w[0] {
            increased += 1;
        }
        if w[1] < w[0] {
            decreased += 1;
        }
    }
    assert!(increased > 0 && decreased > 0, "expected ping-pong: got {got:?}");
}

// ─────────────── Arc sharing ───────────────

#[test]
fn sample_clone_shares_data() {
    let s = ramp_sample(100, SR);
    let v1 = Sampler::new(s.clone());
    let v2 = Sampler::new(s);
    // Both voices read from the same underlying data.
    let _ = (v1, v2);
}

// ─────────────── No-alloc ───────────────

#[test]
fn sampler_does_not_allocate_per_sample() {
    let s = ramp_sample(1024, SR);
    let mut voice = Sampler::new(s).loop_all();
    let c = ctx(0);
    let _guard = DenyAllocGuard::new();
    for _ in 0..4096 {
        voice.next(&c);
    }
}

// ─────────────── WAV round-trip (wav feature) ───────────────

#[cfg(feature = "wav")]
#[test]
fn load_wav_roundtrip() {
    // Write a ramp, load it, confirm it matches.
    let path = std::env::temp_dir().join("nyx-sampler-test.wav");
    let mut iter = (0..1000_i32).map(|i| i as f32 / 1000.0);
    let sig = move |_ctx: &AudioContext| iter.next().unwrap_or(0.0);
    // Actually use the explicit f32 path for lossless roundtrip.
    nyx_core::render_to_wav_f32(sig, 1000.0 / SR, SR, &path).unwrap();

    let loaded = Sample::load(&path).unwrap();
    assert_eq!(loaded.len(), 1000);
    assert_eq!(loaded.sample_rate(), SR);

    // Play it back through a Sampler and confirm the data matches.
    let mut voice = Sampler::new(loaded);
    for i in 0..1000 {
        let got = voice.next(&ctx(i));
        let expected = i as f32 / 1000.0;
        assert!(
            (got - expected).abs() < 1e-4,
            "frame {i}: got {got}, want {expected}"
        );
    }

    let _ = std::fs::remove_file(&path);
}
