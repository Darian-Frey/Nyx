//! Scale library.
//!
//! A scale is defined by a root note and a set of intervals (semitone offsets
//! within one octave). Scales can snap arbitrary f32 values (MIDI note numbers
//! or frequencies) to the nearest note in the scale.

use crate::note::Note;

/// A musical scale: a root pitch class and a set of semitone intervals.
#[derive(Debug, Clone)]
pub struct Scale {
    root: u8, // pitch class 0–11
    intervals: &'static [u8],
}

// --- Interval patterns ---
const MAJOR: &[u8] = &[0, 2, 4, 5, 7, 9, 11];
const MINOR: &[u8] = &[0, 2, 3, 5, 7, 8, 10];
const PENTATONIC_MAJOR: &[u8] = &[0, 2, 4, 7, 9];
const PENTATONIC_MINOR: &[u8] = &[0, 3, 5, 7, 10];
const DORIAN: &[u8] = &[0, 2, 3, 5, 7, 9, 10];
const PHRYGIAN: &[u8] = &[0, 1, 3, 5, 7, 8, 10];
const LYDIAN: &[u8] = &[0, 2, 4, 6, 7, 9, 11];
const MIXOLYDIAN: &[u8] = &[0, 2, 4, 5, 7, 9, 10];
const LOCRIAN: &[u8] = &[0, 1, 3, 5, 6, 8, 10];
const WHOLE_TONE: &[u8] = &[0, 2, 4, 6, 8, 10];
const CHROMATIC: &[u8] = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];

impl Scale {
    /// Create a scale from a root note name (e.g. "C", "F#", "Bb") and a mode.
    ///
    /// The root is parsed as a pitch class — the octave is ignored.
    pub fn new(root: &str, mode: ScaleMode) -> Self {
        let root_pc = parse_pitch_class(root).unwrap_or(0);
        Scale {
            root: root_pc,
            intervals: mode.intervals(),
        }
    }

    /// Major scale.
    pub fn major(root: &str) -> Self {
        Self::new(root, ScaleMode::Major)
    }

    /// Natural minor scale.
    pub fn minor(root: &str) -> Self {
        Self::new(root, ScaleMode::Minor)
    }

    /// Major pentatonic scale.
    pub fn pentatonic(root: &str) -> Self {
        Self::new(root, ScaleMode::PentatonicMajor)
    }

    /// Minor pentatonic scale.
    pub fn pentatonic_minor(root: &str) -> Self {
        Self::new(root, ScaleMode::PentatonicMinor)
    }

    /// Snap a MIDI note number (as f32) to the nearest note in this scale.
    ///
    /// This is the core quantisation function. It works with fractional
    /// MIDI values so it can accept raw control signals.
    pub fn snap(&self, midi_f: f32) -> Note {
        let midi = midi_f.round() as i16;
        let pc = ((midi % 12 + 12) % 12) as u8;
        let octave_base = midi - pc as i16;

        // Find nearest scale degree.
        let mut best = 0_u8;
        let mut best_dist = 12_i16;
        for &interval in self.intervals {
            let scale_pc = (self.root + interval) % 12;
            let dist = ((pc as i16 - scale_pc as i16 + 12) % 12)
                .min((scale_pc as i16 - pc as i16 + 12) % 12);
            if dist < best_dist {
                best_dist = dist;
                best = scale_pc;
            }
        }

        let snapped = octave_base + best as i16;
        // Decide which octave is closer.
        let candidates = [snapped - 12, snapped, snapped + 12];
        let nearest = candidates
            .iter()
            .copied()
            .min_by_key(|&c| (c - midi).unsigned_abs())
            .unwrap_or(snapped);

        Note(nearest.clamp(0, 127) as u8)
    }

    /// Snap a frequency to the nearest note in this scale, returning frequency.
    pub fn snap_freq(&self, freq: f32) -> f32 {
        let midi = 69.0 + 12.0 * (freq / 440.0).log2();
        self.snap(midi).to_freq()
    }

    /// List all notes in this scale within a MIDI range.
    pub fn notes_in_range(&self, low: Note, high: Note) -> Vec<Note> {
        let mut result = Vec::new();
        for midi in low.midi()..=high.midi() {
            let pc = midi % 12;
            for &interval in self.intervals {
                if (self.root + interval) % 12 == pc {
                    result.push(Note(midi));
                    break;
                }
            }
        }
        result
    }

    /// The root pitch class (0 = C, 1 = C#, ..., 11 = B).
    pub fn root(&self) -> u8 {
        self.root
    }

    /// The interval pattern.
    pub fn intervals(&self) -> &[u8] {
        self.intervals
    }
}

/// Available scale modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaleMode {
    Major,
    Minor,
    PentatonicMajor,
    PentatonicMinor,
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
    Locrian,
    WholeTone,
    Chromatic,
}

impl ScaleMode {
    fn intervals(self) -> &'static [u8] {
        match self {
            ScaleMode::Major => MAJOR,
            ScaleMode::Minor => MINOR,
            ScaleMode::PentatonicMajor => PENTATONIC_MAJOR,
            ScaleMode::PentatonicMinor => PENTATONIC_MINOR,
            ScaleMode::Dorian => DORIAN,
            ScaleMode::Phrygian => PHRYGIAN,
            ScaleMode::Lydian => LYDIAN,
            ScaleMode::Mixolydian => MIXOLYDIAN,
            ScaleMode::Locrian => LOCRIAN,
            ScaleMode::WholeTone => WHOLE_TONE,
            ScaleMode::Chromatic => CHROMATIC,
        }
    }
}

/// Parse a pitch class from a string like "C", "C#", "Db", "F#".
fn parse_pitch_class(s: &str) -> Option<u8> {
    let s = s.trim();
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return None;
    }

    let base = match bytes[0].to_ascii_uppercase() {
        b'C' => 0,
        b'D' => 2,
        b'E' => 4,
        b'F' => 5,
        b'G' => 7,
        b'A' => 9,
        b'B' => 11,
        _ => return None,
    };

    let accidental: i8 = if bytes.len() > 1 {
        match bytes[1] {
            b'#' => 1,
            b'b' => -1,
            _ => 0,
        }
    } else {
        0
    };

    Some(((base as i8 + accidental + 12) % 12) as u8)
}
