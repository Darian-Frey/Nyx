//! Oscilloscope tap: `.scope()` wraps a signal and copies samples into a
//! lock-free ring buffer that can be read from the UI thread.

use rtrb::{Consumer, Producer, RingBuffer};

use crate::signal::{AudioContext, Signal};

/// Handle for reading scope data from the UI/render thread.
///
/// Call `read()` to drain available samples into a slice.
pub struct ScopeHandle {
    consumer: Consumer<f32>,
}

impl ScopeHandle {
    /// Read available samples into `buf`. Returns the number of samples written.
    ///
    /// Non-blocking and allocation-free.
    pub fn read(&mut self, buf: &mut [f32]) -> usize {
        let mut count = 0;
        for slot in buf.iter_mut() {
            match self.consumer.pop() {
                Ok(sample) => {
                    *slot = sample;
                    count += 1;
                }
                Err(_) => break,
            }
        }
        count
    }

    /// Number of samples available to read.
    pub fn available(&self) -> usize {
        self.consumer.slots()
    }
}

/// A signal wrapper that copies every sample into a scope ring buffer.
pub struct Scope<A: Signal> {
    source: A,
    producer: Producer<f32>,
}

impl<A: Signal> Signal for Scope<A> {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let sample = self.source.next(ctx);
        // Best-effort: if the buffer is full, drop the sample silently.
        let _ = self.producer.push(sample);
        sample
    }
}

/// Extension trait adding `.scope()` to all signals.
pub trait ScopeExt: Signal + Sized {
    /// Tap this signal for oscilloscope display.
    ///
    /// `buffer_size` is the ring buffer capacity in samples.
    /// A good default is 2048–4096.
    ///
    /// Returns `(wrapped_signal, handle)`. Pass the signal to the audio
    /// engine and keep the handle for reading samples on the UI thread.
    fn scope(self, buffer_size: usize) -> (Scope<Self>, ScopeHandle) {
        let (producer, consumer) = RingBuffer::new(buffer_size);
        let scope = Scope {
            source: self,
            producer,
        };
        let handle = ScopeHandle { consumer };
        (scope, handle)
    }
}

impl<T: Signal + Sized> ScopeExt for T {}
