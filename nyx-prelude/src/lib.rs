//! Nyx prelude — one-line import for sketches and apps.
//!
//! `use nyx_prelude::*;` gives you the Signal trait, combinators, oscillators,
//! filters, dynamics, the clock, envelopes, notes, scales, chords, patterns,
//! instruments, and everything else needed to sketch audio.

// ─── nyx-core: types ─────────────────────────────────────────────────
pub use nyx_core::{
    AudioContext, Param, Signal, SignalExt, VoicePool,
    Add, Amp, Clip, Mix, Mul, Offset, Pan, SoftClip,
    BitCrush, Downsample,
    Delay,
    pluck, Pluck,
    Sample, SampleError, Sampler, SamplerMode,
    Biquad, FilterExt, FilterMode,
    Gain, PeakLimiter,
    Scope, ScopeExt, ScopeHandle,
    Inspect, InspectExt,
    FreqBin, Spectrum, SpectrumConfig, SpectrumExt, SpectrumHandle, WindowFn,
    render_to_buffer,
};
pub use nyx_core::param::{ConstSignal, IntoParam};

// ─── nyx-core: modules (for `osc::sine`, etc.) ───────────────────────
pub use nyx_core::osc;
pub use nyx_core::filter;
pub use nyx_core::dynamics;
pub use nyx_core::golden;

// ─── nyx-core: bridge / alloc guard / hotswap ────────────────────────
pub use nyx_core::{bridge, AudioCommand, BridgeReceiver, BridgeSender};
pub use nyx_core::{DenyAllocGuard, GuardedAllocator};
pub use nyx_core::hotswap;

// ─── nyx-core: MIDI / OSC / mic (types are always exported) ──────────
pub use nyx_core::midi;
pub use nyx_core::osc_input;
pub use nyx_core::{CcMap, CcSignal, CcWriter, MidiEvent, MidiReceiver, MidiSender};

// ─── nyx-core: engine (audio feature only) ───────────────────────────
#[cfg(feature = "audio")]
pub use nyx_core::{Engine, EngineConfig, EngineError};
#[cfg(feature = "audio")]
pub use nyx_core::mic;

// ─── nyx-core: WAV export (wav feature) ──────────────────────────────
#[cfg(feature = "wav")]
pub use nyx_core::{render_to_wav, render_to_wav_f32, WavError};

// ─── nyx-seq: types ──────────────────────────────────────────────────
pub use nyx_seq::{
    Clock, ClockState,
    Adsr, Stage,
    Automation, AutomationExt, Follow,
    Note,
    Scale, ScaleMode,
    Chord, ChordType,
    Pattern,
    Euclid,
    Rng,
    Sequence, StepEvent,
    SubSynth, SynthPatch, OscShape, FilterType, PatchError,
};

// ─── nyx-seq: modules (for `clock::clock`, `automation::automation`, etc.) ─
pub use nyx_seq::clock;
pub use nyx_seq::envelope;
pub use nyx_seq::automation;
pub use nyx_seq::inst;
pub use nyx_seq::{seeded};

/// Start playing a signal on the default audio output device.
///
/// Blocks the current thread until the user presses Enter.
///
/// ```ignore
/// use nyx_prelude::*;
///
/// fn main() {
///     play(osc::sine(440.0).amp(0.3)).unwrap();
/// }
/// ```
#[cfg(feature = "audio")]
pub fn play<S: Signal + 'static>(signal: S) -> Result<Engine, EngineError> {
    let engine = Engine::play(signal)?;
    println!("nyx: playing — press Enter to stop");
    let mut buf = String::new();
    let _ = std::io::stdin().read_line(&mut buf);
    Ok(engine)
}

/// Start playing a signal and return the engine handle immediately
/// (non-blocking). The caller is responsible for keeping the `Engine`
/// alive — audio stops when it is dropped.
#[cfg(feature = "audio")]
pub fn play_async<S: Signal + 'static>(signal: S) -> Result<Engine, EngineError> {
    Engine::play(signal)
}
