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

Everything in Nyx is a **Signal** — a stream of mono audio samples:

```rust
pub trait Signal: Send {
    fn next(&mut self, ctx: &AudioContext) -> f32;
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

| Function | Waveform | Start value |
|---|---|---|
| `osc::sine(freq)` | Sine wave | 0.0 |
| `osc::saw(freq)` | Sawtooth (ramp -1 to +1) | -1.0 |
| `osc::square(freq)` | Square (+1 / -1) | +1.0 |
| `osc::triangle(freq)` | Triangle (-1 to +1 to -1) | -1.0 |

All oscillators track phase as a normalised `f32` in [0, 1), incremented by
`freq / sample_rate` each sample.

### Noise

```rust
osc::noise::white(seed)   // white noise, deterministic from seed
osc::noise::pink(seed)    // pink noise (-3 dB/octave), 12-octave Voss-McCartney
```

Noise generators use a portable xorshift PRNG — same sequence on all platforms.

### Frequency Modulation

```rust
// Vibrato: sine modulated by a slow LFO
let vibrato = osc::sine(5.0).amp(10.0).offset(440.0);
let signal = osc::sine(vibrato);
```

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
|---|---|
| `.lowpass(cutoff, q)` | Resonant low-pass filter |
| `.highpass(cutoff, q)` | Resonant high-pass filter |

Both `cutoff` and `q` accept `f32` or any `Signal`.

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

```rust
use nyx_core::render_to_buffer;

let mut sig = osc::sine(440.0);
let samples = render_to_buffer(&mut sig, 1.0, 44100.0);
// Returns Vec<f32> with 44100 samples
```

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
|---|---|---|
| `nyx-core` | Signal engine, oscillators, filters, dynamics, scope, spectrum, MIDI, OSC, mic, hotswap | Yes |
| `nyx-seq` | Clock, envelopes, automation, notes, scales, chords, patterns, sequencer, instruments, SubSynth | Yes |
| `nyx-iced` | Iced GUI widgets (knob, sliders, XY pad, oscilloscope, spectrum) | Yes |
| `nyx-prelude` | Convenience re-exports + cookbook examples | Yes |
| `nyx-cli` | Hot-reload sketch player | Yes |
| `nyx-examples` | Nannou & Bevy visualisers (heavy deps isolated here) | No — excluded from `default-members` |

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
|---|---|---|
| `SignalExt` | nyx-core | `boxed`, `amp`, `add`, `mul`, `mix`, `pan`, `clip`, `soft_clip`, `offset` |
| `FilterExt` | nyx-core | `lowpass`, `highpass` |
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
- [x] Cookbook examples (4 runnable sketches in `nyx-prelude/examples/`)
- [x] Widget interaction tests (25 tests across knob/slider/xypad)
- [x] Nannou oscilloscope + Bevy spectrum visualisers in `nyx-examples/`
- [x] Expanded `nyx-prelude` to re-export all nyx-seq modules for
      one-line imports in sketches
- [x] Detailed deferred roadmap document

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
