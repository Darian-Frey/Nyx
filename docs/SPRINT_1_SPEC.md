# Sprint 1 — v1.1 Spec

Status: **Spec / Not Started**
Target duration: ~1.5 weeks FTE
Scope: Roadmap items #1–#5 plus Karplus-Strong freebie

Ordering within the sprint, chosen for momentum and risk management:

1. **WAV export** — half-day, removes a category of user frustration immediately
2. **Bitcrusher + downsampler** — half-day, stateless palate cleanser, confidence win
3. **Delay line + feedback** — foundation for everything in Sprint 2, gets the hardest design decision out of the way early
4. **Karplus-Strong `.pluck()`** — half-day freebie on top of delay
5. **Sampler** — largest item, benefits from having delay patterns already settled
6. **Probability / conditional steps** — pure additions to existing `Sequence<T>` / `Pattern<T>`

---

## 1. WAV Export

### 1.1 Public API

```rust
// In nyx-core, behind feature = "wav" (default on)
pub fn render_to_wav<S: Signal>(
    signal: S,
    duration_secs: f32,
    sample_rate: f32,
    path: impl AsRef<Path>,
) -> Result<(), WavError>;

#[derive(Debug, thiserror::Error)]
pub enum WavError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("hound error: {0}")]
    Hound(#[from] hound::Error),
    #[error("invalid duration: {0}")]
    InvalidDuration(f32),
    #[error("invalid sample rate: {0}")]
    InvalidSampleRate(f32),
}
```

### 1.2 Implementation notes

- Wraps existing `render_to_buffer`; this is ~20 lines of hound scaffolding.
- WAV format: 16-bit signed PCM, mono, configurable sample rate. 16-bit is the common ground; 24-bit and float deferred to a `WavFormat` enum in v1.2 if users ask.
- Clamp `signal.next()` to `[-1.0, 1.0]` before quantising. Signals can legally exceed `[-1, 1]`; WAV cannot.
- Main-thread only. Never called from the audio callback — this should be obvious but worth putting in doc comments.

### 1.3 Open questions

1. **Default output format: 16-bit int or 32-bit float?** 16-bit is universal, smaller files, and matches what DAWs expect on import. Float preserves exact signal. Recommend 16-bit default, `render_to_wav_f32` as a sibling function for the nerds.
2. **Should there be a stereo version?** Pending the Sprint 2 stereo decision. For now, mono only.

### 1.4 Tests

- Round-trip: render a 1 kHz sine, read the file back, confirm peak frequency via FFT matches input ±1 Hz.
- Clipping: render a signal with known peaks > 1.0, confirm output is clamped not wrapped.
- Duration: 60-second render produces exactly `60 * sample_rate` samples.

---

## 2. Bitcrusher + Downsampler

### 2.1 Public API

```rust
pub trait SignalExt: Signal + Sized {
    fn bitcrush(self, bits: u32) -> BitCrush<Self>;
    fn downsample(self, ratio: f32) -> Downsample<Self>;
    fn crush(self, bits: u32, ratio: f32) -> Downsample<BitCrush<Self>> {
        self.bitcrush(bits).downsample(ratio)
    }
}

pub struct BitCrush<S> { /* ... */ }
pub struct Downsample<S> { /* held sample, counter */ }

impl<S: Signal> Signal for BitCrush<S> { /* ... */ }
impl<S: Signal> Signal for Downsample<S> { /* ... */ }
```

### 2.2 Implementation notes

- **BitCrush:** `bits` is a fixed `u32` at construction. `levels = (1 << bits) as f32 - 1.0`. Output: `(input * 0.5 + 0.5) * levels).round() / levels * 2.0 - 1.0`. No modulation in v1; adding `impl Param` later is non-breaking if we add a sibling `bitcrush_mod` method.
- **Downsample:** `ratio` is 0.0–1.0, where 1.0 = no reduction, 0.5 = half rate (hold each sample for 2 frames), 0.25 = quarter rate. Internal state: `held: f32`, `phase: f32`. Increment phase by `ratio` per sample; when it crosses 1.0, latch the input and subtract 1.0.
- Both are stateless in the "no DSP memory" sense — Downsample holds one sample, BitCrush holds nothing.

