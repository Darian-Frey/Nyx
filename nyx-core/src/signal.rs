use crate::crush::{BitCrush, Downsample};
use crate::delay::{new_delay, Delay};
use crate::param::{ConstSignal, IntoParam, Param};

/// Per-sample context passed to every `Signal::next` call.
///
/// Carries the stream sample rate and an absolute tick counter so signals
/// can compute phase, tempo, and sample-accurate scheduling without globals.
#[derive(Debug, Clone, Copy)]
pub struct AudioContext {
    pub sample_rate: f32,
    /// Absolute sample count from stream start.
    pub tick: u64,
}

/// The core abstraction: a stream of mono audio samples.
///
/// Every oscillator, filter, envelope, and effect implements `Signal`.
/// The trait is `Send` (signals are moved to the audio thread) but not
/// `Sync` (they are exclusively owned by that thread).
pub trait Signal: Send {
    /// Produce the next mono sample.
    fn next(&mut self, ctx: &AudioContext) -> f32;

    /// Produce the next stereo `(left, right)` pair.
    ///
    /// The default implementation duplicates the mono output into both
    /// channels. Stereo-native signals (like [`Pan`], Haas widener,
    /// future reverb) override this method to produce a real stereo
    /// image while their `next` implementation folds back to mono.
    ///
    /// The audio engine calls `next_stereo` once per frame and writes
    /// the two samples to the left and right output channels.
    fn next_stereo(&mut self, ctx: &AudioContext) -> (f32, f32) {
        let s = self.next(ctx);
        (s, s)
    }
}

/// Blanket impl: any mutable closure that matches the signature is a Signal.
impl<F> Signal for F
where
    F: FnMut(&AudioContext) -> f32 + Send,
{
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        self(ctx)
    }
}

/// Type-erased signal. Allocated once at construction time.
impl Signal for Box<dyn Signal> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        (**self).next(ctx)
    }

    fn next_stereo(&mut self, ctx: &AudioContext) -> (f32, f32) {
        (**self).next_stereo(ctx)
    }
}

/// Extension trait providing `.boxed()` and combinator methods on all signals.
pub trait SignalExt: Signal + Sized {
    /// Type-erase this signal into a `Box<dyn Signal>`.
    ///
    /// Allocates once at call time. Use this when you need heterogeneous
    /// signal collections or recursive signal graphs.
    fn boxed(self) -> Box<dyn Signal>
    where
        Self: 'static,
    {
        Box::new(self)
    }

    /// Multiply output by a gain factor (static or modulated).
    ///
    /// Accepts `f32` or any `Signal`:
    /// ```ignore
    /// signal.amp(0.5)    // static gain
    /// signal.amp(lfo)    // modulated gain
    /// ```
    fn amp<P: IntoParam>(self, gain: P) -> Amp<Self, P::Signal> {
        Amp {
            source: self,
            gain: gain.into_param(),
        }
    }

    /// Add another signal's output to this one.
    fn add<S: Signal>(self, other: S) -> Add<Self, S> {
        Add { a: self, b: other }
    }

    /// Multiply this signal's output by another signal.
    fn mul<S: Signal>(self, other: S) -> Mul<Self, S> {
        Mul { a: self, b: other }
    }

    /// Mix this signal with another at a given ratio (0.0 = all self, 1.0 = all other).
    ///
    /// Ratio accepts `f32` or any `Signal`.
    fn mix<S: Signal, P: IntoParam>(self, other: S, ratio: P) -> Mix<Self, S, P::Signal> {
        Mix {
            a: self,
            b: other,
            ratio: ratio.into_param(),
        }
    }

    /// Stereo pan. Returns a `Pan` that produces (left, right) via `next_stereo()`,
    /// but sums to mono for the `Signal` trait. `pos`: -1.0 = hard left, +1.0 = hard right.
    ///
    /// Accepts `f32` or any `Signal`.
    fn pan<P: IntoParam>(self, pos: P) -> Pan<Self, P::Signal> {
        Pan {
            source: self,
            pos: pos.into_param(),
        }
    }

    /// Hard-clip output to [-threshold, +threshold].
    fn clip(self, threshold: f32) -> Clip<Self> {
        Clip {
            source: self,
            threshold,
        }
    }

    /// Soft-clip output using tanh saturation, scaled by `drive`.
    fn soft_clip(self, drive: f32) -> SoftClip<Self> {
        SoftClip {
            source: self,
            drive,
        }
    }

    /// Add a constant offset to every sample.
    fn offset(self, value: f32) -> Offset<Self> {
        Offset {
            source: self,
            value,
        }
    }

    /// Quantise this signal to a reduced bit depth.
    ///
    /// Produces a digital "crushed" sound. `bits` is clamped to `[1, 24]`.
    /// `1` gives a harsh square-wave-like output, `4` gives classic 80s
    /// sampler grit, `16` is effectively transparent.
    ///
    /// ```ignore
    /// osc::sine(440.0).bitcrush(4)
    /// ```
    fn bitcrush(self, bits: u32) -> BitCrush<Self> {
        BitCrush::new(self, bits)
    }

