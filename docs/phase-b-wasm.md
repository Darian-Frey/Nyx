# Phase B — WASM Target

Status as of **2026-04-21**: **B0, B1, B2 (Option A), and B3 complete**.
Visualisation bridge (B4) and packaging (B5) are next.

This document captures the scope decisions made in Phase B0 and the
code-level state reached in Phase B1. The phased plan itself lives in
[roadmap-deferred.md](roadmap-deferred.md#phase-b--wasm-target);
this file tracks *what was decided* and *what's already shipped*.

---

## B0 — Scope Decisions

### Target

**`wasm32-unknown-unknown`** with `wasm-bindgen`. This is the standard
browser WASM target. `wasm32-wasi` is a red herring for Nyx's use case
(no filesystem or OS I/O needed inside the browser).

### v1 Scope

The v1 WASM build exposes:

- **DSP core** — oscillators, filters, envelopes, dynamics, granular,
  etc. All bit-identical to native.
- **Offline rendering** — `render_to_buffer` works in WASM, suitable
  for pre-rendering audio for a web page or running unit tests via
  `wasm-pack test`.
- **`nyx-seq`** — the whole sequencing / music-theory crate compiles
  unchanged for WASM.
- **`nyx-prelude`** — the one-line-import prelude compiles unchanged
  for WASM.

### v1 Deferred

- **Native audio I/O (`audio` feature)** — cpal on WASM needs
  `AudioWorkletProcessor` integration. Deferred to Phase B2.
- **WebMIDI (`midi` feature)** — shipped in Phase B3 via a direct
  `web-sys` + `wasm-bindgen-futures` backend in [`midi_web.rs`](../nyx-core/src/midi_web.rs),
  bypassing `midir-0.10.3`'s broken WebMIDI path. Same
  `open_midi_input` / `MidiConnection` API as native — user code is
  portable between `cargo build` and `wasm-pack build` unchanged.
- **OSC over UDP (`osc` feature)** — compiles cleanly but the UDP
  listener cannot run in a browser. Users can provide their own
  WebSocket-based transport; the `OscParam` / `OscSignal` API is
  still useful.
- **WAV file I/O (`wav` feature)** — compiles via `hound` + std's
  WASM fs stubs, but actual reads/writes will fail at runtime. Use
  `Sample::from_buffer` on WASM instead.
- **`nyx-iced`** — iced-on-web is experimental; skipping for v1.
- **`nyx-cli`** — hot-reload via `libloading` is impossible under
  WASM; native-only by design.
- **`nyx-examples`** (nannou / bevy visualisers) — native-only.
- **Golden-file DSP regression tests** — use `std::fs`; run them
  natively where it makes sense to.

---

## B1 — Compile `nyx-core` for WASM

### What works today

```bash
# Core DSP — the headline deliverable.
cargo build -p nyx-core --target wasm32-unknown-unknown --no-default-features

# Optional features that also build cleanly on WASM:
cargo build -p nyx-core --target wasm32-unknown-unknown --no-default-features --features wav
cargo build -p nyx-core --target wasm32-unknown-unknown --no-default-features --features osc

# And the higher-level crates:
cargo build -p nyx-seq     --target wasm32-unknown-unknown --no-default-features
cargo build -p nyx-prelude --target wasm32-unknown-unknown --no-default-features
```

All 444 native tests continue to pass. `cargo clippy --workspace -- -D warnings`
stays clean.

### Code changes required

**One** line of code. `nyx-core/src/sample.rs` imported
`std::path::Path` unconditionally; it's only used inside
`#[cfg(feature = "wav")] fn load()`. Added the matching cfg to the
import. That was the full extent of the source changes.

Everything else — the `Signal` trait, all DSP primitives, the bridge,
the voice pool, `render_to_buffer`, the alloc guard, scope/spectrum
taps, pitch detection, granular synthesis — already uses only
WASM-portable parts of `std` and compiles cleanly.

### Known WASM-incompat code paths (all gated out via features)

| Module | Why it can't run on WASM | Status |
| --- | --- | --- |
| `engine.rs` | cpal needs native audio APIs | `#[cfg(feature = "audio")]` |
| `mic.rs` | cpal input backends are native | `#[cfg(feature = "audio")]` |
| `midi.rs` | platform-gated: midir (native) / web-sys (wasm) | shipped both backends in B3 |
| `osc_input.rs` UDP listener | browsers have no UDP | `#[cfg(feature = "osc")]` + runtime init |
| `wav.rs` | std::fs stubs fail at runtime | `#[cfg(feature = "wav")]` |
| `golden.rs` | std::fs test fixtures | compiles; tests gated `#[cfg(not(target_arch = "wasm32"))]` if we want them in a wasm-pack run (not required for v1) |
| `hotswap.rs` via `nyx-cli` | `libloading` = native dynamic linking | nyx-cli excluded from WASM build |

### `midir` WebMIDI bug — worked around (B3)

`midir-0.10.3`'s WebMIDI backend fails to compile on modern rustc
toolchains:

```text
error[E0282]: type annotations needed
  --> midir-0.10.3/src/backend/webmidi/mod.rs:38:44
38 |             on_ok: Closure::wrap(Box::new(|access| { ... }
```

Upstream bug in `midir`, not ours. Three options were considered:

1. Wait for upstream.
2. Bypass `midir` on WASM and call `navigator.requestMIDIAccess()`
   directly via `wasm-bindgen` / `web-sys`.
3. Patch `midir` locally until upstream ships a fix.

**We took option 2.** WebMIDI's surface is small compared to ALSA /
CoreMIDI, and `nyx-core/src/midi.rs` already has a platform-neutral
`MidiEvent` / `MidiReceiver` / `parse_midi` layer — so the new backend
only needed to wire browser events into the existing bridge. See the
**B3** section below.

---

## B2 — WebAudio Output (Option A)

**Shipped.** Nyx now plays end-to-end inside a browser tab via cpal's
WASM backend.

### Surprise finding

`cpal = "0.17"` already has a WebAudio backend and compiles cleanly for
`wasm32-unknown-unknown` with the existing Nyx source tree. **Zero
changes were required to `engine.rs`** — `Engine::play(signal)` works
in both native and browser targets with the same API.

### The demo crate

A new workspace member, [`nyx-wasm-demo/`](../nyx-wasm-demo/), wraps
`nyx-prelude::Engine` with a minimal [`wasm-bindgen`] layer:

```rust
#[wasm_bindgen]
pub struct NyxDemo { _engine: Engine }

#[wasm_bindgen]
impl NyxDemo {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<NyxDemo, JsError> {
        let pad = osc::saw(...).add(osc::saw(...)).freeverb().wet(0.35);
        let engine = Engine::play(pad)?;
        Ok(NyxDemo { _engine: engine })
    }
}
```

The paired `index.html` loads the wasm-bindgen output via `<script
type="module">` and toggles playback from a button click. Full build
instructions in [the crate README](../nyx-wasm-demo/README.md).

### Workspace integration

- `nyx-wasm-demo` is a workspace member but **not** in
  `default-members`, so `cargo build`, `cargo test`, and `cargo clippy`
  at the root stay fast and native-focused.
- The `lib.rs` is wrapped in `#![cfg(target_arch = "wasm32")]`, so a
  native `cargo build --workspace` compiles an empty rlib and moves on.
- The wasm build runs on CI — see below.

### Known limitations (Option A)

- **Main-thread DSP.** cpal's browser backend uses a
  `ScriptProcessorNode` under the hood. Audio callback runs on the
  main thread, not a real audio thread, so latency is 40–100 ms and a
  busy tab can cause glitches. This is fine for demos but not for
  interactive instruments.
- **Autoplay policy.** `AudioContext` only starts inside a user
  gesture. `NyxDemo::new` must be called from a `click` / `keydown`
  handler. The demo's `index.html` does this correctly.
- **No worker / worklet yet.** Option B (`AudioWorkletProcessor` +
  `SharedArrayBuffer`) is the path to real-time-safe, low-latency
  browser audio. That's the next rewrite target when the demo API
  stabilises.

### CI

Two `wasm-build` steps in `.github/workflows/ci.yml` cover this:

```yaml
- cargo build -p nyx-core     --target wasm32-unknown-unknown --no-default-features
- cargo build -p nyx-core     --target wasm32-unknown-unknown --no-default-features --features wav
- cargo build -p nyx-core     --target wasm32-unknown-unknown --no-default-features --features osc
- cargo build -p nyx-seq      --target wasm32-unknown-unknown --no-default-features
- cargo build -p nyx-prelude  --target wasm32-unknown-unknown --no-default-features
- cargo build -p nyx-wasm-demo --target wasm32-unknown-unknown
```

`wasm-pack` itself is not run in CI — installing it adds ~60 s per run
and the raw `cargo build` catches every compile-level regression we
care about. Running the demo in a real browser is still a manual step
during development.

---

## B3 — WebMIDI

**Shipped.** `--features midi` now compiles on both native and WASM.
Same API, different backend:

```rust
// Works identically on native *and* wasm32 targets:
let (mut receiver, _connection) = nyx_core::open_midi_input()?;
for event in receiver.drain() {
    match event {
        MidiEvent::NoteOn { note, velocity, .. } => { /* ... */ }
        MidiEvent::ControlChange { cc, value, .. } => { /* ... */ }
        _ => {}
    }
}
```

### Architecture

`nyx-core/src/midi.rs` now has three sections:

1. **Shared** — `MidiEvent`, `parse_midi(bytes)`, `MidiSender`,
   `MidiReceiver`, `midi_bridge()`, the `CcMap` / `CcSignal` / `CcWriter`
   smoothing primitives, and the `MidiError` enum. Target-agnostic.
2. **Native backend** in [`midi_native.rs`](../nyx-core/src/midi_native.rs)
   — a thin wrapper around `midir`. ALSA / CoreMIDI / WinMM as before.
3. **WebMIDI backend** in [`midi_web.rs`](../nyx-core/src/midi_web.rs) —
   talks to `navigator.requestMIDIAccess()` via `web-sys` /
   `wasm-bindgen-futures`.

Feature-gating in [`Cargo.toml`](../nyx-core/Cargo.toml) picks the
right backend per-target:

```toml
[features]
midi = [
    "dep:midir", "dep:wasm-bindgen", "dep:wasm-bindgen-futures",
    "dep:web-sys", "dep:js-sys",
]

[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
midir = { version = "0.10", optional = true }

[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen          = { version = "0.2", optional = true }
wasm-bindgen-futures  = { version = "0.4", optional = true }
js-sys                = { version = "0.3", optional = true }
web-sys               = { version = "0.3", optional = true, features = [
    "Window", "Navigator", "MidiAccess", "MidiInput", "MidiInputMap",
    "MidiMessageEvent", "MidiOptions", "console",
] }
```

Cargo quietly skips optional-dep activations for crates that aren't in
the current `[target]` block, so the single `midi` feature Just Works on
both platforms — no `midi-native` / `midi-web` split needed in the user
interface.

### Async caveat

`requestMIDIAccess()` returns a Promise, so the browser's permission
grant is inherently asynchronous. `open_midi_input()` on WASM returns
the `(MidiReceiver, MidiConnection)` pair **immediately**; events start
arriving once the user resolves the permission prompt. If they deny it
(or the page isn't served over HTTPS / `localhost`), an error is logged
to the browser console and the receiver stays silent — no panic, no
exception thrown to JS.

### Lifecycle safety

`MidiConnection`'s `Drop` impl explicitly clears every input's
`onmidimessage` before the underlying `Closure`s deallocate, so stale
references in the browser can never call into dropped memory. WASM is
single-threaded, so the setup task and the drop never race.

### Tested

| Surface | Before | After |
| --- | --- | --- |
| native `cargo build -p nyx-core --features midi` | ✓ | ✓ |
| native `cargo test --workspace` (444 tests) | ✓ | ✓ |
| wasm32 `cargo build -p nyx-core --features midi` | ❌ midir bug | ✓ |
| wasm32 `cargo build -p nyx-wasm-demo` | ✓ | ✓ |
| `cargo clippy --workspace -- -D warnings` | ✓ | ✓ |

CI now covers the wasm32 `--features midi` build in the
`wasm-build` job.

---

## Next Up — B4 / B5

- **B4 (Visualisation bridge).** Expose `ScopeHandle` and
  `SpectrumHandle` through `#[wasm_bindgen]` so the demo page can
  draw a live waveform or spectrum in `<canvas>`.
- **B5 (Packaging).** Set up a GitHub Pages deployment of
  `nyx-wasm-demo` + `wasm-pack build --release`, and measure the
  gzipped `.wasm` size against the 200 KB budget.

[`wasm-bindgen`]: https://crates.io/crates/wasm-bindgen
