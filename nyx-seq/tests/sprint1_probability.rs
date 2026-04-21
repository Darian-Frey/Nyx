//! Sprint 1 — Probability & conditional sequencer modifiers.

use nyx_core::AudioContext;
use nyx_seq::{Euclid, Pattern, Sequence, clock};

const SR: f32 = 44100.0;

fn ctx(tick: u64) -> AudioContext {
    AudioContext {
        sample_rate: SR,
        tick,
    }
}

// ─────────────── Pattern::shuffle ───────────────

#[test]
fn shuffle_preserves_elements() {
    let p = Pattern::new(&[1, 2, 3, 4, 5, 6, 7, 8]);
    let s = p.shuffle(42);
    assert_eq!(s.len(), p.len());
    let mut sorted: Vec<i32> = s.steps().to_vec();
    sorted.sort();
    assert_eq!(sorted, vec![1, 2, 3, 4, 5, 6, 7, 8]);
}

#[test]
fn shuffle_deterministic() {
    let p = Pattern::new(&[1, 2, 3, 4, 5]);
    assert_eq!(p.shuffle(7).steps(), p.shuffle(7).steps());
}

#[test]
fn shuffle_different_seeds_different_output() {
    let p = Pattern::new(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    assert_ne!(p.shuffle(1).steps(), p.shuffle(2).steps());
}

// ─────────────── .prob() ───────────────

/// Advance the clock sample-by-sample until the sequence has seen
/// exactly `target_steps` grid boundaries. We detect boundaries
/// independently of the Sequence (so probability-suppressed steps still
/// count) and collect the values of triggers that fired.
fn run_sequence(mut seq: Sequence<bool>, bars: usize) -> Vec<bool> {
    let mut clk = clock::clock(120.0);
    let grid = seq.grid();
    let target_steps = bars * seq.pattern().len();
    let mut hits = Vec::new();
    let mut last_grid: i64 = -1;
    let mut steps_seen = 0_usize;
    let mut tick: u64 = 0;
    while steps_seen < target_steps {
        let state = clk.tick(&ctx(tick));
        let current_grid = (state.beat / grid).floor() as i64;
        let event = seq.tick(&state);
        if current_grid != last_grid {
            last_grid = current_grid;
            steps_seen += 1;
            if event.triggered && event.value {
                hits.push(true);
            }
        }
        tick += 1;
        if tick > 10_000_000 {
            break;
        }
    }
    hits
}

#[test]
fn prob_1_keeps_all_triggers() {
    let pattern = Pattern::new(&[true; 16]);
    let seq = Sequence::new(pattern, 0.25).prob(1.0);
    let hits = run_sequence(seq, 2);
    assert_eq!(hits.len(), 32, "prob=1 should fire every step");
}

#[test]
fn prob_0_suppresses_all() {
    let pattern = Pattern::new(&[true; 16]);
    let seq = Sequence::new(pattern, 0.25).prob(0.0);
    let hits = run_sequence(seq, 2);
    assert_eq!(hits.len(), 0, "prob=0 should fire zero steps");
}

#[test]
fn prob_half_drops_roughly_half() {
    let pattern = Pattern::new(&[true; 16]);
    let seq = Sequence::new(pattern, 0.25).prob(0.5).seed(12345);
    // 10 bars × 16 steps = 160 potential hits; expect ~80 with tolerance.
    let hits = run_sequence(seq, 10);
    assert!(
        (hits.len() as i32 - 80).abs() <= 20,
        "prob=0.5 should fire ~80/160 steps, got {}",
        hits.len()
    );
}

#[test]
fn degrade_is_inverse_of_prob() {
    let pattern = Pattern::new(&[true; 16]);
    let a = Sequence::new(pattern.clone(), 0.25).degrade(0.25).seed(7);
    let b = Sequence::new(pattern, 0.25).prob(0.75).seed(7);
    let hits_a = run_sequence(a, 5);
    let hits_b = run_sequence(b, 5);
    assert_eq!(
        hits_a.len(),
        hits_b.len(),
        ".degrade(0.25) should match .prob(0.75)"
    );
}

#[test]
fn seed_is_reproducible() {
    let pattern = Pattern::new(&[true; 8]);
    let a = Sequence::new(pattern.clone(), 0.25).prob(0.5).seed(99);
    let b = Sequence::new(pattern, 0.25).prob(0.5).seed(99);
    assert_eq!(run_sequence(a, 4).len(), run_sequence(b, 4).len());
}

// ─────────────── .every() ───────────────

/// Collect the value at each grid boundary, for `target_steps` steps.
fn collect_step_values<T: Clone + std::fmt::Debug>(
    mut seq: Sequence<T>,
    target_steps: usize,
) -> Vec<T> {
    let mut clk = clock::clock(120.0);
    let grid = seq.grid();
    let mut out = Vec::new();
    let mut last_grid: i64 = -1;
    let mut tick: u64 = 0;
    while out.len() < target_steps {
        let state = clk.tick(&ctx(tick));
        let current_grid = (state.beat / grid).floor() as i64;
        let event = seq.tick(&state);
        if current_grid != last_grid {
            last_grid = current_grid;
            out.push(event.value);
        }
        tick += 1;
        if tick > 10_000_000 {
            break;
        }
    }
    out
}

#[test]
fn every_4_switches_on_4th_cycle() {
    // Pattern of 4; every 4 cycles (= 16 steps), use reversed.
    let base = Pattern::new(&[1, 2, 3, 4]);
    let seq = Sequence::new(base, 0.25).every(4, |p| p.reverse());
    let got = collect_step_values(seq, 16);
    // Cycles 0,1,2 base [1,2,3,4]; cycle 3 reversed [4,3,2,1].
    assert_eq!(got, vec![1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 4, 3, 2, 1]);
}

#[test]
fn every_0_is_noop() {
    // n=0 should not set an alt pattern at all.
    let base = Pattern::new(&[1, 2, 3]);
    let seq = Sequence::new(base, 0.25).every(0, |p| p.reverse());
    // If a user asks for every(0), there's no "every zero cycles" — we
    // treat it as a no-op: base pattern plays forever.
    assert!(!seq.is_using_alt());
}

// ─────────────── .sometimes() ───────────────

#[test]
fn sometimes_0_never_uses_alt() {
    let base = Pattern::new(&[1, 2, 3, 4]);
    let seq = Sequence::new(base, 0.25).sometimes(0.0, |p| p.reverse());
    // Play 8 steps of base pattern — all should be base [1,2,3,4] cycling.
    let got = collect_step_values(seq, 8);
    assert_eq!(got, vec![1, 2, 3, 4, 1, 2, 3, 4]);
}

#[test]
fn sometimes_1_always_uses_alt() {
    let base = Pattern::new(&[1, 2, 3, 4]);
    let seq = Sequence::new(base, 0.25).sometimes(1.0, |p| p.reverse());
    // Every cycle should use reversed pattern.
    let got = collect_step_values(seq, 8);
    assert_eq!(got, vec![4, 3, 2, 1, 4, 3, 2, 1]);
}

// ─────────────── Integration with Euclid ───────────────

#[test]
fn euclid_with_probability() {
    // 8-step tresillo-ish, prob=0.5 drops half.
    let pattern = Euclid::generate(3, 8);
    let seq = Sequence::new(pattern, 0.25).prob(0.5).seed(42);
    let hits = run_sequence(seq, 10);
    // 10 cycles × 3 hits = 30 potential; expect ~15 after prob=0.5.
    assert!(
        (hits.len() as i32 - 15).abs() <= 7,
        "expected ~15, got {}",
        hits.len()
    );
}
