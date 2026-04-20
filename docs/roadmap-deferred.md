# Deferred Roadmap — Large Multi-Phase Features

This document is the planning doc for features that were intentionally
pushed past v1.0: **DAW integration** (JACK/PipeWire), **WASM target**,
and the **mini-notation DSL** (Strudel-alike). All are large, multi-phase
efforts that don't fit into a weekend. Each section below breaks the
work into ordered phases with explicit deliverables, blockers, and test
strategies so a future session can pick it up cold.

- **A. DAW Bridge** — §2 of this doc
- **B. WASM Target** — §3 of this doc
- **C. Mini-Notation & Cycles DSL** — see [`phase-c-strudel.md`](phase-c-strudel.md)
  (separate doc due to size)

Recommended attack order (revised after Phase C was added):

```text
Sprint 3 (DSP completion)
   ↓
Phase C (mini-notation DSL — biggest UX win, enables Phase B's real use case)
   ↓
Phase B (WASM — pairs with Phase C for browser live-coding)
   ↓
Phase A (DAW bridge — whenever there's demand)
```

---

## A. DAW Bridge (JACK / PipeWire)

**Goal:** Let Nyx be routed into and out of DAWs (Ableton, Bitwig, Reaper,
Ardour, Bitwig, FL Studio) and other audio applications, instead of only
playing to the default system output.

**Why deferred:** Requires new FFI bindings, port lifecycle management, a
unified MIDI/audio input abstraction, and platform-specific code for
non-Linux targets. CI testing is hard because it needs a running
JACK/PipeWire server.

### Phase A0 — Research & Architecture (≤ 1 day)

- [ ] Evaluate Rust JACK bindings: `jack` crate (safe wrapper) vs.
      `pipewire-rs` (PipeWire-native). JACK is cross-platform but lagging;
      PipeWire is Linux-only but modern.
- [ ] Decide: **start with `jack` crate** since PipeWire ships a JACK-compat
      layer on Linux, and JACK works on macOS too. This covers 90% of the
      target ecosystem with one backend.
- [ ] Read `jack` crate docs to understand client lifecycle, port types
      (audio/MIDI), and the process callback contract. It must behave like
      cpal's callback: real-time, no alloc, no locks.
- [ ] Decide on naming: `nyx-core/src/jack_engine.rs` module, feature
      `jack`, struct `JackEngine` parallel to `Engine`. Do **not** replace
      `Engine` — cpal output stays the default.

### Phase A1 — Linux/macOS JACK Audio Output (1–2 days)

- [ ] Add `jack = { version = "0.13", optional = true }` to `nyx-core`.
      Feature: `jack` (mutually compatible with `audio`).
- [ ] `JackEngine::play<S: Signal>(signal, config)` —
  - Open JACK client named `"nyx"` (or configurable)
  - Register two output ports: `"out_l"`, `"out_r"`
  - Activate client; in the process callback, fill both port buffers with
    the signal's output (sum to mono, pan, or accept stereo Signal — TBD)
  - Return a `JackEngine` handle that stops audio on drop
- [ ] Add `JackEngineError` variants for: server not running, port
      registration failed, activation failed
- [ ] Document in manual.md: running `jackd -d alsa` or using PipeWire-JACK,
      prerequisites, how to connect ports with `qjackctl` / `qpwgraph`
- [ ] Example: `nyx-examples/examples/jack_output.rs` playing a sine wave
      through JACK. Instructions to route into Audacity/Ardour/Reaper.

**Deliverable:** `cargo run -p nyx-examples --example jack_output` plays a
tone that shows up as a source in `qpwgraph`, routeable into any DAW.

### Phase A2 — JACK Audio Input / In-the-middle Effects (1 day)

- [ ] Register input ports `"in_l"`, `"in_r"` on the same JackEngine
- [ ] `JackEngine::input_signal() -> JackSignal` returns a Signal that reads
      from the JACK input ports (mirror of MicSignal)
- [ ] Use lock-free `rtrb` between process callback and the user's Signal,
      same pattern as mic.rs
- [ ] Example: noise gate or distortion effect — Nyx reads from a DAW track,
      processes, sends back on its outputs
- [ ] Verify no feedback loops / latency issues. Document JACK's inherent
      one-buffer round-trip latency.

**Deliverable:** `JackSignal` that can be chained with `.lowpass`, `.amp`,
etc. — Nyx as a live insert effect.

### Phase A3 — JACK MIDI I/O (1 day)

- [ ] Register MIDI input port(s) on JackEngine
- [ ] In the process callback, iterate `MidiIn::iter(ps)`, parse events
      with existing `nyx_core::midi::parse_midi`, push into a `MidiReceiver`
      identical to the midir-backed one
- [ ] Register MIDI output port(s) for sending notes from Nyx to DAW synths
- [ ] New trait `MidiBackend` or enum abstracting midir vs JACK so
      existing `CcMap` / voice-pool code works unchanged
