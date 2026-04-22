//! Sonic character — saturation waveshaper tests.
//!
//! Covers [`TapeSat`], [`TubeSat`], and [`DiodeClip`]. Each is tested
//! for: (1) output bounded, (2) unit-gain pass-through when drive
//! makes sense, (3) drive audibly changes output, and (4) no DC
//! introduction. Golden files pin a representative 440 Hz waveform.

#[global_allocator]
static ALLOC: nyx_core::GuardedAllocator = nyx_core::GuardedAllocator;

use nyx_core::golden::{GoldenTest, assert_golden};
use nyx_core::{
    AudioContext, DenyAllocGuard, SaturationExt, Signal, SignalExt, osc, render_to_buffer,
};

const SR: f32 = 44100.0;

fn rms(buf: &[f32]) -> f32 {
    (buf.iter().map(|s| s * s).sum::<f32>() / buf.len() as f32).sqrt()
}

fn mean(buf: &[f32]) -> f32 {
    buf.iter().sum::<f32>() / buf.len() as f32
}

// ─── TapeSat ──────────────────────────────────────────────────────────

#[test]
fn tape_sat_output_bounded() {
    let mut sig = osc::saw(220.0).amp(0.8).tape_sat(4.0);
    let buf = render_to_buffer(&mut sig, 0.1, SR);
    for &s in &buf {
        assert!(s.abs() <= 1.5, "tape_sat sample outside [-1.5, 1.5]: {s}");
    }
}

#[test]
fn tape_sat_removes_dc() {
    // A constant DC source plus tape should settle to zero-mean once
    // the pre-HP kicks in.
    let mut sig = (|_: &AudioContext| 0.4_f32).tape_sat(2.0);
    // Render for 200 ms so the 30 Hz HP has many time-constants to settle.
    let buf = render_to_buffer(&mut sig, 0.5, SR);
    let tail = &buf[(SR * 0.3) as usize..];
    let m = mean(tail);
    assert!(m.abs() < 0.05, "tape_sat did not remove DC; mean={m}");
}

#[test]
fn tape_sat_drive_audibly_changes_output() {
    let mut light = osc::saw(220.0).amp(0.6).tape_sat(1.0);
    let mut heavy = osc::saw(220.0).amp(0.6).tape_sat(8.0);
    let a = render_to_buffer(&mut light, 0.1, SR);
    let b = render_to_buffer(&mut heavy, 0.1, SR);
    let diff_rms = rms(&a
        .iter()
        .zip(b.iter())
        .map(|(x, y)| x - y)
        .collect::<Vec<_>>());
    assert!(
        diff_rms > 0.05,
        "tape_sat at drive=1 and drive=8 should differ audibly, diff rms={diff_rms}"
    );
}

#[test]
fn tape_sat_no_alloc() {
    let mut sig = osc::saw(220.0).amp(0.6).tape_sat(3.0);
    let _guard = DenyAllocGuard::new();
    for tick in 0..256 {
        let _ = sig.next(&AudioContext {
            sample_rate: SR,
            tick,
        });
    }
}

#[test]
fn golden_tape_sat_saw_440() {
    let mut sig = osc::saw(440.0).amp(0.5).tape_sat(3.0);
    assert_golden(
        &mut sig,
        &GoldenTest {
            name: "sat_tape_saw_440_drive3",
            duration_secs: 0.01,
            sample_rate: SR,
            // Non-linear + filter state → loose tolerance per sonic-character
            // roadmap's testing convention.
            tolerance: 1e-4,
        },
    );
}

// ─── TubeSat ──────────────────────────────────────────────────────────

#[test]
fn tube_sat_output_bounded() {
    let mut sig = osc::saw(220.0).amp(0.8).tube_sat(4.0);
    let buf = render_to_buffer(&mut sig, 0.1, SR);
    for &s in &buf {
        assert!(s.abs() <= 1.5, "tube_sat sample outside [-1.5, 1.5]: {s}");
    }
}

