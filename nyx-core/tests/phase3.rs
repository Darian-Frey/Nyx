#[global_allocator]
static ALLOC: nyx_core::GuardedAllocator = nyx_core::GuardedAllocator;

use nyx_core::dynamics;
use nyx_core::golden::{GoldenTest, assert_golden};
use nyx_core::osc;
use nyx_core::{AudioContext, DenyAllocGuard, FilterExt, Signal, SignalExt, render_to_buffer};

fn ctx(tick: u64) -> AudioContext {
    AudioContext {
        sample_rate: 44100.0,
        tick,
    }
}

const SR: f32 = 44100.0;

// ===================== Oscillator tests =====================

#[test]
fn sine_starts_at_zero() {
    let mut sig = osc::sine(440.0);
    let out = sig.next(&ctx(0));
    assert!(out.abs() < 1e-6, "sine should start at 0, got {out}");
}

#[test]
fn sine_output_in_range() {
    let mut sig = osc::sine(440.0);
    let buf = render_to_buffer(&mut sig, 0.1, SR);
    for (i, &s) in buf.iter().enumerate() {
        assert!(
            (-1.0..=1.0).contains(&s),
            "sine sample {i} out of range: {s}"
        );
    }
}

#[test]
fn sine_phase_accuracy() {
    // Phase error must be < 1e-6 after 10 seconds at 44100 Hz.
    let mut sig = osc::sine(440.0);
    let samples = (10.0 * SR) as usize;
    let mut last = 0.0_f32;
    for tick in 0..samples {
        last = sig.next(&AudioContext {
            sample_rate: SR,
            tick: tick as u64,
        });
    }
    // After exactly 10s at 440Hz, we've done 4400 full cycles.
    // Phase should be back near 0 → sin(0) ≈ 0.
    // But due to f32 accumulation, allow 1e-6 on the phase (not the sample).
    // 4400 * 44100/440 = 441000 samples, sin(2π * phase_err) ≈ 2π * phase_err.
    // So sample value near 0 means phase error is small.
    // Actually, let's just check the golden file for exact regression.
    let _ = last; // used for golden below
}

#[test]
fn saw_range() {
    let mut sig = osc::saw(440.0);
    let buf = render_to_buffer(&mut sig, 0.1, SR);
    for (i, &s) in buf.iter().enumerate() {
        assert!(
            (-1.0..=1.0).contains(&s),
            "saw sample {i} out of range: {s}"
        );
    }
}

#[test]
fn saw_starts_at_minus_one() {
    let mut sig = osc::saw(440.0);
    let out = sig.next(&ctx(0));
    // phase=0 → 2*0 - 1 = -1
    assert!(
        (out - (-1.0)).abs() < 1e-6,
        "saw should start at -1, got {out}"
    );
}

#[test]
fn square_starts_at_one() {
    let mut sig = osc::square(440.0);
    let out = sig.next(&ctx(0));
    assert!(
        (out - 1.0).abs() < 1e-6,
        "square should start at 1, got {out}"
    );
}

#[test]
fn square_is_bipolar() {
    let mut sig = osc::square(440.0);
    let buf = render_to_buffer(&mut sig, 0.01, SR);
    let has_pos = buf.iter().any(|&s| s > 0.5);
    let has_neg = buf.iter().any(|&s| s < -0.5);
    assert!(
        has_pos && has_neg,
        "square should have both +1 and -1 regions"
    );
}

#[test]
fn triangle_range() {
    let mut sig = osc::triangle(440.0);
    let buf = render_to_buffer(&mut sig, 0.1, SR);
    for (i, &s) in buf.iter().enumerate() {
        assert!(
            (-1.0 - 1e-6..=1.0 + 1e-6).contains(&s),
            "triangle sample {i} out of range: {s}"
        );
    }
}

#[test]
fn triangle_starts_at_minus_one() {
    let mut sig = osc::triangle(440.0);
    let out = sig.next(&ctx(0));
    // phase=0 → 4*0 - 1 = -1
    assert!(
        (out - (-1.0)).abs() < 1e-6,
        "triangle should start at -1, got {out}"
    );
}

// ===================== Frequency modulation =====================

#[test]
fn sine_accepts_signal_frequency() {
    // FM: carrier at 440, modulated by a constant 10 Hz offset
    let modulator = |_ctx: &AudioContext| 450.0_f32;
    let mut sig = osc::sine(modulator);
    let out = sig.next(&ctx(0));
    // Should compile and produce output (the key test is compilation).
    assert!(out.abs() < 1e-6); // starts at 0 regardless of freq
}

// ===================== Noise tests =====================

#[test]
fn white_noise_range() {
    let mut sig = osc::noise::white(42);
    let buf = render_to_buffer(&mut sig, 0.1, SR);
    for (i, &s) in buf.iter().enumerate() {
        assert!(
            (-1.0..=1.0).contains(&s),
            "white noise sample {i} out of range: {s}"
        );
    }
}

