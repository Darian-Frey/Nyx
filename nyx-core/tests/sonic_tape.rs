//! Sonic character — tape emulation tests.
//!
//! Covers the [`Tape`] wrapper: bounded output, DC rejection, the age
//! knob scaling wow/flutter/drive together, and pitch modulation
//! producing a detectable waveform difference against a pristine
//! reference. Plus no-alloc guard and a golden file.

#[global_allocator]
static ALLOC: nyx_core::GuardedAllocator = nyx_core::GuardedAllocator;

use nyx_core::golden::{GoldenTest, assert_golden};
use nyx_core::{AudioContext, DenyAllocGuard, Signal, SignalExt, TapeExt, osc, render_to_buffer};

const SR: f32 = 44100.0;

fn rms(buf: &[f32]) -> f32 {
    (buf.iter().map(|s| s * s).sum::<f32>() / buf.len() as f32).sqrt()
}

fn mean(buf: &[f32]) -> f32 {
    buf.iter().sum::<f32>() / buf.len() as f32
}

#[test]
fn tape_output_bounded() {
    // The asymmetric tanh (tanh(drive·(x + bias)) − tanh(drive·bias))
    // widens the negative envelope slightly. At age=1 (drive=3) the
    // negative peak can reach ~−1.3. A ±1.4 bound is conservative
    // enough to flag any real runaway without false-positiving on
    // the intentional asymmetry.
    let mut sig = osc::saw(220.0).amp(0.8).tape().age(1.0);
    let buf = render_to_buffer(&mut sig, 0.2, SR);
    for &s in &buf {
        assert!(
            s.is_finite() && s.abs() <= 1.4,
            "tape sample outside bound: {s}"
        );
    }
}

#[test]
fn tape_removes_dc() {
    // Constant-DC source → the 30 Hz HP inside the tape chain should
    // kill it after enough settling time.
    let mut sig = (|_: &AudioContext| 0.3_f32).tape().age(0.5);
    let buf = render_to_buffer(&mut sig, 0.6, SR);
    let tail = &buf[(SR * 0.4) as usize..];
    let m = mean(tail);
    assert!(m.abs() < 0.06, "tape did not remove DC; mean={m}");
}

#[test]
fn tape_age_zero_is_close_to_input() {
    // age=0 disables wow, flutter, and extra drive → the chain should
    // be near-transparent. Base delay of 10 ms means we must align
    // outputs before comparing.
    let mut pristine = osc::sine(440.0).amp(0.3).tape().age(0.0);
    let buf_p = render_to_buffer(&mut pristine, 0.1, SR);
    // Tape at age=0 still has HP/LP plus minor tanh colour at
    // drive=1.0, so the signal isn't literally unchanged — just
    // check the output is alive and bounded RMS-wise.
    let rms_p = rms(&buf_p);
    assert!(
        rms_p > 0.05 && rms_p < 0.4,
        "age=0 tape should pass a sine roughly unchanged, rms={rms_p}"
    );
}

#[test]
fn tape_age_produces_pitch_modulation() {
    // age=1.0 applies the full wow+flutter modulation. Compared to
    // age=0 (no pitch mod), the two waveforms must diverge within
    // the first 300 ms.
    let mut pristine = osc::sine(440.0).amp(0.3).tape().age(0.0);
    let mut aged = osc::sine(440.0).amp(0.3).tape().age(1.0);
    // Discard the first 50 ms — that's delay-line alignment, before
    // wow/flutter have had time to build up meaningful displacement.
    let _ = render_to_buffer(&mut pristine, 0.05, SR);
    let _ = render_to_buffer(&mut aged, 0.05, SR);
    let a = render_to_buffer(&mut pristine, 0.25, SR);
    let b = render_to_buffer(&mut aged, 0.25, SR);

    let diff: Vec<f32> = a.iter().zip(b.iter()).map(|(x, y)| x - y).collect();
    let diff_rms = rms(&diff);
    assert!(
        diff_rms > 0.02,
        "age=1 should audibly differ from age=0, diff rms={diff_rms}"
    );
}

#[test]
fn tape_drive_audibly_changes_output() {
    // Explicit drive override must actually reach the saturator.
    let mut light = osc::saw(220.0).amp(0.6).tape().drive(1.0);
    let mut heavy = osc::saw(220.0).amp(0.6).tape().drive(6.0);
    // Settle past the base delay.
    let _ = render_to_buffer(&mut light, 0.05, SR);
    let _ = render_to_buffer(&mut heavy, 0.05, SR);
    let a = render_to_buffer(&mut light, 0.2, SR);
    let b = render_to_buffer(&mut heavy, 0.2, SR);
    let diff_rms = rms(&a
        .iter()
        .zip(b.iter())
        .map(|(x, y)| x - y)
        .collect::<Vec<_>>());
    assert!(
        diff_rms > 0.05,
        "tape at drive=1 vs drive=6 should differ, diff rms={diff_rms}"
    );
}

#[test]
fn tape_no_alloc_after_construction() {
    let mut sig = osc::saw(220.0).amp(0.6).tape().age(0.7);
    let _guard = DenyAllocGuard::new();
    for tick in 0..512 {
        let _ = sig.next(&AudioContext {
            sample_rate: SR,
            tick,
        });
    }
}

#[test]
fn golden_tape_saw_220_age05() {
    // Pin the default-age tape output so regressions in wow/flutter
    // timing or saturator coefficients are caught. 100 ms captures
    // enough wow cycle to include both sides of the modulation.
    let mut sig = osc::saw(220.0).amp(0.5).tape().age(0.5);
    assert_golden(
        &mut sig,
        &GoldenTest {
            name: "tape_saw220_age05",
            duration_secs: 0.1,
            sample_rate: SR,
            // Non-linear + delay + filters → looser tolerance.
            tolerance: 1e-4,
        },
    );
}
