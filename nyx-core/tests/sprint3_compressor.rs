//! Sprint 3 — Compressor + Sidechain tests.

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

fn peak(buf: &[f32]) -> f32 {
    buf.iter().map(|s| s.abs()).fold(0.0_f32, f32::max)
}

// ───────────────────────── Compressor ─────────────────────────

#[test]
fn compressor_below_threshold_is_transparent() {
    // Quiet signal well below threshold: output ≈ input.
    let mut sig = osc::sine(440.0).amp(0.1).compress(-6.0, 4.0);
    let mut fresh = osc::sine(440.0).amp(0.1);
    let buf = render_to_buffer(&mut sig, 0.2, SR);
    let ref_buf = render_to_buffer(&mut fresh, 0.2, SR);
    for (i, (&s, &e)) in buf.iter().zip(ref_buf.iter()).enumerate().skip(1000) {
        assert!(
            (s - e).abs() < 1e-3,
            "below-threshold compression should be transparent at {i}: got {s}, want {e}"
        );
    }
}

#[test]
fn compressor_reduces_loud_signal_rms() {
    // Hot signal above threshold: RMS should drop meaningfully.
    let mut loud = osc::sine(440.0);
    let mut comp = osc::sine(440.0).compress(-20.0, 8.0).attack_ms(1.0);

    let uncompressed = render_to_buffer(&mut loud, 0.5, SR);
    let compressed = render_to_buffer(&mut comp, 0.5, SR);

    let u = rms(&uncompressed);
    let c = rms(&compressed);
    assert!(
        c < u * 0.7,
        "hot signal should be compressed: uncompressed rms={u}, compressed rms={c}"
    );
}

#[test]
fn compressor_ratio_scales_reduction() {
    // Higher ratio = more reduction for the same signal.
    let mut soft = osc::sine(440.0).compress(-20.0, 2.0).attack_ms(1.0);
    let mut hard = osc::sine(440.0).compress(-20.0, 20.0).attack_ms(1.0);

    let sb = render_to_buffer(&mut soft, 0.5, SR);
    let hb = render_to_buffer(&mut hard, 0.5, SR);

    let s_rms = rms(&sb);
    let h_rms = rms(&hb);
    assert!(
        h_rms < s_rms,
        "higher ratio should reduce more: 2:1 rms={s_rms}, 20:1 rms={h_rms}"
    );
}

#[test]
fn compressor_makeup_gain_boosts_output() {
    let mut plain = osc::sine(440.0).compress(-20.0, 8.0).attack_ms(1.0);
    let mut boosted = osc::sine(440.0)
        .compress(-20.0, 8.0)
        .attack_ms(1.0)
        .makeup_db(6.0);

    let pb = render_to_buffer(&mut plain, 0.5, SR);
    let bb = render_to_buffer(&mut boosted, 0.5, SR);

    assert!(rms(&bb) > rms(&pb) * 1.5, "makeup +6dB should roughly 2x rms");
}

#[test]
fn compressor_output_is_finite_and_bounded() {
    let mut sig = osc::saw(110.0).amp(2.0).compress(-12.0, 4.0).makeup_db(6.0);
    let buf = render_to_buffer(&mut sig, 0.5, SR);
    for (i, &s) in buf.iter().enumerate() {
        assert!(s.is_finite(), "non-finite at {i}: {s}");
    }
    assert!(peak(&buf) < 10.0, "compressor output unbounded");
}

#[test]
fn compressor_infinite_ratio_acts_as_limiter() {
    // ratio = inf → hard limiter. Peak should be close to threshold amp.
    // -6 dB ≈ 0.501 linear. Allow some overshoot during attack.
    let mut sig = osc::sine(440.0).compress(-6.0, f32::INFINITY).attack_ms(0.5);
    let buf = render_to_buffer(&mut sig, 0.5, SR);
    // Ignore first 2000 samples while envelope settles.
    let steady_peak = buf.iter().skip(2000).map(|s| s.abs()).fold(0.0_f32, f32::max);
    assert!(
        steady_peak < 0.7,
        "limiter should clamp to ~threshold: peak={steady_peak}"
    );
}

