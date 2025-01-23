use crate::server::services::deepseek::types::{FunctionDefinition, Tool};

pub fn create_tool(
    name: String,
    description: Option<String>,
    parameters: serde_json::Value,
) -> Tool {
    Tool {
        tool_type: "function".to_string(),
        function: FunctionDefinition {
            name,
            description,
            parameters,
        },
    }
}
