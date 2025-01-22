use std::error::Error as StdError;
use std::fmt;
use serde_json::Value;

#[derive(Debug)]
pub enum ToolError {
    InvalidArguments(String),
    ExecutionFailed(String),
    NetworkError(String),
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArguments(msg) => write!(f, "Invalid arguments: {}", msg),
            Self::ExecutionFailed(msg) => write!(f, "Tool execution failed: {}", msg),
            Self::NetworkError(msg) => write!(f, "Network error: {}", msg),
        }
    }
}

impl StdError for ToolError {}

pub trait Tool {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn parameters(&self) -> Value;
    async fn execute(&self, args: Value) -> Result<String, ToolError>;
}

pub struct Function {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

pub trait ToolExecutor {
    fn available_tools(&self) -> Vec<Function>;
    async fn execute_tool(&self, name: &str, args: Value) -> Result<String, ToolError>;
}