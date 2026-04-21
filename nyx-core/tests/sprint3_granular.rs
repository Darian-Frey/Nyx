//! Sprint 3 — Granular synthesis tests.

use nyx_core::{
    render_to_buffer, AudioContext, DenyAllocGuard, Granular, Sample, Signal,
};

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

/// Build a 1-second sine sample for granulation tests.
fn sine_sample(freq: f32, amp: f32) -> Sample {
    let n = SR as usize;
    let mut buf = Vec::with_capacity(n);
    for i in 0..n {
        let phase = i as f32 * freq / SR;
        buf.push((phase * std::f32::consts::TAU).sin() * amp);
    }
    Sample::from_buffer(buf, SR).unwrap()
}

/// Silent sample of 1 second.
fn silent_sample() -> Sample {
    Sample::from_buffer(vec![0.0; SR as usize], SR).unwrap()
}

#[test]
fn produces_sound_with_defaults() {
    let mut g = Granular::new(sine_sample(440.0, 0.5));
    let buf = render_to_buffer(&mut g, 0.5, SR);
    let r = rms(&buf);
    assert!(r > 0.01, "default config should produce audible output, rms={r}");
    for (i, &s) in buf.iter().enumerate() {
        assert!(s.is_finite(), "non-finite at {i}: {s}");
    }
}

#[test]
fn silent_sample_yields_silence() {
    let mut g = Granular::new(silent_sample()).density(40.0);
    let buf = render_to_buffer(&mut g, 0.3, SR);
    let p = peak(&buf);
    assert!(p < 1e-6, "silent source should give silent granulation, peak={p}");
}

#[test]
fn zero_density_is_silent() {
    // No grains spawn at density=0.
    let mut g = Granular::new(sine_sample(440.0, 0.5)).density(0.0);
    let buf = render_to_buffer(&mut g, 0.3, SR);
    let p = peak(&buf);
    assert!(p < 1e-6, "density=0 should be silent, peak={p}");
}

#[test]
fn higher_density_produces_louder_cloud() {
    // More overlapping grains → more energy.
    let mut sparse = Granular::new(sine_sample(440.0, 0.5))
        .density(10.0)
        .grain_size(0.05)
        .seed(1);
    let mut dense = Granular::new(sine_sample(440.0, 0.5))
        .density(80.0)
        .grain_size(0.05)
        .seed(1);

    let sb = render_to_buffer(&mut sparse, 0.5, SR);
    let db = render_to_buffer(&mut dense, 0.5, SR);

    let r_sparse = rms(&sb);
    let r_dense = rms(&db);
    assert!(
        r_dense > r_sparse * 1.5,
        "high density should be louder: sparse={r_sparse}, dense={r_dense}"
    );
}

#[test]
fn output_is_bounded() {
    // Crank density + amp; verify no runaway.
    let mut g = Granular::new(sine_sample(440.0, 1.0))
        .density(200.0)
        .grain_size(0.1)
        .amp(1.0);
    let buf = render_to_buffer(&mut g, 0.5, SR);
    let p = peak(&buf);
    assert!(p.is_finite() && p < 10.0, "granular output blew up, peak={p}");
}

#[test]
fn pan_spread_creates_stereo_difference() {
    let mut g = Granular::new(sine_sample(440.0, 0.5))
        .density(50.0)
        .pan_spread(1.0)
        .seed(99);

    let mut diff = 0.0_f32;
    for tick in 0..8192 {
        let (l, r) = g.next_stereo(&ctx(tick));
        diff += (l - r).abs();
    }
    assert!(diff > 5.0, "pan_spread=1.0 should yield stereo diff, got {diff}");
}

#[test]
fn zero_pan_spread_is_mono() {
    let mut g = Granular::new(sine_sample(440.0, 0.5))
        .density(40.0)
        .pan_spread(0.0)
        .seed(42);

    for _ in 0..500 {
        let (l, r) = g.next_stereo(&ctx(0));
        assert!(
            (l - r).abs() < 1e-5,
            "pan_spread=0 should be centre-panned (L=R): l={l}, r={r}"
        );
    }
}

