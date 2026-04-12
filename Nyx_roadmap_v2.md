# 🌑 Nyx-Audio: Master Development Roadmap (v2.0)

> **Mission:** The "p5.js of sound" — enabling artists, creative coders, and musicians to sketch with audio using a fluent, expressive API, without managing low-level buffer handling, thread safety, or DSP boilerplate.

---

## Guiding Philosophy

| Pillar | Definition |
|---|---|
| **Immediacy** | `nyx::play(osc::sine(440))` must work in a blank project |
| **Fluency** | APIs read like a signal chain, not a configuration file |
| **Real-Time Safety** | Strict "No Allocation / No Lock" in the audio thread — enforced by the compiler, not convention |
| **Musicality** | Speak in Notes and Beats, not just Hertz and Milliseconds |

---

## Crate Architecture (Pre-Phase Decision)

Before a single line of DSP is written, the project must be structured as a Cargo workspace. Bundling everything into a single crate would force GUI dependencies (Iced) onto users who only want headless synthesis — a non-starter for Nannou/Bevy integration.

```
nyx/
├── nyx-core/        # Headless signal engine — zero UI dependencies
├── nyx-seq/         # Clock, sequencer, music theory
├── nyx-iced/        # Optional Iced GUI widgets (knobs, scopes, XYPad)
├── nyx-cli/         # Standalone sketch player / live-diff binary
└── nyx-prelude/     # Re-exports for the one-line import experience
```

**Rule:** `nyx-core` must compile with no UI dependencies. A user embedding Nyx in Bevy must never be forced to pull in Iced.

---

## Licensing

Dual-license under **MIT + Apache-2.0** — the Rust ecosystem standard. This must be decided and committed before any public release. JUCE's GPL encumbrance is a cautionary tale; Nyx should be unambiguously free for commercial use.

---

## Phase 0 — Signal Trait Design (Architecture Spike)

**Goal:** Resolve the two foundational design questions before building anything else. Wrong answers here propagate into every subsequent phase.

### 0.1 — The `Signal` Trait

Define the core trait in full before any oscillators or processors are written.

```rust
pub trait Signal: Send {
    fn next(&mut self, ctx: &AudioContext) -> f32;
}

pub struct AudioContext {
    pub sample_rate: f32,
    pub tick: u64,   // Absolute sample count — enables phase-accurate sequencing
}
```

**Key decisions to nail here:**
- Sample rate is passed per-call via `AudioContext`, not baked into a global static. This keeps the trait flexible for offline rendering and testing.
- `tick` (absolute sample count) must be present from the start — sample-accurate sequencing in Phase 4 requires it, and retrofitting it later is painful.
- The trait is `Send` but not `Sync` — signals are owned by the audio thread.

### 0.2 — The Allocation / Dispatch Strategy

The PRD demands both "zero allocations on the audio thread" and "every parameter accepts either an `f32` or a `Signal`." These conflict if `Box<dyn Signal>` is the parameter type. The resolution:

**Use a `Param<S>` combinator with two strategies:**

- **Static dispatch (default):** Fluent API returns concrete combinator types. E.g., `osc::sine(440).lowpass(800)` returns `Lowpass<Sine>`. No allocation. Compile times increase with chain depth but are predictable.
- **Boxed escape hatch (explicit):** Users can call `.boxed()` to erase the type when chains become unwieldy. This allocation happens at construction time (before the stream starts), not in the callback.

```rust
pub enum Param<S: Signal> {
    Static(f32),
    Modulated(S),
}
```

**Golden rule:** Any `Box` used for signal construction must be allocated before `nyx::play()` is called. The no-alloc CI guard (Phase 1) enforces this.

### 0.3 — Voice / Polyphony Model

`inst::kick()` triggered on a step sequencer implies multiple concurrent instances of the same signal. The only real-time-safe approach is a fixed-size voice pool allocated at stream start:

```rust
pub struct VoicePool<S: Signal, const N: usize> {
    voices: [Option<S>; N],
}
```

- Voice stealing strategy (oldest-first, quietest-first) must be configured at pool construction.
- The pool is allocated once, before the stream starts. No allocations during note-on events.
- This model must be designed in Phase 0 even if `VoicePool` itself ships in Phase 8, because the `Signal` trait must accommodate it cleanly.

