//! Sprint 1 — Delay line + feedback tests.

use nyx_core::{render_to_buffer, AudioContext, DenyAllocGuard, Signal, SignalExt};

const SR: f32 = 44100.0;

fn ctx(tick: u64) -> AudioContext {
    AudioContext {
        sample_rate: SR,
        tick,
    }
}

/// A one-shot impulse: emits 1.0 on tick 0, then silence forever.
struct Impulse {
    fired: bool,
}

impl Impulse {
    fn new() -> Self {
        Self { fired: false }
    }
}

impl Signal for Impulse {
    fn next(&mut self, _ctx: &AudioContext) -> f32 {
        if self.fired {
            0.0
        } else {
            self.fired = true;
            1.0
        }
    }
}

/// A counter that emits 1.0, 2.0, 3.0, 4.0, ...
struct Counter(f32);

impl Signal for Counter {
    fn next(&mut self, _ctx: &AudioContext) -> f32 {
        self.0 += 1.0;
        self.0
    }
}

// ─────────────────────── Basic behaviour ───────────────────────

#[test]
fn zero_mix_is_dry_signal() {
    let mut sig = Counter(0.0).delay(0.01).mix(0.0);
    // mix=0 → full dry, zero wet. Smoothing starts at 0.5 default, so
    // wait for it to settle near 0.
    for _ in 0..500 {
        sig.next(&ctx(0));
    }
    let out = sig.next(&ctx(0));
    let next = sig.next(&ctx(0));
    // Values are counter+1, counter+2 etc. after settling. Just check
    // they track what we'd expect for a dry-through behavior: i.e. they
    // grow linearly with the counter.
    assert!(next - out > 0.5, "dry path should pass counter through");
}

#[test]
fn full_mix_is_pure_delayed() {
    let mut sig = Impulse::new().delay(0.01).mix(1.0).feedback(0.0);
    // Let smoothing settle
    let _ = render_to_buffer(&mut sig, 0.05, SR);

    // After settling, fire impulse on a fresh delay
    let mut sig = Impulse::new().delay(0.01).mix(1.0).feedback(0.0);
    // Warm up smoothing with the pre-impulse silence
    let buf = render_to_buffer(&mut sig, 0.05, SR);

    // The delay is 441 samples. With smoothing starting mix at 0.5 and
    // converging toward 1.0 over ~5ms (220 samples), by sample ~441 the
    // wet mix should be effectively 1.0. So we should see the impulse
    // echo around sample index 441.
    //
    // Find the peak past sample 200 (after smoothing convergence).
    let (peak_idx, peak_val) = buf
        .iter()
        .enumerate()
        .skip(200)
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .unwrap();

    let expected = 441; // 0.01s * 44100
    assert!(
        (peak_idx as i32 - expected).abs() < 20,
        "echo peak near sample {expected}, got {peak_idx} (value {peak_val})"
    );
}

#[test]
fn feedback_produces_multiple_echoes() {
    // Half-feedback, full-wet. Feed an impulse; expect ~N echoes where
    // echo_n amplitude ≈ 0.5^n (modulo smoothing).
    let mut sig = Impulse::new()
        .delay(0.01)
        .mix(1.0)
        .feedback(0.5);
    let buf = render_to_buffer(&mut sig, 0.1, SR);

    // Look at echoes at 1x, 2x, 3x the delay time.
    let delay_samples = 441;
    let peak_1 = buf[delay_samples - 5..delay_samples + 5]
        .iter()
        .cloned()
        .fold(0.0_f32, f32::max);
    let peak_2 = buf[2 * delay_samples - 5..2 * delay_samples + 5]
        .iter()
        .cloned()
        .fold(0.0_f32, f32::max);
    let peak_3 = buf[3 * delay_samples - 5..3 * delay_samples + 5]
        .iter()
        .cloned()
        .fold(0.0_f32, f32::max);

    // Echo ratio should approximately match feedback gain.
    assert!(peak_2 < peak_1, "2nd echo weaker than 1st");
    assert!(peak_3 < peak_2, "3rd echo weaker than 2nd");
    assert!(peak_2 > peak_1 * 0.2, "2nd echo not degraded too fast: {peak_1} -> {peak_2}");
}

