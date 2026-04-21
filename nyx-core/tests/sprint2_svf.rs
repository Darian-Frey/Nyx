//! Sprint 2 — State-variable filter tests.

use nyx_core::{AudioContext, DenyAllocGuard, FilterExt, Signal, render_to_buffer};

const SR: f32 = 44100.0;

fn ctx(tick: u64) -> AudioContext {
    AudioContext {
        sample_rate: SR,
        tick,
    }
}

/// Simple sine source used by the filter tests.
struct Sine {
    phase: f32,
    freq: f32,
}

impl Sine {
    fn new(freq: f32) -> Self {
        Self { phase: 0.0, freq }
    }
}

impl Signal for Sine {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let out = (self.phase * std::f32::consts::TAU).sin();
        self.phase += self.freq / ctx.sample_rate;
        self.phase -= self.phase.floor();
        out
    }
}

fn rms(buf: &[f32]) -> f32 {
    (buf.iter().map(|s| s * s).sum::<f32>() / buf.len() as f32).sqrt()
}

// ─────────────── Low-pass ───────────────

#[test]
fn svf_lp_passes_low_frequencies() {
    let sig = Sine::new(100.0).svf_lp(2000.0, 0.707);
    let mut sig = sig;
    let buf = render_to_buffer(&mut sig, 0.2, SR);
    // Skip first ~10 ms while the filter settles.
    let rms = rms(&buf[(SR * 0.01) as usize..]);
    assert!(rms > 0.5, "100 Hz through 2 kHz LP should pass, rms={rms}");
}

#[test]
fn svf_lp_attenuates_high_frequencies() {
    let mut sig = Sine::new(10000.0).svf_lp(500.0, 0.707);
    let _ = render_to_buffer(&mut sig, 0.05, SR);
    let buf = render_to_buffer(&mut sig, 0.1, SR);
    let rms = rms(&buf);
    assert!(
        rms < 0.05,
        "10 kHz through 500 Hz LP should attenuate, rms={rms}"
    );
}

// ─────────────── High-pass ───────────────

#[test]
fn svf_hp_passes_high_frequencies() {
    let mut sig = Sine::new(10000.0).svf_hp(500.0, 0.707);
    let _ = render_to_buffer(&mut sig, 0.05, SR);
    let buf = render_to_buffer(&mut sig, 0.1, SR);
    let rms = rms(&buf);
    assert!(rms > 0.5, "10 kHz through 500 Hz HP should pass, rms={rms}");
}

#[test]
fn svf_hp_attenuates_low_frequencies() {
    let mut sig = Sine::new(100.0).svf_hp(5000.0, 0.707);
    let _ = render_to_buffer(&mut sig, 0.05, SR);
    let buf = render_to_buffer(&mut sig, 0.1, SR);
    let rms = rms(&buf);
    assert!(
        rms < 0.05,
        "100 Hz through 5 kHz HP should attenuate, rms={rms}"
    );
}

// ─────────────── Band-pass ───────────────

#[test]
fn svf_bp_passes_resonant_frequency() {
    // Sine right at the BP cutoff should pass more-or-less unattenuated.
    let mut sig = Sine::new(1000.0).svf_bp(1000.0, 2.0);
    let _ = render_to_buffer(&mut sig, 0.05, SR);
    let buf = render_to_buffer(&mut sig, 0.1, SR);
    let rms = rms(&buf);
    assert!(rms > 0.3, "sine at BP centre should pass, rms={rms}");
}

#[test]
fn svf_bp_rejects_far_from_centre() {
    // A 100 Hz sine through a 5 kHz BP (narrow) should be attenuated.
    let mut sig = Sine::new(100.0).svf_bp(5000.0, 5.0);
    let _ = render_to_buffer(&mut sig, 0.05, SR);
    let buf = render_to_buffer(&mut sig, 0.1, SR);
    let rms = rms(&buf);
    assert!(
        rms < 0.2,
        "off-centre sine should be attenuated by BP, rms={rms}"
    );
}

// ─────────────── Notch ───────────────

