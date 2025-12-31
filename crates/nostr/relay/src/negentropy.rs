//! Negentropy session management for NIP-77 support
//!
//! Manages active Negentropy sync sessions between clients and the relay.
//! Each session tracks reconciliation state and has a timeout.

use nostr::nip77::{ReconciliationState, Record};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Unique identifier for a Negentropy session
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionId {
    /// Connection identifier (e.g., socket address)
    pub connection_id: String,
    /// Subscription ID from NEG-OPEN
    pub subscription_id: String,
}

impl SessionId {
    pub fn new(connection_id: String, subscription_id: String) -> Self {
        Self {
            connection_id,
            subscription_id,
        }
    }
}

/// Active Negentropy sync session
pub struct NegentropySession {
    /// Reconciliation state
    pub state: ReconciliationState,
    /// When this session was created
    pub created_at: Instant,
    /// Last activity timestamp
    pub last_active: Instant,
}

impl NegentropySession {
    pub fn new(records: Vec<Record>) -> Self {
        let now = Instant::now();
        Self {
            state: ReconciliationState::new(records),
            created_at: now,
            last_active: now,
        }
    }

    /// Update last activity timestamp
    pub fn touch(&mut self) {
        self.last_active = Instant::now();
    }

    /// Check if session has exceeded timeout
    pub fn is_expired(&self, timeout: Duration) -> bool {
        self.last_active.elapsed() > timeout
    }
}

/// Manager for all active Negentropy sessions
pub struct NegentropySessionManager {
    /// Active sessions
    sessions: HashMap<SessionId, NegentropySession>,
    /// Session timeout duration
    timeout: Duration,
}

impl NegentropySessionManager {
    /// Create new session manager with default 60s timeout
    pub fn new() -> Self {
        Self::with_timeout(Duration::from_secs(60))
    }

    /// Create session manager with custom timeout
    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            sessions: HashMap::new(),
            timeout,
        }
    }

    /// Create a new session
    pub fn create_session(&mut self, session_id: SessionId, records: Vec<Record>) {
        let session = NegentropySession::new(records);
        self.sessions.insert(session_id, session);
    }

    /// Get mutable reference to a session
    pub fn get_session_mut(&mut self, session_id: &SessionId) -> Option<&mut NegentropySession> {
        let session = self.sessions.get_mut(session_id)?;
        session.touch();
        Some(session)
    }

    /// Remove a session
    pub fn remove_session(&mut self, session_id: &SessionId) -> Option<NegentropySession> {
        self.sessions.remove(session_id)
    }

    /// Remove all sessions for a connection
    pub fn remove_connection(&mut self, connection_id: &str) {
        self.sessions
            .retain(|id, _| id.connection_id != connection_id);
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

impl Default for NegentropySessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_session_creation() {
        let mut manager = NegentropySessionManager::new();
        let session_id = SessionId::new("conn1".to_string(), "sub1".to_string());

        manager.create_session(session_id.clone(), vec![]);
        assert_eq!(manager.session_count(), 1);

        let session = manager.get_session_mut(&session_id);
        assert!(session.is_some());
    }

    #[test]
    fn test_session_removal() {
        let mut manager = NegentropySessionManager::new();
        let session_id = SessionId::new("conn1".to_string(), "sub1".to_string());

        manager.create_session(session_id.clone(), vec![]);
        assert_eq!(manager.session_count(), 1);

        manager.remove_session(&session_id);
        assert_eq!(manager.session_count(), 0);
    }

    #[test]
    fn test_connection_removal() {
        let mut manager = NegentropySessionManager::new();

        manager.create_session(
            SessionId::new("conn1".to_string(), "sub1".to_string()),
            vec![],
        );
        manager.create_session(
            SessionId::new("conn1".to_string(), "sub2".to_string()),
            vec![],
        );
        manager.create_session(
            SessionId::new("conn2".to_string(), "sub3".to_string()),
            vec![],
        );
        assert_eq!(manager.session_count(), 3);

        manager.remove_connection("conn1");
        assert_eq!(manager.session_count(), 1);
    }

    #[test]
    fn test_session_expiration() {
        let mut manager = NegentropySessionManager::with_timeout(Duration::from_millis(50));
        let session_id = SessionId::new("conn1".to_string(), "sub1".to_string());

        manager.create_session(session_id.clone(), vec![]);
        assert_eq!(manager.session_count(), 1);

        // Wait for expiration
        sleep(Duration::from_millis(60));

        let cleaned = manager.cleanup_expired();
        assert_eq!(cleaned, 1);
        assert_eq!(manager.session_count(), 0);
    }

    #[test]
    fn test_session_touch_prevents_expiration() {
        let mut manager = NegentropySessionManager::with_timeout(Duration::from_millis(100));
        let session_id = SessionId::new("conn1".to_string(), "sub1".to_string());

        manager.create_session(session_id.clone(), vec![]);

        // Touch session after 50ms
        sleep(Duration::from_millis(50));
        manager.get_session_mut(&session_id); // This touches the session

        // Wait another 60ms (110ms total, but only 60ms since last touch)
        sleep(Duration::from_millis(60));

        let cleaned = manager.cleanup_expired();
        assert_eq!(cleaned, 0);
        assert_eq!(manager.session_count(), 1);
    }
}
