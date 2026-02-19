// a module for special types
// right now most of these are just placeholders
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, JsonSchema, Clone)]
pub struct History;
#[derive(Deserialize, JsonSchema, Clone)]
pub struct ToolCall;

/// A placeholder tool type for when no tools are needed
#[derive(Clone)]
pub struct NoTool;

#[derive(Debug)]
pub struct NoToolError;

impl std::fmt::Display for NoToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NoTool error")
    }
}

impl std::error::Error for NoToolError {}

impl rig::tool::Tool for NoTool {
    const NAME: &'static str = "no_tool";

    type Error = NoToolError;
    type Args = ();
    type Output = String;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: Self::NAME.to_string(),
            description: "No tool available".to_string(),
            parameters: serde_json::json!({}),
        }
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        Ok("No tool".to_string())
    }
}
