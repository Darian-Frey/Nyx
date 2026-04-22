# Roadmap — Final Push to v1.0 (shipping)

This is a different axis from the other roadmaps. The DSP work is
tracked in [roadmap-features.md](roadmap-features.md) and
[roadmap-sonic-character.md](roadmap-sonic-character.md); the
deferred Phase A/B plans live in [roadmap-deferred.md](roadmap-deferred.md).
**This file is about turning a code-complete library into a shipped
product** — the gap between "the tests pass" and "someone I've never
met can `cargo add` it and make something."

The library itself is substantial: 11 numbered phases complete,
sonic-character pack complete, 9 preset voices, interactive WASM
demo deployed, 527 tests green, clippy + fmt clean. Calling Nyx
"finished" requires doing something with all of that.

---

## Tier 1 — Blocking (required for v0.1 release)

Without these, nobody can use Nyx.

### 1.1 Publish to crates.io

Register and publish every library crate at `0.1.0`. Binary crates
(`nyx-cli`, `nyx-wasm-demo`, `nyx-examples`) don't need publishing
unless you want `cargo install nyx-cli` to work.

**Publish order** (dependencies first):

1. `nyx-core`
2. `nyx-seq`
3. `nyx-iced`
4. `nyx-prelude`
5. `nyx-cli` (optional — enables `cargo install nyx-cli`)

**Pre-publish checklist** (per crate):

- [ ] `description` and `repository` fields populated in each
      `Cargo.toml` (currently missing — wasm-pack already flagged this)
- [ ] `keywords` and `categories` set for crates.io search. Suggested
      categories: `multimedia::audio`, `multimedia::encoding`,
      `no-std::no-alloc` (for nyx-core)
- [ ] `documentation = "https://docs.rs/<crate>"` field
- [ ] `readme = "README.md"` at each crate root (symlinks or copies
      of the workspace README or a crate-specific short README)
- [ ] `LICENSE-MIT` and `LICENSE-APACHE` present in every published
      crate (docs.rs requires this; `license.workspace = true` doesn't
      imply file presence in the published tarball)
- [ ] `cargo publish --dry-run -p <crate>` passes

**Name availability** — `nyx`, `nyx-core`, `nyx-seq` may already be
claimed on crates.io. If so, fall back to namespaced variants
(`nyxaudio-*` or `nyxaudio-audio-*`). Check with
`cargo search <name>` before the publish attempt.

**Pre-1.0 note** — README should explicitly state "Nyx is pre-1.0;
expect API changes before v1.0." Set this expectation before anyone
builds on top.

Estimated effort: **½ day** if names are available and metadata is
tidy; **1 day** with naming backfill.

### 1.2 Hands-on native-audio validation on Windows and macOS

CI compiles on `ubuntu-latest` and `macos-latest`, but audio-device
tests are gated behind `#[ignore]` — nobody has actually pressed
"play" on macOS or Windows and confirmed sound comes out. Before
calling this a product, somebody needs to.

**Procedure per OS:**

1. Check out `main`.
2. `cargo run -p nyx-prelude --example wav_export --release` — no
   audio required, confirms rendering works.
3. `cargo run -p nyx-prelude --example dubstep_wobble --release` —
   confirms `play()` and cpal output work end-to-end.
4. `cargo run -p nyx-prelude --example midi_filter --release --features midi`
   with a connected MIDI controller — confirms MIDI input works.
5. Play the Tron demo (`cargo run -p nyx-prelude --example tron_wav --release` then play the WAV)
   and the WASM page (serve `nyx-wasm-demo/index.html` locally).

**Expected outcomes per OS:**

- Linux (ALSA / PulseAudio / PipeWire): already verified in development.
- macOS (CoreAudio via cpal): pending.
- Windows (WASAPI via cpal): pending.

If cpal mis-behaves on either, that's a bug to either work around or
document as a known limitation for v0.1.

Estimated effort: **1–2 hours per OS** if you have access. Without
Windows/macOS hardware, rope in a collaborator or use GitHub
Actions with a manual-trigger workflow that plays to a null sink.

### 1.3 First external user or published artefact

One person outside the project builds something — a sketch, a small
app, a blog post — with Nyx. Their first 30 minutes will surface
paper cuts you can't see. Target: **one real thing you didn't make**
before calling it shipped.

Possible routes:

- Share the live demo link on `/r/rust` or `/r/audiodev` and
  collect reactions. Lowest friction.
- Post a tutorial: "Build a generative-melody sketch in 50 lines of
  Rust." Links to the library, invites contributions.
- Reach out to one or two creative-coders who might be interested
  (Rust-audio Discord, Sonic Pi community, Strudel users) and ask
  for 30 minutes of feedback.

Estimated effort: **variable** — this is a marketing/community task,
not an engineering one.

---

## Tier 2 — Polish (required for v1.0 release)

These upgrade "usable" to "proud to tag 1.0".

### 2.1 docs.rs-quality API documentation

Every public item needs a `///` doc comment. Module-level `//!` docs
should orient new readers. `cargo doc --workspace --no-deps` should
render cleanly with no broken intra-doc links.

**Gap audit**:

```bash
# Lists every public item missing documentation.
cargo doc --workspace --no-deps 2>&1 | grep "warning: missing documentation"
```

