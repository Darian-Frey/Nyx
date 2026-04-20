//! Sprint 2 — Freeverb tests.

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

/// A one-shot impulse: 1.0 on tick 0, then silence forever.
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

// ─────────────── Dry/wet mix ───────────────

#[test]
fn wet_zero_passes_dry_signal_through() {
    // wet=0 → output = dry input (no reverb contribution).
    let mut sig = Impulse::new().freeverb().wet(0.0);
    let buf = render_to_buffer(&mut sig, 0.1, SR);
    // The impulse should appear at sample 0; rest should be zero.
    assert!((buf[0] - 1.0).abs() < 1e-6, "wet=0 should pass impulse, got {}", buf[0]);
    let tail_rms = rms(&buf[100..]);
    assert!(tail_rms < 1e-6, "wet=0 should have silent tail, rms={tail_rms}");
}

#[test]
fn wet_one_produces_pure_reverb_tail() {
    // wet=1 → dry is gone; only the reverb tail remains.
    let mut sig = Impulse::new().freeverb().wet(1.0).room_size(0.8);
    let buf = render_to_buffer(&mut sig, 0.5, SR);
    // Early portion should have reverb buildup (non-zero).
    let rms_mid = rms(&buf[1000..5000]);
    assert!(rms_mid > 1e-4, "wet=1 should have audible tail, rms={rms_mid}");
    // First sample should NOT be the impulse itself.
    assert!(buf[0].abs() < 0.1, "wet=1 should suppress dry, got {}", buf[0]);
}

// ─────────────── Decay tail ───────────────

#[test]
fn larger_room_sustains_longer() {
    // Compare RMS of tails at 1 s with small vs large room sizes.
    let mut small = Impulse::new().freeverb().room_size(0.2).wet(1.0);
    let mut large = Impulse::new().freeverb().room_size(0.95).wet(1.0);
    let small_buf = render_to_buffer(&mut small, 1.0, SR);
    let large_buf = render_to_buffer(&mut large, 1.0, SR);

    let tail_small = rms(&small_buf[40000..]);
    let tail_large = rms(&large_buf[40000..]);
    assert!(
        tail_large > tail_small * 1.5,
        "larger room should sustain longer: small={tail_small}, large={tail_large}"
    );
}

#[test]
fn damping_reduces_high_frequency_content() {
    // Damped reverb should have less high-frequency energy than
    // undamped. Measure by differentiating: an HF-rich signal has
    // high sample-to-sample differences.
    let mut bright = Impulse::new()
        .freeverb()
        .room_size(0.8)
        .damping(0.0)
        .wet(1.0);
    let mut dark = Impulse::new()
        .freeverb()
        .room_size(0.8)
        .damping(1.0)
        .wet(1.0);
    let bright_buf = render_to_buffer(&mut bright, 0.5, SR);
    let dark_buf = render_to_buffer(&mut dark, 0.5, SR);

    let bright_diff: f32 = bright_buf.windows(2).map(|w| (w[1] - w[0]).abs()).sum::<f32>();
    let dark_diff: f32 = dark_buf.windows(2).map(|w| (w[1] - w[0]).abs()).sum::<f32>();

    assert!(
        dark_diff < bright_diff,
        "damping should reduce HF content: bright diff={bright_diff}, dark diff={dark_diff}"
    );
}

// ─────────────── Stereo behaviour ───────────────

#[test]
fn width_1_gives_differing_channels() {
    // width=1 → L and R should differ (stereo spread).
    let mut sig = Impulse::new().freeverb().width(1.0).wet(1.0).room_size(0.7);
    // Run for long enough that the tail is populated.
    for _ in 0..500 {
        sig.next(&ctx(0));
    }
    // Accumulate L/R differences.
    let mut diff_sum = 0.0_f32;
    for tick in 500..2000 {
        let (l, r) = sig.next_stereo(&ctx(tick));
        diff_sum += (l - r).abs();
    }
    assert!(
        diff_sum > 0.01,
        "width=1 should produce L/R differences, got sum={diff_sum}"
    );
}

#[test]
fn width_0_gives_mono_reverb() {
    // width=0 → both channels identical.
    let mut sig = Impulse::new().freeverb().width(0.0).wet(1.0).room_size(0.7);
    for _ in 0..500 {
        sig.next(&ctx(0));
    }
    let (l, r) = sig.next_stereo(&ctx(500));
    assert!((l - r).abs() < 1e-5, "width=0 should give L==R, got L={l} R={r}");
}

// ─────────────── Bounded output ───────────────

#[test]
fn output_stays_bounded_under_continuous_input() {
    // Feed a continuous sine at modest amplitude; output should stay
    // bounded even at high room size + wet.
    let mut sig = osc::sine(440.0)
        .amp(0.5)
        .freeverb()
        .room_size(0.95)
        .damping(0.3)
        .wet(0.7);
    let buf = render_to_buffer(&mut sig, 2.0, SR);
    let peak = buf.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
    assert!(peak < 5.0, "reverb output should stay bounded, peak={peak}");
    assert!(peak > 0.1, "should produce audible output, peak={peak}");
    for (i, &s) in buf.iter().enumerate() {
        assert!(s.is_finite(), "non-finite sample at {i}: {s}");
    }
}

// ─────────────── Parameter clamping ───────────────

#[test]
fn extreme_room_size_clamps() {
    let mut sig = Impulse::new().freeverb().room_size(5.0).wet(1.0);
    let buf = render_to_buffer(&mut sig, 1.0, SR);
    let peak = buf.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
    // Must stay bounded — room_size > 1 clamps to 1.
    assert!(peak < 10.0, "room_size=5 should clamp, peak={peak}");
}

// ─────────────── Sample rate adaptation ───────────────

#[test]
fn works_at_48khz() {
    let ctx48 = |tick: u64| AudioContext {
        sample_rate: 48000.0,
        tick,
    };
    let mut sig = Impulse::new().freeverb().room_size(0.7).wet(1.0);
    let mut got_nonzero = false;
    for tick in 0..20000 {
        let v = sig.next(&ctx48(tick));
        if v.abs() > 0.001 {
            got_nonzero = true;
        }
        assert!(v.is_finite());
    }
    assert!(got_nonzero, "48 kHz reverb should produce output");
}

// ─────────────── No-alloc ───────────────

#[test]
fn freeverb_does_not_allocate_per_sample() {
    let mut sig = osc::sine(440.0).freeverb().room_size(0.7).wet(0.3);
    let c = ctx(0);
    // Prime the init path.
    for _ in 0..10 {
        sig.next_stereo(&c);
    }
    let _guard = DenyAllocGuard::new();
    for _ in 0..4096 {
        sig.next_stereo(&c);
    }
}
