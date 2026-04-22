//! Sonic character — Moog-style ladder lowpass tests.
//!
//! Exercises the four essentials of an analog-style filter:
//! (1) attenuates content above cutoff (4-pole rolloff), (2) passes
//! content well below cutoff, (3) self-oscillates at resonance ≥ 1.0
//! (stable, bounded), (4) tracks cutoff modulation without blowing
//! up. Plus bounds, no-alloc, and a golden file pinning the tuned
//! output at cutoff=800 Hz, resonance=0.7.

#[global_allocator]
static ALLOC: nyx_core::GuardedAllocator = nyx_core::GuardedAllocator;

use nyx_core::golden::{GoldenTest, assert_golden};
use nyx_core::{AudioContext, DenyAllocGuard, LadderExt, Signal, SignalExt, osc, render_to_buffer};

const SR: f32 = 44100.0;

fn rms(buf: &[f32]) -> f32 {
    (buf.iter().map(|s| s * s).sum::<f32>() / buf.len() as f32).sqrt()
}

#[test]
fn ladder_attenuates_high_frequencies() {
    // 10 kHz sine through an 800 Hz lowpass should be all but silent
    // after 4 poles of rolloff (~−54 dB at 10× cutoff).
    let mut sig = osc::sine(10_000.0).ladder_lp(800.0, 0.3);
    // Discard settling tail.
    let _ = render_to_buffer(&mut sig, 0.1, SR);
    let buf = render_to_buffer(&mut sig, 0.2, SR);
    let rms = rms(&buf);
    assert!(
        rms < 0.02,
        "10 kHz through 800 Hz ladder LP should be attenuated, rms={rms}"
    );
}

#[test]
fn ladder_passes_low_frequencies() {
    // Use resonance=0 for a clean pass-through check. At higher `k`
    // the ladder's DC gain drops to `1/(1+k)` — the classic Moog
    // "loudness falls as resonance rises" property, which is correct
    // behaviour but a separate test would obscure this one. Input is
    // kept at 0.3 amplitude so per-stage tanh stays near-linear.
    let mut sig = osc::sine(100.0).amp(0.3).ladder_lp(4_000.0, 0.0);
    let _ = render_to_buffer(&mut sig, 0.1, SR);
    let buf = render_to_buffer(&mut sig, 0.2, SR);
    let rms = rms(&buf);
    // Source RMS = 0.3 / √2 ≈ 0.212. Allow ~15% ladder-induced loss.
    assert!(
        rms > 0.18,
        "100 Hz through 4 kHz ladder LP should pass, rms={rms}"
    );
}

#[test]
fn ladder_dc_gain_drops_with_resonance() {
    // Documents the canonical Moog behaviour: DC gain ≈ 1/(1+4·res).
    // A sanity check that resonance actually suppresses the low-end
    // level (it does — the feedback subtracts from the input).
    let mut quiet = osc::sine(100.0).amp(0.3).ladder_lp(4_000.0, 0.0);
    let mut loud = osc::sine(100.0).amp(0.3).ladder_lp(4_000.0, 0.5);
    let _ = render_to_buffer(&mut quiet, 0.1, SR);
    let _ = render_to_buffer(&mut loud, 0.1, SR);
    let a = rms(&render_to_buffer(&mut quiet, 0.2, SR));
    let b = rms(&render_to_buffer(&mut loud, 0.2, SR));
    assert!(
        b < a,
        "higher resonance should lower DC level; res=0 rms={a}, res=0.5 rms={b}"
    );
}

#[test]
fn ladder_self_oscillates_at_high_resonance() {
    // Silent input + resonance ≥ 1.0 → filter should sustain an
    // oscillation at/near the cutoff frequency. We seed it with a
    // tiny impulse train of noise to kick the loop into motion
    // (real-world self-oscillation needs a nudge; otherwise the
    // all-zero state sits at zero forever).
    let mut seeded = false;
    let mut source = move |_ctx: &AudioContext| {
        if !seeded {
            seeded = true;
            0.01 // single-sample poke
        } else {
            0.0
        }
    };
    let mut sig = (&mut source).ladder_lp(600.0, 1.05);
    // Settle through the attack transient.
    let _ = render_to_buffer(&mut sig, 0.5, SR);
    let buf = render_to_buffer(&mut sig, 0.2, SR);
    let rms = rms(&buf);
    assert!(
        rms > 0.05,
        "ladder should sustain self-oscillation at resonance=1.05, rms={rms}"
    );
    // Output must stay bounded (tanh on output prevents runaway).
    for &s in &buf {
        assert!(s.abs() <= 1.05, "ladder self-osc exceeded unit bound: {s}");
    }
}

#[test]
fn ladder_output_bounded_under_extreme_input() {
    // Hammer the filter with loud saw + max resonance to stress the
    // feedback path. Output must stay finite and bounded by tanh.
    let mut sig = osc::saw(110.0)
        .ladder_lp(1200.0_f32, 1.2_f32)
        .ladder_lp(1200.0_f32, 0.0_f32); // second instance with no resonance — compile check only
    let buf = render_to_buffer(&mut sig, 0.2, SR);
    for &s in &buf {
        assert!(
            s.is_finite() && s.abs() <= 1.1,
            "ladder output exceeded bound: {s}"
        );
    }
}

#[test]
fn ladder_tracks_cutoff_modulation() {
    // Modulated cutoff sweep from 200 → 3000 Hz. Output must stay
    // bounded throughout (no blow-up when cutoff moves fast).
    let lfo = osc::sine(2.0).amp(1400.0).offset(1600.0);
    let mut sig = osc::saw(220.0).ladder_lp(lfo, 0.6);
    let buf = render_to_buffer(&mut sig, 0.5, SR);
    for &s in &buf {
        assert!(s.is_finite() && s.abs() <= 1.1, "modulated ladder out: {s}");
    }
    let rms = rms(&buf);
    assert!(
        rms > 0.05,
        "modulated sweep should produce audible output, rms={rms}"
    );
}

#[test]
fn ladder_no_alloc() {
    let mut sig = osc::saw(220.0).ladder_lp(1200.0, 0.8);
    let _guard = DenyAllocGuard::new();
    for tick in 0..512 {
        let _ = sig.next(&AudioContext {
            sample_rate: SR,
            tick,
        });
    }
}

#[test]
fn golden_ladder_saw_800_res07() {
    let mut sig = osc::saw(220.0).ladder_lp(800.0, 0.7);
    assert_golden(
        &mut sig,
        &GoldenTest {
            name: "ladder_saw220_cut800_res07",
            duration_secs: 0.01,
            sample_rate: SR,
            // Non-linear feedback → loose tolerance per the sonic-character
            // roadmap's testing convention.
            tolerance: 1e-4,
        },
    );
}

#[test]
fn golden_ladder_selfosc_res11() {
    // Silent input + resonance=1.1 should produce a near-pure sine at
    // the cutoff frequency. Seed with an impulse, settle, then capture.
    // Pin this at the edge of stability — catches regressions in the
    // feedback loop or tanh integration.
    let mut fired = false;
    let source = move |_ctx: &AudioContext| {
        if !fired {
            fired = true;
            0.05
        } else {
            0.0
        }
    };
    let mut sig = source.ladder_lp(600.0_f32, 1.1_f32);
    // Run 0.2 s to settle the oscillation, discard.
    let _ = render_to_buffer(&mut sig, 0.2, SR);
    assert_golden(
        &mut sig,
        &GoldenTest {
            name: "ladder_selfosc_cut600_res11",
            duration_secs: 0.01,
            sample_rate: SR,
            tolerance: 1e-3,
        },
    );
}
