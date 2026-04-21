use nyx_core::{AudioCommand, bridge};

#[test]
fn send_and_drain_commands() {
    let (mut tx, mut rx) = bridge(16);
    tx.send(AudioCommand::SetGain(0.5)).unwrap();
    tx.send(AudioCommand::Stop).unwrap();

    let cmds: Vec<_> = rx.drain().collect();
    assert_eq!(cmds.len(), 2);
    assert!(matches!(cmds[0], AudioCommand::SetGain(g) if (g - 0.5).abs() < f32::EPSILON));
    assert!(matches!(cmds[1], AudioCommand::Stop));
}

#[test]
fn drain_empty_returns_nothing() {
    let (_tx, mut rx) = bridge(16);
    assert_eq!(rx.drain().count(), 0);
}

#[test]
fn full_buffer_returns_err() {
    let (mut tx, _rx) = bridge(2);
    assert!(tx.send(AudioCommand::SetGain(1.0)).is_ok());
    assert!(tx.send(AudioCommand::SetGain(1.0)).is_ok());
    // Buffer is full — third push should fail.
    assert!(tx.send(AudioCommand::SetGain(1.0)).is_err());
}
