use rtrb::{Consumer, Producer, RingBuffer};

/// A command that can be sent from the main thread to the audio thread.
///
/// Extend this enum as new control messages are needed. All variants
/// must be `Send` and contain no heap allocations that the audio thread
/// would need to free.
#[derive(Debug)]
pub enum AudioCommand {
    /// Set the master gain (0.0 = silence, 1.0 = unity).
    SetGain(f32),
    /// Stop the audio stream gracefully.
    Stop,
}

/// Producer half — lives on the main thread.
///
/// Call `send()` to enqueue commands for the audio thread.
pub struct BridgeSender {
    producer: Producer<AudioCommand>,
}

impl BridgeSender {
    /// Send a command to the audio thread.
    ///
    /// Returns `Err` if the ring buffer is full (audio thread hasn't
    /// drained fast enough). Callers can retry or drop the command.
    pub fn send(&mut self, cmd: AudioCommand) -> Result<(), AudioCommand> {
        self.producer
            .push(cmd)
            .map_err(|rtrb::PushError::Full(v)| v)
    }
}

/// Consumer half — lives on the audio thread.
///
/// Call `recv()` in the audio callback to drain pending commands.
/// This never allocates and never blocks.
pub struct BridgeReceiver {
    consumer: Consumer<AudioCommand>,
}

impl BridgeReceiver {
    /// Drain all pending commands. Returns an iterator.
    ///
    /// Safe for the audio thread: no allocation, no locking, no blocking.
    pub fn drain(&mut self) -> impl Iterator<Item = AudioCommand> + '_ {
        std::iter::from_fn(move || self.consumer.pop().ok())
    }
}

/// Create a sender/receiver pair backed by a lock-free SPSC ring buffer.
///
/// `capacity` is the maximum number of commands that can be in flight
/// at once. 64 is a sensible default for most use cases.
pub fn bridge(capacity: usize) -> (BridgeSender, BridgeReceiver) {
    let (producer, consumer) = RingBuffer::new(capacity);
    (
        BridgeSender { producer },
        BridgeReceiver { consumer },
    )
}
