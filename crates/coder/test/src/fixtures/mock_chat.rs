//! Mock chat service for testing AI interactions.

use std::collections::VecDeque;

/// A mock response from the chat service.
#[derive(Debug, Clone)]
pub enum MockResponse {
    /// A simple text response.
    Text(String),
    /// A streaming response (chunks).
    Streaming(Vec<String>),
    /// A tool use response.
    ToolUse {
        tool_name: String,
        input: serde_json::Value,
    },
    /// An error response.
    Error(String),
}

impl MockResponse {
    /// Create a text response.
    pub fn text(content: impl Into<String>) -> Self {
        MockResponse::Text(content.into())
    }

    /// Create a streaming response from chunks.
    pub fn streaming(chunks: &[&str]) -> Self {
        MockResponse::Streaming(chunks.iter().map(|s| s.to_string()).collect())
    }

    /// Create a tool use response.
    pub fn tool_use(name: impl Into<String>, input: serde_json::Value) -> Self {
        MockResponse::ToolUse {
            tool_name: name.into(),
            input,
        }
    }

    /// Create an error response.
    pub fn error(message: impl Into<String>) -> Self {
        MockResponse::Error(message.into())
    }
}

/// Mock chat service for testing AI interactions.
///
/// Allows queueing responses that will be returned in order
/// when messages are sent.
pub struct MockChatService {
    /// Queued responses.
    responses: VecDeque<MockResponse>,
    /// Sent messages (for verification).
    sent_messages: Vec<String>,
    /// Whether the service is available.
    available: bool,
    /// Simulated latency in milliseconds.
    latency_ms: u64,
}

impl MockChatService {
    /// Create a new mock chat service.
    pub fn new() -> Self {
        Self {
            responses: VecDeque::new(),
            sent_messages: Vec::new(),
            available: true,
            latency_ms: 0,
        }
    }

    /// Queue a response to be returned on the next message.
    pub fn queue_response(&mut self, response: MockResponse) {
        self.responses.push_back(response);
    }

    /// Queue a simple text response.
    pub fn queue_text(&mut self, content: impl Into<String>) {
        self.queue_response(MockResponse::text(content));
    }

    /// Queue a streaming response.
    pub fn queue_streaming(&mut self, chunks: &[&str]) {
        self.queue_response(MockResponse::streaming(chunks));
    }

    /// Queue a tool use response.
    pub fn queue_tool_use(&mut self, name: impl Into<String>, input: serde_json::Value) {
        self.queue_response(MockResponse::tool_use(name, input));
    }

    /// Queue an error response.
    pub fn queue_error(&mut self, message: impl Into<String>) {
        self.queue_response(MockResponse::error(message));
    }

    /// Queue multiple responses at once.
    pub fn queue_responses(&mut self, responses: impl IntoIterator<Item = MockResponse>) {
        for response in responses {
            self.queue_response(response);
        }
    }

