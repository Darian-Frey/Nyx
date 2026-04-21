# Phase B — WASM Target

Status as of **2026-04-21**: **B0 through B5 complete**. Phase B v1
is done — the browser target is deployable. The B6 live-coding DSL is
out of scope for Nyx per the "separate project" decision in
[CLAUDE.md](../CLAUDE.md).

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

## B4 — Visualisation Bridge

**Shipped.** `ScopeHandle` and `SpectrumHandle` are now reachable from
JavaScript through `NyxDemo` methods, so the demo page draws a live
oscilloscope and spectrum alongside the audio.

### API added to `NyxDemo`

```rust
pub fn read_scope(&mut self, out: &mut [f32]) -> usize;
pub fn scope_available(&self) -> usize;
pub fn spectrum_bin_count(&self) -> usize;
pub fn read_spectrum(&self, out: &mut [f32]) -> usize;
```

wasm-bindgen marshals `&mut [f32]` into / out of a JS `Float32Array`
automatically, so the JS side owns pre-allocated scratch buffers and
the `requestAnimationFrame` loop never touches the garbage collector:

```js
const scopeBuf = new Float32Array(2048);
const specBuf  = new Float32Array(2 * demo.spectrum_bin_count());
function draw() {
    requestAnimationFrame(draw);
    const n = demo.read_scope(scopeBuf);    // drained into buf
    const m = demo.read_spectrum(specBuf);  // filled as (freq, mag) pairs
    // ... canvas paint ...
}
```

### Design: wrappers live in `nyx-wasm-demo`, not `nyx-core`

Keeping `wasm-bindgen` out of `nyx-core` means the core crate stays
small and doesn't force browser-specific types on desktop / DAW / CLI
consumers. Each WASM front-end (demo today, a production app
tomorrow) wraps the underlying `ScopeHandle` / `SpectrumHandle` in
whatever JS shape fits its use case — bin counts, unit scales, typed-
array layouts. The current `NyxDemo` is one opinionated shape.

### The `index.html` page

- Two `<canvas>` elements above the play button — waveform (drawn as
  a polyline) and spectrum (log-magnitude bars capped at 20 kHz).
- HiDPI-correct: `devicePixelRatio` is sampled on load and on
  `resize`, and the `<canvas>` internal dimensions scale accordingly
  so display is crisp on retina.
- A single `requestAnimationFrame` loop runs continuously from page
  load; it's a no-op while `demo === null` so there's no wasted work
  in the stopped state.
- Stopping (`demo.free()`) resets `specBuf` to `null` so the next
  playback re-sizes the scratch buffer from the new bin count — the
  FFT frame size can in principle change between sessions.

### Scope buffer sizing

`NyxDemo` uses a 4096-sample scope ring (~93 ms at 44.1 kHz). At
60 fps that's > 5× the headroom needed per rAF tick, so samples
never fill the ring and get dropped during normal operation. If the
tab is backgrounded and the rAF loop stalls, the oldest samples
silently fall off — the scope display just "skips ahead" on return,
which is the right behaviour.

### B4 verification

- Native `cargo build --workspace` and `cargo test --workspace`
  (444 tests) unaffected.
- `cargo build -p nyx-wasm-demo --target wasm32-unknown-unknown`
  succeeds; CI job already covers this.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.

---

## B5 — Packaging & Deployment

**Shipped.** The demo is deployed to GitHub Pages and gated by a size
budget in CI.

### GitHub Actions workflow

[`deploy-demo.yml`](../.github/workflows/deploy-demo.yml) is a single
workflow with two jobs:

1. **build + size budget** (runs on every push to `main` and every
   PR): installs Rust 1.95 + the wasm32 target, installs `wasm-pack`
   via the official installer, runs `wasm-pack build --release
   --target web --no-typescript` inside `nyx-wasm-demo/`, measures
   the raw and gzipped `.wasm` size, writes the numbers to the
   workflow run summary, and **fails** if gzipped size exceeds
   `WASM_SIZE_BUDGET_BYTES` (currently 204800 = 200 KB). On PRs the
   job stops after this step — the size check runs as a gate but no
   deployment happens on untrusted branches.
