//! Step sequencer driven by a musical clock.
//!
//! A `Sequence` holds a pattern of values and advances through them
//! on a beat grid. Each clock tick, it reports whether the current
//! step changed (a new trigger) and what value is active.

use crate::clock::ClockState;
use crate::pattern::Pattern;

/// A step sequencer that advances through a pattern on a beat grid.
///
/// `T` is the type of each step (bool for triggers, Note for melodies,
/// f32 for parameter sequences, etc.).
pub struct Sequence<T: Clone> {
    pattern: Pattern<T>,
    grid: f32, // beat subdivision (e.g. 0.25 = sixteenth notes)
    current_step: usize,
    last_grid_index: i64,
}

/// The result of advancing the sequencer by one clock tick.
#[derive(Debug, Clone)]
pub struct StepEvent<T: Clone> {
    /// The value at the current step.
    pub value: T,
    /// The step index within the pattern.
    pub step: usize,
    /// Whether this tick is a new trigger (the step just changed).
    pub triggered: bool,
}

impl<T: Clone> Sequence<T> {
    /// Create a new sequencer from a pattern and a beat grid size.
    ///
    /// `grid` is in beats: 1.0 = quarter note, 0.5 = eighth, 0.25 = sixteenth.
    pub fn new(pattern: Pattern<T>, grid: f32) -> Self {
        Sequence {
            pattern,
            grid,
            current_step: 0,
            last_grid_index: -1,
        }
    }

    /// Advance the sequencer using the current clock state.
    ///
    /// Call this once per sample. Returns a `StepEvent` with the current
    /// value and whether a new step was triggered.
    pub fn tick(&mut self, clock: &ClockState) -> StepEvent<T> {
        if self.pattern.is_empty() || self.grid <= 0.0 {
            return StepEvent {
                value: self.pattern.step(0).clone(),
                step: 0,
                triggered: false,
            };
        }

        let grid_index = (clock.beat / self.grid).floor() as i64;
        let triggered = grid_index != self.last_grid_index;

        if triggered {
            self.last_grid_index = grid_index;
            self.current_step = (grid_index as usize) % self.pattern.len();
        }

        StepEvent {
            value: self.pattern.step(self.current_step).clone(),
            step: self.current_step,
            triggered,
        }
    }

    /// Reset the sequencer to the first step.
    pub fn reset(&mut self) {
        self.current_step = 0;
        self.last_grid_index = -1;
    }

    /// The underlying pattern.
    pub fn pattern(&self) -> &Pattern<T> {
        &self.pattern
    }

    /// The beat grid size.
    pub fn grid(&self) -> f32 {
        self.grid
    }
}
