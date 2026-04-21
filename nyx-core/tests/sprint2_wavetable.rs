//! Sprint 2 — Wavetable oscillator tests.

use nyx_core::{AudioContext, DenyAllocGuard, Signal, Wavetable, render_to_buffer};

const SR: f32 = 44100.0;

fn ctx(tick: u64) -> AudioContext {
    AudioContext {
        sample_rate: SR,
        tick,
    }
}

fn zero_crossings(buf: &[f32]) -> usize {
    buf.windows(2)
        .filter(|w| w[0].signum() != w[1].signum())
        .count()
}

// ─────────────── Construction ───────────────

#[test]
fn from_vec_preserves_data() {
    let data = vec![-1.0_f32, -0.5, 0.0, 0.5, 1.0];
    let wt = Wavetable::from_vec(data);
    assert_eq!(wt.len(), 5);
    assert!(!wt.is_empty());
}

#[test]
fn from_slice_copies_data() {
    let data = [0.0_f32, 0.5, 1.0, 0.5];
    let wt = Wavetable::new(&data);
    assert_eq!(wt.len(), 4);
}

#[test]
fn from_fn_evaluates_closure() {
    let wt = Wavetable::from_fn(4, |t| t);
    // t values at 0, 0.25, 0.5, 0.75
    assert_eq!(wt.len(), 4);
}

#[test]
#[should_panic(expected = "at least one sample")]
fn empty_slice_panics() {
    let _ = Wavetable::new(&[]);
}

#[test]
#[should_panic(expected = "size must be > 0")]
fn zero_size_from_fn_panics() {
    let _ = Wavetable::from_fn(0, |_| 0.0);
}

// ─────────────── Preset tables ───────────────

#[test]
fn sine_preset_produces_sine_at_target_freq() {
    // 440 Hz sine for 0.1 s → ~88 zero crossings.
    let table = Wavetable::sine(2048);
    let mut osc = table.freq(440.0);
    let buf = render_to_buffer(&mut osc, 0.1, SR);

    let zc = zero_crossings(&buf);
    assert!(
        (zc as i32 - 88).abs() <= 4,
        "440 Hz wavetable sine: expected ~88 crossings, got {zc}"
    );

    // Peak amplitude should be close to ±1.0.
    let peak = buf.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
    assert!(peak > 0.9 && peak <= 1.0, "sine peak = {peak}");
}

#[test]
fn saw_preset_is_bipolar() {
    let table = Wavetable::saw(2048);
    let mut osc = table.freq(100.0);
    let buf = render_to_buffer(&mut osc, 0.05, SR);
    // Saw should span both positive and negative.
    let min = buf.iter().cloned().fold(f32::INFINITY, f32::min);
    let max = buf.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    assert!(min < -0.8 && max > 0.8, "saw range too small: {min}..{max}");
}

#[test]
fn square_preset_is_bipolar() {
    let table = Wavetable::square(2048);
    let mut osc = table.freq(100.0);
    let buf = render_to_buffer(&mut osc, 0.05, SR);
    let min = buf.iter().cloned().fold(f32::INFINITY, f32::min);
    let max = buf.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    assert!((min + 1.0).abs() < 0.1, "square min not -1: {min}");
    assert!((max - 1.0).abs() < 0.1, "square max not 1: {max}");
}

#[test]
fn triangle_preset_ranges_fully() {
    let table = Wavetable::triangle(2048);
    let mut osc = table.freq(100.0);
    let buf = render_to_buffer(&mut osc, 0.05, SR);
    let min = buf.iter().cloned().fold(f32::INFINITY, f32::min);
    let max = buf.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    assert!(min < -0.95 && max > 0.95);
}

// ─────────────── Frequency accuracy ───────────────

#[test]
fn higher_freq_more_crossings() {
    let table = Wavetable::sine(2048);
    let mut low = table.freq(110.0);
    let mut high = table.freq(880.0);
    let low_buf = render_to_buffer(&mut low, 0.1, SR);
    let high_buf = render_to_buffer(&mut high, 0.1, SR);
    let low_zc = zero_crossings(&low_buf);
    let high_zc = zero_crossings(&high_buf);
    assert!(
        high_zc > low_zc * 7,
        "880 Hz should have ~8× the crossings of 110 Hz: low={low_zc}, high={high_zc}"
    );
}

// ─────────────── Modulation ───────────────

#[test]
fn frequency_accepts_signal() {
    use nyx_core::osc;
    let table = Wavetable::sine(2048);
    // Vibrato: 5 Hz LFO ±10 Hz around 440 Hz.
    let lfo = osc::sine(5.0);
    use nyx_core::SignalExt;
    let vibrato_freq = lfo.amp(10.0).offset(440.0);
    let mut osc = table.freq(vibrato_freq);
    let buf = render_to_buffer(&mut osc, 0.1, SR);
    // Just verify it compiles and produces output.
    let rms = (buf.iter().map(|s| s * s).sum::<f32>() / buf.len() as f32).sqrt();
    assert!(rms > 0.5);
}

// ─────────────── Sharing ───────────────

#[test]
fn wavetable_clone_is_cheap() {
    let table = Wavetable::sine(4096);
    let a = table.clone();
    let b = table.clone();
    // Three Arc refs now — all should work equivalently.
    let mut osc_a = a.freq(440.0);
    let mut osc_b = b.freq(440.0);
    for tick in 0..100 {
        let va = osc_a.next(&ctx(tick));
        let vb = osc_b.next(&ctx(tick));
        assert!((va - vb).abs() < 1e-6);
    }
}

// ─────────────── Custom waveform ───────────────

#[test]
fn from_fn_custom_shape() {
    // Squared sine: f(t) = sin²(2πt)
    let table = Wavetable::from_fn(4096, |t| (t * std::f32::consts::TAU).sin().powi(2));
    let mut osc = table.freq(100.0);
    let buf = render_to_buffer(&mut osc, 0.05, SR);
    // sin² is always ≥ 0 (DC-positive).
    for &s in &buf {
        assert!(s >= -0.01, "squared sine should stay positive: {s}");
    }
}

// ─────────────── No-alloc ───────────────

#[test]
fn wavetable_osc_does_not_allocate_per_sample() {
    let table = Wavetable::sine(2048);
    let mut osc = table.freq(440.0);
    let c = ctx(0);
    for _ in 0..10 {
        osc.next(&c);
    }
    let _guard = DenyAllocGuard::new();
    for _ in 0..4096 {
        osc.next(&c);
    }
}

// ─────────────── Chaining ───────────────

#[test]
fn wavetable_chains_with_filter_and_amp() {
    use nyx_core::{FilterExt, SignalExt};
    let table = Wavetable::saw(2048);
    let mut sig = table.freq(220.0).svf_lp(1000.0, 2.0).amp(0.5);
    let buf = render_to_buffer(&mut sig, 0.1, SR);
    for &s in &buf {
        assert!(s.is_finite() && s.abs() <= 1.0);
    }
}
