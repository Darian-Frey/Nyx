//! Sprint 3 — Chorus + Flanger tests.

use nyx_core::{osc, render_to_buffer, AudioContext, DenyAllocGuard, Signal, SignalExt};

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

// ─────────────── Chorus ───────────────

#[test]
fn chorus_mix_zero_passes_dry() {
    // With mix=0, output should equal input.
    let mut sig = osc::sine(440.0).chorus(0.5, 3.0).mix(0.0);
    // Run a few samples then compare to a fresh mono sine.
    let buf = render_to_buffer(&mut sig, 0.05, SR);

    let mut fresh = osc::sine(440.0);
    for (i, &s) in buf.iter().enumerate() {
        let expected = fresh.next(&ctx(i as u64));
        assert!(
            (s - expected).abs() < 1e-4,
            "mix=0 should pass dry at sample {i}: got {s}, want {expected}"
        );
    }
}

#[test]
fn chorus_mix_one_is_pure_wet() {
    // mix=1: output is the delayed wet, not dry. Should produce audible
    // signal but different from the direct sine.
    let mut sig = osc::sine(440.0).chorus(0.5, 3.0).mix(1.0);
    let buf = render_to_buffer(&mut sig, 0.1, SR);
    let r = rms(&buf);
    assert!(r > 0.3, "mix=1 should still have energy, rms={r}");
}

#[test]
fn chorus_produces_stereo_output() {
    // Two LFOs 180° apart → L and R should differ.
    let mut sig = osc::sine(440.0).chorus(1.0, 5.0).mix(0.8);
    // Skip through a few buffer lengths so the delay line is primed.
    for _ in 0..8192 {
        sig.next_stereo(&ctx(0));
    }
    let mut diff_sum = 0.0_f32;
    for tick in 0..4096 {
        let (l, r) = sig.next_stereo(&ctx(tick));
        diff_sum += (l - r).abs();
    }
    assert!(
        diff_sum > 1.0,
        "chorus stereo should differ L/R, got diff_sum={diff_sum}"
    );
}

#[test]
fn chorus_output_bounded() {
    let mut sig = osc::saw(110.0).amp(0.5).chorus(0.3, 5.0).mix(0.5);
    let buf = render_to_buffer(&mut sig, 0.5, SR);
    let peak = buf.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
    assert!(peak.is_finite() && peak < 2.0, "chorus peak = {peak}");
}

#[test]
fn chorus_does_not_allocate() {
    let mut sig = osc::sine(440.0).chorus(0.5, 3.0).mix(0.5);
    let c = ctx(0);
    for _ in 0..10 {
        sig.next(&c);
    }
    let _guard = DenyAllocGuard::new();
    for _ in 0..4096 {
        sig.next(&c);
    }
}

#[test]
fn chorus_base_delay_builder() {
    // Just verify the builder compiles and changes state.
    let mut sig = osc::sine(440.0)
        .chorus(0.5, 3.0)
        .base_delay(25.0)
        .mix(0.5);
    let _ = sig.next(&ctx(0));
}

// ─────────────── Flanger ───────────────

#[test]
fn flanger_mix_zero_passes_dry() {
    let mut sig = osc::sine(440.0).flanger(0.3, 2.0).mix(0.0);
    let buf = render_to_buffer(&mut sig, 0.05, SR);

    let mut fresh = osc::sine(440.0);
    for (i, &s) in buf.iter().enumerate() {
        let expected = fresh.next(&ctx(i as u64));
        assert!(
            (s - expected).abs() < 1e-4,
            "mix=0 dry mismatch at {i}: {s} vs {expected}"
        );
    }
}

#[test]
fn flanger_mix_one_produces_wet_output() {
    let mut sig = osc::sine(440.0).flanger(0.5, 2.0).mix(1.0);
    let buf = render_to_buffer(&mut sig, 0.2, SR);
    let r = rms(&buf);
    assert!(r > 0.2, "flanger wet-only should produce signal, rms={r}");
}

#[test]
fn flanger_feedback_changes_output() {
    // Feedback doesn't necessarily increase RMS — it shapes the
    // spectrum via comb-filter resonances. Here we just verify that
    // feedback=0 and feedback=0.8 produce materially different output.
    let mut plain = osc::sine(440.0).flanger(0.3, 2.0).feedback(0.0).mix(1.0);
    let mut heavy = osc::sine(440.0).flanger(0.3, 2.0).feedback(0.8).mix(1.0);
    let pb = render_to_buffer(&mut plain, 0.5, SR);
    let hb = render_to_buffer(&mut heavy, 0.5, SR);

    // Sample-by-sample divergence, ignoring the first few samples
    // while the feedback builds up.
    let div: f32 = pb
        .iter()
        .zip(hb.iter())
        .skip(1000)
        .map(|(a, b)| (a - b).abs())
        .sum();
    assert!(div > 10.0, "feedback should change output, divergence={div}");
}

#[test]
fn flanger_feedback_clamped() {
    // feedback=5 is nonsense; internally clamped to 0.95 to prevent blow-up.
    let mut sig = osc::saw(110.0).amp(0.2).flanger(0.3, 2.0).feedback(5.0).mix(1.0);
    let buf = render_to_buffer(&mut sig, 0.5, SR);
    let peak = buf.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
    assert!(peak < 10.0, "feedback clamp should prevent explosion: peak={peak}");
    for &s in &buf {
        assert!(s.is_finite(), "non-finite output: {s}");
    }
}

#[test]
fn flanger_stereo_differs() {
    let mut sig = osc::saw(220.0).flanger(1.0, 3.0).mix(0.8);
    for _ in 0..4096 {
        sig.next_stereo(&ctx(0));
    }
    let mut diff = 0.0_f32;
    for tick in 0..4096 {
        let (l, r) = sig.next_stereo(&ctx(tick));
        diff += (l - r).abs();
    }
    assert!(diff > 1.0, "flanger should produce stereo diff, got {diff}");
}

#[test]
fn flanger_does_not_allocate() {
    let mut sig = osc::saw(110.0).flanger(0.3, 2.0).feedback(0.5).mix(0.5);
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
fn chorus_flanger_chain() {
    // Sanity: chain both on a saw, verify output is finite and bounded.
    let mut sig = osc::saw(220.0)
        .amp(0.3)
        .chorus(0.4, 3.0)
        .flanger(0.2, 1.5)
        .feedback(0.4);
    let buf = render_to_buffer(&mut sig, 0.5, SR);
    for (i, &s) in buf.iter().enumerate() {
        assert!(s.is_finite() && s.abs() < 5.0, "bad sample at {i}: {s}");
    }
}
