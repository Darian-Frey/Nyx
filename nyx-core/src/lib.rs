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

pub use signal::{
    AudioContext, Signal, SignalExt,
    Add, Amp, Clip, Mix, Mul, Offset, Pan, SoftClip,
};
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