    /// Reduce the effective sample rate by sample-and-hold.
    ///
    /// `ratio` ∈ `(0, 1]`: `1.0` is identity, `0.5` halves the rate,
    /// `0.25` quarters it. Values outside this range are clamped.
    ///
    /// ```ignore
    /// osc::saw(220.0).downsample(0.25)  // 11 kHz effective rate
    /// ```
    fn downsample(self, ratio: f32) -> Downsample<Self> {
        Downsample::new(self, ratio)
    }

    /// Convenience: bitcrush then downsample in one chain.
    ///
    /// Equivalent to `self.bitcrush(bits).downsample(ratio)`. Together
    /// these two effects produce the full lo-fi / glitch character —
    /// bit-depth reduction for grit, sample-rate reduction for aliasing.
    ///
    /// ```ignore
    /// osc::saw(110.0).crush(6, 0.5)
    /// ```
    fn crush(self, bits: u32, ratio: f32) -> Downsample<BitCrush<Self>> {
        self.bitcrush(bits).downsample(ratio)
    }

    /// Wrap this signal in a delay line with configurable feedback and
    /// wet/dry mix.
    ///
    /// Returns a [`Delay`] whose builder methods (`.time()`, `.feedback()`,
    /// `.mix()`) configure the effect. Feedback is internally clamped to
    /// `[0.0, 0.95]`. The initial `time_secs` also sets the maximum
    /// buffer length; call `.max_time()` on the returned delay if you
    /// plan to modulate `time` higher.
    ///
    /// ```ignore
    /// osc::saw(220.0)
    ///     .delay(0.375)      // 375 ms echo
    ///     .feedback(0.4)      // 40% feedback
    ///     .mix(0.3)           // 30% wet
    /// ```
    fn delay(self, time_secs: f32) -> Delay<Self, ConstSignal, ConstSignal, ConstSignal> {
        new_delay(self, time_secs)
    }

    /// Apply a Haas-effect stereo widener. Delays the right channel by
    /// `delay_ms` milliseconds (5–30 ms for the classic pop-mix width);
    /// the left channel plays in time.
    ///
    /// ```ignore
    /// osc::saw(220.0).haas(15.0)
    /// ```
    fn haas(self, delay_ms: f32) -> crate::haas::Haas<Self> {
        crate::haas::Haas::new(self, delay_ms, crate::haas::HaasSide::Right)
    }

    /// Haas widener with explicit side selection.
    fn haas_side(self, delay_ms: f32, side: crate::haas::HaasSide) -> crate::haas::Haas<Self> {
        crate::haas::Haas::new(self, delay_ms, side)
    }

    /// Wrap this signal in a Freeverb stereo reverb.
    ///
    /// Returns a [`Freeverb`](crate::reverb::Freeverb) with builder
    /// methods (`.room_size()`, `.damping()`, `.wet()`, `.width()`).
    ///
    /// ```ignore
    /// osc::saw(220.0)
    ///     .freeverb()
    ///     .room_size(0.85)
    ///     .damping(0.5)
    ///     .wet(0.3)
    /// ```
    fn freeverb(self) -> crate::reverb::Freeverb<Self> {
        crate::reverb::Freeverb::new(self)
    }

    /// Apply a stereo chorus — modulated short delay with 180°-offset
    /// LFOs on left and right for natural stereo spread.
    ///
    /// `rate_hz` is the LFO speed (0.1–3 Hz typical); `depth_ms` is
    /// the delay-time deviation from the base (1–10 ms typical).
    /// `.base_delay()` and `.mix()` configure defaults further.
    ///
    /// ```ignore
    /// osc::saw(220.0).chorus(0.5, 3.0)
    /// ```
    fn chorus(self, rate_hz: f32, depth_ms: f32) -> crate::chorus::Chorus<Self> {
        crate::chorus::Chorus::new(self, rate_hz, depth_ms)
    }

    /// Apply a stereo flanger — a modulated short delay with feedback.
    /// Shorter base delay + higher feedback than chorus produces the
    /// classic "jet plane" swooshing comb filter.
    ///
    /// `rate_hz` and `depth_ms` control the LFO. Use `.feedback()` to
    /// crank the swirl (default 0.0).
    ///
    /// ```ignore
    /// osc::saw(110.0).flanger(0.3, 2.0).feedback(0.7)
    /// ```
    fn flanger(self, rate_hz: f32, depth_ms: f32) -> crate::flanger::Flanger<Self> {
        crate::flanger::Flanger::new(self, rate_hz, depth_ms)
    }

