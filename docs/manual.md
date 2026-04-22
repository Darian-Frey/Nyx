# Nyx Audio — User Manual

> High-performance audio synthesis and sequencing for Rust.

Nyx is the "p5.js of sound" — a library for creative coders, algorithmic
composers, and live performers to sketch with audio using a fluent, expressive
API, without managing buffers, thread safety, or DSP boilerplate.

---

## Table of Contents

1. [Quick Start](#quick-start)
2. [The Prelude](#the-prelude)
3. [Core Concepts](#core-concepts)
4. [Oscillators & Noise](#oscillators--noise)
5. [Signal Combinators](#signal-combinators)
6. [Filters](#filters)
7. [Dynamics](#dynamics)
8. [Clock & Timing](#clock--timing)
9. [Envelopes](#envelopes)
10. [Automation](#automation)
11. [Music Theory](#music-theory)
12. [Patterns & Sequencing](#patterns--sequencing)
13. [Euclidean Rhythms](#euclidean-rhythms)
14. [Randomness](#randomness)
15. [Instruments](#instruments)
16. [SubSynth & Patches](#subsynth--patches)
17. [Visual Mirror (Scope & Spectrum)](#visual-mirror)
18. [MIDI Input](#midi-input)
19. [OSC Input](#osc-input)
20. [Microphone Input](#microphone-input)
21. [GUI Widgets (nyx-iced)](#gui-widgets)
22. [Hot Reload (nyx-cli)](#hot-reload)
23. [Cookbook Examples](#cookbook-examples)
24. [Nannou & Bevy Visualisers](#nannou--bevy-visualisers)
25. [Testing Utilities](#testing-utilities)
26. [Real-Time Safety](#real-time-safety)
27. [API Reference](#api-reference)
28. [Roadmap](#roadmap)
29. [License](#license)

---

## Quick Start

Add `nyx-prelude` to your project:

```bash
cargo add nyx-prelude
```

Play a sine wave:

```rust
use nyx_prelude::*;

fn main() {
    play(osc::sine(440.0).amp(0.3)).unwrap();
}
```

This opens the default audio device, plays a 440 Hz sine at 30% volume, and
blocks until you press Enter.

### Non-blocking playback

```rust
use nyx_prelude::*;

fn main() {
    let _engine = play_async(osc::sine(440.0).amp(0.3)).unwrap();
    // Do other work...
    std::thread::sleep(std::time::Duration::from_secs(5));
    // Audio stops when _engine is dropped.
}
```

---

## The Prelude

`use nyx_prelude::*;` is the recommended import for sketches and apps. It
re-exports everything you typically need from `nyx-core` and `nyx-seq` in one
line.

### Modules

You get these modules as bare names (no need for `nyx_core::` or `nyx_seq::`
prefixes):

| Module | Contents |
|---|---|
| `osc` | Oscillators: `osc::sine`, `osc::saw`, `osc::square`, `osc::triangle`, `osc::noise` |
| `filter` | `FilterExt` trait and biquad types |
| `dynamics` | `gain()`, `peak_limiter()` |
| `clock` | `clock::clock(bpm)` constructor |
| `envelope` | `envelope::adsr(a, d, s, r)` constructor |
| `automation` | `automation::automation(\|t\| ...)` |
| `inst` | Drum and instrument primitives: `inst::kick()`, `inst::snare()`, etc. |
| `midi` | MIDI event types and parsing (feature-gated) |
| `osc_input` | OSC parameter store (feature-gated) |
| `mic` | Microphone input (audio feature) |
| `golden` | Golden-file testing framework |
| `hotswap` | `HotSwap` crossfade engine |

### Traits & Types

These are brought into scope directly:

- **Signal core:** `Signal`, `SignalExt`, `Param`, `IntoParam`, `ConstSignal`,
  `AudioContext`, `VoicePool`
- **Combinators (structs):** `Amp`, `Add`, `Mul`, `Mix`, `Pan`, `Clip`,
  `SoftClip`, `Offset`
- **Filters:** `FilterExt`, `Biquad`, `FilterMode`
- **Dynamics:** `Gain`, `PeakLimiter`
- **Scope/Spectrum:** `ScopeExt`, `Scope`, `ScopeHandle`, `InspectExt`,
  `Inspect`, `SpectrumExt`, `Spectrum`, `SpectrumConfig`, `SpectrumHandle`,
  `WindowFn`, `FreqBin`
- **Engine (audio feature):** `Engine`, `EngineConfig`, `EngineError`
- **Bridge / safety:** `bridge`, `AudioCommand`, `DenyAllocGuard`,
  `GuardedAllocator`
- **Clock / envelope / automation:** `Clock`, `ClockState`, `Adsr`, `Stage`,
  `Automation`, `AutomationExt`, `Follow`
- **Music theory:** `Note`, `Scale`, `ScaleMode`, `Chord`, `ChordType`
- **Patterns & sequencing:** `Pattern`, `Euclid`, `Rng`, `seeded`, `Sequence`,
  `StepEvent`
- **Synth:** `SubSynth`, `SynthPatch`, `OscShape`, `FilterType`, `PatchError`

### Functions

- `play(signal)` — blocking playback (waits for Enter)
- `play_async(signal)` — non-blocking, returns the `Engine` handle
- `render_to_buffer(signal, secs, sr)` — offline rendering

### Example — everything with one import

```rust
use nyx_prelude::*;

fn main() {
    // Musical: use scale and chord types
    let scale = Scale::major("C");
    let chord = Chord::major(Note::C4);

    // Clock + sequencer
    let mut clk = clock::clock(120.0);
    let pattern = Euclid::generate(3, 8);
    let mut seq = Sequence::new(pattern, 0.25);

    // Oscillator chain
    let lfo = osc::sine(0.5).amp(400.0).offset(800.0);
    let signal = osc::saw(220.0)
        .lowpass(lfo, 0.707)
        .amp(0.3);

    play(signal).unwrap();
}
```

### Sketch files for hot reload

Sketches loaded by `nyx-cli` use the same prelude:

```rust
use nyx_prelude::*;

#[unsafe(no_mangle)]
pub fn nyx_sketch() -> Box<dyn Signal> {
    osc::sine(440.0).amp(0.3).boxed()
}
```

If you prefer explicit imports, you can still reach into the underlying
crates directly: `use nyx_core::osc;` and `use nyx_seq::inst;` etc.

---

## Core Concepts

### The Signal Trait

Everything in Nyx is a **Signal** — a stream of audio samples. Every
signal produces a mono output and, by default, a stereo `(L, R)` pair
that duplicates the mono sample:

```rust
pub trait Signal: Send {
    fn next(&mut self, ctx: &AudioContext) -> f32;

    /// Default: duplicates mono into both channels.
    /// Stereo-native signals (Pan, Haas, Reverb) override this.
    fn next_stereo(&mut self, ctx: &AudioContext) -> (f32, f32) {
        let s = self.next(ctx);
        (s, s)
    }
}
```

`AudioContext` carries the sample rate and an absolute tick counter:

```rust
pub struct AudioContext {
    pub sample_rate: f32,
    pub tick: u64,
}
```

Any closure matching `FnMut(&AudioContext) -> f32 + Send` automatically
implements `Signal`:

```rust
let my_signal = |ctx: &AudioContext| {
    (ctx.tick as f32 * 440.0 / ctx.sample_rate * std::f32::consts::TAU).sin()
};
```

The audio engine calls `next_stereo` once per frame and writes the
two samples to the output's left and right channels. Mono-only
hardware folds the two channels back to `L + R`.

### Stereo width (Pan + Haas)

Two combinators override `next_stereo` to produce real stereo:

```rust
// Pan: linear pan law, -1 (hard left) to +1 (hard right)
osc::saw(220.0).pan(0.5)         // 75% right

// Haas: mono → widened stereo via a short delay on one channel
osc::saw(220.0).haas(15.0)       // right channel lags by 15 ms
osc::saw(220.0).haas_side(15.0, HaasSide::Left)
```

Mono-fold behaviour:

- **Pan** — pan-law preserving; `L + R` equals the input signal
- **Haas** — mono-folds to a mild comb filter (small HF dip at typical
  widths, usually inaudible)

### Param — Static or Modulated

Many parameters (frequency, cutoff, gain) accept either a fixed `f32` or a
`Signal` via the `IntoParam` trait:

```rust
osc::sine(440.0)           // fixed frequency
osc::sine(lfo)             // frequency modulated by another signal
```

This works because both `f32` and any `Signal` implement `IntoParam`.

### Static Dispatch by Default

Combinator methods return concrete types — `osc::sine(440.0).amp(0.5)` returns
`Amp<Sine<ConstSignal>, ConstSignal>`, not a `Box<dyn Signal>`. Zero allocation.

Use `.boxed()` when you need type erasure (heterogeneous collections, recursive
graphs):

```rust
let signals: Vec<Box<dyn Signal>> = vec![
    osc::sine(440.0).boxed(),
    osc::saw(220.0).amp(0.5).boxed(),
];
```

---

## Oscillators & Noise

All oscillators live in `nyx_core::osc`. Frequency accepts `f32` or any `Signal`.

| Function | Waveform | Start value | Band-limited |
| --- | --- | --- | --- |
| `osc::sine(freq)` | Sine wave | 0.0 | n/a — one harmonic |
| `osc::saw(freq)` | Naive sawtooth (aliases above ~1 kHz) | -1.0 | ❌ |
| `osc::saw_bl(freq)` | Band-limited sawtooth (PolyBLEP) | ≈ -1.0 | ✅ |
| `osc::square(freq)` | Naive square (aliases above ~1 kHz) | +1.0 | ❌ |
| `osc::square_bl(freq)` | Band-limited square (PolyBLEP) | ≈ +1.0 | ✅ |
| `osc::pwm_bl(freq, width)` | Band-limited pulse with modulatable duty cycle | depends on width | ✅ |
| `osc::triangle(freq)` | Triangle (harmonics fall as 1/n², rarely audible aliasing) | -1.0 | n/a |

All oscillators track phase as a normalised `f32` in [0, 1), incremented by
`freq / sample_rate` each sample.

#### Naive vs. band-limited

`osc::saw` and `osc::square` generate the mathematical ideal waveform —
every harmonic above Nyquist folds back as inharmonic aliasing, giving
the characteristic "digital" / chiptune / 8-bit edge. Reach for them
when you want that retro timbre.

The `_bl` variants apply PolyBLEP (Polynomial Band-Limited stEP)
correction at each discontinuity to suppress aliasing ~70 dB down.
Use these for clean subtractive-synth leads, supersaws, and anything
in the mid-to-high register where the naive aliasing bites.

```rust
// Retro acid bass — naive saw, aliases are the point
osc::saw(110.0).lowpass(600.0, 0.7)

// Clean trance lead — band-limited
osc::saw_bl(880.0).lowpass(6000.0, 0.7)

// Juno-style PWM pad — modulatable pulse width
let lfo = osc::sine(0.3).amp(0.15).offset(0.5);
osc::pwm_bl(220.0, lfo)
```

`pwm_bl` clamps `width` to `[0.05, 0.95]` so the up-edge and down-edge
PolyBLEP windows never collide. At `width = 0.5` it reproduces
`square_bl` sample-for-sample.

### Noise

```rust
osc::noise::white(seed)   // uniform [-1, 1], xorshift32
osc::noise::pink(seed)    // pink noise (-3 dB/octave), Paul Kellett filter
```

Both use a portable xorshift32 PRNG — same sequence on all platforms.
Pink noise is implemented via the Paul Kellett filter (five parallel
one-poles driven from a shared white source) for constant per-sample
cost and a clean 1/f slope.

### Frequency Modulation

```rust
// Vibrato: sine modulated by a slow LFO
let vibrato = osc::sine(5.0).amp(10.0).offset(440.0);
let signal = osc::sine(vibrato);
```

### Wavetable

User-drawn or preset waveforms with linear interpolation. A `Wavetable`
holds one period as `Arc<[f32]>` — cheap to clone across many voices.

```rust
// Preset tables
Wavetable::sine(2048).freq(440.0)
Wavetable::saw(2048).freq(110.0)
Wavetable::square(2048).freq(220.0)
Wavetable::triangle(2048).freq(440.0)

// Custom shape via closure (t ∈ [0, 1))
let harmonic_stack = Wavetable::from_fn(4096, |t| {
    let tau = t * std::f32::consts::TAU;
    (tau.sin() + (2.0 * tau).sin() * 0.3 + (3.0 * tau).sin() * 0.2) / 1.5
});
let osc = harmonic_stack.freq(220.0);

// Or from raw data
let table = Wavetable::from_vec(my_samples);
```

| Method | Description |
| --- | --- |
| `Wavetable::new(&[f32])` / `::from_vec(Vec<f32>)` | Build from existing samples |
| `Wavetable::from_fn(size, f)` | Sample `f(t)` at `size` points with `t ∈ [0, 1)` |
| `Wavetable::sine/saw/square/triangle(size)` | Preset naïve waveforms |
| `.freq(impl IntoParam) -> WavetableOsc` | Build an oscillator reading this table |
| `.len()` / `.is_empty()` / `.clone()` | Introspection + cheap refcount-bump clone |

Cloning a `Wavetable` is a refcount bump, not a data copy — build a
table once, share it across an arbitrary number of voices.

**Lifetime caveat:** same as [`Sample`](#sample-playback) — keep one
reference to the source `Wavetable` alive while any oscillator cloned
from it is playing, so the final `Arc::drop` doesn't land on the audio
thread.

### FM Operator (Phase Modulation)

True DX7-style FM — technically *phase modulation* of a sine carrier.
The classic bell/electric-piano/bass sound engine.

```rust
// DX7-style bell with a 1:2 modulator ratio, index 3
play(fm_op(440.0, osc::sine(880.0), 3.0)).unwrap();

// Fluent alternative: convert an existing sine into an FM operator
osc::sine(440.0).fm(osc::sine(880.0), 3.0)

// Self-feedback adds harmonics (sawtooth-ish as feedback → 1)
fm_op(440.0, osc::sine(660.0), 2.0).feedback(0.5)
```

**Formula:**

```text
output = sin(2π * (phase + index * modulator + feedback * last_output))
phase += carrier_freq / sample_rate
```

| Parameter | Typical range | Effect |
| --- | --- | --- |
| `carrier_freq` | any Hz | Pitch of the operator |
| `modulator` | any `Signal` | Added to the carrier's phase each sample |
| `index` | `0.0` – `~10.0` | Modulation depth. `0` = pure sine; `3+` = bright bells |
| `feedback` | `[-1, 1]` (clamped) | Routes own output back into phase — adds harmonics |

Carrier freq, modulator, and index all accept `IntoParam` — so any of
them can be modulated by another signal, envelope, or LFO.

**Classical FM ratios** (modulator/carrier):

- `1:1` → sawtooth-ish
- `1:2` → bell / chime
- `1:3` → clarinet
- `2:3` → brass
- non-integer → inharmonic metal

---

## Signal Combinators

Every `Signal` gains these methods via `SignalExt`:

| Method | Description | Example |
|---|---|---|
| `.amp(gain)` | Multiply by gain (f32 or Signal) | `sig.amp(0.5)` |
| `.add(other)` | Sum two signals | `sig.add(other)` |
| `.mul(other)` | Multiply two signals | `sig.mul(lfo)` |
| `.mix(other, ratio)` | Crossfade (0=self, 1=other) | `sig.mix(other, 0.5)` |
| `.pan(pos)` | Stereo pan (-1=L, +1=R) | `sig.pan(0.0)` |
| `.clip(threshold)` | Hard clip to [-t, +t] | `sig.clip(0.8)` |
| `.soft_clip(drive)` | Tanh saturation | `sig.soft_clip(2.0)` |
| `.offset(value)` | Add DC offset | `sig.offset(0.5)` |
| `.boxed()` | Type-erase to `Box<dyn Signal>` | `sig.boxed()` |

Combinators chain fluently:

```rust
osc::saw(220.0)
    .amp(0.5)
    .add(osc::sine(221.0).amp(0.3))
    .clip(0.8)
    .soft_clip(1.5)
```

---

## Filters

Resonant biquad filters (Transposed Direct Form II) with automatic coefficient
smoothing (~5ms one-pole) to prevent clicks:

```rust
use nyx_core::filter::FilterExt;

// Static cutoff and Q
osc::saw(110.0).lowpass(800.0, 0.707)

// Modulated cutoff (LFO on filter)
let lfo = osc::sine(0.5).amp(400.0).offset(800.0);
osc::saw(110.0).lowpass(lfo, 2.0)
```

| Method | Description |
| --- | --- |
| `.lowpass(cutoff, q)` | Resonant biquad low-pass filter (smoothed ~5 ms) |
| `.highpass(cutoff, q)` | Resonant biquad high-pass filter (smoothed ~5 ms) |

Both `cutoff` and `q` accept `f32` or any `Signal`.

### State-Variable Filter (SVF)

For fast-modulating cutoff or Q, use the zero-delay-feedback (ZDF)
state-variable filter. It tracks parameter changes correctly at the
sample rate with no coefficient smoothing needed — so you can sweep
the cutoff at audio rate without clicks, which biquad can't do
cleanly.

```rust
// Wobble bass: audio-rate cutoff modulation
let wobble = osc::sine(4.0).amp(1500.0).offset(1700.0);
osc::saw(55.0).svf_lp(wobble, 4.0).soft_clip(1.5)
```

| Method | Description |
| --- | --- |
| `.svf_lp(cutoff, q)` | SVF low-pass |
| `.svf_hp(cutoff, q)` | SVF high-pass |
| `.svf_bp(cutoff, q)` | SVF band-pass (higher Q → narrower band) |
| `.svf_notch(cutoff, q)` | SVF notch / band-reject |

**Topology:** Andy Simper's "Linear Trapezoidal State Variable Filter"
(2013) — the same variant used in Surge XT, Vital, and other modern
soft synths.

**When to use biquad vs SVF:**

- **Biquad** — static or slowly-changing filters, mastering EQ, filter
  banks. Cheaper (fixed coefficients after smoothing).
- **SVF** — wobble basses, formant sweeps, fast filter LFOs, anything
  that needs per-sample modulation. Exposes band-pass and notch modes
  that biquad currently doesn't.

### Ladder Filter (Moog-style)

Huovilainen's 4-pole non-linear ladder — the canonical "analog synth
lowpass" sound. Four cascaded one-pole stages with global feedback
through a saturating `tanh`, plus a `tanh` on each stage's input and
state. Self-oscillates at `resonance ≥ 1.0` (the acid-bass squeal).

```rust
use nyx_core::LadderExt;

// Acid bass: squelchy saw through a resonant ladder, LFO'd cutoff.
let lfo = osc::sine(0.3).amp(400.0).offset(800.0);
let acid = osc::saw_bl(55.0).amp(0.6).ladder_lp(lfo, 1.05);
```

| Method | Description |
| --- | --- |
| `.ladder_lp(cutoff, resonance)` | Non-linear 4-pole lowpass. `cutoff` clamped to `[20, sr·0.45]`, `resonance` to `[0, 1.2]` |

**DC gain drops with resonance** — the canonical Moog behaviour. DC
gain ≈ `1 / (1 + 4·resonance)`: unity at `resonance = 0`, ⅓ at `0.5`,
⅕ at `1.0`. Scale input or add `.amp(1.0 + 4.0 * resonance)` upstream
to compensate.

**When to use ladder vs SVF/biquad:**

- **Biquad / SVF** — clean, linear filters for mastering, formant EQ,
  general subtractive synthesis. Neither self-oscillates musically.
- **Ladder** — acid basses, squelchy leads, analog character. The
  non-linearity is the sound; don't reach for it when you want a
  transparent filter.

---

## Dynamics

```rust
use nyx_core::dynamics;

// Named gain processor
let sig = dynamics::gain(osc::sine(440.0), 0.5);

// Peak limiter (threshold, attack_ms, release_ms, sample_rate)
let sig = dynamics::peak_limiter(loud_signal, 1.0, 0.1, 100.0, 44100.0);
```

Hard clip and soft clip are also available as combinators:

```rust
signal.clip(0.8)        // hard clip to [-0.8, 0.8]
signal.soft_clip(2.0)   // tanh(signal * 2.0)
```

### Reverb (Freeverb)

Classic Freeverb stereo reverb — 8 parallel lowpass-feedback comb
filters feeding 4 series Schroeder all-pass filters, with a stereo
spread on the right channel. Public-domain algorithm from Jezar at
Dreampoint (2000). Overrides `next_stereo` for genuine stereo bloom.

```rust
let wet = osc::saw(220.0)
    .freeverb()
    .room_size(0.85)     // larger → longer decay
    .damping(0.5)        // more → warmer, less HF tail
    .width(1.0)          // 1 = full stereo spread, 0 = mono reverb
    .wet(0.3);           // 0 = dry only, 1 = pure reverb
```

| Method | Range | Effect |
| --- | --- | --- |
| `.room_size(r)` | `[0, 1]` | Comb feedback. Longer tails at higher values. |
| `.damping(d)` | `[0, 1]` | One-pole lowpass on feedback. Warmer/darker at higher values. |
| `.wet(w)` | `[0, 1]` | Wet/dry mix. |
| `.width(w)` | `[0, 1]` | `1` = full stereo spread, `0` = mono reverb (both channels identical). |

**Mono compat:** `next()` returns `dry + (wet_l + wet_r) / 2`. At
`wet(0)`, mono output is the dry signal unchanged.

**Buffers:** all comb and allpass buffers pre-allocated at construction
(sized for 96 kHz upper bound); zero allocation per sample. At first
`.next()` call, the active ring lengths are scaled to match the actual
stream sample rate.

### Chorus & Flanger

Two modulated-delay effects, both producing genuine stereo via LFOs
offset by 180° on left and right channels.

```rust
// Chorus: thickens a mono source (15–30 ms base delay, 1–10 ms depth)
osc::saw(220.0).chorus(0.5, 3.0)

// Flanger: short delay + feedback, the classic jet whoosh
osc::saw(110.0).flanger(0.3, 2.0).feedback(0.7)
```

| Effect | Base delay | Feedback | Typical rate | Character |
| --- | --- | --- | --- | --- |
| `.chorus(rate_hz, depth_ms)` | 20 ms | none | 0.1–3 Hz | Ensemble, widening, detune |
| `.flanger(rate_hz, depth_ms)` | 2.5 ms | 0 (configurable) | 0.1–1 Hz | Swooping comb filter, jet plane |

Both return types expose builder methods:

| Method | Applies to | Default |
| --- | --- | --- |
| `.mix(wet)` | both | `0.5` |
| `.base_delay(ms)` | both | `20.0` chorus, `2.5` flanger |
| `.feedback(amount)` | flanger only | `0.0` (clamped to `[0, 0.95]`) |

### Compressor & Sidechain

A feed-forward compressor with peak detection and an asymmetric attack/
release envelope follower. Two entry points on `SignalExt`:

```rust
// Self-compression — the signal drives its own gain reduction.
drums.compress(-12.0, 4.0)
    .attack_ms(5.0)
    .release_ms(100.0)
    .makeup_db(3.0);

// Sidechain — an external trigger drives the reduction on `self`.
// The trigger is consumed for detection only; it is never audible.
bass.sidechain(kick, -20.0, 8.0)
    .attack_ms(1.0)
    .release_ms(150.0);
```

| Method | Applies to | Default | Notes |
| --- | --- | --- | --- |
| `.compress(threshold_db, ratio)` | `SignalExt` | — | `ratio >= 1.0`, `f32::INFINITY` = limiter |
| `.sidechain(trigger, threshold_db, ratio)` | `SignalExt` | — | `trigger: impl Signal`; detected on, not heard |
| `.threshold_db(db)` | both | construction | Re-set threshold after construction |
| `.ratio(r)` | both | construction | `r` is clamped to ≥ 1.0 internally |
| `.attack_ms(ms)` | both | `5.0` | Envelope rise time |
| `.release_ms(ms)` | both | `100.0` | Envelope fall time |
| `.makeup_db(db)` | both | `0.0` | Post-compression gain |

**Stereo handling.** Both compressors detect on `max(|L|, |R|)` and apply
one gain reduction to both channels. This preserves the stereo image
instead of collapsing panned content.

**Sidechain = trance pumping bass.** Ducking a bassline with a kick drum
on every beat is the archetypal use case. See
[`sidechain_pump.rs`](../nyx-prelude/examples/sidechain_pump.rs) for a
minimal 128 BPM demo.

### Bus / Mixer

`Bus` is a collection of signals summed into a single output, with a
post-sum gain. Because `Bus` itself implements `Signal`, bus processing
like compression, EQ, or reverb is just the usual fluent chain applied
to the bus as a whole:

```rust
let drums = Bus::new()
    .add(kick)
    .add(snare.amp(0.7))
    .add(hat.amp(0.4))
    .gain(0.85)
    .compress(-10.0, 4.0);   // bus compression

let mix = Bus::new()
    .add(drums)
    .add(bass)
    .add(pad.freeverb().wet(0.4))
    .soft_clip(1.1);          // master bus limiter

play(mix).unwrap();
```

| Method | Purpose |
| --- | --- |
| `Bus::new()` | Empty bus |
| `Bus::with_capacity(n)` | Empty bus with pre-reserved `Vec` capacity |
| `.add(source)` | Append a source (heap-allocs a `Box<dyn Signal>` — pre-stream only) |
| `.gain(g)` | Post-sum gain (default `1.0`) |
| `.len()` / `.is_empty()` | Introspection |

**Real-time safety.** `.add()` allocates (one `Box` per source) but must
be called before `play()`. Once the stream is running, the bus callback
never allocates — the underlying `Vec<Box<dyn Signal>>` is iterated
in place.

**Stereo.** `Bus::next_stereo` sums each source's stereo output, so
panned / stereo-native sources (e.g. a `.haas()` widener, a
`.freeverb()` room) retain their image through the mix.

**Nested buses.** Buses compose — a bus can contain other buses — so
a "drum bus → master bus" topology is natural.

**Send / return pattern.** Nyx does not provide a true multi-reader
send bus. Express sends by building a separate instance of the source
scaled by the send amount, then routing it into a dedicated effect bus:

```rust
// 30% of the lead goes to a reverb return; 100% stays dry.
let lead_dry = make_lead();
let lead_send = make_lead().amp(0.3).freeverb().wet(1.0);
let mix = Bus::new().add(lead_dry).add(lead_send);
```

The sources stay in sync when triggered from the same clock, MIDI event,
or `OscParam`. See [`multi_bus.rs`](../nyx-prelude/examples/multi_bus.rs)
for a full drum-bus / harmony-bus / master-bus topology.

### Delay / Echo

Single-buffer delay line with feedback and wet/dry mix. Foundation for
chorus, flanger, ping-pong, Karplus-Strong, and comb filtering.

```rust
osc::saw(220.0)
    .delay(0.375)    // 375 ms base delay
    .feedback(0.4)   // 40% feedback (clamped to [0, 0.95] internally)
    .mix(0.3)        // 30% wet
```

| Method | Parameter | Effect |
| --- | --- | --- |
| `.delay(time_secs: f32)` | seconds, sets max buffer size | Start the delay chain |
| `.time(secs: impl IntoParam)` | static `f32` or `Signal` | Set/modulate delay time (smoothed ~5 ms) |
| `.feedback(amount: impl IntoParam)` | clamped to `[0, 0.95]` | Recirculation amount; smoothed |
| `.mix(wet: impl IntoParam)` | clamped to `[0, 1]` | 0 = dry, 1 = pure wet; smoothed |
| `.max_time(secs: f32)` | reallocates buffer | Call before `play()` if you plan to modulate `time` beyond the initial value |

**Key design choices:**

- **Single allocation** at construction (main thread), `Box<[f32]>` sized
  for the maximum requested delay at `DELAY_MAX_SR = 96_000` Hz. Zero
  allocation per sample in the audio callback.
- **Linear interpolation** on buffer reads — clean enough for typical
  modulation rates (< 10 Hz). Hermite interpolation deferred to v1.2.
- **All three parameters smoothed** by a ~5 ms one-pole filter to
  prevent zipper noise when modulated.
- **Feedback clamp** to `[0, 0.95]` prevents infinite gain even under
  misuse (e.g. `feedback(2.0)` is silently bounded).

### Sample Playback

Load a WAV file once on the main thread, play it back through one or
many `Sampler` voices. Audio data is shared via `Arc<[f32]>` — cloning
a `Sample` is a refcount bump, not a copy.

```rust
use nyx_prelude::*;

// From a file (requires `wav` feature, enabled by default)
let kick = Sample::load("kick.wav")?;

// Or from an in-memory buffer
let buf = render_to_buffer(&mut inst::kick(), 0.4, 44100.0);
let kick = Sample::from_buffer(buf, 44100.0)?;

// Build a voice
play(Sampler::new(kick).pitch(1.5)).unwrap();
```

**`Sample` API:**

| Method | Description |
| --- | --- |
| `Sample::load(path)` | Load a WAV (16/24/32-bit int or 32-bit float). Stereo → mono downmix on load. Requires `wav` feature. |
| `Sample::from_buffer(data, sr)` | Build from an in-memory mono f32 `Vec`. Always available. |
| `.len()` / `.is_empty()` / `.duration_secs()` / `.sample_rate()` | Metadata |

**`Sampler` API:**

| Method | Description |
| --- | --- |
| `Sampler::new(sample)` | One-shot voice at native pitch |
| `.pitch(rate: impl IntoParam)` | `1.0` = native, `2.0` = octave up. Accepts `f32` or `Signal` |
| `.loop_all()` | Loop the whole sample |
| `.loop_region(start_secs, end_secs)` | Loop a sub-region |
| `.ping_pong()` | Bounce back and forth at loop bounds |
| `.trigger()` | Reset position to start (for retriggering one-shots) |
| `.is_finished()` | `true` when a one-shot has played out |
| `.position()` | Current fractional sample index |

**Sample-rate mismatch:** if the sample's native rate (say 44.1 kHz)
differs from the stream rate (say 48 kHz), `Sampler` automatically
scales the playback rate so `pitch(1.0)` always sounds at the sample's
native pitch. No explicit resampling step is needed.

**Lifetime caveat:** keep at least one `Sample` reference alive for as
long as any `Sampler` cloned from it is playing. Otherwise the last
`Arc::drop` may occur on the audio thread, triggering the allocator.
(A "sample graveyard" that ships `Arc`s back to the main thread for
drop is planned for Sprint 2.)

### Granular Synthesis

`Granular` reads a `Sample` through many short, Hann-windowed "grains"
whose position, pitch, pan, and amplitude each jitter independently.
A fixed voice pool (default 64) is pre-allocated once; the scheduler
spawns new grains at the configured density and the callback never
allocates.

```rust
use nyx_prelude::*;

let pad = Sample::load("pad.wav")?;
let cloud = Granular::new(pad)
    .grain_size(0.08)        // 80 ms grains
    .density(40.0)           // 40 grains / sec (≈ 3.2 concurrent)
    .position(0.4)           // read around 40 % into the source
    .position_jitter(0.15)   // ±15 % of the sample length
    .pitch_jitter(0.025)     // ±2.5 % pitch wobble
    .pan_spread(1.0);        // full stereo field
play(cloud).unwrap();
```

| Method | Default | Notes |
| --- | --- | --- |
| `Granular::new(sample)` | — | 64-voice pool |
| `Granular::with_voices(sample, n)` | — | Explicit pool size; caps concurrent overlap |
| `.grain_size(secs)` | `0.05` | 5 ms–200 ms typical |
| `.density(hz)` | `30.0` | Grains / sec. `0.0` = silence |
| `.position(frac)` | `0.5` | `0.0..=1.0` of sample length |
| `.position_jitter(frac)` | `0.05` | Fraction of sample length — bipolar spread |
| `.pitch(rate)` | `1.0` | `2.0` = octave up, `0.5` = octave down |
| `.pitch_jitter(frac)` | `0.0` | `0.02` = ±2 % (gentle chorus); `0.5` = dramatic |
| `.pan_spread(amount)` | `0.5` | `0.0` = mono, `1.0` = hard-left to hard-right |
| `.amp(gain)` / `.amp_jitter(amount)` | `0.8` / `0.0` | Per-grain level |
| `.seed(u32)` | internal | Reproducible grain patterns |

**Stereo & mono.** `next_stereo` renders the real stereo field; `next`
sums L+R (energy-preserving with linear pan). `Granular` composes with
the rest of the fluent API — `granular.freeverb().compress(...)` is the
natural way to add a tail and tame peaks.

**Voice stealing.** When density × grain_size exceeds the voice count,
new grains are *dropped* rather than cutting off active ones (so grains
never click out mid-envelope). Raise the pool with
`Granular::with_voices` if you need more overlap, or lower the density.

See [`granular_cloud.rs`](../nyx-prelude/examples/granular_cloud.rs) for
a full drone example that synthesises a Cm7 pad, granulates it, then
reverb-glues the result.

### Karplus-Strong (Plucked String)

The canonical DSP teaching example — a noise burst circulating through
a delay line with a gentle one-pole lowpass in the feedback path. Delay
length sets the pitch; the lowpass progressively loses high frequencies
each pass, giving the characteristic plucked-string decay shape.

```rust
// A 440 Hz plucked string with long sustain
play(pluck(440.0, 0.996)).unwrap();

// Stack for a chord
let chord = pluck(Note::C4.to_freq(), 0.996)
    .add(pluck(Note::E4.to_freq(), 0.996))
    .add(pluck(Note::G4.to_freq(), 0.996))
    .amp(0.3);
```

| Parameter | Range | Effect |
| --- | --- | --- |
| `freq` | Hz, clamped ≥ 20 Hz | Pitch of the plucked note |
| `decay` | `[0, 1]` | Feedback gain. `0.99` = long sustain, `0.9` = short, `0` = instant silence |

**Single-shot behaviour:** each `Pluck` strikes once at first `.next()`
call (when we know the real sample rate) and rings out from there.
Dropping the `Pluck` ends the note. For repeated strikes, build a fresh
`Pluck` per note-on event.

**Why a dedicated function?** `pluck()` could be built by hand from
`osc::noise::white() + .delay() + .lowpass()`, but:

- The `sample_rate / freq` → delay-length conversion is a trap for
  newcomers
- It's the first example in every DSP textbook; its absence is surprising
- `play(pluck(440.0, 0.99))` is a one-line library demo

### Lo-fi / Glitch

Bitcrusher and sample-rate reducer — the two halves of classic lo-fi
character. Both are stateless (`BitCrush` holds nothing; `Downsample`
holds one latched sample + a counter), so they're trivially real-time
safe.

```rust
// Quantise to 4-bit depth for 80s sampler grit
signal.bitcrush(4)

// Sample-and-hold: each input held for 4 output samples (quarter rate)
signal.downsample(0.25)

// Both at once
signal.crush(6, 0.5)  // 6-bit, half rate
```

| Method | Parameter | Effect |
| --- | --- | --- |
| `.bitcrush(bits: u32)` | `bits` in `[1, 24]`, clamped | `1` = harsh square-ish, `4–8` = classic crush, `16+` = transparent |
| `.downsample(ratio: f32)` | `ratio` in `(0, 1]`, clamped | `1.0` = identity, `0.5` = each sample held twice, `0.25` = held four times |
| `.crush(bits, ratio)` | convenience | shorthand for `.bitcrush(bits).downsample(ratio)` |

**Character notes:**

- At low bit depths, even DC (0.0) input doesn't quantise to 0.0 — there
  isn't an output level at zero. That's expected; it's part of the grit.
- Upstream oscillators continue running at the full sample rate (their
  phase accumulators advance per frame). `.downsample()` is a
  sample-and-hold on the emitted value, not true decimation — the
  resulting aliasing is what gives the effect its crunch.

### Saturation (Tape / Tube / Diode)

Three named waveshapers, each voiced for a classic analog flavour.
Use these instead of the generic `.soft_clip(drive)` when you want a
specific character rather than a generic `tanh`:

```rust
use nyx_core::SaturationExt;

let warm  = osc::saw_bl(220.0).amp(0.6).tape_sat(2.0);    // tape machine
let vocal = osc::saw_bl(220.0).amp(0.6).tube_sat(3.0);    // 2nd-harmonic tube
let fuzz  = osc::saw_bl(220.0).amp(0.6).diode_clip(8.0);  // transistor pedal
```

| Method | Signal chain |
| --- | --- |
| `.tape_sat(drive)` | 30 Hz HP → asymmetric `tanh` (bias = 0.1) → 12 kHz LP → `1/√drive` gain comp |
| `.tube_sat(drive)` | `tanh(drive·x)` pre-limit → `y − k·y²` (even harmonics) → `y − y³/3` → DC-blocking HP → 15 kHz LP |
| `.diode_clip(drive)` | Algebraic soft-clip `y = x·drive / (1 + |x·drive|)`. No filtering. |

`drive` typically sits in `[1, 10]`: 1 is near-transparent, 2 is
audible colour, 6+ is heavy. Tape's asymmetry is what gives it the
characteristic sub-audible warmth; tube's even-harmonic emphasis
reads as "vocal" or "musical"; diode's sharper knee gives the
hard-edged fuzz-pedal character.

### Tape Machine (wow + flutter + EQ + saturation)

Run anything through a tape deck. Combines pitch modulation via a
modulated delay line (slow wow LFO + filtered-noise flutter), tape EQ
(30 Hz HP + 12 kHz LP), and asymmetric soft-clip. A single `.age()`
knob scales wow depth, flutter depth, and drive together from `0.0`
(pristine) to `1.0` (battered).

```rust
use nyx_core::TapeExt;

let cassette = osc::saw_bl(220.0).amp(0.5).tape().age(0.6);
let pristine = osc::sine(440.0).tape().age(0.1);
let destroyed = osc::square_bl(110.0).amp(0.4).tape().age(1.0);

// Override individual parameters:
osc::saw_bl(220.0).tape().wow(0.7, 0.0012).flutter(5.0, 0.0004).drive(2.5)
```

| Builder | Effect |
| --- | --- |
| `.age(amount)` | Master "how battered" knob — sweeps wow/flutter/drive together |
| `.wow(rate_hz, depth_frac)` | Sine LFO pitch wobble (default 0.5 Hz) |
| `.flutter(rate_hz, depth_frac)` | Filtered-noise pitch flutter (default 6 Hz) |
| `.drive(amount)` | Saturation drive (default `1.0 + 2.0 * age`) |

Depth is a fraction of sample rate, so pitch wobble is SR-independent.

### Analog Drift

Slow random wander around **1.0**, intended to be multiplied into an
oscillator's frequency parameter to simulate the pitch instability of
analog VCOs.

```rust
use nyx_core::drift;

// Saw around 440 Hz wobbling by ±4 cents at 0.3 Hz update rate.
let freq = drift(4.0, 0.3).amp(440.0);
let osc = osc::saw_bl(freq);

// Deterministic for tests / hot-reload:
let d = drift(4.0, 0.3).seed(42);
```

| Argument | Meaning |
| --- | --- |
| `amount_cents` | Half-range of the wander in cents (±amount) |
| `rate_hz` | How often a new random target is picked |
| `.seed(u32)` | Fixed PRNG seed for reproducible drift |

Output is `2^(cents/1200)`, centred at 1.0. Drop `.amp(base_freq)` on
top to convert to a modulated frequency signal.

### Lo-fi Preset Wrappers

One-liner aesthetic presets composing the primitives above. Each
returns `impl Signal`, so chain as you would any other combinator:

```rust
use nyx_core::LofiExt;

let drums   = osc::saw_bl(110.0).amp(0.5).cassette();     // tape + crush + hiss
let beat    = osc::saw_bl(220.0).amp(0.5).lofi_hiphop();  // aged tape + LP + hiss
let haunted = osc::sine(440.0).vhs();                     // heavy wow + aggressive HF loss
```

| Method | Composition |
| --- | --- |
| `.cassette()` | `.tape()` (age 0.5) + pink hiss at 1.5% + 10-bit crush |
| `.lofi_hiphop()` | `.tape().age(0.7).lowpass(4000 Hz)` + 1.2% pink hiss |
| `.vhs()` | `.tape().age(1.0).lowpass(2500 Hz)` — aggressive HF loss |

Pink-hiss seeds are fixed per preset so results are reproducible.

### Vinyl Crackle & Hiss

Completes the "old medium" aesthetic — sparse clicks through a 2 kHz
resonator for dusty-vinyl ambience, plus pink noise at a chosen dB
level for a tape/vinyl noise floor.

```rust
use nyx_core::{vinyl, SignalExt};

let ambience = osc::saw_bl(220.0).amp(0.4)
    .add(vinyl::crackle(0.35))   // several clicks per second at max
    .add(vinyl::hiss(-58.0));    // pink noise floor at −58 dBFS
```

| Function | Purpose |
| --- | --- |
| `vinyl::crackle(intensity)` | Random impulses through a 2 kHz ring; `intensity ∈ [0, 1]`. Output stays near zero except at click events. |
| `vinyl::hiss(level_db)` | Pink noise scaled to the given dBFS level. Typical: −70 (subtle) to −40 (dusty). |

`vinyl::crackle` also has `.seed(u32)` for reproducible patterns.

---

## Clock & Timing

The clock converts sample ticks into musical time (beats, bars):

```rust
use nyx_seq::clock;

let mut clk = clock::clock(120.0);           // 120 BPM
let mut clk = clock::clock(120.0).beats_per_bar(3.0);  // 3/4 time
let mut clk = clock::clock(bpm_signal);      // modulatable tempo
```

In the audio callback:

```rust
let state = clk.tick(&ctx);
// state.beat          — total beats elapsed (e.g. 4.5)
// state.bar           — total bars elapsed
// state.phase_in_beat — fractional position in current beat [0, 1)
// state.phase_in_bar  — fractional position in current bar [0, 1)
```

### Quantisation

Snap a beat position to a grid:

```rust
use nyx_seq::Clock;

Clock::snap(1.3, 1.0)    // → 1.0 (quarter note grid)
Clock::snap(1.3, 0.25)   // → 1.25 (sixteenth note grid)
```

---

## Envelopes

Trigger-based ADSR envelope generator:

```rust
use nyx_seq::envelope;

let mut env = envelope::adsr(
    0.01,  // attack (seconds)
    0.1,   // decay (seconds)
    0.7,   // sustain (level, 0–1)
    0.3,   // release (seconds)
);

env.trigger();   // note-on → starts attack
env.release();   // note-off → starts release
```

The envelope implements `Signal` and outputs values in [0, 1]:

```rust
// Apply envelope to an oscillator
let voice = osc::sine(440.0).mul(env);
```

Query the current stage:

```rust
env.stage()     // Stage::Idle | Attack | Decay | Sustain | Release
env.is_idle()   // true when envelope has finished
```

---

## Automation

Time-based parameter curves without allocating breakpoint arrays:

```rust
use nyx_seq::automation;

// Standalone automation signal: linear ramp 0→1 over 2 seconds
let ramp = automation::automation(|t| (t / 2.0).min(1.0));

// Multiply a signal by an automation curve
let fading_sine = osc::sine(440.0).follow(|t| (t / 2.0).min(1.0));
```

The closure receives elapsed time in seconds.

---

## Music Theory

### Notes

```rust
use nyx_seq::Note;

Note::A4                     // MIDI 69
Note::C4                     // MIDI 60
Note::from_midi(60)          // C4
Note::parse("C#4")           // Some(Note(61))
Note::parse("Bb2")           // Some(Note(46))
Note::A4.to_freq()           // 440.0
Note::from_freq(442.0)       // A4 (nearest)

Note::C4.transpose(7)        // G4
Note::C4.up_octave()         // C5
Note::C4.down_octave()       // C3
Note::A4.pitch_class()       // 9
Note::A4.octave()            // 4
```

### Scales

```rust
use nyx_seq::{Scale, ScaleMode};

let scale = Scale::major("C");
let scale = Scale::minor("A");
let scale = Scale::pentatonic("C");
let scale = Scale::new("D", ScaleMode::Dorian);
```

Available modes: Major, Minor, PentatonicMajor, PentatonicMinor, Dorian,
Phrygian, Lydian, Mixolydian, Locrian, WholeTone, Chromatic.

#### Scale Snapping

```rust
scale.snap(61.0)            // C#4 → nearest note in C major (C4 or D4)
scale.snap_freq(445.0)      // → 440.0 (A4, nearest in scale)
scale.notes_in_range(Note::C4, Note::B4)  // all scale notes in range
```

### Chords

```rust
use nyx_seq::{Chord, ChordType, Note};

let chord = Chord::major(Note::C4);        // C E G
let chord = Chord::minor(Note::A4);        // A C E
let chord = Chord::dom7(Note::G4);         // G B D F
let chord = Chord::new(Note::C4, ChordType::Sus4);

chord.notes()       // Vec<Note>
chord.freqs()       // Vec<f32>
chord.transpose(7)  // transpose whole chord
```

Available types: Major, Minor, Diminished, Augmented, Major7, Minor7,
Dominant7, Sus2, Sus4.

---

## Patterns & Sequencing

### Pattern

A generic sequence of values with combinators:

```rust
use nyx_seq::Pattern;

let p = Pattern::new(&[1, 2, 3, 4]);

p.step(0)              // &1
p.step(5)              // &2 (wraps cyclically)
p.reverse()            // [4, 3, 2, 1]
p.retrograde()         // same as reverse
p.rotate(1)            // [4, 1, 2, 3] (right by 1)
p.rotate(-1)           // [2, 3, 4, 1] (left by 1)
p.concat(&other)       // join end-to-end
p.interleave(&other)   // A[0], B[0], A[1], B[1], ...
```

Note patterns support `.invert()`:

```rust
let notes = Pattern::new(&[Note::C4, Note::E4, Note::G4]);
notes.invert()  // mirrors pitches around first note
```

### Step Sequencer

Driven by the musical clock:

```rust
use nyx_seq::{Pattern, Sequence, clock};

let pattern = Pattern::new(&[true, false, true, false]);
let mut seq = Sequence::new(pattern, 0.25);  // sixteenth note grid

// In the audio callback:
let clock_state = clk.tick(&ctx);
let event = seq.tick(&clock_state);
if event.triggered && event.value {
    kick.trigger();
}
```

Works with any type — bools for triggers, `Note` for melodies, `f32` for
parameter automation.

#### Probability & Conditional Modifiers

TidalCycles-style modifiers layer on top of any `Sequence<T>`. All use
a seeded PRNG, so `.seed(n)` makes output reproducible across runs.

```rust
let seq = Sequence::new(pattern, 0.25)
    .prob(0.75)                         // 75% of hits fire
    .seed(42)
    .every(4, |p| p.reverse())          // every 4 cycles, reverse
    .sometimes(0.3, |p| p.rotate(2));   // 30% of cycles, rotate by 2
```

| Method | Behaviour |
| --- | --- |
| `.prob(p: f32)` | Each step has `p` probability of firing. `1.0` = all fire, `0.0` = silent. |
| `.degrade(amount: f32)` | TidalCycles alias: `.degrade(0.25)` == `.prob(0.75)`. |
| `.every(n, \|p\| ...)` | Every `n`-th cycle, use the transformed pattern. Returns to base after. |
| `.sometimes(p, \|p\| ...)` | Per-cycle coin flip at probability `p` picks between base and transformed. |
| `.seed(seed: u64)` | Set the PRNG seed. Same seed → identical output. |

Pattern transformations usable inside `.every()` / `.sometimes()`:

- `p.reverse()` — reverse the step order
- `p.rotate(n: i32)` — rotate right by `n` steps (negative = left)
- `p.shuffle(seed: u64)` — Fisher-Yates random permutation
- `p.retrograde()` — alias for `.reverse()`

Only the most recent `.every()` or `.sometimes()` call is active; for
composition, combine inside a single closure:

```rust
seq.sometimes(0.5, |p| p.rotate(2).reverse())
```

---

## Euclidean Rhythms

Distribute N hits as evenly as possible across M steps:

```rust
use nyx_seq::Euclid;

Euclid::generate(3, 8)   // tresillo: [x . . x . . x .]
Euclid::generate(5, 8)   // cinquillo
Euclid::generate(4, 16)  // four-on-the-floor

// Rotate for variations
Euclid::generate(3, 8).rotate(1)  // shifted tresillo
```

Returns a `Pattern<bool>` — composable with all pattern combinators.

---

## Randomness

Portable seeded PRNG (xorshift64) — deterministic across platforms:

```rust
use nyx_seq::{seeded, Scale, Note};

let mut rng = seeded(42);

rng.next_f32()                    // [0, 1)
rng.next_f32_range(0.5, 2.0)     // [0.5, 2.0)
rng.next_range(1, 6)             // 1..=6 (inclusive)
rng.choose(&[440.0, 880.0])      // pick random element
rng.next_note(Note::C4, Note::C5)  // random MIDI note in range

// Scale-aware random notes
let scale = Scale::minor("A");
rng.next_note_in(&scale, Note::A3, Note::A5)  // always in A minor
```

---

## Instruments

Pre-built instruments from `nyx-core` primitives. All implement `Signal`.

### Percussion & one-shots (`nyx_seq::inst`)

```rust
use nyx_seq::inst;

let mut kick = inst::kick();
kick.trigger();                    // fire the kick

let mut snare = inst::snare();
snare.trigger();

let mut hat = inst::hihat(false);  // false = closed, true = open
hat.trigger();

let drone = inst::drone(Note::A2); // continuous detuned saws

let riser = inst::riser(4.0);      // 4-second noise riser

let mut pad = inst::pad(Chord::major(Note::C4));
pad.trigger();
pad.release();
```

### Preset Voices (`nyx_seq::presets`)

Named synth recipes with opinionated defaults — "pick a voice, play
a note" instruments. Each wraps several Nyx primitives into a
one-call instrument that sounds recognisable without configuration.
All expose a `set_freq(freq)` for pitch changes; most include a
`.trigger()` / `.release()` envelope.

```rust
use nyx_prelude::*;

// Acid bass — squelchy saw through a resonant ladder filter.
let mut bass = presets::tb303(55.0);
bass.trigger();

// Trance lead — 7 detuned band-limited saws; wrap with your own envelope.
let mut lead = presets::supersaw(440.0);
let mut env = envelope::adsr(0.05, 0.3, 0.7, 0.4);
env.trigger();
let sample = lead.next(ctx) * env.next(ctx);

// Tuned handpan — 4-partial modal synthesis.
let mut pan = presets::handpan(261.63);
pan.trigger();
```

| Preset | Technique | Trigger? | Character |
| --- | --- | --- | --- |
| `presets::tb303(freq)` | Saw → envelope-swept ladder (res 0.75) → amp env | Yes | Acid bass squelch |
| `presets::moog_bass(freq)` | Saw + square → ladder at fixed cutoff | Yes | Fat analog bass (`.cutoff(hz)` to tune) |
| `presets::supersaw(freq)` | 7 band-limited saws, ±6/±12/±18 c detune | No (continuous) | Trance lead — gate with external env |
| `presets::prophet_pad(freq)` | 2 detuned saws + sub → soft LP → slow swell | Yes | Warm OB-Xa-style pad |
| `presets::dx7_bell(freq)` | FM (1:1.4 ratio, index envelope) | Yes | FM mallet / bell |
| `presets::juno_pad(freq)` | 2 PWM voices with counter-phased LFO → LP → swell | Yes | Juno-60 chorus pad |
| `presets::handpan(freq)` | 4 damped sines at 1, 2, 3.01, 5.03 × fundamental | Yes | Tuned steel-drum "thonk" |
| `presets::chime(freq)` | 4 damped sines at 0.5, 1.19, 2, 3 × (longer τ) | Yes | Tubular-bell ring |
| `presets::noise_sweep(secs)` | White noise → bandpass sweep 200 Hz → 4 kHz | Yes | Build-up riser |

`handpan` and `chime` use **modal synthesis** (sum of exponentially-
damped sines at inharmonic ratios) rather than FM — the partial
spectrum is what gives tuned metal its ring. Both use recursive
decay (one mul per partial per sample, not one `exp()`) for
efficiency.

> **Naming note.** `dx7_bell` is FM-based; `chime` is modal. Both
> produce bell-like sounds but via different algorithms, hence
> different names.

See `nyx-prelude/examples/presets_demo.rs` for a runnable tour of
every preset over 20 seconds.

---

## SubSynth & Patches

Configurable subtractive synthesizer: oscillator -> filter -> ADSR -> gain.

```rust
use nyx_seq::{SynthPatch, OscShape, FilterType};

// Build from defaults
let mut synth = SynthPatch::default().build();
synth.trigger();

// Custom patch
let patch = SynthPatch {
    name: "BassStab".to_string(),
    osc_shape: OscShape::Saw,
    frequency: 55.0,
    filter_type: FilterType::LowPass,
    filter_cutoff: 800.0,
    filter_q: 2.0,
    attack: 0.001,
    decay: 0.1,
    sustain: 0.3,
    release: 0.05,
    gain: 0.8,
};
let mut synth = patch.build();
synth.trigger();
synth.set_frequency(110.0);  // change pitch
synth.release();
```

### Save / Load Patches

```rust
patch.save("bass_stab.toml").unwrap();
let loaded = SynthPatch::load("bass_stab.toml").unwrap();
```

Patches serialise to human-readable TOML. Note: only `SynthPatch` is
serialisable — `dyn Signal` cannot be serialised.

---

## Visual Mirror

### Oscilloscope (Scope)

Tap a signal to read waveform samples on another thread:

```rust
use nyx_core::ScopeExt;

let (signal, mut handle) = osc::sine(440.0).scope(4096);
// Pass `signal` to the engine, read from `handle`:
let mut buf = vec![0.0f32; 1024];
let n = handle.read(&mut buf);       // non-blocking, no alloc
handle.available()                    // samples ready to read
```

### Spectrum

Tap a signal for FFT analysis:

```rust
use nyx_core::{SpectrumExt, SpectrumConfig, WindowFn};

let config = SpectrumConfig {
    frame_size: 2048,           // must be power of 2
    window: WindowFn::Hann,     // or WindowFn::Blackman
};
let (signal, handle) = osc::sine(440.0).spectrum(config);

// Read bins (freq, magnitude pairs):
let bins = handle.snapshot();       // Vec<FreqBin>
handle.bin_count()                  // number of bins
```

### Inspect

Per-sample callback without a ring buffer:

```rust
use nyx_core::InspectExt;

let signal = osc::sine(440.0).inspect(|sample, ctx| {
    // Runs on the audio thread — don't allocate or block!
});
```

### Pitch Detection (YIN)

Tap a signal for fundamental-frequency estimation using the YIN
algorithm (de Cheveigné & Kawahara, 2002):

```rust
use nyx_prelude::*;

let (signal, pitch) = mic().pitch(PitchConfig::default());
let _engine = play_async(signal).unwrap();

loop {
    let (f, clarity) = pitch.read();
    println!("{f:>7.1} Hz  (clarity {clarity:.2})");
    std::thread::sleep(std::time::Duration::from_millis(100));
}
```

| Field | Default | Notes |
| --- | --- | --- |
| `frame_size` | `2048` | Analysis window. Larger = better low-frequency resolution, more CPU, more latency. |
| `hop_size` | `1024` | Samples between analyses. 50% overlap is typical. |
| `threshold` | `0.15` | CMNDF dip threshold. Lower = stricter. Range `0.10`–`0.20`. |
| `min_freq` | `40.0` | Hz. Caps `max_τ`. |
| `max_freq` | `4000.0` | Hz. Caps `min_τ`. |

**Handle API.**

| Method | Returns |
| --- | --- |
| `handle.freq()` | Latest fundamental in Hz (`0.0` if no pitch found) |
| `handle.confidence()` | Clarity score `0.0`–`1.0` (higher = cleaner periodic signal) |
| `handle.read()` | `(freq, confidence)` tuple |
| `handle.clone()` | Cheap Arc-clone — read from any thread |

**Real-time cost.** Inner loop is `O(frame_size × max_τ)`. With the
default config at 44.1 kHz, that's ~2.2 M multiply-adds per hop
(≈ every 23 ms). Analysis runs on the audio thread — match
[`spectrum`](#spectrum)'s convention — with pre-allocated work buffers
so the callback does not allocate. `.pitch()` is a passive tap; the
underlying samples pass through unchanged.

See [`pitch_tune.rs`](../nyx-prelude/examples/pitch_tune.rs) for a
self-contained demo sweeping an oscillator while the tracker prints
the detected frequency in real time.

---

## MIDI Input

Requires the `midi` feature: `nyx-core = { features = ["midi"] }`

### Open a MIDI port

```rust
use nyx_core::midi;

let (mut receiver, _connection) = midi::open_midi_input().unwrap();
// or by name:
let (mut receiver, _conn) = midi::open_midi_input_named(Some("Arturia")).unwrap();
```

### Process events on the audio thread

```rust
for event in receiver.drain() {
    match event {
        MidiEvent::NoteOn { note, velocity, .. } => { /* trigger voice */ }
        MidiEvent::NoteOff { note, .. } => { /* release voice */ }
        MidiEvent::ControlChange { cc, value, .. } => { /* update param */ }
    }
}
```

### CC Mapping with Smoothing

```rust
let cc_map = CcMap::new();
let writer = cc_map.writer();  // give to MIDI callback

// In the MIDI callback:
writer.set(1, value);  // CC1 = mod wheel

// As a Signal (smoothed, no zipper noise):
let cutoff = cc_map.signal(1, 5.0);  // CC1, 5ms smoothing
osc::saw(220.0).lowpass(cutoff, 0.707)
```

---

## OSC Input

Requires the `osc` feature: `nyx-core = { features = ["osc"] }`

```rust
use nyx_core::osc_input::{OscParam, osc_listen};

let cutoff = OscParam::new(1000.0);
let mappings = vec![
    ("/cutoff".to_string(), cutoff.writer()),
];
let _listener = osc_listen("0.0.0.0:9000", mappings).unwrap();

// Use as a Signal:
let cutoff_sig = cutoff.signal(5.0);  // 5ms smoothing
osc::saw(220.0).lowpass(cutoff_sig, 0.707)
```

---

## Microphone Input

Requires the `audio` feature (enabled by default).

```rust
use nyx_core::mic;

let (mic_signal, _handle) = mic::mic().unwrap();
// mic_signal implements Signal — process it like any other:
let processed = mic_signal.lowpass(2000.0, 0.707).amp(0.5);
```

---

## GUI Widgets

The `nyx-iced` crate provides iced canvas widgets with the Nyx Midnight Theme.

### Controls

```rust
use nyx_iced::*;

// Rotary knob
let state = KnobState::new(0.5);
let widget = Knob::new(&state).size(80.0).view();
// Emits KnobMessage::Changed(f32) on drag

// Horizontal slider
let state = SliderState::new(0.5);
let widget = HSlider::new(&state).width(200.0).view();
// Emits SliderMessage::Changed(f32) on drag

// Vertical slider
let widget = VSlider::new(&state).height(150.0).view();

// XY Pad (2D control)
let state = XYPadState::new(0.5, 0.5);
let widget = XYPad::new(&state).size(150.0).view();
// Emits XYPadMessage::Changed { x, y } on drag
```

### Visualisers

```rust
// Oscilloscope
let mut scope_view = OscilloscopeCanvas::new(1024).width(400.0).height(200.0);
scope_view.update(&mut scope_handle);  // call in your update()
scope_view.view()                       // call in your view()

// Spectrum analyser
let mut spec_view = SpectrumCanvas::new(64).width(400.0).height(200.0);
spec_view.update(&spectrum_handle);
spec_view.view()
```

### Connecting Controls to Audio

Use `OscParam` to bridge GUI values to the audio thread:

```rust
let gain_param = OscParam::new(0.5);

// In audio signal chain:
let gain_sig = gain_param.signal(5.0);
let output = osc::sine(440.0).amp(gain_sig);

// In GUI update():
gain_param.writer().set(new_value);
```

### Theme Colours

```rust
use nyx_iced::NyxColors;

NyxColors::BG_DARK       // deep background
NyxColors::BG_SURFACE    // panel background
NyxColors::ACCENT        // neon cyan
NyxColors::WARM          // orange (peaks, warnings)
NyxColors::WAVEFORM      // oscilloscope line colour
```

---

## Hot Reload

The `nyx-cli` binary watches a sketch file, recompiles on save, and crossfades
to the new signal chain.

### Writing a Sketch

A sketch is a single `.rs` file that exports a `nyx_sketch()` function. Use the
prelude for the shortest possible imports:

```rust
// examples/sketches/my_sketch.rs
use nyx_prelude::*;

#[unsafe(no_mangle)]
pub fn nyx_sketch() -> Box<dyn Signal> {
    osc::sine(440.0).amp(0.3).boxed()
}
```

A richer example with drums, sequencing, and time-based automation:

```rust
use nyx_prelude::*;

#[unsafe(no_mangle)]
pub fn nyx_sketch() -> Box<dyn Signal> {
    let mut clk = clock::clock(140.0);
    let mut kick = inst::kick();

    let cutoff = automation::automation(|t| 200.0 + 800.0 * (t * 0.5).sin().abs());
    let bass = osc::saw(55.0).lowpass(cutoff, 2.0).soft_clip(1.5);

    let kick_pat = Euclid::generate(4, 16);
    let mut seq = Sequence::new(kick_pat, 0.25);

    let mut bass = bass;
    (move |ctx: &AudioContext| {
        let state = clk.tick(ctx);
        let event = seq.tick(&state);
        if event.triggered && event.value { kick.trigger(); }
        bass.next(ctx) * 0.4 + kick.next(ctx)
    }).boxed()
}
```

Sketches may freely use any type from `nyx-core` or `nyx-seq` — the prelude
re-exports everything commonly needed.

### Running

```bash
cargo run -p nyx-cli -- my_sketch.rs
```

Edit the file while it's playing — changes are heard within seconds. The old
signal fades out and the new one fades in (configurable crossfade).

### CLI Options

```
nyx <sketch.rs> [OPTIONS]

Options:
  --watch             Watch for changes (default: true)
  --crossfade-ms N    Crossfade duration in ms (default: 50)
  --sample-rate N     Sample rate in Hz (default: 44100)
  --buffer-size N     Buffer size in samples (default: 512)
```

---

## Cookbook Examples

Short, runnable examples living in [`nyx-prelude/examples/`](../nyx-prelude/examples/).
Each uses `use nyx_prelude::*;` and calls `play()` — the point is to show
how little code it takes to get a real musical result.

| Example | What it does | Run |
|---|---|---|
| [`dubstep_wobble.rs`](../nyx-prelude/examples/dubstep_wobble.rs) | LFO-modulated filter cutoff on a detuned saw bass with soft clipping | `cargo run -p nyx-prelude --example dubstep_wobble --release` |
| [`wind.rs`](../nyx-prelude/examples/wind.rs) | Pink noise lowpassed and shaped by a slow gain LFO | `cargo run -p nyx-prelude --example wind --release` |
| [`generative_melody.rs`](../nyx-prelude/examples/generative_melody.rs) | Euclidean rhythm triggers seeded-random notes from A pentatonic through a `SubSynth` | `cargo run -p nyx-prelude --example generative_melody --release` |
| [`midi_filter.rs`](../nyx-prelude/examples/midi_filter.rs) | MIDI CC1 sweeps filter cutoff from 100 Hz to 8 kHz (exponential, 5 ms smoothed) | `cargo run -p nyx-prelude --example midi_filter --features midi --release` |
| [`wav_export.rs`](../nyx-prelude/examples/wav_export.rs) | Renders a 10-second filtered saw to `track.wav` (16-bit mono) | `cargo run -p nyx-prelude --example wav_export --release` |
| [`lofi.rs`](../nyx-prelude/examples/lofi.rs) | Filtered saw crushed to 6-bit / quarter-rate for 80s sampler grit | `cargo run -p nyx-prelude --example lofi --release` |
| [`echo.rs`](../nyx-prelude/examples/echo.rs) | Walking pluck bassline through a 750 ms delay with 50% feedback | `cargo run -p nyx-prelude --example echo --release` |
| [`pluck.rs`](../nyx-prelude/examples/pluck.rs) | Four-voice Karplus-Strong Cm7 chord that rings out | `cargo run -p nyx-prelude --example pluck --release` |
| [`sampler.rs`](../nyx-prelude/examples/sampler.rs) | Synthesised kick rendered into a buffer, retriggered on beats at shifting pitches | `cargo run -p nyx-prelude --example sampler --release` |
| [`conditional.rs`](../nyx-prelude/examples/conditional.rs) | `.degrade()` kick on every beat + `.every(4, reverse)` on Euclidean hi-hats | `cargo run -p nyx-prelude --example conditional --release` |
| [`svf_sweep.rs`](../nyx-prelude/examples/svf_sweep.rs) | Pink noise through a narrow SVF band-pass whose cutoff sweeps 200 Hz → 8 kHz | `cargo run -p nyx-prelude --example svf_sweep --release` |
| [`fm_bell.rs`](../nyx-prelude/examples/fm_bell.rs) | DX7-style FM bell with 1:2 modulator ratio and decaying modulation index, playing random C pentatonic-minor notes | `cargo run -p nyx-prelude --example fm_bell --release` |
| [`wavetable.rs`](../nyx-prelude/examples/wavetable.rs) | Custom "supersaw-lite" wavetable + sine sub through a slow-wobble SVF lowpass | `cargo run -p nyx-prelude --example wavetable --release` |
| [`stereo_sweep.rs`](../nyx-prelude/examples/stereo_sweep.rs) | Panning saw bass (LFO-swept L↔R) plus Haas-widened pluck chord — demonstrates the stereo engine | `cargo run -p nyx-prelude --example stereo_sweep --release` |
| [`reverb.rs`](../nyx-prelude/examples/reverb.rs) | 5-voice Cm7 pad with slow LFO swell through a big Freeverb room | `cargo run -p nyx-prelude --example reverb --release` |
| [`trance.rs`](../nyx-prelude/examples/trance.rs) | 90-second trance track at 138 BPM — full production: kick/snare/hats, 16th bass, arp, supersaw lead, reverb pad, riser — across 6 sections (intro/build/drop/breakdown/final build/final drop) | `cargo run -p nyx-prelude --example trance --release` |
| [`chorus_flanger.rs`](../nyx-prelude/examples/chorus_flanger.rs) | Chorused A-minor triad pad over a heavy-flanged bass — both effects output real stereo | `cargo run -p nyx-prelude --example chorus_flanger --release` |
| [`sidechain_pump.rs`](../nyx-prelude/examples/sidechain_pump.rs) | 128 BPM four-on-the-floor kick ducks a sub-bass via sidechain compression — the classic trance pumping groove | `cargo run -p nyx-prelude --example sidechain_pump --release` |
| [`multi_bus.rs`](../nyx-prelude/examples/multi_bus.rs) | Drum bus + harmony bus + bass → master bus, showing grouped compression, shared reverb, and soft-clip on the master | `cargo run -p nyx-prelude --example multi_bus --release` |
| [`pitch_tune.rs`](../nyx-prelude/examples/pitch_tune.rs) | YIN pitch tracker printing detected frequency + clarity while a sine sweeps across three octaves | `cargo run -p nyx-prelude --example pitch_tune --release` |
| [`granular_cloud.rs`](../nyx-prelude/examples/granular_cloud.rs) | 3-second Cm7 pad stretched into an evolving drone via 64-voice granular synthesis + Freeverb tail | `cargo run -p nyx-prelude --example granular_cloud --release` |

**Why release mode?** Debug builds of cpal + DSP are ~20× slower than
release. Always use `--release` for anything that produces audio.

---

## Nannou & Bevy Visualisers

Heavier examples that integrate with external creative-coding frameworks
live in a dedicated [`nyx-examples`](../nyx-examples/) crate. This crate is
**excluded from the default workspace build** so `cargo build`, `cargo
test`, and `cargo clippy` on the main workspace stay fast.

| Example | Framework | What it does |
|---|---|---|
| [`nannou_scope.rs`](../nyx-examples/examples/nannou_scope.rs) | Nannou | Live oscilloscope with a sliding-window waveform buffer |
| [`bevy_spectrum.rs`](../nyx-examples/examples/bevy_spectrum.rs) | Bevy | 64-bar FFT spectrum driven by a frequency sweep, rendered as ECS sprite entities |

### Running

```bash
cargo run -p nyx-examples --example nannou_scope --release
cargo run -p nyx-examples --example bevy_spectrum --release
```

First build of `nyx-examples` takes ~40 seconds (compiles Nannou + Bevy
trees). Subsequent rebuilds are ~1–2 seconds.

### How the bug-free sliding scope works

The `ScopeHandle::read()` method only fills as many samples as are
available. At ~60 fps with 44.1 kHz audio, only ~735 samples arrive between
frames, but a 2048-sample display buffer needs the rest to be *previous*
samples, not zeros. The Nannou example demonstrates the correct pattern:

```rust
let n = scope.read(&mut scratch);
let buf_len = buffer.len();
if n >= buf_len {
    buffer.copy_from_slice(&scratch[n - buf_len..n]);
} else {
    buffer.rotate_left(n);
    buffer[buf_len - n..].copy_from_slice(&scratch[..n]);
}
```

Use this pattern in your own scope-consuming code.

### Proportional bar grouping for spectra

When mapping `N` FFT bins to `M` display bars and `N` isn't divisible by
`M`, naive integer division drops bins. Use proportional integer math to
distribute the remainder evenly:

```rust
let start = (bar_index * total_bins) / BAR_COUNT;
let end   = ((bar_index + 1) * total_bins) / BAR_COUNT;
```

Both visualisers use this pattern so the highest frequencies are always
represented.

---

## Testing Utilities

### Offline Rendering

Render a signal to a `Vec<f32>` without audio hardware:

```rust
use nyx_core::render_to_buffer;

let mut sig = osc::sine(440.0);
let samples = render_to_buffer(&mut sig, 1.0, 44100.0);
// Returns Vec<f32> with 44100 samples
```

### WAV Export

Render a signal directly to a `.wav` file on disk. Gated behind the
`wav` feature (enabled by default).

```rust
use nyx_prelude::*;

let signal = osc::saw(110.0).lowpass(800.0, 0.707).amp(0.3);
render_to_wav(signal, 10.0, 44100.0, "track.wav")?;
```

| Function | Format | Use when |
| --- | --- | --- |
| `render_to_wav(signal, secs, sr, path)` | 16-bit signed PCM mono | Default choice — universal, matches DAW expectations, half the file size |
| `render_to_wav_f32(signal, secs, sr, path)` | 32-bit float mono | Lossless preservation, values outside [-1, 1] are kept as-is |

Both variants take the signal **by value** (consumed). The 16-bit
version hard-clamps samples to `[-1, 1]` before quantising. Both reject
non-positive durations and sample rates with `WavError::InvalidDuration`
/ `WavError::InvalidSampleRate`.

**Main thread only.** Never call from the audio callback — it allocates,
blocks on I/O, and can take many seconds. Use it for offline composition
rendering.

### Golden File Tests

Compare signal output against stored binary blobs:

```rust
use nyx_core::golden::{assert_golden, GoldenTest};

assert_golden(&mut signal, &GoldenTest {
    name: "my_test",
    duration_secs: 0.01,
    sample_rate: 44100.0,
    tolerance: 1e-6,
});
```

Set `NYX_UPDATE_GOLDEN=1` to regenerate golden files.

### No-Alloc Guard

Prove a signal doesn't allocate in the audio callback:

```rust
#[global_allocator]
static ALLOC: nyx_core::GuardedAllocator = nyx_core::GuardedAllocator;

#[test]
fn my_signal_is_alloc_free() {
    let mut sig = osc::sine(440.0).lowpass(800.0, 0.707);
    let _guard = nyx_core::DenyAllocGuard::new();
    for _ in 0..1024 {
        sig.next(&ctx);  // panics if anything allocates
    }
}
```

### Widget Interaction Tests

The iced canvas widgets each include inline unit tests that drive
`canvas::Program::update()` with synthetic mouse events and assert on
state changes. Pattern:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use iced::widget::canvas::Program;

    fn cursor_at(x: f32, y: f32) -> mouse::Cursor {
        mouse::Cursor::Available(Point::new(x, y))
    }

    fn press() -> Event {
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
    }

    #[test]
    fn click_updates_value() {
        let canvas = HSliderCanvas { value: 0.0 };
        let mut state = SliderInteraction::default();
        let bounds = Rectangle { x: 0.0, y: 0.0, width: 200.0, height: 24.0 };
        let (_, msg) = canvas.update(&mut state, press(), bounds, cursor_at(100.0, 12.0));
        assert!(matches!(msg, Some(SliderMessage::Changed(v)) if (v - 0.5).abs() < 0.02));
    }
}
```

All four widgets (Knob, HSlider, VSlider, XYPad) ship with this coverage
— 25 tests total across [knob.rs](../nyx-iced/src/knob.rs),
[slider.rs](../nyx-iced/src/slider.rs), and
[xypad.rs](../nyx-iced/src/xypad.rs).

### Running the Test Suite

```bash
# Full native test suite — excludes nyx-examples (nannou/bevy heavy deps)
cargo test

# Just widget tests
cargo test -p nyx-iced --lib

# Specific phase
cargo test -p nyx-core --test phase3

# With clippy (default members only — stays fast)
cargo clippy -- -D warnings
```

**Current status:** 254 tests passing across `nyx-core`, `nyx-seq`,
`nyx-iced`, and `nyx-prelude`.

---

## Real-Time Safety

These rules are enforced, not advisory:

1. **No heap allocation** after the audio stream starts (`Box`, `Vec`, `String`).
   Enforced by `GuardedAllocator` in tests.
2. **No locks** in the audio callback (`Mutex`, `RwLock`). Use atomics or SPSC
   ring buffers.
3. **No I/O** in the audio callback (file reads, syscalls, `println!`).
4. **Coefficient smoothing** on all filter parameters to prevent clicks.

Communication between threads uses:
- `rtrb` SPSC ring buffers for event streams (MIDI, commands)
- `AtomicU8` / `AtomicU32` for CC values and OSC params
- One-pole smoothing on all atomic reads to prevent zipper noise

---

## API Reference

### Crate Overview

| Crate | Purpose | In default workspace |
| --- | --- | --- |
| `nyx-core` | Signal engine, oscillators, filters, dynamics, saturation, tape, drift, vinyl, scope, spectrum, MIDI, OSC, mic, hotswap | Yes |
| `nyx-seq` | Clock, envelopes, automation, notes, scales, chords, patterns, sequencer, instruments, presets, SubSynth | Yes |
| `nyx-iced` | Iced GUI widgets (knob, sliders, XY pad, oscilloscope, spectrum) | Yes |
| `nyx-prelude` | Convenience re-exports, reusable demo tracks, cookbook examples | Yes |
| `nyx-cli` | Hot-reload sketch player | Yes |
| `nyx-examples` | Nannou & Bevy visualisers (heavy deps isolated here) | No — excluded from `default-members` |
| `nyx-wasm-demo` | Browser demo — pad / Tron cue / preset keyboard via wasm-bindgen + Web Audio | No — built with `wasm-pack` |

The root `Cargo.toml` lists `nyx-examples` as a workspace member but
excludes it from `default-members`, so `cargo build`, `cargo test`, and
`cargo clippy` run fast on the core crates without compiling Nannou or
Bevy. Target `nyx-examples` explicitly (`cargo run -p nyx-examples
--example <name>`) to build those.

### Feature Flags (nyx-core)

| Feature | Default | Enables |
|---|---|---|
| `audio` | Yes | cpal engine, `play()`, `mic()` |
| `midi` | No | midir MIDI input, `open_midi_input()` |
| `osc` | No | rosc OSC input, `osc_listen()` |

### Extension Traits

| Trait | Crate | Methods Added |
| --- | --- | --- |
| `SignalExt` | nyx-core | `boxed`, `amp`, `add`, `mul`, `mix`, `pan`, `clip`, `soft_clip`, `offset`, `bitcrush`, `downsample`, `crush`, plus delay/chorus/flanger/freeverb/compress/sidechain wrappers |
| `FilterExt` | nyx-core | `lowpass`, `highpass`, `svf_lp`, `svf_hp`, `svf_bp`, `svf_notch` |
| `LadderExt` | nyx-core | `ladder_lp` (Moog-style 4-pole non-linear lowpass) |
| `SaturationExt` | nyx-core | `tape_sat`, `tube_sat`, `diode_clip` |
| `TapeExt` | nyx-core | `tape` (wow + flutter + EQ + saturation wrapper) |
| `LofiExt` | nyx-core | `cassette`, `lofi_hiphop`, `vhs` (preset chains) |
| `ScopeExt` | nyx-core | `scope` |
| `SpectrumExt` | nyx-core | `spectrum` |
| `InspectExt` | nyx-core | `inspect` |
| `AutomationExt` | nyx-seq | `follow` |

### Dependencies

| Crate | Version | Purpose |
|---|---|---|
| `cpal` | 0.17 | Cross-platform audio I/O |
| `rtrb` | 0.3 | Lock-free SPSC ring buffer |
| `spectrum-analyzer` | 1.6+ | FFT magnitude bins |
| `midir` | 0.10 | MIDI input |
| `rosc` | 0.10 | OSC input |
| `serde` + `toml` | 1.0 / 0.8 | Patch serialisation |
| `iced` | 0.13 | GUI framework |
| `notify` | 7 | File watching |
| `libloading` | 0.8 | Dynamic library loading |
| `nannou` | 0.19 | Creative-coding visualiser (nyx-examples) |
| `bevy` | 0.14 | ECS game engine visualiser (nyx-examples) |

---

## Roadmap

All 11 original phases of the project are complete. Remaining work is
tracked in [CLAUDE.md](../CLAUDE.md) (source of truth for the project
status) and two longer planning docs:

- **[CLAUDE.md — Post-Phase-11 Backlog](../CLAUDE.md)** — the master
  status board. Completed items are checked off; remaining small-polish
  work (e.g. CI pipeline, more sketches) lives here.
- **[docs/roadmap-deferred.md](roadmap-deferred.md)** — detailed phased
  plans for the two large deferred features:
  - **A. DAW Bridge** (JACK/PipeWire integration, phases A0–A5)
  - **B. WASM Target** (browser support, phases B0–B6)

Both documents include explicit deliverables, test strategies, API
sketches, and a recommended order of attack. Start there when resuming
work on either feature.

### Recently Completed

- [x] License files (MIT + Apache 2.0) with `license.workspace = true`
      on all crates
- [x] Cookbook examples (runnable sketches in `nyx-prelude/examples/`)
- [x] Widget interaction tests (25 tests across knob/slider/xypad)
- [x] Nannou oscilloscope + Bevy spectrum visualisers in `nyx-examples/`
- [x] Expanded `nyx-prelude` to re-export all nyx-seq modules for
      one-line imports in sketches
- [x] Detailed deferred roadmap document
- [x] **Sonic character roadmap** (all 8 items, see
      [docs/roadmap-sonic-character.md](roadmap-sonic-character.md)):
      PolyBLEP saw/square + PWM, tape/tube/diode saturation, Moog-style
      ladder filter, tape emulator with wow/flutter, Paul Kellett pink
      noise, analog drift, lofi preset wrappers, vinyl crackle + hiss
- [x] **Preset voices** (`nyx_seq::presets`): tb303, moog_bass,
      supersaw, prophet_pad, dx7_bell, juno_pad, handpan, chime,
      noise_sweep — 9 named synth recipes, each a one-call instrument
- [x] **Reusable demo tracks** (`nyx_prelude::demos`): tron() and
      tron_2() — 90-second electro-orchestral cues shared between the
      WAV renderer and the WASM browser demo
- [x] **Interactive WASM demo** with preset keyboard — pick a voice
      from the dropdown, hit a chromatic key, hear it via Web Audio.
      Deployed to GitHub Pages via `deploy-demo.yml`.

---

## License

Nyx is dual-licensed under:

- [MIT License](../LICENSE-MIT)
- [Apache License 2.0](../LICENSE-APACHE)

You may use this software under the terms of **either** license, at your
option. All crates in the workspace inherit `license = "MIT OR Apache-2.0"`
from the root `Cargo.toml`.

When contributing, you agree that your contributions will be dual-licensed
under the same terms.
