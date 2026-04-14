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

Massive DX/UX wins for modest effort. Land these first.

### 1. Delay line with feedback (3–4 days) — HIGHEST VALUE
Fixed-size circular buffer. Unlocks echo, slapback, ping-pong,
Karplus-Strong, flanger, chorus, comb filtering in one primitive.

```rust
osc::saw(220.0).delay(0.375).feedback(0.4).mix(0.3)
```

- API: `Delay<S, const N: usize>` with `.feedback(f32)`, `.mix(wet)`
- Pre-allocated buffer → trivially no-alloc
- fundsp ships `delay()`, Tone.js has `FeedbackDelay`

### 2. WAV export / `render_to_wav` (1 day)
`render_to_buffer` already exists — just wrap it in a file writer.

```rust
nyx::render_to_wav(signal, 60.0, 44100.0, "track.wav")?;
```

- Add `hound = "3"` as optional feature dep
- Unlocks offline rendering workflow for non-real-time users

### 3. Sample playback (3–4 days)
Load WAV on the main thread, play back from an `Arc<[f32]>` with
variable-rate interpolation (linear or 4-point Hermite).

```rust
Sampler::load("kick.wav")?.pitch(1.5).loop_region(0.0, 1.0)
```

- Pitch via playback rate
- Loop points, one-shot mode
- Enables drum machines beyond synthesised `inst::kick` etc.
- Table-stakes for any beat-making audience

### 4. Probability & conditional steps in `Sequence` (1–2 days)
TidalCycles/Elektron-style pattern modifiers.

```rust
seq.prob(0.75)                    // drop 25% of hits randomly
seq.every(4, |s| s.reverse())     // every 4 bars, reverse
```

- Purely additive API on existing `Sequence<T>` / `Pattern<T>`
- Huge creative-coder appeal for minimal code

### 5. Bitcrusher + sample-rate reducer (½ day)
Bit depth quantisation + zero-order-hold downsampling.

```rust
signal.bitcrush(8).downsample(0.25)
```

- Stateless math, trivially RT-safe
- Beloved lo-fi / glitch effect, ubiquitous expected feature

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

**Sprint 1 (~1.5 weeks)**: #1 (Delay), #2 (WAV export), #3 (Sampler),
#4 (Probability), #5 (Bitcrusher)

**Sprint 2 (~3 weeks)**: #6 (Reverb), #7 (Stereo refactor), #8 (FM op),
#9 (Wavetable), #10 (SVF)

**Sprint 3**: #11 (Chorus/Flanger), #12 (Bus routing), #13 (Compressor),
#14 (Granular), #15 (Pitch detection)

After Sprint 1, Nyx will feel dramatically more complete for
beat-making and lo-fi work. After Sprint 2, it'll be genuinely
competitive with fundsp feature-wise while staying far more accessible.

---

## Sanity Checks

Before starting any item, verify:

- [ ] It compiles to a concrete type (no `Box<dyn>` except via `.boxed()`)
- [ ] Audio-callback path allocates zero bytes (test with `GuardedAllocator`)
- [ ] Parameters accept `IntoParam` so both `f32` and `Signal` work
- [ ] A cookbook example ≤ 20 lines demonstrates a real musical result
- [ ] Docs/manual updated with API shape and at least one example
- [ ] Golden-file regression test if the output is deterministic
