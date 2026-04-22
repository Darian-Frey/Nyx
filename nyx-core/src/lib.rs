mod alloc_guard;
mod bridge;
pub mod bus;
pub mod chorus;
pub mod compressor;
pub mod crush;
pub mod delay;
pub mod drift;
pub mod dynamics;
#[cfg(feature = "audio")]
mod engine;
pub mod filter;
pub mod flanger;
pub mod fm;
pub mod golden;
pub mod granular;
pub mod haas;
pub mod hotswap;
pub mod inspect;
pub mod ladder;
pub mod lofi;
pub mod mic;
pub mod midi;
pub mod osc;
pub mod osc_input;
pub mod param;
pub mod pitch;
pub mod pluck;
mod render;
pub mod reverb;
pub mod sample;
pub mod saturation;
pub mod scope;
mod signal;
pub mod spectrum;
pub mod svf;
pub mod tape;
pub mod vinyl;
mod voice;
#[cfg(feature = "wav")]
pub mod wav;
pub mod wavetable;

pub use alloc_guard::{DenyAllocGuard, GuardedAllocator};
pub use bridge::{AudioCommand, BridgeReceiver, BridgeSender, bridge};
pub use bus::Bus;
pub use chorus::Chorus;
pub use compressor::{Compressor, Sidechain};
pub use crush::{BitCrush, Downsample};
pub use delay::{DELAY_MAX_SR, Delay, MAX_FEEDBACK};
pub use drift::{Drift, drift};
pub use dynamics::{Gain, PeakLimiter, gain, peak_limiter};
#[cfg(feature = "audio")]
pub use engine::{Engine, EngineConfig, EngineError};
pub use filter::{Biquad, FilterExt, FilterMode};
pub use flanger::Flanger;
pub use fm::{FmOp, fm_op};
pub use granular::Granular;
pub use haas::{Haas, HaasSide};
pub use inspect::{Inspect, InspectExt};
pub use ladder::{Ladder, LadderExt};
pub use lofi::LofiExt;
#[cfg(feature = "audio")]
pub use mic::{MicError, MicHandle, MicSignal, mic, mic_with_buffer};
pub use midi::{
    CcMap, CcSignal, CcWriter, MidiEvent, MidiReceiver, MidiSender, midi_bridge, parse_midi,
};
#[cfg(feature = "midi")]
pub use midi::{MidiConnection, MidiError, open_midi_input, open_midi_input_named};
#[cfg(feature = "osc")]
pub use osc_input::{OscError, OscListener, osc_listen};
pub use osc_input::{OscParam, OscParamWriter, OscSignal};
pub use param::Param;
pub use pitch::{PitchConfig, PitchHandle, PitchTracker, pitch};
pub use pluck::{Pluck, pluck};
pub use render::render_to_buffer;
pub use reverb::Freeverb;
pub use sample::{Sample, SampleError, Sampler, SamplerMode};
pub use saturation::{DiodeClip, SaturationExt, TapeSat, TubeSat};
pub use scope::{Scope, ScopeExt, ScopeHandle};
pub use signal::{
    Add, Amp, AudioContext, Clip, Mix, Mul, Offset, Pan, Signal, SignalExt, SoftClip,
};
pub use spectrum::{FreqBin, Spectrum, SpectrumConfig, SpectrumExt, SpectrumHandle, WindowFn};
pub use svf::{Svf, SvfMode};
pub use tape::{Tape, TapeExt};
pub use vinyl::VinylCrackle;
pub use voice::VoicePool;
#[cfg(feature = "wav")]
pub use wav::{WavError, render_to_wav, render_to_wav_f32};
pub use wavetable::{Wavetable, WavetableOsc};
