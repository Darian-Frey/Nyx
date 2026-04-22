//! Sonic character — vinyl crackle + hiss tests.

#[global_allocator]
static ALLOC: nyx_core::GuardedAllocator = nyx_core::GuardedAllocator;

use nyx_core::{AudioContext, DenyAllocGuard, Signal, render_to_buffer, vinyl};

const SR: f32 = 44100.0;

fn rms(buf: &[f32]) -> f32 {
    (buf.iter().map(|s| s * s).sum::<f32>() / buf.len() as f32).sqrt()
}

// ─── Crackle ──────────────────────────────────────────────────────────

#[test]
fn crackle_zero_intensity_is_silent() {
    // intensity=0 must never fire and never ring — output is exactly 0.
    let mut sig = vinyl::crackle(0.0);
    for tick in 0..4096 {
        let s = sig.next(&AudioContext {
            sample_rate: SR,
            tick,
        });
        assert_eq!(
            s, 0.0,
            "crackle(0.0) should be silent, got {s} at tick {tick}"
        );
    }
}

#[test]
fn crackle_produces_output_at_moderate_intensity() {
    // At intensity=0.5 the fire rate is ~2.2/s and each click's
    // resonator is `(1 − r)`-scaled so individual hits land ≈ 0.04
    // peak. Over 2 s the total RMS lands in the 1e-4 range — crackle
    // is designed as subtle ambience, not a headline effect. A 1e-5
    // floor catches "never fired" / "resonator broken" regressions
    // without being sensitive to the impulse-gain constants.
    let mut sig = vinyl::crackle(0.5).seed(42);
    let buf = render_to_buffer(&mut sig, 2.0, SR);
    let r = rms(&buf);
    assert!(
        r > 1e-5,
        "crackle(0.5) should fire at least occasionally, rms={r}"
    );
    for &s in &buf {
        assert!(
            s.is_finite() && s.abs() <= 2.0,
            "crackle sample outside ±2.0: {s}"
        );
    }
}

#[test]
fn crackle_intensity_scales_activity() {
    // Higher intensity → more non-zero samples (more clicks fired).
    let mut quiet = vinyl::crackle(0.1).seed(1);
    let mut busy = vinyl::crackle(0.9).seed(1);
    let a = render_to_buffer(&mut quiet, 2.0, SR);
    let b = render_to_buffer(&mut busy, 2.0, SR);
    let a_rms = rms(&a);
    let b_rms = rms(&b);
    assert!(
        b_rms > a_rms * 2.0,
        "higher intensity should be substantially louder on average: quiet={a_rms} busy={b_rms}"
    );
}

#[test]
fn crackle_seed_determinism() {
    let mut a = vinyl::crackle(0.5).seed(777);
    let mut b = vinyl::crackle(0.5).seed(777);
    let buf_a = render_to_buffer(&mut a, 0.5, SR);
    let buf_b = render_to_buffer(&mut b, 0.5, SR);
    assert_eq!(buf_a, buf_b);
}

#[test]
fn crackle_no_alloc() {
    let mut sig = vinyl::crackle(0.5).seed(1);
    let _guard = DenyAllocGuard::new();
    for tick in 0..2048 {
        let _ = sig.next(&AudioContext {
            sample_rate: SR,
            tick,
        });
    }
}

// ─── Hiss ─────────────────────────────────────────────────────────────

#[test]
fn hiss_level_roughly_matches_db() {
    // Pink noise RMS ≈ its peak scale (see Pink::PINK_KELLETT_SCALE).
    // For −60 dB request, scale = 0.001 and observed RMS should sit
    // somewhere in (5e-5, 5e-3) — a wide band that catches a scaling
    // regression without being sensitive to pink-noise RMS constants.
    let mut sig = vinyl::hiss(-60.0);
    let buf = render_to_buffer(&mut sig, 1.0, SR);
    let r = rms(&buf);
    assert!(
        (5e-5..5e-3).contains(&r),
        "hiss(-60 dB) rms out of expected band: {r}"
    );
}

#[test]
fn hiss_higher_level_is_louder() {
    // A 20 dB increase should produce roughly 10× louder RMS.
    let mut soft = vinyl::hiss(-60.0);
    let mut loud = vinyl::hiss(-40.0);
    let a = render_to_buffer(&mut soft, 1.0, SR);
    let b = render_to_buffer(&mut loud, 1.0, SR);
    let ratio = rms(&b) / rms(&a);
    assert!(
        (5.0..20.0).contains(&ratio),
        "hiss +20 dB should be ~10× louder, got ratio={ratio}"
    );
}

#[test]
fn hiss_no_alloc() {
    let mut sig = vinyl::hiss(-55.0);
    let _guard = DenyAllocGuard::new();
    for tick in 0..2048 {
        let _ = sig.next(&AudioContext {
            sample_rate: SR,
            tick,
        });
    }
}
