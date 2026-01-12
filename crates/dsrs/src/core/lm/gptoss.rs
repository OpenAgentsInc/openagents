//! GPT-OSS LM client with structured output support.
//!
//! Uses the Responses API with tool definitions to constrain output
//! to a JSON schema, ensuring clean structured responses from local LLMs.

use anyhow::Result;
use gpt_oss::{
    GptOssClient, GptOssResponsesRequest, GptOssToolChoice, GptOssToolChoiceFunction,
    GptOssToolDefinition, GptOssToolFunction,
};
use rig::completion::{CompletionError, CompletionRequest, CompletionResponse};
use rig::message::{AssistantContent, Text};
use rig::one_or_many::OneOrMany;
use serde_json::{json, Value};

use super::CompletionProvider;

/// GPT-OSS completion model using the Responses API.
///
/// This client uses tool definitions with JSON schemas to constrain
/// the model's output, avoiding the parsing issues with local LLMs
/// that mix reasoning with structured output.
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

    /// Extract output schema from the prompt if present.
    ///
    /// The ChatAdapter formats prompts with output field descriptions.
    /// We parse these to build a JSON schema for structured output.
    fn extract_output_schema(prompt: &str) -> Option<Value> {
        // Look for the output fields section in the prompt
        // Format: "[[ ## field_name ## ]] (type: Type)"
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();

        for line in prompt.lines() {
            if line.starts_with("[[ ## ") && line.ends_with(" ## ]]") {
                // Extract field name
                let field_name = line
                    .trim_start_matches("[[ ## ")
                    .trim_end_matches(" ## ]]")
                    .to_string();

                if field_name != "completed" {
                    // Default to string type
                    properties.insert(
                        field_name.clone(),
                        json!({
                            "type": "string"
                        }),
                    );
                    required.push(Value::String(field_name));
                }
            }
        }

        if properties.is_empty() {
            return None;
        }

        Some(json!({
            "type": "object",
            "properties": properties,
            "required": required
        }))
    }

    /// Create a tool definition for structured output.
    fn create_output_tool(schema: Value) -> GptOssToolDefinition {
        GptOssToolDefinition {
            tool_type: "function".to_string(),
            function: GptOssToolFunction {
                name: "complete_signature".to_string(),
                description: Some(
                    "Complete the signature by providing all required output fields".to_string(),
                ),
                parameters: schema,
            },
        }
    }
}

/// Build prompt string from rig CompletionRequest
fn build_prompt_from_request(request: &CompletionRequest) -> String {
    let mut parts = Vec::new();

    // Add preamble/system prompt
    if let Some(preamble) = &request.preamble {
        parts.push(format!("System: {}", preamble));
    }

    // Add chat history
    for msg in request.chat_history.iter() {
        match msg {
            rig::message::Message::User { content } => {
                for c in content.iter() {
                    if let rig::message::UserContent::Text(text) = c {
                        parts.push(format!("User: {}", text.text));
                    }
                }
            }
            rig::message::Message::Assistant { content, .. } => {
                for c in content.iter() {
                    if let rig::message::AssistantContent::Text(text) = c {
                        parts.push(format!("Assistant: {}", text.text));
                    }
                }
            }
        }
    }

    parts.join("\n\n")
}

impl CompletionProvider for GptOssCompletionModel {
    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse<()>, CompletionError> {
        // Build prompt from request
        let prompt = build_prompt_from_request(&request);

        // Try to extract schema from prompt and use structured output
        let response = if let Some(schema) = Self::extract_output_schema(&prompt) {
            let tool = Self::create_output_tool(schema);

            let req = GptOssResponsesRequest::new(&self.model, &prompt)
                .with_tools(vec![tool])
                .with_tool_choice(GptOssToolChoice::Named {
                    tool_type: "function".to_string(),
                    function: GptOssToolChoiceFunction {
                        name: "complete_signature".to_string(),
                    },
                });

            let resp = self
                .client
                .responses(req)
                .await
                .map_err(|e| CompletionError::ProviderError(e.to_string()))?;

            // Extract tool call arguments as the structured output
            let tool_calls = resp.tool_calls();
            if let Some(tc) = tool_calls.first() {
                // Format the response in the expected format
                let mut output = String::new();
                if let Some(obj) = tc.arguments.as_object() {
                    for (key, value) in obj {
                        output.push_str(&format!("[[ ## {} ## ]]\n", key));
                        if let Some(s) = value.as_str() {
                            output.push_str(s);
                        } else {
                            output.push_str(&value.to_string());
                        }
                        output.push_str("\n\n");
                    }
                }
                output.push_str("[[ ## completed ## ]]");
                output
            } else {
                // Fallback to text output
                resp.output_text()
            }
        } else {
            // No schema detected, use basic completion
            let req = gpt_oss::GptOssRequest {
                model: self.model.clone(),
                prompt,
                max_tokens: self.max_tokens,
                temperature: self.temperature,
                top_p: None,
                stop: None,
                stream: false,
            };

            let resp = self
                .client
                .complete(req)
                .await
                .map_err(|e| CompletionError::ProviderError(e.to_string()))?;

            resp.text
        };

        Ok(CompletionResponse {
            choice: OneOrMany::one(AssistantContent::Text(Text { text: response })),
            usage: rig::completion::Usage {
                input_tokens: 0,
                output_tokens: 0,
                total_tokens: 0,
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
System: You are a helpful assistant.

Respond with the following fields:
[[ ## task_type ## ]]
[[ ## requirements ## ]]
[[ ## scope_estimate ## ]]
[[ ## completed ## ]]
"#;

        let schema = GptOssCompletionModel::extract_output_schema(prompt);
        assert!(schema.is_some());

        let schema = schema.unwrap();
        let props = schema.get("properties").unwrap().as_object().unwrap();
        assert!(props.contains_key("task_type"));
        assert!(props.contains_key("requirements"));
        assert!(props.contains_key("scope_estimate"));
        assert!(!props.contains_key("completed"));
    }
}