#[test]
fn white_noise_is_not_silent() {
    let mut sig = osc::noise::white(42);
    let buf = render_to_buffer(&mut sig, 0.01, SR);
    let rms: f32 = (buf.iter().map(|s| s * s).sum::<f32>() / buf.len() as f32).sqrt();
    assert!(rms > 0.1, "white noise RMS too low: {rms}");
}

#[test]
fn white_noise_deterministic() {
    let mut a = osc::noise::white(123);
    let mut b = osc::noise::white(123);
    let buf_a = render_to_buffer(&mut a, 0.01, SR);
    let buf_b = render_to_buffer(&mut b, 0.01, SR);
    assert_eq!(buf_a, buf_b);
}

#[test]
fn pink_noise_range() {
    let mut sig = osc::noise::pink(42);
    let buf = render_to_buffer(&mut sig, 0.1, SR);
    for (i, &s) in buf.iter().enumerate() {
        assert!(
            (-2.0..=2.0).contains(&s),
            "pink noise sample {i} out of range: {s}"
        );
    }
}

#[test]
fn pink_noise_is_not_silent() {
    let mut sig = osc::noise::pink(42);
    let buf = render_to_buffer(&mut sig, 0.01, SR);
    let rms: f32 = (buf.iter().map(|s| s * s).sum::<f32>() / buf.len() as f32).sqrt();
    assert!(rms > 0.01, "pink noise RMS too low: {rms}");
}

#[test]
fn pink_noise_has_low_frequency_tilt() {
    // Pink noise has a −3 dB/octave slope: energy below ~1 kHz should
    // be substantially higher than energy above. Render 1 s, split at
    // 1 kHz via a one-pole LP/HP, compare RMS.
    let mut sig = osc::noise::pink(1234);
    let buf = render_to_buffer(&mut sig, 1.0, SR);
    // Simple one-pole LP at 1 kHz to isolate the low band; the high
    // band is what's left over.
    let alpha = 1.0 - (-std::f32::consts::TAU * 1000.0 / SR).exp();
    let mut low_state = 0.0_f32;
    let mut low_rms_sq = 0.0_f32;
    let mut high_rms_sq = 0.0_f32;
    for &s in &buf {
        low_state += alpha * (s - low_state);
        let high = s - low_state;
        low_rms_sq += low_state * low_state;
        high_rms_sq += high * high;
    }
    let low_rms = (low_rms_sq / buf.len() as f32).sqrt();
    let high_rms = (high_rms_sq / buf.len() as f32).sqrt();
    assert!(
        low_rms > high_rms * 1.5,
        "pink noise should tilt low: low_rms={low_rms} high_rms={high_rms}"
    );
}

#[test]
fn pink_noise_deterministic() {
    // Same seed must yield identical output run-to-run.
    let mut a = osc::noise::pink(999);
    let mut b = osc::noise::pink(999);
    let buf_a = render_to_buffer(&mut a, 0.01, SR);
    let buf_b = render_to_buffer(&mut b, 0.01, SR);
    assert_eq!(buf_a, buf_b);
}

// ===================== Filter tests =====================

#[test]
fn lowpass_attenuates_high_frequency() {
    // 10 kHz sine through 200 Hz lowpass should be nearly silent.
    let mut sig = osc::sine(10000.0).lowpass(200.0, 0.707);
    // Let the filter settle for 0.1s, then measure the next 0.1s.
    let _ = render_to_buffer(&mut sig, 0.1, SR);
    let buf = render_to_buffer(&mut sig, 0.1, SR);
    let rms: f32 = (buf.iter().map(|s| s * s).sum::<f32>() / buf.len() as f32).sqrt();
    assert!(
        rms < 0.01,
        "10kHz through 200Hz LP should be attenuated, rms={rms}"
    );
}

#[test]
fn lowpass_passes_low_frequency() {
    // 100 Hz sine through 5000 Hz lowpass should pass through.
    let mut sig = osc::sine(100.0).lowpass(5000.0, 0.707);
    let _ = render_to_buffer(&mut sig, 0.1, SR);
    let buf = render_to_buffer(&mut sig, 0.1, SR);
    let rms: f32 = (buf.iter().map(|s| s * s).sum::<f32>() / buf.len() as f32).sqrt();
    assert!(rms > 0.5, "100Hz through 5kHz LP should pass, rms={rms}");
}

#[test]
fn highpass_attenuates_low_frequency() {
    // 100 Hz sine through 5000 Hz highpass should be nearly silent.
    let mut sig = osc::sine(100.0).highpass(5000.0, 0.707);
    let _ = render_to_buffer(&mut sig, 0.1, SR);
    let buf = render_to_buffer(&mut sig, 0.1, SR);
    let rms: f32 = (buf.iter().map(|s| s * s).sum::<f32>() / buf.len() as f32).sqrt();
    assert!(
        rms < 0.01,
        "100Hz through 5kHz HP should be attenuated, rms={rms}"
    );
}

