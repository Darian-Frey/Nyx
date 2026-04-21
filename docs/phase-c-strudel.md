# Research: Mini-Notation & Cycles DSL (Out of scope for Nyx)

> **Status: not a Nyx roadmap item.** This document was originally
> drafted as a Phase C plan for Nyx, but the project decided to keep
> Nyx focused on its "p5.js of sound" mission — a fluent Rust DSP
> library — and spin any Strudel-alike live-coding environment off as
> a **separate project**. The notes below are preserved as research in
> case that separate project is ever built; Nyx could serve as its
> audio engine, or the separate project could use WebAudio/Strudel's
> existing stack directly.

**Original scope (kept for reference):** Strudel/TidalCycles-style
pattern DSL on top of Nyx's DSP kernel. Mini-notation string parser,
cycle-based time model, pattern algebra (polymeter, polyrhythm, scale
binding).

---

## 1. Goal

Strudel (https://strudel.cc) is the JavaScript port of TidalCycles —
the dominant live-coding language for algorithmic music. Its defining
features:

- **Mini-notation strings**: `"bd ~ sn ~"`, `"c [e g]"`, `"a*4"`
- **Cycle-based time**: everything is fractions of a repeating cycle,
  not BPM × seconds
- **Functional pattern algebra**: `.fast(2)`, `.rev()`, `.every(4, rev)`,
  `.jux()`, `.ply()`, `.degrade()`
- **Polymeters / polyrhythms** built in
- **Scale DSL**: `n("0 2 4").scale("C:minor")`

Phase C's goal: produce a Rust equivalent that uses Nyx's DSP engine
underneath, giving Rust users a TidalCycles-like interface and pairing
with Phase B (WASM) to become a real browser alternative to Strudel.

---

## 2. Current state vs Strudel

| Concept | Strudel | Nyx today |
|---|---|---|
| Pattern type | `Pattern<T>` = `(TimeSpan) -> [Event<T>]` (function over time) | `Pattern<T>` = `Vec<T>` (fixed step array) |
| Time model | Cycles (dimensionless), CPS (cycles per second) | Beats (BPM-based), `ClockState` |
| Sequencing | Mini-notation string + combinators | `Sequence::new(pattern, grid)` + `.every/.prob/.sometimes` |
| Polymeters | Built in via `"[3,5]"` | Not supported |
| Polyrhythms | Built in via `"{a b, c d e}"` | Not supported |
| Live coding | String edit → instant hot-swap | Rust file edit → cdylib recompile (~1–3 s) |
| Scale binding | `n("0 2").scale("C:minor")` | `Scale::snap(f32)` (not pattern-aware) |
| Samples | `s("bd").bank("RolandTR909")` | `Sample::load("kick.wav")` + `Sampler` |
| FX routing | `s("bd").room(0.5).delay(0.25)` | `.freeverb()`, `.delay()` combinators |

Nyx already has about **60% of the primitives** — Sprint 1 explicitly
built `.prob()/.degrade()/.every()/.sometimes()` on `Sequence` to match
Tidal idioms. What's missing is:

1. The string DSL parser
2. Function-based pattern abstraction (for polymeters / time-warping)
3. Cycle-based clock mode
4. Scale-degree binding
5. A live-coding UX that's actually fast (<100 ms text→sound)

---

## 3. Design decisions to resolve before coding

These shape the whole phase. Resolve each before starting implementation.

### 3.1 Cycle model — replace or parallel?

Strudel's `Pattern<T>` is fundamentally a **function from time-spans to
events**, not a sequence of discrete steps. This enables time-warping,
stretching, polymeters, and arbitrary subdivision.

Our `Pattern<T>` is a `Vec<T>`. Simple, but it can't represent
`"a [b c] d"` without decomposing into a finer grid.

**Recommendation:** keep `Pattern<T>` as-is (stable, already shipped,
covers 80% of use cases) and add a **new `CyclePattern<T>` type** for
the function-based model. Users pick:

- `Pattern<T>` for straightforward N-step patterns (existing ergonomics)
- `CyclePattern<T>` for mini-notation output (polymeters, nested time)

The mini-notation parser produces `CyclePattern<T>`. `Sequence<T>` gets
a sibling `CycleSequence<T>` that ticks a `CyclePattern` against a
cycle-based clock.

### 3.2 Clock: beats or cycles?

Strudel's natural time unit is the cycle; ours is the beat. One cycle
typically corresponds to one bar (4 beats at 4/4), but Strudel doesn't
enforce a "bar" concept — `cps(0.5)` just means "0.5 cycles per second."

**Recommendation:** add a **`CycleClock`** type alongside `Clock`. It
exposes the same `ClockState`-ish struct but with `cycle: f32`,
`phase_in_cycle: f32` fields. Both clocks can be driven by the same
sample tick; users pick which mental model they want.

Conversion: `cycles = beats / beats_per_cycle` where `beats_per_cycle`
defaults to 4 (one bar at 4/4).

### 3.3 Parser — new crate or module?

**Recommendation:** new workspace crate `nyx-mini`, depending only on
`nyx-core` (for `Pattern`/`CyclePattern`). Pure parser, no DSP, no
runtime.

Why a separate crate:
- Parser has different tradeoffs (allocation at parse time is fine)
- Keeps `nyx-core` focused on DSP
- Allows WASM users to ship just the parser without `cpal`
- Easier to iterate on syntax without bumping core

### 3.4 Mini-notation subset for v1

The full Tidal mini-notation is rich but has edge cases. Start with a
useful, well-defined subset:

**v1 tokens** (must have):

- Whitespace-separated events: `"a b c d"`
- Rests: `"~"`
- Nested groups: `"a [b c] d"` → a, (b,c at half time), d
- Repetition: `"a*3 b"` → a a a b
- Slow-down: `"a/2 b"` → a over 2 cycles, b in 1 cycle
- Alternation (one per cycle): `"<a b c>"`
- Polymeter: `"[a b c, d e]"` → two parallel patterns aligned to one cycle

**v1.1 tokens** (nice to have):

- Degrade: `"a b c?"` — c has 50% probability per cycle
- Elongate: `"a b!"` — b takes twice as long as its neighbors

**Out of scope for v1** (add later or never):

- Euclidean mini-notation `"a(3,8)"` (we have `Euclid::generate`)
- Arbitrary math in values `"a:{0+1}"`
- Swing / humanize — keep as combinators, not mini-notation

### 3.5 Live coding UX

Two paths:

**Path A — compile-time mini-notation.** Strings are parsed when the
sketch file compiles; hot reload still goes through cdylib. Same
latency as today (~1–3 s), same API.

**Path B — runtime mini-notation.** A sketch holds a mutable string;
editing the string re-parses the pattern without recompiling. Latency
~10 ms. Needs an edit loop (file watcher + parser) but no compiler.

**Recommendation:** ship Path A first. It's strictly additive on top of
existing infrastructure. Add Path B once the parser stabilizes — it's
basically "watch `*.nyx` files, parse on save, send new Pattern over
SPSC to the audio thread." That's a small additional module on
`nyx-cli`.

### 3.6 Cross-cutting compatibility

`CyclePattern` and the existing `Pattern`/`Sequence` APIs must coexist
cleanly:

- `CyclePattern::to_steps(divisions: usize) -> Pattern<T>` — coarse
  conversion for users who want to feed a cycle pattern into an
  existing `Sequence`.
- `Pattern::to_cycle(&self) -> CyclePattern<T>` — trivial lift (N
  equally-spaced events per cycle).

This lets users gradually adopt the new system without rewrites.

---

## 4. Phased implementation

### Phase C0 — Research & architecture (2–3 days)

- [ ] Read Strudel's `@strudel/core` source to understand their
      `Pattern<T>` internals (how `Hap<T>` events are represented,
      time spans, queries)
- [ ] Decide: keep our `Pattern<T>` and add `CyclePattern<T>` in a new
      module, or refactor `Pattern<T>` itself? (Recommendation above.)
- [ ] Nail down the cycle clock API — does it replace `Clock` in
      cycle-based sketches, or coexist? (Coexist, per recommendation.)
- [ ] Lock the v1 mini-notation token set.
- [ ] Decide on crate layout: `nyx-mini` new crate, with a `parser`
      module and a `render` module.

**Deliverable:** a short design doc resolving all §3 questions.

### Phase C1 — `CyclePattern<T>` type (3–5 days)

- [ ] `nyx-core/src/cycle_pattern.rs`:
  - `pub struct CyclePattern<T: Clone>` — internally a
    `Vec<Hap<T>>` where `Hap` = `{ whole: TimeSpan, part: TimeSpan,
    value: T }` (Strudel's naming)
  - `pub struct TimeSpan { start: f64, end: f64 }` — fractional cycle
    positions
  - `CyclePattern::query(&self, span: TimeSpan) -> Vec<Hap<T>>` —
    returns all events whose `part` intersects the span
  - Constructors: `pure(value)`, `silence()`, `from_vec(Vec<(TimeSpan, T)>)`
- [ ] Combinators:
  - `.fast(n: f64)`, `.slow(n: f64)` — stretch the cycle
  - `.rev()` — mirror events around cycle midpoint
  - `.rotate(amount: f64)` — shift events along the cycle
  - `.cat(others: &[CyclePattern])` — concatenate cycles
  - `.stack(others: &[CyclePattern])` — polyphonic overlay
  - `.every(n, transform)` / `.sometimes(p, transform)` — same semantics
    as current `Sequence` modifiers
  - `.ply(n)` — replace each event with `n` copies at that event's time
- [ ] Conversion:
  - `Pattern::<T>::to_cycle(&self) -> CyclePattern<T>`
  - `CyclePattern::<T>::to_steps(divisions: usize) -> Pattern<T>`
- [ ] Tests: pure value, silence, fast/slow, rev, rotate, cat, stack,
      every, conversion roundtrips.

**Deliverable:** `CyclePattern<T>` working standalone; pattern algebra
passes tests. No string parser yet.

### Phase C2 — `CycleClock` + `CycleSequence<T>` (2–3 days)

- [ ] `nyx-seq/src/cycle_clock.rs`:
  - `pub struct CycleClock<S: Signal>` — like `Clock` but tracks
    `cycle: f64` with `cps: Param<S>` (cycles per second)
  - `.tick(ctx) -> CycleState { cycle, phase_in_cycle, cps }`
  - `.reset()`, `.sync_to(cycle: f64)` for live-coding re-sync
- [ ] `nyx-seq/src/cycle_sequence.rs`:
  - `pub struct CycleSequence<T: Clone>` — queries a `CyclePattern<T>`
    each sample, emits `StepEvent<T>`-like events when a new `Hap`
    starts
- [ ] Integration with existing `Clock`: document how to translate
      BPM → CPS and vice versa.
- [ ] Tests: pattern of 4 events at `cps=1.0` for 4 seconds emits
      each event once; polymeter `"[a b c, d e]"` emits both streams
      correctly.

**Deliverable:** mini-notation-ready backend — can query patterns
against a cycle clock and dispatch events.

### Phase C3 — Mini-notation parser (`nyx-mini`) (5–7 days)

- [ ] New workspace crate `nyx-mini`:
  ```toml
  [package]
  name = "nyx-mini"
  [dependencies]
  nyx-core = { path = "../nyx-core" }
  ```
- [ ] Parser module (handwritten recursive descent, not nom/pest):
  - Tokens: whitespace-separated, `~`, `*N`, `/N`, `[`, `]`, `<`, `>`,
    `,`
  - AST: `Node = Value(String) | Seq(Vec<Node>) | Stack(Vec<Node>) |
    Alt(Vec<Node>) | Fast(Box<Node>, f64) | Slow(Box<Node>, f64) |
    Rest`
  - `parse(input: &str) -> Result<Node, ParseError>`
- [ ] Render module:
  - `Node::render<T>(&self, map_leaf: impl Fn(&str) -> T) -> CyclePattern<T>`
  - Handles time-span division for nested `Seq` and `Stack`
  - `Alt` cycles through options (one per parent cycle)
- [ ] Convenience: `fn mini<T, F: Fn(&str) -> T>(s: &str, f: F) -> CyclePattern<T>`
  panicking on parse error (for prelude use), plus the non-panicking
  `try_mini`.
- [ ] Tests:
  - `"a b c d"` → 4 equal events
  - `"a ~ b ~"` → 2 events at 0 and 0.5
  - `"[a b] c"` → a at 0, b at 0.25, c at 0.5
  - `"a*3"` → 3 events of a
  - `"a/2 b"` → a over 2 cycles, b in cycle 2 only (alternation-like)
  - `"[a b, c d e]"` → polymeter (both groups fill the cycle)
  - `"<a b c>"` → a in cycle 0, b in cycle 1, c in cycle 2, loop

**Deliverable:** `mini("bd ~ sn ~", |s| s.to_string())` returns a
`CyclePattern<String>` that matches the expected events.

### Phase C4 — Scale DSL + Samples integration (2–3 days)

- [ ] Extend `nyx-seq::Scale` with pattern-aware mapping:
  - `Scale::map_pattern<F: Fn(i32) -> Note>(&self, pat: CyclePattern<i32>) -> CyclePattern<Note>`
  - Or a helper trait: `CyclePattern<i32>::scale(self, &Scale) -> CyclePattern<Note>`
- [ ] Note names in mini-notation:
  - `mini("c e g b", Note::parse)` — lifts each string to a `Note`
  - `n("0 2 4 7")` helper — parses integers and wraps into a
    `CyclePattern<i32>` ready for `.scale(...)`
- [ ] Sample pattern:
  - `s("kick snare hat kick", |name| sample_bank.get(name))` — bind
    drum names to pre-loaded samples
  - Requires a `SampleBank` helper in `nyx-seq` or the prelude
    (HashMap<String, Sample> built once at startup)

**Deliverable:** full Strudel-style composition:
```rust
let kick_pat = mini("bd ~ sn ~", |s| bank.get(s));
let bass_pat = n("0 3 5 7").scale("a:minor");
```

### Phase C5 — Prelude integration + live coding helper (2–3 days)

- [ ] `nyx-prelude` re-exports:
  - `mini`, `n`, `try_mini` from `nyx-mini`
  - `CyclePattern`, `CycleClock`, `CycleSequence` from core/seq
- [ ] Sketch template for mini-notation:
  ```rust
  use nyx_prelude::*;

  #[unsafe(no_mangle)]
  pub fn nyx_sketch() -> Box<dyn Signal> {
      let pat = mini("bd ~ sn ~", inst_name);
      // ... dispatch samples from the pattern
      voice.boxed()
  }
  ```
- [ ] Cookbook examples:
  - `mini_drums.rs` — mini-notation drum pattern driving samples
  - `mini_melody.rs` — `n("0 3 5 7").scale("c:minor")` through SubSynth
  - `polymeter.rs` — `"[a b c, d e]"` showing polymeter

### Phase C6 — Runtime mini-notation (stretch, 5–7 days)

Only pursue after C5 lands and stabilises. This is Path B from §3.5.

- [ ] `nyx-cli` gains a new mode: `nyx watch sketch.nyx` — watches a
      plain-text file of mini-notation expressions, re-parses on save,
      hot-swaps the `CyclePattern` via SPSC to the audio thread
- [ ] File format:
  ```
  # drums.nyx
  kick = "bd ~ sn ~"
  hats = "~ hh ~ hh"
  bass = n("0 5 3 0")
  ```
- [ ] Runtime sends the parsed pattern over the SPSC bridge; audio
      thread swaps it atomically at the next cycle boundary (avoiding
      mid-event glitches)
- [ ] Target latency: < 100 ms file-save → audible change

**Deliverable:** a browser-less, terminal-based Strudel UX — edit a
text file, save, hear it immediately.

---

## 5. Shared infrastructure

### `Pattern` vs `CyclePattern` coexistence

- `Pattern<T>` stays the "beginner" type — fixed step count, simple
  array ops.
- `CyclePattern<T>` is the "advanced" type — function-based, supports
  polymeters, mini-notation output.
- Document which to reach for when.
- Conversions are cheap (step pattern → cycle pattern is a linear map;
  cycle pattern → step pattern samples at equally-spaced times).

### Cycle vs beat clocks

- `Clock` (beat-based) stays the default for BPM-native sketches (the
  existing trance example, etc.).
- `CycleClock` (cycle-based) is the mini-notation-native clock.
- Both are driven by the same sample tick; which one a sketch uses is
  a style choice, not a technical one.
- Document the mental model: "cycles ≈ bars at 4/4; `cps=1.0` ≈ 120 BPM."

### Error handling

- `ParseError` in `nyx-mini` uses `thiserror` (established Sprint 1
  convention).
- Include source position: `ParseError { pos: usize, message: String }`.

---

## 6. Order of attack relative to existing roadmap

```
Sprint 3 (DSP completion — chorus/flanger, bus, compressor, granular,
          pitch detection)
   ↓
Phase C  (Mini-notation DSL — this doc)
   ↓
Phase B  (WASM target — pairs with Phase C to make a real
          Strudel alternative in the browser)
   ↓
Phase A  (DAW bridge — whenever there's demand)
```

**Why Phase C before Phase B:** a WASM build of Nyx without the DSL is
"Rust compiled to WASM" — narrow audience. With the DSL, it becomes a
legitimate browser live-coding platform. The DSL also doesn't depend
on WASM to be useful (native Rust users benefit immediately).

**Why Sprint 3 first:** the DSL is a UX layer on top of DSP. If you
finish the DSP primitives first (chorus, compressor, granular), the
DSL doesn't need revisiting every time a new effect lands. The parser's
leaf-mapping function just gains a new option.

---

## 7. Out of scope for Phase C v1

Explicit no's:

- **Euclidean mini-notation `"a(3, 8)"`** — users can call
  `Euclid::generate(3, 8)` and lift to a `CyclePattern` via `.to_cycle()`
- **Arbitrary math in values `"a:{0+1}"`** — Strudel has this;
  Tidal-style "control patterns" with operators; wait for demand
- **Swing / humanize / nudge** — these should be combinators on
  `CyclePattern`, not mini-notation tokens
- **MIDI out from patterns** — defer to Phase A (DAW bridge) integration
- **Browser renderer / live editor UI** — pairs with Phase B
- **Pattern-level effects routing** (`.room()`, `.delay()` as pattern
  modifiers like Strudel has) — our existing combinators on `Signal`
  serve this role differently; revisit if users ask

---

## 8. Success criteria

Phase C is done when:

- `mini("bd ~ sn ~", &bank)` parses and plays the expected drum pattern
- `n("0 3 5 7").scale("a:minor")` produces a melodic line
- Polymeter `"[a b c, d e]"` emits two streams aligned to one cycle
- `.fast(2)`, `.slow(2)`, `.rev()`, `.every(4, rev)`, `.sometimes(0.3, rotate)`
  all work on `CyclePattern`
- A cookbook `trance.rs`-scale example using mini-notation compiles
  and plays
- All existing `Pattern<T>` / `Sequence<T>` code keeps working
  unchanged (non-breaking)
- 50+ new tests across `nyx-mini` and the cycle-pattern modules

---

## 9. API sketch (final shape)

```rust
use nyx_prelude::*;

fn main() {
    // Samples
    let bank = SampleBank::new()
        .load("bd", "kick.wav").unwrap()
        .load("sn", "snare.wav").unwrap()
        .load("hh", "hihat.wav").unwrap();

    // Mini-notation
    let drums = mini("bd ~ sn ~", |s| bank.get(s).unwrap())
        .every(4, |p| p.rev())
        .fast(1.0);

    // Melody
    let melody = n("0 3 5 7 5 3")
        .scale("a:minor")
        .slow(2);

    // Cycle clock at 0.5 cps (≈ 120 BPM at 4 beats/cycle)
    let mut clk = cycle_clock(0.5);
    let mut drum_seq = CycleSequence::new(drums);
    let mut mel_seq = CycleSequence::new(melody);

    // ... assemble into a Signal via a closure, as usual
}
```

`nyx-mini` API surface is deliberately small:

```rust
pub fn mini<T>(input: &str, leaf: impl Fn(&str) -> T) -> CyclePattern<T>;
pub fn try_mini<T>(input: &str, leaf: impl Fn(&str) -> T)
    -> Result<CyclePattern<T>, ParseError>;
pub fn n(input: &str) -> CyclePattern<i32>;  // integer shortcut
```

---

## 10. Timeline estimate

| Phase | Days (FTE) |
|---|---|
| C0 research | 2–3 |
| C1 CyclePattern type + combinators | 3–5 |
| C2 CycleClock + CycleSequence | 2–3 |
| C3 Mini-notation parser | 5–7 |
| C4 Scale DSL + sample banks | 2–3 |
| C5 Prelude integration + cookbook | 2–3 |
| C6 Runtime mini-notation (stretch) | 5–7 |

Total without C6: **16–24 days** (~3–5 weeks)
With C6: **21–31 days** (~4–6 weeks)

This slots comfortably between Sprint 3 (~2–3 weeks) and Phase B (WASM,
~2–3 weeks) in the longer-term roadmap.
