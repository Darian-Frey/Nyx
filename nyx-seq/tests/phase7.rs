use nyx_core::AudioContext;
use nyx_seq::{Euclid, Note, Pattern, Scale, Sequence, clock, seeded};

const SR: f32 = 44100.0;

fn ctx(tick: u64) -> AudioContext {
    AudioContext {
        sample_rate: SR,
        tick,
    }
}

// ===================== Pattern basics =====================

#[test]
fn pattern_new_and_len() {
    let p = Pattern::new(&[1, 2, 3]);
    assert_eq!(p.len(), 3);
    assert!(!p.is_empty());
}

#[test]
fn pattern_step_wraps() {
    let p = Pattern::new(&[10, 20, 30]);
    assert_eq!(*p.step(0), 10);
    assert_eq!(*p.step(2), 30);
    assert_eq!(*p.step(3), 10); // wraps
    assert_eq!(*p.step(5), 30);
}

#[test]
fn pattern_reverse() {
    let p = Pattern::new(&[1, 2, 3, 4]);
    assert_eq!(p.reverse().steps(), &[4, 3, 2, 1]);
}

#[test]
fn pattern_retrograde_is_reverse() {
    let p = Pattern::new(&[1, 2, 3]);
    assert_eq!(p.retrograde(), p.reverse());
}

#[test]
fn pattern_concat() {
    let a = Pattern::new(&[1, 2]);
    let b = Pattern::new(&[3, 4]);
    assert_eq!(a.concat(&b).steps(), &[1, 2, 3, 4]);
}

#[test]
fn pattern_interleave_same_len() {
    let a = Pattern::new(&[1, 3, 5]);
    let b = Pattern::new(&[2, 4, 6]);
    assert_eq!(a.interleave(&b).steps(), &[1, 2, 3, 4, 5, 6]);
}

#[test]
fn pattern_interleave_different_len() {
    let a = Pattern::new(&[1, 2]);
    let b = Pattern::new(&[10, 20, 30]);
    let result = a.interleave(&b);
    // a cycles: 1, 2, 1; b: 10, 20, 30 → [1, 10, 2, 20, 1, 30]
    assert_eq!(result.steps(), &[1, 10, 2, 20, 1, 30]);
}

#[test]
fn pattern_rotate_right() {
    let p = Pattern::new(&[1, 2, 3, 4]);
    assert_eq!(p.rotate(1).steps(), &[4, 1, 2, 3]);
}

#[test]
fn pattern_rotate_left() {
    let p = Pattern::new(&[1, 2, 3, 4]);
    assert_eq!(p.rotate(-1).steps(), &[2, 3, 4, 1]);
}

#[test]
fn pattern_rotate_zero() {
    let p = Pattern::new(&[1, 2, 3]);
    assert_eq!(p.rotate(0), p);
}

#[test]
fn pattern_rotate_full_cycle() {
    let p = Pattern::new(&[1, 2, 3]);
    assert_eq!(p.rotate(3), p); // full rotation = identity
}

// ===================== Pattern<Note> invert =====================

#[test]
fn note_pattern_invert() {
    // C4(60), E4(64), G4(67) → invert around C4 → C4(60), Ab3(56), F3(53)
    let p = Pattern::new(&[Note::C4, Note::E4, Note::G4]);
    let inv = p.invert();
    let midi: Vec<u8> = inv.steps().iter().map(|n| n.midi()).collect();
    assert_eq!(midi, vec![60, 56, 53]);
}

// ===================== Pattern<f32> invert =====================

#[test]
fn f32_pattern_invert() {
    let p = Pattern::new(&[0.0, 1.0, 2.0]);
    let inv = p.invert();
    // axis=0.0, inversion: [0, -1, -2]
    assert_eq!(inv.steps(), &[0.0, -1.0, -2.0]);
}

// ===================== Pattern<bool> =====================