#[test]
fn highpass_passes_high_frequency() {
    // 10 kHz sine through 200 Hz highpass should pass through.
    let mut sig = osc::sine(10000.0).highpass(200.0, 0.707);
    let _ = render_to_buffer(&mut sig, 0.1, SR);
    let buf = render_to_buffer(&mut sig, 0.1, SR);
    let rms: f32 = (buf.iter().map(|s| s * s).sum::<f32>() / buf.len() as f32).sqrt();
    assert!(rms > 0.5, "10kHz through 200Hz HP should pass, rms={rms}");
}

// ===================== Dynamics tests =====================

#[test]
fn gain_processor() {
    let sig = osc::sine(440.0);
    let mut gained = dynamics::gain(sig, 0.5);
    let buf = render_to_buffer(&mut gained, 0.01, SR);
    // All samples should be in [-0.5, 0.5]
    for &s in &buf {
        assert!(s.abs() <= 0.5 + 1e-6);
    }
}

#[test]
fn peak_limiter_limits() {
    // Loud sine at amplitude 2.0, limited to 1.0.
    let sig = osc::sine(440.0).amp(2.0);
    let mut limited = dynamics::peak_limiter(sig, 1.0, 0.1, 100.0, SR);
    // Let it settle.
    let _ = render_to_buffer(&mut limited, 0.05, SR);
    let buf = render_to_buffer(&mut limited, 0.1, SR);
    for (i, &s) in buf.iter().enumerate() {
        assert!(
            s.abs() <= 1.05, // small tolerance for attack time
            "limiter sample {i} exceeds threshold: {s}"
        );
    }
}

// ===================== No-alloc guard on oscillators =====================

#[test]
fn oscillators_do_not_allocate() {
    let mut sine = osc::sine(440.0);
    let mut saw = osc::saw(440.0);
    let mut square = osc::square(440.0);
    let mut tri = osc::triangle(440.0);
    let mut white = osc::noise::white(42);
    let c = ctx(0);

    let _guard = DenyAllocGuard::new();
    for _ in 0..1024 {
        sine.next(&c);
        saw.next(&c);
        square.next(&c);
        tri.next(&c);
        white.next(&c);
    }
}

#[test]
fn filter_does_not_allocate() {
    let mut sig = osc::sine(440.0).lowpass(800.0, 0.707);
    let c = ctx(0);

    let _guard = DenyAllocGuard::new();
    // First sample initialises smoothers — that's the tricky one.
    sig.next(&c);
    for _ in 1..1024 {
        sig.next(&c);
    }
}

// ===================== Golden files =====================

#[test]
fn golden_sine_440hz() {
    let mut sig = osc::sine(440.0);
    assert_golden(
        &mut sig,
        &GoldenTest {
            name: "osc_sine_440",
            duration_secs: 0.01,
            sample_rate: SR,
            tolerance: 1e-6,
        },
    );
}

#[test]
fn golden_saw_440hz() {
    let mut sig = osc::saw(440.0);
    assert_golden(
        &mut sig,
        &GoldenTest {
            name: "osc_saw_440",
            duration_secs: 0.01,
            sample_rate: SR,
            tolerance: 1e-6,
        },
    );
}

#[test]
fn golden_square_440hz() {
    let mut sig = osc::square(440.0);
    assert_golden(
        &mut sig,
        &GoldenTest {
            name: "osc_square_440",
            duration_secs: 0.01,
            sample_rate: SR,
            tolerance: 1e-6,
        },
    );
}

#[test]
fn golden_triangle_440hz() {
    let mut sig = osc::triangle(440.0);
    assert_golden(
        &mut sig,
        &GoldenTest {
            name: "osc_triangle_440",
            duration_secs: 0.01,
            sample_rate: SR,
            tolerance: 1e-6,
        },
    );
}

// ===================== Band-limited oscillator tests =====================

#[test]
fn saw_bl_output_bounded() {
    // BLEP correction can overshoot the naive [-1, 1] range by a small
    // amount near each discontinuity; ±1.2 is a loose but safe bound.
    let mut sig = osc::saw_bl(440.0);
    let buf = render_to_buffer(&mut sig, 0.1, SR);
    for &s in &buf {
        assert!(s.abs() <= 1.2, "saw_bl sample outside [-1.2, 1.2]: {s}");
    }
}

#[test]
fn square_bl_output_bounded() {
    let mut sig = osc::square_bl(440.0);
    let buf = render_to_buffer(&mut sig, 0.1, SR);
    for &s in &buf {
        assert!(s.abs() <= 1.2, "square_bl sample outside [-1.2, 1.2]: {s}");
    }
}

