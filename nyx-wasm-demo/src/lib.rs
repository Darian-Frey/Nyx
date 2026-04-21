//! Nyx-Audio WASM demo — proves Phase B (browser) works end-to-end.
//!
//! This crate exposes a thin wasm-bindgen wrapper around
//! `nyx_prelude::Engine` so a browser can start and stop a Nyx signal
//! graph and read live waveform / spectrum data back out. It is not
//! part of the workspace's `default-members`; build it explicitly with
//! `wasm-pack` (see `README.md`).
//!
//! ```js
//! import init, { NyxDemo } from "./pkg/nyx_wasm_demo.js";
//! await init();
//! const demo = new NyxDemo(); // starts audio — must be in a user gesture
//!
//! // Read the scope and spectrum every animation frame:
//! const scope = new Float32Array(2048);
//! const spec  = new Float32Array(2 * demo.spectrum_bin_count());
//! function draw() {
//!     requestAnimationFrame(draw);
//!     const n = demo.read_scope(scope);
//!     const m = demo.read_spectrum(spec);
//!     // ... paint n scope samples and m (freq, mag) bins ...
//! }
//! draw();
//!
//! // later:
//! demo.free();                 // stops audio and releases the handles
//! ```
//!
//! **Autoplay policy.** Modern browsers require a user gesture (click,
//! keypress) before an `AudioContext` is allowed to start. `NyxDemo::new`
//! therefore must be called from inside a `click`/`keydown` handler.
//! Calling it on page load will build the WebAudio graph but produce
//! silence until the context is resumed.

#![cfg(target_arch = "wasm32")]

use nyx_prelude::*;
use wasm_bindgen::prelude::*;

/// Size of the scope ring buffer. At 44.1 kHz this is ≈ 93 ms of audio,
/// which comfortably covers one `requestAnimationFrame` tick (~16 ms at
/// 60 fps) while still fitting the tightest display budget.
const SCOPE_BUFFER: usize = 4096;

/// Handle to a running Nyx audio stream with scope + spectrum taps.
///
/// Dropping the handle (or calling `.free()` from JS) stops playback
/// and releases the analysis ring buffers.
#[wasm_bindgen]
pub struct NyxDemo {
    _engine: Engine,
    scope: ScopeHandle,
    spectrum: SpectrumHandle,
}

#[wasm_bindgen]
impl NyxDemo {
    /// Build the default demo — a gentle detuned sawtooth pad through a
    /// low-pass filter and Freeverb — install scope + spectrum taps on
    /// the signal, and start playing.
    ///
    /// Returns an error if the browser denies an `AudioContext`
    /// (for example, when called outside a user gesture).
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<NyxDemo, JsError> {
        // Surface Rust panics in the browser devtools console.
        console_error_panic_hook::set_once();

        // A3 + C4 + E4 detuned sawtooth pad, lowpassed, with reverb.
        let pad = osc::saw(Note::from_midi(57).to_freq())
            .add(osc::saw(Note::C4.to_freq() * 1.003))
            .add(osc::saw(Note::E4.to_freq() * 0.997))
            .amp(0.08)
            .lowpass(1400.0, 0.7)
            .freeverb()
            .room_size(0.82)
            .damping(0.4)
            .wet(0.35);

        // Tap the signal for visualisation before handing it to the engine.
        // Both taps are pass-throughs — they don't change the audio.
        let (pad, scope_handle) = pad.scope(SCOPE_BUFFER);
        let (pad, spectrum_handle) = pad.spectrum(SpectrumConfig::default());

        let engine = Engine::play(pad).map_err(|e| JsError::new(&format!("{e:?}")))?;
        Ok(NyxDemo {
            _engine: engine,
            scope: scope_handle,
            spectrum: spectrum_handle,
        })
    }

    /// Drain up to `out.len()` waveform samples from the scope tap into
    /// `out`. Returns the number of samples actually written.
    ///
    /// Call this from a `requestAnimationFrame` loop with a reusable
    /// `Float32Array` to draw a live oscilloscope without allocating
    /// per frame.
    pub fn read_scope(&mut self, out: &mut [f32]) -> usize {
        self.scope.read(out)
    }

    /// Number of waveform samples currently waiting in the scope ring
    /// buffer. Useful for deciding whether to redraw.
    pub fn scope_available(&self) -> usize {
        self.scope.available()
    }

    /// Number of frequency bins in the latest spectrum snapshot.
    /// Returns `0` until the first FFT frame has been captured.
    pub fn spectrum_bin_count(&self) -> usize {
        self.spectrum.bin_count()
    }

    /// Copy the latest spectrum into `out` as interleaved
    /// `(freq_hz, magnitude)` pairs. `out` should be sized to
    /// `2 * spectrum_bin_count()`. Returns the number of bins written.
    pub fn read_spectrum(&self, out: &mut [f32]) -> usize {
        let bins = self.spectrum.snapshot();
        let n = (out.len() / 2).min(bins.len());
        for (i, bin) in bins.iter().take(n).enumerate() {
            out[2 * i] = bin.freq;
            out[2 * i + 1] = bin.magnitude;
        }
        n
    }

    /// Report whether the underlying audio stream has entered an error
    /// state (e.g. the browser suspended the `AudioContext`).
    pub fn has_error(&self) -> bool {
        self._engine.has_error()
    }
}