#[test]
fn feedback_clamps_at_boundary() {
    // User sets feedback to 2.0 — we must clamp internally so output
    // stays bounded rather than exploding.
    let mut sig = Impulse::new()
        .delay(0.005)
        .mix(1.0)
        .feedback(2.0);
    let buf = render_to_buffer(&mut sig, 0.2, SR);

    let peak_abs = buf.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
    assert!(
        peak_abs < 2.0,
        "feedback=2.0 must be clamped — output exploded to {peak_abs}"
    );
}

#[test]
fn feedback_zero_gives_single_echo() {
    let mut sig = Impulse::new()
        .delay(0.005)
        .mix(1.0)
        .feedback(0.0);
    let buf = render_to_buffer(&mut sig, 0.03, SR);

    // After the single echo, everything should decay to ~0.
    let delay_samples = (0.005 * SR) as usize;
    let late = &buf[3 * delay_samples..];
    let late_peak = late.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
    assert!(late_peak < 0.1, "no feedback should mean single echo, late peak={late_peak}");
}

// ─────────────────────── Modulation / smoothing ───────────────────────

#[test]
fn modulated_time_produces_no_zipper() {
    // Sweep the delay time via an LFO. Confirm output RMS stays bounded
    // and doesn't spike (which would indicate a click from unsmoothed
    // parameter jumps).
    use std::f32::consts::TAU;
    let mut lfo_phase = 0.0_f32;
    let lfo = move |ctx: &AudioContext| {
        let out = (lfo_phase * TAU).sin() * 0.005 + 0.020; // 15–25 ms
        lfo_phase += 3.0 / ctx.sample_rate;
        lfo_phase -= lfo_phase.floor();
        out
    };

    let mut src_phase = 0.0_f32;
    let src = move |ctx: &AudioContext| {
        let out = (src_phase * TAU).sin();
        src_phase += 440.0 / ctx.sample_rate;
        src_phase -= src_phase.floor();
        out
    };

    let mut sig = src
        .delay(0.025)
        .max_time(0.05)
        .time(lfo)
        .feedback(0.3)
        .mix(0.5);
    let buf = render_to_buffer(&mut sig, 0.5, SR);

    // Peak magnitude of the derivative (first difference). A clean sweep
    // produces small derivatives; clicks produce huge ones.
    let max_diff = buf
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0_f32, f32::max);
    assert!(
        max_diff < 0.5,
        "zipper detected — max sample-to-sample diff = {max_diff}"
    );
}

#[test]
fn static_dry_path_passes_input() {
    // With mix=0 and enough warm-up, the delay should be fully bypassed.
    let mut sig = Counter(0.0).delay(0.01).mix(0.0).feedback(0.0);
    // Warm up smoothing — initial mix defaults to 0.5.
    let _ = render_to_buffer(&mut sig, 0.05, SR);
    // After warm-up, output should track the counter.
    let a = sig.next(&ctx(0));
    let b = sig.next(&ctx(0));
    assert!(
        (b - a - 1.0).abs() < 0.01,
        "dry path should increment counter by 1, got {a} -> {b}"
    );
}

#[test]
fn max_time_grows_buffer() {
    // The buffer length should be large enough to accommodate max_time
    // that exceeds the initial delay value.
    let mut sig = Impulse::new()
        .delay(0.01)
        .max_time(0.1)
        .time(0.08)
        .mix(1.0)
        .feedback(0.0);
    let buf = render_to_buffer(&mut sig, 0.15, SR);

    // Echo should appear near sample 0.08 * SR = 3528.
    let target = (0.08 * SR) as usize;
    let peak_idx = buf
        .iter()
        .enumerate()
        .skip(500)
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .unwrap()
        .0;
    assert!(
        (peak_idx as i32 - target as i32).abs() < 30,
        "echo near sample {target}, got {peak_idx}"
    );
}

// ─────────────────────── No-alloc guard ───────────────────────

#[test]
fn delay_does_not_allocate_per_sample() {
    let mut sig = Impulse::new().delay(0.02).feedback(0.3).mix(0.5);
    let c = ctx(0);
    // Prime the smoother — any lazy init happens before we arm the guard.
    for _ in 0..10 {
        sig.next(&c);
    }
    let _guard = DenyAllocGuard::new();
    for _ in 0..4096 {
        sig.next(&c);
    }
}
