//! A transport that answers from a script, for tests.

use std::cell::RefCell;
use std::collections::VecDeque;

use serde_json::{Value, json};

use crate::transport::{Reply, Request, TokenUsage, Transport, TransportError};

/// Replies in the order they answer.
type Script = VecDeque<Result<Reply, TransportError>>;

/// Answers each request with the next scripted reply, and keeps every
/// request it was sent so a test can inspect what the session built.
///
/// Sessions that run at the same time share one transport, so their
/// requests interleave. A lane ([`FakeTransport::lane`]) keeps a script of
/// its own for the requests whose input mentions its marker, so each
/// concurrent session follows its own script whatever the interleaving.
#[derive(Debug, Default)]
pub struct FakeTransport {
    script: RefCell<Script>,
    lanes: RefCell<Vec<(String, Script)>>,
    requests: RefCell<Vec<Request>>,
}

impl FakeTransport {
    /// A transport that answers with `replies`, in order.
    #[must_use]
    pub fn new(replies: Vec<Reply>) -> Self {
        FakeTransport {
            script: RefCell::new(replies.into_iter().map(Ok).collect()),
            lanes: RefCell::default(),
            requests: RefCell::default(),
        }
    }

    /// Adds a failure as the next answer.
    pub fn then_fail(&self, error: TransportError) {
        self.script.borrow_mut().push_back(Err(error));
    }

    /// Adds a reply as the next answer.
    pub fn then(&self, reply: Reply) {
        self.script.borrow_mut().push_back(Ok(reply));
    }

    /// The requests sent so far.
    #[must_use]
    pub fn requests(&self) -> Vec<Request> {
        self.requests.borrow().clone()
    }

    /// Answers the requests whose input text contains `marker` with
    /// `replies`, in order, before the main script. Lanes are tried in the
    /// order they were added; a lane that ran out falls through.
    pub fn lane(&self, marker: &str, replies: Vec<Reply>) {
        self.lanes
            .borrow_mut()
            .push((marker.to_string(), replies.into_iter().map(Ok).collect()));
    }
}

impl Transport for FakeTransport {
    async fn respond(&self, request: &Request) -> Result<Reply, TransportError> {
        self.requests.borrow_mut().push(request.clone());
        let text = serde_json::to_string(&request.input).unwrap_or_default();
        for (marker, script) in self.lanes.borrow_mut().iter_mut() {
            if text.contains(marker.as_str())
                && let Some(reply) = script.pop_front()
            {
                return reply;
            }
        }
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
