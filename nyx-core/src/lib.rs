mod alloc_guard;
mod signal;
pub mod param;
mod voice;
mod bridge;
#[cfg(feature = "audio")]
mod engine;
pub mod golden;
mod render;
pub mod osc;
pub mod filter;
pub mod dynamics;
pub mod scope;
pub mod inspect;
pub mod spectrum;
pub mod midi;
pub mod osc_input;
pub mod mic;
pub mod hotswap;
pub mod crush;
pub mod delay;
pub mod pluck;
pub mod sample;
#[cfg(feature = "wav")]
pub mod wav;

pub use signal::{
    AudioContext, Signal, SignalExt,
    Add, Amp, Clip, Mix, Mul, Offset, Pan, SoftClip,
};
pub use crush::{BitCrush, Downsample};
pub use delay::{Delay, DELAY_MAX_SR, MAX_FEEDBACK};
pub use pluck::{pluck, Pluck};
pub use sample::{Sample, SampleError, Sampler, SamplerMode};
pub use param::Param;
pub use voice::VoicePool;
#[cfg(feature = "audio")]
pub use engine::{Engine, EngineConfig, EngineError};
pub use alloc_guard::{DenyAllocGuard, GuardedAllocator};
pub use bridge::{bridge, AudioCommand, BridgeReceiver, BridgeSender};
pub use render::render_to_buffer;
pub use filter::{Biquad, FilterExt, FilterMode};
pub use dynamics::{gain, peak_limiter, Gain, PeakLimiter};
pub use scope::{Scope, ScopeExt, ScopeHandle};
pub use inspect::{Inspect, InspectExt};
pub use spectrum::{
    FreqBin, Spectrum, SpectrumConfig, SpectrumExt, SpectrumHandle, WindowFn,
};
pub use midi::{
    parse_midi, midi_bridge, CcMap, CcSignal, CcWriter,
    MidiEvent, MidiReceiver, MidiSender,
};
#[cfg(feature = "midi")]
pub use midi::{open_midi_input, open_midi_input_named, MidiConnection, MidiError};
pub use osc_input::{OscParam, OscParamWriter, OscSignal};
#[cfg(feature = "osc")]
pub use osc_input::{osc_listen, OscListener, OscError};
#[cfg(feature = "audio")]
pub use mic::{mic, mic_with_buffer, MicHandle, MicError, MicSignal};
#[cfg(feature = "wav")]
pub use wav::{render_to_wav, render_to_wav_f32, WavError};