---

## Phase 1 — The "Night-Safe" Core (Infrastructure)

**Goal:** A running audio stream that is provably glitch-free, with infrastructure to enforce real-time safety permanently.

### 1.1 — Cross-Platform I/O
- Integrate `cpal (0.17+)` for device initialisation.
- Handle WASAPI (Windows), CoreAudio (macOS), ALSA/PipeWire (Linux).
- **Default buffer size targeting < 20ms latency** for "playable" feel.
- Target architectures: `x86_64`, `aarch64` (Apple Silicon, Raspberry Pi). WASM is explicitly deferred — see Appendix A.

### 1.2 — The SPSC Bridge
- Implement lock-free Single-Producer Single-Consumer ring buffers using `rtrb` or `ringbuf`.
- **No `std::sync::Mutex` or `Arc<Mutex<T>>` in the audio callback — ever.**
- Simple thread-safe toggles (mute, start/stop) use `std::sync::atomic` types only.

### 1.3 — No-Alloc CI Guard
- Add a CI step using `#[global_allocator]` instrumentation or a custom allocator that panics on allocation inside the audio callback.
- Every PR touching the signal chain must pass this guard.
- This is non-negotiable infrastructure — adding it retroactively is far harder.

### 1.4 — Device Hot-Plug & Error Recovery
- Handle audio device disconnection gracefully (e.g., headphones unplugged mid-performance).
- Implement a reconnection loop with configurable fallback to the system default device.
- Expose an `on_device_error` callback so integrators can respond in their own UI.

### 1.5 — Offline / Test Rendering Mode
- Add a `render_to_buffer(signal, duration_secs, sample_rate)` function that runs the signal chain offline.
- This is critical for unit testing (golden file tests) and for the future WAV export feature.
- No real audio device required — enables CI testing of DSP correctness.

### 1.6 — DSP Quality Testing Framework
- Establish golden-file regression tests for all oscillators and filters.
- Tests verify: phase accuracy (no drift over 10s of samples), filter coefficient stability, amplitude normalisation.
- Add to CI alongside the no-alloc guard. A sine wave that drifts in phase is a silent bug without this.

---

## Phase 2 — The Signal DNA (The Fluent API)

**Goal:** Build the grammar of Nyx — the trait system and combinator layer that makes everything chainable.

### 2.1 — Core Signal Trait Implementation
- Implement the `Signal` trait and `AudioContext` as specified in Phase 0.
- Provide the `.boxed()` escape hatch for type erasure when needed.

### 2.2 — The `Param<S>` Type
- Implement `Param<S: Signal>` supporting both `f32` constants and modulated signals.
- Every processor parameter (frequency, cutoff, gain, feedback) must accept `Param<S>`.

### 2.3 — Combinator Wrappers
- `.amp(gain)` — scales amplitude
- `.mix(other, blend)` — crossfades two signals
- `.pan(position)` — stereo panning
- `.clip(threshold)` — hard clipping
- Basic arithmetic: `.add(other)`, `.mul(other)`

### 2.4 — The Prelude
- `use nyx_audio::prelude::*` must be the only import a user needs.
- Bundle all top-level constructors, traits, and type aliases.

### 2.5 — The `nyx::play()` Macro
- Handle all cpal boilerplate: device enumeration, stream creation, buffer callbacks.
- Accept any `impl Signal` — the user never touches `cpal` directly.

---

## Phase 3 — The Primitive Palette (Synthesis Basics)

**Goal:** Give users the fundamental building blocks to make actual sound.

### 3.1 — Oscillators
- `osc::sine(freq)`, `osc::saw(freq)`, `osc::square(freq)`, `osc::triangle(freq)`
- `osc::noise::white()`, `osc::noise::pink()` (pink via Paul Kellett filter approximation)
- All oscillators accept `Param<S>` for frequency — LFO modulation works from day one.
- Phase-accurate: phase tracked as normalised `f32` in `[0, 1)`, incremented by `freq / sample_rate` per sample.

### 3.2 — Filters
- Resonant Low-Pass and High-Pass (Transposed Direct Form II Biquad).
- Cutoff and resonance accept `Param<S>`.
- Coefficient smoothing (one-pole) to prevent clicks when parameters change.

