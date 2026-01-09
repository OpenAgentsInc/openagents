//! TraceContract for converting DSPy execution graphs to OpenTelemetry-compatible spans.
//!
//! This module provides:
//! - OTel-compatible span types (TraceSpan, SpanKind, SpanStatus)
//! - Graph-to-spans conversion
//! - Attribute mapping for DSPy-specific metadata

use crate::manifest::CompiledModuleManifest;
use crate::trace::{Graph, Node, NodeType};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// OpenTelemetry-compatible span kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SpanKind {
    /// Internal operation (default).
    Internal,
    /// Outgoing request (LM calls).
    Client,
    /// Message producer (job submissions).
    Producer,
    /// Incoming request.
    Server,
    /// Message consumer.
    Consumer,
}

impl Default for SpanKind {
    fn default() -> Self {
        Self::Internal
    }
}

/// OpenTelemetry-compatible span status.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SpanStatus {
    /// Status not set.
    Unset,
    /// Operation succeeded.
    Ok,
    /// Operation failed.
    Error,
}

impl Default for SpanStatus {
    fn default() -> Self {
        Self::Unset
    }
}

/// An OpenTelemetry-compatible trace span.
///
/// This structure is designed to be easily exported to:
/// - OTLP (OpenTelemetry Protocol)
/// - Jaeger
/// - Zipkin
/// - Cloud trace providers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceSpan {
    /// Unique trace ID (shared across all spans in a trace).
    pub trace_id: String,

    /// Unique span ID.
    pub span_id: String,

    /// Parent span ID (None for root spans).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_span_id: Option<String>,

    /// Human-readable span name.
    pub name: String,

    /// Span kind (client, server, internal, etc.).
    #[serde(default)]
    pub kind: SpanKind,

    /// Start time as Unix timestamp in nanoseconds.
    pub start_time_unix_nano: u64,

    /// End time as Unix timestamp in nanoseconds.
    pub end_time_unix_nano: u64,

    /// Span attributes (key-value pairs).
    #[serde(default)]
    pub attributes: HashMap<String, Value>,

    /// Span status.
    #[serde(default)]
    pub status: SpanStatus,

    /// Status message (if error).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,

    /// Events within the span.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<SpanEvent>,
}

/// An event within a span.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanEvent {
    /// Event name.
    pub name: String,

    /// Event timestamp in nanoseconds.
    pub time_unix_nano: u64,

    /// Event attributes.
    #[serde(default)]
    pub attributes: HashMap<String, Value>,
}

impl TraceSpan {
    /// Create a new span with basic information.
    pub fn new(trace_id: impl Into<String>, span_id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            trace_id: trace_id.into(),
            span_id: span_id.into(),
            parent_span_id: None,
            name: name.into(),
            kind: SpanKind::Internal,
            start_time_unix_nano: 0,
            end_time_unix_nano: 0,
            attributes: HashMap::new(),
            status: SpanStatus::Unset,
            status_message: None,
            events: Vec::new(),
        }
    }

    /// Set the parent span.
    pub fn with_parent(mut self, parent_id: impl Into<String>) -> Self {
        self.parent_span_id = Some(parent_id.into());
        self
    }

    /// Set the span kind.
    pub fn with_kind(mut self, kind: SpanKind) -> Self {
        self.kind = kind;
        self
    }

    /// Set timestamps.
    pub fn with_times(mut self, start_nano: u64, end_nano: u64) -> Self {
        self.start_time_unix_nano = start_nano;
        self.end_time_unix_nano = end_nano;
        self
    }

    /// Add an attribute.
    pub fn with_attribute(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.attributes.insert(key.into(), value.into());
        self
    }

    /// Set status to OK.
    pub fn ok(mut self) -> Self {
        self.status = SpanStatus::Ok;
        self
    }

    /// Set status to Error.
    pub fn error(mut self, message: impl Into<String>) -> Self {
        self.status = SpanStatus::Error;
        self.status_message = Some(message.into());
        self
    }

    /// Add an event.
    pub fn add_event(&mut self, event: SpanEvent) {
        self.events.push(event);
    }

    /// Calculate duration in milliseconds.
    pub fn duration_ms(&self) -> f64 {
        (self.end_time_unix_nano - self.start_time_unix_nano) as f64 / 1_000_000.0
    }
}

/// Converts DSPy execution graphs to OpenTelemetry-compatible spans.
pub struct TraceContract;

