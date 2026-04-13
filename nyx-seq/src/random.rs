//! Seeded portable PRNG for deterministic generative music.
//!
//! Uses xorshift64 — fast, deterministic, and produces the same
//! sequence on all platforms. NOT cryptographically secure.

use crate::note::Note;
use crate::scale::Scale;

/// A portable seeded pseudo-random number generator (xorshift64).
pub struct Rng {
    state: u64,
}

/// Create a new seeded PRNG.
pub fn seeded(seed: u64) -> Rng {
    Rng {
        state: if seed == 0 { 1 } else { seed },
    }
}

impl Rng {
    /// Generate the next raw u64.
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    /// Generate a random f32 in [0, 1).
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Generate a random f32 in [min, max).
    pub fn next_f32_range(&mut self, min: f32, max: f32) -> f32 {
        min + self.next_f32() * (max - min)
    }

    /// Generate a random integer in [min, max] (inclusive).
    pub fn next_range(&mut self, min: i32, max: i32) -> i32 {
        if min >= max {
            return min;
        }
        let range = (max - min + 1) as u64;
        min + (self.next_u64() % range) as i32
    }

    /// Pick a random element from a slice.
    pub fn choose<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        let idx = self.next_u64() as usize % items.len();
        &items[idx]
    }

    /// Generate a random MIDI note in a given range.
    pub fn next_note(&mut self, low: Note, high: Note) -> Note {
        let midi = self.next_range(low.midi() as i32, high.midi() as i32);
        Note::from_midi(midi as u8)
    }

    /// Generate a random note that belongs to the given scale,
    /// within a MIDI range.
    pub fn next_note_in(&mut self, scale: &Scale, low: Note, high: Note) -> Note {
        let notes = scale.notes_in_range(low, high);
        if notes.is_empty() {
            return low;
        }
        self.choose(&notes).to_owned()
    }
}
