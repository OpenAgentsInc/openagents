//! Transport abstraction for replication bytes.
//!
//! Implementations:
//! - `LocalLoopbackTransport`: in-proc bounded channels for demo/local server
//! - (future) `WebSocketTransport`: remote connection

use crate::channel;

#[derive(Debug)]
pub enum TrySendError {
    Full,
    Disconnected,
}

/// Minimal transport trait for byte messages.
pub trait Transport: Send + Sync {
    fn try_send(&self, bytes: Vec<u8>) -> Result<(), TrySendError>;
    fn try_recv(&self) -> Option<Vec<u8>>;
    fn depth(&self) -> usize;
}

/// In-process loopback using crossbeam bounded channels.
#[derive(Clone)]
pub struct LocalLoopbackTransport {
    tx: channel::Tx,
    rx: channel::Rx,
}

impl LocalLoopbackTransport {
    #[must_use]
    pub fn new(capacity: usize) -> (Self, Self) {
        let (tx_a, rx_a) = channel::channel_bounded(capacity);
        let (tx_b, rx_b) = channel::channel_bounded(capacity);
        let a = Self {
            tx: tx_a,
            rx: rx_b.clone(),
        };
        let b = Self {
            tx: tx_b,
            rx: rx_a.clone(),
        };
        (a, b)
    }
    #[must_use]
    pub fn from_channel_pair(tx: channel::Tx, rx: channel::Rx) -> Self {
        Self { tx, rx }
    }
    #[must_use]
    pub fn split(self) -> (channel::Tx, channel::Rx) {
        (self.tx, self.rx)
    }
}

impl Transport for LocalLoopbackTransport {
    fn try_send(&self, bytes: Vec<u8>) -> Result<(), TrySendError> {
        if self.tx.try_send(bytes) {
            Ok(())
        } else {
            Err(TrySendError::Full)
        }
    }
    fn try_recv(&self) -> Option<Vec<u8>> {
        self.rx.try_recv()
    }
    fn depth(&self) -> usize {
        self.rx.depth()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn loopback_send_recv() {
        let (a, b) = LocalLoopbackTransport::new(2);
        a.try_send(b"ping".to_vec()).unwrap();
        b.try_send(b"pong".to_vec()).unwrap();
        assert_eq!(b.try_recv(), Some(b"ping".to_vec()));
        assert_eq!(a.try_recv(), Some(b"pong".to_vec()));
    }
}
