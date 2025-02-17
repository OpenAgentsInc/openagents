use futures::StreamExt;
use tokio::sync::mpsc;
use tracing::{debug, error, info};
use bytes::Bytes;
use reqwest::Response;
use tokio_stream::Stream;
use serde::Deserialize;

use crate::server::services::deepseek::streaming::{StreamResponse, StreamUpdate};
use crate::server::services::deepseek::types::{ChatMessage, ChatRequest};
use crate::server::services::deepseek::DeepSeekService;

impl DeepSeekService {
    pub async fn chat_stream(
        &self,
        prompt: String,
        _use_reasoner: bool,
    ) -> mpsc::Receiver<StreamUpdate> {
        let (tx, rx) = mpsc::channel(100);
        let client = self.client.clone();
        let api_key = self.api_key.clone();
        let base_url = self.base_url.clone();

        tokio::spawn(async move {
            // Always use deepseek-reasoner for better streaming support
            let model = "deepseek-reasoner";

            let messages = vec![ChatMessage {
                role: "user".to_string(),
                content: prompt.clone(),
                tool_call_id: None,
                tool_calls: None,
            }];

            let request = ChatRequest {
                model: model.to_string(),
                messages,
                stream: true,
                temperature: 0.0, // Reasoner ignores temperature, set to 0
                max_tokens: Some(4096),
                tools: None,
                tool_choice: None,
            };

            let url = format!("{}/chat/completions", base_url);
            info!("Making streaming request to {} with model {}", url, model);
            debug!("Request prompt: {}", prompt);
            debug!("Full request: {:?}", request);

            let response = client
                .post(&url)
                .header("Content-Type", "application/json")
                .header("Accept", "text/event-stream")
                .header("Cache-Control", "no-cache")
                .header("Connection", "keep-alive")
                .header("Authorization", format!("Bearer {}", api_key))
                .json(&request)
                .send()
                .await;

            match response {
                Ok(response) => {
                    info!("Got initial response with status: {}", response.status());
                    debug!("Response headers: {:?}", response.headers());

                    if !response.status().is_success() {
                        let status = response.status();
                        let error_text = response.text().await.unwrap_or_default();
                        error!("DeepSeek API error: {} - {}", status, error_text);
                        return;
                    }

                    let mut stream = response.bytes_stream();
                    let mut buffer = String::new();
                    let mut keep_alive_count = 0;

                    while let Some(chunk_result) = stream.next().await {
                        match chunk_result {
                            Ok(chunk) => {
                                let chunk_str = String::from_utf8_lossy(&chunk);
                                debug!("Raw chunk received: {:?}", chunk_str);

                                buffer.push_str(&chunk_str);

                                // Process complete SSE messages
                                while let Some(pos) = buffer.find('\n') {
                                    let line = buffer[..pos].trim().to_string();
                                    buffer = buffer[pos + 1..].to_string();

                                    debug!("Processing line: {:?}", line);

                                    // Skip empty lines and keep-alive messages
                                    if line.is_empty() || line == ": keep-alive" {
                                        if line == ": keep-alive" {
                                            keep_alive_count += 1;
                                            info!("Received keep-alive #{}", keep_alive_count);

                                            // If we've received too many keep-alives with no content, abort
                                            if keep_alive_count > 5 {
                                                error!("Too many keep-alives without content, aborting");
                                                let _ = tx.send(StreamUpdate::Done).await;
                                                return;
                                            }
                                        }
                                        continue;
                                    }

                                    // Reset keep-alive count when we get actual data
                                    keep_alive_count = 0;

                                    if let Some(data) = line.strip_prefix("data: ") {
                                        debug!("Processing data: {:?}", data);
                                        if data == "[DONE]" {
                                            info!("Received [DONE] message");
                                            let _ = tx.send(StreamUpdate::Done).await;
                                            break;
                                        }

                                        match serde_json::from_str::<StreamResponse>(data) {
                                            Ok(response) => {
                                                debug!("Parsed stream response: {:?}", response);
                                                if let Some(choice) = response.choices.first() {
                                                    // First check for reasoning content since it comes first
                                                    if let Some(ref reasoning) =
                                                        choice.delta.reasoning_content
                                                    {
                                                        if !reasoning.is_empty() {
                                                            info!(
                                                                "Sending reasoning update: {:?}",
                                                                reasoning
                                                            );
                                                            let _ = tx
                                                                .send(StreamUpdate::Reasoning(
                                                                    reasoning.to_string(),
                                                                ))
                                                                .await;
                                                        }
                                                    }
                                                    // Then check for regular content
                                                    if let Some(ref content) = choice.delta.content
                                                    {
                                                        if !content.is_empty() {
                                                            info!(
                                                                "Sending content update: {:?}",
                                                                content
                                                            );
                                                            let _ = tx
                                                                .send(StreamUpdate::Content(
                                                                    content.to_string(),
                                                                ))
                                                                .await;
                                                        }
                                                    }
                                                    if choice.finish_reason.is_some() {
                                                        info!(
                                                            "Received finish reason: {:?}",
                                                            choice.finish_reason
                                                        );
                                                        let _ = tx.send(StreamUpdate::Done).await;
                                                        break;
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                error!("Failed to parse stream response: {} - Data: {}", e, data);
                                                debug!("Parse error details: {:?}", e);
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                error!("Stream error: {}", e);
                                debug!("Stream error details: {:?}", e);
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to send request: {}", e);
                    debug!("Request error details: {:?}", e);
                }
            }
        });

        rx
    }
}

async fn handle_stream(response: Response) -> impl Stream<Item = Result<Bytes, anyhow::Error>> {
    let stream = response.bytes_stream();
    tokio_stream::StreamExt::map(stream, |chunk| {
        chunk.map_err(anyhow::Error::from)
    })
}
