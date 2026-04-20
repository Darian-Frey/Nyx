use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, Stream, StreamConfig};

use crate::signal::{AudioContext, Signal};

/// Configuration for the audio engine.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub sample_rate: u32,
    pub buffer_size: u32,
    pub channels: u16,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            sample_rate: 44100,
            buffer_size: 512, // ~11.6ms at 44100 Hz — well under 20ms target
            channels: 2,
        }
    }
}

/// A running audio output stream with device-error detection.
///
/// Holds the cpal `Stream` handle. Audio stops when this is dropped.
/// Check `has_error()` periodically from the main thread to detect
/// device disconnection, then call `reconnect()` with a new signal.
pub struct Engine {
    _stream: Stream,
    error_flag: Arc<AtomicBool>,
}

impl Engine {
    /// Open the default output device and start playing the given signal.
    pub fn play<S: Signal + 'static>(signal: S) -> Result<Self, EngineError> {
        Self::play_with(signal, EngineConfig::default())
    }

    /// Open the default output device with a custom configuration.
    pub fn play_with<S: Signal + 'static>(
        mut signal: S,
        config: EngineConfig,
    ) -> Result<Self, EngineError> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or(EngineError::NoOutputDevice)?;

        let stream_config = StreamConfig {
            channels: config.channels,
            sample_rate: config.sample_rate,
            buffer_size: BufferSize::Fixed(config.buffer_size),
        };

        let sample_rate = config.sample_rate as f32;
        let channels = config.channels as usize;
        let mut tick: u64 = 0;

        let error_flag = Arc::new(AtomicBool::new(false));
        let error_flag_cb = Arc::clone(&error_flag);

        let stream = device
            .build_output_stream(
                &stream_config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    for frame in data.chunks_mut(channels) {
                        let ctx = AudioContext { sample_rate, tick };
                        let (left, right) = signal.next_stereo(&ctx);
                        match channels {
                            1 => {
                                // Mono output: fold L+R.
                                frame[0] = left + right;
                            }
                            _ => {
                                frame[0] = left;
                                if let Some(r_slot) = frame.get_mut(1) {
                                    *r_slot = right;
                                }
                                // Surround / extra channels: fill with
                                // (L+R)/2 mono mix so nothing's silent.
                                let mono = (left + right) * 0.5;
                                for out in frame.iter_mut().skip(2) {
                                    *out = mono;
                                }
                            }
                        }
                        tick += 1;
                    }
                },
                move |err| {
                    eprintln!("nyx: audio stream error: {err}");
                    error_flag_cb.store(true, Ordering::Release);
                },
                None,
            )
            .map_err(EngineError::BuildStream)?;

        stream.play().map_err(EngineError::PlayStream)?;

        Ok(Engine {
            _stream: stream,
            error_flag,
        })
    }

    /// Returns `true` if the audio stream has encountered an error
    /// (e.g. device disconnection). Call `reconnect()` to recover.
    pub fn has_error(&self) -> bool {
        self.error_flag.load(Ordering::Acquire)
    }

    /// Drop the current stream and attempt to open a new one on the
    /// default output device. The caller provides a fresh signal because
    /// the old one was moved into the (now-dead) stream.
    ///
    /// Returns a new `Engine` on success, or an error if no device is
    /// available yet (caller should retry after a delay).
    pub fn reconnect<S: Signal + 'static>(
        self,
        signal: S,
        config: EngineConfig,
    ) -> Result<Self, EngineError> {
        // Drop the old stream by consuming self, then build a new one.
        drop(self);
        Self::play_with(signal, config)
    }

    /// Query the default output device name (useful for diagnostics).
    pub fn default_device_name() -> Option<String> {
        let host = cpal::default_host();
        let device = host.default_output_device()?;
        device.description().ok().map(|d| d.name().to_string())
    }
}

/// Errors that can occur when starting the audio engine.
#[derive(Debug)]
pub enum EngineError {
    NoOutputDevice,
    BuildStream(cpal::BuildStreamError),
    PlayStream(cpal::PlayStreamError),
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EngineError::NoOutputDevice => write!(f, "no audio output device found"),
            EngineError::BuildStream(e) => write!(f, "failed to build audio stream: {e}"),
            EngineError::PlayStream(e) => write!(f, "failed to start audio stream: {e}"),
        }
    }
}

impl std::error::Error for EngineError {}
