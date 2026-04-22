# Nyx

**High-performance audio synthesis and sequencing for Rust.**

Nyx is the "p5.js of sound" — a library for creative coders, algorithmic composers, and live performers to sketch with audio using a fluent, expressive API, without managing buffers, thread safety, or DSP boilerplate.

```rust
use nyx_prelude::*;

// A sine wave. That's it.
nyx::play(osc::sine(440));
```

## Try it in a browser

A live demo rendered by Nyx compiled to WASM, with real-time oscilloscope and spectrum views, is deployed to GitHub Pages:

**→ [darian-frey.github.io/Nyx](https://darian-frey.github.io/Nyx/)**

Three tabs:

- **A-minor pad** — a gentle detuned saw through a low-pass and Freeverb
- **Tron cue (90 s)** — a full electro-orchestral cue built from [`nyx_prelude::demos::tron`](nyx-prelude/src/demos.rs)
- **Presets** — pick a voice from 9 synth recipes (`tb303`, `supersaw`, `handpan`, `juno_pad`, and more), then play a chromatic keyboard

Source and build instructions: [`nyx-wasm-demo/`](nyx-wasm-demo/). Architecture and scope decisions for the WASM port: [`docs/phase-b-wasm.md`](docs/phase-b-wasm.md).

## Goals

- **Immediacy** — `nyx::play(osc::sine(440))` works in a blank project
- **Fluency** — APIs read like a signal chain, not a configuration file
- **Real-Time Safety** — No allocation, no locking in the audio callback. Ever.
- **Musicality** — Speak in Notes and Beats, not just Hertz and Milliseconds

## Workspace

| Crate | Description |
| --- | --- |
| `nyx-core` | Headless signal engine — zero UI dependencies |
| `nyx-seq` | Clock, sequencer, music theory, instruments, preset voices |
| `nyx-iced` | Optional Iced GUI widgets |
| `nyx-cli` | Standalone sketch player / live-diff binary |
| `nyx-prelude` | Re-exports for one-line imports, plus reusable demo tracks |
| `nyx-examples` | Nannou & Bevy visualisers (excluded from default workspace build) |
| `nyx-wasm-demo` | Browser demo crate — built with `wasm-pack`, deployed to GitHub Pages |

## Quick Start

```bash
cargo add nyx-prelude
```

```rust
use nyx_prelude::*;

fn main() {
    // LFO-modulated filter on a sawtooth
    let lfo = osc::sine(0.5).amp(400.0).offset(800.0);
    let sound = osc::saw(220.0).lowpass(lfo, 0.707);
    play(sound).unwrap();
}
```

## The Signal Trait

Everything in Nyx is a `Signal` — a stream of mono audio samples:

```rust
pub trait Signal: Send {
    fn next(&mut self, ctx: &AudioContext) -> f32;
}
```

Signals compose through combinators that return concrete types (zero allocation by default). Use `.boxed()` when you need type erasure.

## Real-Time Safety

Nyx enforces zero allocations after the audio stream starts:

- No `Box`, `Vec`, `String`, or heap-freeing `Drop` in the audio callback
- No `Mutex`, `RwLock`, or any locking primitive on the audio thread
- No I/O or syscalls in the hot path
- Parameter smoothing on all filter coefficients to prevent clicks

These rules are enforced by a CI allocator guard, not just convention.

## Documentation

Full API reference and usage guide: **[docs/manual.md](docs/manual.md)**

## Status

All 11 numbered phases are complete; active work is feature expansion and sonic polish. See [CLAUDE.md](CLAUDE.md) for the full status board.

| Phase | Description | Status |
| --- | --- | --- |
| 0 | Architecture spike — Signal trait, Param, VoicePool | Done |
| 1 | Night-safe core — cpal, SPSC bridge, no-alloc guard | Done |
| 2 | Fluent API — combinators, `nyx::play` | Done |
| 3 | Primitive palette — oscillators, filters, noise | Done |
| 4 | Time & the pulse — clock, envelopes, automation | Done |
| 5 | Music theory — notes, scales, chords | Done |
| 6 | Visual mirror — scope, spectrum, FFT | Done |
| 7 | Patterns & sequencing — step seq, Euclidean rhythms | Done |
| 8 | Macro-synth layer — instruments, patches | Done |
| 9 | MIDI & live input | Done |
| 10 | Iced GUI widgets | Done |
| 11 | Live-diff / hot reload | Done |

**Since Phase 11:**

- **Sonic character pack** — PolyBLEP oscillators (`saw_bl`, `square_bl`, `pwm_bl`), tape / tube / diode saturation, Moog-style non-linear ladder filter, tape emulator with wow + flutter + `age` knob, analog drift, Paul Kellett pink noise, lofi preset wrappers (`.cassette()` / `.lofi_hiphop()` / `.vhs()`), vinyl crackle + hiss. See [docs/roadmap-sonic-character.md](docs/roadmap-sonic-character.md).
- **Preset voice library** (`nyx_seq::presets`) — 9 named recipes: `tb303`, `moog_bass`, `supersaw`, `prophet_pad`, `dx7_bell`, `juno_pad`, `handpan`, `chime`, `noise_sweep`.
- **Reusable demo tracks** (`nyx_prelude::demos`) — 90-second electro-orchestral cues shared between the offline WAV renderer and the interactive browser demo.
- **Interactive WASM demo** — preset keyboard on the live page, deployed to GitHub Pages via `.github/workflows/deploy-demo.yml`.

## Requirements

- Rust Edition 2024+
- Native targets: x86_64, aarch64
- Browser target: `wasm32-unknown-unknown` via `wasm-pack` (`nyx-wasm-demo`)

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
