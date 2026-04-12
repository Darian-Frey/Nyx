//! Spectrum analysis tap: `.spectrum()` wraps a signal, collects frames,
//! runs FFT via `spectrum-analyzer`, and publishes magnitude bins to a
//! lock-free handle readable from the UI thread.

use std::sync::{Arc, Mutex};

use spectrum_analyzer::windows::{blackman_harris_4term, hann_window};
use spectrum_analyzer::{samples_fft_to_spectrum, FrequencyLimit};

use crate::signal::{AudioContext, Signal};

/// Window function applied before FFT.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowFn {
    Hann,
    Blackman,
}

/// Configuration for spectrum analysis.
#[derive(Debug, Clone)]
pub struct SpectrumConfig {
    /// FFT frame size. Must be a power of 2 (e.g. 1024, 2048, 4096).
    pub frame_size: usize,
    /// Window function applied before FFT.
    pub window: WindowFn,
}

impl Default for SpectrumConfig {
    fn default() -> Self {
        Self {
            frame_size: 2048,
            window: WindowFn::Hann,
        }
    }
}

/// A single frequency bin: frequency in Hz and magnitude (linear).
#[derive(Debug, Clone, Copy)]
pub struct FreqBin {
    pub freq: f32,
    pub magnitude: f32,
}

/// Handle for reading spectrum data from the UI/render thread.
///
/// The spectrum is updated every `frame_size` samples. Reading is
/// lock-based (not lock-free) because FFT results are large and
/// updated infrequently. The lock is held only for the duration of
/// a memcpy, so contention is minimal.
pub struct SpectrumHandle {
    bins: Arc<Mutex<Vec<FreqBin>>>,
}

impl SpectrumHandle {
    /// Copy the latest spectrum bins into `out`. Returns the number
    /// of bins written (may be less than `out.len()`).
    pub fn read(&self, out: &mut [FreqBin]) -> usize {
        let bins = self.bins.lock().unwrap();
        let n = out.len().min(bins.len());
        out[..n].copy_from_slice(&bins[..n]);
        n
    }

    /// Clone the latest spectrum bins into a new Vec.
    pub fn snapshot(&self) -> Vec<FreqBin> {
        self.bins.lock().unwrap().clone()
    }

    /// Number of bins in the latest spectrum.
    pub fn bin_count(&self) -> usize {
        self.bins.lock().unwrap().len()
    }
}

/// A signal wrapper that collects samples and publishes FFT results.
pub struct Spectrum<A: Signal> {
    source: A,
    config: SpectrumConfig,
    frame: Vec<f32>,
    cursor: usize,
    bins: Arc<Mutex<Vec<FreqBin>>>,
    sample_rate: f32,
}

impl<A: Signal> Signal for Spectrum<A> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let sample = self.source.next(ctx);
        self.sample_rate = ctx.sample_rate;

        self.frame[self.cursor] = sample;
        self.cursor += 1;

        if self.cursor >= self.config.frame_size {
            self.cursor = 0;
            self.compute_spectrum();
        }

        sample
    }
}

impl<A: Signal> Spectrum<A> {
    fn compute_spectrum(&self) {
        let mut windowed = self.frame.clone();

        match self.config.window {
            WindowFn::Hann => {
                let window = hann_window(&windowed);
                windowed.copy_from_slice(&window);
            }
            WindowFn::Blackman => {
                let window = blackman_harris_4term(&windowed);
                windowed.copy_from_slice(&window);
            }
        }

        let result = samples_fft_to_spectrum(
            &windowed,
            self.sample_rate as u32,
            FrequencyLimit::All,
            None,
        );

        if let Ok(spectrum) = result {
            let new_bins: Vec<FreqBin> = spectrum
                .data()
                .iter()
                .map(|&(freq, mag)| FreqBin {
                    freq: freq.val(),
                    magnitude: mag.val(),
                })
                .collect();

            if let Ok(mut bins) = self.bins.lock() {
                *bins = new_bins;
            }
        }
    }
}

/// Extension trait adding `.spectrum()` to all signals.
pub trait SpectrumExt: Signal + Sized {
    /// Tap this signal for spectrum analysis.
    ///
    /// Returns `(wrapped_signal, handle)`. Pass the signal to the audio
    /// engine and keep the handle for reading spectrum data on the UI thread.
    fn spectrum(self, config: SpectrumConfig) -> (Spectrum<Self>, SpectrumHandle) {
        let frame_size = config.frame_size;
        let bins = Arc::new(Mutex::new(Vec::new()));
        let handle = SpectrumHandle {
            bins: Arc::clone(&bins),
        };
        let spectrum = Spectrum {
            source: self,
            config,
            frame: vec![0.0; frame_size],
            cursor: 0,
            bins,
            sample_rate: 44100.0,
        };
        (spectrum, handle)
    }
}

impl<T: Signal + Sized> SpectrumExt for T {}