    /// Send a message and get the next queued response.
    pub fn send_message(&mut self, message: impl Into<String>) -> Option<MockResponse> {
        self.sent_messages.push(message.into());

        if !self.available {
            return Some(MockResponse::error("Service unavailable"));
        }

        // Simulate latency
        if self.latency_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(self.latency_ms));
        }

        self.responses.pop_front()
    }

    /// Get all sent messages.
    pub fn sent_messages(&self) -> &[String] {
        &self.sent_messages
    }

    /// Get the last sent message.
    pub fn last_sent_message(&self) -> Option<&str> {
        self.sent_messages.last().map(|s| s.as_str())
    }

    /// Get the number of sent messages.
    pub fn sent_count(&self) -> usize {
        self.sent_messages.len()
    }

    /// Get the number of remaining queued responses.
    pub fn remaining_responses(&self) -> usize {
        self.responses.len()
    }

    /// Check if all queued responses have been consumed.
    pub fn all_responses_consumed(&self) -> bool {
        self.responses.is_empty()
    }

    /// Set service availability.
    pub fn set_available(&mut self, available: bool) {
        self.available = available;
    }

    /// Simulate the service going down.
    pub fn go_down(&mut self) {
        self.available = false;
    }

    /// Simulate the service coming back up.
    pub fn come_up(&mut self) {
        self.available = true;
    }

    /// Set simulated latency.
    pub fn set_latency(&mut self, ms: u64) {
        self.latency_ms = ms;
    }

    /// Clear all state.
    pub fn clear(&mut self) {
        self.responses.clear();
        self.sent_messages.clear();
    }

    // Assertions

    /// Assert that a message was sent.
    pub fn assert_message_sent(&self, expected: &str) {
        assert!(
            self.sent_messages.iter().any(|m| m == expected),
            "Expected message '{}' to be sent, but it wasn't.\nSent messages: {:?}",
            expected,
            self.sent_messages
        );
    }

    /// Assert that a message containing substring was sent.
    pub fn assert_message_contains(&self, substring: &str) {
        assert!(
            self.sent_messages.iter().any(|m| m.contains(substring)),
            "Expected message containing '{}' to be sent.\nSent messages: {:?}",
            substring,
            self.sent_messages
        );
    }

    /// Assert exactly N messages were sent.
    pub fn assert_sent_count(&self, expected: usize) {
        let actual = self.sent_messages.len();
        assert_eq!(
            actual, expected,
            "Expected {} messages sent, got {}",
            expected, actual
        );
    }

    /// Assert all queued responses were consumed.
    pub fn assert_all_consumed(&self) {
        assert!(
            self.responses.is_empty(),
            "Expected all responses to be consumed, but {} remain",
            self.responses.len()
        );
    }

    /// Assert last sent message equals.
    pub fn assert_last_message(&self, expected: &str) {
        match self.last_sent_message() {
            Some(actual) => assert_eq!(
                actual, expected,
                "Last message '{}' != expected '{}'",
                actual, expected
            ),
            None => panic!("No messages sent, expected '{}'", expected),
        }
    }
}

impl Default for MockChatService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_chat_service_basic() {
        let mut service = MockChatService::new();

        service.queue_text("Hello!");
        service.queue_text("How can I help?");

        let resp1 = service.send_message("Hi");
        let resp2 = service.send_message("Help me");

        assert!(matches!(resp1, Some(MockResponse::Text(t)) if t == "Hello!"));
        assert!(matches!(resp2, Some(MockResponse::Text(t)) if t == "How can I help?"));

        assert_eq!(service.sent_count(), 2);
        service.assert_all_consumed();
    }

    #[test]
    fn test_mock_chat_service_streaming() {
        let mut service = MockChatService::new();

        service.queue_streaming(&["Hello", " ", "world", "!"]);

        let resp = service.send_message("Hi");

        match resp {
            Some(MockResponse::Streaming(chunks)) => {
                assert_eq!(chunks.len(), 4);
                assert_eq!(chunks.join(""), "Hello world!");
            }
            _ => panic!("Expected streaming response"),
        }
    }

    #[test]
    fn test_mock_chat_service_tool_use() {
        let mut service = MockChatService::new();

        service.queue_tool_use(
            "read_file",
            serde_json::json!({ "path": "/tmp/test.txt" }),
        );

        let resp = service.send_message("Read the file");

        match resp {
            Some(MockResponse::ToolUse { tool_name, input }) => {
                assert_eq!(tool_name, "read_file");
                assert_eq!(input["path"], "/tmp/test.txt");
            }
            _ => panic!("Expected tool use response"),
        }
    }

    #[test]
    fn test_mock_chat_service_unavailable() {
        let mut service = MockChatService::new();
        service.go_down();

        let resp = service.send_message("Hi");

        assert!(matches!(resp, Some(MockResponse::Error(_))));
    }

    #[test]
    fn test_mock_chat_service_assertions() {
        let mut service = MockChatService::new();
        service.queue_text("Response");

        service.send_message("Hello world");

        service.assert_message_sent("Hello world");
        service.assert_message_contains("Hello");
        service.assert_sent_count(1);
        service.assert_last_message("Hello world");
    }

    #[test]
    fn test_mock_chat_service_no_response() {
        let mut service = MockChatService::new();

        let resp = service.send_message("Hi");

        assert!(resp.is_none());
    }
}