impl TraceContract {
    /// Convert a DSPy Graph to a list of TraceSpans.
    ///
    /// Each node in the graph becomes a span, with parent-child relationships
    /// preserved based on the graph structure.
    ///
    /// # Arguments
    ///
    /// * `graph` - The execution graph to convert
    /// * `manifest` - Optional compiled module manifest for additional metadata
    /// * `trace_id` - The trace ID to use for all spans
    ///
    /// # Returns
    ///
    /// A vector of TraceSpans representing the execution.
    pub fn graph_to_spans(
        graph: &Graph,
        manifest: Option<&CompiledModuleManifest>,
        trace_id: impl Into<String>,
    ) -> Vec<TraceSpan> {
        let trace_id = trace_id.into();
        let now_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;

        graph
            .nodes
            .iter()
            .map(|node| Self::node_to_span(node, &trace_id, manifest, now_ns))
            .collect()
    }

    /// Convert a single Node to a TraceSpan.
    fn node_to_span(
        node: &Node,
        trace_id: &str,
        manifest: Option<&CompiledModuleManifest>,
        base_time_ns: u64,
    ) -> TraceSpan {
        let span_id = format!("{:016x}", node.id);
        let parent_span_id = node.inputs.first().map(|id| format!("{:016x}", id));

        let (name, kind, attributes) = Self::extract_node_info(node, manifest);

        let mut span = TraceSpan::new(trace_id, span_id, name)
            .with_kind(kind)
            .with_times(base_time_ns, base_time_ns); // Actual times not available from graph

        // Set parent if not root
        if let Some(parent_id) = parent_span_id {
            span = span.with_parent(parent_id);
        }

        // Add attributes
        for (key, value) in attributes {
            span = span.with_attribute(key, value);
        }

        // Add standard DSPy attributes
        span = span.with_attribute("dsrs.node_id", Value::from(node.id));

        // Add manifest attributes if available
        if let Some(m) = manifest {
            span = span
                .with_attribute("dsrs.compiled_id", Value::from(m.compiled_id.clone().unwrap_or_default()))
                .with_attribute("dsrs.optimizer", Value::from(m.optimizer.clone()));
        }

        // Set status based on output
        if node.output.is_some() {
            span = span.ok();
        }

        span
    }

    /// Extract name, kind, and attributes from a node.
    fn extract_node_info(
        node: &Node,
        _manifest: Option<&CompiledModuleManifest>,
    ) -> (String, SpanKind, HashMap<String, Value>) {
        let mut attributes = HashMap::new();

        match &node.node_type {
            NodeType::Root => {
                attributes.insert("dsrs.node_type".to_string(), Value::from("root"));
                ("dsrs.root".to_string(), SpanKind::Internal, attributes)
            }
            NodeType::Predict { signature_name, .. } => {
                attributes.insert("dsrs.node_type".to_string(), Value::from("predict"));
                attributes.insert("dsrs.signature_name".to_string(), Value::from(signature_name.clone()));

                // Add token usage if available
                if let Some(output) = &node.output {
                    attributes.insert(
                        "lm.prompt_tokens".to_string(),
                        Value::from(output.lm_usage.prompt_tokens),
                    );
                    attributes.insert(
                        "lm.completion_tokens".to_string(),
                        Value::from(output.lm_usage.completion_tokens),
                    );
                    attributes.insert(
                        "lm.total_tokens".to_string(),
                        Value::from(output.lm_usage.total_tokens),
                    );
                    attributes.insert(
                        "lm.cost_msats".to_string(),
                        Value::from(output.lm_usage.cost_msats),
                    );
                }

                (
                    format!("dsrs.predict.{}", signature_name),
                    SpanKind::Client, // LM calls are client spans
                    attributes,
                )
            }
            NodeType::Operator { name } => {
                attributes.insert("dsrs.node_type".to_string(), Value::from("operator"));
                attributes.insert("dsrs.operator_name".to_string(), Value::from(name.clone()));

                (
                    format!("dsrs.operator.{}", name),
                    SpanKind::Internal,
                    attributes,
                )
            }
            NodeType::Map { mapping } => {
                attributes.insert("dsrs.node_type".to_string(), Value::from("map"));
                attributes.insert("dsrs.mapping_count".to_string(), Value::from(mapping.len()));

                ("dsrs.map".to_string(), SpanKind::Internal, attributes)
            }
        }
    }