### 2.3 Open questions

1. **`bits` as `u32` vs. `f32`.** Fractional bit depths (7.3 bits) are a legitimate aesthetic effect — you get more subtle crunch. Cost: extra math per sample. Recommend `u32` for v1, revisit if users ask.
2. **Downsample: fixed ratio or modulatable?** Modulating `ratio` is where the effect gets interesting (pitch-like artefacts). Requires `Param<S>`. Recommend: ship fixed-ratio v1, add modulatable variant in Sprint 3.

### 2.4 Tests

- Bitcrush with `bits=1` produces only two output values. Confirm.
- Bitcrush with `bits=16` produces output within ~1 LSB of input for a ramp signal.
- Downsample with `ratio=0.5` outputs each input sample exactly twice.
- Downsample with `ratio=1.0` is identity.

---

## 3. Delay Line + Feedback

This is the foundation for chorus, flanger, reverb, ping-pong, Karplus-Strong. Getting the API right here pays back across the whole roadmap. **Resolve the open questions before implementation.**

### 3.1 Public API — recommended

```rust
// Trait extension — one entry point
pub trait SignalExt: Signal + Sized {
    fn delay(self, time_secs: f32) -> Delay<Self>;
}

// All modifiers as inherent methods on Delay<S>, returning Delay<S>
pub struct Delay<S> {
    input: S,
    buffer: Box<[f32]>,
    write_idx: usize,
    time_param: Param<f32>,      // seconds, smoothed
    feedback_param: Param<f32>,  // 0.0–0.95, smoothed
    mix_param: Param<f32>,       // 0.0–1.0, smoothed
    max_samples: usize,
}

impl<S: Signal> Delay<S> {
    pub fn max_time(mut self, secs: f32) -> Self;  // reallocates, must be called before play()
    pub fn time(mut self, secs: impl IntoParam<f32>) -> Self;
    pub fn feedback(mut self, amount: impl IntoParam<f32>) -> Self;
    pub fn mix(mut self, wet: impl IntoParam<f32>) -> Self;
}

impl<S: Signal> Signal for Delay<S> { /* ... */ }
```

Usage:

```rust
osc::saw(220.0)
    .delay(0.375)
    .feedback(0.4)
    .mix(0.3)
```

### 3.2 Why `Box<[f32]>` over const generic `N`

The roadmap proposed `Delay<S, const N: usize>`. Rejecting this:

- **Const N locks max delay at compile time.** 2-second delay at 48 kHz needs `N = 96000` hardcoded. Changing sample rate at runtime is common (44.1 vs. 48); hardcoding is fragile.
- **Const N pollutes downstream types.** `Delay<S, 96000>` vs. `Delay<S, 44100>` are different types. Combinators that produce delays need their own const generics. Viral.
- **`Box<[f32]>` allocates once in `.delay()`,** which is called on the main thread before `play()`. Allocator guard isn't violated. This is how `fundsp`, `Tone.js`, and every DAW plugin handle it.

Default `max_time` = `time` value at construction, with a `.max_time()` builder method for users who plan to modulate longer. Document clearly: modulating `time` beyond `max_time` silently clamps.

### 3.3 Feedback safety

Hard clamp `feedback` to `[0.0, 0.95]` internally. Values ≥ 1.0 produce infinite gain. Silent clamping with a `#[debug_assertions]` log when user exceeds the range is friendlier than a panic.

### 3.4 Read interpolation

Linear interpolation in v1. Hermite (4-point cubic) deferred to v1.2 — linear is indistinguishable from Hermite at 44.1 kHz for typical delay modulation rates (< 10 Hz). Hermite matters for chorus/flanger with high-rate modulation; revisit then.

