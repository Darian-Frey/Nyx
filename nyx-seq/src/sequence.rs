//! Step sequencer driven by a musical clock.
//!
//! A `Sequence` holds a pattern of values and advances through them
//! on a beat grid. Each clock tick, it reports whether the current
//! step changed (a new trigger) and what value is active.
//!
//! # TidalCycles-style modifiers
//!
//! On top of the basic step-advance behaviour, `Sequence` supports
//! probabilistic and conditional pattern transformations:
//!
//! - [`Sequence::prob`] — per-step dice roll suppresses triggers
//! - [`Sequence::degrade`] — TidalCycles alias for `.prob(1.0 - amount)`
//! - [`Sequence::every`] — every N cycles, switch to a transformed pattern
//! - [`Sequence::sometimes`] — per-cycle coin flip picks which pattern
//! - [`Sequence::seed`] — set the PRNG seed for reproducibility
//!
//! All randomness uses a seeded xorshift64 PRNG, so `.seed(42)` will
//! always produce the same output — important for live-diff reloads.

use crate::clock::ClockState;
use crate::pattern::Pattern;
use crate::random::{Rng, seeded};

/// Default PRNG seed when `.seed(...)` is not called.
const DEFAULT_SEED: u64 = 0xBA55_A557;

/// A step sequencer that advances through a pattern on a beat grid.
///
/// `T` is the type of each step (bool for triggers, Note for melodies,
/// f32 for parameter sequences, etc.).
pub struct Sequence<T: Clone> {
    pattern: Pattern<T>,
    grid: f32,
    current_step: usize,
    last_grid_index: i64,
    /// Per-step trigger probability in `[0, 1]`. `1.0` = all triggers fire.
    prob: f32,
    rng: Rng,
    alt: Option<AltConfig<T>>,
    cycle_counter: u32,
    using_alt: bool,
}

/// Alternate-pattern configuration for `.every()` and `.sometimes()`.
enum AltConfig<T: Clone> {
    /// Every `n` cycles, use this pattern instead of the base.
    Every { n: u32, pattern: Pattern<T> },
    /// Per-cycle coin flip at probability `p` picks this pattern.
    Sometimes { p: f32, pattern: Pattern<T> },
}

