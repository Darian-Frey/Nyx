# CLAUDE.md — Nyx-Audio Development Handoff

This file is the single source of truth for Claude Code sessions on this project.
Read it in full before writing any code. Do not deviate from the architectural
decisions documented here without explicit instruction from the user.

---

## Project Overview

**Nyx-Audio** is a high-performance audio synthesis and sequencing library for
the Rust ecosystem. The goal is to be the "p5.js of sound" — enabling creative
coders, algorithmic composers, and live performers to sketch with audio using a
fluent, expressive API, without managing buffers, thread safety, or DSP boilerplate.

**Repo:** https://github.com/Darian-Frey/Nyx  
**Language:** Rust (Edition 2024+)  
**Targets:** x86_64, aarch64. WASM is explicitly deferred to v2.0.  
**License:** MIT + Apache-2.0 (dual) — add both license files if not present.

---

## Crate Workspace Structure

The project is a Cargo workspace. This split is non-negotiable — `nyx-core`
must never depend on UI crates.

```
nyx/
├── Cargo.toml          # Workspace root
├── CLAUDE.md           # This file
├── nyx-core/           # Headless signal engine — zero UI dependencies
├── nyx-seq/            # Clock, sequencer, music theory
├── nyx-iced/           # Optional Iced GUI widgets
├── nyx-cli/            # Standalone sketch player / live-diff binary
└── nyx-prelude/        # Re-exports for one-line import experience
```

**If the workspace scaffold does not yet exist, create it first before
anything else.** Use `cargo new --lib` for library crates and `cargo new`
for `nyx-cli`.

---

## The Four Pillars (Never Compromise These)

1. **Immediacy** — `nyx::play(osc::sine(440))` works in a blank project.
2. **Fluency** — APIs read like a signal chain, not a configuration file.
3. **Real-Time Safety** — No allocation, no locking in the audio callback. Ever.
4. **Musicality** — Speak in Notes and Beats, not just Hertz and Milliseconds.

---

## Locked Architectural Decisions

These were resolved before coding started. Do not reopen them.

### The Signal Trait

```rust
pub trait Signal: Send {
    fn next(&mut self, ctx: &AudioContext) -> f32;
}

pub struct AudioContext {
    pub sample_rate: f32,
    pub tick: u64,  // Absolute sample count from stream start
}
```

- Sample rate is passed per-call via `AudioContext`. No global statics.
- `tick` is an absolute sample count. It is required from Phase 1 because
  sample-accurate sequencing depends on it.
- The trait is `Send` but not `Sync`. Signals are owned by the audio thread.

### The Allocation / Dispatch Strategy

- **Static dispatch is the default.** Combinator methods return concrete types.
  `osc::sine(440).lowpass(800)` returns `Lowpass<Sine>` — zero allocation.
- **`.boxed()` is the explicit escape hatch** for type erasure. It allocates,
  but only at construction time (before the stream starts), never in the callback.
- **`Param<S>` is the universal parameter type:**

```rust
pub enum Param<S: Signal> {
    Static(f32),
    Modulated(S),
}
```

Every processor parameter (frequency, cutoff, gain) accepts `Param<S>`.
This is what enables `osc.lowpass(800.0)` and `osc.lowpass(lfo)` to both work.

### The No-Alloc Rule

**Zero allocations (`Box`, `Vec`, etc.) after the audio stream has started.**
This is enforced by a CI guard using a custom allocator that panics on
heap allocation inside the audio callback. Set this up in Phase 1 and
never remove it.

### The Voice / Polyphony Model

Fixed-size voice pool, allocated once before stream start:

```rust
pub struct VoicePool<S: Signal, const N: usize> {
    voices: [Option<S>; N],
}
```

Voice stealing strategy (oldest-first by default) is configured at pool
construction. No allocation on note-on. This model must be compatible with
the `Signal` trait.

---

## Key Dependencies

| Crate | Purpose | Status |
|---|---|---|
| `cpal (0.17)` | Cross-platform audio I/O (WASAPI / CoreAudio / ALSA / PipeWire) | In use (optional `audio` feature) |
| `rtrb (0.3)` | Lock-free SPSC ring buffer for UI→audio thread comms | In use |
| `spectrum-analyzer (1.6+)` | FFT magnitude bins for the visual mirror | In use |
| `midir` | Low-latency cross-platform MIDI input | Phase 9 |
| `rosc` | OSC support | Phase 9 |
| `serde` + `serde_json` / `toml` | Patch serialisation | Phase 8 |
| `iced (0.13+)` | GUI framework (nyx-iced only) | Phase 10 |
| `iced_audio (0.12+)` | Knobs, sliders, XYPad (nyx-iced only) | Phase 10 |

Do not add `iced` or `iced_audio` as dependencies of `nyx-core` or `nyx-seq`.

---

## Development Phases

Work through phases in order. Do not start a phase until the previous one
has passing tests. Current status: **Phase 8 is next. Phases 0–7 complete (178 tests passing).**