    /// Apply a feed-forward compressor that detects on its own output.
    ///
    /// `threshold_db` is typically negative (e.g. `-12.0` dB); `ratio` is
    /// ≥ 1.0 (`4.0` is a common musical compression; `f32::INFINITY` makes
    /// this a brick-wall limiter). Further shaping via `.attack_ms()`,
    /// `.release_ms()`, and `.makeup_db()` builders.
    ///
    /// ```ignore
    /// drums.compress(-12.0, 4.0).attack_ms(5.0).release_ms(100.0);
    /// ```
    fn compress(self, threshold_db: f32, ratio: f32) -> crate::compressor::Compressor<Self> {
        crate::compressor::Compressor::new(self, threshold_db, ratio)
    }

    /// Apply a sidechain compressor — the *trigger* signal drives gain
    /// reduction applied to `self`. The trigger is consumed but not
    /// audible; only `self` reaches the output.
    ///
    /// Classic use: duck a bassline in time with a four-on-the-floor kick
    /// to produce the pumping trance / house feel.
    ///
    /// ```ignore
    /// bass.sidechain(kick, -20.0, 8.0)
    ///     .attack_ms(1.0)
    ///     .release_ms(150.0);
    /// ```
    fn sidechain<T: Signal>(
        self,
        trigger: T,
        threshold_db: f32,
        ratio: f32,
    ) -> crate::compressor::Sidechain<Self, T> {
        crate::compressor::Sidechain::new(self, trigger, threshold_db, ratio)
    }

    /// Tap this signal with a YIN pitch tracker.
    ///
    /// Returns `(wrapped_signal, handle)`. The wrapped signal is a
    /// passive pass-through (samples are unchanged); feed it into the
    /// audio engine. The handle publishes the detected fundamental and
    /// clarity from any thread via lock-free atomics.
    ///
    /// ```ignore
    /// let (sig, pitch) = mic().pitch(PitchConfig::default());
    /// let _engine = play_async(sig).unwrap();
    /// println!("{}", pitch.freq());
    /// ```
    fn pitch(
        self,
        config: crate::pitch::PitchConfig,
    ) -> (crate::pitch::PitchTracker<Self>, crate::pitch::PitchHandle) {
        crate::pitch::pitch(self, config)
    }
}

// Blanket impl: every Signal gets combinator methods for free.
impl<T: Signal + Sized> SignalExt for T {}

// ---------- Combinator structs ----------

pub struct Amp<A: Signal, S: Signal> {
    source: A,
    gain: Param<S>,
}

impl<A: Signal, S: Signal> Signal for Amp<A, S> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        self.source.next(ctx) * self.gain.next(ctx)
    }
}

pub struct Add<A: Signal, B: Signal> {
    a: A,
    b: B,
}

impl<A: Signal, B: Signal> Signal for Add<A, B> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        self.a.next(ctx) + self.b.next(ctx)
    }
}

pub struct Mul<A: Signal, B: Signal> {
    a: A,
    b: B,
}

impl<A: Signal, B: Signal> Signal for Mul<A, B> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        self.a.next(ctx) * self.b.next(ctx)
    }
}

pub struct Mix<A: Signal, B: Signal, M: Signal> {
    a: A,
    b: B,
    ratio: Param<M>,
}

impl<A: Signal, B: Signal, M: Signal> Signal for Mix<A, B, M> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let r = self.ratio.next(ctx).clamp(0.0, 1.0);
        let sa = self.a.next(ctx);
        let sb = self.b.next(ctx);
        sa * (1.0 - r) + sb * r
    }
}

pub struct Pan<A: Signal, S: Signal> {
    source: A,
    pos: Param<S>,
}

impl<A: Signal, S: Signal> Signal for Pan<A, S> {
    /// Mono fold: sum of left and right channels (pan-law preserving;
    /// hard-left pan outputs `(sample, 0)`, summing to `sample`).
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let (l, r) = self.next_stereo(ctx);
        l + r
    }

    /// Real stereo pair with linear pan law.
    fn next_stereo(&mut self, ctx: &AudioContext) -> (f32, f32) {
        let sample = self.source.next(ctx);
        let p = self.pos.next(ctx).clamp(-1.0, 1.0);
        let left = sample * (1.0 - p) * 0.5;
        let right = sample * (1.0 + p) * 0.5;
        (left, right)
    }
}

pub struct Clip<A: Signal> {
    source: A,
    threshold: f32,
}

impl<A: Signal> Signal for Clip<A> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        self.source.next(ctx).clamp(-self.threshold, self.threshold)
    }
}

pub struct SoftClip<A: Signal> {
    source: A,
    drive: f32,
}

impl<A: Signal> Signal for SoftClip<A> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        (self.source.next(ctx) * self.drive).tanh()
    }
}

pub struct Offset<A: Signal> {
    source: A,
    value: f32,
}

impl<A: Signal> Signal for Offset<A> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        self.source.next(ctx) + self.value
    }
}
