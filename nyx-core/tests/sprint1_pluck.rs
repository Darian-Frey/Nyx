//! Sprint 1 — Karplus-Strong pluck tests.

use nyx_core::{pluck, render_to_buffer, AudioContext, DenyAllocGuard, Signal};

const SR: f32 = 44100.0;

fn ctx(tick: u64) -> AudioContext {
    AudioContext {
        sample_rate: SR,
        tick,
    }
}

// ──────────────── Frequency accuracy ────────────────

/// Find the lag in `[min_lag, max_lag]` that maximises autocorrelation.
/// Returns the lag corresponding to the fundamental period.
fn dominant_period(buf: &[f32], min_lag: usize, max_lag: usize) -> usize {
    let mut best_lag = min_lag;
    let mut best_corr = f32::NEG_INFINITY;
    for lag in min_lag..=max_lag.min(buf.len() / 2) {
        let n = buf.len() - lag;
        let corr: f32 = (0..n).map(|i| buf[i] * buf[i + lag]).sum::<f32>() / n as f32;
        if corr > best_corr {
            best_corr = corr;
            best_lag = lag;
        }
    }
    best_lag
}

#[test]
fn pluck_frequency_matches_target() {
    // 440 Hz pluck → period should be sample_rate / freq = 100.23 samples.
    let mut sig = pluck(440.0, 0.99);
    let buf = render_to_buffer(&mut sig, 0.2, SR);
    let period = dominant_period(&buf, 80, 140);
    let expected = (SR / 440.0).round() as usize;
    assert!(
        (period as i32 - expected as i32).abs() <= 2,
        "expected period near {expected} samples for 440 Hz, got {period}"
    );
}

#[test]
fn pluck_low_frequency() {
    // 110 Hz → period = sample_rate / 110 = 400.9 samples.
    let mut sig = pluck(110.0, 0.99);
    let buf = render_to_buffer(&mut sig, 0.5, SR);
    let period = dominant_period(&buf, 380, 430);
    let expected = (SR / 110.0).round() as usize;
    assert!(
        (period as i32 - expected as i32).abs() <= 3,
        "expected period near {expected} samples for 110 Hz, got {period}"
    );
}

// ──────────────── Decay behaviour ────────────────

#[test]
fn pluck_decays_to_silence() {
    // With decay=0.9 the signal should be near silent within a second.
    let mut sig = pluck(440.0, 0.9);
    let buf = render_to_buffer(&mut sig, 1.0, SR);

    let tail = &buf[buf.len() - 4410..]; // last 100 ms
    let rms: f32 = (tail.iter().map(|s| s * s).sum::<f32>() / tail.len() as f32).sqrt();
    assert!(rms < 0.01, "pluck should decay; tail rms = {rms}");
}

#[test]
fn long_decay_sustains_longer_than_short() {
    // Compare energy at 0.3 s between decay=0.9 and decay=0.99.
    let mut short = pluck(440.0, 0.9);
    let mut long = pluck(440.0, 0.99);
    let short_buf = render_to_buffer(&mut short, 0.3, SR);
    let long_buf = render_to_buffer(&mut long, 0.3, SR);

    let window = 2000;
    let short_tail = &short_buf[short_buf.len() - window..];
    let long_tail = &long_buf[long_buf.len() - window..];

    let short_rms: f32 =
        (short_tail.iter().map(|s| s * s).sum::<f32>() / window as f32).sqrt();
    let long_rms: f32 = (long_tail.iter().map(|s| s * s).sum::<f32>() / window as f32).sqrt();

    assert!(
        long_rms > short_rms * 5.0,
        "long decay should sustain longer: long={long_rms} vs short={short_rms}"
    );
}

#[test]
fn zero_decay_is_silent_quickly() {
    // decay=0 wipes the buffer within one period.
    let mut sig = pluck(440.0, 0.0);
    // First period (100 samples ≈ 1 cycle at 440 Hz on 44100 Hz) will
    // still contain the original noise burst.
    // After that, everything should be silent.
    let buf = render_to_buffer(&mut sig, 0.01, SR);
    let period = (SR / 440.0).ceil() as usize + 5;
    let after = &buf[period..];
    let rms: f32 = (after.iter().map(|s| s * s).sum::<f32>() / after.len() as f32).sqrt();
    assert!(
        rms < 0.01,
        "decay=0.0 should kill signal within a period, got rms={rms}"
    );
}

// ──────────────── Output range ────────────────

#[test]
fn output_stays_bounded() {
    let mut sig = pluck(440.0, 0.99);
    let buf = render_to_buffer(&mut sig, 0.5, SR);
    for (i, &s) in buf.iter().enumerate() {
        assert!(
            (-1.5..=1.5).contains(&s),
            "sample {i} out of bounds: {s}"
        );
    }
}

// ──────────────── Reproducibility ────────────────

#[test]
fn same_params_same_output() {
    let mut a = pluck(330.0, 0.98);
    let mut b = pluck(330.0, 0.98);
    let buf_a = render_to_buffer(&mut a, 0.05, SR);
    let buf_b = render_to_buffer(&mut b, 0.05, SR);
    for (i, (x, y)) in buf_a.iter().zip(buf_b.iter()).enumerate() {
        assert!(
            (x - y).abs() < 1e-6,
            "reproducibility broken at sample {i}: {x} vs {y}"
        );
    }
}

#[test]
fn different_freq_different_burst() {
    let mut a = pluck(220.0, 0.99);
    let mut b = pluck(440.0, 0.99);
    let buf_a = render_to_buffer(&mut a, 0.001, SR);
    let buf_b = render_to_buffer(&mut b, 0.001, SR);
    // At least one sample should differ noticeably — different seeds
    // produce different noise bursts.
    let max_diff = buf_a
        .iter()
        .zip(buf_b.iter())
        .map(|(x, y)| (x - y).abs())
        .fold(0.0_f32, f32::max);
    assert!(max_diff > 0.1, "bursts should differ, max_diff={max_diff}");
}

// ──────────────── Edge cases ────────────────

#[test]
fn very_low_freq_clamped() {
    // A 5 Hz pluck should be clamped to ≥ 20 Hz internally; still
    // produces some output without panicking.
    let mut sig = pluck(5.0, 0.99);
    let _ = sig.next(&ctx(0));
}

// ──────────────── No-alloc ────────────────

#[test]
fn pluck_does_not_allocate_per_sample() {
    let mut sig = pluck(440.0, 0.99);
    let c = ctx(0);
    // Prime the init path (fills the buffer with noise once).
    sig.next(&c);
    let _guard = DenyAllocGuard::new();
    for _ in 0..4096 {
        sig.next(&c);
    }
}
