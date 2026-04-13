use nyx_core::{render_to_buffer, AudioContext, Signal};
use nyx_seq::inst;
use nyx_seq::{
    Chord, ChordType, Note, SubSynth, SynthPatch, OscShape, FilterType,
};

const SR: f32 = 44100.0;

fn ctx(tick: u64) -> AudioContext {
    AudioContext {
        sample_rate: SR,
        tick,
    }
}

// ===================== Instrument tests =====================

#[test]
fn kick_produces_sound() {
    let mut k = inst::kick();
    k.trigger();
    let buf = render_to_buffer(&mut k, 0.2, SR);
    let rms: f32 = (buf.iter().map(|s| s * s).sum::<f32>() / buf.len() as f32).sqrt();
    assert!(rms > 0.01, "kick should produce sound, rms={rms}");
}

#[test]
fn kick_silent_without_trigger() {
    let mut k = inst::kick();
    let buf = render_to_buffer(&mut k, 0.1, SR);
    let rms: f32 = (buf.iter().map(|s| s * s).sum::<f32>() / buf.len() as f32).sqrt();
    assert!(rms < 0.001, "kick should be silent without trigger, rms={rms}");
}

#[test]
fn kick_output_bounded() {
    let mut k = inst::kick();
    k.trigger();
    let buf = render_to_buffer(&mut k, 0.3, SR);
    for &s in &buf {
        assert!(s.abs() <= 1.5, "kick sample out of range: {s}");
    }
}

#[test]
fn snare_produces_sound() {
    let mut s = inst::snare();
    s.trigger();
    let buf = render_to_buffer(&mut s, 0.2, SR);
    let rms: f32 = (buf.iter().map(|s| s * s).sum::<f32>() / buf.len() as f32).sqrt();
    assert!(rms > 0.01, "snare should produce sound, rms={rms}");
}

#[test]
fn hihat_closed_shorter_than_open() {
    let mut closed = inst::hihat(false);
    closed.trigger();
    let buf_closed = render_to_buffer(&mut closed, 0.5, SR);

    let mut open = inst::hihat(true);
    open.trigger();
    let buf_open = render_to_buffer(&mut open, 0.5, SR);

    // Measure energy in the second half — open should have more.
    let half = buf_closed.len() / 2;
    let energy_closed: f32 = buf_closed[half..].iter().map(|s| s * s).sum();
    let energy_open: f32 = buf_open[half..].iter().map(|s| s * s).sum();
    assert!(
        energy_open > energy_closed,
        "open hihat should sustain longer: open={energy_open}, closed={energy_closed}"
    );
}

#[test]
fn drone_produces_continuous_sound() {
    let mut d = inst::drone(Note::A4);
    let buf = render_to_buffer(&mut d, 0.5, SR);
    // Check first quarter and last quarter both have energy.
    let quarter = buf.len() / 4;
    let rms_start: f32 =
        (buf[..quarter].iter().map(|s| s * s).sum::<f32>() / quarter as f32).sqrt();
    let rms_end: f32 =
        (buf[buf.len() - quarter..].iter().map(|s| s * s).sum::<f32>() / quarter as f32).sqrt();
    assert!(rms_start > 0.1, "drone start too quiet: {rms_start}");
    assert!(rms_end > 0.1, "drone end too quiet: {rms_end}");
}

#[test]
fn riser_amplitude_increases() {
    let mut r = inst::riser(1.0);
    let buf = render_to_buffer(&mut r, 1.0, SR);
    let quarter = buf.len() / 4;
    let rms_start: f32 =
        (buf[..quarter].iter().map(|s| s * s).sum::<f32>() / quarter as f32).sqrt();
    let rms_end: f32 =
        (buf[buf.len() - quarter..].iter().map(|s| s * s).sum::<f32>() / quarter as f32).sqrt();
    assert!(
        rms_end > rms_start * 2.0,
        "riser should get louder: start={rms_start}, end={rms_end}"
    );
}

#[test]
fn pad_produces_sound_when_triggered() {
    let chord = Chord::major(Note::C4);
    let mut p = inst::pad(chord);
    p.trigger();
    let buf = render_to_buffer(&mut p, 0.5, SR);
    let rms: f32 = (buf.iter().map(|s| s * s).sum::<f32>() / buf.len() as f32).sqrt();
    assert!(rms > 0.01, "pad should produce sound, rms={rms}");
}

#[test]
fn pad_silent_without_trigger() {
    let chord = Chord::minor(Note::A4);
    let mut p = inst::pad(chord);
    let buf = render_to_buffer(&mut p, 0.1, SR);
    let rms: f32 = (buf.iter().map(|s| s * s).sum::<f32>() / buf.len() as f32).sqrt();
    assert!(rms < 0.001, "pad should be silent without trigger, rms={rms}");
}

// ===================== SubSynth tests =====================

#[test]
fn subsynth_default_patch() {
    let patch = SynthPatch::default();
    assert_eq!(patch.osc_shape, OscShape::Saw);
    assert_eq!(patch.filter_type, FilterType::LowPass);
    assert!((patch.frequency - 440.0).abs() < 0.01);
}