- [ ] Example: `nyx-examples/examples/jack_midi.rs` — DAW sends MIDI notes,
      Nyx plays SubSynth voices, audio goes back to DAW.

**Deliverable:** Nyx as a usable DAW synthesizer — MIDI in, audio out,
everything routable with standard DAW patching.

### Phase A4 — PipeWire Native Backend (optional, 2–3 days)

Only pursue if JACK compat layer shows problems (extra latency, missing
features, PipeWire-specific metadata).

- [ ] Evaluate `pipewire-rs` maturity (often in flux)
- [ ] Parallel module `pipewire_engine.rs` with same API surface as JackEngine
- [ ] Feature `pipewire` mutually exclusive with `jack`
- [ ] Benchmark latency & jitter vs JACK compat layer

### Phase A5 — macOS / Windows (multi-day, likely v2.0)

- [ ] **macOS:** JACK runs but most pro users use Core Audio + aggregate
      devices. Investigate building a virtual audio device via AudioKit /
      coreaudio-rs. Significant native code.
- [ ] **Windows:** ASIO is standard. Investigate `asio-rs` or `rust-asio`.
      Licensing constraints (Steinberg SDK). WASAPI loopback is a simpler
      fallback.
- [ ] Document platform matrix clearly in manual.md.

### Testing Strategy

JACK testing can't run in standard CI. Options:
- [ ] Start `jackd -d dummy` in a CI script (no sound card needed)
- [ ] Use `pipewire` service in containers (newer CI runners support this)
- [ ] Mock `jack::Client` via trait abstraction for unit tests
- [ ] Integration tests marked `#[ignore]` by default, run locally
      with `cargo test -- --ignored --test-threads=1`

### API Sketch

```rust
use nyx_core::jack::{JackEngine, JackConfig};
use nyx_prelude::*;

let config = JackConfig {
    client_name: "nyx".into(),
    audio_outs: 2,
    audio_ins: 2,
    midi_in: true,
    midi_out: false,
};
let engine = JackEngine::new(config)?;

// Use as output:
engine.play(osc::sine(440.0).amp(0.3))?;

// Or as input processor:
let mic_like = engine.input_signal();
let processed = mic_like.lowpass(1000.0, 0.707).amp(2.0).soft_clip(1.5);
engine.play(processed)?;

// Keep the engine alive. Audio stops on drop.
```

---

## B. WASM Target

**Goal:** Run Nyx in the browser. Enable live-coding sketches on the web,
embed audio synthesis in web apps, publish a playground site for the
library.

**Why deferred:** WASM has fundamental differences from native that require
re-architecting several subsystems — threading model, I/O, audio callback
semantics. The library was built native-first with real-time guarantees
that need rethinking for the web.

### Phase B0 — Research & Scope Decision (≤ 1 day)

- [ ] Decide target: `wasm32-unknown-unknown` (plain WASM) vs `wasm32-wasi`
      (system-like). For the browser, `wasm32-unknown-unknown` with
      wasm-bindgen is standard.
- [ ] Audit external deps for WASM compat:
  - cpal: has a web backend, uses WebAudio under the hood, but only works
    on main thread unless you set up AudioWorklet
  - rtrb: pure Rust, works
  - spectrum-analyzer: pure Rust, works
  - midir: has a `midir-web` WebMIDI backend, good
  - rosc: works if we swap UDP for WebSocket in the listener
  - cpal, midir — both feature-gated, can be stubbed out on WASM
  - iced: has `iced-wasm` support, but immature; consider skipping GUI on
    WASM v1 and providing raw handles instead
- [ ] Decide scope: **DSP core + WebAudio output + WebMIDI** for v1 WASM.
      Defer iced-on-web, hot-reload (impossible without dynamic linking),
      MIDI/OSC UDP (no UDP in browser), golden tests (no fs).

### Phase B1 — Compile `nyx-core` for WASM (≤ 1 day)

- [ ] Add `wasm32-unknown-unknown` target: `rustup target add wasm32-unknown-unknown`
- [ ] Feature-gate everything that touches `std::fs`, `std::thread`,
      `std::sync::Mutex` under `#[cfg(not(target_arch = "wasm32"))]`:
  - `golden.rs` — disable on WASM
  - `engine.rs` (cpal) — swap for WASM-specific engine
  - `mic.rs` (cpal input) — swap or disable
  - `osc_input.rs` (UDP listener) — disable or WebSocket variant
  - Hot-reload is impossible — `nyx-cli` is native-only
- [ ] `cargo build -p nyx-core --target wasm32-unknown-unknown
      --no-default-features` must succeed
- [ ] Run offline tests (`render_to_buffer` works in WASM) via
      `wasm-pack test --headless --firefox`

