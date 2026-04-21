//! Nyx prelude — one-line import for sketches and apps.
//!
//! `use nyx_prelude::*;` gives you the Signal trait, combinators, oscillators,
//! filters, dynamics, the clock, envelopes, notes, scales, chords, patterns,
//! instruments, and everything else needed to sketch audio.

// ─── nyx-core: types ─────────────────────────────────────────────────
pub use nyx_core::param::{ConstSignal, IntoParam};
pub use nyx_core::{
    Add, Amp, AudioContext, Biquad, BitCrush, Bus, Chorus, Clip, Compressor, Delay, Downsample,
    FilterExt, FilterMode, Flanger, FmOp, Freeverb, FreqBin, Gain, Granular, Haas, HaasSide,
    Inspect, InspectExt, Mix, Mul, Offset, Pan, Param, PeakLimiter, PitchConfig, PitchHandle,
    PitchTracker, Pluck, Sample, SampleError, Sampler, SamplerMode, Scope, ScopeExt, ScopeHandle,
    Sidechain, Signal, SignalExt, SoftClip, Spectrum, SpectrumConfig, SpectrumExt, SpectrumHandle,
    Svf, SvfMode, VoicePool, Wavetable, WavetableOsc, WindowFn, fm_op, pluck, render_to_buffer,
};

// ─── nyx-core: modules (for `osc::sine`, etc.) ───────────────────────
pub use nyx_core::dynamics;
pub use nyx_core::filter;
pub use nyx_core::golden;
pub use nyx_core::osc;

// ─── nyx-core: bridge / alloc guard / hotswap ────────────────────────
pub use nyx_core::hotswap;
pub use nyx_core::{AudioCommand, BridgeReceiver, BridgeSender, bridge};
pub use nyx_core::{DenyAllocGuard, GuardedAllocator};

// ─── nyx-core: MIDI / OSC / mic (types are always exported) ──────────
pub use nyx_core::midi;
pub use nyx_core::osc_input;
pub use nyx_core::osc_input::{OscParam, OscParamWriter, OscSignal};
pub use nyx_core::{CcMap, CcSignal, CcWriter, MidiEvent, MidiReceiver, MidiSender};

// ─── nyx-core: engine (audio feature only) ───────────────────────────
#[cfg(feature = "audio")]
pub use nyx_core::mic;
#[cfg(feature = "audio")]
pub use nyx_core::{Engine, EngineConfig, EngineError};

// ─── nyx-core: WAV export (wav feature) ──────────────────────────────
#[cfg(feature = "wav")]
pub use nyx_core::{WavError, render_to_wav, render_to_wav_f32};

// ─── nyx-seq: types ──────────────────────────────────────────────────
pub use nyx_seq::{
    Adsr, Automation, AutomationExt, Chord, ChordType, Clock, ClockState, Euclid, FilterType,
    Follow, Note, OscShape, PatchError, Pattern, Rng, Scale, ScaleMode, Sequence, Stage, StepEvent,
    SubSynth, SynthPatch,
};

// ─── nyx-seq: modules (for `clock::clock`, `automation::automation`, etc.) ─
pub use nyx_seq::automation;
pub use nyx_seq::clock;
pub use nyx_seq::envelope;
pub use nyx_seq::inst;
pub use nyx_seq::seeded;

// ─── Reusable demo signal builders ───────────────────────────────────
pub mod demos;

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
