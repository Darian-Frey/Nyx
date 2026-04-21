//! Feed-forward compressor with optional sidechain input.
//!
//! Two public types:
//!
//! - [`Compressor`] — self-detecting. The level of the signal being
//!   processed drives its own gain reduction. Good for taming loud peaks,
//!   glue on a bus, or gentle levelling.
//! - [`Sidechain`] — the gain reduction is driven by an *external* trigger
//!   signal. This is the classic trance "pumping bass" effect: bass is
//!   compressed, kick drum is the trigger.
//!
//! Both use peak detection (absolute value), a simple asymmetric attack /
//! release envelope follower, and a hard-knee gain computer. Ratios above
//! 1.0 compress; `f32::INFINITY` makes it a brick-wall limiter.
//!
//! ```ignore
//! // Self-compressing bus
//! drums.compress(-12.0, 4.0)
//!     .attack_ms(5.0)
//!     .release_ms(100.0)
//!     .makeup_db(3.0);
//!
//! // Kick ducks bass
//! bass.sidechain(kick_trigger, -20.0, 8.0)
//!     .attack_ms(1.0)
//!     .release_ms(150.0);
//! ```

use crate::signal::{AudioContext, Signal};

/// Shared DSP state between the self-detecting and sidechain compressors.
struct Core {
    threshold_db: f32,
    ratio: f32,
    attack_ms: f32,
    release_ms: f32,
    makeup_db: f32,

    // Cached coefficients (recomputed on sample-rate change).
    cached_sr: f32,
    attack_coeff: f32,
    release_coeff: f32,
    makeup_amp: f32,

    // Envelope follower state (linear amplitude).
    envelope: f32,
}

impl Core {
    fn new(threshold_db: f32, ratio: f32) -> Self {
        Self {
            threshold_db,
            ratio: ratio.max(1.0),
            attack_ms: 5.0,
            release_ms: 100.0,
            makeup_db: 0.0,
            cached_sr: 0.0,
            attack_coeff: 0.0,
            release_coeff: 0.0,
            makeup_amp: 1.0,
            envelope: 0.0,
        }
    }

    #[inline]
    fn update_coeffs(&mut self, sr: f32) {
        if sr != self.cached_sr {
            self.attack_coeff = time_coeff(self.attack_ms, sr);
            self.release_coeff = time_coeff(self.release_ms, sr);
            self.cached_sr = sr;
        }
    }

    /// Given a detector sample and the current sample rate, step the
    /// envelope follower and return the output gain (linear amplitude,
    /// including makeup).
    #[inline]
    fn compute_gain(&mut self, detector: f32, sr: f32) -> f32 {
        self.update_coeffs(sr);

        let level = detector.abs();
        let c = if level > self.envelope {
            self.attack_coeff
        } else {
            self.release_coeff
        };
        self.envelope += c * (level - self.envelope);

        // Convert envelope to dB, subtract threshold, scale by ratio.
        let env_db = amp_to_db(self.envelope);
        let over = env_db - self.threshold_db;
        let reduction_db = if over > 0.0 {
            over * (1.0 - 1.0 / self.ratio)
        } else {
            0.0
        };
        db_to_amp(-reduction_db) * self.makeup_amp
    }

    fn set_attack(&mut self, ms: f32) {
        self.attack_ms = ms.max(0.01);
        self.cached_sr = 0.0;
    }

    fn set_release(&mut self, ms: f32) {
        self.release_ms = ms.max(0.01);
        self.cached_sr = 0.0;
    }

    fn set_makeup(&mut self, db: f32) {
        self.makeup_db = db;
        self.makeup_amp = db_to_amp(db);
    }

    fn set_threshold(&mut self, db: f32) {
        self.threshold_db = db;
    }

    fn set_ratio(&mut self, ratio: f32) {
        self.ratio = ratio.max(1.0);
    }
}

#[inline]
fn time_coeff(time_ms: f32, sample_rate: f32) -> f32 {
    let samples = time_ms * 0.001 * sample_rate;
    if samples <= 0.0 {
        return 1.0;
    }
    1.0 - (-1.0 / samples).exp()
}

