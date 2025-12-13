use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Structured tool or function invocation made by the agent.
///
/// ## Example
///
/// ```
/// use atif::ToolCall;
/// use serde_json::json;
///
/// let tool_call = ToolCall {
///     tool_call_id: "call_price_1".to_string(),
///     function_name: "financial_search".to_string(),
///     arguments: json!({ "ticker": "GOOGL", "metric": "price" }),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    /// Unique identifier for this specific tool call
    ///
    /// Used to correlate with observation results via `source_call_id`.
    pub tool_call_id: String,

    /// The name of the function or tool being invoked
    ///
    /// Examples: "financial_search", "file_write", "web_search"
    pub function_name: String,

    /// Arguments passed to the function
    ///
    /// Must be a valid JSON object, but can be empty (`{}`) if no arguments needed.
    pub arguments: Value,
}

impl ToolCall {
    /// Create a new tool call
    pub fn new(
        tool_call_id: impl Into<String>,
        function_name: impl Into<String>,
        arguments: Value,
    ) -> Self {
        Self {
            tool_call_id: tool_call_id.into(),
            function_name: function_name.into(),
            arguments,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_tool_call_creation() {
        let tool_call = ToolCall::new("call_1", "search", json!({"query": "test"}));

        assert_eq!(tool_call.tool_call_id, "call_1");
        assert_eq!(tool_call.function_name, "search");
    }

    #[test]
    fn test_tool_call_serialization() {
        let tool_call = ToolCall::new("call_1", "search", json!({}));

        let json = serde_json::to_string(&tool_call).unwrap();
        let deserialized: ToolCall = serde_json::from_str(&json).unwrap();
        assert_eq!(tool_call, deserialized);
    }
}
