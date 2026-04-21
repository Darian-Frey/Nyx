use nyx_seq::{Chord, ChordType, Note, Scale, ScaleMode};

// ===================== Note tests =====================

#[test]
fn a4_is_midi_69() {
    assert_eq!(Note::A4.midi(), 69);
}

#[test]
fn c4_is_midi_60() {
    assert_eq!(Note::C4.midi(), 60);
}

#[test]
fn a4_freq_is_440() {
    let freq = Note::A4.to_freq();
    assert!(
        (freq - 440.0).abs() < 0.01,
        "A4 should be 440 Hz, got {freq}"
    );
}

#[test]
fn c4_freq() {
    let freq = Note::C4.to_freq();
    // C4 ≈ 261.63 Hz
    assert!(
        (freq - 261.63).abs() < 0.1,
        "C4 should be ~261.63 Hz, got {freq}"
    );
}

#[test]
fn from_midi_roundtrip() {
    let note = Note::from_midi(69);
    assert_eq!(note, Note::A4);
    assert_eq!(note.to_freq(), Note::A4.to_freq());
}

#[test]
fn from_freq_nearest() {
    let note = Note::from_freq(442.0); // slightly sharp A4
    assert_eq!(note, Note::A4);
}

#[test]
fn from_freq_c4() {
    let note = Note::from_freq(261.63);
    assert_eq!(note, Note::C4);
}

// ===================== Transpose tests =====================

#[test]
fn transpose_up() {
    assert_eq!(Note::C4.transpose(4), Note::E4); // C4 + 4 semitones = E4
}

#[test]
fn transpose_down() {
    assert_eq!(Note::A4.transpose(-12), Note::from_midi(57)); // A3
}

#[test]
fn up_octave() {
    assert_eq!(Note::C4.up_octave(), Note::C5);
}

#[test]
fn down_octave() {
    assert_eq!(Note::C5.down_octave(), Note::C4);
}

#[test]
fn transpose_clamps() {
    // Can't go below 0.
    let note = Note::from_midi(5).transpose(-10);
    assert_eq!(note.midi(), 0);
    // Can't go above 127.
    let note = Note::from_midi(120).transpose(20);
    assert_eq!(note.midi(), 127);
}

// ===================== Pitch class and octave =====================

#[test]
fn pitch_class() {
    assert_eq!(Note::C4.pitch_class(), 0);
    assert_eq!(Note::A4.pitch_class(), 9);
    assert_eq!(Note::FS4.pitch_class(), 6);
}

#[test]
fn octave() {
    assert_eq!(Note::C4.octave(), 4);
    assert_eq!(Note::A4.octave(), 4);
    assert_eq!(Note::A0.octave(), 0);
}

// ===================== Note parsing =====================

#[test]
fn parse_simple() {
    assert_eq!(Note::parse("C4"), Some(Note::C4));
    assert_eq!(Note::parse("A4"), Some(Note::A4));
}

#[test]
fn parse_sharp() {
    assert_eq!(Note::parse("C#4"), Some(Note::CS4));
    assert_eq!(Note::parse("F#4"), Some(Note::FS4));
}

#[test]
fn parse_flat() {
    // Bb2 = A#2 = MIDI 46
    assert_eq!(Note::parse("Bb2"), Some(Note::from_midi(46)));
    // Db3 = C#3 = MIDI 49
    assert_eq!(Note::parse("Db3"), Some(Note::from_midi(49)));
}

#[test]
fn parse_lowercase() {
    assert_eq!(Note::parse("c4"), Some(Note::C4));
    assert_eq!(Note::parse("a4"), Some(Note::A4));
}

#[test]
fn parse_invalid() {
    assert_eq!(Note::parse("X4"), None);
    assert_eq!(Note::parse(""), None);
    assert_eq!(Note::parse("C"), None); // no octave
}

// ===================== Note display =====================

#[test]
fn display_note() {
    assert_eq!(Note::C4.to_string(), "C4");
    assert_eq!(Note::A4.to_string(), "A4");
    assert_eq!(Note::CS4.to_string(), "C#4");
}

// ===================== Scale tests =====================

#[test]
fn c_major_notes() {
    let scale = Scale::major("C");
    let notes = scale.notes_in_range(Note::C4, Note::B4);
    let midi: Vec<u8> = notes.iter().map(|n| n.midi()).collect();
    // C D E F G A B
    assert_eq!(midi, vec![60, 62, 64, 65, 67, 69, 71]);
}

#[test]
fn a_minor_notes() {
    let scale = Scale::minor("A");
    let notes = scale.notes_in_range(Note::A4, Note::from_midi(81)); // A4 to A5
    let midi: Vec<u8> = notes.iter().map(|n| n.midi()).collect();
    // A B C D E F G A
    assert_eq!(midi, vec![69, 71, 72, 74, 76, 77, 79, 81]);
}

