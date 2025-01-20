use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
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
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatResponseMessage {
    role: String,
    content: String,
    reasoning_content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
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
            stream: false,
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

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::mock;
    use serde_json::json;

    #[tokio::test]
    async fn test_chat_basic() {
        let mut server = mockito::Server::new();
        let mock_response = json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "Hello! How can I help you?",
                    "reasoning_content": null
                },
                "finish_reason": "stop"
            }]
        });

        let _m = server.mock("POST", "/chat/completions")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_response.to_string())
            .create();

        let service = DeepSeekService::with_base_url(
            "test_key".to_string(),
            server.url(),
        );

        let (response, reasoning) = service.chat("Hello".to_string(), false).await.unwrap();
        assert_eq!(response, "Hello! How can I help you?");
        assert_eq!(reasoning, None);
    }

    #[tokio::test]
    async fn test_chat_with_reasoning() {
        let mut server = mockito::Server::new();
        let mock_response = json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "9.11 is greater than 9.8",
                    "reasoning_content": "Let's compare these numbers:\n9.11 vs 9.8\n9.11 = 9 + 0.11\n9.8 = 9 + 0.8\n0.8 is greater than 0.11\nTherefore, 9.8 is greater than 9.11"
                },
                "finish_reason": "stop"
            }]
        });

        let _m = server.mock("POST", "/chat/completions")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_response.to_string())
            .create();

        let service = DeepSeekService::with_base_url(
            "test_key".to_string(),
            server.url(),
        );

        let (response, reasoning) = service.chat("Compare 9.11 and 9.8".to_string(), true).await.unwrap();
        assert_eq!(response, "9.11 is greater than 9.8");
        assert!(reasoning.is_some());
        assert!(reasoning.unwrap().contains("Let's compare these numbers"));
    }
}