//! Tool Log Buffer
//!
//! Accumulates streaming tool output chunks and builds final payloads.
//! Used during agent execution to collect tool call inputs/outputs.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Buffer for accumulating tool call chunks
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolLogBuffer {
    /// Tool call ID
    pub id: String,
    /// Tool name (optional, may be set later)
    pub name: Option<String>,
    /// Accumulated chunks
    pub chunks: Vec<String>,
}

impl ToolLogBuffer {
    /// Create a new buffer with the given ID
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: None,
            chunks: Vec::new(),
        }
    }

    /// Create a new buffer with ID and name
    pub fn with_name(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: Some(name.into()),
            chunks: Vec::new(),
        }
    }

    /// Append a chunk to the buffer
    pub fn push(&mut self, chunk: impl Into<String>) {
        self.chunks.push(chunk.into());
    }

    /// Get the combined content
    pub fn content(&self) -> String {
        self.chunks.join("")
    }
}

/// Map of tool call IDs to their buffers
pub type ToolLogBufferMap = HashMap<String, ToolLogBuffer>;

/// Ensure a buffer exists for the given ID, creating one if needed
pub fn ensure_tool_buffer<'a>(
    buffers: &'a mut ToolLogBufferMap,
    id: &str,
    name: Option<&str>,
) -> &'a mut ToolLogBuffer {
    let id_string = id.to_string();
    let name_string = name.map(|n| n.to_string());

    buffers.entry(id_string.clone()).or_insert_with(|| {
        let mut buffer = ToolLogBuffer::new(&id_string);
        if let Some(ref n) = name_string {
            buffer.name = Some(n.clone());
        }
        buffer
    });

    // Update name if provided and not already set
    let buffer = buffers.get_mut(&id_string).unwrap();
    if let Some(n) = name_string {
        if buffer.name.is_none() {
            buffer.name = Some(n);
        }
    }
    buffer
}

/// Append a chunk to a tool buffer, creating the buffer if needed
pub fn append_tool_chunk(buffers: &mut ToolLogBufferMap, id: &str, chunk: Option<&str>) {
    if let Some(c) = chunk {
        let buffer = ensure_tool_buffer(buffers, id, None);
        buffer.push(c);
    }
}

/// Parse raw string as JSON, falling back to raw string on parse error
fn parse_input(raw: Option<&str>) -> Option<Value> {
    raw.map(|s| serde_json::from_str(s).unwrap_or_else(|_| Value::String(s.to_string())))
}

/// Payload for a completed tool call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolPayload {
    /// Tool call ID
    pub id: String,
    /// Tool name (if known)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    /// Parsed input (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<Value>,
}

/// Build the final tool payload from accumulated chunks
pub fn build_tool_payload(
    buffers: &ToolLogBufferMap,
    id: &str,
    final_input: Option<Value>,
) -> ToolPayload {
    let buffer = buffers.get(id);
    let raw = buffer.map(|b| b.content());
    let raw_ref = raw.as_deref();
    let parsed = parse_input(raw_ref);
    let input = final_input.or(parsed);

    ToolPayload {
        id: id.to_string(),
        tool: buffer.and_then(|b| b.name.clone()),
        input,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_log_buffer_new() {
        let buffer = ToolLogBuffer::new("test-id");
        assert_eq!(buffer.id, "test-id");
        assert!(buffer.name.is_none());
        assert!(buffer.chunks.is_empty());
    }

    #[test]
    fn test_tool_log_buffer_with_name() {
        let buffer = ToolLogBuffer::with_name("test-id", "Read");
        assert_eq!(buffer.id, "test-id");
        assert_eq!(buffer.name, Some("Read".to_string()));
    }

    #[test]
    fn test_append_chunks() {
        let mut buffer = ToolLogBuffer::new("id");
        buffer.push("hello ");
        buffer.push("world");
        assert_eq!(buffer.content(), "hello world");
    }

    #[test]
    fn test_ensure_tool_buffer() {
        let mut buffers = ToolLogBufferMap::new();

        // First call creates buffer
        ensure_tool_buffer(&mut buffers, "id1", Some("Read"));
        assert!(buffers.contains_key("id1"));
        assert_eq!(buffers.get("id1").unwrap().name, Some("Read".to_string()));

        // Second call returns existing
        ensure_tool_buffer(&mut buffers, "id1", None);
        assert_eq!(buffers.len(), 1);
    }

    #[test]
    fn test_append_tool_chunk() {
        let mut buffers = ToolLogBufferMap::new();

        append_tool_chunk(&mut buffers, "id1", Some("chunk1"));
        append_tool_chunk(&mut buffers, "id1", Some("chunk2"));
        append_tool_chunk(&mut buffers, "id1", None); // Should be ignored

        let buffer = buffers.get("id1").unwrap();
        assert_eq!(buffer.content(), "chunk1chunk2");
    }

    #[test]
    fn test_build_tool_payload_json() {
        let mut buffers = ToolLogBufferMap::new();
        let buffer = ensure_tool_buffer(&mut buffers, "id1", Some("Edit"));
        buffer.push(r#"{"file": "test.rs"}"#);

        let payload = build_tool_payload(&buffers, "id1", None);
        assert_eq!(payload.id, "id1");
        assert_eq!(payload.tool, Some("Edit".to_string()));
        assert!(payload.input.is_some());
    }

    #[test]
    fn test_build_tool_payload_raw_string() {
        let mut buffers = ToolLogBufferMap::new();
        let buffer = ensure_tool_buffer(&mut buffers, "id1", None);
        buffer.push("not json");

        let payload = build_tool_payload(&buffers, "id1", None);
        assert_eq!(payload.input, Some(Value::String("not json".to_string())));
    }

    #[test]
    fn test_build_tool_payload_with_final_input() {
        let mut buffers = ToolLogBufferMap::new();
        ensure_tool_buffer(&mut buffers, "id1", Some("Bash"));

        let final_input = serde_json::json!({"command": "ls"});
        let payload = build_tool_payload(&buffers, "id1", Some(final_input.clone()));
        assert_eq!(payload.input, Some(final_input));
    }
}
