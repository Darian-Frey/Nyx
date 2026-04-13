//! Pattern type and combinators.
//!
//! A `Pattern<T>` is a finite sequence of values (notes, triggers, velocities, etc.)
//! that can be transformed with combinators like `.reverse()`, `.retrograde()`,
//! `.invert()`, `.concat()`, and `.interleave()`.

use crate::note::Note;

/// A finite sequence of values that can be transformed and iterated.
#[derive(Debug, Clone, PartialEq)]
pub struct Pattern<T: Clone> {
    steps: Vec<T>,
}

impl<T: Clone> Pattern<T> {
    /// Create a pattern from a slice of values.
    pub fn new(steps: &[T]) -> Self {
        Pattern {
            steps: steps.to_vec(),
        }
    }

    /// Create a pattern from a Vec.
    pub fn from_vec(steps: Vec<T>) -> Self {
        Pattern { steps }
    }

    /// Number of steps in the pattern.
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Whether the pattern is empty.
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Get the step at `index`, wrapping around for infinite cycling.
    pub fn step(&self, index: usize) -> &T {
        &self.steps[index % self.steps.len()]
    }

    /// Get the underlying steps as a slice.
    pub fn steps(&self) -> &[T] {
        &self.steps
    }

    /// Reverse the pattern.
    pub fn reverse(&self) -> Self {
        let mut steps = self.steps.clone();
        steps.reverse();
        Pattern { steps }
    }

    /// Retrograde — same as reverse (standard music theory term).
    pub fn retrograde(&self) -> Self {
        self.reverse()
    }

    /// Concatenate two patterns.
    pub fn concat(&self, other: &Pattern<T>) -> Self {
        let mut steps = self.steps.clone();
        steps.extend_from_slice(&other.steps);
        Pattern { steps }
    }

    /// Interleave two patterns: A[0], B[0], A[1], B[1], ...
    ///
    /// If patterns differ in length, the shorter one cycles.
    pub fn interleave(&self, other: &Pattern<T>) -> Self {
        if self.is_empty() && other.is_empty() {
            return Pattern { steps: Vec::new() };
        }
        let max_len = self.len().max(other.len());
        let mut steps = Vec::with_capacity(max_len * 2);
        for i in 0..max_len {
            if !self.is_empty() {
                steps.push(self.step(i).clone());
            }
            if !other.is_empty() {
                steps.push(other.step(i).clone());
            }
        }
        Pattern { steps }
    }

    /// Rotate the pattern by `n` steps to the right.
    /// Negative values rotate left.
    pub fn rotate(&self, n: i32) -> Self {
        if self.is_empty() {
            return self.clone();
        }
        let len = self.len() as i32;
        let shift = ((n % len) + len) % len;
        let split = self.len() - shift as usize;
        let mut steps = self.steps[split..].to_vec();
        steps.extend_from_slice(&self.steps[..split]);
        Pattern { steps }
    }
}

/// Pattern of MIDI notes can be inverted (mirror around an axis).
impl Pattern<Note> {
    /// Invert: mirror note pitches around the first note.
    ///
    /// Each note's interval from the first note is negated.
    /// E.g. [C4, E4, G4] → [C4, Ab3, F3] (intervals 0,+4,+7 → 0,-4,-7).
    pub fn invert(&self) -> Self {
        if self.is_empty() {
            return self.clone();
        }
        let axis = self.steps[0].midi() as i16;
        let steps = self
            .steps
            .iter()
            .map(|n| {
                let interval = n.midi() as i16 - axis;
                Note::from_midi((axis - interval).clamp(0, 127) as u8)
            })
            .collect();
        Pattern { steps }
    }
}

/// Pattern of f32 values can be inverted (mirror around the first value).
impl Pattern<f32> {
    /// Invert: mirror values around the first element.
    pub fn invert(&self) -> Self {
        if self.is_empty() {
            return self.clone();
        }
        let axis = self.steps[0];
        let steps = self
            .steps
            .iter()
            .map(|&v| 2.0 * axis - v)
            .collect();
        Pattern { steps }
    }
}

/// Pattern of bools (trigger patterns) — used by step sequencers and Euclidean rhythms.
impl Pattern<bool> {
    /// Count the number of active (true) steps.
    pub fn hits(&self) -> usize {
        self.steps.iter().filter(|&&b| b).count()
    }
}