### 3.5 Open questions

1. **Ping-pong.** Stereo-dependent, defers to Sprint 2. Flag in docs: `osc::saw(220).delay(0.375).pingpong()` is a Sprint 2 addition.
2. **Sync-to-tempo.** `.delay_beats(Beats::new(3, 16))` feels natural given Nyx's musical API. Recommend: ship seconds-only in v1.1, add beat-synced variant once the whole sprint lands and we see how delay composes with the existing clock.

### 3.6 Tests

- Impulse response: feed a single 1.0 sample, confirm echoes at `time_secs * sample_rate` with amplitude `feedback^n * mix`.
- Feedback clamp: set `feedback = 2.0`, confirm output stays bounded.
- Time modulation: sweep `time` with a slow LFO, confirm no zipper noise (RMS of output derivative bounded).
- Zero feedback, zero mix: output == input exactly.

---

## 4. Karplus-Strong (Freebie)

### 4.1 Public API

```rust
pub fn pluck(freq: f32, decay: f32) -> Pluck;

pub struct Pluck { /* noise burst state + delay line + one-pole lowpass */ }
impl Signal for Pluck { /* ... */ }
```

### 4.2 Implementation notes

- Delay length = `sample_rate / freq` samples.
- On construction, fill delay buffer with white noise scaled to ±1.0.
- Each sample: read delay, apply one-pole lowpass (avg of current and previous read), write back, output.
- `decay` ∈ `[0.0, 1.0]` scales the feedback coefficient (0.99 = long sustain, 0.9 = short).
- Self-triggering: plays once then decays to silence. Re-triggering requires a separate `.trigger()` method — defer to v1.2 or expose a `PluckVoice` inside a `VoicePool`.

### 4.3 Why it earns its own combinator

`Pluck` is technically "noise burst → delay → feedback → lowpass," which a user could wire up. But:

- It's the canonical DSP teaching example. Its absence from the palette is surprising.
- The frequency-to-delay-samples conversion is non-obvious for newcomers.
- One-line demos sell the library: `play(pluck(440.0, 0.98))`.

### 4.4 Tests

- Dominant frequency of output matches `freq` ±2 Hz (FFT).
- Output decays to < 0.01 RMS within `(duration implied by decay)` — document the formula.

---

## 5. Sampler

Largest Sprint 1 item. Gets its own section treatment.

### 5.1 Public API

```rust
pub struct Sample {
    data: Arc<[f32]>,
    sample_rate: f32,
}

impl Sample {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, SampleError>;
    pub fn from_buffer(data: Vec<f32>, sample_rate: f32) -> Self;
}

pub struct Sampler {
    sample: Sample,
    rate: Param<f32>,              // playback rate, 1.0 = native
    loop_region: Option<(f32, f32)>, // (start_secs, end_secs)
    mode: SamplerMode,
    position: f64,                 // fractional sample index
    finished: bool,
}

#[derive(Copy, Clone)]
pub enum SamplerMode {
    OneShot,
    Loop,
    PingPong,
}

impl Sampler {
    pub fn new(sample: Sample) -> Self;
    pub fn pitch(self, rate: impl IntoParam<f32>) -> Self;
    pub fn loop_all(self) -> Self;
    pub fn loop_region(self, start_secs: f32, end_secs: f32) -> Self;
    pub fn ping_pong(self) -> Self;
    pub fn trigger(&mut self);  // resets position to start, for VoicePool use
}

impl Signal for Sampler { /* ... */ }
```

Usage:

```rust
let kick = Sample::load("kick.wav")?;
play(Sampler::new(kick).pitch(1.5))?;
```

### 5.2 The Arc-drop problem

The roadmap specifies `Arc<[f32]>` for sample data. Correct choice, but:

