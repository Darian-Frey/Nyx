//! MIDI input: CC mapping, note events, and smoothed CC signals.
//!
//! Gated behind the `midi` feature. Uses `midir` for cross-platform
//! low-latency MIDI input.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use rtrb::{Consumer, Producer, RingBuffer};

use crate::signal::{AudioContext, Signal};

// ─── MIDI Events ────────────────────────────────────────────────────

/// A MIDI event sent from the input thread to the audio thread.
#[derive(Debug, Clone, Copy)]
pub enum MidiEvent {
    NoteOn { channel: u8, note: u8, velocity: u8 },
    NoteOff { channel: u8, note: u8 },
    ControlChange { channel: u8, cc: u8, value: u8 },
}

/// Parse raw MIDI bytes into a `MidiEvent`.
pub fn parse_midi(data: &[u8]) -> Option<MidiEvent> {
    if data.len() < 2 {
        return None;
    }
    let status = data[0] & 0xF0;
    let channel = data[0] & 0x0F;
    match status {
        0x90 if data.len() >= 3 && data[2] > 0 => Some(MidiEvent::NoteOn {
            channel,
            note: data[1],
            velocity: data[2],
        }),
        0x90 if data.len() >= 3 => Some(MidiEvent::NoteOff {
            channel,
            note: data[1],
        }),
        0x80 if data.len() >= 3 => Some(MidiEvent::NoteOff {
            channel,
            note: data[1],
        }),
        0xB0 if data.len() >= 3 => Some(MidiEvent::ControlChange {
            channel,
            cc: data[1],
            value: data[2],
        }),
        _ => None,
    }
}

// ─── MIDI Bridge ────────────────────────────────────────────────────

/// Producer side for sending MIDI events to the audio thread.
pub struct MidiSender {
    producer: Producer<MidiEvent>,
}

impl MidiSender {
    /// Send a MIDI event. Drops silently if the buffer is full.
    pub fn send(&mut self, event: MidiEvent) {
        let _ = self.producer.push(event);
    }
}

/// Consumer side for receiving MIDI events on the audio thread.
pub struct MidiReceiver {
    consumer: Consumer<MidiEvent>,
}

impl MidiReceiver {
    /// Drain all pending MIDI events.
    pub fn drain(&mut self) -> impl Iterator<Item = MidiEvent> + '_ {
        std::iter::from_fn(move || self.consumer.pop().ok())
    }
}

/// Create a MIDI event bridge (SPSC ring buffer).
pub fn midi_bridge(capacity: usize) -> (MidiSender, MidiReceiver) {
    let (producer, consumer) = RingBuffer::new(capacity);
    (MidiSender { producer }, MidiReceiver { consumer })
}

// ─── CC Map ─────────────────────────────────────────────────────────

/// Atomic CC value store: 128 CC slots, each an `AtomicU8`.
///
/// Written from the MIDI callback, read from the audio thread.
/// Lock-free and allocation-free after construction.
pub struct CcMap {
    values: Arc<[AtomicU8; 128]>,
}

impl CcMap {
    /// Create a new CC map with all values at 0.
    pub fn new() -> Self {
        CcMap {
            values: Arc::new(core::array::from_fn(|_| AtomicU8::new(0))),
        }
    }

    /// Get a shared reference for the MIDI callback to write into.
    pub fn writer(&self) -> CcWriter {
        CcWriter {
            values: Arc::clone(&self.values),
        }
    }

    /// Read the raw CC value (0–127) for a given CC number.
    pub fn get(&self, cc: u8) -> u8 {
        self.values[cc as usize].load(Ordering::Relaxed)
    }

    /// Get a CC value normalised to [0, 1].
    pub fn get_normalized(&self, cc: u8) -> f32 {
        self.get(cc) as f32 / 127.0
    }

    /// Create a `Signal` that reads a CC value with one-pole smoothing.
    ///
    /// `smooth_ms` is the smoothing time in milliseconds (default 5.0).
    pub fn signal(&self, cc: u8, smooth_ms: f32) -> CcSignal {
        CcSignal {
            values: Arc::clone(&self.values),
            cc,
            smooth_ms,
            current: 0.0,
            coeff: 0.0,
            initialised: false,
        }
    }
}

impl Default for CcMap {
    fn default() -> Self {
        Self::new()
    }
}

/// Writer handle for the MIDI callback to update CC values.
pub struct CcWriter {
    values: Arc<[AtomicU8; 128]>,
}

impl CcWriter {
    /// Set a CC value (called from the MIDI callback).
    pub fn set(&self, cc: u8, value: u8) {
        self.values[cc as usize].store(value, Ordering::Relaxed);
    }
}

/// A `Signal` that reads a CC value with one-pole smoothing.
///
/// Prevents zipper noise when CC values change abruptly.
pub struct CcSignal {
    values: Arc<[AtomicU8; 128]>,
    cc: u8,
    smooth_ms: f32,
    current: f32,
    coeff: f32,
    initialised: bool,
}

impl Signal for CcSignal {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        if !self.initialised {
            let samples = self.smooth_ms * 0.001 * ctx.sample_rate;
            self.coeff = if samples > 0.0 {
                1.0 - (-1.0 / samples).exp()
            } else {
                1.0
            };
            self.current = self.values[self.cc as usize].load(Ordering::Relaxed) as f32 / 127.0;
            self.initialised = true;
        }

        let target = self.values[self.cc as usize].load(Ordering::Relaxed) as f32 / 127.0;
        self.current += self.coeff * (target - self.current);
        self.current
    }
}

// ─── MIDI Input backends ────────────────────────────────────────────
//
// Native (ALSA/CoreMIDI/WinMM) goes through `midir`. The browser uses a
// hand-rolled WebMIDI backend in `midi_web` — this side-steps an upstream
// type-inference bug in `midir-0.10.3`'s WebMIDI backend (see
// docs/phase-b-wasm.md). Both backends expose the same
// `open_midi_input` / `open_midi_input_named` / `MidiConnection`
// surface so user code compiles against either target unchanged.

#[cfg(all(feature = "midi", not(target_arch = "wasm32")))]
#[path = "midi_native.rs"]
mod midi_native;
#[cfg(all(feature = "midi", not(target_arch = "wasm32")))]
pub use midi_native::{MidiConnection, open_midi_input, open_midi_input_named};

#[cfg(all(feature = "midi", target_arch = "wasm32"))]
#[path = "midi_web.rs"]
mod midi_web;
#[cfg(all(feature = "midi", target_arch = "wasm32"))]
pub use midi_web::{MidiConnection, open_midi_input, open_midi_input_named};

/// Errors from MIDI input. Shared between native and web backends.
#[cfg(feature = "midi")]
#[derive(Debug)]
pub enum MidiError {
    /// Backend failed to initialise (host error, insecure context, etc.).
    Init(String),
    /// No MIDI input port matched the requested filter.
    NoPort,
    /// Opening the selected port failed.
    Connect(String),
}

#[cfg(feature = "midi")]
impl std::fmt::Display for MidiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MidiError::Init(e) => write!(f, "MIDI init error: {e}"),
            MidiError::NoPort => write!(f, "no MIDI input port found"),
            MidiError::Connect(e) => write!(f, "MIDI connect error: {e}"),
        }
    }
}

#[cfg(feature = "midi")]
impl std::error::Error for MidiError {}