#[test]
fn svf_notch_rejects_at_centre() {
    // Sine right at notch frequency should be heavily attenuated.
    let mut sig = Sine::new(1000.0).svf_notch(1000.0, 5.0);
    let _ = render_to_buffer(&mut sig, 0.05, SR);
    let buf = render_to_buffer(&mut sig, 0.1, SR);
    let rms = rms(&buf);
    assert!(
        rms < 0.2,
        "sine at notch centre should be rejected, rms={rms}"
    );
}

#[test]
fn svf_notch_passes_far_from_centre() {
    // Sine well below notch centre should pass.
    let mut sig = Sine::new(100.0).svf_notch(5000.0, 2.0);
    let _ = render_to_buffer(&mut sig, 0.05, SR);
    let buf = render_to_buffer(&mut sig, 0.1, SR);
    let rms = rms(&buf);
    assert!(
        rms > 0.5,
        "sine well away from notch should pass, rms={rms}"
    );
}

// ─────────────── Modulation ───────────────

#[test]
fn svf_handles_fast_cutoff_modulation_without_clicks() {
    // LFO at 10 Hz sweeping cutoff 200–2200 Hz. SVF should handle this
    // cleanly without clicks (biquad would need smoothing for this).
    use std::f32::consts::TAU;
    let mut lfo_phase = 0.0_f32;
    let lfo = move |ctx: &AudioContext| {
        let v = (lfo_phase * TAU).sin() * 1000.0 + 1200.0;
        lfo_phase += 10.0 / ctx.sample_rate;
        lfo_phase -= lfo_phase.floor();
        v
    };
    let mut sig = Sine::new(440.0).svf_lp(lfo, 2.0);
    let buf = render_to_buffer(&mut sig, 1.0, SR);

    // Sample-to-sample derivative should stay bounded — huge spikes
    // indicate clicks.
    let max_diff = buf
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0_f32, f32::max);
    assert!(
        max_diff < 0.5,
        "fast cutoff sweep should not click; max diff = {max_diff}"
    );
}

// ─────────────── Parameter clamping ───────────────

#[test]
fn svf_clamps_extreme_cutoffs() {
    // Cutoff of 10 Hz (below minimum) — should be clamped, not NaN out.
    let mut sig = Sine::new(440.0).svf_lp(5.0, 1.0);
    for tick in 0..1000 {
        let v = sig.next(&ctx(tick));
        assert!(v.is_finite(), "low cutoff should clamp, got {v}");
    }
    // Cutoff near Nyquist — should also stay bounded.
    let mut sig = Sine::new(440.0).svf_lp(50000.0, 1.0);
    for tick in 0..1000 {
        let v = sig.next(&ctx(tick));
        assert!(v.is_finite(), "high cutoff should clamp, got {v}");
    }
}

#[test]
fn svf_clamps_low_q() {
    // Q below 0.5 should be clamped (else filter blows up).
    let mut sig = Sine::new(440.0).svf_lp(1000.0, 0.01);
    let buf = render_to_buffer(&mut sig, 0.1, SR);
    for &s in &buf {
        assert!(s.is_finite() && s.abs() < 100.0, "low Q clamp failed: {s}");
    }
}

// ─────────────── No-alloc ───────────────

#[test]
fn svf_does_not_allocate_per_sample() {
    let mut sig = Sine::new(440.0).svf_lp(1000.0, 2.0);
    let c = ctx(0);
    // Prime with a few samples first
    for _ in 0..10 {
        sig.next(&c);
    }
    let _guard = DenyAllocGuard::new();
    for _ in 0..4096 {
        sig.next(&c);
    }
}

// ─────────────── Chaining with combinators ───────────────

#[test]
fn svf_chains_with_amp_and_clip() {
    use nyx_core::SignalExt;
    let mut sig = Sine::new(440.0).svf_lp(2000.0, 0.7).amp(0.5).clip(0.4);
    let buf = render_to_buffer(&mut sig, 0.1, SR);
    for &s in &buf {
        assert!(s.abs() <= 0.4 + 1e-6, "clip exceeded: {s}");
    }
}