    /// Create a summary of the trace.
    pub fn summarize(spans: &[TraceSpan]) -> TraceSummary {
        let mut total_tokens = 0u64;
        let mut total_cost_msats = 0u64;
        let mut predict_count = 0;

        for span in spans {
            if span.kind == SpanKind::Client {
                predict_count += 1;
                if let Some(tokens) = span.attributes.get("lm.total_tokens") {
                    if let Some(t) = tokens.as_u64() {
                        total_tokens += t;
                    }
                }
                if let Some(cost) = span.attributes.get("lm.cost_msats") {
                    if let Some(c) = cost.as_u64() {
                        total_cost_msats += c;
                    }
                }
            }
        }

        TraceSummary {
            span_count: spans.len(),
            predict_count,
            total_tokens,
            total_cost_msats,
        }
    }
}

/// Summary statistics for a trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceSummary {
    /// Total number of spans.
    pub span_count: usize,
    /// Number of LM prediction spans.
    pub predict_count: usize,
    /// Total tokens used.
    pub total_tokens: u64,
    /// Total cost in millisatoshis.
    pub total_cost_msats: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::Graph;
    use std::sync::Arc;

    #[test]
    fn test_span_creation() {
        let span = TraceSpan::new("trace-123", "span-456", "test-operation")
            .with_kind(SpanKind::Client)
            .with_attribute("key", "value")
            .ok();

        assert_eq!(span.trace_id, "trace-123");
        assert_eq!(span.span_id, "span-456");
        assert_eq!(span.kind, SpanKind::Client);
        assert_eq!(span.status, SpanStatus::Ok);
    }

    #[test]
    fn test_span_with_parent() {
        let span = TraceSpan::new("trace", "span", "name").with_parent("parent-span");

        assert_eq!(span.parent_span_id, Some("parent-span".to_string()));
    }

    #[test]
    fn test_span_error() {
        let span = TraceSpan::new("trace", "span", "name").error("Something went wrong");

        assert_eq!(span.status, SpanStatus::Error);
        assert_eq!(span.status_message, Some("Something went wrong".to_string()));
    }

    #[test]
    fn test_graph_to_spans_empty() {
        let graph = Graph::new();
        let spans = TraceContract::graph_to_spans(&graph, None, "trace-id");

        assert!(spans.is_empty());
    }

    #[test]
    fn test_graph_to_spans_with_nodes() {
        let mut graph = Graph::new();

        // Add root node
        graph.add_node(NodeType::Root, vec![], None);

        // Add predict node
        graph.add_node(
            NodeType::Predict {
                signature_name: "TestSignature".to_string(),
                signature: Arc::new(crate::core::signature::DummySignature::new()),
            },
            vec![0],
            None,
        );

        let spans = TraceContract::graph_to_spans(&graph, None, "test-trace");

        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].name, "dsrs.root");
        assert!(spans[1].name.contains("TestSignature"));
        assert_eq!(spans[1].kind, SpanKind::Client);
    }

    #[test]
    fn test_trace_summary() {
        let spans = vec![
            TraceSpan::new("t", "s1", "root").with_kind(SpanKind::Internal),
            TraceSpan::new("t", "s2", "predict")
                .with_kind(SpanKind::Client)
                .with_attribute("lm.total_tokens", 100u64)
                .with_attribute("lm.cost_msats", 50u64),
            TraceSpan::new("t", "s3", "predict")
                .with_kind(SpanKind::Client)
                .with_attribute("lm.total_tokens", 200u64)
                .with_attribute("lm.cost_msats", 100u64),
        ];

        let summary = TraceContract::summarize(&spans);

        assert_eq!(summary.span_count, 3);
        assert_eq!(summary.predict_count, 2);
        assert_eq!(summary.total_tokens, 300);
        assert_eq!(summary.total_cost_msats, 150);
    }

    #[test]
    fn test_span_serde() {
        let span = TraceSpan::new("trace", "span", "test")
            .with_kind(SpanKind::Producer)
            .with_attribute("key", "value");

        let json = serde_json::to_string(&span).unwrap();
        let parsed: TraceSpan = serde_json::from_str(&json).unwrap();

        assert_eq!(span.trace_id, parsed.trace_id);
        assert_eq!(span.kind, parsed.kind);
    }
}