**Deliverable:** The DSP engine (oscillators, filters, sequencing) compiles
cleanly for WASM and produces bit-identical output to native.

### Phase B2 — WebAudio Output (1–2 days)

- [ ] Research: cpal's web backend vs a hand-rolled AudioWorklet integration
- [ ] Option A (easier): use cpal with `wasm-bindgen-futures`, pay the main-
      thread overhead. Good for demos, bad for low-latency.
- [ ] Option B (better): build `nyx-core/src/wasm_engine.rs` that generates
      an AudioWorkletProcessor in JS/TS, communicates via `postMessage` +
      `SharedArrayBuffer`. Real-time-safe like native.
- [ ] Recommend **starting with Option A**, upgrade to B once the API
      stabilises.
- [ ] `WasmEngine::play(signal)` returns a `Promise`-like handle. Browsers
      require a user gesture to start audio — surface this clearly in docs.
- [ ] Example: wasm-pack build producing a small JS loader + a simple
      index.html that plays a sine on button click.

### Phase B3 — WebMIDI Support (½ day)

- [ ] Behind `cfg(target_arch = "wasm32")`, swap `midir` for its
      WebMIDI-backed variant (midir 0.10 supports this natively)
- [ ] No API change — existing `open_midi_input()` should work transparently
- [ ] Verify with a USB MIDI device in Chrome (WebMIDI not supported in Firefox)

### Phase B4 — Visualisation Bridge (1–2 days)

- [ ] iced-on-web is still experimental; skip for v1
- [ ] Instead: expose `ScopeHandle` / `SpectrumHandle` through wasm-bindgen
      so JS code can read them and render with `<canvas>` / WebGL / WebGPU
- [ ] Provide a minimal JS/TS helper package that wraps the bindings and
      shows how to draw the waveform/spectrum
- [ ] Example site: embed a Nyx synth + scope in a blog post

### Phase B5 — Packaging & Deployment (1 day)

- [ ] Integrate `wasm-pack` and `wasm-bindgen` properly
- [ ] Publish `@nyx/audio` npm package (JS wrapper)
- [ ] Set up a GitHub Pages / Netlify playground site:
  - Code editor (Monaco/CodeMirror)
  - Compile to WASM server-side or use a pre-baked DSL
  - Live audio output
- [ ] Bundle size budget: aim for < 200 KB gzipped for the core engine

### Phase B6 — Live-Coding in the Browser (stretch, multi-day)

Nyx's hot-reload doesn't work in WASM (no `dlopen`) but we can simulate it:

- [ ] Define a subset DSL that maps to our Signal combinators (JSON-based
      signal graph, or a small interpreted language)
- [ ] User edits DSL in the browser, we rebuild the signal graph in-place
      and crossfade via the existing `HotSwap` engine
- [ ] This is effectively Strudel/Tidal-in-Rust territory — be cautious
      about scope creep

### Testing Strategy

- [ ] `wasm-pack test --headless --chrome` for unit tests
- [ ] `render_to_buffer` comparisons between native and WASM (same bits)
- [ ] Manual audio verification on Chrome, Firefox, Safari, mobile browsers
- [ ] Latency measurement — WebAudio typically adds 20–50 ms; document this

### API Sketch (TypeScript user view)

```typescript
import init, { NyxEngine, sine, saw, lowpass } from '@nyx/audio';

await init();

const engine = new NyxEngine();
await engine.unlock();  // requires user gesture

const lfo = sine(0.5).amp(400).offset(800);
const signal = saw(220).lowpass(lfo, 0.707).amp(0.3);

engine.play(signal);

const scope = signal.scope(4096);
// Render scope into a canvas element...
```

---

## Shared Infrastructure Improvements

These help both roadmaps when they start:

- [ ] **Engine trait abstraction** — extract a `AudioEngine` trait that
      cpal, JACK, and WASM backends all implement. Keeps `Signal` unchanged
      but makes swapping backends trivial.
- [ ] **MIDI backend abstraction** — same idea for midir vs JACK vs WebMIDI
- [ ] **Feature matrix in CI** — test matrix for
      `{native, wasm} × {audio, jack, pipewire, midi, osc}`. Early warning
      for breakage.
- [ ] **Target-specific sub-crates** — if the conditional compilation gets
      too ugly, split: `nyx-native`, `nyx-wasm`, `nyx-jack`, etc., with
      `nyx-core` staying pure DSP.

---

## Order of Attack

If starting fresh:

1. **Engine trait abstraction first** (prep work — 1 day)
2. **JACK audio output** (quickest tangible win — Phase A1)
3. **WASM compile check** (Phase B1 — surfaces portability issues early)
4. **JACK MIDI** (Phase A3 — high value for DAW users)
5. **WASM WebAudio output** (Phase B2 — lets you demo on the web)
6. Everything else as appetite allows

Both roadmaps can proceed in parallel after step 1.
