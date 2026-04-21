# Feature Roadmap — Beyond v1.0

Research-backed list of high-value features that would extend Nyx,
ranked by **value per unit effort**. All items fit Nyx's philosophy
(no-alloc in the audio callback, fluent combinators, creative-coder
friendly) unless marked otherwise.

Comparison libraries surveyed: `fundsp`, `dasp`, `nih-plug`,
SuperCollider, TidalCycles, Sonic Pi, Max/MSP, Pure Data,
`p5.sound.js`, Tone.js.

---

## Tier 1 — First Sprint (~1.5 weeks)

> **Implementation spec: [SPRINT_1_SPEC.md](SPRINT_1_SPEC.md)** — full
> API signatures, error types, test strategies, and open questions.
> That document supersedes the brief bullets here.

Order chosen for momentum and risk management: start with half-day wins
to build momentum, resolve the hardest design decision (delay API) mid-
sprint, then use the settled delay to trivialize Karplus-Strong, then
the largest item (Sampler), then pure additions (probability).

### 1. WAV Export (½ day)

`render_to_wav(signal, secs, sr, path)` wraps existing `render_to_buffer`
with `hound`. 16-bit signed mono default. Behind `wav` feature.

```rust
nyx::render_to_wav(osc::sine(440.0), 5.0, 44100.0, "tone.wav")?;
```

### 2. Bitcrusher + Sample-Rate Reducer (½ day)

Stateless math, confidence win before the harder items.

```rust
signal.bitcrush(8).downsample(0.25)
signal.crush(8, 0.25)  // shorthand
```

Both are pure combinators. `bits: u32` for v1 (fractional deferred).
`ratio: f32` in 0..1 (1.0 = identity, 0.5 = half rate).

### 3. Delay Line + Feedback (3–4 days) — HIGHEST VALUE

Foundation for chorus, flanger, reverb, ping-pong, Karplus-Strong.
Getting the API right here pays back across the whole feature roadmap.

```rust
osc::saw(220.0).delay(0.375).feedback(0.4).mix(0.3)
```

- **Uses `Box<[f32]>` (not const generic `N`)** — see architectural
  notes below.
- `.max_time()` builder for users who plan to modulate longer
- Feedback internally clamped to `[0.0, 0.95]` (silent clamp, debug log)
- Linear interpolation for v1 (Hermite deferred to v1.2)
- Time, feedback, and mix all accept `IntoParam`

### 4. Karplus-Strong `pluck()` (½ day)

Earns its own combinator despite being a delay+feedback+LP composition:

- Canonical DSP teaching example
- Frequency-to-delay-samples conversion is non-obvious
- One-line demos sell the library

```rust
play(pluck(440.0, 0.98)).unwrap();
```

### 5. Sampler (3–4 days)

Largest item. Load WAV on main thread into `Arc<[f32]>`, play back with
variable-rate linear interpolation. One-shot / Loop / PingPong modes.

```rust
let kick = Sample::load("kick.wav")?;
play(Sampler::new(kick).pitch(1.5)).unwrap();
```

- **Requires the "sample graveyard" infrastructure** (see below) to
  prevent audio-thread deallocation when the last `Arc` reference is
  dropped in the callback.
- Mono only in v1; stereo samples downmixed at load.

### 6. Probability & Conditional Steps (1–2 days)

TidalCycles/Elektron-style modifiers on existing `Sequence<T>`.

```rust
seq.prob(0.75)                     // drop 25% of hits randomly
seq.every(4, |s| s.reverse())       // every 4 bars, reverse
seq.sometimes(0.3, |s| s.rotate(2)) // 30% chance per bar
seq.degrade(0.25)                   // TidalCycles alias for .prob(0.75)
```

Pattern gets `.shuffle(seed)` (Fisher-Yates). Sequence uses a seeded
PRNG per-instance so live-diff reloads are reproducible.

---

## Project-Wide Architectural Notes

Surfaced by the Sprint 1 spec. These decisions apply beyond Sprint 1 and
should be treated as project-wide conventions.

### `Box<[f32]>` over const generic `N` for fixed buffers

For any effect that needs a pre-allocated buffer (delay, reverb, granular
grains, sample data), use a boxed slice allocated once at construction
time — **not** `const N: usize`.