#[test]
fn subsynth_produces_sound() {
    let mut synth = SynthPatch::default().build();
    synth.trigger();
    let buf = render_to_buffer(&mut synth, 0.2, SR);
    let rms: f32 = (buf.iter().map(|s| s * s).sum::<f32>() / buf.len() as f32).sqrt();
    assert!(rms > 0.01, "subsynth should produce sound, rms={rms}");
}

#[test]
fn subsynth_silent_without_trigger() {
    let mut synth = SynthPatch::default().build();
    let buf = render_to_buffer(&mut synth, 0.1, SR);
    let rms: f32 = (buf.iter().map(|s| s * s).sum::<f32>() / buf.len() as f32).sqrt();
    assert!(rms < 0.001, "subsynth should be silent without trigger");
}

#[test]
fn subsynth_release_decays() {
    let mut patch = SynthPatch::default();
    patch.attack = 0.001;
    patch.decay = 0.001;
    patch.sustain = 0.8;
    patch.release = 0.05;

    let mut synth = patch.build();
    synth.trigger();
    let _ = render_to_buffer(&mut synth, 0.05, SR); // reach sustain
    synth.release();
    let buf = render_to_buffer(&mut synth, 0.2, SR);

    // Last quarter should be nearly silent.
    let quarter = buf.len() / 4;
    let rms_end: f32 =
        (buf[buf.len() - quarter..].iter().map(|s| s * s).sum::<f32>() / quarter as f32).sqrt();
    assert!(rms_end < 0.05, "should decay after release, rms_end={rms_end}");
}

#[test]
fn subsynth_all_osc_shapes() {
    for shape in [OscShape::Sine, OscShape::Saw, OscShape::Square, OscShape::Triangle] {
        let mut patch = SynthPatch::default();
        patch.osc_shape = shape;
        let mut synth = patch.build();
        synth.trigger();
        let buf = render_to_buffer(&mut synth, 0.05, SR);
        let rms: f32 = (buf.iter().map(|s| s * s).sum::<f32>() / buf.len() as f32).sqrt();
        assert!(rms > 0.01, "{shape:?} should produce sound, rms={rms}");
    }
}

#[test]
fn subsynth_bypass_filter() {
    let mut patch = SynthPatch::default();
    patch.filter_type = FilterType::Bypass;
    let mut synth = patch.build();
    synth.trigger();
    let buf = render_to_buffer(&mut synth, 0.05, SR);
    let rms: f32 = (buf.iter().map(|s| s * s).sum::<f32>() / buf.len() as f32).sqrt();
    assert!(rms > 0.01, "bypass filter should still produce sound");
}

#[test]
fn subsynth_set_frequency() {
    let mut synth = SynthPatch::default().build();
    synth.set_frequency(880.0);
    synth.trigger();
    let buf = render_to_buffer(&mut synth, 0.05, SR);
    assert!(!buf.is_empty());
}

// ===================== SynthPatch serialisation tests =====================

#[test]
fn patch_serialise_roundtrip() {
    let patch = SynthPatch {
        name: "TestPad".to_string(),
        osc_shape: OscShape::Triangle,
        frequency: 330.0,
        filter_type: FilterType::HighPass,
        filter_cutoff: 500.0,
        filter_q: 1.5,
        attack: 0.1,
        decay: 0.2,
        sustain: 0.6,
        release: 0.4,
        gain: 0.9,
    };

    let toml_str = toml::to_string_pretty(&patch).unwrap();
    let restored: SynthPatch = toml::from_str(&toml_str).unwrap();

    assert_eq!(restored.name, "TestPad");
    assert_eq!(restored.osc_shape, OscShape::Triangle);
    assert!((restored.frequency - 330.0).abs() < 0.01);
    assert_eq!(restored.filter_type, FilterType::HighPass);
    assert!((restored.filter_cutoff - 500.0).abs() < 0.01);
    assert!((restored.gain - 0.9).abs() < 0.01);
}

#[test]
fn patch_save_and_load() {
    let patch = SynthPatch {
        name: "SaveTest".to_string(),
        ..SynthPatch::default()
    };

    let path = "/tmp/nyx_test_patch.toml";
    patch.save(path).unwrap();

    let loaded = SynthPatch::load(path).unwrap();
    assert_eq!(loaded.name, "SaveTest");
    assert_eq!(loaded.osc_shape, patch.osc_shape);
    assert!((loaded.frequency - patch.frequency).abs() < 0.01);

    // Cleanup
    let _ = std::fs::remove_file(path);
}

#[test]
fn patch_build_produces_same_sound() {
    let patch = SynthPatch::default();
    let mut synth1 = patch.build();
    let mut synth2 = patch.build();

    synth1.trigger();
    synth2.trigger();

    let buf1 = render_to_buffer(&mut synth1, 0.05, SR);
    let buf2 = render_to_buffer(&mut synth2, 0.05, SR);

    for (i, (a, b)) in buf1.iter().zip(buf2.iter()).enumerate() {
        assert!(
            (a - b).abs() < 1e-6,
            "same patch should produce same output, sample {i}: {a} vs {b}"
        );
    }
}