#[test]
fn bool_pattern_hits() {
    let p = Pattern::new(&[true, false, true, false, true]);
    assert_eq!(p.hits(), 3);
}

// ===================== Euclidean rhythm tests =====================

#[test]
fn euclid_3_8_is_tresillo() {
    let p = Euclid::generate(3, 8);
    assert_eq!(p.len(), 8);
    assert_eq!(p.hits(), 3);
    // Classic tresillo: x..x..x.
    let steps: Vec<bool> = p.steps().to_vec();
    assert_eq!(
        steps,
        vec![true, false, false, true, false, false, true, false]
    );
}

#[test]
fn euclid_4_16() {
    let p = Euclid::generate(4, 16);
    assert_eq!(p.len(), 16);
    assert_eq!(p.hits(), 4);
    // Should be evenly spaced: every 4th step.
    for (i, &b) in p.steps().iter().enumerate() {
        assert_eq!(b, i % 4 == 0, "step {i}: expected {}, got {b}", i % 4 == 0);
    }
}

#[test]
fn euclid_0_hits() {
    let p = Euclid::generate(0, 8);
    assert_eq!(p.len(), 8);
    assert_eq!(p.hits(), 0);
}

#[test]
fn euclid_all_hits() {
    let p = Euclid::generate(8, 8);
    assert_eq!(p.hits(), 8);
}

#[test]
fn euclid_more_hits_than_steps() {
    let p = Euclid::generate(10, 8);
    assert_eq!(p.len(), 8);
    assert_eq!(p.hits(), 8); // capped
}

#[test]
fn euclid_1_step() {
    let p = Euclid::generate(1, 1);
    assert_eq!(p.steps(), &[true]);
}

#[test]
fn euclid_5_8_is_cinquillo() {
    let p = Euclid::generate(5, 8);
    assert_eq!(p.len(), 8);
    assert_eq!(p.hits(), 5);
}

#[test]
fn euclid_rotate() {
    let p = Euclid::generate(3, 8);
    let rotated = p.rotate(1);
    assert_eq!(rotated.len(), 8);
    assert_eq!(rotated.hits(), 3);
    // First step should now be false (shifted right by 1).
    assert!(!rotated.steps()[0]);
}

#[test]
fn euclid_0_steps() {
    let p = Euclid::generate(3, 0);
    assert!(p.is_empty());
}

// ===================== Seeded PRNG tests =====================

#[test]
fn rng_deterministic() {
    let mut a = seeded(42);
    let mut b = seeded(42);
    for _ in 0..1000 {
        assert_eq!(a.next_u64(), b.next_u64());
    }
}

#[test]
fn rng_different_seeds_differ() {
    let mut a = seeded(42);
    let mut b = seeded(43);
    // Very unlikely to produce the same sequence.
    let mut same = true;
    for _ in 0..100 {
        if a.next_u64() != b.next_u64() {
            same = false;
            break;
        }
    }
    assert!(!same);
}

#[test]
fn rng_f32_in_range() {
    let mut rng = seeded(123);
    for _ in 0..10000 {
        let v = rng.next_f32();
        assert!((0.0..1.0).contains(&v), "f32 out of range: {v}");
    }
}

#[test]
fn rng_range() {
    let mut rng = seeded(99);
    for _ in 0..10000 {
        let v = rng.next_range(10, 20);
        assert!((10..=20).contains(&v), "range out of bounds: {v}");
    }
}

#[test]
fn rng_choose() {
    let items = [10, 20, 30, 40, 50];
    let mut rng = seeded(7);
    for _ in 0..1000 {
        let v = rng.choose(&items);
        assert!(items.contains(v));
    }
}

#[test]
fn rng_next_note() {
    let mut rng = seeded(55);
    for _ in 0..1000 {
        let n = rng.next_note(Note::C4, Note::C5);
        assert!(n.midi() >= 60 && n.midi() <= 72, "note out of range: {n}");
    }
}

