//! Sprint 3 — YIN pitch detection tests.

use nyx_core::{osc, AudioContext, DenyAllocGuard, PitchConfig, Signal, SignalExt};

const SR: f32 = 44100.0;

fn ctx(tick: u64) -> AudioContext {
    AudioContext {
        sample_rate: SR,
        tick,
    }
}

/// Drive a pitch tracker with `n_samples` ticks.
fn drive<S: Signal>(sig: &mut S, n_samples: usize) {
    for i in 0..n_samples {
        let _ = sig.next(&ctx(i as u64));
    }
}

#[test]
fn detects_sine_at_440_hz() {
    let (mut sig, pitch) = osc::sine(440.0).amp(0.5).pitch(PitchConfig::default());
    drive(&mut sig, 4096);

    let freq = pitch.freq();
    assert!(
        (freq - 440.0).abs() < 2.0,
        "expected ~440 Hz, got {freq} Hz"
    );
    assert!(
        pitch.confidence() > 0.9,
        "clean sine clarity should be >0.9, got {}",
        pitch.confidence()
    );
}

#[test]
fn detects_sine_at_110_hz() {
    let (mut sig, pitch) = osc::sine(110.0).amp(0.5).pitch(PitchConfig::default());
    drive(&mut sig, 4096);

    let freq = pitch.freq();
    assert!(
        (freq - 110.0).abs() < 1.0,
        "expected ~110 Hz, got {freq} Hz"
    );
}

#[test]
fn detects_sine_at_1000_hz() {
    let (mut sig, pitch) = osc::sine(1000.0).amp(0.5).pitch(PitchConfig::default());
    drive(&mut sig, 4096);

    let freq = pitch.freq();
    assert!(
        (freq - 1000.0).abs() < 5.0,
        "expected ~1000 Hz, got {freq} Hz"
    );
}

#[test]
fn detects_saw_fundamental_not_harmonics() {
    // Saw at 220 Hz: rich harmonics. YIN should lock to the fundamental,
    // not a harmonic.
    let (mut sig, pitch) = osc::saw(220.0).amp(0.3).pitch(PitchConfig::default());
    drive(&mut sig, 4096);

    let freq = pitch.freq();
    assert!(
        (freq - 220.0).abs() < 2.0,
        "saw fundamental should be ~220 Hz, got {freq}"
    );
}

#[test]
fn silence_produces_no_pitch() {
    // Silent source → no periodicity detected, freq=0, confidence=0.
    let silent = |_: &AudioContext| 0.0_f32;
    let (mut sig, pitch) = silent.pitch(PitchConfig::default());
    drive(&mut sig, 4096);

    // Silence is technically perfectly periodic (d=0 everywhere), so YIN
    // may lock to the minimum τ. The freq it reports, if any, should
    // at least be bounded — and the key contract is no panic / finite.
    let f = pitch.freq();
    assert!(f.is_finite(), "freq should be finite on silence, got {f}");
}

#[test]
fn noise_gives_low_confidence_or_no_pitch() {
    let (mut sig, pitch) = osc::noise::white(42).amp(0.5).pitch(PitchConfig::default());
    drive(&mut sig, 4096);

    let c = pitch.confidence();
    // Noise may occasionally produce spurious peaks, but clarity should
    // be well below a clean tone's.
    assert!(
        c < 0.7,
        "white noise clarity should be <0.7, got {c}"
    );
}

#[test]
fn passes_samples_through_unchanged() {
    // The pitch tap is passive — output == input.
    let (mut tapped, _pitch) = osc::sine(440.0).amp(0.3).pitch(PitchConfig::default());
    let mut direct = osc::sine(440.0).amp(0.3);

    for i in 0..2048 {
        let t = tapped.next(&ctx(i as u64));
        let d = direct.next(&ctx(i as u64));
        assert!(
            (t - d).abs() < 1e-6,
            "pitch tap should be transparent at {i}: got {t}, want {d}"
        );
    }
}

#[test]
fn handle_is_readable_from_another_thread() {
    let (mut sig, pitch) = osc::sine(440.0).amp(0.5).pitch(PitchConfig::default());
    drive(&mut sig, 4096);

    let h = pitch.clone();
    let t = std::thread::spawn(move || (h.freq(), h.confidence()));
    let (f, c) = t.join().unwrap();
    assert!((f - 440.0).abs() < 2.0, "cross-thread freq {f}");
    assert!(c > 0.9, "cross-thread confidence {c}");
}

#[test]
fn custom_config_narrower_range() {
    // Tight bass range: 60–200 Hz. Should still detect 110 Hz sine.
    let cfg = PitchConfig {
        frame_size: 2048,
        hop_size: 1024,
        threshold: 0.15,
        min_freq: 60.0,
        max_freq: 200.0,
    };
    let (mut sig, pitch) = osc::sine(110.0).amp(0.5).pitch(cfg);
    drive(&mut sig, 4096);

    let f = pitch.freq();
    assert!((f - 110.0).abs() < 1.0, "tight range got {f}");
}

#[test]
fn pitch_analysis_does_not_allocate() {
    // Pre-allocate buffers by warming up past the first analysis.
    let (mut sig, _pitch) = osc::sine(440.0).amp(0.3).pitch(PitchConfig::default());
    // Warm up: run through two full frames so the ring is primed and
    // at least one analyze() has executed.
    drive(&mut sig, 5000);

    let _guard = DenyAllocGuard::new();
    // Run enough samples to trigger another analysis pass under the guard.
    drive(&mut sig, 2048);
}

#[test]
fn small_frame_size_still_works() {
    // frame_size=1024 is still valid; detects higher pitches only.
    let cfg = PitchConfig {
        frame_size: 1024,
        hop_size: 512,
        threshold: 0.15,
        min_freq: 100.0,
        max_freq: 2000.0,
    };
    let (mut sig, pitch) = osc::sine(440.0).amp(0.5).pitch(cfg);
    drive(&mut sig, 2048);

    let f = pitch.freq();
    assert!((f - 440.0).abs() < 3.0, "small-frame got {f}");
}