- `Arc::clone` is RT-safe (atomic refcount bump).
- `Arc::drop` on the *last* reference calls the allocator. If the audio thread ever holds the last reference, dropping happens on the audio thread → allocator guard fires.
- Typical case: main thread holds a `Sample`, clones an `Arc` into a `Sampler`, then the `Sample` goes out of scope. Main thread's reference is gone. When the audio thread eventually drops its `Sampler`, it holds the last `Arc` and deallocates.

**Fix:** when a `Sampler` is dropped on the audio thread, send its `Arc` back to the main thread via an SPSC channel for main-thread drop.

```rust
// in nyx-core, initialised when the stream starts
static SAMPLE_GRAVEYARD: OnceCell<Producer<Arc<[f32]>>> = OnceCell::new();

impl Drop for Sampler {
    fn drop(&mut self) {
        if let Some(prod) = SAMPLE_GRAVEYARD.get() {
            // Try to send. If the channel is full, we fall back to dropping
            // here — not ideal, but bounded by graveyard capacity (default 64).
            let _ = prod.push(self.sample.data.clone());
        }
    }
}
```

Or equivalently, the `Sampler`'s `Drop` clones the `Arc` into the graveyard before its own `Arc` drops, keeping the refcount > 0 on the audio thread. Main thread polls the graveyard and drops the clones.

This pattern should generalise for any future `Arc`-holding signal. Consider a `nyx-core` internal `GraveyardSender<T>` helper for Sprint 2 reverb and sampler.

### 5.3 Interpolation

Linear for v1.1. Hermite deferred. Linear aliasing when pitching up is noticeable but acceptable; documented trade-off.

### 5.4 Loop modes

- `OneShot`: play once, `finished = true`, output silence afterward.
- `Loop`: wrap position at loop end.
- `PingPong`: reverse playback direction at loop bounds. Requires signed rate internally.

### 5.5 Open questions

1. **WAV load: sync or async?** Main-thread sync is simplest. Async loading (for large samples) is a v1.2 concern.
2. **Sample-rate mismatch.** If a 44.1 kHz sample plays at 48 kHz stream rate, do we resample at load time or adjust playback rate? Recommend: adjust rate at playback (no quality loss from pre-emptive resampling), document that native pitch maps to `rate = stream_sr / sample_sr`.
3. **Multi-channel WAVs.** Mono only in v1.1 — downmix stereo WAVs to mono at load time with a warning. Stereo sampler is a Sprint 2 concern.

### 5.6 Tests

- Load a known WAV, confirm sample count and rate.
- Playback at `rate = 1.0` on matched sample rate reproduces the file bit-exactly.
- Playback at `rate = 2.0` halves the duration.
- Loop mode wraps correctly across `loop_region` boundaries.
- Allocator guard: load sample, push into pool, trigger 100 times, drop pool → no alloc on audio thread.

---

## 6. Probability & Conditional Steps

Pure additions to the existing sequencing API. No refactor, no new types, small surface area.

### 6.1 Public API

```rust
impl<T: Clone> Sequence<T> {
    pub fn prob(self, probability: f32) -> Self;
    pub fn every(self, n: u32, transform: impl Fn(Self) -> Self + Send + 'static) -> Self;
    pub fn sometimes(self, probability: f32, transform: impl Fn(Self) -> Self + Send + 'static) -> Self;
    pub fn degrade(self, amount: f32) -> Self;  // alias for .prob(1.0 - amount), TidalCycles style
}

impl<T: Clone> Pattern<T> {
    pub fn reverse(self) -> Self;
    pub fn rotate(self, steps: i32) -> Self;
    pub fn shuffle(self, seed: u64) -> Self;
}
```

### 6.2 Implementation notes

- **`prob`:** each step has a `probability: f32`. At trigger time, sample PRNG; if > probability, skip the step.
- **`every(n, f)`:** counter wraps at `n`; on bar 0, apply `f` to the next bar's pattern. Requires holding a transformed clone. Memory cost: `sizeof(Sequence<T>)` per `every` call. Acceptable for typical use.
- **`sometimes(p, f)`:** per-bar dice roll instead of counter.
- **PRNG:** use a deterministic seeded RNG (seeded per-sequence) so that patterns are reproducible across runs. Live-diff users will thank you.

