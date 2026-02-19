//! NIP-77 Negentropy protocol support for efficient event syncing
//!
//! Provides client-side Negentropy implementation to efficiently sync events with relays.

use crate::error::Result;
use nostr::Event;
use nostr::nip77::{Bound, NegentropyMessage, Range, ReconciliationState, Record};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Unique identifier for a Negentropy sync session
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SyncSessionId {
    /// Relay URL
    pub relay_url: String,
    /// Subscription ID
    pub subscription_id: String,
}

impl SyncSessionId {
    pub fn new(relay_url: String, subscription_id: String) -> Self {
        Self {
            relay_url,
            subscription_id,
        }
    }
}

/// Active Negentropy sync session (client-side)
pub struct SyncSession {
    /// Reconciliation state
    pub state: ReconciliationState,
    /// Created timestamp
    pub created_at: Instant,
    /// Last activity
    pub last_active: Instant,
}

impl SyncSession {
    pub fn new(records: Vec<Record>) -> Self {
        let now = Instant::now();
        Self {
            state: ReconciliationState::new(records),
            created_at: now,
            last_active: now,
        }
    }

    pub fn touch(&mut self) {
        self.last_active = Instant::now();
    }

    pub fn is_expired(&self, timeout: Duration) -> bool {
        self.last_active.elapsed() > timeout
    }
}

/// Manager for client-side Negentropy sync sessions
pub struct SyncSessionManager {
    sessions: HashMap<SyncSessionId, SyncSession>,
    timeout: Duration,
}

impl SyncSessionManager {
    /// Create new manager with default 60s timeout
    pub fn new() -> Self {
        Self::with_timeout(Duration::from_secs(60))
    }

    /// Create manager with custom timeout
    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            sessions: HashMap::new(),
            timeout,
        }
    }

    /// Create a new sync session
    pub fn create_session(&mut self, session_id: SyncSessionId, records: Vec<Record>) {
        let session = SyncSession::new(records);
        self.sessions.insert(session_id, session);
    }

    /// Get mutable reference to a session
    pub fn get_session_mut(&mut self, session_id: &SyncSessionId) -> Option<&mut SyncSession> {
        let session = self.sessions.get_mut(session_id)?;
        session.touch();
        Some(session)
    }

    /// Remove a session
    pub fn remove_session(&mut self, session_id: &SyncSessionId) -> Option<SyncSession> {
        self.sessions.remove(session_id)
    }

    /// Remove all sessions for a relay
    pub fn remove_relay(&mut self, relay_url: &str) {
        self.sessions.retain(|id, _| id.relay_url != relay_url);
    }

    /// Remove expired sessions
    pub fn cleanup_expired(&mut self) -> usize {
        let timeout = self.timeout;
        let before_count = self.sessions.len();
        self.sessions
            .retain(|_, session| !session.is_expired(timeout));
        before_count - self.sessions.len()
    }

    /// Get active session count
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }
}

impl Default for SyncSessionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Build initial Negentropy message for a set of events
pub fn build_initial_message(events: &[Event]) -> Result<NegentropyMessage> {
    // Convert events to records
    let records: Vec<Record> = events
        .iter()
        .map(|event| {
            let id_bytes = hex::decode(&event.id).unwrap_or_else(|_| vec![0u8; 32]);
            let mut id = [0u8; 32];
            id.copy_from_slice(&id_bytes[..32.min(id_bytes.len())]);
            Record::new(event.created_at, id)
        })
        .collect();

    // Calculate fingerprint for all records
    let ids: Vec<[u8; 32]> = records.iter().map(|r| r.id).collect();
    let fingerprint = nostr::nip77::calculate_fingerprint(&ids);

    // Create single range covering everything
    let range = Range::fingerprint(Bound::infinity(), fingerprint);

    Ok(NegentropyMessage::new(vec![range]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_sync_session_creation() {
        let mut manager = SyncSessionManager::new();
        let session_id = SyncSessionId::new("wss://relay.test".to_string(), "sub1".to_string());

        manager.create_session(session_id.clone(), vec![]);
        assert_eq!(manager.session_count(), 1);

        let session = manager.get_session_mut(&session_id);
        assert!(session.is_some());
    }

    #[test]
    fn test_sync_session_removal() {
        let mut manager = SyncSessionManager::new();
        let session_id = SyncSessionId::new("wss://relay.test".to_string(), "sub1".to_string());

        manager.create_session(session_id.clone(), vec![]);
        assert_eq!(manager.session_count(), 1);

        manager.remove_session(&session_id);
        assert_eq!(manager.session_count(), 0);
    }

    #[test]
    fn test_relay_removal() {
        let mut manager = SyncSessionManager::new();

        manager.create_session(
            SyncSessionId::new("wss://relay1.test".to_string(), "sub1".to_string()),
            vec![],
        );
        manager.create_session(
            SyncSessionId::new("wss://relay1.test".to_string(), "sub2".to_string()),
            vec![],
        );
        manager.create_session(
            SyncSessionId::new("wss://relay2.test".to_string(), "sub3".to_string()),
            vec![],
        );
        assert_eq!(manager.session_count(), 3);

        manager.remove_relay("wss://relay1.test");
        assert_eq!(manager.session_count(), 1);
    }

    #[test]
    fn test_session_expiration() {
        let mut manager = SyncSessionManager::with_timeout(Duration::from_millis(50));
        let session_id = SyncSessionId::new("wss://relay.test".to_string(), "sub1".to_string());

        manager.create_session(session_id.clone(), vec![]);
        assert_eq!(manager.session_count(), 1);

        sleep(Duration::from_millis(60));

        let cleaned = manager.cleanup_expired();
        assert_eq!(cleaned, 1);
        assert_eq!(manager.session_count(), 0);
    }

    #[test]
    fn test_build_initial_message() {
        // Create test events
        let events = vec![];

        let message = build_initial_message(&events).unwrap();

        // Should have one range with fingerprint
        assert_eq!(message.ranges.len(), 1);
        assert!(matches!(
            message.ranges[0].payload,
            nostr::nip77::RangePayload::Fingerprint(_)
        ));
    }
}
