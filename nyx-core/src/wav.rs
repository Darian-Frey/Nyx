//! WAV file export — offline render a signal to disk.
//!
//! Wraps `render_to_buffer` with a `hound` WAV writer. Two variants:
//! [`render_to_wav`] writes 16-bit signed PCM (universal, smaller files,
//! matches DAW import expectations), and [`render_to_wav_f32`] writes
//! 32-bit float (exact signal preservation, larger files).
//!
//! **This is main-thread only.** Never call from the audio callback —
//! it allocates, blocks on I/O, and takes potentially many seconds to
//! return. Use it for offline composition rendering, not live playback.
//!
//! Gated behind the `wav` feature (enabled by default).

use std::path::Path;

use crate::render::render_to_buffer;
use crate::signal::Signal;

/// Errors produced by WAV export.
#[derive(Debug, thiserror::Error)]
pub enum WavError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("hound error: {0}")]
    Hound(#[from] hound::Error),
    #[error("invalid duration (must be > 0): {0}")]
    InvalidDuration(f32),
    #[error("invalid sample rate (must be > 0): {0}")]
    InvalidSampleRate(f32),
}

/// Render a signal to a 16-bit signed PCM mono WAV file.
///
/// - `signal` — the signal to render (consumed)
/// - `duration_secs` — how many seconds of audio to produce
/// - `sample_rate` — sample rate in Hz (typically 44100.0 or 48000.0)
/// - `path` — output file path
///
/// Samples exceeding `[-1.0, 1.0]` are hard-clamped (not wrapped) before
/// quantisation to 16-bit integers. This is standard behaviour for WAV
/// export — signals that legally exceed unity gain still produce a valid
/// file, just clipped at full scale.
///
/// # Example
///
/// ```ignore
/// use nyx_core::{osc, render_to_wav, SignalExt};
///
/// let signal = osc::sine(440.0).amp(0.3);
/// render_to_wav(signal, 5.0, 44100.0, "tone.wav")?;
/// ```
pub fn render_to_wav<S: Signal>(
    mut signal: S,
    duration_secs: f32,
    sample_rate: f32,
    path: impl AsRef<Path>,
) -> Result<(), WavError> {
    if duration_secs <= 0.0 || !duration_secs.is_finite() {
        return Err(WavError::InvalidDuration(duration_secs));
    }
    if sample_rate <= 0.0 || !sample_rate.is_finite() {
        return Err(WavError::InvalidSampleRate(sample_rate));
    }

    let buf = render_to_buffer(&mut signal, duration_secs, sample_rate);

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: sample_rate as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for sample in buf {
        let clamped = sample.clamp(-1.0, 1.0);
        let int_sample = (clamped * i16::MAX as f32) as i16;
        writer.write_sample(int_sample)?;
    }
    writer.finalize()?;
    Ok(())
}

/// Render a signal to a 32-bit float mono WAV file.
///
/// Same contract as [`render_to_wav`] but preserves the exact signal
/// without quantisation. No clamping is applied — the file faithfully
/// records whatever the signal produced, including values outside
/// `[-1.0, 1.0]`.
///
/// Use this when you want a lossless render for further processing in
/// a DAW or analysis tool. Files are ~2× larger than the 16-bit version.
pub fn render_to_wav_f32<S: Signal>(
    mut signal: S,
    duration_secs: f32,
    sample_rate: f32,
    path: impl AsRef<Path>,
) -> Result<(), WavError> {
    if duration_secs <= 0.0 || !duration_secs.is_finite() {
        return Err(WavError::InvalidDuration(duration_secs));
    }
    if sample_rate <= 0.0 || !sample_rate.is_finite() {
        return Err(WavError::InvalidSampleRate(sample_rate));
    }

    let buf = render_to_buffer(&mut signal, duration_secs, sample_rate);

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: sample_rate as u32,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for sample in buf {
        writer.write_sample(sample)?;
    }
    writer.finalize()?;
    Ok(())
}
