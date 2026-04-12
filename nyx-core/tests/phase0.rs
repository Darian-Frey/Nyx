use nyx_core::{AudioContext, Param, Signal, VoicePool};

fn ctx(tick: u64) -> AudioContext {
    AudioContext {
        sample_rate: 44100.0,
        tick,
    }
}

// ---------- Signal trait basics ----------

struct ConstSig(f32);

impl Signal for ConstSig {
    fn next(&mut self, _ctx: &AudioContext) -> f32 {
        self.0
    }
}

#[test]
fn signal_next_returns_value() {
    let mut sig = ConstSig(0.5);
    assert_eq!(sig.next(&ctx(0)), 0.5);
    assert_eq!(sig.next(&ctx(1)), 0.5);
}

// ---------- Closure as Signal ----------

#[test]
fn closure_implements_signal() {
    let mut sig = |ctx: &AudioContext| ctx.tick as f32 * 0.01;
    assert!((sig.next(&ctx(0)) - 0.0).abs() < f32::EPSILON);
    assert!((sig.next(&ctx(100)) - 1.0).abs() < f32::EPSILON);
}

// ---------- Signal is Send ----------

#[test]
fn signal_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<ConstSig>();
}

// ---------- AudioContext fields ----------

#[test]
fn audio_context_carries_sample_rate_and_tick() {
    let c = ctx(42);
    assert_eq!(c.sample_rate, 44100.0);
    assert_eq!(c.tick, 42);
}

// ---------- Param ----------

#[test]
fn param_static_returns_constant() {
    let mut p: Param<ConstSig> = Param::Static(0.75);
    assert_eq!(p.next(&ctx(0)), 0.75);
    assert_eq!(p.next(&ctx(1)), 0.75);
}

#[test]
fn param_modulated_delegates_to_signal() {
    let counter = CounterSig(0);
    let mut p = Param::Modulated(counter);
    assert_eq!(p.next(&ctx(0)), 0.0);
    assert_eq!(p.next(&ctx(1)), 1.0);
    assert_eq!(p.next(&ctx(2)), 2.0);
}

struct CounterSig(u32);

impl Signal for CounterSig {
    fn next(&mut self, _ctx: &AudioContext) -> f32 {
        let v = self.0 as f32;
        self.0 += 1;
        v
    }
}

#[test]
fn param_from_f32() {
    let p: Param<nyx_core::param::ConstSignal> = Param::from(440.0);
    match p {
        Param::Static(v) => assert_eq!(v, 440.0),
        _ => panic!("expected Static"),
    }
}

// ---------- VoicePool ----------

#[test]
fn voice_pool_starts_empty() {
    let pool: VoicePool<ConstSig, 4> = VoicePool::new();
    assert_eq!(pool.active_count(), 0);
}

#[test]
fn voice_pool_note_on_fills_slots() {
    let mut pool: VoicePool<ConstSig, 4> = VoicePool::new();
    assert_eq!(pool.note_on(ConstSig(1.0)), Some(0));
    assert_eq!(pool.note_on(ConstSig(2.0)), Some(1));
    assert_eq!(pool.active_count(), 2);
}

#[test]
fn voice_pool_note_on_returns_none_when_full() {
    let mut pool: VoicePool<ConstSig, 2> = VoicePool::new();
    assert!(pool.note_on(ConstSig(1.0)).is_some());
    assert!(pool.note_on(ConstSig(2.0)).is_some());
    assert_eq!(pool.note_on(ConstSig(3.0)), None);
}

#[test]
fn voice_pool_note_off_frees_slot() {
    let mut pool: VoicePool<ConstSig, 2> = VoicePool::new();
    pool.note_on(ConstSig(1.0));
    pool.note_off(0);
    assert_eq!(pool.active_count(), 0);
    // Slot can be reused.
    assert_eq!(pool.note_on(ConstSig(2.0)), Some(0));
}

#[test]
fn voice_pool_steal_oldest_replaces_first_active() {
    let mut pool: VoicePool<ConstSig, 2> = VoicePool::new();
    pool.note_on(ConstSig(1.0));
    pool.note_on(ConstSig(2.0));
    let stolen = pool.steal_oldest(ConstSig(99.0));
    assert_eq!(stolen, 0);
    // The stolen slot now holds the new voice.
    assert_eq!(pool.next(&ctx(0)), 99.0 + 2.0);
}

#[test]
fn voice_pool_mixes_active_voices() {
    let mut pool: VoicePool<ConstSig, 4> = VoicePool::new();
    pool.note_on(ConstSig(0.25));
    pool.note_on(ConstSig(0.25));
    pool.note_on(ConstSig(0.5));
    let mixed = pool.next(&ctx(0));
    assert!((mixed - 1.0).abs() < f32::EPSILON);
}