#[test]
fn c_pentatonic() {
    let scale = Scale::pentatonic("C");
    let notes = scale.notes_in_range(Note::C4, Note::B4);
    let midi: Vec<u8> = notes.iter().map(|n| n.midi()).collect();
    // C D E G A
    assert_eq!(midi, vec![60, 62, 64, 67, 69]);
}

#[test]
fn all_modes_have_intervals() {
    let modes = [
        ScaleMode::Major,
        ScaleMode::Minor,
        ScaleMode::PentatonicMajor,
        ScaleMode::PentatonicMinor,
        ScaleMode::Dorian,
        ScaleMode::Phrygian,
        ScaleMode::Lydian,
        ScaleMode::Mixolydian,
        ScaleMode::Locrian,
        ScaleMode::WholeTone,
        ScaleMode::Chromatic,
    ];
    for mode in modes {
        let scale = Scale::new("C", mode);
        assert!(!scale.intervals().is_empty(), "{mode:?} has no intervals");
    }
}

// ===================== Scale snap =====================

#[test]
fn snap_c_to_c_major() {
    let scale = Scale::major("C");
    assert_eq!(scale.snap(60.0), Note::C4); // C → C
}

#[test]
fn snap_c_sharp_to_c_major() {
    let scale = Scale::major("C");
    // C#4 (61) should snap to C4 (60) or D4 (62) — nearest.
    let snapped = scale.snap(61.0);
    assert!(
        snapped == Note::C4 || snapped == Note::D4,
        "C#4 should snap to C4 or D4, got {snapped}"
    );
}

#[test]
fn snap_preserves_scale_notes() {
    let scale = Scale::major("C");
    // All notes of C major should snap to themselves.
    for midi in [60, 62, 64, 65, 67, 69, 71] {
        let snapped = scale.snap(midi as f32);
        assert_eq!(
            snapped.midi(),
            midi,
            "MIDI {midi} should stay, got {}",
            snapped.midi()
        );
    }
}

#[test]
fn snap_freq_works() {
    let scale = Scale::major("C");
    let freq = scale.snap_freq(445.0); // slightly sharp A4
    let expected = Note::A4.to_freq();
    assert!(
        (freq - expected).abs() < 0.1,
        "snap_freq(445) should give A4 freq, got {freq}"
    );
}

// ===================== Chord tests =====================

#[test]
fn c_major_chord() {
    let chord = Chord::major(Note::C4);
    let notes = chord.notes();
    let midi: Vec<u8> = notes.iter().map(|n| n.midi()).collect();
    assert_eq!(midi, vec![60, 64, 67]); // C E G
}

#[test]
fn a_minor_chord() {
    let chord = Chord::minor(Note::A4);
    let notes = chord.notes();
    let midi: Vec<u8> = notes.iter().map(|n| n.midi()).collect();
    assert_eq!(midi, vec![69, 72, 76]); // A C E
}

#[test]
fn g_dom7_chord() {
    let chord = Chord::dom7(Note::G4);
    let notes = chord.notes();
    let midi: Vec<u8> = notes.iter().map(|n| n.midi()).collect();
    assert_eq!(midi, vec![67, 71, 74, 77]); // G B D F
}

#[test]
fn all_chord_types() {
    let types = [
        (ChordType::Major, 3),
        (ChordType::Minor, 3),
        (ChordType::Diminished, 3),
        (ChordType::Augmented, 3),
        (ChordType::Major7, 4),
        (ChordType::Minor7, 4),
        (ChordType::Dominant7, 4),
        (ChordType::Sus2, 3),
        (ChordType::Sus4, 3),
    ];
    for (ct, expected_len) in types {
        let chord = Chord::new(Note::C4, ct);
        assert_eq!(
            chord.notes().len(),
            expected_len,
            "{ct:?} should have {expected_len} notes"
        );
    }
}

#[test]
fn chord_transpose() {
    let chord = Chord::major(Note::C4).transpose(7); // C → G
    let notes = chord.notes();
    let midi: Vec<u8> = notes.iter().map(|n| n.midi()).collect();
    assert_eq!(midi, vec![67, 71, 74]); // G B D
}

#[test]
fn chord_freqs() {
    let chord = Chord::major(Note::A4);
    let freqs = chord.freqs();
    assert_eq!(freqs.len(), 3);
    assert!((freqs[0] - 440.0).abs() < 0.1); // A4
}

// ===================== Integration: scale + note + chord =====================

#[test]
fn chord_notes_are_in_scale() {
    let scale = Scale::major("C");
    let chord = Chord::major(Note::C4);
    let scale_notes = scale.notes_in_range(Note::C4, Note::B4);
    for note in chord.notes() {
        assert!(
            scale_notes.contains(&note),
            "Chord note {note} not in C major scale"
        );
    }
}
