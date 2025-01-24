use anyhow::Result;
use tracing::info;

use crate::server::services::deepseek::types::{ChatMessage, ChatRequest, ChatResponse};
use crate::server::services::deepseek::DeepSeekService;

impl DeepSeekService {
    pub async fn chat(
        &self,
        prompt: String,
        use_reasoner: bool,
    ) -> Result<(String, Option<String>)> {
        self.chat_internal(prompt, use_reasoner, false).await
    }

    pub(crate) async fn chat_internal(
        &self,
        prompt: String,
        use_reasoner: bool,
        stream: bool,
    ) -> Result<(String, Option<String>)> {
        info!("Making chat request to DeepSeek API");

        let model = if use_reasoner {
            "deepseek-reasoner"
        } else {
            "deepseek-chat"
        };

        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: prompt,
            tool_call_id: None,
            tool_calls: None,
        }];

        let request = ChatRequest {
            model: model.to_string(),
            messages,
            stream,
            temperature: 0.7,
            max_tokens: None,
            tools: None,
            tool_choice: None,
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

        let chat_response: ChatResponse = response.json().await?;

        if let Some(choice) = chat_response.choices.first() {
            Ok((
                choice.message.content.clone(),
                choice.message.reasoning_content.clone(),
            ))
        } else {
            Err(anyhow::anyhow!("No response from model"))
        }
    }
}
