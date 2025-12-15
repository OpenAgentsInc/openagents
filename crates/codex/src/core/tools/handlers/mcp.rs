use async_trait::async_trait;

use crate::core::function_tool::FunctionCallError;
use crate::core::mcp_tool_call::handle_mcp_tool_call;
use crate::core::tools::context::ToolInvocation;
use crate::core::tools::context::ToolOutput;
use crate::core::tools::context::ToolPayload;
use crate::core::tools::registry::ToolHandler;
use crate::core::tools::registry::ToolKind;

pub struct McpHandler;

#[async_trait]
impl ToolHandler for McpHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Mcp
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            call_id,
            payload,
            ..
        } = invocation;

        let payload = match payload {
            ToolPayload::Mcp {
                server,
                tool,
                raw_arguments,
            } => (server, tool, raw_arguments),
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "mcp handler received unsupported payload".to_string(),
                ));
            }
        };

        let (server, tool, raw_arguments) = payload;
        let arguments_str = raw_arguments;

        let response = handle_mcp_tool_call(
            session.as_ref(),
            turn.as_ref(),
            call_id.clone(),
            server,
            tool,
            arguments_str,
        )
        .await;

        match response {
            crate::protocol::models::ResponseInputItem::McpToolCallOutput { result, .. } => {
                Ok(ToolOutput::Mcp { result })
            }
            crate::protocol::models::ResponseInputItem::FunctionCallOutput { output, .. } => {
                let crate::protocol::models::FunctionCallOutputPayload {
                    content,
                    content_items,
                    success,
                } = output;
                Ok(ToolOutput::Function {
                    content,
                    content_items,
                    success,
                })
            }
            _ => Err(FunctionCallError::RespondToModel(
                "mcp handler received unexpected response variant".to_string(),
            )),
        }
    }
}
