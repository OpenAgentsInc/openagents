//! GPT-OSS LM client with structured output support.
//!
//! Uses the Harmony prompt format with:
//! 1. System message with reasoning effort and valid channels
//! 2. Developer message with instructions and Response Formats section
//! 3. response_format for grammar enforcement during sampling

use anyhow::Result;
use gpt_oss::{
    ChatCompletionsRequest, ChatMessage, GptOssClient, JsonSchemaSpec, ResponseFormat,
};
use rig::completion::{CompletionError, CompletionRequest, CompletionResponse};
use rig::message::{AssistantContent, Text};
use rig::one_or_many::OneOrMany;
use serde_json::{json, Value};

use super::CompletionProvider;

/// GPT-OSS completion model using Harmony format with structured output.
///
/// This client uses the proper GPT-OSS Harmony format:
/// - System message with reasoning effort
/// - Developer message with # Response Formats section
/// - response_format for GBNF grammar enforcement
#[derive(Clone)]
pub struct GptOssCompletionModel {
    client: GptOssClient,
    model: String,
    temperature: Option<f32>,
    max_tokens: Option<usize>,
}

impl GptOssCompletionModel {
    /// Create a new GPT-OSS completion model.
    pub fn new(base_url: &str, model: impl Into<String>) -> Result<Self> {
        let client = GptOssClient::with_base_url(base_url)?;
        Ok(Self {
            client,
            model: model.into(),
            temperature: None,
            max_tokens: None,
        })
    }

    /// Set the temperature.
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Set the max tokens.
    pub fn with_max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Extract output schema from the DSPy-formatted prompt.
    ///
    /// The ChatAdapter formats prompts with output field descriptions like:
    /// "1. `field_name` (Type): description"
    /// We parse these to build a JSON schema for structured output.
    fn extract_output_schema(prompt: &str) -> Option<Value> {
        // Look for "Your output fields are:" section
        let output_section = prompt.split("Your output fields are:").nth(1)?;
        let output_section = output_section.split("All interactions").next()?;

        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();

        // Parse numbered field list: "1. `field_name` (Type): description"
        for line in output_section.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // Match pattern like "1. `field_name` (Type): description"
            if let Some(backtick_start) = line.find('`') {
                if let Some(backtick_end) = line[backtick_start + 1..].find('`') {
                    let field_name = &line[backtick_start + 1..backtick_start + 1 + backtick_end];

                    if field_name == "completed" {
                        continue;
                    }

                    // Extract type from (Type) pattern
                    let after_name = &line[backtick_start + 2 + backtick_end..];
                    let json_type = if let Some(paren_start) = after_name.find('(') {
                        if let Some(paren_end) = after_name[paren_start..].find(')') {
                            let type_str = &after_name[paren_start + 1..paren_start + paren_end];
                            rust_type_to_json_schema(type_str)
                        } else {
                            json!({"type": "string"})
                        }
                    } else {
                        json!({"type": "string"})
                    };

                    properties.insert(field_name.to_string(), json_type);
                    required.push(Value::String(field_name.to_string()));
                }
            }
        }

        if properties.is_empty() {
            return None;
        }

