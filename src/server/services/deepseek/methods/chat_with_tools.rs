use anyhow::Result;
use tracing::info;

use crate::server::services::deepseek::types::{
    ChatMessage, ChatRequest, ChatResponse, Tool, ToolCallResponse, ToolChoice,
};
use crate::server::services::deepseek::DeepSeekService;

impl DeepSeekService {
    pub async fn chat_with_tools(
        &self,
        prompt: String,
        tools: Vec<Tool>,
        tool_choice: Option<ToolChoice>,
        use_reasoner: bool,
    ) -> Result<(String, Option<String>, Option<Vec<ToolCallResponse>>)> {
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: prompt,
            tool_call_id: None,
            tool_calls: None,
        }];

        self.chat_with_tools_messages(messages, tools, tool_choice, use_reasoner)
            .await
    }

    pub async fn chat_with_tools_messages(
        &self,
        messages: Vec<ChatMessage>,
        tools: Vec<Tool>,
        tool_choice: Option<ToolChoice>,
        use_reasoner: bool,
    ) -> Result<(String, Option<String>, Option<Vec<ToolCallResponse>>)> {
        let model = if use_reasoner {
            "deepseek-reasoner"
        } else {
            "deepseek-chat"
        };

        // Only include tools if we have them and we're not using the reasoner
        let tools = if !tools.is_empty() && !use_reasoner {
            Some(tools)
        } else {
            None
        };

        let request = ChatRequest {
            model: model.to_string(),
            messages,
            stream: false,
            temperature: 0.7,
            max_tokens: None,
            tools,
            tool_choice: if tools.is_some() { tool_choice } else { None },
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