//! LLM-based plan parsing using Claude

use claude_agent_sdk::{QueryOptions, SdkMessage, query};
use futures::StreamExt;
use serde::{Deserialize, Serialize};

use crate::{IssueType, Priority};

/// Errors that can occur during plan parsing
#[derive(Debug)]
pub enum ParseError {
    /// Claude SDK error
    SdkError(String),
    /// Failed to parse JSON response
    JsonError(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::SdkError(msg) => write!(f, "Claude SDK error: {}", msg),
            ParseError::JsonError(msg) => write!(f, "JSON parse error: {}", msg),
        }
    }
}

impl std::error::Error for ParseError {}

/// A parsed plan with extracted tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedPlan {
    /// Title of the plan (extracted from first heading)
    pub title: String,
    /// Extracted tasks
    pub tasks: Vec<ParsedTask>,
}

/// A single task extracted from the plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedTask {
    /// Short, action-oriented title
    pub title: String,
    /// Detailed description
    #[serde(default)]
    pub description: Option<String>,
    /// Priority level (P0-P4)
    #[serde(default = "default_priority")]
    pub priority: String,
    /// Task type (bug, feature, task, epic, chore)
    #[serde(default = "default_type", rename = "type")]
    pub issue_type: String,
    /// Dependencies (titles of other tasks this depends on)
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// What defines "done"
    #[serde(default)]
    pub acceptance_criteria: Option<String>,
}

fn default_priority() -> String {
    "P2".to_string()
}

fn default_type() -> String {
    "task".to_string()
}

impl ParsedTask {
    /// Convert the string priority to Priority enum
    pub fn priority_enum(&self) -> Priority {
        match self.priority.to_uppercase().as_str() {
            "P0" | "CRITICAL" => Priority::Critical,
            "P1" | "HIGH" => Priority::High,
            "P2" | "MEDIUM" => Priority::Medium,
            "P3" | "LOW" => Priority::Low,
            "P4" | "BACKLOG" => Priority::Backlog,
            _ => Priority::Medium,
        }
    }

    /// Convert the string type to IssueType enum
    pub fn issue_type_enum(&self) -> IssueType {
        match self.issue_type.to_lowercase().as_str() {
            "bug" => IssueType::Bug,
            "feature" => IssueType::Feature,
            "epic" => IssueType::Epic,
            "chore" => IssueType::Chore,
            _ => IssueType::Task,
        }
    }
}

const SYSTEM_PROMPT: &str = r#"You are a task extraction assistant. Given a markdown plan document, extract all actionable tasks and return them as JSON.

For each task, identify:
- title: Short, action-oriented title (max 100 chars)
- description: Detailed description if available
- priority: P0-P4 based on urgency/importance mentioned (default P2)
- type: bug/feature/task/epic/chore (default task)
- dependencies: List of other task titles this depends on
- acceptance_criteria: What defines "done"

Return ONLY a valid JSON object with this structure (no markdown, no explanation):
{
  "title": "Plan title from first heading",
  "tasks": [
    {
      "title": "...",
      "description": "...",
      "priority": "P2",
      "type": "task",
      "dependencies": [],
      "acceptance_criteria": "..."
    }
  ]
}

Rules:
- Only include concrete, actionable items
- Skip section headers without actionable content
- Skip notes or context that aren't tasks
- Skip already completed items
- If phases are numbered, include phase number in title
- Combine related sub-steps into single tasks when appropriate
- Look for implementation steps, phases, or numbered items"#;

/// Parse a plan file content using Claude LLM
///
/// # Arguments
/// * `content` - The markdown content of the plan file
/// * `plan_name` - Name of the plan file (for context)
///
/// # Returns
/// A `ParsedPlan` with extracted tasks
pub async fn parse_plan_with_llm(content: &str, plan_name: &str) -> Result<ParsedPlan, ParseError> {
    // Combine system prompt with the plan content
    let prompt = format!(
        "{}\n\n---\n\nExtract tasks from this plan document named '{}':\n\n{}",
        SYSTEM_PROMPT, plan_name, content
    );

    let options = QueryOptions::new()
        .model("claude-haiku-4-5-20251001") // Use Haiku for cost efficiency
        .max_turns(1);

    let stream = query(&prompt, options)
        .await
        .map_err(|e| ParseError::SdkError(format!("Failed to start Claude query: {}", e)))?;

    futures::pin_mut!(stream);

    let mut response_text = String::new();

    while let Some(result) = stream.next().await {
        match result {
            Ok(msg) => {
                if let SdkMessage::Assistant(assistant_msg) = msg {
                    if let Some(text) = extract_text_from_message(&assistant_msg.message) {
                        response_text.push_str(&text);
                    }
                }
            }
            Err(e) => {
                return Err(ParseError::SdkError(format!("Claude stream error: {}", e)));
            }
        }
    }

    // Parse the JSON response
    parse_json_response(&response_text)
}