#[test]
fn rng_next_note_in_scale() {
    let scale = Scale::major("C");
    let c_major_midi: Vec<u8> = scale
        .notes_in_range(Note::C4, Note::B4)
        .iter()
        .map(|n| n.midi())
        .collect();
    let mut rng = seeded(77);
    for _ in 0..1000 {
        let n = rng.next_note_in(&scale, Note::C4, Note::B4);
        assert!(
            c_major_midi.contains(&n.midi()),
            "note {} not in C major",
            n
        );
    }
}

// ===================== Step sequencer tests =====================

#[test]
fn sequencer_triggers_on_beat_grid() {
    let pattern = Pattern::new(&[true, false, true, false]);
    let mut seq = Sequence::new(pattern, 0.25); // sixteenth notes
    let mut clk = clock::clock(120.0);

    let samples_per_16th = (SR * 60.0 / 120.0 / 4.0) as u64; // ~5512

    let mut triggers = Vec::new();
    // Run exactly 4 sixteenth notes worth of samples (no margin overflow).
    for tick in 0..(samples_per_16th * 4) {
        let c = ctx(tick);
        let clock_state = clk.tick(&c);
        let event = seq.tick(&clock_state);
        if event.triggered {
            triggers.push((event.step, event.value));
        }
    }

    // Should have triggered 4 times (one per sixteenth note grid).
    assert_eq!(
        triggers.len(),
        4,
        "expected 4 triggers, got {}: {:?}",
        triggers.len(),
        triggers
    );
    // Values cycle through the pattern: true, false, true, false.
    assert!(triggers[0].1);
    assert!(!triggers[1].1);
    assert!(triggers[2].1);
    assert!(!triggers[3].1);
}

#[test]
fn sequencer_reset() {
    let pattern = Pattern::new(&[10, 20, 30]);
    let mut seq = Sequence::new(pattern, 1.0);
    let mut clk = clock::clock(120.0);

    // Advance a bit.
    let samples = (SR * 60.0 / 120.0) as u64;
    for tick in 0..samples {
        let c = ctx(tick);
        let state = clk.tick(&c);
        seq.tick(&state);
    }

    seq.reset();
    clk.reset();
    let c = ctx(0);
    let state = clk.tick(&c);
    let event = seq.tick(&state);
    assert_eq!(event.step, 0);
    assert!(event.triggered);
}

#[test]
fn sequencer_with_note_pattern() {
    let notes = Pattern::new(&[Note::C4, Note::E4, Note::G4]);
    let mut seq = Sequence::new(notes, 1.0); // quarter notes
    let mut clk = clock::clock(120.0);

    let samples_per_beat = (SR * 60.0 / 120.0) as u64;
    let mut triggered_notes = Vec::new();

    for tick in 0..(samples_per_beat * 3) {
        let c = ctx(tick);
        let state = clk.tick(&c);
        let event = seq.tick(&state);
        if event.triggered {
            triggered_notes.push(event.value);
        }
    }

    assert_eq!(triggered_notes.len(), 3);
    assert_eq!(triggered_notes[0], Note::C4);
    assert_eq!(triggered_notes[1], Note::E4);
    assert_eq!(triggered_notes[2], Note::G4);
}

#[test]
fn sequencer_with_euclidean_pattern() {
    let euclid = Euclid::generate(3, 8);
    let mut seq = Sequence::new(euclid, 0.25); // sixteenths
    let mut clk = clock::clock(120.0);

    let samples_per_16th = (SR * 60.0 / 120.0 / 4.0) as u64;
    let mut hit_count = 0;

    for tick in 0..(samples_per_16th * 8) {
        let c = ctx(tick);
        let state = clk.tick(&c);
        let event = seq.tick(&state);
        if event.triggered && event.value {
            hit_count += 1;
        }
    }

    assert_eq!(hit_count, 3, "tresillo should trigger 3 hits in 8 steps");
}