/// The result of advancing the sequencer by one clock tick.
#[derive(Debug, Clone)]
pub struct StepEvent<T: Clone> {
    /// The value at the current step.
    pub value: T,
    /// The step index within the pattern.
    pub step: usize,
    /// Whether this tick is a new trigger (the step just changed and
    /// passed the probability gate).
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
            prob: 1.0,
            rng: seeded(DEFAULT_SEED),
            alt: None,
            cycle_counter: 0,
            using_alt: false,
        }
    }

    /// Per-step trigger probability. `1.0` = always fire (default);
    /// `0.5` = half the triggers drop; `0.0` = silence. Clamped to `[0, 1]`.
    pub fn prob(mut self, probability: f32) -> Self {
        self.prob = probability.clamp(0.0, 1.0);
        self
    }

    /// TidalCycles alias: `seq.degrade(0.25)` is equivalent to
    /// `seq.prob(0.75)` — drops 25% of triggers randomly.
    pub fn degrade(self, amount: f32) -> Self {
        let amount = amount.clamp(0.0, 1.0);
        self.prob(1.0 - amount)
    }

    /// Set the PRNG seed used for probability and conditional decisions.
    /// Same seed → identical output across runs (important for
    /// reproducible live-diff reloads).
    pub fn seed(mut self, seed: u64) -> Self {
        self.rng = seeded(seed);
        self
    }

    /// Every `n` cycles of the pattern, switch to the pattern returned
    /// by `f` for that cycle. Resets to the base pattern on the cycle
    /// after.
    ///
    /// ```ignore
    /// // Every 4 bars, play the pattern in reverse.
    /// seq.every(4, |p| p.reverse())
    /// ```
    pub fn every<F: FnOnce(Pattern<T>) -> Pattern<T>>(mut self, n: u32, f: F) -> Self {
        if n > 0 {
            let alt_pattern = f(self.pattern.clone());
            self.alt = Some(AltConfig::Every {
                n,
                pattern: alt_pattern,
            });
        }
        self
    }

    /// Per-cycle, roll a dice at probability `p` — on success, use the
    /// pattern returned by `f` for that cycle.
    ///
    /// ```ignore
    /// // 30% of cycles, rotate the pattern by 2.
    /// seq.sometimes(0.3, |p| p.rotate(2))
    /// ```
    pub fn sometimes<F: FnOnce(Pattern<T>) -> Pattern<T>>(mut self, p: f32, f: F) -> Self {
        let alt_pattern = f(self.pattern.clone());
        self.alt = Some(AltConfig::Sometimes {
            p: p.clamp(0.0, 1.0),
            pattern: alt_pattern,
        });
        self
    }

    /// Advance the sequencer using the current clock state.
    ///
    /// Call this once per sample. Returns a `StepEvent` with the current
    /// value and whether a new step was triggered (after probability gate).
    pub fn tick(&mut self, clock: &ClockState) -> StepEvent<T> {
        let pattern_len = self.active_pattern().len();
        if pattern_len == 0 || self.grid <= 0.0 {
            return StepEvent {
                value: self.pattern.step(0).clone(),
                step: 0,
                triggered: false,
            };
        }

        let grid_index = (clock.beat / self.grid).floor() as i64;
        let is_first = self.last_grid_index == -1;
        let step_advanced = grid_index != self.last_grid_index;
        let mut effective_trigger = step_advanced;

        if step_advanced {
            self.last_grid_index = grid_index;
            let abs_index = grid_index.max(0) as usize;
            let new_cycle = (abs_index / pattern_len) as u32;

            // Cycle boundary: decide if this cycle uses the alt pattern.
            if is_first || new_cycle != self.cycle_counter {
                self.cycle_counter = new_cycle;
                self.update_alt_decision();
            }

            self.current_step = abs_index % pattern_len;

            // Probability gate: per-step coin flip.
            if self.prob < 1.0 && self.rng.next_f32() >= self.prob {
                effective_trigger = false;
            }
        }

        StepEvent {
            value: self.active_pattern().step(self.current_step).clone(),
            step: self.current_step,
            triggered: effective_trigger,
        }
    }

    /// Reset the sequencer to the first step.
    pub fn reset(&mut self) {
        self.current_step = 0;
        self.last_grid_index = -1;
        self.cycle_counter = 0;
        self.using_alt = false;
    }

    /// The underlying base pattern (not the alternate, if active).
    pub fn pattern(&self) -> &Pattern<T> {
        &self.pattern
    }

    /// The beat grid size.
    pub fn grid(&self) -> f32 {
        self.grid
    }

    /// Returns `true` if the current cycle is using the alternate pattern
    /// (set by `.every()` or `.sometimes()`).
    pub fn is_using_alt(&self) -> bool {
        self.using_alt
    }

    fn active_pattern(&self) -> &Pattern<T> {
        if self.using_alt
            && let Some(alt) = &self.alt
        {
            return match alt {
                AltConfig::Every { pattern, .. } => pattern,
                AltConfig::Sometimes { pattern, .. } => pattern,
            };
        }
        &self.pattern
    }

    fn update_alt_decision(&mut self) {
        self.using_alt = match &self.alt {
            Some(AltConfig::Every { n, .. }) if *n > 0 => {
                // Use alt on every N-th cycle: cycles 0..n-2 use base,
                // cycle n-1 uses alt, then counter wraps.
                (self.cycle_counter + 1).is_multiple_of(*n)
            }
            Some(AltConfig::Sometimes { p, .. }) => self.rng.next_f32() < *p,
            _ => false,
        };
    }
}