**Why:**

- Const N locks buffer size at compile time, breaking runtime sample-rate
  flexibility (44.1 vs 48 kHz needs different sizes)
- Const N pollutes downstream types virally — `Delay<S, 96000>` and
  `Delay<S, 44100>` are different types; every combinator that wraps a
  delay would need its own const generic
- `Box<[f32]>` allocates once on the main thread before `play()`, never
  in the callback — the `GuardedAllocator` is not violated
- This is how `fundsp`, `Tone.js`, and DAW plugins handle it

Reserve `const N` for things that genuinely are fixed by nature (e.g.
`VoicePool<S, 16>` where the user picks the polyphony at construction
and it never changes).

### Sample graveyard for audio-thread `Arc` drops

Any signal that holds an `Arc<[f32]>` (Sampler, future Reverb impulse
responses, future Granular sample pools) has a lifecycle problem:

- `Arc::clone` on the audio thread is fine (atomic refcount bump)
- `Arc::drop` on the **last** reference calls the allocator
- If the audio thread ends up holding the last `Arc`, dropping happens
  in the callback → `GuardedAllocator` fires

**Fix:** a shared SPSC "graveyard" that lets the audio thread ship `Arc`s
back to the main thread for dropping.

```rust
// Conceptual API — flesh out in Sprint 1 alongside Sampler
pub(crate) struct Graveyard<T: Send + 'static>(Producer<Arc<T>>);

impl<T: Send + 'static> Graveyard<T> {
    pub fn send(&self, arc: Arc<T>) { let _ = self.0.push(arc); }
}
```

Factor this into `nyx-core` as first-class infrastructure during Sprint 1
so Sprint 2 reverb and Sprint 3 granular can reuse it immediately. Do
**not** ship it ad-hoc inside `Sampler`.

### `thiserror` for new error types

Adopt `thiserror = "1"` for new error types starting Sprint 1 (`WavError`,
`SampleError`). Existing hand-rolled `Display + Error` impls
(`EngineError`, `MicError`, `MidiError`, etc.) stay as-is; migrate them
opportunistically when touched for other reasons. `thiserror` is a tiny,
ubiquitous dep — the consistency win across the codebase outweighs the
extra crate.

---

## Sprint-Wide Checklist

Before merging any feature (Sprint 1 or later):

- [ ] Public API signatures locked and documented
- [ ] Concrete return types — no `Box<dyn>` except via explicit `.boxed()`
- [ ] Audio-callback path passes the `GuardedAllocator` test
- [ ] `IntoParam` accepted wherever a parameter could plausibly be modulated
- [ ] At least one ≤ 20-line cookbook example per item in
      `nyx-prelude/examples/`
- [ ] `docs/manual.md` updated with the API shape and an example
- [ ] Deterministic outputs have golden-file regression tests
- [ ] No new `unsafe` without a justification comment
- [ ] `cargo clippy --workspace -- -D warnings` still clean
- [ ] No regressions in existing tests (currently 254 + new tests)

---

## Cross-Cutting Open Questions

Decisions that shape multiple features. Resolve once, re-use everywhere.

1. **`SignalExt` placement.** One canonical `SignalExt` with all
   combinators, or grouped (`DelayExt`, `SamplerExt`, `FilterExt`)?
   Recommendation: **one `SignalExt`** for Sprint 1, split later if it
   grows beyond ~30 methods.

2. **`IntoParam` for `f32` vs `Signal`.** The fluency goal requires
   `osc::sine(440.0).delay(0.375)` to work (with bare `f32`) AND
   `osc::sine(440.0).delay(lfo)` (with a `Signal`). Our current
   `IntoParam` trait with associated `type Signal` handles both via
   blanket impls — keep as-is. Sprint 1 spec uses `IntoParam<f32>`
   notation as shorthand for "our `IntoParam` where the parameter is a
   float"; the trait itself doesn't need a type parameter.

3. **Graveyard infrastructure — built in Sprint 1.** Not ad-hoc. See
   architectural notes above.

4. **Feature flags.** `wav` feature gates both `render_to_wav` and
   `Sampler::load(path)`. `Sampler::from_buffer(...)` is available
   without the feature for users who load samples through other means.

---

