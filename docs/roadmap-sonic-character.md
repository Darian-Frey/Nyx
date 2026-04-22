# Roadmap — Sonic Character ("make existing sounds better")

Nyx's DSP *features* are documented in [roadmap-features.md](roadmap-features.md).
This file tracks a separate axis: improving how the existing primitives
*sound*. The work is motivated by an external audit (see
`target/nyx-sonic-character-report.md`) that diagnosed two root causes
for Nyx's current "digital / 16-bit console" character:

1. **Non-band-limited oscillators.** `osc::saw` and `osc::square` are
   mathematical ideals; their harmonics fold back as inharmonic
   aliasing, which every downstream filter / reverb / chorus inherits.
2. **No warm / analog palette.** `crush`, `softclip`, and `freeverb`
   cover the harsh / digital lo-fi side. There is no tape saturation,
   ladder filter, analog drift, or BBD-style chorus — the primitives
   that make a synth sound "warm" rather than "digital but pleasant."

These two lines of work are independent: fixing (1) improves every
existing sound for free; adding (2) opens a new sonic palette.

---

## Priority order (by audible impact ÷ effort)

| # | Item | Est. LOC | Status |
| --- | ------ | ---------: | -------- |
| 1 | **PolyBLEP `saw_bl` / `square_bl`** — band-limited oscillators; keep naive versions for chiptune | ~80 | ☑ done |
| 2 | **`saturation.rs`** — `TapeSat`, `TubeSat`, `DiodeClip` waveshapers via `SaturationExt` | ~200 | ☑ done |
| 3 | **`ladder.rs`** — Huovilainen non-linear Moog-style 4-pole lowpass, self-oscillates at resonance ≈ 1.0 | ~100 | ☑ done |
| 4 | **`tape.rs`** — wow + flutter + tape EQ + saturation wrapper with an `age` knob | ~150 | ☑ done |
| 5 | **Paul Kellett pink noise** — replaces Voss-McCartney; cheaper, cleaner 1/f slope | ~20 | ☑ done |
| 6 | **`drift.rs`** — slow random oscillator detune in cents, 0.1–1 Hz rate | ~50 | ☑ done |
| 7 | **`lofi.rs`** — preset wrappers (`.cassette()`, `.lofi_hiphop()`, `.vhs()`) composing items 2–5 | ~30 | ☑ done |
| 8 | **`vinyl.rs`** — crackle impulse stream + hiss floor | ~50 | ☑ done |

Items 1–4 alone change Nyx's baseline timbre. Items 5–8 are polish.

---

## Design rules for every item

Every new module on this axis must:

- Allocate state once at construction, on the main thread; no alloc in `next()`.
- Be `Send`.
- Use `IntoParam` for every modulatable parameter.
- Clamp coefficients to stable ranges; prefer saturating non-linearities
  (`tanh`) over hard clamps where numerical explosions are a risk.
- Ship with a golden-file test at representative parameter values
  (tolerance `1e-4` or looser for non-linear modules).
- For filters/waveshapers with self-oscillation (ladder),
  include at least one golden at the edge of stability.

---

## Compatibility rule

**Do not retrofit existing types in place.** Two reasons:

1. Chiptune and retro aesthetics *want* the aliased / raw sound.
2. Existing golden files are contracts — silently changing `osc::saw`
   would break them and hide the behaviour change from users.

New behaviour ships under new names (`saw_bl`, `tape_sat`, `ladder_lp`,
…). The old names keep working. Prelude examples point at the new
names; the old ones stay documented as "raw" / "retro" variants.

---

## References

See the source audit at `target/nyx-sonic-character-report.md` for
full algorithm sketches, canonical literature references, and
implementation notes. Each item's detailed spec is in §3 of that
report.
