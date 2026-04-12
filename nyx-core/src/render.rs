use crate::signal::{AudioContext, Signal};

/// Render a signal to a buffer of mono samples without audio hardware.
///
/// This is the workhorse for testing — every DSP unit can be validated
/// offline without opening an audio device.
///
/// # Arguments
/// * `signal` — the signal to render (consumed by mutable reference)
/// * `duration_secs` — how many seconds of audio to produce
/// * `sample_rate` — sample rate in Hz (e.g. 44100.0)
///
/// # Returns
/// A `Vec<f32>` containing `(duration_secs * sample_rate)` samples.
pub fn render_to_buffer(signal: &mut dyn Signal, duration_secs: f32, sample_rate: f32) -> Vec<f32> {
    let num_samples = (duration_secs * sample_rate) as usize;
    let mut buf = Vec::with_capacity(num_samples);
    for tick in 0..num_samples {
        let ctx = AudioContext {
            sample_rate,
            tick: tick as u64,
        };
        buf.push(signal.next(&ctx));
    }
    buf
}
