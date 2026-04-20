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
    fn next(&mut self, ctx: &AudioContext) -> f32;
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

impl<A: Signal, S: Signal> Pan<A, S> {
    /// Get the stereo pair (left, right) for the current sample.
    pub fn next_stereo(&mut self, ctx: &AudioContext) -> (f32, f32) {
        let sample = self.source.next(ctx);
        let p = self.pos.next(ctx).clamp(-1.0, 1.0);
        // Constant-power-ish pan: linear crossfade.
        let left = sample * (1.0 - p) * 0.5;
        let right = sample * (1.0 + p) * 0.5;
        (left, right)
    }
}

impl<A: Signal, S: Signal> Signal for Pan<A, S> {
    /// Mono fold: sum of left and right channels.
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let (l, r) = self.next_stereo(ctx);
        l + r
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
