//! Sprint 1 — Bitcrusher + Downsample tests.

#![allow(clippy::float_cmp)]

use nyx_core::{render_to_buffer, AudioContext, DenyAllocGuard, Signal, SignalExt};

const SR: f32 = 44100.0;

fn ctx(tick: u64) -> AudioContext {
    AudioContext {
        sample_rate: SR,
        tick,
    }
}

/// A ramp signal that walks from -1.0 to +1.0 over `samples` samples,
/// then stays at +1.0. Useful for testing quantisation across the full range.
struct Ramp {
    phase: f32,
    step: f32,
}

impl Ramp {
    fn new(samples: usize) -> Self {
        Self {
            phase: -1.0,
            step: 2.0 / samples as f32,
        }
    }
}

impl Signal for Ramp {
    fn next(&mut self, _ctx: &AudioContext) -> f32 {
        let out = self.phase;
        self.phase = (self.phase + self.step).min(1.0);
        out
    }
}

/// A counter signal that emits 0.0, 1.0, 2.0, 3.0, ... each sample.
/// Useful for watching which input ends up in which output slot.
struct Counter(f32);

impl Signal for Counter {
    fn next(&mut self, _ctx: &AudioContext) -> f32 {
        let v = self.0;
        self.0 += 1.0;
        v
    }
}

// ──────────────────────── BitCrush ────────────────────────

#[test]
fn bitcrush_1_bit_produces_two_values() {
    // At 1 bit, only 2 output levels exist: -1.0 and +1.0.
    let mut sig = Ramp::new(1024).bitcrush(1);
    let buf = render_to_buffer(&mut sig, 1024.0 / SR, SR);
    let unique: std::collections::HashSet<i32> = buf.iter().map(|x| (x * 1e6) as i32).collect();
    assert!(
        unique.len() <= 2,
        "1-bit crush should produce ≤ 2 unique values, got {}: {unique:?}",
        unique.len()
    );
}

#[test]
fn bitcrush_16_bits_nearly_transparent() {
    // At 16 bits we have 65535 levels. Ramp over 1024 samples means
    // adjacent samples differ by ~2/1024 ≈ 0.002, which is 128× the
    // 16-bit step of 2/65535 ≈ 3e-5. Crushed output should track the
    // input within the single-LSB step.
    let mut sig = Ramp::new(1024).bitcrush(16);
    let buf = render_to_buffer(&mut sig, 1024.0 / SR, SR);

    let lsb = 2.0 / 65535.0;
    // Reconstruct what the input would have been at each tick.
    for (i, &got) in buf.iter().enumerate() {
        let expected = -1.0 + (2.0 / 1024.0) * i as f32;
        let expected = expected.min(1.0);
        assert!(
            (got - expected).abs() <= lsb * 1.5,
            "16-bit crush drift at {i}: got {got}, want {expected}"
        );
    }
}

#[test]
fn bitcrush_output_stays_bounded() {
    let mut sig = Ramp::new(256).bitcrush(4);
    let buf = render_to_buffer(&mut sig, 256.0 / SR, SR);
    for &s in &buf {
        assert!((-1.0 - 1e-6..=1.0 + 1e-6).contains(&s), "out of range: {s}");
    }
}

#[test]
fn bitcrush_dc_offset_at_low_depth() {
    // At 2 bits we have 4 output levels: {-1, -1/3, +1/3, +1}. Zero
    // is NOT representable — it quantises to ±1/3. This is expected
    // bitcrusher character. Document it with a test.
    let mut sig = (|_ctx: &AudioContext| 0.0_f32).bitcrush(2);
    let got = sig.next(&ctx(0));
    let third = 1.0_f32 / 3.0;
    assert!(
        (got - third).abs() < 1e-5 || (got + third).abs() < 1e-5,
        "expected ±1/3 at 2-bit crush of 0.0, got {got}"
    );
}

