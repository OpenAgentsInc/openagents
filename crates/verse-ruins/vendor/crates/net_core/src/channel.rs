//! Simple in-proc channel for replication messages (bytes).
//!
//! Bounded channel backed by `crossbeam-channel` to provide basic backpressure
//! and avoid unbounded memory growth under load. Exposes non-blocking helpers.

use crossbeam_channel as xchan;

#[derive(Clone)]
pub struct Tx(pub xchan::Sender<Vec<u8>>);
#[derive(Clone)]
pub struct Rx(pub xchan::Receiver<Vec<u8>>);

/// Default capacity for replication channels when not specified.
const DEFAULT_CAPACITY: usize = 4096;

/// Create a sender/receiver pair with default capacity.
#[must_use]
pub fn channel() -> (Tx, Rx) {
    channel_bounded(DEFAULT_CAPACITY)
}

/// Create a sender/receiver pair with the given bounded capacity.
#[must_use]
pub fn channel_bounded(capacity: usize) -> (Tx, Rx) {
    let (s, r) = xchan::bounded::<Vec<u8>>(capacity.max(1));
    (Tx(s), Rx(r))
}

impl Tx {
    /// Try to send; returns false if the receiver is dropped or the channel is full.
    #[must_use]
    pub fn try_send(&self, bytes: Vec<u8>) -> bool {
        match self.0.try_send(bytes) {
            Ok(()) => {
                metrics::counter!("replication.enqueued_total").increment(1);
                true
            }
            Err(xchan::TrySendError::Full(_bytes)) => {
                metrics::counter!("replication.dropped_total", "reason" => "full").increment(1);
                false
            }
            Err(xchan::TrySendError::Disconnected(_bytes)) => false,
        }
    }
}

impl Rx {
    /// Non-blocking receive of a single message.
    #[must_use]
    pub fn try_recv(&self) -> Option<Vec<u8>> {
        self.0.try_recv().ok()
    }
    /// Drain all currently queued messages.
    #[must_use]
    pub fn drain(&self) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        while let Some(b) = self.try_recv() {
            out.push(b);
        }
        out
    }
    /// Current approximate queue depth.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.0.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn send_and_drain() {
        let (tx, rx) = channel_bounded(4);
        assert!(tx.try_send(vec![1, 2, 3]));
        assert!(tx.try_send(vec![4, 5]));
        let drained = rx.drain();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0], vec![1, 2, 3]);
    }
    #[test]
    fn drops_when_full() {
        let (tx, rx) = channel_bounded(2);
        assert!(tx.try_send(b"a".to_vec()));
        assert!(tx.try_send(b"b".to_vec()));
        // Now channel is full; next try_send should return false (dropped).
        assert!(!tx.try_send(b"c".to_vec()));
        let drained = rx.drain();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0], b"a".to_vec());
        assert_eq!(drained[1], b"b".to_vec());
    }
}
