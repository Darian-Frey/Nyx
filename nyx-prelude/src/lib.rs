// Re-export core types
pub use nyx_core::{
    AudioContext, Param, Signal, SignalExt, VoicePool,
    Add, Amp, Clip, Mix, Mul, Offset, Pan, SoftClip,
    render_to_buffer,
};
pub use nyx_core::param::{ConstSignal, IntoParam};
pub use nyx_core::golden;

// Re-export bridge
pub use nyx_core::{bridge, AudioCommand, BridgeReceiver, BridgeSender};

// Re-export alloc guard
pub use nyx_core::{DenyAllocGuard, GuardedAllocator};

// Re-export engine (audio feature only)
#[cfg(feature = "audio")]
pub use nyx_core::{Engine, EngineConfig, EngineError};

/// Start playing a signal on the default audio output device.
///
/// Blocks the current thread until the user presses Enter.
/// This is the quickest way to hear a signal:
///
/// ```ignore
/// use nyx_prelude::*;
///
/// fn main() {
///     play(|_ctx: &AudioContext| 0.0); // silence
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
