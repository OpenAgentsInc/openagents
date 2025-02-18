use anyhow::Result;
use tracing::info;

use crate::server::services::deepseek::types::{
    ChatMessage, ChatRequest, ChatResponse, Tool, ToolCallResponse, ToolChoice,
};
use crate::server::services::deepseek::DeepSeekService;

impl DeepSeekService {
    pub async fn chat_with_tool_response(
        &self,
        messages: Vec<ChatMessage>,
        tool_response: ChatMessage,
        tools: Vec<Tool>,
        use_reasoner: bool,
    ) -> Result<(String, Option<String>, Option<Vec<ToolCallResponse>>)> {
        let model = if use_reasoner {
            "deepseek-reasoner"
        } else {
            "deepseek-chat"
        };

        // Find the assistant message with tool calls
        let has_assistant_with_tools = messages
            .iter()
            .any(|msg| msg.role == "assistant" && msg.tool_calls.is_some());

        if !has_assistant_with_tools {
            return Err(anyhow::anyhow!(
                "No assistant message with tool calls found"
            ));
        }

        // Make sure tool response has tool_call_id
        if tool_response.tool_call_id.is_none() {
            return Err(anyhow::anyhow!("Tool response must have tool_call_id"));
        }

        // Create a new sequence of messages with the tool response
        let mut all_messages = messages;
        all_messages.push(tool_response);

        // Debug print the messages
        info!("Sending messages to API:");
        for (i, msg) in all_messages.iter().enumerate() {
            info!("Message {}: role={}, content={}", i, msg.role, msg.content);
            if let Some(tool_calls) = &msg.tool_calls {
                info!("  Tool calls: {:?}", tool_calls);
            }
            if let Some(tool_call_id) = &msg.tool_call_id {
                info!("  Tool call ID: {}", tool_call_id);
            }
        }

        let request = ChatRequest {
            model: model.to_string(),
            messages: all_messages,
            stream: false,
            temperature: 0.7,
            max_tokens: None,
            tools: Some(tools),
            tool_choice: Some(ToolChoice::Auto("none".to_string())),
        };

        let url = format!("{}/chat/completions", self.base_url);
        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await?;

        // Add status code check
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await?;
            return Err(anyhow::anyhow!(
                "API request failed with status {}: {}",
                status,
                text
            ));
        }

        // Get response text for debugging
        let text = response.text().await?;
        info!("API Response: {}", text);

        if text.is_empty() {
            return Err(anyhow::anyhow!("Empty response from API"));
        }

        // Parse the response
        let chat_response: ChatResponse = serde_json::from_str(&text).map_err(|e| {
            anyhow::anyhow!("Failed to parse response: {}\nResponse text: {}", e, text)
        })?;

        if let Some(choice) = chat_response.choices.first() {
            info!("Response content: {}", choice.message.content);
            Ok((
                choice.message.content.clone(),
                choice.message.reasoning_content.clone(),
                choice.message.tool_calls.clone(),
            ))
        } else {
            Err(anyhow::anyhow!("No response from model"))
        }
    }
}
