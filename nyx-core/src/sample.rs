//! Sample playback.
//!
//! A [`Sample`] is audio data loaded once on the main thread and
//! referenced via `Arc<[f32]>` so it can be cheaply shared across many
//! playback voices. A [`Sampler`] is the voice that plays back a
//! sample — one-shot, looped, or ping-pong — with pitch control.
//!
//! # Example
//!
//! ```ignore
//! use nyx_prelude::*;
//!
//! let kick = Sample::load("kick.wav")?;
//! play(Sampler::new(kick).pitch(1.5))?;
//! ```
//!
//! # Lifetime caveat
//!
//! `Sampler` holds an `Arc<[f32]>` clone of the original `Sample`'s
//! data. Keep **at least one reference to the `Sample`** alive for as
//! long as any `Sampler` cloned from it is playing. Otherwise the last
//! `Arc::drop` may occur on the audio thread, triggering the allocator.
//! (A "sample graveyard" that ships `Arc`s back to the main thread for
//! drop is planned for Sprint 2.)

use std::path::Path;
use std::sync::Arc;

use crate::param::{ConstSignal, IntoParam, Param};
use crate::signal::{AudioContext, Signal};

/// Errors that can occur loading a sample.
#[derive(Debug, thiserror::Error)]
pub enum SampleError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[cfg(feature = "wav")]
    #[error("wav error: {0}")]
    Hound(#[from] hound::Error),
    #[error("sample is empty")]
    Empty,
}

/// Shared, immutable audio data. Cheap to clone (refcount bump only).
///
/// Load from a WAV file with [`Sample::load`] (requires the `wav`
/// feature) or build from an in-memory buffer with [`Sample::from_buffer`].
#[derive(Clone)]
pub struct Sample {
    data: Arc<[f32]>,
    sample_rate: f32,
}

impl Sample {
    /// Build a sample from an in-memory mono f32 buffer at the given
    /// sample rate. Always available — no feature flag required.
    pub fn from_buffer(data: Vec<f32>, sample_rate: f32) -> Result<Self, SampleError> {
        if data.is_empty() {
            return Err(SampleError::Empty);
        }
        Ok(Sample {
            data: data.into_boxed_slice().into(),
            sample_rate,
        })
    }

    /// Load a WAV file. Requires the `wav` feature.
    ///
    /// Stereo WAVs are downmixed to mono at load time (average of both
    /// channels). The sample's stored `sample_rate` is taken from the
    /// file; at playback, the `Sampler` automatically adjusts for any
    /// stream/sample rate mismatch.
    #[cfg(feature = "wav")]
    pub fn load(path: impl AsRef<Path>) -> Result<Self, SampleError> {
        let mut reader = hound::WavReader::open(path.as_ref())?;
        let spec = reader.spec();
        let channels = spec.channels as usize;

        let raw: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Int => {
                // Max value that a sample of `bits_per_sample` can produce.
                let max = (1_i64 << (spec.bits_per_sample - 1)) as f32;
                reader
                    .samples::<i32>()
                    .filter_map(|s| s.ok())
                    .map(|s| s as f32 / max)
                    .collect()
            }
            hound::SampleFormat::Float => reader
                .samples::<f32>()
                .filter_map(|s| s.ok())
                .collect(),
        };

        // Downmix stereo/multi-channel to mono by averaging.
        let mono: Vec<f32> = if channels > 1 {
            raw.chunks(channels)
                .filter(|c| c.len() == channels)
                .map(|frame| frame.iter().sum::<f32>() / channels as f32)
                .collect()
        } else {
            raw
        };

        Self::from_buffer(mono, spec.sample_rate as f32)
    }

    /// Number of samples (frames — always mono after load).
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Whether this sample is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Duration in seconds at the sample's native rate.
    pub fn duration_secs(&self) -> f32 {
        self.data.len() as f32 / self.sample_rate
    }

    /// The native sample rate of the loaded/provided audio.
    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// Clone the underlying `Arc<[f32]>`. Used by other DSP modules
    /// (sampler, granular) that need shared read-only access to the
    /// audio data without copying.
    pub(crate) fn data_arc(&self) -> Arc<[f32]> {
        Arc::clone(&self.data)
    }
}

/// Playback mode for a [`Sampler`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SamplerMode {
    /// Play once, then emit silence. `is_finished()` returns true.
    OneShot,
    /// Loop from region start to region end indefinitely.
    Loop,
    /// Bounce between region start and region end, reversing direction
    /// at each boundary.
    PingPong,
}

