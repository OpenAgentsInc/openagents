use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod github;
pub mod files;
pub mod external;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    #[serde(rename = "type")]
    pub type_: String,
    pub function: Function,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Debug)]
pub enum ToolError {
    InvalidArguments(String),
    ExecutionFailed(String),
    PermissionDenied(String),
    ResourceNotFound(String),
    RateLimitExceeded,
    NetworkError(String),
}

#[async_trait]
pub trait ToolExecutor {
    async fn execute(&self, name: &str, args: Value) -> Result<String>;
    fn get_available_tools(&self) -> Vec<Tool>;
    fn validate_arguments(&self, name: &str, args: &Value) -> Result<()>;
}

impl Tool {
    pub fn new(name: &str, description: &str, parameters: Value) -> Self {
        Self {
            type_: "function".to_string(),
            function: Function {
                name: name.to_string(),
                description: description.to_string(),
                parameters,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_tool_creation() {
        let params = json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path"
                }
            },
            "required": ["path"]
        });

        let tool = Tool::new("view_file", "View file contents", params.clone());

        assert_eq!(tool.type_, "function");
        assert_eq!(tool.function.name, "view_file");
        assert_eq!(tool.function.description, "View file contents");
        assert_eq!(tool.function.parameters, params);
    }

    #[test]
    fn test_tool_serialization() {
        let params = json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path"
                }
            },
            "required": ["path"]
        });

        let tool = Tool::new("view_file", "View file contents", params);
        let serialized = serde_json::to_string(&tool).unwrap();
        let deserialized: Tool = serde_json::from_str(&serialized).unwrap();

        assert_eq!(tool.function.name, deserialized.function.name);
        assert_eq!(tool.function.description, deserialized.function.description);
        assert_eq!(tool.function.parameters, deserialized.function.parameters);
    }
}