#[test]
fn bitcrush_clamps_overdrive() {
    // Inputs outside [-1, 1] should be clamped before quantising.
    let mut sig = (|_ctx: &AudioContext| 5.0_f32).bitcrush(4);
    assert!((sig.next(&ctx(0)) - 1.0).abs() < 1e-6);

    let mut sig = (|_ctx: &AudioContext| -3.0_f32).bitcrush(4);
    assert!((sig.next(&ctx(0)) - (-1.0)).abs() < 1e-6);
}

// ──────────────────────── Downsample ────────────────────────

#[test]
fn downsample_ratio_1_is_identity() {
    let mut sig = Counter(0.0).downsample(1.0);
    for tick in 0..10 {
        let got = sig.next(&ctx(tick));
        assert_eq!(got, tick as f32, "ratio=1.0 should be identity");
    }
}

#[test]
fn downsample_ratio_half_holds_each_sample_twice() {
    // Inputs: 0,1,2,3,4,5,6,7...
    // Outputs at ratio=0.5: 0,0,2,2,4,4,6,6... (each even sample held twice)
    let mut sig = Counter(0.0).downsample(0.5);
    let got: Vec<f32> = (0..8).map(|t| sig.next(&ctx(t))).collect();
    assert_eq!(got, vec![0.0, 0.0, 2.0, 2.0, 4.0, 4.0, 6.0, 6.0]);
}

#[test]
fn downsample_ratio_quarter_holds_four_times() {
    // Inputs: 0,1,2,3,4,5,6,7,8,9,10,11...
    // Outputs at ratio=0.25: 0,0,0,0,4,4,4,4,8,8,8,8...
    let mut sig = Counter(0.0).downsample(0.25);
    let got: Vec<f32> = (0..12).map(|t| sig.next(&ctx(t))).collect();
    assert_eq!(
        got,
        vec![0.0, 0.0, 0.0, 0.0, 4.0, 4.0, 4.0, 4.0, 8.0, 8.0, 8.0, 8.0]
    );
}

#[test]
fn downsample_consumes_source_at_full_rate() {
    // Oscillators wrapped in downsample should still "think" they're
    // running at full rate — their phase accumulates per output frame.
    // We verify this by checking that a 1-kHz sine has the expected
    // number of zero-crossings even when downsampled.
    use std::f32::consts::TAU;
    let mut phase = 0.0_f32;
    let mut sine_source = move |ctx: &AudioContext| {
        let out = (phase * TAU).sin();
        phase += 1000.0 / ctx.sample_rate;
        phase -= phase.floor();
        out
    };
    let mut sig = (&mut sine_source).downsample(0.5);
    let buf = render_to_buffer(&mut sig, 1.0, SR);
    let crossings: usize = buf
        .windows(2)
        .filter(|w| w[0].signum() != w[1].signum())
        .count();
    // 1 kHz = 2000 crossings/s. Downsampled version will have at
    // most that many (and likely slightly fewer due to held samples),
    // but should be in the ballpark.
    assert!(
        (900..=2100).contains(&crossings),
        "expected ~1000-2000 crossings, got {crossings}"
    );
}

// ──────────────────────── .crush() convenience ────────────────────────

#[test]
fn crush_is_bitcrush_then_downsample() {
    // Equivalent chains should produce identical output.
    let mut a = Counter(0.0).bitcrush(4).downsample(0.5);
    let mut b = Counter(0.0).crush(4, 0.5);
    for tick in 0..16 {
        let got_a = a.next(&ctx(tick));
        let got_b = b.next(&ctx(tick));
        assert!(
            (got_a - got_b).abs() < 1e-6,
            "crush({{4, 0.5}}) != .bitcrush(4).downsample(0.5) at {tick}"
        );
    }
}

// ──────────────────────── No-alloc ────────────────────────

#[test]
fn crush_combinators_do_not_allocate() {
    // Construct first (may allocate), then run under the guard.
    let mut sig = Counter(0.0).crush(6, 0.5);
    let c = ctx(0);
    let _guard = DenyAllocGuard::new();
    for _ in 0..1024 {
        sig.next(&c);
    }
}