### 3.3 — Gain & Dynamics
- Gain stage, soft clipping (tanh), hard clipping.
- Basic peak limiter to protect output.

---

## Phase 4 — Time & The Pulse (Clock System)

**Goal:** Move away from "seconds" and towards musical time. Sample-accurate, not scheduler-accurate.

### 4.1 — The Global Clock
- A BPM-based clock driven by the `tick` field in `AudioContext`.
- Exposes: `clock.beat()`, `clock.bar()`, `clock.phase_in_beat()` — all as `f32` for smooth LFO sync.
- BPM is a `Param<S>` — tempo can itself be modulated.

### 4.2 — Quantisation
- `clock.snap(tick, grid)` — snaps an event tick to the nearest `1/4`, `1/8`, `1/16` note boundary.
- Quantisation amount is configurable (0.0 = free, 1.0 = locked to grid).

### 4.3 — ADSR Envelopes
- Trigger-based: `Adsr::new(attack, decay, sustain, release).trigger()`
- Accepts a `Trigger` signal (from the clock or a MIDI note-on) rather than polling.
- All four stages accept `Param<S>` for dynamic envelope shapes.

### 4.4 — Time-Travel Automation
- `signal.follow(|t| expr)` — define a parameter's value as a function of musical time `t`.
- This is non-linear, generative automation: `filter.cutoff.follow(|t| (t.sin().abs()) * 1000.0 + 200.0)`.
- Distinct from LFOs (which are periodic) — `follow` can encode arbitrary curves.

---

## Phase 5 — The Semantic Theory Module (Musicality)

**Goal:** Make Nyx speak the language of musicians, not just mathematicians.

### 5.1 — Note Conversion
- `Note::A4` → `440.0 Hz` (bi-directional)
- `MidiNote(69)` → `Note::A4` → `440.0 Hz`
- `NoteName("C#4")` parsing with octave handling.
- Helper: `Note::from_midi(n)`, `Note::to_freq()`.

### 5.2 — Scales & Chords
- Scale library: Major, Minor, Pentatonic, Dorian, Phrygian, Lydian, Mixolydian, Locrian, Whole Tone, Chromatic.
- `Scale::Minor("C").snap(val)` — snaps an arbitrary `f32` (0.0–1.0) to the nearest note in scale.
- `Scale::Major("G").notes()` — returns an iterator of `Note` values.
- Chord types: Major, Minor, Diminished, Augmented, Maj7, Min7, Dom7, Sus2, Sus4.
- `Chord::Maj7("Eb").voicing(Voicing::Spread)` — returns a `Vec<Note>`.

### 5.3 — Interval & Transposition Helpers
- `note.transpose(semitones)`, `note.up_octave()`, `note.down_octave()`.
- Interval constants: `Interval::PerfectFifth`, `Interval::MinorThird`, etc.

---

## Phase 6 — The Visual Mirror (Analysis)

**Goal:** Bridge sound and sight for the core creative coding audience. This is a primary differentiator and should reach users' hands early.

### 6.1 — Inspection Buffers
- `signal.scope()` — returns a `ScopeHandle` containing a lock-free shared ring buffer of recent samples.
- `signal.inspect()` — lower-level: calls a closure with each sample without leaving the audio thread. Safe because closures are `Send`.
- Zero setup: the buffer is allocated at `scope()` call time (before stream start), not in the callback.

### 6.2 — FFT / Spectrum Analysis
- `signal.spectrum()` — returns a `SpectrumHandle` yielding FFT magnitude bins.
- Uses `spectrum-analyzer` crate internally.
- Frame size and window function (Hann, Blackman) are configurable.

### 6.3 — Nannou / Bevy Integration Examples
- Provide first-class example crates showing:
  - `nyx-core` + Nannou oscilloscope in < 50 lines.
  - `nyx-core` + Bevy spectrum visualiser as a system.
- These live in `examples/` and are part of the official "Cookbook."

---

## Phase 7 — Patterns & Sequencing (Generative Core)

**Goal:** The heart of creative coding — making the machine jam.

### 7.1 — Step Sequencer
```rust
Sequence::new(120)   // 120 BPM
    .every(Beat(0.25), |t| kick.trigger())
    .every(Beat(0.5),  |t| snare.trigger())
```
- Steps are defined relative to the global clock.
- Patterns can be any length (not just powers of two).