### Phase 0 — Architecture Spike (Complete)
- [x] Create Cargo workspace with all five crate stubs
- [x] Define `Signal` trait and `AudioContext` in `nyx-core`
- [x] Define `Param<S>` enum in `nyx-core`
- [x] Define `VoicePool<S, N>` skeleton in `nyx-core`
- [x] Write unit tests proving the trait compiles and `next()` is callable

### Phase 1 — The "Night-Safe" Core (Complete)
- [x] Integrate `cpal` — device init, stream creation, audio callback
- [x] Implement SPSC bridge (`rtrb`) between main thread and audio thread
- [x] Set up custom allocator CI guard (panics on alloc in audio callback)
- [x] Implement offline render mode: `render_to_buffer(signal, secs, sr)`
- [x] Device hot-plug handling and reconnection loop
- [x] Golden-file DSP regression test framework
- [x] Default buffer size targeting < 20ms latency

### Phase 2 — The Fluent API (Complete)
- [x] Implement `Signal` trait with `.boxed()` escape hatch
- [x] Implement `Param<S>` with `IntoParam` trait (`From<f32>` and `From<S: Signal>`)
- [x] Combinator wrappers: `.amp()`, `.mix()`, `.pan()`, `.clip()`, `.add()`, `.mul()`, `.soft_clip()`, `.offset()`
- [x] `nyx_prelude::play(signal)` function — wraps all cpal boilerplate
- [x] `nyx-prelude` re-exports

### Phase 3 — Primitive Palette (Complete)
- [x] Oscillators: `osc::sine`, `osc::saw`, `osc::square`, `osc::triangle`
- [x] Noise: `osc::noise::white(seed)`, `osc::noise::pink(seed)`
- [x] All oscillators: frequency accepts `IntoParam` (f32 or Signal)
- [x] Phase tracked as normalised f32 in [0, 1), incremented by `freq / sample_rate`
- [x] Resonant Low-Pass and High-Pass (Biquad, Transposed Direct Form II)
- [x] Filter coefficient smoothing (one-pole ~5ms) to prevent clicks
- [x] Gain, soft clip (tanh), hard clip, peak limiter

### Phase 4 — Time & The Pulse (Complete)
- [x] BPM-based global clock driven by `AudioContext.tick` (f64 accumulator)
- [x] `clock.tick(ctx)` returns `ClockState` with `.beat`, `.bar`, `.phase_in_beat`, `.phase_in_bar`
- [x] BPM is a `Param<S>` (tempo can be modulated)
- [x] Quantisation: `Clock::snap(beat, grid)`
- [x] Trigger-based ADSR envelopes (attack/decay/sustain/release)
- [x] Time-travel automation: `signal.follow(|t| expr)` and `automation(|t| expr)`

### Phase 5 — Music Theory Module (`nyx-seq`) (Complete)
- [x] `Note` type: `Note::A4`, `Note::from_midi(n)`, `Note::to_freq()`
- [x] `Note::parse("C#4")` string parsing ("C#4", "Bb2", etc.)
- [x] Scale library: Major, Minor, Pentatonic, Dorian, Phrygian, Lydian,
      Mixolydian, Locrian, Whole Tone, Chromatic
- [x] `Scale::minor("C").snap(val)` — snaps f32 to nearest note in scale
- [x] Chord types: Maj, Min, Dim, Aug, Maj7, Min7, Dom7, Sus2, Sus4
- [x] Interval helpers: `.transpose(semitones)`, `.up_octave()`, `.down_octave()`

### Phase 6 — The Visual Mirror (Complete)
- [x] `signal.scope(buffer_size)` → `(Scope, ScopeHandle)` (lock-free rtrb ring buffer)
- [x] `signal.inspect(|sample, ctx| ...)` → closure called per-sample, stays on audio thread
- [x] `signal.spectrum(config)` → `(Spectrum, SpectrumHandle)` (FFT magnitude bins via `spectrum-analyzer`)
- [x] Configurable FFT frame size and window function (Hann, Blackman)
- [ ] Example: Nannou oscilloscope in < 50 lines (deferred — infrastructure ready)
- [ ] Example: Bevy spectrum visualiser as a system (deferred — infrastructure ready)

### Phase 7 — Patterns & Sequencing (Complete)
- [x] Step sequencer: `Sequence::new(pattern, grid)` driven by `ClockState`
- [x] Euclidean rhythms: `Euclid::generate(hits, steps)` with `.rotate(offset)`
- [x] Seeded randomness: `nyx_seq::seeded(42)` (portable xorshift64 PRNG)
- [x] `rng.next_note_in(scale)` — scale-aware random note
- [x] Pattern combinators: `.reverse()`, `.retrograde()`, `.invert()`,
      `.concat()`, `.interleave()`, `.rotate()`

### Phase 8 — Macro-Synth Layer
- [ ] `inst::kick()`, `inst::snare()`, `inst::hihat(open)`, `inst::drone(note)`,
      `inst::riser(duration)`, `inst::pad(chord)`