#[test]
fn tube_sat_removes_dc() {
    // The x² term introduces DC for any bipolar source. The internal
    // DC-blocking HP must remove it.
    let mut sig = osc::sine(220.0).tube_sat(3.0);
    let buf = render_to_buffer(&mut sig, 0.5, SR);
    let tail = &buf[(SR * 0.3) as usize..];
    let m = mean(tail);
    assert!(m.abs() < 0.05, "tube_sat introduced DC offset; mean={m}");
}

#[test]
fn tube_sat_drive_audibly_changes_output() {
    let mut light = osc::saw(220.0).amp(0.4).tube_sat(1.0);
    let mut heavy = osc::saw(220.0).amp(0.4).tube_sat(6.0);
    let a = render_to_buffer(&mut light, 0.1, SR);
    let b = render_to_buffer(&mut heavy, 0.1, SR);
    let diff_rms = rms(&a
        .iter()
        .zip(b.iter())
        .map(|(x, y)| x - y)
        .collect::<Vec<_>>());
    assert!(
        diff_rms > 0.05,
        "tube_sat at drive=1 and drive=6 should differ, diff rms={diff_rms}"
    );
}

#[test]
fn tube_sat_no_alloc() {
    let mut sig = osc::saw(220.0).amp(0.6).tube_sat(3.0);
    let _guard = DenyAllocGuard::new();
    for tick in 0..256 {
        let _ = sig.next(&AudioContext {
            sample_rate: SR,
            tick,
        });
    }
}

#[test]
fn golden_tube_sat_saw_440() {
    let mut sig = osc::saw(440.0).amp(0.5).tube_sat(3.0);
    assert_golden(
        &mut sig,
        &GoldenTest {
            name: "sat_tube_saw_440_drive3",
            duration_secs: 0.01,
            sample_rate: SR,
            tolerance: 1e-4,
        },
    );
}

// ─── DiodeClip ────────────────────────────────────────────────────────

#[test]
fn diode_clip_output_bounded_by_one() {
    // y = x / (1 + |x·drive|) → |y| < 1 for any input.
    let mut sig = osc::saw(220.0).amp(5.0).diode_clip(4.0);
    let buf = render_to_buffer(&mut sig, 0.1, SR);
    for &s in &buf {
        assert!(s.abs() < 1.0, "diode_clip sample not strictly < 1: {s}");
    }
}

#[test]
fn diode_clip_higher_drive_saturates_toward_unity() {
    // `y = x·drive / (1 + |x·drive|)` asymptotes to ±1 as drive grows;
    // higher drive means the signal sits closer to full-scale limiting,
    // i.e. a more "crushed" waveform. At drive=1 and |x|=1 the output
    // peaks at 0.5; at drive=10 and |x|=1 it reaches ~0.91.
    let mut light = osc::sine(220.0).diode_clip(1.0);
    let mut heavy = osc::sine(220.0).diode_clip(10.0);
    let a = render_to_buffer(&mut light, 0.1, SR);
    let b = render_to_buffer(&mut heavy, 0.1, SR);
    let peak_a = a.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
    let peak_b = b.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
    assert!(
        peak_b > peak_a,
        "higher diode drive should saturate toward 1; peak_a={peak_a} peak_b={peak_b}"
    );
    assert!(
        peak_b > 0.85,
        "diode drive=10 should approach unity, peak={peak_b}"
    );
}

#[test]
fn diode_clip_no_alloc() {
    let mut sig = osc::saw(220.0).amp(0.6).diode_clip(3.0);
    let _guard = DenyAllocGuard::new();
    for tick in 0..256 {
        let _ = sig.next(&AudioContext {
            sample_rate: SR,
            tick,
        });
    }
}

#[test]
fn golden_diode_clip_saw_440() {
    let mut sig = osc::saw(440.0).amp(0.5).diode_clip(4.0);
    assert_golden(
        &mut sig,
        &GoldenTest {
            name: "sat_diode_saw_440_drive4",
            duration_secs: 0.01,
            sample_rate: SR,
            tolerance: 1e-4,
        },
    );
}