### 6.3 Open questions

1. **`every` closure signature.** `Fn(Self) -> Self` requires the closure to return a full `Sequence<T>`. Ergonomic for `.every(4, |s| s.reverse())`, less ergonomic for partial modifications. Consider a mutation variant `.every_mut(n, |s: &mut Self| ...)` if users end up fighting the API.
2. **Composability.** Should `.prob(0.5).every(4, |s| s.prob(1.0))` work? The nested closure holds a transformed sequence whose `prob` is different from the outer. Recommend: yes, this should work; the transform closure receives the sequence as-is and can modify whatever.

### 6.4 Tests

- `.prob(0.0)` produces silence.
- `.prob(1.0)` is identity.
- `.prob(0.5)` produces roughly half the triggers over 1000 bars (tolerance ±10%).
- `.every(4, |s| s.reverse())` on a known pattern produces forward, forward, forward, reverse, forward, forward, forward, reverse.
- Deterministic seed: same seed → identical output across runs.

---

## 7. Sprint-Wide Checklist

Before merging any item:

- [ ] Public API signatures locked and documented
- [ ] Concrete return types (no `Box<dyn>` except via explicit `.boxed()`)
- [ ] Audio-callback path passes `GuardedAllocator` test
- [ ] `IntoParam<f32>` accepted wherever a parameter could plausibly be modulated
- [ ] At least one ≤ 20-line cookbook example per item
- [ ] Manual / README updated
- [ ] Deterministic items have golden-file regression tests
- [ ] No new `unsafe` without justification comment

---

## 8. Cross-Cutting Open Questions

Resolve these once, they shape everything in the sprint:

1. **`SignalExt` trait placement.** Most combinators are added as extension methods. Is there one canonical `SignalExt` trait, or are combinators grouped (`DelayExt`, `SamplerExt`)? Recommend: one `SignalExt` with all Sprint 1 methods, split later if it becomes unwieldy (> 30 methods).
2. **`IntoParam<T>` trait.** Does `0.5_f32` auto-convert into `Param<f32>`? If not, every example becomes `.feedback(Param::fixed(0.4))` which kills the fluency goal. Recommend: yes, blanket impl `IntoParam<f32> for f32` and `for Signal`.
3. **Graveyard infrastructure for `Arc` drops.** The Sampler needs this. Reverb will need it (large internal buffers). Factor it into `nyx-core` as a first-class helper in this sprint, not ad-hoc per effect.
4. **Feature flags.** `wav` feature for hound dep is clear. Should the sampler be behind `sampler` feature (adds hound too)? Recommend: `wav` feature gates both WAV export and WAV loading; sampler's in-memory `from_buffer` constructor works without the feature.

---

## 9. Out of Scope for v1.1

Explicit no's to prevent creep:

- Stereo anything (Sprint 2 decision)
- Hermite / sinc interpolation (v1.2)
- Beat-synced delay times (v1.2)
- Bit-depth modulation (v1.2)
- Sample time-stretching (v2 — granular territory)
- Async sample loading (v1.2)
- Modulatable downsample ratio (Sprint 3)
- Re-triggering `Pluck` without a pool (v1.2)
- Multi-channel sample playback (Sprint 2)

---

## 10. Success Criteria

Sprint 1 is done when:

- All six items ship with locked APIs and tests
- A new user can `cargo add nyx-prelude`, write a 15-line beat-making sketch using kick/snare samples + delay + bitcrush + probability, and have it sound musical
- Allocator guard passes for all Sprint 1 combinators in arbitrary chains
- `manual.md` has a "v1.1 Cookbook" section with runnable examples for each item
- No regressions in existing benchmarks
- At least one example sketch committed to `examples/v1_1/` per item
