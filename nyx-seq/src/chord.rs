//! Chord types and construction.
//!
//! A chord is built from a root `Note` and a `ChordType` that defines
//! the intervals. Chords return their constituent notes as a `Vec<Note>`.

use crate::note::Note;

/// Chord interval patterns (semitones from root).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChordType {
    Major,
    Minor,
    Diminished,
    Augmented,
    Major7,
    Minor7,
    Dominant7,
    Sus2,
    Sus4,
}

impl ChordType {
    /// Return the semitone intervals for this chord type.
    pub fn intervals(self) -> &'static [u8] {
        match self {
            ChordType::Major => &[0, 4, 7],
            ChordType::Minor => &[0, 3, 7],
            ChordType::Diminished => &[0, 3, 6],
            ChordType::Augmented => &[0, 4, 8],
            ChordType::Major7 => &[0, 4, 7, 11],
            ChordType::Minor7 => &[0, 3, 7, 10],
            ChordType::Dominant7 => &[0, 4, 7, 10],
            ChordType::Sus2 => &[0, 2, 7],
            ChordType::Sus4 => &[0, 5, 7],
        }
    }
}

/// A chord: a root note plus a chord type.
#[derive(Debug, Clone)]
pub struct Chord {
    root: Note,
    chord_type: ChordType,
}

impl Chord {
    /// Build a chord from a root note and type.
    pub fn new(root: Note, chord_type: ChordType) -> Self {
        Chord { root, chord_type }
    }

    /// Convenience: major chord.
    pub fn major(root: Note) -> Self {
        Self::new(root, ChordType::Major)
    }

    /// Convenience: minor chord.
    pub fn minor(root: Note) -> Self {
        Self::new(root, ChordType::Minor)
    }

    /// Convenience: dominant 7th chord.
    pub fn dom7(root: Note) -> Self {
        Self::new(root, ChordType::Dominant7)
    }

    /// Return the notes in this chord.
    pub fn notes(&self) -> Vec<Note> {
        self.chord_type
            .intervals()
            .iter()
            .map(|&interval| self.root.transpose(interval as i8))
            .collect()
    }

    /// Return the frequencies of all notes in the chord.
    pub fn freqs(&self) -> Vec<f32> {
        self.notes().iter().map(|n| n.to_freq()).collect()
    }

    /// Transpose the entire chord by semitones.
    pub fn transpose(self, semitones: i8) -> Self {
        Chord {
            root: self.root.transpose(semitones),
            chord_type: self.chord_type,
        }
    }

    /// The root note.
    pub fn root(&self) -> Note {
        self.root
    }

    /// The chord type.
    pub fn chord_type(&self) -> ChordType {
        self.chord_type
    }
}