/// Extract text content from an assistant message
fn extract_text_from_message(message: &serde_json::Value) -> Option<String> {
    // The message can be a string or an array of content blocks
    if let Some(s) = message.as_str() {
        return Some(s.to_string());
    }

    if let Some(arr) = message.as_array() {
        let mut text = String::new();
        for block in arr {
            if let Some(block_type) = block.get("type").and_then(|t| t.as_str()) {
                if block_type == "text" {
                    if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                        text.push_str(t);
                    }
                }
            }
        }
        if !text.is_empty() {
            return Some(text);
        }
    }

    None
}

/// Parse the JSON response from Claude
fn parse_json_response(text: &str) -> Result<ParsedPlan, ParseError> {
    // Try to find JSON in the response (might be wrapped in markdown code blocks)
    let json_str = extract_json(text);

    serde_json::from_str(&json_str).map_err(|e| {
        ParseError::JsonError(format!(
            "Failed to parse LLM response as JSON: {}. Response was: {}",
            e,
            &text[..text.len().min(500)]
        ))
    })
}

/// Extract JSON from text that might have markdown code blocks
fn extract_json(text: &str) -> String {
    // Try to find JSON block in markdown
    if let Some(start) = text.find("```json") {
        if let Some(end) = text[start..]
            .find("```\n")
            .or_else(|| text[start..].rfind("```"))
        {
            let json_start = start + 7; // Skip "```json"
            let json_end = start + end;
            if json_start < json_end {
                return text[json_start..json_end].trim().to_string();
            }
        }
    }

    // Try to find JSON block without language specifier
    if let Some(start) = text.find("```") {
        if let Some(end) = text[start + 3..].find("```") {
            let json_start = start + 3;
            let json_end = json_start + end;
            let content = text[json_start..json_end].trim();
            if content.starts_with('{') {
                return content.to_string();
            }
        }
    }

    // Try to find raw JSON object
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            return text[start..=end].to_string();
        }
    }

    text.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_raw() {
        let text = r#"{"title": "Test", "tasks": []}"#;
        let json = extract_json(text);
        assert!(json.starts_with('{'));
    }

    #[test]
    fn test_extract_json_markdown() {
        let text = r#"Here's the JSON:

```json
{"title": "Test", "tasks": []}
```

That's it."#;
        let json = extract_json(text);
        assert_eq!(json, r#"{"title": "Test", "tasks": []}"#);
    }

    #[test]
    fn test_parsed_task_priority() {
        let task = ParsedTask {
            title: "Test".to_string(),
            description: None,
            priority: "P0".to_string(),
            issue_type: "bug".to_string(),
            dependencies: vec![],
            acceptance_criteria: None,
        };
        assert_eq!(task.priority_enum(), Priority::Critical);
    }

    #[test]
    fn test_parsed_task_type() {
        let task = ParsedTask {
            title: "Test".to_string(),
            description: None,
            priority: "P2".to_string(),
            issue_type: "feature".to_string(),
            dependencies: vec![],
            acceptance_criteria: None,
        };
        assert_eq!(task.issue_type_enum(), IssueType::Feature);
    }

    #[test]
    fn test_parse_json_response() {
        let json = r#"{"title": "My Plan", "tasks": [{"title": "Do thing", "priority": "P1", "type": "task"}]}"#;
        let parsed = parse_json_response(json).unwrap();
        assert_eq!(parsed.title, "My Plan");
        assert_eq!(parsed.tasks.len(), 1);
        assert_eq!(parsed.tasks[0].title, "Do thing");
    }
}