#[inline]
fn amp_to_db(amp: f32) -> f32 {
    // Floor to -120 dB to avoid -inf / NaN from log10(0).
    20.0 * amp.max(1e-6).log10()
}

#[inline]
fn db_to_amp(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

// ───────────────────────── Compressor (self-detect) ─────────────────────────

/// Feed-forward compressor driven by its own signal.
///
/// Construct via [`SignalExt::compress`](crate::signal::SignalExt::compress).
pub struct Compressor<A: Signal> {
    source: A,
    core: Core,
}

impl<A: Signal> Compressor<A> {
    /// Create a compressor with the given threshold (in dB, typically
    /// negative) and ratio (≥ 1.0).
    pub fn new(source: A, threshold_db: f32, ratio: f32) -> Self {
        Self {
            source,
            core: Core::new(threshold_db, ratio),
        }
    }

    /// Envelope attack time in milliseconds (default 5 ms).
    pub fn attack_ms(mut self, ms: f32) -> Self {
        self.core.set_attack(ms);
        self
    }

    /// Envelope release time in milliseconds (default 100 ms).
    pub fn release_ms(mut self, ms: f32) -> Self {
        self.core.set_release(ms);
        self
    }

    /// Post-compression makeup gain in dB (default 0 dB).
    pub fn makeup_db(mut self, db: f32) -> Self {
        self.core.set_makeup(db);
        self
    }

    /// Change the threshold (in dB).
    pub fn threshold_db(mut self, db: f32) -> Self {
        self.core.set_threshold(db);
        self
    }

    /// Change the ratio.
    pub fn ratio(mut self, ratio: f32) -> Self {
        self.core.set_ratio(ratio);
        self
    }
}

impl<A: Signal> Signal for Compressor<A> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let input = self.source.next(ctx);
        let gain = self.core.compute_gain(input, ctx.sample_rate);
        input * gain
    }

    fn next_stereo(&mut self, ctx: &AudioContext) -> (f32, f32) {
        let (l, r) = self.source.next_stereo(ctx);
        // Detect on the louder of the two channels so the stereo image
        // doesn't collapse when one side is much hotter.
        let detector = l.abs().max(r.abs());
        let gain = self.core.compute_gain(detector, ctx.sample_rate);
        (l * gain, r * gain)
    }
}

// ───────────────────────── Sidechain compressor ─────────────────────────

/// Sidechain compressor — the level of the *trigger* signal drives the
/// gain reduction applied to `source`.
///
/// The trigger signal is consumed (its samples are pulled and discarded);
/// only `source` is audible. Construct via
/// [`SignalExt::sidechain`](crate::signal::SignalExt::sidechain).
pub struct Sidechain<A: Signal, T: Signal> {
    source: A,
    trigger: T,
    core: Core,
}

impl<A: Signal, T: Signal> Sidechain<A, T> {
    pub fn new(source: A, trigger: T, threshold_db: f32, ratio: f32) -> Self {
        Self {
            source,
            trigger,
            core: Core::new(threshold_db, ratio),
        }
    }

    pub fn attack_ms(mut self, ms: f32) -> Self {
        self.core.set_attack(ms);
        self
    }

    pub fn release_ms(mut self, ms: f32) -> Self {
        self.core.set_release(ms);
        self
    }

    pub fn makeup_db(mut self, db: f32) -> Self {
        self.core.set_makeup(db);
        self
    }

    pub fn threshold_db(mut self, db: f32) -> Self {
        self.core.set_threshold(db);
        self
    }

    pub fn ratio(mut self, ratio: f32) -> Self {
        self.core.set_ratio(ratio);
        self
    }
}

impl<A: Signal, T: Signal> Signal for Sidechain<A, T> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let input = self.source.next(ctx);
        let trig = self.trigger.next(ctx);
        let gain = self.core.compute_gain(trig, ctx.sample_rate);
        input * gain
    }

    fn next_stereo(&mut self, ctx: &AudioContext) -> (f32, f32) {
        let (l, r) = self.source.next_stereo(ctx);
        let (tl, tr) = self.trigger.next_stereo(ctx);
        let detector = tl.abs().max(tr.abs());
        let gain = self.core.compute_gain(detector, ctx.sample_rate);
        (l * gain, r * gain)
    }
}
