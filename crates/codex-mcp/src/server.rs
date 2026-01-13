use anyhow::Context;
use serde_json::Value;

use crate::protocol::{Response, ToolCallParams, ToolCallResult, ToolsListResult};
use crate::transport::Transport;

/// Minimal handler surface for Codex-style MCP servers.
///
/// The MCP spec is broader (resources, prompts, sampling, etc). This trait
/// intentionally keeps the core tool flow ergonomic for small servers.
#[async_trait::async_trait]
pub trait McpHandler: Send + Sync {
    async fn initialize(&self, _params: Value) -> anyhow::Result<Value> {
        Ok(serde_json::json!({
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "codex-mcp", "version": env!("CARGO_PKG_VERSION") }
        }))
    }

    async fn tools_list(&self) -> anyhow::Result<ToolsListResult>;
    async fn tools_call(&self, params: ToolCallParams) -> anyhow::Result<ToolCallResult>;
}

pub async fn serve<T: Transport>(
    mut transport: T,
    handler: &impl McpHandler,
) -> anyhow::Result<()> {
    while let Some(request) = transport.read_request().await? {
        if request.method.is_empty() {
            continue;
        }

        let id = request.id.clone();
        let response = match request.method.as_str() {
            "initialize" => match handler.initialize(request.params).await {
                Ok(result) => Response::ok(id, result),
                Err(err) => Response::err(
                    id,
                    -32000,
                    "initialize failed",
                    Some(serde_json::json!({ "error": err.to_string() })),
                ),
            },
            "tools/list" => match handler.tools_list().await {
                Ok(result) => Response::ok(id, serde_json::to_value(result)?),
                Err(err) => Response::err(
                    id,
                    -32000,
                    "tools/list failed",
                    Some(serde_json::json!({ "error": err.to_string() })),
                ),
            },
            "tools/call" => {
                let params: ToolCallParams =
                    serde_json::from_value(request.params).context("decode tools/call params")?;
                match handler.tools_call(params).await {
                    Ok(result) => Response::ok(id, serde_json::to_value(result)?),
                    Err(err) => Response::err(
                        id,
                        -32000,
                        "tools/call failed",
                        Some(serde_json::json!({ "error": err.to_string() })),
                    ),
                }
            }
            _ => Response::err(id, -32601, "Method not found", None),
        };

        transport.write_response(&response).await?;
    }

    Ok(())
}