#[test]
fn seed_reproducibility() {
    // Same seed + same params → byte-identical output.
    let mut a = Granular::new(sine_sample(440.0, 0.5))
        .density(30.0)
        .pitch_jitter(0.05)
        .pan_spread(0.5)
        .seed(12345);
    let mut b = Granular::new(sine_sample(440.0, 0.5))
        .density(30.0)
        .pitch_jitter(0.05)
        .pan_spread(0.5)
        .seed(12345);

    let ba = render_to_buffer(&mut a, 0.3, SR);
    let bb = render_to_buffer(&mut b, 0.3, SR);

    for (i, (&xa, &xb)) in ba.iter().zip(bb.iter()).enumerate() {
        assert!(
            (xa - xb).abs() < 1e-6,
            "seed={{12345}} should be reproducible, diverges at {i}: {xa} vs {xb}"
        );
    }
}

#[test]
fn different_seeds_diverge() {
    let mut a = Granular::new(sine_sample(440.0, 0.5))
        .density(30.0)
        .pitch_jitter(0.05)
        .pan_spread(0.5)
        .seed(1);
    let mut b = Granular::new(sine_sample(440.0, 0.5))
        .density(30.0)
        .pitch_jitter(0.05)
        .pan_spread(0.5)
        .seed(2);

    let ba = render_to_buffer(&mut a, 0.3, SR);
    let bb = render_to_buffer(&mut b, 0.3, SR);
    let diff: f32 = ba.iter().zip(bb.iter()).map(|(x, y)| (x - y).abs()).sum();
    assert!(diff > 1.0, "different seeds should diverge, sum diff={diff}");
}

#[test]
fn pitch_zero_grain_freezes_read_position() {
    // pitch=0 → grains don't advance through the source; each grain's
    // window still plays but reads a DC-ish value from a single position.
    // Output should be finite and bounded; just guard against NaN/inf.
    let mut g = Granular::new(sine_sample(440.0, 0.5))
        .density(40.0)
        .pitch(0.0)
        .grain_size(0.03);
    let buf = render_to_buffer(&mut g, 0.2, SR);
    for (i, &s) in buf.iter().enumerate() {
        assert!(s.is_finite(), "pitch=0 produced non-finite at {i}: {s}");
    }
}

#[test]
fn does_not_allocate_in_callback() {
    let mut g = Granular::new(sine_sample(440.0, 0.5))
        .density(60.0)
        .pitch_jitter(0.03)
        .pan_spread(0.8);

    // Warm up — run through the scheduler and let grains fill the pool.
    let c = ctx(0);
    for _ in 0..4096 {
        g.next(&c);
    }

    let _guard = DenyAllocGuard::new();
    for tick in 0..8192 {
        g.next(&ctx(tick));
    }
}

#[test]
fn stereo_does_not_allocate_in_callback() {
    let mut g = Granular::new(sine_sample(440.0, 0.5))
        .density(50.0)
        .pan_spread(1.0);

    let c = ctx(0);
    for _ in 0..4096 {
        g.next_stereo(&c);
    }

    let _guard = DenyAllocGuard::new();
    for tick in 0..4096 {
        g.next_stereo(&ctx(tick));
    }
}

#[test]
fn voice_pool_cap_is_respected() {
    // With 4 voices and high density, no panic, no alloc, just dropped grains.
    let mut g = Granular::with_voices(sine_sample(440.0, 0.5), 4)
        .density(500.0) // way more than 4 voices can cover with 50 ms grains
        .grain_size(0.05);

    let buf = render_to_buffer(&mut g, 0.3, SR);
    let p = peak(&buf);
    assert!(p.is_finite() && p < 5.0, "pool-capped output peak={p}");
}
