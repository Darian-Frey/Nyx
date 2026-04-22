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

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicU32, AtomicU64, Ordering};

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

        Self::start(pad)
    }

    /// Start the 90-second Tron: Legacy-style electro-orchestral cue.
    ///
    /// See [`nyx_prelude::demos::tron`] for the full musical breakdown.
    /// After ~90 s the track enters its decay section and fades to
    /// silence; call `.free()` and then this again to restart it.
    pub fn tron() -> Result<NyxDemo, JsError> {
        console_error_panic_hook::set_once();
        Self::start(demos::tron())
    }

    /// Internal: install scope + spectrum taps and hand the signal to the
    /// audio engine. Shared by every demo constructor so the
    /// visualisation pipeline stays identical across patches.
    fn start<S: Signal + 'static>(signal: S) -> Result<NyxDemo, JsError> {
        let (signal, scope_handle) = signal.scope(SCOPE_BUFFER);
        let (signal, spectrum_handle) = signal.spectrum(SpectrumConfig::default());

        let engine = Engine::play(signal).map_err(|e| JsError::new(&format!("{e:?}")))?;
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

/// Shared command block between JS (producer) and the audio thread
/// (consumer). The `counter` field is edge-triggered — every time JS
/// calls [`NyxPresets::play_note`] it increments it; the audio loop
/// notices the change, reads `preset` + `freq`, and dispatches a
/// trigger on the matching voice.
///
/// WASM runs single-threaded (ScriptProcessorNode on the main thread),
/// but atomic accesses keep the code correct for any backend — the
/// same struct will fit a cpal native build where audio runs off-main.
struct TriggerState {
    /// Incremented by `play_note`; watched by the audio loop.
    counter: AtomicU64,
    /// Preset index 0..=5. See `PRESET_NAMES`.
    preset: AtomicU8,
    /// Requested pitch in Hz, stored as `f32` bits.
    freq: AtomicU32,
}

/// Symbolic preset names — order must match the dispatch in
/// [`NyxPresets::new`]. Exposed to JS so the HTML can be generated
/// from this list rather than hard-coded twice.
const PRESET_NAMES: [&str; 9] = [
    "tb303",
    "moog_bass",
    "supersaw",
    "prophet_pad",
    "dx7_bell",
    "noise_sweep",
    "juno_pad",
    "handpan",
    "chime",
];

/// Interactive "pick a preset, play a note" demo.
///
/// Holds every preset pre-allocated and routes JS-side `play_note`
/// calls to the chosen one via an atomic command block. The unused
/// presets are still being `.next()`-ed each sample for state
/// coherence; their built-in envelopes keep them silent until
/// triggered.
#[wasm_bindgen]
pub struct NyxPresets {
    state: Arc<TriggerState>,
    _engine: Engine,
    scope: ScopeHandle,
    spectrum: SpectrumHandle,
}

#[wasm_bindgen]
impl NyxPresets {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<NyxPresets, JsError> {
        console_error_panic_hook::set_once();

        let state = Arc::new(TriggerState {
            counter: AtomicU64::new(0),
            preset: AtomicU8::new(0),
            freq: AtomicU32::new(440.0_f32.to_bits()),
        });
        let state_audio = Arc::clone(&state);

        let mut tb = presets::tb303(110.0);
        let mut moog = presets::moog_bass(55.0);
        let mut saw = presets::supersaw(440.0);
        // Supersaw has no intrinsic envelope — wrap with an external one
        // that JS retriggers alongside the oscillator.
        let mut saw_env = envelope::adsr(0.03, 0.25, 0.70, 0.35);
        let mut prophet = presets::prophet_pad(220.0);
        let mut bell = presets::dx7_bell(523.25);
        let mut sweep = presets::noise_sweep(1.2);
        let mut juno = presets::juno_pad(220.0);
        let mut pan = presets::handpan(261.63);
        let mut chime = presets::chime(440.0);

        let mut last_counter: u64 = 0;

