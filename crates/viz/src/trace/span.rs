//! Span types for structured tracing

use serde::{Deserialize, Serialize};

use super::Venue;

/// Kind of span
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum SpanKind {
    /// A request to an inference backend
    Request,
    /// A tool call within a request
    ToolCall,
    /// A routing decision
    Routing,
    /// Model loading
    ModelLoad,
    /// Data fetching
    Fetch,
    /// GPU kernel execution
    Kernel,
}

/// A span representing a timed operation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Span {
    /// Unique span ID
    pub id: String,
    /// Parent span ID (if nested)
    pub parent_id: Option<String>,
    /// Kind of span
    pub kind: SpanKind,
    /// Human-readable label
    pub label: String,
    /// Execution venue
    pub venue: Venue,
    /// Start timestamp (ms since epoch)
    pub start_ms: u64,
    /// End timestamp (ms since epoch), None if still running
    pub end_ms: Option<u64>,
    /// Whether the span completed successfully
    pub success: Option<bool>,
    /// Additional metadata
    pub metadata: std::collections::HashMap<String, String>,
}

impl Span {
    pub fn new(id: impl Into<String>, kind: SpanKind, label: impl Into<String>, venue: Venue) -> Self {
        Self {
            id: id.into(),
            parent_id: None,
            kind,
            label: label.into(),
            venue,
            start_ms: 0,
            end_ms: None,
            success: None,
            metadata: std::collections::HashMap::new(),
        }
    }

    pub fn with_parent(mut self, parent_id: impl Into<String>) -> Self {
        self.parent_id = Some(parent_id.into());
        self
    }

    pub fn with_start(mut self, start_ms: u64) -> Self {
        self.start_ms = start_ms;
        self
    }

    pub fn duration_ms(&self) -> Option<u64> {
        self.end_ms.map(|end| end.saturating_sub(self.start_ms))
    }

    pub fn is_complete(&self) -> bool {
        self.end_ms.is_some()
    }

    pub fn complete(&mut self, end_ms: u64, success: bool) {
        self.end_ms = Some(end_ms);
        self.success = Some(success);
    }
}
