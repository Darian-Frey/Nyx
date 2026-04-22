//! Sonic character — lo-fi preset wrapper tests.
//!
//! Each preset should: (1) produce bounded output, (2) not silence
//! the signal, (3) audibly differ from the raw source, and (4) not
//! allocate after construction.

#[global_allocator]
static ALLOC: nyx_core::GuardedAllocator = nyx_core::GuardedAllocator;

use nyx_core::{AudioContext, DenyAllocGuard, LofiExt, Signal, SignalExt, osc, render_to_buffer};

const SR: f32 = 44100.0;

fn rms(buf: &[f32]) -> f32 {
    (buf.iter().map(|s| s * s).sum::<f32>() / buf.len() as f32).sqrt()
}

fn make_source() -> impl Signal {
    osc::saw(220.0).amp(0.5)
}

#[test]
fn cassette_bounded_and_alive() {
    let mut sig = make_source().cassette();
    let buf = render_to_buffer(&mut sig, 0.3, SR);
    for &s in &buf {
        assert!(
            s.is_finite() && s.abs() <= 1.5,
            "cassette sample outside bound: {s}"
        );
    }
    let r = rms(&buf);
    assert!(r > 0.05, "cassette output too quiet, rms={r}");
}

#[test]
fn lofi_hiphop_bounded_and_alive() {
    let mut sig = make_source().lofi_hiphop();
    let buf = render_to_buffer(&mut sig, 0.3, SR);
    for &s in &buf {
        assert!(
            s.is_finite() && s.abs() <= 1.5,
            "lofi_hiphop sample outside bound: {s}"
        );
    }
    let r = rms(&buf);
    assert!(r > 0.05, "lofi_hiphop output too quiet, rms={r}");
}

#[test]
fn vhs_bounded_and_alive() {
    let mut sig = make_source().vhs();
    let buf = render_to_buffer(&mut sig, 0.3, SR);
    for &s in &buf {
        assert!(
            s.is_finite() && s.abs() <= 1.5,
            "vhs sample outside bound: {s}"
        );
    }
    let r = rms(&buf);
    assert!(r > 0.05, "vhs output too quiet, rms={r}");
}

#[test]
fn presets_differ_from_raw_source() {
    // All three presets should measurably change the signal — if any
    // returned a pass-through, this would catch it. Compare RMS of
    // (preset - raw) after aligning for the 10 ms tape base delay.
    let skip_samples = (0.02 * SR) as usize;
    let mut raw = make_source();
    let raw_buf = render_to_buffer(&mut raw, 0.3, SR);

    let mut cassette = make_source().cassette();
    let cassette_buf = render_to_buffer(&mut cassette, 0.3, SR);

    let mut lofi = make_source().lofi_hiphop();
    let lofi_buf = render_to_buffer(&mut lofi, 0.3, SR);

    let mut vhs = make_source().vhs();
    let vhs_buf = render_to_buffer(&mut vhs, 0.3, SR);

    // Skip the initial delay-alignment transient.
    let diff_rms = |a: &[f32], b: &[f32]| -> f32 {
        rms(&a[skip_samples..]
            .iter()
            .zip(b[skip_samples..].iter())
            .map(|(x, y)| x - y)
            .collect::<Vec<_>>())
    };

    assert!(
        diff_rms(&cassette_buf, &raw_buf) > 0.05,
        "cassette should modify the source"
    );
    assert!(
        diff_rms(&lofi_buf, &raw_buf) > 0.05,
        "lofi_hiphop should modify the source"
    );
    assert!(
        diff_rms(&vhs_buf, &raw_buf) > 0.05,
        "vhs should modify the source"
    );
}

#[test]
fn presets_differ_from_each_other() {
    // The three presets target different aesthetics — their outputs
    // on the same source must not be identical to each other.
    let mut cassette = make_source().cassette();
    let mut lofi = make_source().lofi_hiphop();
    let mut vhs = make_source().vhs();
    let a = render_to_buffer(&mut cassette, 0.3, SR);
    let b = render_to_buffer(&mut lofi, 0.3, SR);
    let c = render_to_buffer(&mut vhs, 0.3, SR);

    let diff = |x: &[f32], y: &[f32]| -> f32 {
        rms(&x
            .iter()
            .zip(y.iter())
            .map(|(a, b)| a - b)
            .collect::<Vec<_>>())
    };
    assert!(diff(&a, &b) > 0.05, "cassette vs lofi_hiphop should differ");
    assert!(diff(&a, &c) > 0.05, "cassette vs vhs should differ");
    assert!(diff(&b, &c) > 0.05, "lofi_hiphop vs vhs should differ");
}

#[test]
fn cassette_no_alloc() {
    let mut sig = make_source().cassette();
    let _guard = DenyAllocGuard::new();
    for tick in 0..512 {
        let _ = sig.next(&AudioContext {
            sample_rate: SR,
            tick,
        });
    }
}

#[test]
fn lofi_hiphop_no_alloc() {
    let mut sig = make_source().lofi_hiphop();
    let _guard = DenyAllocGuard::new();
    for tick in 0..512 {
        let _ = sig.next(&AudioContext {
            sample_rate: SR,
            tick,
        });
    }
}

#[test]
fn vhs_no_alloc() {
    let mut sig = make_source().vhs();
    let _guard = DenyAllocGuard::new();
    for tick in 0..512 {
        let _ = sig.next(&AudioContext {
            sample_rate: SR,
            tick,
        });
    }
}
