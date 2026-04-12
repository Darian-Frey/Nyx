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
