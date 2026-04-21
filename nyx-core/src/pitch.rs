//! Real-time pitch detection via the YIN algorithm
//! (de Cheveigné & Kawahara, 2002).
//!
//! `.pitch(config)` wraps a source signal, passes every sample through
//! unchanged, and every `hop_size` samples runs YIN on the most recent
//! `frame_size` samples. The detected fundamental frequency (Hz) and
//! clarity score (0.0–1.0) are published to a [`PitchHandle`] via
//! lock-free atomics, so any thread can read the current pitch without
//! blocking the audio callback.
//!
//! ```ignore
//! use nyx_prelude::*;
//!
//! let (signal, handle) = mic().pitch(PitchConfig::default());
//! let _engine = play_async(signal).unwrap();
//! loop {
//!     let (f, c) = handle.read();
//!     println!("freq = {f:>7.2} Hz, clarity = {c:.2}");
//!     std::thread::sleep(std::time::Duration::from_millis(100));
//! }
//! ```
//!
//! **Algorithm overview.** YIN is an autocorrelation-based f0 estimator
//! with four steps:
//!
//! 1. Difference function `d(τ) = Σ (x[i] − x[i+τ])²`.
//! 2. Cumulative mean normalised difference function (CMNDF):
//!    `d'(τ) = d(τ) · τ / Σ_{j≤τ} d(j)`.
//! 3. Absolute threshold — take the first `τ` where `d'(τ) < threshold`,
//!    then walk down into the local minimum.
//! 4. Parabolic interpolation of the minimum for sub-sample accuracy.
//!
//! **Real-time cost.** Inner loop is `O(frame_size × max_τ)`. With the
//! default 2048-sample frame and a 40 Hz min-freq cap (`max_τ ≈ 1100`),
//! analysis is roughly 2.2 M multiply-adds per hop (≈ every 23 ms at
//! 44.1 kHz). This runs inside the audio callback — matching the
//! convention used by [`crate::spectrum::Spectrum`].
//!
//! **Published fields.** `freq = 0.0` means no periodic signal was
//! detected (silence, transient, or noise above the clarity threshold).
//! `confidence = 1.0 − d'(τ*)` is the "clarity": 1.0 is a perfect
//! periodic signal, 0.0 is pure noise.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use crate::signal::{AudioContext, Signal};

/// Pitch detection configuration.
#[derive(Debug, Clone)]
pub struct PitchConfig {
    /// Analysis window length in samples. Larger = better low-frequency
    /// resolution but higher CPU and latency. Default `2048`.
    pub frame_size: usize,
    /// How often to run analysis, in samples. Smaller = smoother tracking
    /// but higher CPU. Default `1024` (50% overlap with `frame_size`).
    pub hop_size: usize,
    /// YIN's absolute threshold on the CMNDF. Values where `d'(τ)` dips
    /// below this are candidates. Typical `0.10`–`0.20`; lower is more
    /// strict. Default `0.15`.
    pub threshold: f32,
    /// Minimum pitch to search for, in Hz. Sets an upper bound on τ.
    /// Default `40.0` (low bass).
    pub min_freq: f32,
    /// Maximum pitch to search for, in Hz. Sets a lower bound on τ.
    /// Default `4000.0` (piccolo high register).
    pub max_freq: f32,
}

impl Default for PitchConfig {
    fn default() -> Self {
        Self {
            frame_size: 2048,
            hop_size: 1024,
            threshold: 0.15,
            min_freq: 40.0,
            max_freq: 4000.0,
        }
    }
}

/// Lock-free handle for reading the latest pitch estimate.
///
/// Cheap to clone — internally two `Arc<AtomicU32>`.
#[derive(Clone)]
pub struct PitchHandle {
    freq_bits: Arc<AtomicU32>,
    conf_bits: Arc<AtomicU32>,
}

impl PitchHandle {
    /// Latest fundamental frequency estimate in Hz. Returns `0.0` when
    /// the input is silent or non-periodic.
    pub fn freq(&self) -> f32 {
        f32::from_bits(self.freq_bits.load(Ordering::Relaxed))
    }

    /// Latest clarity score. `1.0` = perfectly periodic, `0.0` = noise.
    pub fn confidence(&self) -> f32 {
        f32::from_bits(self.conf_bits.load(Ordering::Relaxed))
    }

    /// Read both atoms atomically (well, independently — pitch and
    /// confidence are not strictly synchronised, but both are stale by
    /// at most one hop).
    pub fn read(&self) -> (f32, f32) {
        (self.freq(), self.confidence())
    }
}

/// Signal wrapper that runs YIN on its source's samples.
///
/// Construct via [`SignalExt::pitch`](crate::signal::SignalExt::pitch).
/// Output is identical to the input — this is a passive tap.
pub struct PitchTracker<A: Signal> {
    source: A,
    config: PitchConfig,

    // Circular buffer of the most recent `frame_size` samples.
    ring: Box<[f32]>,
    write: usize,
    primed: bool,
    since_hop: usize,

    // Analysis scratch.
    work: Box<[f32]>,
    d_prime: Box<[f32]>,

    // Cached τ bounds, derived from sample_rate + min/max_freq.
    cached_sr: f32,
    min_tau: usize,
    max_tau: usize,

