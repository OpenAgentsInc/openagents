//! Event broadcast system for real-time event delivery
//!
//! Uses tokio::sync::broadcast to distribute new events to all active connections.
//! Each connection filters events against its subscriptions and delivers matches.

use nostr::Event;
use tokio::sync::broadcast;

/// Default capacity for the broadcast channel
const BROADCAST_CAPACITY: usize = 1000;

/// Broadcast message containing an event
#[derive(Debug, Clone)]
pub struct BroadcastEvent {
    pub event: Event,
}

/// Creates a new broadcast channel for events
pub fn create_broadcast_channel() -> (
    broadcast::Sender<BroadcastEvent>,
    broadcast::Receiver<BroadcastEvent>,
) {
    broadcast::channel(BROADCAST_CAPACITY)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::{EventTemplate, finalize_event, generate_secret_key};

    fn create_test_event(content: &str) -> Event {
        let secret_key = generate_secret_key();
        let template = EventTemplate {
            kind: 1,
            tags: vec![],
            content: content.to_string(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };
        finalize_event(&template, &secret_key).unwrap()
    }

    #[tokio::test]
    async fn test_broadcast_channel() {
        let (tx, mut rx) = create_broadcast_channel();

        let event = create_test_event("test");
        let broadcast = BroadcastEvent {
            event: event.clone(),
        };

        tx.send(broadcast).unwrap();

        let received = rx.recv().await.unwrap();
        assert_eq!(received.event.content, "test");
    }

    #[tokio::test]
    async fn test_multiple_receivers() {
        let (tx, _rx1) = create_broadcast_channel();
        let mut rx2 = tx.subscribe();
        let mut rx3 = tx.subscribe();

        let event = create_test_event("broadcast test");
        let broadcast = BroadcastEvent {
            event: event.clone(),
        };

        tx.send(broadcast).unwrap();

        let received2 = rx2.recv().await.unwrap();
        let received3 = rx3.recv().await.unwrap();

        assert_eq!(received2.event.content, "broadcast test");
        assert_eq!(received3.event.content, "broadcast test");
    }
}
