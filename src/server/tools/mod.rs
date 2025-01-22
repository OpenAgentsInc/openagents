use std::error::Error as StdError;
use std::fmt;
use serde_json::Value;
use mockall::automock;

#[derive(Debug)]
pub enum ToolError {
    InvalidArguments(String),
    ExecutionFailed(String),
    NetworkError(String),
}

impl std::error::Error for ToolError {}

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

#[automock]
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

#[automock]
pub trait ToolExecutor {
    fn available_tools(&self) -> Vec<Function>;
    async fn execute_tool(&self, name: &str, args: Value) -> Result<String, ToolError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_mock_tool() {
        let mut mock = MockTool::new();
        mock.expect_name()
            .returning(|| "test_tool");
        mock.expect_description()
            .returning(|| "Test tool description");
        mock.expect_parameters()
            .returning(|| json!({"type": "object"}));
        mock.expect_execute()
            .with(eq(json!({"arg": "value"})))
            .returning(|_| Ok("result".to_string()));

        assert_eq!(mock.name(), "test_tool");
        assert_eq!(mock.description(), "Test tool description");
        assert_eq!(mock.parameters(), json!({"type": "object"}));
        
        let result = mock.execute(json!({"arg": "value"})).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "result");
    }

    #[tokio::test]
    async fn test_mock_tool_error() {
        let mut mock = MockTool::new();
        mock.expect_execute()
            .with(eq(json!({"arg": "invalid"})))
            .returning(|_| Err(ToolError::InvalidArguments("invalid argument".to_string())));

        let result = mock.execute(json!({"arg": "invalid"})).await;
        assert!(matches!(result, Err(ToolError::InvalidArguments(_))));
    }
}
