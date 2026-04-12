use nyx_core::{
    render_to_buffer, AudioContext, Signal, SignalExt,
    ScopeExt, InspectExt, SpectrumExt,
    SpectrumConfig, WindowFn,
};
use nyx_core::osc;

const SR: f32 = 44100.0;

fn ctx(tick: u64) -> AudioContext {
    AudioContext {
        sample_rate: SR,
        tick,
    }
}

// ===================== Scope tests =====================

#[test]
fn scope_captures_samples() {
    let (mut sig, mut handle) = osc::sine(440.0).scope(4096);
    // Render 1000 samples through the scope.
    for tick in 0..1000 {
        sig.next(&ctx(tick));
    }
    let mut buf = vec![0.0_f32; 2000];
    let n = handle.read(&mut buf);
    assert_eq!(n, 1000, "should have read 1000 samples, got {n}");
}

#[test]
fn scope_passthrough_unchanged() {
    // The scope should not alter the signal.
    let mut plain = osc::sine(440.0);
    let (mut scoped, _handle) = osc::sine(440.0).scope(4096);
    for tick in 0..100 {
        let a = plain.next(&ctx(tick));
        let b = scoped.next(&ctx(tick));
        assert!(
            (a - b).abs() < 1e-10,
            "scope altered sample at tick {tick}: {a} vs {b}"
        );
    }
}

#[test]
fn scope_drops_when_full() {
    let (mut sig, mut handle) = osc::sine(440.0).scope(64);
    // Write more samples than buffer capacity.
    for tick in 0..200 {
        sig.next(&ctx(tick));
    }
    // Should only get 64 (buffer size), rest are dropped.
    let mut buf = vec![0.0; 200];
    let n = handle.read(&mut buf);
    assert_eq!(n, 64, "should read at most buffer capacity, got {n}");
}

#[test]
fn scope_available_count() {
    let (mut sig, handle) = osc::sine(440.0).scope(4096);
    assert_eq!(handle.available(), 0);
    for tick in 0..100 {
        sig.next(&ctx(tick));
    }
    assert_eq!(handle.available(), 100);
}

// ===================== Inspect tests =====================

#[test]
fn inspect_sees_every_sample() {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    let count = Arc::new(AtomicU32::new(0));
    let count_clone = Arc::clone(&count);

    let mut sig = osc::sine(440.0).inspect(move |_sample, _ctx| {
        count_clone.fetch_add(1, Ordering::Relaxed);
    });

    for tick in 0..500 {
        sig.next(&ctx(tick));
    }
    assert_eq!(count.load(Ordering::Relaxed), 500);
}

#[test]
fn inspect_passthrough_unchanged() {
    let mut plain = osc::sine(440.0);
    let mut inspected = osc::sine(440.0).inspect(|_s, _ctx| {});
    for tick in 0..100 {
        let a = plain.next(&ctx(tick));
        let b = inspected.next(&ctx(tick));
        assert!(
            (a - b).abs() < 1e-10,
            "inspect altered sample at tick {tick}"
        );
    }
}

#[test]
fn inspect_tracks_peak() {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    // Use atomic u32 to store peak as bits (no f32 atomics).
    let peak_bits = Arc::new(AtomicU32::new(0));
    let peak_clone = Arc::clone(&peak_bits);

    let mut sig = osc::sine(440.0).inspect(move |s, _ctx| {
        let abs = s.abs();
        loop {
            let current = peak_clone.load(Ordering::Relaxed);
            let current_f = f32::from_bits(current);
            if abs <= current_f {
                break;
            }
            if peak_clone
                .compare_exchange(current, abs.to_bits(), Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }
    });

    for tick in 0..44100 {
        sig.next(&ctx(tick));
    }

    let peak = f32::from_bits(peak_bits.load(Ordering::Relaxed));
    // Sine wave peak should be ~1.0.
    assert!(
        (peak - 1.0).abs() < 0.01,
        "peak should be ~1.0, got {peak}"
    );
}

// ===================== Spectrum tests =====================

#[test]
fn spectrum_produces_bins() {
    let config = SpectrumConfig {
        frame_size: 1024,
        window: WindowFn::Hann,
    };
    let (mut sig, handle) = osc::sine(440.0).spectrum(config);

    // Need to render at least one full frame.
    for tick in 0..1024 {
        sig.next(&ctx(tick));
    }

    let bins = handle.snapshot();
    assert!(!bins.is_empty(), "spectrum should have produced bins");
}

#[test]
fn spectrum_peak_at_440hz() {
    let config = SpectrumConfig {
        frame_size: 4096,
        window: WindowFn::Hann,
    };
    let (mut sig, handle) = osc::sine(440.0).spectrum(config);

    // Render a few frames to get a stable spectrum.
    for tick in 0..8192 {
        sig.next(&ctx(tick));
    }

    let bins = handle.snapshot();
    assert!(!bins.is_empty());

    // Find the bin with maximum magnitude.
    let peak_bin = bins
        .iter()
        .max_by(|a, b| a.magnitude.partial_cmp(&b.magnitude).unwrap())
        .unwrap();

    // Should be near 440 Hz (within one bin width).
    let bin_width = SR / 4096.0; // ~10.77 Hz
    assert!(
        (peak_bin.freq - 440.0).abs() < bin_width * 2.0,
        "peak should be near 440 Hz, got {} Hz",
        peak_bin.freq
    );
}

#[test]
fn spectrum_passthrough_unchanged() {
    let config = SpectrumConfig::default();
    let mut plain = osc::sine(440.0);
    let (mut spec, _handle) = osc::sine(440.0).spectrum(config);

    for tick in 0..100 {
        let a = plain.next(&ctx(tick));
        let b = spec.next(&ctx(tick));
        assert!(
            (a - b).abs() < 1e-10,
            "spectrum altered sample at tick {tick}"
        );
    }
}

#[test]
fn spectrum_blackman_window() {
    let config = SpectrumConfig {
        frame_size: 1024,
        window: WindowFn::Blackman,
    };
    let (mut sig, handle) = osc::sine(1000.0).spectrum(config);

    for tick in 0..1024 {
        sig.next(&ctx(tick));
    }

    let bins = handle.snapshot();
    assert!(!bins.is_empty(), "blackman window should produce bins");
}

#[test]
fn spectrum_bin_count() {
    let config = SpectrumConfig {
        frame_size: 2048,
        window: WindowFn::Hann,
    };
    let (mut sig, handle) = osc::sine(440.0).spectrum(config);

    for tick in 0..2048 {
        sig.next(&ctx(tick));
    }

    // FFT of N real samples produces N/2 + 1 bins, but spectrum-analyzer
    // may vary. Just check it's reasonable.
    let count = handle.bin_count();
    assert!(
        count > 100 && count <= 1025,
        "unexpected bin count: {count}"
    );
}

// ===================== Combined: scope + inspect + chain =====================

#[test]
fn scope_after_filter_chain() {
    let (mut sig, mut handle) = osc::saw(220.0)
        .amp(0.5)
        .clip(0.8)
        .scope(4096);

    let buf = render_to_buffer(&mut sig, 0.01, SR);
    assert!(!buf.is_empty());

    let mut scope_buf = vec![0.0_f32; 1000];
    let n = handle.read(&mut scope_buf);
    assert!(n > 0);
    for &s in &scope_buf[..n] {
        assert!(s.abs() <= 0.8 + 1e-6, "clipped scope sample out of range: {s}");
    }
}