## Tier 2 — Second Sprint (~3 weeks)

Significant features that round out the synthesis toolkit.

### 6. Reverb (Freeverb or Dattorro plate) (1 week)

The single most-requested effect.

- Freeverb: 8 comb + 4 allpass, ~200 LOC, fixed buffers
- Dattorro plate: lusher, more complex
- Essential for ambient/pad work; `inst::pad` already targets this
- fundsp ships `reverb_stereo`

### 7. Stereo signal type + proper pan/width (4–5 days) — REFACTOR

Introduce `StereoSignal` trait returning `(f32, f32)` or `[f32; 2]`.

```rust
osc::sine(440).widen(0.7).haas(15.0)
```

- Required before reverb/chorus/flanger feel right
- Parallel trait hierarchy — meaningful refactor but high leverage
- Current `.pan()` is a hack mono-fold; this makes stereo real

### 8. FM operator (3–4 days)

First-class phase modulation of a carrier with feedback.

```rust
osc::sine(440.0).fm(osc::sine(660.0), 2.0)
```

- Core to DX7-style patches, bells, basses
- Already expressible, but fluent combinator is the DX win
- Fits `Param<S>` perfectly

### 9. Wavetable oscillator (3 days)

User-drawn or preset waveforms with interpolation.

```rust
Wavetable::new(&[f32; 2048]).freq(220.0)
```

- Optional mipmapped bandlimiting
- Unlocks Serum/Vital-style sound design
- Pairs beautifully with hot-reload

### 10. State-variable filter (1–2 days)

SVF (Chamberlin or Andy Simper ZDF) gives LP/HP/BP/notch from one
struct with cleaner modulation than biquad.

```rust
signal.svf_lp(cutoff, q)
signal.svf_bp(cutoff, q)
signal.svf_notch(cutoff, q)
```

- Standard in fundsp, SuperCollider
- Small code, real sonic upgrade (smoother sweeps)
- Complements existing biquad, doesn't replace

---

## Tier 3 — Third Sprint

Ecosystem + advanced synthesis.

### 11. Chorus + Flanger (1 day each, after #1)

Both are modulated short delays. ~50 LOC wrappers on Delay.
Ubiquitous, expected.

### 12. Bus / send-return routing (1 week)

"One reverb, many sources" — the standard DAW mental model.

```rust
let reverb_bus = Bus::new();
voice_a.send(&reverb_bus, 0.3);
voice_b.send(&reverb_bus, 0.5);
let mix = dry_mix + reverb_bus.process(dattorro_reverb);
```

- Uses SPSC/atomic send-level pattern (same as CC)
- High value for anything beyond single-voice sketches

### 13. Compressor / sidechain (3–4 days)

Feed-forward RMS or peak compressor with attack/release/ratio/threshold.
Sidechain input = a second `Signal`.

```rust
bass.compress(threshold, ratio).sidechain(kick_trigger)
```

- Essential for pumping house kicks, glue on buses
- Completes the dynamics section (peak limiter + clip already exist)

### 14. Granular engine (1–2 weeks)

Fixed grain pool (same pattern as `VoicePool<S, N>`).

```rust
Granular::new(sample, GRAIN_POOL_SIZE)
    .grain_rate(10.0)
    .grain_length(0.05)
    .position(0.5)
```

