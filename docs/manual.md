# Nyx Audio — User Manual

> High-performance audio synthesis and sequencing for Rust.

Nyx is the "p5.js of sound" — a library for creative coders, algorithmic
composers, and live performers to sketch with audio using a fluent, expressive
API, without managing buffers, thread safety, or DSP boilerplate.

---

## Table of Contents

1. [Quick Start](#quick-start)
2. [Core Concepts](#core-concepts)
3. [Oscillators & Noise](#oscillators--noise)
4. [Signal Combinators](#signal-combinators)
5. [Filters](#filters)
6. [Dynamics](#dynamics)
7. [Clock & Timing](#clock--timing)
8. [Envelopes](#envelopes)
9. [Automation](#automation)
10. [Music Theory](#music-theory)
11. [Patterns & Sequencing](#patterns--sequencing)
12. [Euclidean Rhythms](#euclidean-rhythms)
13. [Randomness](#randomness)
14. [Instruments](#instruments)
15. [SubSynth & Patches](#subsynth--patches)
16. [Visual Mirror (Scope & Spectrum)](#visual-mirror)
17. [MIDI Input](#midi-input)
18. [OSC Input](#osc-input)
19. [Microphone Input](#microphone-input)
20. [GUI Widgets (nyx-iced)](#gui-widgets)
21. [Hot Reload (nyx-cli)](#hot-reload)
22. [Testing Utilities](#testing-utilities)
23. [Real-Time Safety](#real-time-safety)
24. [API Reference](#api-reference)

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

```rust
// examples/sketches/my_sketch.rs
use nyx_core::osc;
use nyx_core::SignalExt;
use nyx_core::Signal;

#[unsafe(no_mangle)]
pub fn nyx_sketch() -> Box<dyn Signal> {
    osc::sine(440.0).amp(0.3).boxed()
}
```

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

| Crate | Purpose |
|---|---|
| `nyx-core` | Signal engine, oscillators, filters, dynamics, scope, spectrum, MIDI, OSC, mic |
| `nyx-seq` | Clock, envelopes, automation, notes, scales, chords, patterns, sequencer, instruments, SubSynth |
| `nyx-iced` | Iced GUI widgets (knob, sliders, XY pad, oscilloscope, spectrum) |
| `nyx-prelude` | Convenience re-exports for one-line imports |
| `nyx-cli` | Hot-reload sketch player |

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
