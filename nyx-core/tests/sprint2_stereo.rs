//! Sprint 2 — Stereo refactor: trait default, Pan override, Haas widener.

use nyx_core::{AudioContext, DenyAllocGuard, HaasSide, Signal, SignalExt, osc};

const SR: f32 = 44100.0;

fn ctx(tick: u64) -> AudioContext {
    AudioContext {
        sample_rate: SR,
        tick,
    }
}

// ─────────────── Default impl: mono → duplicated stereo ───────────────

#[test]
fn mono_signal_default_duplicates_channels() {
    // A plain sine — no stereo override. Default next_stereo should
    // return (s, s) for all samples.
    let mut sig = osc::sine(440.0);
    for tick in 0..100 {
        let mono = {
            // Rebuild a fresh sig each time so we can compare mono vs stereo.
            // Actually we need to advance both in parallel — reset isn't an option.

            sig.next(&ctx(tick))
        };
        // For the same tick, next_stereo should give (mono, mono). But
        // we've already advanced 'sig' via next(). Build a parallel
        // signal to get next_stereo outputs.
        // Just verify that calling next_stereo gives identical L/R.
        let mut sig2 = osc::sine(440.0);
        for _ in 0..tick {
            sig2.next(&ctx(0));
        }
        let (l, r) = sig2.next_stereo(&ctx(tick));
        assert!(
            (l - r).abs() < 1e-6,
            "mono default should give L==R at tick {tick}"
        );
        let _ = mono;
    }
}

#[test]
fn mono_default_uses_next_output() {
    // The default next_stereo should consume one sample from next and
    // return it on both channels — verify by comparing directly.
    let mut sig = osc::sine(440.0);
    let (l, r) = sig.next_stereo(&ctx(0));
    assert!((l - r).abs() < 1e-6);
    // A fresh sine at the same tick should produce the same sample.
    let mut fresh = osc::sine(440.0);
    let mono = fresh.next(&ctx(0));
    assert!((l - mono).abs() < 1e-6);
}

// ─────────────── Pan: real stereo behaviour ───────────────

#[test]
fn pan_centre_splits_evenly() {
    // Constant 1.0 signal at centre pan → L = R = 0.5
    let mut sig = (|_ctx: &AudioContext| 1.0_f32).pan(0.0);
    let (l, r) = sig.next_stereo(&ctx(0));
    assert!((l - 0.5).abs() < 1e-6, "centre L should be 0.5, got {l}");
    assert!((r - 0.5).abs() < 1e-6, "centre R should be 0.5, got {r}");
}

#[test]
fn pan_hard_left() {
    let mut sig = (|_ctx: &AudioContext| 1.0_f32).pan(-1.0);
    let (l, r) = sig.next_stereo(&ctx(0));
    assert!((l - 1.0).abs() < 1e-6, "hard-left L should be 1.0, got {l}");
    assert!(r.abs() < 1e-6, "hard-left R should be 0, got {r}");
}

#[test]
fn pan_hard_right() {
    let mut sig = (|_ctx: &AudioContext| 1.0_f32).pan(1.0);
    let (l, r) = sig.next_stereo(&ctx(0));
    assert!(l.abs() < 1e-6, "hard-right L should be 0, got {l}");
    assert!(
        (r - 1.0).abs() < 1e-6,
        "hard-right R should be 1.0, got {r}"
    );
}

#[test]
fn pan_mono_fold_preserves_gain() {
    // Pan-law: summing L+R should always equal the input signal.
    let sig = |_ctx: &AudioContext| 1.0_f32;
    for pos in [-1.0, -0.5, 0.0, 0.5, 1.0] {
        let mut panned = sig.pan(pos);
        let mono = panned.next(&ctx(0));
        assert!(
            (mono - 1.0).abs() < 1e-6,
            "pan={pos} should sum to 1.0, got {mono}"
        );
    }
}

// ─────────────── Haas widener ───────────────

