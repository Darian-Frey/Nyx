//! Sprint 3 — Bus / mixer tests.

use nyx_core::{osc, render_to_buffer, AudioContext, Bus, DenyAllocGuard, Signal, SignalExt};

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

#[test]
fn empty_bus_is_silent() {
    let mut bus = Bus::new();
    let c = ctx(0);
    for _ in 0..1024 {
        assert_eq!(bus.next(&c), 0.0);
    }
}

#[test]
fn bus_len_and_is_empty_track_adds() {
    let bus = Bus::new();
    assert!(bus.is_empty());
    assert_eq!(bus.len(), 0);
    let bus = bus.add(osc::sine(440.0)).add(osc::saw(220.0));
    assert!(!bus.is_empty());
    assert_eq!(bus.len(), 2);
}

#[test]
fn bus_sums_multiple_sources() {
    // Two sines at different freqs — combined RMS should exceed either alone.
    let mut solo = osc::sine(440.0).amp(0.3);
    let solo_buf = render_to_buffer(&mut solo, 0.2, SR);

    let mut bus = Bus::new()
        .add(osc::sine(440.0).amp(0.3))
        .add(osc::sine(660.0).amp(0.3));
    let bus_buf = render_to_buffer(&mut bus, 0.2, SR);

    let r_solo = rms(&solo_buf);
    let r_bus = rms(&bus_buf);
    assert!(
        r_bus > r_solo * 1.2,
        "two-source bus should be louder than solo: bus={r_bus}, solo={r_solo}"
    );
}

#[test]
fn bus_gain_scales_output() {
    let mut full = Bus::new().add(osc::sine(440.0).amp(0.3)).gain(1.0);
    let mut half = Bus::new().add(osc::sine(440.0).amp(0.3)).gain(0.5);

    let full_buf = render_to_buffer(&mut full, 0.2, SR);
    let half_buf = render_to_buffer(&mut half, 0.2, SR);

    let r_full = rms(&full_buf);
    let r_half = rms(&half_buf);
    let ratio = r_full / r_half;
    assert!(
        (ratio - 2.0).abs() < 0.01,
        "gain=0.5 should halve RMS: ratio={ratio}"
    );
}

#[test]
fn single_source_bus_matches_source_with_gain() {
    // Bus with one source and gain=1.0 should be sample-identical to source.
    let mut bus = Bus::new().add(osc::sine(440.0).amp(0.2));
    let mut solo = osc::sine(440.0).amp(0.2);

    let bus_buf = render_to_buffer(&mut bus, 0.1, SR);
    let solo_buf = render_to_buffer(&mut solo, 0.1, SR);

    for (i, (&b, &s)) in bus_buf.iter().zip(solo_buf.iter()).enumerate() {
        assert!(
            (b - s).abs() < 1e-6,
            "single-source bus should equal source at {i}: bus={b}, solo={s}"
        );
    }
}

#[test]
fn bus_preserves_stereo_from_panned_sources() {
    // Left-panned sine + right-panned sine → both channels should have content
    // but L and R should differ noticeably.
    let mut bus = Bus::new()
        .add(osc::sine(440.0).amp(0.3).pan(-1.0))
        .add(osc::sine(660.0).amp(0.3).pan(1.0));

    let mut sum_l = 0.0_f32;
    let mut sum_r = 0.0_f32;
    let mut diff = 0.0_f32;
    for tick in 0..8192 {
        let (l, r) = bus.next_stereo(&ctx(tick));
        sum_l += l.abs();
        sum_r += r.abs();
        diff += (l - r).abs();
    }
    assert!(sum_l > 10.0, "L should have content, sum_l={sum_l}");
    assert!(sum_r > 10.0, "R should have content, sum_r={sum_r}");
    assert!(diff > 10.0, "L and R should differ (stereo content), diff={diff}");
}

#[test]
fn bus_composes_with_signalext_combinators() {
    // Bus → compress → freeverb should compile and produce bounded output.
    let bus = Bus::new()
        .add(osc::sine(440.0).amp(0.2))
        .add(osc::saw(220.0).amp(0.15))
        .compress(-12.0, 4.0)
        .freeverb()
        .wet(0.3);

    let mut sig = bus;
    let buf = render_to_buffer(&mut sig, 0.3, SR);
    let peak = buf.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
    assert!(peak.is_finite() && peak < 5.0, "bus chain peak={peak}");
}

#[test]
fn bus_does_not_allocate_in_callback() {
    let mut bus = Bus::new()
        .add(osc::sine(440.0).amp(0.3))
        .add(osc::saw(220.0).amp(0.2))
        .add(osc::square(110.0).amp(0.15))
        .gain(0.8);

    // Warm up.
    let c = ctx(0);
    for _ in 0..10 {
        bus.next(&c);
    }

    let _guard = DenyAllocGuard::new();
    for _ in 0..4096 {
        bus.next(&c);
    }
}

#[test]
fn bus_stereo_does_not_allocate() {
    let mut bus = Bus::new()
        .add(osc::sine(440.0).pan(-0.5))
        .add(osc::saw(220.0).pan(0.5))
        .gain(0.5);

    let c = ctx(0);
    for _ in 0..10 {
        bus.next_stereo(&c);
    }

    let _guard = DenyAllocGuard::new();
    for _ in 0..4096 {
        bus.next_stereo(&c);
    }
}

#[test]
fn nested_buses_work() {
    // Bus of buses — models a mix-bus architecture.
    let drums = Bus::new()
        .add(osc::noise::white(1).amp(0.1))
        .add(osc::sine(55.0).amp(0.2))
        .gain(0.8);

    let mut mix = Bus::new()
        .add(drums)
        .add(osc::sine(440.0).amp(0.2))
        .gain(0.9);

    let buf = render_to_buffer(&mut mix, 0.2, SR);
    let r = rms(&buf);
    assert!(r > 0.05, "nested bus should produce sound, rms={r}");
    for (i, &s) in buf.iter().enumerate() {
        assert!(s.is_finite(), "non-finite at {i}: {s}");
    }
}

#[test]
fn with_capacity_matches_new() {
    // Behavior-wise identical; just a perf hint.
    let mut a = Bus::new().add(osc::sine(440.0).amp(0.2));
    let mut b = Bus::with_capacity(4).add(osc::sine(440.0).amp(0.2));

    let ba = render_to_buffer(&mut a, 0.05, SR);
    let bb = render_to_buffer(&mut b, 0.05, SR);

    for (i, (&xa, &xb)) in ba.iter().zip(bb.iter()).enumerate() {
        assert!((xa - xb).abs() < 1e-6, "with_capacity diverges at {i}");
    }
}
