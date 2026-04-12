use crate::signal::{AudioContext, Signal};

/// Fixed-size voice pool, allocated once before the audio stream starts.
///
/// `N` voices are pre-allocated in an array. Each slot is `Option<S>` —
/// `Some` when a voice is active, `None` when free. Voice stealing
/// (oldest-first by default) is handled at note-on time, never inside the
/// audio callback's hot path.
pub struct VoicePool<S: Signal, const N: usize> {
    voices: [Option<S>; N],
}

impl<S: Signal, const N: usize> Default for VoicePool<S, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: Signal, const N: usize> VoicePool<S, N> {
    /// Create an empty pool. All slots start as `None`.
    pub fn new() -> Self
    where
        S: Sized,
    {
        Self {
            voices: core::array::from_fn(|_| None),
        }
    }

    /// Activate a voice in the first free slot.
    /// Returns `Some(index)` on success, or `None` if the pool is full.
    pub fn note_on(&mut self, voice: S) -> Option<usize> {
        for (i, slot) in self.voices.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(voice);
                return Some(i);
            }
        }
        None
    }

    /// Steal the oldest (lowest-index) active voice and replace it.
    /// Returns the index of the stolen slot.
    pub fn steal_oldest(&mut self, voice: S) -> usize {
        // Oldest-first: the lowest occupied index.
        for (i, slot) in self.voices.iter_mut().enumerate() {
            if slot.is_some() {
                *slot = Some(voice);
                return i;
            }
        }
        // Pool is completely empty — just use slot 0.
        self.voices[0] = Some(voice);
        0
    }

    /// Deactivate the voice at `index`.
    pub fn note_off(&mut self, index: usize) {
        if index < N {
            self.voices[index] = None;
        }
    }

    /// Number of currently active voices.
    pub fn active_count(&self) -> usize {
        self.voices.iter().filter(|v| v.is_some()).count()
    }
}

impl<S: Signal, const N: usize> Signal for VoicePool<S, N> {
    /// Mix all active voices by summing their outputs.
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let mut sum = 0.0_f32;
        for voice in self.voices.iter_mut().flatten() {
            sum += voice.next(ctx);
        }
        sum
    }
}
