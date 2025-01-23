use futures::StreamExt;
use tokio::sync::mpsc;
use tracing::info;
use crate::server::services::deepseek::{
    ChatMessage, ChatRequest, StreamResponse, StreamUpdate, DeepSeekService
};

impl DeepSeekService {
    pub async fn chat_stream(
        &self,
        prompt: String,
        use_reasoner: bool,
    ) -> mpsc::Receiver<StreamUpdate> {
        self.chat_stream_with_history(Vec::new(), prompt, use_reasoner).await
    }

    pub async fn chat_stream_with_history(
        &self,
        history: Vec<ChatMessage>,
        prompt: String,
        use_reasoner: bool,
    ) -> mpsc::Receiver<StreamUpdate> {
        let (tx, rx) = mpsc::channel(100);
        let client = self.client.clone();
        let api_key = self.api_key.clone();
        let base_url = self.base_url.clone();

        // Add current message to history
        let mut messages = history;
        messages.push(ChatMessage {
            role: "user".to_string(),
            content: prompt,
            tool_call_id: None,
            tool_calls: None,
        });

        tokio::spawn(async move {
            let model = if use_reasoner {
                "deepseek-reasoner"
            } else {
                "deepseek-chat"
            };

            let request = ChatRequest {
                model: model.to_string(),
                messages,
                stream: true,
                temperature: 0.7,
                max_tokens: None,
                tools: None,
                tool_choice: None,
            };

            let url = format!("{}/chat/completions", base_url);
            let response = client
                .post(&url)
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", api_key))
                .json(&request)
                .send()
                .await;

            match response {
                Ok(response) => {
                    let mut stream = response.bytes_stream();
                    let mut buffer = String::new();

                    while let Some(chunk) = stream.next().await {
                        match chunk {
                            Ok(chunk) => {
                                let chunk_str = String::from_utf8_lossy(&chunk);
                                buffer.push_str(&chunk_str);

                                // Process complete SSE messages
                                while let Some(pos) = buffer.find('\n') {
                                    let line = buffer[..pos].trim().to_string();
                                    buffer = buffer[pos + 1..].to_string();

                                    if let Some(data) = line.strip_prefix("data: ") {
                                        if data == "[DONE]" {
                                            let _ = tx.send(StreamUpdate::Done).await;
                                            break;
                                        }

                                        if let Ok(response) = serde_json::from_str::<StreamResponse>(data) {
                                            if let Some(choice) = response.choices.first() {
                                                if let Some(ref content) = choice.delta.content {
                                                    let _ = tx.send(StreamUpdate::Content(content.to_string())).await;
                                                }
                                                if let Some(ref reasoning) = choice.delta.reasoning_content {
                                                    let _ = tx.send(StreamUpdate::Reasoning(reasoning.to_string())).await;
                                                }
                                                if let Some(tool_calls) = &choice.delta.tool_calls {
                                                    let _ = tx.send(StreamUpdate::ToolCalls(tool_calls.clone())).await;
                                                }
                                                if choice.finish_reason.is_some() {
                                                    let _ = tx.send(StreamUpdate::Done).await;
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                info!("Stream error: {}", e);
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    info!("Request error: {}", e);
                }
            }
        });

        rx
    }
}