### 7.2 — Euclidean Rhythms
- `Euclid::new(hits: 3, steps: 8)` — generates a `Pattern` iterator.
- Necklace rotation: `.rotate(offset)`.
- Euclidean patterns are the backbone of much modern electronic music; they belong in the standard library.

### 7.3 — Seeded Randomness
- `nyx::random::seeded(42)` — deterministic PRNG (use `rand` with a fixed seed, or `wyrand` for speed).
- `rng.next_note_in(Scale::Minor("A"))` — scale-aware random note generation.
- `osc::noise().seeded(42)` — reproducible noise for generative sound design.
- The same seed must produce identical output across runs and platforms (use a portable PRNG, not the OS RNG).

### 7.4 — Pattern Combinators
- `.reverse()`, `.retrograde()` (reverse pitch, keep rhythm), `.invert()` (melodic inversion).
- `.concat(other)`, `.interleave(other)`.

---

## Phase 8 — The Macro-Synth Layer ("Vibe" Presets)

**Goal:** One-line instruments for rapid sketching.

### 8.1 — Preset Instruments
- `inst::kick()` — tunable sine-swept percussive hit with noise transient.
- `inst::snare()` — tunable noise burst with short decay.
- `inst::hihat(open: bool)` — filtered white noise.
- `inst::drone(note)` — self-modulating pad (slow LFO on filter cutoff).
- `inst::riser(duration)` — frequency-climbing noise sweep.
- `inst::pad(chord)` — detuned sawtooth chord with slow attack and reverb.

All preset instruments are built entirely from `nyx-core` primitives — they are documentation as much as they are features. Users are expected to read their source and copy-modify.

### 8.2 — Subtractive Synth Template
- A pre-wired `SubSynth` struct: oscillator → filter → ADSR → gain.
- Fully tweakable, fully documented.
- The canonical "how Nyx signals compose" reference implementation.

### 8.3 — Snapshot State-Saving
- `SynthPatch` — a concrete `enum`-based IR (Intermediate Representation) that captures the state of any preset instrument as a serialisable value.
- Serialise to / deserialise from `.toml` or `.json` via `serde`.
- **Note:** `dyn Signal` trait objects cannot be serialised directly. Only preset instruments (which have concrete types) support snapshots. Custom signal chains do not, by design — document this clearly.
- `patch.save("my_patch.toml")`, `SubSynth::load("my_patch.toml")`.

---

## Phase 9 — "Night-Safe" MIDI & Live Input

**Goal:** External control without breaking real-time safety.

### 9.1 — MIDI Input
- `midir` for cross-platform, low-latency MIDI input.
- MIDI CC messages mapped to `Param<S>` values via a `MidiMap`.
- **Zipper noise prevention:** CC values are smoothed through a one-pole low-pass before reaching the signal parameter (configurable time constant, default ~5ms).
- Note-on / note-off events sent to the voice pool via the SPSC bridge — no allocation in the callback.

### 9.2 — OSC Support
- `rosc` for Open Sound Control input (common in live performance environments).
- OSC addresses map to named `Param` handles.

### 9.3 — Microphone Input
- `input::mic()` returns a `Signal` sourced from the default input device.
- Full signal chain compatibility: `input::mic().lowpass(500).amp(2.0)`.

---

## Phase 10 — The `nyx-iced` GUI Layer (Optional)

**Goal:** A "Midnight Studio" visual interface for users who want knobs and scopes without writing a renderer.

This phase is explicitly opt-in (`nyx-iced` crate only). Users building headless or Bevy/Nannou integrations never touch this.

### 10.1 — Audio Widgets (via `iced_audio`)
- `Knob` — rotary control for frequency, gain, resonance.
- `HSlider` / `VSlider` — precision faders.
- `XYPad` — 2D modulation (e.g., filter cutoff vs. resonance).

### 10.2 — Visualisers
- `OscilloscopeCanvas` — renders a `ScopeHandle` from Phase 6 in real time.
- `SpectrumCanvas` — renders a `SpectrumHandle` as a frequency bar graph.