- Time-stretch, clouds, texture synthesis
- Distinctive — few Rust libs have it (fundsp doesn't)
- Fits the pool pattern we already use

### 15. Pitch detection (YIN or autocorrelation) (4–5 days)

Enables mic-driven synths, tuners, auto-harmonisers.

```rust
let pitch = mic_signal.pitch_detect(YinConfig::default());
```

- Pairs with existing `mic()`
- YIN in a fixed-size window is well-documented, RT-safe
- Differentiator vs fundsp/dasp (neither has it)

---

## Skip / Defer — Not Worth the Effort

These were considered and rejected:

- **Autotune / vocoder / time-stretch** — weeks each, narrow audience,
  phase-vocoder needs FFT-in-callback (fights no-alloc rule)
- **Physical modelling beyond Karplus-Strong** — niche; Karplus-Strong
  falls out of #1 (delay + feedback) for free
- **Markov / L-systems sequencing** — already expressible in user code
  via existing PRNG + `Sequence`; would be library bloat
- **Additive synth with 100 partials** — already trivial by summing
  sines; a helper is nice but low priority
- **MIDI output** — possible, but most use cases covered by existing
  SPSC pattern; defer until a concrete user asks
- **Polyphonic aftertouch / MPE** — niche controllers; add if demand
  materialises

---

## Recommended Order

**Sprint 1 (~1.5 weeks)** — see [SPRINT_1_SPEC.md](SPRINT_1_SPEC.md):
WAV export → Bitcrusher → Delay → Karplus-Strong → Sampler → Probability

**Sprint 2 (~3 weeks)**: Reverb, Stereo refactor, FM operator,
Wavetable, State-variable filter

**Sprint 3**: Chorus/Flanger, Bus routing, Compressor, Granular,
Pitch detection

After Sprint 1, Nyx will feel dramatically more complete for
beat-making and lo-fi work. After Sprint 2, it'll be genuinely
competitive with fundsp feature-wise while staying far more accessible.

Before starting any item, run through the **Sprint-Wide Checklist**
above.

---

## Audio Analysis Scopes (Nice to Have)

Nyx currently ships **two** analysis taps — `.scope()` (oscilloscope)
and `.spectrum()` (FFT), plus the adjacent `.pitch()` (YIN). The rest
of the classical audio-engineering scope family is absent. They all
fit the same "passive tap + lock-free handle" architecture as the
existing two, so they drop in cleanly as an incremental sprint.

Not prioritised — pick up when a concrete use case (a specific Iced
widget, a mastering tool, a room-measurement workflow) motivates one.

| Category | Tool | Status | Rough effort |
| --- | --- | --- | --- |
| Amplitude / Level | Peak Meter | ❌ | ½ day (rolling max, one-pole decay) |
| Amplitude / Level | RMS Meter | ❌ | ½ day (running sum of squares, sliding window) |
| Amplitude / Level | VU Meter | ❌ | trivial on top of RMS — add the 300 ms ballistic lag |
| Amplitude / Level | LUFS (ITU-R BS.1770) | ❌ | 1–2 days — K-weighting biquads + 400 ms integration + gating |
| Time-Domain | Oscilloscope | ✅ shipped | [`scope.rs`](../nyx-core/src/scope.rs) |
| Frequency-Domain | Spectrum Analyzer (FFT) | ✅ shipped | [`spectrum.rs`](../nyx-core/src/spectrum.rs) |
| Frequency-Domain | Spectrogram | ❌ | 1 day — 2-D ring buffer of FFT frames with a frame counter |
| Stereo / Phase | Goniometer (Vectorscope) | ❌ | ½ day — X-Y tap on `(L, R)` via `next_stereo` |
| Stereo / Phase | Correlation Meter | ❌ | ½ day — running cross-correlation, `[-1, +1]` output |
| Specialised | 3D Waterfall Plot | ❌ | same primitive as Spectrogram with a different rendering contract |
| Specialised | Real-Time Analyzer (RTA) | ❌ | 1 day — Spectrum + pink-noise stimulus + calibration |

**Recommended bundling** (if/when picked up):

1. **Meters pack** — PeakMeter + RmsMeter + CorrelationMeter in one PR
   (~1 day total). All three are trivial and share the "atomic f32
   writeback from the audio thread" pattern.
2. **Goniometer** — pairs naturally with CorrelationMeter since both
   operate on `(L, R)`. Adds maybe a half day on top of the meters pack.
3. **VU Meter** — once RMS is in, VU is 30 LOC of ballistic lag.
4. **Spectrogram** — 1 day, extends Spectrum with a time-axis ring.
   Blocks Waterfall, which is the same data with a different read
   interface.
5. **LUFS** — 1–2 days standalone. Only worth shipping if mastering
   becomes a first-class Nyx use case (not currently in the mission
   statement — "p5.js of sound", not "Pro Tools in Rust").
6. **RTA** — last, and only if room-measurement lands on the roadmap.

**Architecture note.** All of these go in `nyx-core/src/*.rs` as new
modules next to `scope.rs` / `spectrum.rs`, follow the same `SignalExt`
method → `(Tap<S>, Handle)` pattern, and stay pass-through so they
compose freely with existing processing chains.
