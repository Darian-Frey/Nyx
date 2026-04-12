//! Note type and pitch utilities.
//!
//! MIDI note numbers are the canonical representation. Note 69 = A4 = 440 Hz.
//! Conversion to frequency uses equal temperament (12-TET, A4 = 440 Hz).

use std::fmt;

/// A musical note, stored as a MIDI note number (0–127).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Note(pub u8);

// --- Named constants ---
impl Note {
    pub const C0: Note = Note(12);
    pub const CS0: Note = Note(13);
    pub const D0: Note = Note(14);
    pub const DS0: Note = Note(15);
    pub const E0: Note = Note(16);
    pub const F0: Note = Note(17);
    pub const FS0: Note = Note(18);
    pub const G0: Note = Note(19);
    pub const GS0: Note = Note(20);
    pub const A0: Note = Note(21);
    pub const AS0: Note = Note(22);
    pub const B0: Note = Note(23);

    pub const C4: Note = Note(60);
    pub const CS4: Note = Note(61);
    pub const D4: Note = Note(62);
    pub const DS4: Note = Note(63);
    pub const E4: Note = Note(64);
    pub const F4: Note = Note(65);
    pub const FS4: Note = Note(66);
    pub const G4: Note = Note(67);
    pub const GS4: Note = Note(68);
    pub const A4: Note = Note(69);
    pub const AS4: Note = Note(70);
    pub const B4: Note = Note(71);

    pub const C5: Note = Note(72);
}

impl Note {
    /// Create a note from a MIDI note number.
    pub fn from_midi(n: u8) -> Self {
        Note(n)
    }

    /// Return the MIDI note number.
    pub fn midi(self) -> u8 {
        self.0
    }

    /// Convert to frequency in Hz (12-TET, A4 = 440 Hz).
    pub fn to_freq(self) -> f32 {
        440.0 * 2.0_f32.powf((self.0 as f32 - 69.0) / 12.0)
    }

    /// Create a note from a frequency (nearest MIDI note).
    pub fn from_freq(freq: f32) -> Self {
        let midi = 69.0 + 12.0 * (freq / 440.0).log2();
        Note(midi.round().clamp(0.0, 127.0) as u8)
    }

    /// Transpose by a number of semitones (can be negative).
    pub fn transpose(self, semitones: i8) -> Self {
        let n = self.0 as i16 + semitones as i16;
        Note(n.clamp(0, 127) as u8)
    }

    /// Move up one octave.
    pub fn up_octave(self) -> Self {
        self.transpose(12)
    }

    /// Move down one octave.
    pub fn down_octave(self) -> Self {
        self.transpose(-12)
    }

    /// The pitch class (0 = C, 1 = C#, ..., 11 = B).
    pub fn pitch_class(self) -> u8 {
        self.0 % 12
    }

    /// The octave number (A4 = octave 4, C4 = octave 4, etc.).
    /// Uses the convention where C is the start of each octave.
    pub fn octave(self) -> i8 {
        (self.0 as i8 / 12) - 1
    }

    /// Parse a note name string like "C4", "C#4", "Db3", "Bb2".
    ///
    /// Returns `None` if the string is not a valid note name.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }

        let bytes = s.as_bytes();
        let mut i = 0;

        // Letter name
        let base = match bytes[i].to_ascii_uppercase() {
            b'C' => 0,
            b'D' => 2,
            b'E' => 4,
            b'F' => 5,
            b'G' => 7,
            b'A' => 9,
            b'B' => 11,
            _ => return None,
        };
        i += 1;

        // Accidental (optional)
        let mut accidental: i8 = 0;
        if i < bytes.len() {
            match bytes[i] {
                b'#' => {
                    accidental = 1;
                    i += 1;
                }
                b'b' => {
                    accidental = -1;
                    i += 1;
                }
                _ => {}
            }
        }

        // Octave number (may be negative, e.g. "C-1")
        let octave_str = &s[i..];
        let octave: i8 = octave_str.parse().ok()?;

        let midi = (octave as i16 + 1) * 12 + base as i16 + accidental as i16;
        if (0..=127).contains(&midi) {
            Some(Note(midi as u8))
        } else {
            None
        }
    }
}

impl fmt::Display for Note {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const NAMES: [&str; 12] = [
            "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
        ];
        let name = NAMES[self.pitch_class() as usize];
        let oct = self.octave();
        write!(f, "{name}{oct}")
    }
}
