//! Sonic character — oscillator-drift signal tests.

#[global_allocator]
static ALLOC: nyx_core::GuardedAllocator = nyx_core::GuardedAllocator;

use nyx_core::{AudioContext, DenyAllocGuard, Signal, SignalExt, drift, render_to_buffer};

const SR: f32 = 44100.0;

#[test]
fn drift_output_centred_near_one() {
    // The multiplier for ±8 cents is 2^(±8/1200) ≈ [0.9954, 1.0046],
    // so every sample must sit inside a small window around 1.0.
    // Pad the bound a bit for the smoother's transients.
    let mut sig = drift(8.0, 0.5).seed(12345);
    let buf = render_to_buffer(&mut sig, 1.0, SR);
    for &s in &buf {
        assert!(
            s.is_finite() && (0.990..=1.010).contains(&s),
            "drift sample outside ±10 cents window: {s}"
        );
    }
}

#[test]
fn drift_moves_over_time() {
    // Use a higher-than-musical rate so several picks land in a 1 s
    // window and the mean demonstrably wanders. Compare first- and
    // second-half means — they shouldn't both be pinned.
    let mut sig = drift(10.0, 4.0).seed(42);
    let buf = render_to_buffer(&mut sig, 1.0, SR);
    let half = buf.len() / 2;
    let a_mean = buf[..half].iter().sum::<f32>() / half as f32;
    let b_mean = buf[half..].iter().sum::<f32>() / (buf.len() - half) as f32;
    assert!(
        (a_mean - b_mean).abs() > 1e-4,
        "drift did not move: first-half mean {a_mean}, second-half mean {b_mean}"
    );
}

#[test]
fn drift_zero_amount_is_constant_one() {
    // amount_cents = 0 must output exactly 1.0 every sample — the
    // product `freq · drift` should then equal `freq`.
    let mut sig = drift(0.0, 0.5).seed(1);
    for tick in 0..2048 {
        let s = sig.next(&AudioContext {
            sample_rate: SR,
            tick,
        });
        assert!(
            (s - 1.0).abs() < 1e-6,
            "zero-amount drift should be 1.0: {s}"
        );
    }
}

#[test]
fn drift_seed_determinism() {
    let mut a = drift(5.0, 0.5).seed(7);
    let mut b = drift(5.0, 0.5).seed(7);
    let buf_a = render_to_buffer(&mut a, 0.2, SR);
    let buf_b = render_to_buffer(&mut b, 0.2, SR);
    assert_eq!(buf_a, buf_b);
}

#[test]
fn drift_different_seeds_diverge() {
    // Use well-separated seeds — xorshift32 mixes poorly for adjacent
    // small seeds, which would produce nearly-identical first picks.
    let mut a = drift(5.0, 0.5).seed(0x1234_5678);
    let mut b = drift(5.0, 0.5).seed(0xABCD_EF01);
    let buf_a = render_to_buffer(&mut a, 1.0, SR);
    let buf_b = render_to_buffer(&mut b, 1.0, SR);
    let diff_rms = (buf_a
        .iter()
        .zip(buf_b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f32>()
        / buf_a.len() as f32)
        .sqrt();
    assert!(
        diff_rms > 1e-5,
        "different seeds should produce different drift, rms={diff_rms}"
    );
}

#[test]
fn drift_composes_with_amp_for_frequency() {
    // The intended usage: `drift(...).amp(base_freq)` yields a
    // frequency signal near `base_freq`. Verify the output mean sits
    // close to base_freq and varies within ±amount cents' multiplier.
    let mut freq = drift(10.0, 0.4).seed(99).amp(440.0);
    let buf = render_to_buffer(&mut freq, 1.0, SR);
    let mean = buf.iter().sum::<f32>() / buf.len() as f32;
    assert!((mean - 440.0).abs() < 5.0, "mean {mean} should be near 440");
    let min = buf.iter().copied().fold(f32::INFINITY, f32::min);
    let max = buf.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    // ±10 cents → 440 · [0.9942, 1.0058] → [437.45, 442.55]. Allow a
    // little slack for any transient before the target is first set.
    assert!(
        min > 437.0 && max < 443.0,
        "freq swept outside expected range: min={min} max={max}"
    );
}

#[test]
fn drift_no_alloc() {
    let mut sig = drift(5.0, 0.5).seed(123);
    let _guard = DenyAllocGuard::new();
    for tick in 0..1024 {
        let _ = sig.next(&AudioContext {
            sample_rate: SR,
            tick,
        });
    }
}
