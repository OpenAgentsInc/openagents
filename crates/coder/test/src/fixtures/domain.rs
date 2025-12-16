//! Domain fixture for setting up domain entities.

use chrono::{DateTime, Utc};
use coder_domain::{Message, Role, SessionId, ThreadId};

/// Simple session struct for testing (not from coder_domain which stores in DB).
#[derive(Debug, Clone)]
pub struct Session {
    pub id: SessionId,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Simple thread struct for testing (not from coder_domain which stores in DB).
#[derive(Debug, Clone)]
pub struct Thread {
    pub id: ThreadId,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Fixture for setting up domain model state.
///
/// Provides convenient methods for creating sessions, threads,
/// and messages for testing.
pub struct DomainFixture {
    /// Created sessions.
    sessions: Vec<Session>,
    /// Created threads.
    threads: Vec<Thread>,
    /// Created messages.
    messages: Vec<Message>,
    /// Current working session.
    current_session: Option<SessionId>,
    /// Current working thread.
    current_thread: Option<ThreadId>,
}

impl DomainFixture {
    /// Create a new domain fixture.
    pub fn new() -> Self {
        Self {
            sessions: Vec::new(),
            threads: Vec::new(),
            messages: Vec::new(),
            current_session: None,
            current_thread: None,
        }
    }

    /// Create a session with default settings.
    pub fn with_session(&mut self, id: &str) -> &mut Self {
        let session_id = SessionId::new();
        let session = Session {
            id: session_id,
            title: format!("Test Session {}", id),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        self.sessions.push(session);
        self.current_session = Some(session_id);
        self
    }

    /// Create a thread with default settings.
    pub fn with_thread(&mut self, id: &str) -> &mut Self {
        let thread_id = ThreadId::new();
        let thread = Thread {
            id: thread_id,
            title: format!("Test Thread {}", id),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        self.threads.push(thread);
        self.current_thread = Some(thread_id);
        self
    }

    /// Add a user message to the current thread.
    pub fn with_user_message(&mut self, content: &str) -> &mut Self {
        self.add_message(Role::User, content)
    }

    /// Add an assistant message to the current thread.
    pub fn with_assistant_message(&mut self, content: &str) -> &mut Self {
        self.add_message(Role::Assistant, content)
    }

    /// Add multiple messages at once.
    pub fn with_messages(&mut self, messages: &[(&str, &str)]) -> &mut Self {
        for (role, content) in messages {
            let role = match *role {
                "user" => Role::User,
                "assistant" => Role::Assistant,
                "system" => Role::System,
                _ => Role::User,
            };
            self.add_message(role, content);
        }
        self
    }

    /// Add a message to the current thread.
    fn add_message(&mut self, role: Role, content: &str) -> &mut Self {
        // Ensure we have a thread
        if self.current_thread.is_none() {
            self.with_thread("default");
        }

        let message = Message::new(role, content);
        self.messages.push(message);
        self
    }

    /// Get all sessions.
    pub fn sessions(&self) -> &[Session] {
        &self.sessions
    }

    /// Get all threads.
    pub fn threads(&self) -> &[Thread] {
        &self.threads
    }

    /// Get all messages.
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// Get the current session ID.
    pub fn current_session_id(&self) -> Option<SessionId> {
        self.current_session
    }

    /// Get the current thread ID.
    pub fn current_thread_id(&self) -> Option<ThreadId> {
        self.current_thread
    }

    /// Get the current session.
    pub fn current_session(&self) -> Option<&Session> {
        self.current_session
            .and_then(|id| self.sessions.iter().find(|s| s.id == id))
    }

    /// Get the current thread.
    pub fn current_thread(&self) -> Option<&Thread> {
        self.current_thread
            .and_then(|id| self.threads.iter().find(|t| t.id == id))
    }

    /// Get the last message.
    pub fn last_message(&self) -> Option<&Message> {
        self.messages.last()
    }

    /// Get message count.
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Assert message count.
    pub fn assert_message_count(&self, expected: usize) {
        let actual = self.messages.len();
        assert_eq!(
            actual, expected,
            "Message count {} != expected {}",
            actual, expected
        );
    }

    /// Assert last message content.
    pub fn assert_last_message(&self, expected_content: &str) {
        match self.last_message() {
            Some(msg) => assert_eq!(
                msg.content, expected_content,
                "Last message '{}' != expected '{}'",
                msg.content, expected_content
            ),
            None => panic!("No messages, expected '{}'", expected_content),
        }
    }

    /// Assert last message role.
    pub fn assert_last_message_role(&self, expected_role: Role) {
        match self.last_message() {
            Some(msg) => assert_eq!(
                msg.role, expected_role,
                "Last message role {:?} != expected {:?}",
                msg.role, expected_role
            ),
            None => panic!("No messages"),
        }
    }

    /// Clear all data.
    pub fn clear(&mut self) {
        self.sessions.clear();
        self.threads.clear();
        self.messages.clear();
        self.current_session = None;
        self.current_thread = None;
    }
}

impl Default for DomainFixture {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_fixture_session() {
        let mut fixture = DomainFixture::new();
        fixture.with_session("test");

        assert_eq!(fixture.sessions().len(), 1);
        assert!(fixture.current_session_id().is_some());
    }

    #[test]
    fn test_domain_fixture_thread() {
        let mut fixture = DomainFixture::new();
        fixture.with_thread("main");

        assert_eq!(fixture.threads().len(), 1);
        assert!(fixture.current_thread_id().is_some());
    }

    #[test]
    fn test_domain_fixture_messages() {
        let mut fixture = DomainFixture::new();
        fixture
            .with_session("test")
            .with_thread("main")
            .with_user_message("Hello")
            .with_assistant_message("Hi there!");

        assert_eq!(fixture.message_count(), 2);
        fixture.assert_message_count(2);

        let last = fixture.last_message().unwrap();
        assert_eq!(last.content, "Hi there!");
        assert_eq!(last.role, Role::Assistant);
    }

    #[test]
    fn test_domain_fixture_bulk_messages() {
        let mut fixture = DomainFixture::new();
        fixture.with_messages(&[
            ("user", "First message"),
            ("assistant", "Second message"),
            ("user", "Third message"),
        ]);

        assert_eq!(fixture.message_count(), 3);
    }

    #[test]
    fn test_domain_fixture_clear() {
        let mut fixture = DomainFixture::new();
        fixture
            .with_session("test")
            .with_thread("main")
            .with_user_message("Hello");

        fixture.clear();

        assert!(fixture.sessions().is_empty());
        assert!(fixture.threads().is_empty());
        assert!(fixture.messages().is_empty());
    }
}
