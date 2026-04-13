use nyx_core::{
    AudioContext, Signal,
    parse_midi, midi_bridge, CcMap, MidiEvent,
};
use nyx_core::osc_input::OscParam;

const SR: f32 = 44100.0;

fn ctx(tick: u64) -> AudioContext {
    AudioContext {
        sample_rate: SR,
        tick,
    }
}

// ===================== MIDI parsing =====================

#[test]
fn parse_note_on() {
    let data = [0x90, 60, 100]; // channel 0, note 60, velocity 100
    let event = parse_midi(&data).unwrap();
    match event {
        MidiEvent::NoteOn { channel, note, velocity } => {
            assert_eq!(channel, 0);
            assert_eq!(note, 60);
            assert_eq!(velocity, 100);
        }
        _ => panic!("expected NoteOn"),
    }
}

#[test]
fn parse_note_on_velocity_zero_is_off() {
    let data = [0x90, 60, 0]; // velocity 0 = note off
    let event = parse_midi(&data).unwrap();
    assert!(matches!(event, MidiEvent::NoteOff { note: 60, .. }));
}

#[test]
fn parse_note_off() {
    let data = [0x80, 60, 64];
    let event = parse_midi(&data).unwrap();
    assert!(matches!(event, MidiEvent::NoteOff { note: 60, .. }));
}

#[test]
fn parse_cc() {
    let data = [0xB0, 1, 64]; // CC1 = 64 on channel 0
    let event = parse_midi(&data).unwrap();
    match event {
        MidiEvent::ControlChange { channel, cc, value } => {
            assert_eq!(channel, 0);
            assert_eq!(cc, 1);
            assert_eq!(value, 64);
        }
        _ => panic!("expected CC"),
    }
}

#[test]
fn parse_channel_extraction() {
    let data = [0x95, 60, 100]; // note on, channel 5
    let event = parse_midi(&data).unwrap();
    match event {
        MidiEvent::NoteOn { channel, .. } => assert_eq!(channel, 5),
        _ => panic!("expected NoteOn"),
    }
}

#[test]
fn parse_short_data_returns_none() {
    assert!(parse_midi(&[0x90]).is_none());
    assert!(parse_midi(&[]).is_none());
}

#[test]
fn parse_unknown_status_returns_none() {
    assert!(parse_midi(&[0xF0, 0, 0]).is_none()); // sysex
}

// ===================== MIDI bridge =====================

#[test]
fn midi_bridge_send_receive() {
    let (mut tx, mut rx) = midi_bridge(16);
    tx.send(MidiEvent::NoteOn { channel: 0, note: 60, velocity: 100 });
    tx.send(MidiEvent::ControlChange { channel: 0, cc: 1, value: 64 });

    let events: Vec<_> = rx.drain().collect();
    assert_eq!(events.len(), 2);
    assert!(matches!(events[0], MidiEvent::NoteOn { note: 60, .. }));
    assert!(matches!(events[1], MidiEvent::ControlChange { cc: 1, .. }));
}

#[test]
fn midi_bridge_empty_drain() {
    let (_tx, mut rx) = midi_bridge(16);
    assert_eq!(rx.drain().count(), 0);
}

// ===================== CC Map =====================

#[test]
fn cc_map_read_write() {
    let cc_map = CcMap::new();
    let writer = cc_map.writer();

    writer.set(1, 64);
    assert_eq!(cc_map.get(1), 64);
    assert!((cc_map.get_normalized(1) - 0.5039).abs() < 0.01);
}

#[test]
fn cc_map_defaults_to_zero() {
    let cc_map = CcMap::new();
    for cc in 0..128 {
        assert_eq!(cc_map.get(cc), 0);
    }
}

#[test]
fn cc_signal_smooths_value() {
    let cc_map = CcMap::new();
    let writer = cc_map.writer();
    let mut sig = cc_map.signal(1, 5.0); // 5ms smoothing

    // Initial value = 0
    let v0 = sig.next(&ctx(0));
    assert!(v0.abs() < 0.01, "should start at ~0, got {v0}");

    // Set CC to max
    writer.set(1, 127);

    // After a few samples, should be moving toward 1.0 but not there yet
    let mut v = 0.0;
    for tick in 1..100 {
        v = sig.next(&ctx(tick));
    }
    assert!(v > 0.0 && v < 1.0, "should be smoothing, got {v}");

    // After many samples, should converge to ~1.0
    for tick in 100..10000 {
        v = sig.next(&ctx(tick));
    }
    assert!(
        (v - 1.0).abs() < 0.01,
        "should converge to ~1.0, got {v}"
    );
}

#[test]
fn cc_signal_instant_smoothing() {
    let cc_map = CcMap::new();
    let writer = cc_map.writer();
    let mut sig = cc_map.signal(1, 0.0); // instant

    writer.set(1, 127);
    let v = sig.next(&ctx(0));
    assert!(
        (v - 1.0).abs() < 0.01,
        "instant smoothing should jump to target, got {v}"
    );
}

// ===================== OSC Param =====================

#[test]
fn osc_param_read_write() {
    let param = OscParam::new(0.5);
    assert!((param.get() - 0.5).abs() < 1e-6);

    let writer = param.writer();
    writer.set(0.75);
    assert!((param.get() - 0.75).abs() < 1e-6);
}

#[test]
fn osc_signal_smooths() {
    let param = OscParam::new(0.0);
    let writer = param.writer();
    let mut sig = param.signal(5.0);

    // Start at 0
    let v0 = sig.next(&ctx(0));
    assert!(v0.abs() < 0.01);

    // Jump to 1.0
    writer.set(1.0);

    let mut v = 0.0;
    for tick in 1..10000 {
        v = sig.next(&ctx(tick));
    }
    assert!(
        (v - 1.0).abs() < 0.01,
        "OSC signal should converge to 1.0, got {v}"
    );
}

// ===================== MicSignal (unit test, no hardware) =====================

#[test]
fn mic_signal_outputs_silence_when_empty() {
    // We can't open a real mic in CI, but we can test the signal type.
    // MicSignal reads from a consumer — if empty, outputs 0.
    let (producer, consumer) = rtrb::RingBuffer::new(64);
    let mut sig = nyx_core::mic::MicSignal::from_consumer(consumer);
    assert!(sig.next(&ctx(0)).abs() < 1e-10);
}
