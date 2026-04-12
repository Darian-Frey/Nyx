use nyx_core::{AudioContext, Param, Signal, SignalExt};
use nyx_core::param::{ConstSignal, IntoParam};

fn ctx(tick: u64) -> AudioContext {
    AudioContext {
        sample_rate: 44100.0,
        tick,
    }
}

struct Const(f32);

impl Signal for Const {
    fn next(&mut self, _ctx: &AudioContext) -> f32 {
        self.0
    }
}

// ---------- .boxed() ----------

#[test]
fn boxed_signal_works() {
    let mut sig = Const(0.75).boxed();
    assert_eq!(sig.next(&ctx(0)), 0.75);
}

#[test]
fn boxed_heterogeneous_vec() {
    let mut signals: Vec<Box<dyn Signal>> = vec![
        Const(0.25).boxed(),
        Const(0.5).boxed(),
    ];
    let sum: f32 = signals.iter_mut().map(|s| s.next(&ctx(0))).sum();
    assert!((sum - 0.75).abs() < f32::EPSILON);
}

// ---------- .amp() ----------

#[test]
fn amp_with_f32() {
    let mut sig = Const(1.0).amp(0.5);
    assert!((sig.next(&ctx(0)) - 0.5).abs() < f32::EPSILON);
}

#[test]
fn amp_with_signal() {
    let mut sig = Const(2.0).amp(Const(0.25));
    assert!((sig.next(&ctx(0)) - 0.5).abs() < f32::EPSILON);
}

// ---------- .add() ----------

#[test]
fn add_two_signals() {
    let mut sig = Const(0.3).add(Const(0.2));
    assert!((sig.next(&ctx(0)) - 0.5).abs() < f32::EPSILON);
}

// ---------- .mul() ----------

#[test]
fn mul_two_signals() {
    let mut sig = Const(0.5).mul(Const(0.4));
    assert!((sig.next(&ctx(0)) - 0.2).abs() < f32::EPSILON);
}

// ---------- .mix() ----------

#[test]
fn mix_zero_is_all_first() {
    let mut sig = Const(1.0).mix(Const(0.0), 0.0);
    assert!((sig.next(&ctx(0)) - 1.0).abs() < f32::EPSILON);
}

#[test]
fn mix_one_is_all_second() {
    let mut sig = Const(1.0).mix(Const(0.0), 1.0);
    assert!(sig.next(&ctx(0)).abs() < f32::EPSILON);
}

#[test]
fn mix_half() {
    let mut sig = Const(1.0).mix(Const(0.0), 0.5);
    assert!((sig.next(&ctx(0)) - 0.5).abs() < f32::EPSILON);
}

// ---------- .pan() ----------

#[test]
fn pan_center_preserves_level() {
    let mut sig = Const(1.0).pan(0.0);
    // Mono fold: left + right, center pan gives 0.5 + 0.5 = 1.0
    assert!((sig.next(&ctx(0)) - 1.0).abs() < f32::EPSILON);
}

#[test]
fn pan_hard_left() {
    let mut pan = Const(1.0).pan(-1.0);
    let (l, r) = pan.next_stereo(&ctx(0));
    assert!((l - 1.0).abs() < f32::EPSILON);
    assert!(r.abs() < f32::EPSILON);
}

#[test]
fn pan_hard_right() {
    let mut pan = Const(1.0).pan(1.0);
    let (l, r) = pan.next_stereo(&ctx(0));
    assert!(l.abs() < f32::EPSILON);
    assert!((r - 1.0).abs() < f32::EPSILON);
}

// ---------- .clip() ----------

#[test]
fn clip_clamps_positive() {
    let mut sig = Const(2.0).clip(0.5);
    assert!((sig.next(&ctx(0)) - 0.5).abs() < f32::EPSILON);
}

#[test]
fn clip_clamps_negative() {
    let mut sig = Const(-2.0).clip(0.5);
    assert!((sig.next(&ctx(0)) - (-0.5)).abs() < f32::EPSILON);
}

#[test]
fn clip_passes_within_range() {
    let mut sig = Const(0.3).clip(0.5);
    assert!((sig.next(&ctx(0)) - 0.3).abs() < f32::EPSILON);
}

// ---------- .soft_clip() ----------

#[test]
fn soft_clip_tanh_saturation() {
    let mut sig = Const(1.0).soft_clip(1.0);
    let out = sig.next(&ctx(0));
    // tanh(1.0) ≈ 0.7616
    assert!((out - 1.0_f32.tanh()).abs() < 1e-6);
}

// ---------- .offset() ----------

#[test]
fn offset_adds_dc() {
    let mut sig = Const(0.5).offset(0.25);
    assert!((sig.next(&ctx(0)) - 0.75).abs() < f32::EPSILON);
}

// ---------- Chaining ----------

#[test]
fn chain_multiple_combinators() {
    // 1.0 * 0.5 = 0.5, then clip to 0.3
    let mut sig = Const(1.0).amp(0.5).clip(0.3);
    assert!((sig.next(&ctx(0)) - 0.3).abs() < f32::EPSILON);
}

#[test]
fn chain_with_boxed() {
    let mut sig = Const(1.0).amp(0.5).add(Const(0.1)).boxed();
    assert!((sig.next(&ctx(0)) - 0.6).abs() < f32::EPSILON);
}

// ---------- IntoParam ----------

#[test]
fn into_param_from_f32() {
    let p = 440.0_f32.into_param();
    match p {
        Param::Static(v) => assert_eq!(v, 440.0),
        _ => panic!("expected Static"),
    }
}

#[test]
fn into_param_from_signal() {
    let p = Const(1.0).into_param();
    match p {
        Param::Modulated(_) => {}
        _ => panic!("expected Modulated"),
    }
}

// ---------- Param<S> From<f32> still works ----------

#[test]
fn param_from_f32_conversion() {
    let p: Param<ConstSignal> = Param::from(220.0);
    match p {
        Param::Static(v) => assert_eq!(v, 220.0),
        _ => panic!("expected Static"),
    }
}
