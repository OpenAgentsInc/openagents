use anyhow::Result;
use futures::{Stream, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{pin::Pin, time::Duration};
use tokio::sync::mpsc;
use tracing::{error, info};

#[derive(Debug, Clone)]
pub struct OpenRouterService {
    client: Client,
    api_key: String,
    base_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InferenceResponse {
    pub output: String,
}

pub type StreamingOutput = Pin<Box<dyn Stream<Item = Result<String>> + Send>>;

impl OpenRouterService {
    pub fn new(api_key: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))  // 2 minute timeout
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            api_key,
            base_url: "https://openrouter.ai/api/v1".to_string(),
        }
    }

    pub fn with_base_url(api_key: String, base_url: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))  // 2 minute timeout
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            api_key,
            base_url,
        }
    }

    pub async fn inference_stream(&self, prompt: String) -> Result<StreamingOutput> {
        info!("Making streaming inference request to OpenRouter");
        // info!("Sending prompt to OpenRouter: {}", prompt);

        let request_body = serde_json::json!({
            "model": "deepseek/deepseek-chat",
            "messages": [{
                "role": "user",
                "content": prompt
            }],
            "stream": true
        });


        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("HTTP-Referer", "https://openagents.com")
            .header("X-Title", "OpenAgents")
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await?;
            error!("OpenRouter API error ({}): {}", status, error_text);
            return Err(anyhow::anyhow!(
                "OpenRouter API error ({}): {}",
                status,
                error_text
            ));
        }

        let stream = response.bytes_stream();
        let (tx, rx) = mpsc::channel(32);

        // Spawn a task to process the stream
        tokio::spawn(async move {
            let mut buffer = String::new();
            let mut stream = stream;

            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        if let Ok(text) = String::from_utf8(chunk.to_vec()) {
                            buffer.push_str(&text);
                            
                            // Process complete messages
                            while let Some(pos) = buffer.find('\n') {
                                let line = buffer[..pos].trim().to_string();
                                let remaining = buffer[pos + 1..].to_string();
                                buffer = remaining;

                                if line.starts_with("data: ") {
                                    let data = &line["data: ".len()..];
                                    if data == "[DONE]" {
                                        break;
                                    }
                                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                                        if let Some(content) = json["choices"][0]["delta"]["content"]
                                            .as_str()
                                        {
                                            if tx.send(Ok(content.to_string())).await.is_err() {
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(anyhow::anyhow!("Stream error: {}", e))).await;
                        break;
                    }
                }
            }
        });

        // Convert receiver into a Stream
        Ok(Box::pin(tokio_stream::wrappers::ReceiverStream::new(rx)))
    }
    pub async fn inference(&self, prompt: String) -> Result<InferenceResponse> {
        info!("Making non-streaming inference request to OpenRouter");
        
        let mut stream = self.inference_stream(prompt).await?;
        let mut output = String::new();
        
        while let Some(chunk) = stream.next().await {
            output.push_str(&chunk?);
        }
        
        Ok(InferenceResponse { output })
    }
}