Current state is good in most places (the manual is thorough), but
some internal structs and small helpers probably lack `///` comments
and will show up in the above listing.

Estimated effort: **1 day** to audit and fill gaps, assuming the
audit finds < 50 items.

### 2.2 AudioWorklet backend for WASM

The current WASM demo uses cpal's ScriptProcessorNode backend, which
W3C deprecated years ago in favour of AudioWorklet. ScriptProcessor
still works in every major browser but has higher latency and is a
main-thread citizen.

**Paths:**

- Wait for cpal to add AudioWorklet support (upstream issue open for
  years; no eta).
- Write a custom WebAudio backend in `nyx-wasm-demo` using
  `wasm-bindgen` against Web Audio's `AudioWorkletProcessor` directly.
- Fork cpal or contribute AudioWorklet support upstream. Meaningful
  engineering effort (weeks).

**Recommendation for v1.0**: document the ScriptProcessor dependency
as a known limitation, keep the demo working, revisit when cpal
catches up or when it becomes the last real blocker.

Estimated effort: **weeks** if implemented; **1 hour** to document.

### 2.3 Semver commitment + API surface freeze

Calling a crate `1.0.0` is a promise: breaking changes only on
major-version bumps. Before tagging `1.0`, do a deliberate pass on
the public API:

- [ ] Every public type reviewed for "do I want to be stuck with
      this name / signature / field layout?"
- [ ] Every `pub` that should be `pub(crate)` demoted. The no-alloc
      guard, golden-file helpers, and internal math helpers are
      likely candidates.
- [ ] Trait method signatures locked — adding methods later is fine
      in a point release; changing existing ones isn't.
- [ ] Deprecation policy documented (e.g. "deprecated methods live
      for one major version before removal").

**Crates that need the most review**: `nyx-core` (the trait surface
is broad), `nyx-seq` (lots of instrument / preset structs whose
public fields may not need to stay public).

Estimated effort: **2 days** of focused review.

### 2.4 Badges + repository metadata

- [ ] `crates.io` version badge in README
- [ ] `docs.rs` badge in README
- [ ] CI status badge in README
- [ ] GitHub Pages status badge
- [ ] GitHub repository topics set: `audio`, `dsp`, `synthesis`,
      `rust`, `creative-coding`, `music`, `wasm`
- [ ] `CONTRIBUTING.md` at repo root describing the development
      workflow, required checks (fmt / clippy / tests), and the
      release process

Estimated effort: **½ day**.

---

## Tier 3 — Nice to have (post-1.0 or stretch)

### 3.1 Binary distribution for `nyx-cli`

Pre-built binaries for Linux / macOS / Windows on GitHub Releases,
triggered by git tags. Pairs with `cargo install nyx-cli` for
Rust-aware users; binaries serve everyone else.

Template exists in many Rust projects (`cargo-dist`, `release-plz`).

Estimated effort: **1 day** using `cargo-dist`.

### 3.2 Tutorial content

- "Your first sketch in 10 minutes" walkthrough
- "Making a drum machine" — sequencer + presets
- "Live-coding a techno loop" — hot-reload demo
- Embed the WASM demo page in a proper landing site with narrative

A dedicated landing page (GitHub Pages, separate from the WASM demo)
could host this. `mdBook` is a natural fit for the tutorial content.

Estimated effort: **1 week** for a modest tutorial set + landing
page.

### 3.3 Bench suite

Document real-world performance: voices-per-millisecond at 44.1 kHz
on a reference machine, relative cost of PolyBLEP vs naive saw,
ladder filter cost, granular engine cost. Useful for users deciding
how many simultaneous voices they can afford.

`criterion` is the idiomatic benchmark framework; results can live
in `BENCHMARKS.md` or as docs.rs-rendered pages.

Estimated effort: **2–3 days**.

### 3.4 Known-users page

Once there's more than one project built with Nyx, a list of them
in the README (or a dedicated `USERS.md`) is strong social proof and
a recruiting tool for contributors. Start tracking from the first
one.

Estimated effort: ongoing.

---

## Implementation order recommendation

1. **Tier 1.1** (crates.io publish) — unblocks everything else.
2. **Tier 1.2** (cross-OS validation) — do before publish or
   immediately after; whichever bug reports you want first.
3. **Tier 2.4** (badges, metadata) — cheap, high signal-to-effort.
4. **Tier 1.3** (external user) — in parallel with 2.x; real feedback
   informs the API freeze.
5. **Tier 2.1 + 2.3** (docs.rs quality + semver review) — the last
   work before tagging v1.0.
6. **Tier 2.2** (AudioWorklet) — document as known limitation for
   v1.0; revisit post-1.0 when cpal catches up.
7. **Tier 3** — opportunistic; none of it blocks a v1.0 release.

---

## Success criteria for each release

| Release | Criteria |
| --- | --- |
| **v0.1.0** (pre-1.0 public) | Published to crates.io, README states pre-1.0 status, Linux + one of (macOS / Windows) audio hand-verified |
| **v0.9.0** (release candidate) | All Tier 2 items complete except 2.2, at least one external project built with Nyx |
| **v1.0.0** (stable) | Semver committed, API surface reviewed and frozen, badges + metadata, CONTRIBUTING.md |

---

*End of final-push roadmap.*
