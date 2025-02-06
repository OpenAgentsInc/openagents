use crate::server::services::deepseek::{DeepSeekService, Tool};
use serde_json::json;

pub fn create_tools() -> Vec<Tool> {
    vec![
        // GitHub issue tool
        DeepSeekService::create_tool(
            "read_github_issue".to_string(),
            Some("Read a GitHub issue by number".to_string()),
            json!({
                "type": "object",
                "properties": {
                    "owner": {
                        "type": "string",
                        "description": "The owner of the repository"
                    },
                    "repo": {
                        "type": "string",
                        "description": "The name of the repository"
                    },
                    "issue_number": {
                        "type": "integer",
                        "description": "The issue number"
                    }
                },
                "required": ["owner", "repo", "issue_number"]
            }),
        ),
        // Calculator tool
        DeepSeekService::create_tool(
            "calculate".to_string(),
            Some("Perform a calculation".to_string()),
            json!({
                "type": "object",
                "properties": {
                    "expression": {
                        "type": "string",
                        "description": "The mathematical expression to evaluate"
                    }
                },
                "required": ["expression"]
            }),
        ),
        // File analysis tool (handled by Gemini)
        DeepSeekService::create_tool(
            "analyze_files".to_string(),
            Some("Analyze repository files for changes using Gemini".to_string()),
            json!({
                "type": "object",
                "properties": {
                    "issue_description": {
                        "type": "string",
                        "description": "The description of the issue to analyze"
                    },
                    "repo_context": {
                        "type": "string",
                        "description": "The repository context information"
                    }
                },
                "required": ["issue_description", "repo_context"]
            }),
        ),
    ]
}