/// A single playback voice. Reads from a `Sample` with linear
/// interpolation at a user-settable rate.
///
/// Typical usage:
/// ```ignore
/// let kick = Sample::load("kick.wav")?;
/// let mut voice = Sampler::new(kick).pitch(1.5);
/// voice.trigger();           // resets to start
/// voice.next(&ctx);          // advances one sample
/// ```
pub struct Sampler<PR: Signal> {
    data: Arc<[f32]>,
    sample_sr: f32,
    rate: Param<PR>,
    /// Optional loop region in sample indices (start_idx, end_idx).
    /// Defaults to the entire sample.
    loop_region: Option<(f64, f64)>,
    mode: SamplerMode,
    position: f64,
    direction: f64,
    finished: bool,
}

impl Sampler<ConstSignal> {
    /// Build a one-shot sampler from a loaded sample. Default pitch
    /// (1.0) plays at native speed; use `.pitch()` to change.
    pub fn new(sample: Sample) -> Self {
        Sampler {
            data: Arc::clone(&sample.data),
            sample_sr: sample.sample_rate,
            rate: Param::Static(1.0),
            loop_region: None,
            mode: SamplerMode::OneShot,
            position: 0.0,
            direction: 1.0,
            finished: false,
        }
    }
}

impl<PR: Signal> Sampler<PR> {
    /// Set the playback rate. `1.0` = native pitch (adjusted for any
    /// sample-rate mismatch), `2.0` = octave up, `0.5` = octave down.
    ///
    /// Accepts `f32` or any `Signal` for modulated pitch.
    pub fn pitch<P: IntoParam>(self, rate: P) -> Sampler<P::Signal> {
        Sampler {
            data: self.data,
            sample_sr: self.sample_sr,
            rate: rate.into_param(),
            loop_region: self.loop_region,
            mode: self.mode,
            position: self.position,
            direction: self.direction,
            finished: self.finished,
        }
    }

    /// Loop over the entire sample.
    pub fn loop_all(mut self) -> Self {
        self.mode = SamplerMode::Loop;
        self.loop_region = None;
        self
    }

    /// Loop over a specific region, in seconds.
    pub fn loop_region(mut self, start_secs: f32, end_secs: f32) -> Self {
        let start = (start_secs * self.sample_sr).max(0.0) as f64;
        let end = (end_secs * self.sample_sr)
            .min(self.data.len() as f32)
            .max(start as f32 + 1.0) as f64;
        self.mode = SamplerMode::Loop;
        self.loop_region = Some((start, end));
        self
    }

    /// Switch to ping-pong mode (bounce between loop boundaries).
    pub fn ping_pong(mut self) -> Self {
        self.mode = SamplerMode::PingPong;
        self
    }

    /// Reset playback to the start of the sample (or loop region).
    /// Use to retrigger a one-shot voice for a new note-on event.
    pub fn trigger(&mut self) {
        self.position = self
            .loop_region
            .map(|(s, _)| s)
            .unwrap_or(0.0);
        self.direction = 1.0;
        self.finished = false;
    }

    /// Returns `true` if this one-shot voice has finished playing.
    /// Always returns `false` for `Loop` and `PingPong` modes.
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Current fractional playback position, in samples.
    pub fn position(&self) -> f64 {
        self.position
    }

    fn effective_bounds(&self) -> (f64, f64) {
        self.loop_region
            .unwrap_or((0.0, self.data.len() as f64))
    }

    fn read_interpolated(&self) -> f32 {
        let pos = self.position;
        let len = self.data.len();
        if len == 0 {
            return 0.0;
        }
        let idx0 = (pos.floor() as usize).min(len - 1);
        let idx1 = (idx0 + 1).min(len - 1);
        let frac = (pos - pos.floor()) as f32;
        self.data[idx0] * (1.0 - frac) + self.data[idx1] * frac
    }
}

impl<PR: Signal> Signal for Sampler<PR> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        if self.finished {
            return 0.0;
        }

        let pitch = self.rate.next(ctx);
        // Adjust playback increment for sample-rate mismatch: native
        // pitch (rate=1.0) plays at the sample's original rate even if
        // the stream runs at a different rate.
        let sr_ratio = (self.sample_sr / ctx.sample_rate) as f64;
        let step = pitch as f64 * sr_ratio * self.direction;

        let out = self.read_interpolated();
        self.position += step;

        let (lo, hi) = self.effective_bounds();

        match self.mode {
            SamplerMode::OneShot => {
                if self.position >= self.data.len() as f64 || self.position < 0.0 {
                    self.finished = true;
                }
            }
            SamplerMode::Loop => {
                let span = hi - lo;
                if span > 0.0 {
                    if self.position >= hi {
                        self.position = lo + (self.position - hi).rem_euclid(span);
                    } else if self.position < lo {
                        self.position = hi - (lo - self.position).rem_euclid(span);
                    }
                }
            }
            SamplerMode::PingPong => {
                if self.position >= hi {
                    self.position = 2.0 * hi - self.position;
                    self.direction = -1.0;
                } else if self.position < lo {
                    self.position = 2.0 * lo - self.position;
                    self.direction = 1.0;
                }
            }
        }

        out
    }
}
