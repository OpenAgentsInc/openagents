use anyhow::Result;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Debug, Clone)]
pub struct DeepSeekService {
    client: Client,
    api_key: String,
    base_url: String,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatResponseMessage,
}

#[derive(Debug, Deserialize)]
struct ChatResponseMessage {
    content: String,
    reasoning_content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

// Streaming response types
#[derive(Debug, Deserialize)]
struct StreamChoice {
    delta: StreamDelta,
}

#[derive(Debug, Deserialize)]
struct StreamDelta {
    content: Option<String>,
    reasoning_content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StreamResponse {
    choices: Vec<StreamChoice>,
}

impl DeepSeekService {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            base_url: "https://api.deepseek.com".to_string(),
        }
    }

    pub fn with_base_url(api_key: String, base_url: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            base_url,
        }
    }

    pub async fn chat(&self, prompt: String, use_reasoner: bool) -> Result<(String, Option<String>)> {
        self.chat_internal(prompt, use_reasoner, false).await
    }

    pub async fn chat_stream<F>(&self, prompt: String, use_reasoner: bool, mut callback: F) -> Result<()>
    where
        F: FnMut(Option<&str>, Option<&str>) -> Result<()>,
    {
        let model = if use_reasoner {
            "deepseek-reasoner"
        } else {
            "deepseek-chat"
        };

        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: prompt,
        }];

        let request = ChatRequest {
            model: model.to_string(),
            messages,
            stream: true,
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

        let mut stream = response.bytes_stream();
        let mut content = String::new();
        let mut reasoning = String::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            let text = String::from_utf8_lossy(&chunk);
            
            // Split by "data: " and process each SSE message
            for line in text.split("data: ") {
                let line = line.trim();
                if line.is_empty() || line == "[DONE]" {
                    continue;
                }

                if let Ok(response) = serde_json::from_str::<StreamResponse>(line) {
                    if let Some(choice) = response.choices.first() {
                        if let Some(ref c) = choice.delta.content {
                            content.push_str(c);
                            callback(Some(c), None)?;
                        }
                        if let Some(ref r) = choice.delta.reasoning_content {
                            reasoning.push_str(r);
                            callback(None, Some(r))?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn chat_internal(&self, prompt: String, use_reasoner: bool, stream: bool) -> Result<(String, Option<String>)> {
        info!("Making chat request to DeepSeek API");
        
        let model = if use_reasoner {
            "deepseek-reasoner"
        } else {
            "deepseek-chat"
        };

        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: prompt,
        }];

        let request = ChatRequest {
            model: model.to_string(),
            messages,
            stream,
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