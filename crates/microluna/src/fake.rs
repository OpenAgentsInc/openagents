//! A transport that answers from a script, for tests.

use std::cell::RefCell;
use std::collections::VecDeque;

use serde_json::{Value, json};

use crate::transport::{Reply, Request, TokenUsage, Transport, TransportError};

/// Answers each request with the next scripted reply, and keeps every
/// request it was sent so a test can inspect what the session built.
#[derive(Debug, Default)]
pub struct FakeTransport {
    script: RefCell<VecDeque<Result<Reply, TransportError>>>,
    requests: RefCell<Vec<Request>>,
}

impl FakeTransport {
    /// A transport that answers with `replies`, in order.
    #[must_use]
    pub fn new(replies: Vec<Reply>) -> Self {
        FakeTransport {
            script: RefCell::new(replies.into_iter().map(Ok).collect()),
            requests: RefCell::default(),
        }
    }

    /// Adds a failure as the next answer.
    pub fn then_fail(&self, error: TransportError) {
        self.script.borrow_mut().push_back(Err(error));
    }

    /// The requests sent so far.
    #[must_use]
    pub fn requests(&self) -> Vec<Request> {
        self.requests.borrow().clone()
    }
}

impl Transport for FakeTransport {
    async fn respond(&self, request: &Request) -> Result<Reply, TransportError> {
        self.requests.borrow_mut().push(request.clone());
        self.script
            .borrow_mut()
            .pop_front()
            .unwrap_or(Err(TransportError::Exhausted))
    }
}

/// A reply that makes one function call.
#[must_use]
pub fn call(call_id: &str, name: &str, arguments: &Value, usage: TokenUsage) -> Reply {
    Reply {
        id: Some(format!("resp-{call_id}")),
        model: "gpt-6-luna".to_string(),
        items: vec![json!({
            "type": "function_call",
            "id": format!("fc-{call_id}"),
            "call_id": call_id,
            "name": name,
            "arguments": arguments.to_string(),
        })],
        usage,
    }
}

/// A reply that only says something.
#[must_use]
pub fn say(text: &str, usage: TokenUsage) -> Reply {
    Reply {
        id: Some("resp-say".to_string()),
        model: "gpt-6-luna".to_string(),
        items: vec![json!({
            "type": "message",
            "role": "assistant",
            "content": [{ "type": "output_text", "text": text }],
        })],
        usage,
    }
}