- [ ] All instruments built from `nyx-core` primitives (they are documentation)
- [ ] `SubSynth` template: oscillator → filter → ADSR → gain
- [ ] `SynthPatch` enum-based IR for serde serialisation
- [ ] `patch.save("name.toml")`, `SubSynth::load("name.toml")`
- [ ] Note: `dyn Signal` is not serialisable. Only preset instruments support
      snapshots. Document this clearly.

### Phase 9 — MIDI & Live Input
- [ ] `midir` MIDI input → CC values mapped to `Param<S>` via `MidiMap`
- [ ] One-pole smoothing on CC values to prevent zipper noise (~5ms default)
- [ ] Note-on/off events → voice pool via SPSC bridge (no alloc in callback)
- [ ] OSC input via `rosc`
- [ ] `input::mic()` returns a `Signal` from the default input device

### Phase 10 — nyx-iced GUI (Optional Crate)
- [ ] Knob, HSlider, VSlider, XYPad via `iced_audio`
- [ ] `OscilloscopeCanvas` consuming a `ScopeHandle`
- [ ] `SpectrumCanvas` consuming a `SpectrumHandle`
- [ ] Nyx Midnight Theme (deep grays, neon accent, monospace)

### Phase 11 — Live-Diff / Hot Reload (Moonshot)
- [ ] DSP logic as `cdylib`, hot-reloaded via `hot-lib-reloader`
- [ ] Graceful handoff: old chain faded to silence before new chain loads
- [ ] `nyx-cli` watches a `.rs` sketch file, recompiles on save
- [ ] Target: < 2s from file save to audible change
- [ ] DAW bridge via JACK/PipeWire (stretch goal)

---

## Real-Time Safety Rules (Enforced — Not Advisory)

These are compiler-level rules, not style preferences:

1. **No `std::sync::Mutex` or `Arc<Mutex<T>>` in the audio callback.** Use
   `std::sync::atomic` for simple flags. Use the SPSC bridge for everything else.
2. **No heap allocation in the audio callback.** No `Box`, `Vec`, `String`,
   or any type whose `Drop` may free memory. The CI guard enforces this.
3. **No I/O in the audio callback.** No file reads, no syscalls, no `println!`.
4. **No locks in the audio callback.** This includes `RwLock`, `Mutex`,
   and `std::sync::Once`.
5. **Coefficient smoothing is mandatory on all filter parameters.** Instantaneous
   parameter jumps cause audible clicks. Use a one-pole smoother.

---

## Testing Standards

- **Unit tests** for all `Signal` implementations using `render_to_buffer`.
- **Golden-file regression tests** for all oscillators and filters. Store
  expected output as binary blobs in `tests/golden/`. Compare on CI.
- **Phase accuracy test:** Sine oscillator phase error must be < 1e-6 after
  10 seconds of continuous playback at 44100 Hz.
- **No-alloc guard** on every PR touching the audio callback path.
- Run `cargo clippy -- -D warnings` and `cargo fmt --check` on all PRs.
- `cargo audit` and `cargo deny` for dependency security.

---

## Cookbook Requirement

Every completed phase must ship with at least two Cookbook examples in
`examples/cookbook/` — self-contained sketches of ≤ 20 lines demonstrating
a real musical result. Examples are first-class deliverables, not afterthoughts.

Target examples:
- `dubstep_wobble.rs` — LFO on filter cutoff, BPM-synced
- `wind.rs` — pink noise + slow random LFO on gain
- `generative_melody.rs` — Euclidean rhythm + scale snap + seeded RNG
- `nannou_scope.rs` — waveform visualiser in Nannou
- `midi_filter.rs` — MIDI CC mapped to filter cutoff

---

## What NOT to Do

- Do not add `iced` or any UI dependency to `nyx-core` or `nyx-seq`.
- Do not use `Box<dyn Signal>` as a parameter type in processor structs.
  Use `Param<S: Signal>` instead.
- Do not use `std::sync::Mutex` anywhere near the audio thread.
- Do not open the WASM target. It is deferred to v2.0.
- Do not skip Phase 0. The Signal trait must be finalised before oscillators
  are written.
- Do not serialise `dyn Signal`. Only concrete `SynthPatch` variants are
  serialisable.
- Do not start Phase 11 until Phase 9 is complete and stable.

---

## Success Metrics

| Metric | Target |
|---|---|
| Hello World time | < 5 min from `cargo new` to first sound |
| Simultaneous oscillators | 100+ without underruns on a modern laptop |
| Default latency | < 20ms on default buffer settings |
| No-alloc guard | Zero violations, enforced per-PR |
| Phase accuracy | Sine error < 1e-6 after 10s at 44100 Hz |

---

## Session Startup Checklist

At the start of every Claude Code session:
1. Read this file in full.
2. Run `cargo build --workspace` and note any errors.
3. Run `cargo test --workspace` and note failures.
4. Identify the current phase from the checklist above.
5. Ask the user which task to tackle before writing code.