        Some(json!({
            "type": "object",
            "properties": properties,
            "required": required,
            "additionalProperties": false
        }))
    }

    /// Extract the task instruction from DSPy prompt.
    fn extract_instruction(prompt: &str) -> String {
        // Look for "your objective is:" section
        if let Some(idx) = prompt.to_lowercase().find("your objective is:") {
            let after = &prompt[idx + 18..];
            if let Some(end) = after.find('\n') {
                return after[..end].trim().to_string();
            }
            return after.trim().to_string();
        }

        // Fallback: use first non-empty line after "Given the fields"
        if let Some(idx) = prompt.find("Given the fields") {
            let after = &prompt[idx..];
            for line in after.lines().skip(1) {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    return trimmed.to_string();
                }
            }
        }

        "Complete the task as specified.".to_string()
    }

    /// Build Harmony-formatted messages for GPT-OSS.
    fn build_harmony_messages(
        request: &CompletionRequest,
        schema: Option<&Value>,
    ) -> Vec<ChatMessage> {
        let mut messages = Vec::new();

        // Get today's date
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();

        // 1. System message with Harmony format
        let system_content = format!(
            r#"You are a helpful AI assistant.
Knowledge cutoff: 2024-06
Current date: {}

Reasoning: medium

# Valid channels: analysis, commentary, final. Channel must be included for every message."#,
            today
        );

        messages.push(ChatMessage {
            role: "system".to_string(),
            content: system_content,
        });

        // 2. Developer message with instructions and Response Formats
        let mut developer_content = String::new();

        // Extract instruction from the original prompt
        let full_prompt: String = request
            .preamble
            .as_deref()
            .unwrap_or("")
            .to_string()
            + &request
                .chat_history
                .iter()
                .filter_map(|m| match m {
                    rig::message::Message::User { content } => {
                        content.iter().find_map(|c| {
                            if let rig::message::UserContent::Text(t) = c {
                                Some(t.text.clone())
                            } else {
                                None
                            }
                        })
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");

        let instruction = Self::extract_instruction(&full_prompt);

        developer_content.push_str("# Instructions\n\n");
        developer_content.push_str(&instruction);
        developer_content.push_str("\n\n");

        // Add Response Formats section if we have a schema
        if let Some(schema) = schema {
            developer_content.push_str("# Response Formats\n\n");
            developer_content.push_str("## dspy_output\n\n");
            developer_content.push_str("// Output must be valid JSON matching this schema\n");
            developer_content.push_str(&schema.to_string());
        }

        messages.push(ChatMessage {
            role: "user".to_string(), // Note: llama-server may not support "developer" role, use user
            content: developer_content,
        });

        // 3. Add user message with the actual task/input
        if let Some(preamble) = &request.preamble {
            // The preamble often contains the full DSPy formatted prompt
            // Extract just the user input part
            if let Some(user_section) = preamble.split("[[ ## ").nth(1) {
                if let Some(content) = user_section.split(" ## ]]").nth(1) {
                    let user_input = content.split("[[ ##").next().unwrap_or(content).trim();
                    if !user_input.is_empty() {
                        messages.push(ChatMessage {
                            role: "user".to_string(),
                            content: user_input.to_string(),
                        });
                    }
                }
            }
        }

        // Add any additional user messages from chat history
        for msg in request.chat_history.iter() {
            match msg {
                rig::message::Message::User { content } => {
                    let mut text_parts = Vec::new();
                    for c in content.iter() {
                        if let rig::message::UserContent::Text(text) = c {
                            text_parts.push(text.text.clone());
                        }
                    }
                    if !text_parts.is_empty() {
                        messages.push(ChatMessage {
                            role: "user".to_string(),
                            content: text_parts.join("\n"),
                        });
                    }
                }
                rig::message::Message::Assistant { content, .. } => {
                    let mut text_parts = Vec::new();
                    for c in content.iter() {
                        if let rig::message::AssistantContent::Text(text) = c {
                            text_parts.push(text.text.clone());
                        }
                    }
                    if !text_parts.is_empty() {
                        messages.push(ChatMessage {
                            role: "assistant".to_string(),
                            content: text_parts.join("\n"),
                        });
                    }
                }
            }
        }

        messages
    }
}

/// Convert Rust type annotation to JSON schema type
fn rust_type_to_json_schema(rust_type: &str) -> Value {
    match rust_type.trim() {
        "String" | "str" | "&str" => json!({"type": "string"}),
        "i32" | "i64" | "u32" | "u64" | "isize" | "usize" => json!({"type": "integer"}),
        "f32" | "f64" => json!({"type": "number"}),
        "bool" => json!({"type": "boolean"}),
        t if t.starts_with("Vec<") => {
            // Extract inner type from Vec<T>
            let inner = t.strip_prefix("Vec<").and_then(|s| s.strip_suffix('>'));
            if let Some(inner_type) = inner {
                json!({
                    "type": "array",
                    "items": rust_type_to_json_schema(inner_type)
                })
            } else {
                json!({"type": "array", "items": {"type": "string"}})
            }
        }
        t if t.starts_with("Option<") => {
            // For Option<T>, just use the inner type (null handled separately)
            let inner = t.strip_prefix("Option<").and_then(|s| s.strip_suffix('>'));
            if let Some(inner_type) = inner {
                rust_type_to_json_schema(inner_type)
            } else {
                json!({"type": "string"})
            }
        }
        // Unknown types default to string
        _ => json!({"type": "string"}),
    }
}

impl CompletionProvider for GptOssCompletionModel {
    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse<()>, CompletionError> {
        // Get combined prompt text to extract schema
        let full_prompt: String = request
            .preamble
            .as_deref()
            .unwrap_or("")
            .to_string()
            + &request
                .chat_history
                .iter()
                .filter_map(|m| match m {
                    rig::message::Message::User { content } => {
                        content.iter().find_map(|c| {
                            if let rig::message::UserContent::Text(t) = c {
                                Some(t.text.clone())
                            } else {
                                None
                            }
                        })
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");

        // Extract schema from DSPy prompt
        let schema = Self::extract_output_schema(&full_prompt);

        // Build Harmony-formatted messages
        let messages = Self::build_harmony_messages(&request, schema.as_ref());

        // Set up response_format for grammar enforcement
        let response_format = schema.as_ref().map(|s| ResponseFormat::JsonSchema {
            json_schema: JsonSchemaSpec {
                name: Some("dspy_output".to_string()),
                schema: s.clone(),
                strict: Some(true),
            },
        });

        let req = ChatCompletionsRequest {
            model: self.model.clone(),
            messages,
            max_tokens: self.max_tokens,
            temperature: self.temperature,
            top_p: None,
            stop: None,
            response_format,
            stream: false,
        };

        let resp = self
            .client
            .chat_completions(req)
            .await
            .map_err(|e| CompletionError::ProviderError(e.to_string()))?;

        let content = resp.content().to_string();

        // If we used structured output, wrap the JSON in DSPy format
        let response = if schema.is_some() {
            // Parse JSON and format as DSPy expected format
            if let Ok(json_obj) = serde_json::from_str::<serde_json::Map<String, Value>>(&content) {
                let mut output = String::new();
                for (key, value) in json_obj {
                    output.push_str(&format!("[[ ## {} ## ]]\n", key));
                    if let Some(s) = value.as_str() {
                        output.push_str(s);
                    } else {
                        output.push_str(&value.to_string());
                    }
                    output.push_str("\n\n");
                }
                output.push_str("[[ ## completed ## ]]");
                output
            } else {
                content
            }
        } else {
            content
        };

        let usage = resp.usage.as_ref();
        Ok(CompletionResponse {
            choice: OneOrMany::one(AssistantContent::Text(Text { text: response })),
            usage: rig::completion::Usage {
                input_tokens: usage.map(|u| u.prompt_tokens as u64).unwrap_or(0),
                output_tokens: usage.map(|u| u.completion_tokens as u64).unwrap_or(0),
                total_tokens: usage.map(|u| u.total_tokens as u64).unwrap_or(0),
            },
            raw_response: (),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_output_schema() {
        let prompt = r#"
Your input fields are:
1. `user_task` (String): The task description

Your output fields are:
1. `task_type` (String): Type of task
2. `requirements` (Vec<String>): List of requirements
3. `confidence` (f64): Confidence score

All interactions will be structured...
"#;

        let schema = GptOssCompletionModel::extract_output_schema(prompt);
        assert!(schema.is_some());

        let schema = schema.unwrap();
        let props = schema.get("properties").unwrap().as_object().unwrap();
        assert!(props.contains_key("task_type"));
        assert!(props.contains_key("requirements"));
        assert!(props.contains_key("confidence"));
        assert!(!props.contains_key("completed"));

        // Check types
        assert_eq!(props["task_type"]["type"], "string");
        assert_eq!(props["requirements"]["type"], "array");
        assert_eq!(props["confidence"]["type"], "number");
    }

    #[test]
    fn test_rust_type_to_json_schema() {
        assert_eq!(
            rust_type_to_json_schema("String"),
            json!({"type": "string"})
        );
        assert_eq!(rust_type_to_json_schema("i32"), json!({"type": "integer"}));
        assert_eq!(rust_type_to_json_schema("f64"), json!({"type": "number"}));
        assert_eq!(rust_type_to_json_schema("bool"), json!({"type": "boolean"}));
        assert_eq!(
            rust_type_to_json_schema("Vec<String>"),
            json!({"type": "array", "items": {"type": "string"}})
        );
    }

    #[test]
    fn test_extract_instruction() {
        let prompt = "In adhering to this structure, your objective is:\n\tAnalyze the task and produce requirements.";
        let instruction = GptOssCompletionModel::extract_instruction(prompt);
        assert!(instruction.contains("Analyze"));
    }
}
