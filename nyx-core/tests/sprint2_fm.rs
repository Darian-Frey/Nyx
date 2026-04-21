//! Sprint 2 — FM operator tests.

use nyx_core::{AudioContext, DenyAllocGuard, Signal, fm_op, osc, render_to_buffer};

const SR: f32 = 44100.0;

fn ctx(tick: u64) -> AudioContext {
    AudioContext {
        sample_rate: SR,
        tick,
    }
}

fn rms(buf: &[f32]) -> f32 {
    (buf.iter().map(|s| s * s).sum::<f32>() / buf.len() as f32).sqrt()
}

/// Count sign changes — a rough proxy for spectral complexity.
fn zero_crossings(buf: &[f32]) -> usize {
    buf.windows(2)
        .filter(|w| w[0].signum() != w[1].signum())
        .count()
}

// ─────────────── Basic carrier behaviour ───────────────

#[test]
fn zero_index_is_pure_carrier() {
    // Index=0 means no phase modulation — output should be a pure sine
    // at the carrier frequency.
    let mut sig = fm_op(440.0, osc::sine(880.0), 0.0);
    let buf = render_to_buffer(&mut sig, 0.1, SR);

    // Range check
    for &s in &buf {
        assert!((-1.0..=1.0).contains(&s), "pure sine out of range: {s}");
    }

    // Zero crossings ≈ 2 * freq * duration = 2 * 440 * 0.1 = 88.
    let crossings = zero_crossings(&buf);
    assert!(
        (crossings as i32 - 88).abs() <= 4,
        "440 Hz carrier should have ~88 crossings over 0.1 s, got {crossings}"
    );
}

#[test]
fn fluent_and_direct_apis_agree() {
    // fm_op(freq, mod, idx) and osc::sine(freq).fm(mod, idx) should
    // produce identical output (preserving initial phase = 0).
    let mut a = fm_op(440.0, osc::sine(880.0), 2.0);
    let mut b = osc::sine(440.0).fm(osc::sine(880.0), 2.0);
    for tick in 0..256 {
        let got_a = a.next(&ctx(tick));
        let got_b = b.next(&ctx(tick));
        assert!(
            (got_a - got_b).abs() < 1e-6,
            "divergence at tick {tick}: {got_a} vs {got_b}"
        );
    }
}

// ─────────────── Modulation adds spectral content ───────────────

#[test]
fn nonzero_index_adds_sidebands() {
    // A simple test: higher modulation index → more zero-crossings
    // (rough proxy for spectral brightness). 3:1 modulator ratio with
    // index 4 should have noticeably more crossings than carrier alone.
    let mut plain = fm_op(440.0, osc::sine(1320.0), 0.0);
    let mut modulated = fm_op(440.0, osc::sine(1320.0), 4.0);
    let plain_buf = render_to_buffer(&mut plain, 0.2, SR);
    let mod_buf = render_to_buffer(&mut modulated, 0.2, SR);

    let plain_zc = zero_crossings(&plain_buf);
    let mod_zc = zero_crossings(&mod_buf);
    assert!(
        mod_zc > plain_zc * 2,
        "FM should increase crossings: plain={plain_zc}, modulated={mod_zc}"
    );
}

#[test]
fn output_stays_in_range() {
    // Any index produces sin() output which is always in [-1, 1].
    let mut sig = fm_op(440.0, osc::sine(880.0), 50.0); // extreme index
    for tick in 0..4096 {
        let v = sig.next(&ctx(tick));
        assert!((-1.0..=1.0).contains(&v), "sin output out of range: {v}");
    }
}

// ─────────────── Feedback ───────────────

#[test]
fn zero_feedback_is_default() {
    // Without calling .feedback(), the operator should behave like a
    // normal FM operator (no self-modulation).
    let mut a = fm_op(440.0, osc::sine(880.0), 1.0);
    let mut b = fm_op(440.0, osc::sine(880.0), 1.0).feedback(0.0);
    for tick in 0..256 {
        let ga = a.next(&ctx(tick));
        let gb = b.next(&ctx(tick));
        assert!((ga - gb).abs() < 1e-6);
    }
}

#[test]
fn feedback_changes_timbre() {
    let mut plain = fm_op(440.0, osc::sine(880.0), 0.0);
    let mut fb = fm_op(440.0, osc::sine(880.0), 0.0).feedback(0.6);
    let plain_buf = render_to_buffer(&mut plain, 0.1, SR);
    let fb_buf = render_to_buffer(&mut fb, 0.1, SR);

    // Feedback alone (index=0) should still produce bright output due
    // to self-modulation. Compare zero-crossings.
    let plain_zc = zero_crossings(&plain_buf);
    let fb_zc = zero_crossings(&fb_buf);
    assert!(
        fb_zc > plain_zc,
        "feedback should add harmonics: plain={plain_zc}, fb={fb_zc}"
    );
}

#[test]
fn feedback_clamped_to_range() {
    // feedback=5.0 should be clamped internally.
    let mut sig = fm_op(440.0, osc::sine(880.0), 1.0).feedback(5.0);
    for tick in 0..4096 {
        let v = sig.next(&ctx(tick));
        assert!(v.is_finite() && v.abs() <= 1.0, "feedback blow-up: {v}");
    }
}

// ─────────────── Modulatable params ───────────────

#[test]
fn index_accepts_signal() {
    // Index as a slow LFO — confirms IntoParam works for `index`.
    let index_lfo = osc::sine(0.5);
    let mut sig = fm_op(440.0, osc::sine(880.0), index_lfo);
    let buf = render_to_buffer(&mut sig, 0.1, SR);
    assert!(rms(&buf) > 0.1, "modulated-index FM should produce audio");
}

#[test]
fn freq_accepts_signal() {
    use nyx_core::SignalExt;
    // Carrier freq as a signal — vibrato.
    let vibrato = osc::sine(5.0).amp(10.0).offset(440.0);
    let mut sig = fm_op(vibrato, osc::sine(880.0), 2.0);
    let buf = render_to_buffer(&mut sig, 0.1, SR);
    assert!(rms(&buf) > 0.1);
}

// ─────────────── No-alloc ───────────────

#[test]
fn fm_does_not_allocate_per_sample() {
    let mut sig = fm_op(440.0, osc::sine(880.0), 2.0).feedback(0.3);
    let c = ctx(0);
    for _ in 0..10 {
        sig.next(&c);
    }
    let _guard = DenyAllocGuard::new();
    for _ in 0..4096 {
        sig.next(&c);
    }
}

// ─────────────── Chaining ───────────────

#[test]
fn fm_chains_with_amp_and_filter() {
    use nyx_core::{FilterExt, SignalExt};
    let mut sig = fm_op(440.0, osc::sine(660.0), 3.0)
        .feedback(0.2)
        .amp(0.5)
        .svf_lp(3000.0, 0.7);
    let buf = render_to_buffer(&mut sig, 0.1, SR);
    assert!(rms(&buf) > 0.05);
    for &s in &buf {
        assert!(s.is_finite() && s.abs() <= 1.0);
    }
}