        let signal = move |ctx: &AudioContext| {
            // Edge-triggered command dispatch.
            let cur = state_audio.counter.load(Ordering::Relaxed);
            if cur != last_counter {
                last_counter = cur;
                let preset = state_audio.preset.load(Ordering::Relaxed);
                let freq = f32::from_bits(state_audio.freq.load(Ordering::Relaxed));
                match preset {
                    0 => {
                        tb.set_freq(freq);
                        tb.trigger();
                    }
                    1 => {
                        moog.set_freq(freq);
                        moog.trigger();
                    }
                    2 => {
                        saw.set_freq(freq);
                        saw_env.trigger();
                    }
                    3 => {
                        prophet.set_freq(freq);
                        prophet.trigger();
                    }
                    4 => {
                        bell.set_freq(freq);
                        bell.trigger();
                    }
                    5 => sweep.trigger(),
                    6 => {
                        juno.set_freq(freq);
                        juno.trigger();
                    }
                    7 => {
                        pan.set_freq(freq);
                        pan.trigger();
                    }
                    _ => {
                        chime.set_freq(freq);
                        chime.trigger();
                    }
                }
            }

            // Run every voice to keep internal state coherent; their
            // built-in envelopes keep them silent between notes.
            let tb_s = tb.next(ctx);
            let moog_s = moog.next(ctx);
            let saw_s = saw.next(ctx) * saw_env.next(ctx);
            let prophet_s = prophet.next(ctx);
            let bell_s = bell.next(ctx);
            let sweep_s = sweep.next(ctx);
            let juno_s = juno.next(ctx);
            let pan_s = pan.next(ctx);
            let chime_s = chime.next(ctx);

            let mix =
                tb_s + moog_s + saw_s + prophet_s + bell_s + sweep_s + juno_s + pan_s + chime_s;
            (mix * 0.40).tanh()
        };

        let (signal, scope_handle) = signal.scope(SCOPE_BUFFER);
        let (signal, spectrum_handle) = signal.spectrum(SpectrumConfig::default());
        let engine = Engine::play(signal).map_err(|e| JsError::new(&format!("{e:?}")))?;

        Ok(NyxPresets {
            state,
            _engine: engine,
            scope: scope_handle,
            spectrum: spectrum_handle,
        })
    }

    /// Fire a note on the given preset.
    ///
    /// - `preset`: index into [`PRESET_NAMES`] (0..=5).
    /// - `freq_hz`: pitch for the note. Ignored by `noise_sweep` (index 5).
    ///
    /// Safe to call repeatedly from any JS event handler. The audio
    /// loop picks up the change on its next sample.
    pub fn play_note(&self, preset: u8, freq_hz: f32) {
        // Clamp the preset index to the valid range so an out-of-range
        // value from JS can't wrap to a surprising voice.
        let p = if (preset as usize) < PRESET_NAMES.len() {
            preset
        } else {
            0
        };
        self.state.preset.store(p, Ordering::Relaxed);
        self.state.freq.store(freq_hz.to_bits(), Ordering::Relaxed);
        self.state.counter.fetch_add(1, Ordering::Relaxed);
    }

    /// Return the list of available preset names as a JSON-like string
    /// (comma-separated). Use this from JS to populate the dropdown
    /// without hard-coding the names in two places.
    pub fn preset_names() -> String {
        PRESET_NAMES.join(",")
    }

    pub fn read_scope(&mut self, out: &mut [f32]) -> usize {
        self.scope.read(out)
    }

    pub fn scope_available(&self) -> usize {
        self.scope.available()
    }

    pub fn spectrum_bin_count(&self) -> usize {
        self.spectrum.bin_count()
    }

    pub fn read_spectrum(&self, out: &mut [f32]) -> usize {
        let bins = self.spectrum.snapshot();
        let n = (out.len() / 2).min(bins.len());
        for (i, bin) in bins.iter().take(n).enumerate() {
            out[2 * i] = bin.freq;
            out[2 * i + 1] = bin.magnitude;
        }
        n
    }

    pub fn has_error(&self) -> bool {
        self._engine.has_error()
    }
}
