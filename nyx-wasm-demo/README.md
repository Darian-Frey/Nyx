# nyx-wasm-demo

The Phase B **"hello sine in a browser"** demo — proves Nyx's DSP
engine runs end-to-end inside WebAudio via cpal's WASM backend.

## Prerequisites

```bash
# One-time setup:
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
```

## Build

From the repo root:

```bash
cd nyx-wasm-demo
wasm-pack build --target web --no-typescript
```

This produces a `pkg/` directory next to `index.html` containing:

- `nyx_wasm_demo_bg.wasm` — the compiled Rust / DSP engine.
- `nyx_wasm_demo.js` — wasm-bindgen's loader shim.

## Run locally

Browsers refuse to `fetch()` a `.wasm` file over `file://`, so serve the
folder over HTTP. Any static file server works:

```bash
# Python 3 — easiest if you have it.
python3 -m http.server 8080

# Or via `basic-http-server` (cargo install basic-http-server):
basic-http-server .
```

Then open <http://localhost:8080/index.html> and click **play**.

## What you hear (and see)

An A-minor triad (A3 + C4 + E4) rendered from three detuned saw
oscillators, low-passed at 1.4 kHz, fed through Nyx's Freeverb. All of
it generated sample-by-sample inside the browser's audio thread via
Nyx's `Signal` trait.

The page also shows a live **oscilloscope** and **spectrum** above the
play button. Both pull from lock-free ring buffers in the DSP engine
via `NyxDemo::read_scope()` / `NyxDemo::read_spectrum()`, using
pre-allocated `Float32Array`s so the `requestAnimationFrame` loop
never hits the JS garbage collector.

## Notes

- **Autoplay policy.** The `AudioContext` can only start from inside a
  user gesture (click, keypress). `NyxDemo::new()` is called from the
  button handler for that reason.
- **Latency.** cpal's WASM backend uses a `ScriptProcessorNode` under
  the hood which adds 40–100 ms of latency and runs on the main thread.
  This is fine for demos and non-interactive playback. The production
  path (Phase B2 Option B) is a hand-rolled `AudioWorkletProcessor`,
  which puts the DSP on the audio thread and drops latency to ≤ 20 ms.
- **Build size.** A release wasm build of this demo is ≈ 200 KB gzipped
  with all of `nyx-core` + `nyx-seq` + reverb in it — within the B5
  target of 200 KB for the core engine.
- **Features.** This demo uses `nyx-prelude --features audio`. WAV I/O
  (`wav`) and OSC (`osc`) both compile on WASM but would fail at runtime
  because the browser has no filesystem or UDP. MIDI via `midir` has an
  upstream bug — see [docs/phase-b-wasm.md](../docs/phase-b-wasm.md).

## CI

`.github/workflows/ci.yml` builds this crate for `wasm32-unknown-unknown`
on every push. It does not run `wasm-pack` (that tool is slower to
install and the cargo build alone is enough to catch regressions).
