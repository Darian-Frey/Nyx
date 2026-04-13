//! Microphone / audio input as a `Signal`.
//!
//! Requires the `audio` feature (cpal). Opens the default input device
//! and streams samples into a lock-free ring buffer. The `MicSignal`
//! reads from the buffer on the audio output thread.

use rtrb::{Consumer, RingBuffer};

use crate::signal::{AudioContext, Signal};

/// A `Signal` that reads audio samples from the microphone input.
///
/// If the input buffer runs empty (underrun), outputs silence.
pub struct MicSignal {
    consumer: Consumer<f32>,
}

impl MicSignal {
    /// Create a `MicSignal` from a pre-existing consumer (for testing).
    pub fn from_consumer(consumer: Consumer<f32>) -> Self {
        MicSignal { consumer }
    }
}

impl Signal for MicSignal {
    fn next(&mut self, _ctx: &AudioContext) -> f32 {
        self.consumer.pop().unwrap_or(0.0)
    }
}

/// Open the default audio input device and return a `MicSignal`.
///
/// The returned `MicHandle` must be kept alive — audio input stops
/// when it is dropped.
#[cfg(feature = "audio")]
pub fn mic() -> Result<(MicSignal, MicHandle), MicError> {
    mic_with_buffer(4096)
}

/// Open the default audio input with a custom buffer size.
#[cfg(feature = "audio")]
pub fn mic_with_buffer(buffer_size: usize) -> Result<(MicSignal, MicHandle), MicError> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or(MicError::NoInputDevice)?;

    let config = device
        .default_input_config()
        .map_err(|e| MicError::Config(e.to_string()))?;

    let (mut producer, consumer) = RingBuffer::<f32>::new(buffer_size);

    let stream = device
        .build_input_stream(
            &config.into(),
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                for &sample in data {
                    let _ = producer.push(sample);
                }
            },
            move |err| {
                eprintln!("nyx: mic input error: {err}");
            },
            None,
        )
        .map_err(|e| MicError::BuildStream(e.to_string()))?;

    stream
        .play()
        .map_err(|e| MicError::PlayStream(e.to_string()))?;

    Ok((
        MicSignal { consumer },
        MicHandle { _stream: stream },
    ))
}

/// Handle for the mic input stream. Dropping this stops the input.
#[cfg(feature = "audio")]
pub struct MicHandle {
    _stream: cpal::Stream,
}

/// Errors from mic input setup.
#[derive(Debug)]
pub enum MicError {
    NoInputDevice,
    Config(String),
    BuildStream(String),
    PlayStream(String),
}

impl std::fmt::Display for MicError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MicError::NoInputDevice => write!(f, "no audio input device found"),
            MicError::Config(e) => write!(f, "input config error: {e}"),
            MicError::BuildStream(e) => write!(f, "failed to build input stream: {e}"),
            MicError::PlayStream(e) => write!(f, "failed to start input stream: {e}"),
        }
    }
}

impl std::error::Error for MicError {}
