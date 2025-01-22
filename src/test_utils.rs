use mockall::predicate::*;
use mockall::mock;

// Re-export mockall predicates
pub use mockall::predicate::*;

// Create mock for Tool trait
mock! {
    pub Tool {
        fn name(&self) -> &'static str;
        fn description(&self) -> &'static str;
        fn parameters(&self) -> serde_json::Value;
        fn execute(&self, args: serde_json::Value) -> Result<String, crate::server::tools::ToolError>;
    }
}

// Create mock for WebSocketStateService trait
mock! {
    pub WebSocketStateService {
        async fn broadcast(&self, msg: crate::server::ws::types::Message);
    }
}

// Create mock for DeepSeekService trait
mock! {
    pub DeepSeekService {
        async fn chat_stream(&self, content: String, tools: Vec<serde_json::Value>) -> tokio::sync::mpsc::Receiver<crate::server::services::StreamUpdate>;
    }
}

// Create mock for ToolExecutorFactory trait
mock! {
    pub ToolExecutorFactory {
        fn create_executor(&self, tool_name: &str) -> Option<std::sync::Arc<dyn crate::server::tools::Tool>>;
        fn list_tools(&self) -> Vec<String>;
    }
}

// Create mock for ChatHandlerService trait
mock! {
    pub ChatHandlerService {
        async fn enable_tool(&self, tool: &str) -> Result<(), crate::server::tools::ToolError>;
        async fn disable_tool(&self, tool: &str) -> Result<(), crate::server::tools::ToolError>;
        async fn handle_message(&self, msg: crate::server::ws::types::Message) -> Result<(), crate::server::tools::ToolError>;
    }
}