#[test]
fn compressor_does_not_allocate() {
    let mut sig = osc::sine(440.0).compress(-12.0, 4.0).attack_ms(5.0).release_ms(100.0);
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
fn compressor_preserves_stereo_balance() {
    // Pan the source hard-left; compressor must not redistribute to R.
    let mut sig = osc::sine(440.0).pan(-1.0).compress(-12.0, 4.0);
    let mut sum_l = 0.0_f32;
    let mut sum_r = 0.0_f32;
    for tick in 0..8192 {
        let (l, r) = sig.next_stereo(&ctx(tick));
        sum_l += l.abs();
        sum_r += r.abs();
    }
    assert!(sum_l > 10.0, "L channel should have content, sum_l={sum_l}");
    assert!(
        sum_r < 0.01,
        "R channel should stay silent under panned compression, sum_r={sum_r}"
    );
}

// ───────────────────────── Sidechain ─────────────────────────

/// Impulse trigger that fires a single loud sample on cycle start, silence elsewhere.
/// Models a kick drum at `period` samples.
struct PulseTrigger {
    period: u64,
    amp: f32,
}

impl Signal for PulseTrigger {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        // Emit a short decaying burst at each period boundary.
        let phase = ctx.tick % self.period;
        if phase < 64 {
            self.amp * (1.0 - phase as f32 / 64.0)
        } else {
            0.0
        }
    }
}

#[test]
fn sidechain_ducks_with_trigger() {
    // Sustained sine, ducked by periodic kick pulses.
    let trigger = PulseTrigger { period: 22050, amp: 1.0 }; // pulse every 0.5s
    let mut sig = osc::sine(440.0)
        .sidechain(trigger, -30.0, 20.0)
        .attack_ms(1.0)
        .release_ms(200.0);

    let buf = render_to_buffer(&mut sig, 1.0, SR);
    assert_eq!(buf.len(), SR as usize);

    // Windows of 256 samples just after each pulse should have lower RMS
    // than windows that are well clear of any pulse.
    let pulse_window: f32 = {
        let start = 100; // just after first pulse at t=0
        rms(&buf[start..start + 256])
    };
    let clear_window: f32 = {
        // 0.25s into the cycle, far from pulses at 0 and 0.5s
        let start = 11000;
        rms(&buf[start..start + 256])
    };

    assert!(
        pulse_window < clear_window * 0.6,
        "sidechain should duck post-trigger: pulse_window={pulse_window}, clear_window={clear_window}"
    );
}

#[test]
fn sidechain_ratio_one_is_near_transparent() {
    // ratio=1.0 means no compression regardless of trigger level.
    let trigger = osc::sine(2.0).amp(1.0); // loud LFO
    let mut ducked = osc::sine(440.0)
        .amp(0.3)
        .sidechain(trigger, -40.0, 1.0)
        .attack_ms(1.0);

    let buf = render_to_buffer(&mut ducked, 0.5, SR);

    // Reference: same source with no compressor.
    let mut clean = osc::sine(440.0).amp(0.3);
    let ref_buf = render_to_buffer(&mut clean, 0.5, SR);

    let diff: f32 = buf.iter().zip(ref_buf.iter())
        .skip(500)
        .map(|(a, b)| (a - b).abs())
        .sum();
    assert!(
        diff < 1.0,
        "ratio=1 should be near-transparent, total diff={diff}"
    );
}

#[test]
fn sidechain_silent_trigger_passes_source() {
    // Trigger is silent → no compression → source passes through.
    let silent = |_: &AudioContext| 0.0_f32;
    let mut sig = osc::sine(440.0).amp(0.2).sidechain(silent, -30.0, 10.0);
    let mut fresh = osc::sine(440.0).amp(0.2);
    let buf = render_to_buffer(&mut sig, 0.1, SR);
    let ref_buf = render_to_buffer(&mut fresh, 0.1, SR);
    for (i, (&s, &e)) in buf.iter().zip(ref_buf.iter()).enumerate().skip(500) {
        assert!(
            (s - e).abs() < 1e-3,
            "silent trigger should pass source at {i}: got {s}, want {e}"
        );
    }
}

#[test]
fn sidechain_does_not_allocate() {
    let trigger = osc::sine(2.0);
    let mut sig = osc::sine(440.0).sidechain(trigger, -20.0, 8.0).attack_ms(1.0);
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
fn sidechain_stereo_balance_preserved() {
    // Panned source, loud trigger — both channels should duck equally.
    let trigger = osc::sine(2.0);
    let mut sig = osc::sine(440.0)
        .pan(0.7)
        .sidechain(trigger, -30.0, 10.0)
        .attack_ms(1.0);

    let mut sum_l = 0.0_f32;
    let mut sum_r = 0.0_f32;
    for tick in 0..8192 {
        let (l, r) = sig.next_stereo(&ctx(tick));
        sum_l += l.abs();
        sum_r += r.abs();
    }
    // pos=0.7 → L = (1 - 0.7)/2 = 0.15, R = (1 + 0.7)/2 = 0.85, ratio ≈ 5.67
    let ratio = sum_r / sum_l.max(1e-6);
    assert!(
        ratio > 4.0 && ratio < 8.0,
        "pan ratio should survive sidechain, got sum_r/sum_l={ratio}"
    );
}
