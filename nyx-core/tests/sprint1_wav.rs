//! Sprint 1 — WAV export tests.

#![cfg(feature = "wav")]

use nyx_core::{AudioContext, Signal, WavError, render_to_wav, render_to_wav_f32};

const SR: f32 = 44100.0;

/// Helper: a signal that emits a fixed value on every call.
struct Const(f32);
impl Signal for Const {
    fn next(&mut self, _ctx: &AudioContext) -> f32 {
        self.0
    }
}

/// Helper: a signal that emits a sine at `freq`.
struct Sine {
    phase: f32,
    freq: f32,
}
impl Signal for Sine {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let out = (self.phase * std::f32::consts::TAU).sin();
        self.phase += self.freq / ctx.sample_rate;
        self.phase -= self.phase.floor();
        out
    }
}

fn tmp_path(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("nyx-wav-tests");
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn cleanup(path: &std::path::Path) {
    let _ = std::fs::remove_file(path);
}

// ───────────────────────── Basic contract ─────────────────────────

#[test]
fn exact_duration_produces_exact_sample_count() {
    let path = tmp_path("duration.wav");
    render_to_wav(Const(0.5), 1.0, SR, &path).unwrap();

    let reader = hound::WavReader::open(&path).unwrap();
    let expected = SR as u32;
    assert_eq!(
        reader.len(),
        expected,
        "expected {expected} samples for 1s @ 44100Hz, got {}",
        reader.len()
    );
    cleanup(&path);
}

#[test]
fn longer_duration() {
    let path = tmp_path("longer.wav");
    render_to_wav(Const(0.1), 2.5, SR, &path).unwrap();

    let reader = hound::WavReader::open(&path).unwrap();
    let expected = (2.5 * SR) as u32;
    assert_eq!(reader.len(), expected);
    cleanup(&path);
}

#[test]
fn invalid_duration_rejected() {
    let path = tmp_path("invalid_dur.wav");
    assert!(matches!(
        render_to_wav(Const(0.0), 0.0, SR, &path),
        Err(WavError::InvalidDuration(_))
    ));
    assert!(matches!(
        render_to_wav(Const(0.0), -1.0, SR, &path),
        Err(WavError::InvalidDuration(_))
    ));
    assert!(matches!(
        render_to_wav(Const(0.0), f32::NAN, SR, &path),
        Err(WavError::InvalidDuration(_))
    ));
}

#[test]
fn invalid_sample_rate_rejected() {
    let path = tmp_path("invalid_sr.wav");
    assert!(matches!(
        render_to_wav(Const(0.0), 1.0, 0.0, &path),
        Err(WavError::InvalidSampleRate(_))
    ));
    assert!(matches!(
        render_to_wav(Const(0.0), 1.0, -48000.0, &path),
        Err(WavError::InvalidSampleRate(_))
    ));
}

// ───────────────────────── Clipping ─────────────────────────

#[test]
fn clipping_clamps_positive_overdrive() {
    let path = tmp_path("clip_pos.wav");
    render_to_wav(Const(2.5), 0.01, SR, &path).unwrap();

    let reader = hound::WavReader::open(&path).unwrap();
    let spec = reader.spec();
    assert_eq!(spec.bits_per_sample, 16);

    let mut reader = reader;
    let max = reader.samples::<i16>().map(|s| s.unwrap()).max().unwrap();
    // 2.5 clamped to 1.0 → i16::MAX (32767). Allow 1-LSB rounding error.
    assert!(max >= i16::MAX - 1, "expected ~i16::MAX, got {max}");
    cleanup(&path);
}

#[test]
fn clipping_clamps_negative_overdrive() {
    let path = tmp_path("clip_neg.wav");
    render_to_wav(Const(-3.0), 0.01, SR, &path).unwrap();

    let mut reader = hound::WavReader::open(&path).unwrap();
    let min = reader.samples::<i16>().map(|s| s.unwrap()).min().unwrap();
    assert!(min <= -i16::MAX + 1, "expected ~-i16::MAX, got {min}");
    cleanup(&path);
}

#[test]
fn in_range_signal_not_clipped() {
    let path = tmp_path("no_clip.wav");
    render_to_wav(Const(0.5), 0.01, SR, &path).unwrap();

    let mut reader = hound::WavReader::open(&path).unwrap();
    let first = reader.samples::<i16>().next().unwrap().unwrap();
    // 0.5 * i16::MAX = 16383 (or 16384 depending on rounding)
    assert!((first - 16383).abs() <= 1, "expected ~16383, got {first}");
    cleanup(&path);
}

// ───────────────────────── Format checks ─────────────────────────

#[test]
fn i16_format_spec_is_correct() {
    let path = tmp_path("format_i16.wav");
    render_to_wav(Const(0.0), 0.01, 48000.0, &path).unwrap();

    let reader = hound::WavReader::open(&path).unwrap();
    let spec = reader.spec();
    assert_eq!(spec.channels, 1);
    assert_eq!(spec.sample_rate, 48000);
    assert_eq!(spec.bits_per_sample, 16);
    assert_eq!(spec.sample_format, hound::SampleFormat::Int);
    cleanup(&path);
}

#[test]
fn f32_format_spec_is_correct() {
    let path = tmp_path("format_f32.wav");
    render_to_wav_f32(Const(0.0), 0.01, 96000.0, &path).unwrap();

    let reader = hound::WavReader::open(&path).unwrap();
    let spec = reader.spec();
    assert_eq!(spec.channels, 1);
    assert_eq!(spec.sample_rate, 96000);
    assert_eq!(spec.bits_per_sample, 32);
    assert_eq!(spec.sample_format, hound::SampleFormat::Float);
    cleanup(&path);
}

#[test]
fn f32_preserves_overdrive() {
    // f32 export should NOT clamp — faithful recording of signal values.
    let path = tmp_path("f32_overdrive.wav");
    render_to_wav_f32(Const(2.5), 0.01, SR, &path).unwrap();

    let mut reader = hound::WavReader::open(&path).unwrap();
    let first: f32 = reader.samples::<f32>().next().unwrap().unwrap();
    assert!((first - 2.5).abs() < 1e-5, "expected 2.5, got {first}");
    cleanup(&path);
}

// ───────────────────────── Round-trip ─────────────────────────

#[test]
fn sine_roundtrip_preserves_frequency() {
    // Render 1 kHz sine, read back, confirm the peak-to-peak pattern
    // matches a 1 kHz oscillation by checking zero crossings.
    let path = tmp_path("sine_1khz.wav");
    let sig = Sine {
        phase: 0.0,
        freq: 1000.0,
    };
    render_to_wav_f32(sig, 1.0, SR, &path).unwrap();

    let mut reader = hound::WavReader::open(&path).unwrap();
    let samples: Vec<f32> = reader.samples::<f32>().map(|s| s.unwrap()).collect();

    // Count zero crossings — a 1 kHz sine over 1 second should have
    // ~2000 zero crossings (2 per cycle × 1000 cycles).
    let mut crossings: i32 = 0;
    for i in 1..samples.len() {
        if samples[i - 1].signum() != samples[i].signum() {
            crossings += 1;
        }
    }
    assert!(
        (crossings - 2000).abs() < 20,
        "expected ~2000 zero crossings, got {crossings}"
    );
    cleanup(&path);
}

#[test]
fn f32_roundtrip_bit_exact() {
    // Write a known sequence, read it back, confirm exact equality.
    let path = tmp_path("f32_exact.wav");
    let values = [0.0_f32, 0.1, -0.5, 0.75, -0.99];
    let mut iter = values.iter().copied();
    let sig = move |_ctx: &AudioContext| iter.next().unwrap_or(0.0);
    render_to_wav_f32(sig, values.len() as f32 / SR, SR, &path).unwrap();

    let mut reader = hound::WavReader::open(&path).unwrap();
    let read: Vec<f32> = reader.samples::<f32>().map(|s| s.unwrap()).collect();
    for (i, &v) in values.iter().enumerate() {
        assert!(
            (read[i] - v).abs() < 1e-6,
            "mismatch at {i}: {v} vs {}",
            read[i]
        );
    }
    cleanup(&path);
}
