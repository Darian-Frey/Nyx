# Nyx

**High-performance audio synthesis and sequencing for Rust.**

Nyx is the "p5.js of sound" — a library for creative coders, algorithmic composers, and live performers to sketch with audio using a fluent, expressive API, without managing buffers, thread safety, or DSP boilerplate.

```rust
use nyx_prelude::*;

// A sine wave. That's it.
nyx::play(osc::sine(440));
```

## Goals

- **Immediacy** — `nyx::play(osc::sine(440))` works in a blank project
- **Fluency** — APIs read like a signal chain, not a configuration file
- **Real-Time Safety** — No allocation, no locking in the audio callback. Ever.
- **Musicality** — Speak in Notes and Beats, not just Hertz and Milliseconds

## Workspace

| Crate | Description |
|---|---|
| `nyx-core` | Headless signal engine — zero UI dependencies |
| `nyx-seq` | Clock, sequencer, music theory |
| `nyx-iced` | Optional Iced GUI widgets |
| `nyx-cli` | Standalone sketch player / live-diff binary |
| `nyx-prelude` | Re-exports for one-line imports |

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

## Status

Nyx is in early development. See [CLAUDE.md](CLAUDE.md) for the full development roadmap.

| Phase | Description | Status |
|---|---|---|
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

## Requirements

- Rust Edition 2024+
- Targets: x86_64, aarch64 (WASM deferred to v2.0)

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
