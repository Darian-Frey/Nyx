use nyx_core::hotswap::HotSwap;
use nyx_core::{AudioContext, Signal, SignalExt};

const SR: f32 = 44100.0;

fn ctx(tick: u64) -> AudioContext {
    AudioContext {
        sample_rate: SR,
        tick,
    }
}

struct Const(f32);

impl Signal for Const {
    fn next(&mut self, _ctx: &AudioContext) -> f32 {
        self.0
    }
}

#[test]
fn hotswap_plays_initial_signal() {
    let mut hs = HotSwap::new(Box::new(Const(0.5)), 10.0, SR);
    let out = hs.next(&ctx(0));
    assert!((out - 0.5).abs() < 1e-6);
}

#[test]
fn hotswap_crossfade_transitions() {
    let mut hs = HotSwap::new(Box::new(Const(0.0)), 10.0, SR);
    // Initial: outputs 0.0
    assert!(hs.next(&ctx(0)).abs() < 1e-6);

    // Swap to 1.0
    hs.swap(Box::new(Const(1.0)));
    assert!(hs.is_crossfading());

    // During crossfade, output ramps from 0 toward 1.
    let mut last = 0.0;
    for tick in 1..500 {
        let v = hs.next(&ctx(tick));
        assert!(v >= last - 1e-6, "should be monotonically rising during crossfade");
        last = v;
    }

    // After crossfade completes, should be at ~1.0.
    assert!((last - 1.0).abs() < 0.01, "should converge to 1.0, got {last}");
    assert!(!hs.is_crossfading());
}

#[test]
fn hotswap_instant_crossfade() {
    let mut hs = HotSwap::new(Box::new(Const(0.0)), 0.0, SR);
    hs.swap(Box::new(Const(1.0)));
    // With 0ms crossfade, should jump immediately.
    let v = hs.next(&ctx(0));
    assert!((v - 1.0).abs() < 1e-6, "instant crossfade should jump, got {v}");
}

#[test]
fn hotswap_double_swap() {
    let mut hs = HotSwap::new(Box::new(Const(0.0)), 10.0, SR);
    hs.swap(Box::new(Const(0.5)));

    // Partially through crossfade.
    for tick in 0..100 {
        hs.next(&ctx(tick));
    }

    // Swap again before the first completes.
    hs.swap(Box::new(Const(1.0)));

    // Should now be crossfading from ~0.5 to 1.0.
    let mut last = hs.next(&ctx(100));
    for tick in 101..1000 {
        last = hs.next(&ctx(tick));
    }
    assert!(
        (last - 1.0).abs() < 0.01,
        "double swap should end at 1.0, got {last}"
    );
}

#[test]
fn hotswap_no_swap_stays_stable() {
    let mut hs = HotSwap::new(Box::new(Const(0.42)), 10.0, SR);
    for tick in 0..10000 {
        let v = hs.next(&ctx(tick));
        assert!(
            (v - 0.42).abs() < 1e-6,
            "no-swap should stay constant, got {v}"
        );
    }
    assert!(!hs.is_crossfading());
}
