//! Nyx-Audio WASM demo — the "hello sine" of Phase B.
//!
//! This crate exposes a thin wasm-bindgen wrapper around
//! `nyx_prelude::Engine` so a browser can start and stop a Nyx signal
//! graph. It is not part of the workspace's `default-members`; build
//! it explicitly with `wasm-pack` (see `README.md`).
//!
//! ```js
//! import init, { NyxDemo } from "./pkg/nyx_wasm_demo.js";
//! await init();
//! const demo = new NyxDemo(); // starts audio — must be in a user gesture
//! // later:
//! demo.free();                 // stops audio
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

/// Handle to a running Nyx audio stream.
///
/// Dropping the handle (or calling `.free()` from JS) stops playback.
#[wasm_bindgen]
pub struct NyxDemo {
    _engine: Engine,
}

#[wasm_bindgen]
impl NyxDemo {
    /// Build the default demo — a gentle detuned sawtooth pad through
    /// a low-pass filter and a touch of Freeverb — and start playing.
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

        let engine = Engine::play(pad).map_err(|e| JsError::new(&format!("{e:?}")))?;
        Ok(NyxDemo { _engine: engine })
    }

    /// Report whether the underlying audio stream has entered an error
    /// state (e.g. the browser suspended the `AudioContext`).
    pub fn has_error(&self) -> bool {
        self._engine.has_error()
    }
}