2. **deploy to GitHub Pages** (push-to-main only): takes the staged
   `_site/` artifact from the build job and publishes it via
   `actions/deploy-pages@v4`.

### Deploy layout

```text
_site/
├── index.html              # copied from nyx-wasm-demo/
└── pkg/
    ├── nyx_wasm_demo_bg.wasm   # wasm-pack release output
    ├── nyx_wasm_demo.js        # wasm-bindgen loader shim
    ├── package.json
    └── README.md
```

The relative `./pkg/nyx_wasm_demo.js` import in the HTML resolves
correctly against the Pages root.

### Size measurement

Local release build (2026-04-21):

| metric | value |
| --- | --- |
| raw `.wasm` | 37 KB |
| gzipped `.wasm` | **17 KB** |
| budget (gzipped) | 200 KB |
| headroom | ≈ 12× |

The tiny size comes from three things working together:

1. `--release` profile with LTO.
2. `wasm-opt -O` (run automatically by `wasm-pack`).
3. Aggressive dead-code elimination — the demo only pulls in `osc::saw`,
   `.lowpass()`, `.freeverb()`, `.scope()`, `.spectrum()`, and `Engine::play`.
   Every other DSP module (compressor, flanger, chorus, granular,
   wavetable, FM, bus, pitch…) is dropped because the demo doesn't
   reference it, even though it all lives in `nyx-core`.

The 12× headroom means: the budget will bite only if we either add the
whole `nyx-core` surface area to the demo (unlikely — the demo is
deliberately small) or if a new dependency arrives with large static
tables. That's the right failure mode for this gate.

### What's intentionally *not* in B5

The original Phase B roadmap listed two sub-items that we cut:

- **"Publish `@nyx/audio` npm package."** The `pkg/` directory is
  already npm-shaped, so users who want an npm entry can add their
  own `"publishConfig"` stanza and push it. Making Nyx a real
  first-party npm publisher adds version-management overhead (registry
  tokens, SemVer discipline across two ecosystems) for little
  payoff. Defer until a concrete downstream user asks.
- **"Code editor + DSL + live audio output"** (the Strudel-style
  playground). Explicitly out of scope for Nyx per the
  [CLAUDE.md](../CLAUDE.md) decision to pursue DSL-style live-coding
  as a separate project.

### CI matrix (Phase B total)

With B5 in place, the full set of WASM-related CI is:

| Job | File | What it proves |
| --- | --- | --- |
| `wasm-build` | [`ci.yml`](../.github/workflows/ci.yml) | Every feature config compiles for `wasm32-unknown-unknown` on every PR |
| `build + size budget` | [`deploy-demo.yml`](../.github/workflows/deploy-demo.yml) | Release-mode `wasm-pack` build succeeds and stays ≤ 200 KB gzipped |
| `deploy to GitHub Pages` | same file | Push to `main` → live demo at [darian-frey.github.io/Nyx](https://darian-frey.github.io/Nyx/) |

---

## Phase B — Complete

All six milestones from the original plan have landed:

| ID | What | Status |
| --- | --- | --- |
| B0 | Research & scope decisions | ✅ |
| B1 | `nyx-core` compiles for `wasm32-unknown-unknown` | ✅ |
| B2 | WebAudio output (cpal Option A) | ✅ |
| B3 | WebMIDI via direct `web-sys` bindings | ✅ |
| B4 | Visualisation bridge (scope + spectrum to JS) | ✅ |
| B5 | Packaging, GitHub Pages deploy, size budget | ✅ |
| B6 | Live-coding DSL | **out of scope** (separate project) |

The browser target is now as first-class as the native one for every
piece of Nyx except the three that are fundamentally native: Jack /
CoreAudio direct access, `libloading`-based hot reload, and
`std::net::UdpSocket` OSC input. Everything else — all 15 Sprint 1–3
DSP features, both analysis taps, pitch detection, `nyx-seq`,
`nyx-prelude` — works in a browser tab, and there's a URL that
proves it.

[`wasm-bindgen`]: https://crates.io/crates/wasm-bindgen
