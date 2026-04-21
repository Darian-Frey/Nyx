//! Bus / mixer — a collection of signals summed into a single output.
//!
//! A `Bus` owns a `Vec<Box<dyn Signal>>` of sources and a post-sum gain.
//! It is itself a `Signal`, so it composes seamlessly with the rest of
//! the fluent API: `drums_bus.compress(-6.0, 4.0).freeverb()` applies
//! a bus compressor and a shared reverb.
//!
//! Construction is builder-style and happens **before** the audio stream
//! starts — `.add()` heap-allocates the `Box<dyn Signal>`, but no
//! allocation happens once the bus is running.
//!
//! ```ignore
//! let drums = Bus::new()
//!     .add(kick)
//!     .add(snare.amp(0.7))
//!     .add(hat.amp(0.4))
//!     .gain(0.9);
//!
//! let mix = Bus::new()
//!     .add(drums.compress(-6.0, 4.0))
//!     .add(bass)
//!     .add(pad.amp(0.4).freeverb().wet(0.5))
//!     .soft_clip(1.1);
//!
//! play(mix).unwrap();
//! ```
//!
//! **Send / return pattern.** Nyx does not provide a true multi-reader
//! send bus (that would require a per-sample fan-out buffer). Instead,
//! express sends by routing an `amp`-scaled copy of a source into a
//! dedicated effect bus:
//!
//! ```ignore
//! // 30% of the lead goes to a reverb bus; the dry 100% stays in the mix.
//! let lead = make_lead();
//! let reverb_send = make_lead().amp(0.3).freeverb().wet(1.0);
//! let mix = Bus::new().add(lead).add(reverb_send);
//! ```
//!
//! Because `Signal` is not `Clone`, the send is built from a second
//! instance of the source. For identical output, pair with a shared
//! trigger via `OscParam` / MIDI / sequencer state.

use crate::signal::{AudioContext, Signal};

/// Sum of N signals with a post-sum gain.
///
/// Exposed as a `Signal`; compose with `SignalExt` combinators freely.
pub struct Bus {
    sources: Vec<Box<dyn Signal>>,
    gain: f32,
}

impl Bus {
    /// Create an empty bus.
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
            gain: 1.0,
        }
    }

    /// Create an empty bus with capacity pre-reserved for `n` sources,
    /// avoiding `Vec` growth reallocations during `.add()` calls.
    pub fn with_capacity(n: usize) -> Self {
        Self {
            sources: Vec::with_capacity(n),
            gain: 1.0,
        }
    }

    /// Append a source to the bus. Consumes and returns `self` for
    /// fluent construction.
    ///
    /// Heap-allocates one `Box<dyn Signal>` per call. Do this before
    /// `play()`; the audio callback never allocates.
    #[allow(clippy::should_implement_trait)] // named for the mixer/DAW idiom
    pub fn add<S: Signal + 'static>(mut self, source: S) -> Self {
        self.sources.push(Box::new(source));
        self
    }

    /// Post-sum gain applied after every source has been mixed.
    /// Defaults to `1.0`.
    pub fn gain(mut self, g: f32) -> Self {
        self.gain = g;
        self
    }

    /// Number of sources currently in the bus.
    pub fn len(&self) -> usize {
        self.sources.len()
    }

    /// Whether the bus has any sources.
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }
}

impl Default for Bus {
    fn default() -> Self {
        Self::new()
    }
}

impl Signal for Bus {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let mut sum = 0.0_f32;
        for s in &mut self.sources {
            sum += s.next(ctx);
        }
        sum * self.gain
    }

    fn next_stereo(&mut self, ctx: &AudioContext) -> (f32, f32) {
        let mut sum_l = 0.0_f32;
        let mut sum_r = 0.0_f32;
        for s in &mut self.sources {
            let (l, r) = s.next_stereo(ctx);
            sum_l += l;
            sum_r += r;
        }
        (sum_l * self.gain, sum_r * self.gain)
    }
}
