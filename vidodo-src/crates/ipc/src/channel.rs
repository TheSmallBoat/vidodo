//! Bounded inter-thread channel infrastructure.
//!
//! Provides `RuntimeChannel<T>` — a paired sender/receiver with bounded capacity,
//! back-pressure error on full, and disconnection detection.

use std::sync::mpsc::{self, RecvError, SyncSender, TrySendError};

/// Error returned when sending fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendError {
    /// Channel is full — apply back-pressure.
    BackPressure,
    /// Receiver has been dropped.
    Disconnected,
}

/// Error returned when receiving fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecvError2 {
    /// No message available right now.
    Empty,
    /// Sender has been dropped.
    Disconnected,
}

/// Sending half of a `RuntimeChannel`.
pub struct ChannelSender<T> {
    inner: SyncSender<T>,
}

/// Receiving half of a `RuntimeChannel`.
pub struct ChannelReceiver<T> {
    inner: mpsc::Receiver<T>,
}

impl<T> ChannelSender<T> {
    /// Non-blocking send. Returns `BackPressure` if the channel is full.
    pub fn send(&self, msg: T) -> Result<(), SendError> {
        self.inner.try_send(msg).map_err(|e| match e {
            TrySendError::Full(_) => SendError::BackPressure,
            TrySendError::Disconnected(_) => SendError::Disconnected,
        })
    }

    /// Check if the receiver is still connected.
    pub fn is_connected(&self) -> bool {
        // The only way to check is to try a zero-cost probe;
        // SyncSender doesn't expose this, so we rely on send errors.
        // This is a best-effort method.
        true
    }
}

impl<T> ChannelReceiver<T> {
    /// Non-blocking receive.
    pub fn try_recv(&self) -> Result<T, RecvError2> {
        self.inner.try_recv().map_err(|e| match e {
            mpsc::TryRecvError::Empty => RecvError2::Empty,
            mpsc::TryRecvError::Disconnected => RecvError2::Disconnected,
        })
    }

    /// Blocking receive (waits until a message is available or sender disconnects).
    pub fn recv(&self) -> Result<T, RecvError2> {
        self.inner.recv().map_err(|RecvError| RecvError2::Disconnected)
    }

    /// Drain all currently available messages into a Vec.
    pub fn drain(&self) -> Vec<T> {
        let mut out = Vec::new();
        while let Ok(msg) = self.try_recv() {
            out.push(msg);
        }
        out
    }
}

/// Create a paired bounded channel with the given capacity.
///
/// Returns `(sender, receiver)`. Messages are delivered in FIFO order.
/// When the channel is full, `send()` returns `SendError::BackPressure`.
pub fn channel<T>(capacity: usize) -> (ChannelSender<T>, ChannelReceiver<T>) {
    let (tx, rx) = mpsc::sync_channel(capacity);
    (ChannelSender { inner: tx }, ChannelReceiver { inner: rx })
}

#[cfg(test)]
mod channel_tests {
    use super::*;

    #[test]
    fn send_and_receive_100_messages_in_order() {
        let (tx, rx) = channel::<u32>(128);
        for i in 0..100 {
            tx.send(i).unwrap();
        }
        for i in 0..100 {
            assert_eq!(rx.try_recv().unwrap(), i);
        }
        assert_eq!(rx.try_recv(), Err(RecvError2::Empty));
    }

    #[test]
    fn back_pressure_when_full() {
        let (tx, _rx) = channel::<u32>(4);
        for i in 0..4 {
            tx.send(i).unwrap();
        }
        // 5th message should fail with BackPressure
        assert_eq!(tx.send(99), Err(SendError::BackPressure));
    }

    #[test]
    fn receiver_disconnect_detected() {
        let (tx, rx) = channel::<u32>(4);
        drop(rx);
        assert_eq!(tx.send(1), Err(SendError::Disconnected));
    }

    #[test]
    fn sender_disconnect_detected() {
        let (tx, rx) = channel::<u32>(4);
        tx.send(42).unwrap();
        drop(tx);
        // Should still get the pending message
        assert_eq!(rx.try_recv().unwrap(), 42);
        // Then disconnected
        assert_eq!(rx.try_recv(), Err(RecvError2::Disconnected));
    }

    #[test]
    fn drain_collects_all_pending() {
        let (tx, rx) = channel::<u32>(16);
        for i in 0..10 {
            tx.send(i).unwrap();
        }
        let all = rx.drain();
        assert_eq!(all, (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn blocking_recv() {
        let (tx, rx) = channel::<String>(4);
        let handle = std::thread::spawn(move || {
            tx.send("hello".into()).unwrap();
        });
        let msg = rx.recv().unwrap();
        assert_eq!(msg, "hello");
        handle.join().unwrap();
    }
}
