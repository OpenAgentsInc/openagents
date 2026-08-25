//! Agent Client Protocol (ACP) & Devin / external harness integration

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: u64,
    pub result: Option<serde_json::Value>,
    pub error: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionMode {
    Dangerous,
    Prompt,
    ReadOnly,
}

pub struct DevinAcpClient {
    pub mode: PermissionMode,
    pub seq: u64,
}

impl DevinAcpClient {
    pub fn new(mode: PermissionMode) -> Self {
        Self { mode, seq: 0 }
    }

    pub fn build_initialize_request(&mut self) -> JsonRpcRequest {
        self.seq += 1;
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: self.seq,
            method: "initialize".to_string(),
            params: serde_json::json!({
                "protocolVersion": "2024-11-05",
                "clientInfo": {
                    "name": "openagents-cli-rust",
                    "version": "0.1.0"
                },
                "capabilities": {
                    "tools": true,
                    "prompts": true
                }
            }),
        }
    }

    pub fn handle_response(&self, res: JsonRpcResponse) -> Result<serde_json::Value, String> {
        if let Some(err) = res.error {
            Err(format!("ACP error: {:?}", err))
        } else {
            Ok(res.result.unwrap_or(serde_json::Value::Null))
        }
    }
}