### 10.3 — Nyx Midnight Theme
- Deep grays, neon accent (configurable), monospace type.
- Defined as an Iced `Theme` impl — usable in any Iced application.

---

## Phase 11 — The Live-Diff & Hot Reload (The Moonshot)

**Goal:** Make Nyx usable as a live coding instrument where code changes take effect without stopping the audio.

This is the most technically complex phase. It should not be started until the core library is stable.

### 11.1 — Dynamic Library Loading
- DSP logic compiled as a `cdylib`.
- `hot-lib-reloader` or `libloading` watches the file and reloads on change.
- **The handoff seam:** the old signal chain must be drained to silence (amplitude envelope applied) before the new chain takes over. This prevents glitches and potential UB from calling into a deallocated library.

### 11.2 — The Standalone Player
- `nyx-cli` binary watches a `.rs` sketch file.
- On save: recompiles the DSP crate, performs graceful handoff, resumes audio.
- Target: < 2s from file save to audible change (incremental compilation).

### 11.3 — DAW Bridge (Stretch Goal)
- A virtual audio device or JACK/PipeWire output that `nyx-cli` streams into.
- Allows Ableton / Reaper to receive Nyx output as a live input channel.

---

## Appendix A — WASM / Web Audio Deferral

WASM via AudioWorklets is explicitly **deferred to v2.0+**. The reasons:

1. `SharedArrayBuffer` (required for the SPSC bridge) is gated behind COOP/COEP headers in browsers.
2. `cpal` does not currently have a stable WASM backend.
3. AudioWorklet threading differs fundamentally from native thread models.

When WASM is revisited, it requires its own design document addressing the bridge, atomics, and cpal backend choice independently.

---

## Appendix B — Competitive Positioning

| Tool | Strength | Nyx Advantage |
|---|---|---|
| **fundsp** | Mature Rust DSP graph | Nyx has higher-level API, music theory, sequencing, and visual mirror |
| **SuperCollider** | Extremely powerful, live coding ecosystem | Nyx: zero boilerplate, native Rust, no separate server process |
| **Sonic Pi** | Accessible, musician-friendly | Nyx: type-safe, faster, integrates with Rust graphics ecosystems |
| **JUCE (C++)** | DAW plugin standard | Nyx: Rust safety, modern API, not GPL-encumbered |

**Why not build on `fundsp`?** fundsp's graph model (fixed at compile time, algebra-based) is powerful but conflicts with Nyx's goal of a fluent, mutable, clock-driven API. The combinators diverge enough that the Nyx `Signal` trait needs to own its design. Credit fundsp in documentation; consider cross-compatibility examples.

---

## Appendix C — The "Cookbook" (Documentation Standard)

Every shipped phase must include at least two Cookbook entries — self-contained snippets of ≤ 20 lines demonstrating a real musical result.

Examples to target:
- "How to make a dubstep wobble" (LFO on filter cutoff, synced to BPM)
- "How to make wind" (pink noise + slow random LFO on gain)
- "How to make a generative melody" (Euclidean rhythm + scale snap + seeded RNG)
- "How to visualise a waveform in Nannou"
- "How to map a MIDI knob to filter cutoff"

The Cookbook is the primary onboarding path. **"Hello World" target: audio from speakers in under 5 minutes from `cargo new`.**

---

## Appendix D — Success Metrics

| Metric | Target |
|---|---|
| Hello World time | < 5 minutes from `cargo new` to first sound |
| Simultaneous oscillators | 100+ on a modern laptop without underruns |
| Default latency | < 20ms on default buffer settings |
| CI no-alloc guard | Zero allocation violations in audio callback, enforced per-PR |
| Golden file tests | All oscillators and filters regression-tested for DSP correctness |
| Phase-accuracy | Sine oscillator phase error < 1e-6 after 10s continuous playback |

---

## v2.0+ Future Scope

- **WAV/MP3 Export** — offline render to file (foundation laid by Phase 1.5)
- **Granular Synthesis** — `Grain` engine for texture-based creative coding
- **VST/AU Hosting** — use Nyx as a plugin inside a DAW (requires `vst3-sys` or `nih-plug`)
- **WASM / Web Audio** — browser target via AudioWorklets (see Appendix A)
- **Nyx Studio** — a full standalone `nyx-iced` application shipping the complete GUI
