use codex_client::{DynamicToolCallOutputContentItem, DynamicToolCallResponse};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::app_state::AutopilotToolCallRequest;

pub(super) const OPENAGENTS_TOOL_PREFIX: &str = "openagents.";
pub(super) const OPENAGENTS_TOOL_NAMES: &[&str] = &[
    "openagents.pane.list",
    "openagents.pane.open",
    "openagents.pane.focus",
    "openagents.pane.close",
    "openagents.pane.set_input",
    "openagents.pane.action",
    "openagents.cad.intent",
    "openagents.cad.action",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ToolBridgeRequest {
    pub tool: String,
    pub arguments: Value,
}

impl ToolBridgeRequest {
    pub(super) fn decode_arguments<T: DeserializeOwned>(
        &self,
    ) -> Result<T, ToolBridgeResultEnvelope> {
        serde_json::from_value::<T>(self.arguments.clone()).map_err(|error| {
            ToolBridgeResultEnvelope::error(
                "OA-TOOL-ARGS-INVALID-SHAPE",
                format!(
                    "Arguments for '{}' did not match expected shape: {}",
                    self.tool, error
                ),
                json!({
                    "tool": self.tool,
                    "arguments": self.arguments,
                }),
            )
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct ToolBridgeResultEnvelope {
    pub success: bool,
    pub code: String,
    pub message: String,
    pub details: Value,
}

impl ToolBridgeResultEnvelope {
    pub(super) fn ok(code: &str, message: impl Into<String>, details: Value) -> Self {
        Self {
            success: true,
            code: code.to_string(),
            message: message.into(),
            details,
        }
    }

    pub(super) fn error(code: &str, message: impl Into<String>, details: Value) -> Self {
        Self {
            success: false,
            code: code.to_string(),
            message: message.into(),
            details,
        }
    }

    pub(super) fn to_response(&self) -> DynamicToolCallResponse {
        DynamicToolCallResponse {
            content_items: vec![DynamicToolCallOutputContentItem::InputText {
                text: serde_json::to_string(self)
                    .unwrap_or_else(|_| "{\"success\":false,\"code\":\"OA-TOOL-RESPONSE-SERIALIZE-FAILED\",\"message\":\"failed to serialize tool response\",\"details\":{}}".to_string()),
            }],
            success: self.success,
        }
    }
}

pub(super) fn decode_tool_call_request(
    request: &AutopilotToolCallRequest,
) -> Result<ToolBridgeRequest, ToolBridgeResultEnvelope> {
    let tool = request.tool.trim();
    if !tool.starts_with(OPENAGENTS_TOOL_PREFIX) || !is_supported_tool(tool) {
        return Err(ToolBridgeResultEnvelope::error(
            "OA-TOOL-UNSUPPORTED",
            format!(
                "Unsupported tool '{}'. Supported tools must be in '{}' namespace and allowlisted.",
                request.tool, OPENAGENTS_TOOL_PREFIX
            ),
            json!({
                "tool": request.tool,
                "supported_tools": OPENAGENTS_TOOL_NAMES,
            }),
        ));
    }

    let arguments = if request.arguments.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str::<Value>(&request.arguments).map_err(|error| {
            ToolBridgeResultEnvelope::error(
                "OA-TOOL-ARGS-INVALID-JSON",
                format!("Failed to parse tool arguments JSON: {error}"),
                json!({
                    "tool": tool,
                    "arguments_raw": request.arguments,
                }),
            )
        })?
    };

    if !arguments.is_object() {
        return Err(ToolBridgeResultEnvelope::error(
            "OA-TOOL-ARGS-NOT-OBJECT",
            "Tool arguments must decode to a JSON object",
            json!({
                "tool": tool,
                "arguments": arguments,
            }),
        ));
    }

    Ok(ToolBridgeRequest {
        tool: tool.to_string(),
        arguments,
    })
}

fn is_supported_tool(tool: &str) -> bool {
    OPENAGENTS_TOOL_NAMES.iter().any(|entry| *entry == tool)
}

#[cfg(test)]
mod tests {
    use super::{ToolBridgeResultEnvelope, decode_tool_call_request};
    use crate::app_state::AutopilotToolCallRequest;
    use codex_client::AppServerRequestId;
    use serde::Deserialize;

    fn request(tool: &str, arguments: &str) -> AutopilotToolCallRequest {
        AutopilotToolCallRequest {
            request_id: AppServerRequestId::String("test-request-id".to_string()),
            thread_id: "thread".to_string(),
            turn_id: "turn".to_string(),
            call_id: "call".to_string(),
            tool: tool.to_string(),
            arguments: arguments.to_string(),
        }
    }

    #[derive(Debug, Deserialize)]
    struct PaneArgs {
        pane: String,
    }

    fn assert_error(result: Result<super::ToolBridgeRequest, ToolBridgeResultEnvelope>) -> String {
        let error = result.expect_err("expected decode failure");
        error.code
    }

    #[test]
    fn decode_accepts_supported_tool_and_object_arguments() {
        let decoded = decode_tool_call_request(&request(
            "openagents.pane.open",
            r#"{"pane":"Spark Wallet"}"#,
        ))
        .expect("decode should succeed");
        assert_eq!(decoded.tool, "openagents.pane.open");

        let pane_args: PaneArgs = decoded.decode_arguments().expect("pane args decode");
        assert_eq!(pane_args.pane, "Spark Wallet");
    }

    #[test]
    fn decode_rejects_unsupported_tool_name() {
        let code = assert_error(decode_tool_call_request(&request("openagents.not_real", "{}")));
        assert_eq!(code, "OA-TOOL-UNSUPPORTED");
    }

    #[test]
    fn decode_rejects_malformed_json_arguments() {
        let code = assert_error(decode_tool_call_request(&request("openagents.pane.open", "{")));
        assert_eq!(code, "OA-TOOL-ARGS-INVALID-JSON");
    }

    #[test]
    fn decode_rejects_non_object_arguments() {
        let code = assert_error(decode_tool_call_request(&request("openagents.pane.open", "[]")));
        assert_eq!(code, "OA-TOOL-ARGS-NOT-OBJECT");
    }

    #[test]
    fn decode_arguments_reports_missing_required_field() {
        let decoded =
            decode_tool_call_request(&request("openagents.pane.open", r#"{"wrong":"field"}"#))
                .expect("decode should succeed");
        let error = decoded
            .decode_arguments::<PaneArgs>()
            .expect_err("missing required field should fail");
        assert_eq!(error.code, "OA-TOOL-ARGS-INVALID-SHAPE");
    }
}