    freq_bits: Arc<AtomicU32>,
    conf_bits: Arc<AtomicU32>,
}

impl<A: Signal> PitchTracker<A> {
    fn update_tau_bounds(&mut self, sr: f32) {
        if sr == self.cached_sr {
            return;
        }
        self.cached_sr = sr;
        let cap = self.work.len() / 2;
        self.min_tau = ((sr / self.config.max_freq).ceil() as usize).max(2);
        let raw_max = (sr / self.config.min_freq).ceil() as usize;
        self.max_tau = raw_max.min(cap).max(self.min_tau + 2);
    }

    /// Copy the ring contents into `work` in chronological order.
    fn linearize(&mut self) {
        let n = self.work.len();
        let start = self.write; // oldest sample sits here
        let (a, b) = self.ring.split_at(start);
        // Oldest half = b (from `start` to end of ring)
        // then a (from ring start to `start`).
        let (work_head, work_tail) = self.work.split_at_mut(b.len());
        work_head.copy_from_slice(b);
        work_tail.copy_from_slice(a);
        debug_assert_eq!(work_head.len() + work_tail.len(), n);
    }

    /// Run YIN on the current `work` buffer and publish the result.
    fn analyze(&mut self, sr: f32) {
        self.update_tau_bounds(sr);
        self.linearize();

        let win = self.work.len();
        let max_tau = self.max_tau;

        // Difference function + CMNDF in one pass.
        self.d_prime[0] = 1.0;
        let mut running_sum = 0.0_f32;

        for tau in 1..max_tau {
            let mut d = 0.0_f32;
            // Using slices would bounds-check; take raw pointers for
            // speed. Lengths are invariant per call, bounds checked once.
            let limit = win - tau;
            for i in 0..limit {
                let diff = self.work[i] - self.work[i + tau];
                d += diff * diff;
            }
            running_sum += d;
            self.d_prime[tau] = if running_sum > 0.0 {
                d * tau as f32 / running_sum
            } else {
                1.0
            };
        }

        // Absolute-threshold search with local-minimum walk.
        let mut best_tau: Option<usize> = None;
        let mut tau = self.min_tau;
        while tau < max_tau {
            if self.d_prime[tau] < self.config.threshold {
                while tau + 1 < max_tau && self.d_prime[tau + 1] < self.d_prime[tau] {
                    tau += 1;
                }
                best_tau = Some(tau);
                break;
            }
            tau += 1;
        }

        let Some(t) = best_tau else {
            // No periodicity found — publish zeros.
            self.freq_bits.store(0u32, Ordering::Relaxed);
            self.conf_bits.store(0u32, Ordering::Relaxed);
            return;
        };

        // Parabolic interpolation around d_prime[t].
        let refined_tau = if t > 0 && t + 1 < self.d_prime.len() {
            let s0 = self.d_prime[t - 1];
            let s1 = self.d_prime[t];
            let s2 = self.d_prime[t + 1];
            let denom = 2.0 * (s0 - 2.0 * s1 + s2);
            if denom.abs() > 1e-9 {
                t as f32 + (s0 - s2) / denom
            } else {
                t as f32
            }
        } else {
            t as f32
        };

        let freq = sr / refined_tau.max(1.0);
        let clarity = (1.0 - self.d_prime[t]).clamp(0.0, 1.0);
        self.freq_bits.store(freq.to_bits(), Ordering::Relaxed);
        self.conf_bits.store(clarity.to_bits(), Ordering::Relaxed);
    }
}

impl<A: Signal> Signal for PitchTracker<A> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let s = self.source.next(ctx);

        self.ring[self.write] = s;
        self.write += 1;
        if self.write >= self.ring.len() {
            self.write = 0;
            self.primed = true;
        }
        self.since_hop += 1;

        if self.primed && self.since_hop >= self.config.hop_size {
            self.since_hop = 0;
            self.analyze(ctx.sample_rate);
        }

        s
    }
}

/// Build a `(PitchTracker, PitchHandle)` pair from a source signal.
///
/// Normally called via [`SignalExt::pitch`](crate::signal::SignalExt::pitch).
pub fn pitch<A: Signal>(source: A, config: PitchConfig) -> (PitchTracker<A>, PitchHandle) {
    let frame_size = config.frame_size.max(64);
    let half = frame_size / 2;

    let freq_bits = Arc::new(AtomicU32::new(0));
    let conf_bits = Arc::new(AtomicU32::new(0));

    let tracker = PitchTracker {
        source,
        config: PitchConfig {
            frame_size,
            ..config
        },
        ring: vec![0.0; frame_size].into_boxed_slice(),
        write: 0,
        primed: false,
        since_hop: 0,
        work: vec![0.0; frame_size].into_boxed_slice(),
        d_prime: vec![0.0; half].into_boxed_slice(),
        cached_sr: 0.0,
        min_tau: 2,
        max_tau: half,
        freq_bits: Arc::clone(&freq_bits),
        conf_bits: Arc::clone(&conf_bits),
    };

    let handle = PitchHandle {
        freq_bits,
        conf_bits,
    };

    (tracker, handle)
}