#[test]
fn haas_right_produces_stereo_width() {
    // Impulse input: 1.0 on tick 0, then 0s. Haas::Right delays the
    // right channel, so L should see the impulse first, R some samples
    // later.
    let mut fired = false;
    let impulse = move |_ctx: &AudioContext| {
        if fired {
            0.0
        } else {
            fired = true;
            1.0
        }
    };
    let delay_ms = 10.0_f32;
    let delay_samples = (delay_ms * 0.001 * 44100.0_f32).round() as i64;
    let mut sig = impulse.haas(delay_ms);

    // First sample: L gets the impulse, R gets silence (no delay yet).
    let (l0, r0) = sig.next_stereo(&ctx(0));
    assert!(
        (l0 - 1.0).abs() < 1e-6,
        "L should see impulse immediately, got {l0}"
    );
    assert!(
        r0.abs() < 1e-6,
        "R should be silent on first sample, got {r0}"
    );

    // Advance until the delay kicks in on R.
    let mut r_saw_impulse = false;
    for tick in 1..=(delay_samples as u64 + 5) {
        let (_, r) = sig.next_stereo(&ctx(tick));
        if r > 0.5 {
            r_saw_impulse = true;
            break;
        }
    }
    assert!(r_saw_impulse, "R channel never received the impulse");
}

#[test]
fn haas_left_side() {
    // With HaasSide::Left, the LEFT channel is delayed.
    let mut fired = false;
    let impulse = move |_ctx: &AudioContext| {
        if fired {
            0.0
        } else {
            fired = true;
            1.0
        }
    };
    let mut sig = impulse.haas_side(10.0, HaasSide::Left);
    let (l0, r0) = sig.next_stereo(&ctx(0));
    assert!(
        (r0 - 1.0).abs() < 1e-6,
        "R should see impulse first with HaasSide::Left"
    );
    assert!(l0.abs() < 1e-6, "L should be silent on first sample");
}

#[test]
fn haas_mono_fold_is_nearly_original() {
    // Haas mono-folds to comb-filtered source. Low-frequency content
    // passes through fairly intact (gain ≈ 1.0 at DC).
    let mut sig = (|_ctx: &AudioContext| 1.0_f32).haas(15.0);
    // Skip the first few samples while the delay buffer primes.
    for _ in 0..2048 {
        sig.next(&ctx(0));
    }
    // DC through a Haas widener: L + R = input + delayed_input = 2.0.
    let mono = sig.next(&ctx(0));
    assert!(
        (mono - 2.0).abs() < 0.1,
        "DC mono-fold should be ~2.0 (both channels pass DC), got {mono}"
    );
}

// ─────────────── Box<dyn Signal> forwards ───────────────

#[test]
fn boxed_signal_forwards_next_stereo() {
    // Pan through .boxed() — the Box<dyn Signal> must forward
    // next_stereo to the inner Pan's override, not fall back to the
    // default duplicate.
    let mut sig = (|_ctx: &AudioContext| 1.0_f32).pan(-1.0).boxed();
    let (l, r) = sig.next_stereo(&ctx(0));
    assert!((l - 1.0).abs() < 1e-6);
    assert!(r.abs() < 1e-6);
}

// ─────────────── No-alloc ───────────────

#[test]
fn haas_does_not_allocate_per_sample() {
    let mut sig = osc::sine(440.0).haas(15.0);
    let c = ctx(0);
    for _ in 0..10 {
        sig.next_stereo(&c);
    }
    let _guard = DenyAllocGuard::new();
    for _ in 0..4096 {
        sig.next_stereo(&c);
    }
}

#[test]
fn pan_stereo_does_not_allocate() {
    let mut sig = osc::sine(440.0).pan(0.3);
    let c = ctx(0);
    let _guard = DenyAllocGuard::new();
    for _ in 0..4096 {
        sig.next_stereo(&c);
    }
}