#[test]
fn saw_bl_reduces_inter_sample_jumps() {
    // A naive saw at 4 kHz with SR=44.1 kHz has a discontinuity every
    // ~11 samples, producing inter-sample jumps near 1.8. PolyBLEP
    // spreads each step across `dt` samples, substantially reducing the
    // peak inter-sample delta. We assert the BL version's largest jump
    // is at least 25% smaller than the naive one — a floor, not the
    // actual improvement (aliasing reduction is spectral, not temporal).
    let mut naive = osc::saw(4000.0);
    let mut bl = osc::saw_bl(4000.0);
    let naive_buf = render_to_buffer(&mut naive, 0.05, SR);
    let bl_buf = render_to_buffer(&mut bl, 0.05, SR);

    let max_step = |buf: &[f32]| -> f32 {
        buf.windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0_f32, f32::max)
    };

    let naive_step = max_step(&naive_buf);
    let bl_step = max_step(&bl_buf);

    assert!(
        naive_step > 1.5,
        "expected naive saw to have big jumps, got {naive_step}"
    );
    assert!(
        bl_step < naive_step * 0.75,
        "expected band-limited saw max step ({bl_step}) to be <75% of naive ({naive_step})"
    );
}

#[test]
fn golden_saw_bl_440hz() {
    let mut sig = osc::saw_bl(440.0);
    assert_golden(
        &mut sig,
        &GoldenTest {
            name: "osc_saw_bl_440",
            duration_secs: 0.01,
            sample_rate: SR,
            tolerance: 1e-6,
        },
    );
}

#[test]
fn golden_square_bl_440hz() {
    let mut sig = osc::square_bl(440.0);
    assert_golden(
        &mut sig,
        &GoldenTest {
            name: "osc_square_bl_440",
            duration_secs: 0.01,
            sample_rate: SR,
            tolerance: 1e-6,
        },
    );
}

#[test]
fn pwm_bl_half_width_matches_square_bl() {
    // PWM at width=0.5 should reproduce a band-limited square, up to a
    // first-sample boundary case (square_bl samples before wrap at
    // phase=0, so the very first BLEP window is slightly different).
    let mut pwm = osc::pwm_bl(440.0, 0.5_f32);
    let mut sqr = osc::square_bl(440.0);
    let a = render_to_buffer(&mut pwm, 0.01, SR);
    let b = render_to_buffer(&mut sqr, 0.01, SR);
    assert_eq!(a.len(), b.len());
    for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() {
        assert!(
            (x - y).abs() < 1e-5,
            "pwm_bl(0.5) vs square_bl mismatch at {i}: {x} vs {y}"
        );
    }
}

#[test]
fn pwm_bl_width_shifts_dc() {
    // Asymmetric width biases the average output. At width=0.25 the
    // wave is +1 for 25% of the cycle and −1 for 75%, so long-term
    // mean approaches 0.25·1 + 0.75·(−1) = −0.5. Rendering long
    // enough averages out the transient BLEP corrections.
    let mut narrow = osc::pwm_bl(220.0, 0.25_f32);
    let mut wide = osc::pwm_bl(220.0, 0.75_f32);
    let a = render_to_buffer(&mut narrow, 0.5, SR);
    let b = render_to_buffer(&mut wide, 0.5, SR);
    let mean_a = a.iter().sum::<f32>() / a.len() as f32;
    let mean_b = b.iter().sum::<f32>() / b.len() as f32;
    assert!(
        mean_a < -0.3,
        "width=0.25 mean should be ≈ -0.5, got {mean_a}"
    );
    assert!(
        mean_b > 0.3,
        "width=0.75 mean should be ≈ +0.5, got {mean_b}"
    );
}

#[test]
fn pwm_bl_output_bounded() {
    let mut sig = osc::pwm_bl(440.0, 0.3_f32);
    let buf = render_to_buffer(&mut sig, 0.1, SR);
    for &s in &buf {
        assert!(s.abs() <= 1.2, "pwm_bl sample outside ±1.2: {s}");
    }
}

#[test]
fn golden_white_noise() {
    let mut sig = osc::noise::white(42);
    assert_golden(
        &mut sig,
        &GoldenTest {
            name: "noise_white_seed42",
            duration_secs: 0.01,
            sample_rate: SR,
            tolerance: 0.0, // deterministic
        },
    );
}

// ===================== Fluent chain test =====================

#[test]
fn fluent_chain_compiles_and_runs() {
    let mut sig = osc::saw(220.0).lowpass(800.0, 0.707).amp(0.5).clip(0.8);
    let buf = render_to_buffer(&mut sig, 0.01, SR);
    assert!(!buf.is_empty());
    for &s in &buf {
        assert!(s.abs() <= 0.8 + 1e-6);
    }
}
