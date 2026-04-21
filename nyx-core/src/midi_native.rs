//! Native MIDI backend — thin wrapper around `midir`.
//!
//! Provides the `open_midi_input*` / `MidiConnection` surface on
//! non-wasm targets. The wasm backend lives in `midi_web.rs`.

use crate::midi::{midi_bridge, parse_midi, MidiError, MidiReceiver};

/// A live native MIDI connection. MIDI input stops when this is dropped.
pub struct MidiConnection {
    _connection: midir::MidiInputConnection<()>,
}

/// Open the first available MIDI input port and start forwarding events.
///
/// Returns a `MidiReceiver` for the audio thread and a `MidiConnection`
/// that must be kept alive (MIDI stops when it is dropped).
pub fn open_midi_input() -> Result<(MidiReceiver, MidiConnection), MidiError> {
    open_midi_input_named(None)
}

/// Open a MIDI input port by name substring (or first available if `None`).
pub fn open_midi_input_named(
    name_filter: Option<&str>,
) -> Result<(MidiReceiver, MidiConnection), MidiError> {
    let midi_in =
        midir::MidiInput::new("nyx-midi-in").map_err(|e| MidiError::Init(e.to_string()))?;

    let ports = midi_in.ports();
    if ports.is_empty() {
        return Err(MidiError::NoPort);
    }

    let port = if let Some(filter) = name_filter {
        ports
            .iter()
            .find(|p| midi_in.port_name(p).unwrap_or_default().contains(filter))
            .ok_or(MidiError::NoPort)?
    } else {
        &ports[0]
    };

    let (mut sender, receiver) = midi_bridge(256);

    let connection = midi_in
        .connect(
            port,
            "nyx-midi",
            move |_timestamp, data, _| {
                if let Some(event) = parse_midi(data) {
                    sender.send(event);
                }
            },
            (),
        )
        .map_err(|e| MidiError::Connect(e.to_string()))?;

    Ok((
        receiver,
        MidiConnection {
            _connection: connection,
        },
    ))
}
