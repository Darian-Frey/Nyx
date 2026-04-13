//! OSC (Open Sound Control) input via `rosc`.
//!
//! Gated behind the `osc` feature. Listens on a UDP port and forwards
//! OSC messages to the audio thread via a lock-free bridge.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use crate::signal::{AudioContext, Signal};

// ─── OSC Value Store ────────────────────────────────────────────────

/// A named OSC parameter that can be read as a `Signal`.
///
/// The value is stored as atomic u32 bits (f32 reinterpreted) so
/// it can be written from the network thread and read from the audio
/// thread without locks.
pub struct OscParam {
    value: Arc<AtomicU32>,
}

impl OscParam {
    /// Create a new OSC parameter with an initial value.
    pub fn new(initial: f32) -> Self {
        OscParam {
            value: Arc::new(AtomicU32::new(initial.to_bits())),
        }
    }

    /// Get a writer handle for the network thread.
    pub fn writer(&self) -> OscParamWriter {
        OscParamWriter {
            value: Arc::clone(&self.value),
        }
    }

    /// Read the current value.
    pub fn get(&self) -> f32 {
        f32::from_bits(self.value.load(Ordering::Relaxed))
    }

    /// Create a `Signal` that reads this parameter with smoothing.
    pub fn signal(&self, smooth_ms: f32) -> OscSignal {
        OscSignal {
            value: Arc::clone(&self.value),
            smooth_ms,
            current: self.get(),
            coeff: 0.0,
            initialised: false,
        }
    }
}

/// Writer handle for the OSC network thread.
pub struct OscParamWriter {
    value: Arc<AtomicU32>,
}

impl OscParamWriter {
    /// Set the parameter value (called from the network thread).
    pub fn set(&self, v: f32) {
        self.value.store(v.to_bits(), Ordering::Relaxed);
    }
}

/// A `Signal` that reads an OSC parameter with one-pole smoothing.
pub struct OscSignal {
    value: Arc<AtomicU32>,
    smooth_ms: f32,
    current: f32,
    coeff: f32,
    initialised: bool,
}

impl Signal for OscSignal {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        if !self.initialised {
            let samples = self.smooth_ms * 0.001 * ctx.sample_rate;
            self.coeff = if samples > 0.0 {
                1.0 - (-1.0 / samples).exp()
            } else {
                1.0
            };
            self.current = f32::from_bits(self.value.load(Ordering::Relaxed));
            self.initialised = true;
        }

        let target = f32::from_bits(self.value.load(Ordering::Relaxed));
        self.current += self.coeff * (target - self.current);
        self.current
    }
}

// ─── OSC Listener (requires rosc) ───────────────────────────────────

/// Start an OSC listener on the given UDP port.
///
/// Returns a handle that can be used to register address patterns
/// and map them to `OscParamWriter`s. The listener runs in a
/// background thread.
///
/// Messages with a single float argument are supported. Other
/// argument types are ignored.
#[cfg(feature = "osc")]
pub fn osc_listen(
    addr: &str,
    mappings: Vec<(String, OscParamWriter)>,
) -> Result<OscListener, OscError> {
    use std::net::UdpSocket;

    let socket = UdpSocket::bind(addr)
        .map_err(|e| OscError::Bind(e.to_string()))?;
    // Non-blocking would be complex; use a dedicated thread.
    let socket_clone = socket.try_clone()
        .map_err(|e| OscError::Bind(e.to_string()))?;

    let running = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let running_clone = Arc::clone(&running);

    let handle = std::thread::spawn(move || {
        let mut buf = [0u8; 1024];
        // Set a read timeout so we can check the running flag.
        let _ = socket_clone.set_read_timeout(Some(std::time::Duration::from_millis(100)));

        while running_clone.load(Ordering::Relaxed) {
            let Ok((size, _)) = socket_clone.recv_from(&mut buf) else {
                continue;
            };

            if let Ok((_, packet)) = rosc::decoder::decode_udp(&buf[..size]) {
                dispatch_packet(&packet, &mappings);
            }
        }
    });

    Ok(OscListener {
        _socket: socket,
        running,
        _thread: Some(handle),
    })
}

#[cfg(feature = "osc")]
fn dispatch_packet(
    packet: &rosc::OscPacket,
    mappings: &[(String, OscParamWriter)],
) {
    match packet {
        rosc::OscPacket::Message(msg) => {
            for (addr, writer) in mappings {
                if msg.addr == *addr {
                    if let Some(rosc::OscType::Float(v)) = msg.args.first() {
                        writer.set(*v);
                    }
                }
            }
        }
        rosc::OscPacket::Bundle(bundle) => {
            for p in &bundle.content {
                dispatch_packet(p, mappings);
            }
        }
    }
}

/// A running OSC listener. Stops when dropped.
#[cfg(feature = "osc")]
pub struct OscListener {
    _socket: std::net::UdpSocket,
    running: Arc<std::sync::atomic::AtomicBool>,
    _thread: Option<std::thread::JoinHandle<()>>,
}

#[cfg(feature = "osc")]
impl Drop for OscListener {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self._thread.take() {
            let _ = handle.join();
        }
    }
}

/// Errors from OSC listener setup.
#[derive(Debug)]
pub enum OscError {
    Bind(String),
}

impl std::fmt::Display for OscError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OscError::Bind(e) => write!(f, "OSC bind error: {e}"),
        }
    }
}

impl std::error::Error for OscError {}
