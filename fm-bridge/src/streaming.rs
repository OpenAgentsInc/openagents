use crate::client::FMClient;
use crate::error::FMError;
use crate::types::*;
use futures::stream::Stream;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

/// Streaming client for Server-Sent Events (SSE) responses
pub struct StreamingClient {
    client: FMClient,
}

impl StreamingClient {
    /// Create a new streaming client
    pub fn new(client: FMClient) -> Self {
        Self { client }
    }

    /// Stream a chat completion
    pub async fn stream(
        &self,
        request: CompletionRequest,
    ) -> Result<impl Stream<Item = Result<StreamChunk, FMError>>, FMError> {
        let url = format!("{}/v1/chat/completions?stream=true", self.client.base_url());

        // Create request with stream=true in body as well
        let mut streaming_request = request;
        streaming_request.stream = Some(true);

        let response = self
            .client
            .http_client()
            .post(&url)
            .json(&streaming_request)
            .send()
            .await
            .map_err(|e| FMError::HttpError(e))?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(FMError::RequestFailed(error_text));
        }

        // Parse SSE stream
        let stream = response.bytes_stream();
        let sse_stream = parse_sse_stream(stream);

        Ok(sse_stream)
    }
}

/// A chunk from the streaming response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunk {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<StreamChoice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChoice {
    pub index: i32,
    pub delta: Delta,
    #[serde(rename = "finish_reason")]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Delta {
    pub role: Option<String>,
    pub content: Option<String>,
}

/// Parse Server-Sent Events stream
fn parse_sse_stream<S, E>(
    stream: S,
) -> Pin<Box<dyn Stream<Item = Result<StreamChunk, FMError>> + Send>>
where
    S: Stream<Item = Result<bytes::Bytes, E>> + Send + 'static,
    E: std::error::Error + Send + Sync + 'static,
{
    Box::pin(stream.map(move |result| {
        let bytes = result.map_err(|e| FMError::HttpError(
            reqwest::Error::new(reqwest::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        ))?;

        // Parse SSE format: "data: {json}\n\n"
        let text = String::from_utf8_lossy(&bytes);

        for line in text.lines() {
            if line.starts_with("data: ") {
                let json_str = &line[6..]; // Skip "data: " prefix

                // Check for [DONE] message
                if json_str.trim() == "[DONE]" {
                    continue;
                }

                // Parse JSON chunk
                if let Ok(chunk) = serde_json::from_str::<StreamChunk>(json_str) {
                    return Ok(chunk);
                }
            }
        }

        // If no valid chunk found, return an error
        Err(FMError::InvalidResponse("No valid SSE chunk in response".to_string()))
    }).filter_map(|result| async move {
        match result {
            Ok(chunk) => Some(Ok(chunk)),
            Err(FMError::InvalidResponse(_)) => None, // Filter out invalid chunks
            Err(e) => Some(Err(e)),
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_chunk_deserialization() {
        let json = r#"{
            "id": "fm-123",
            "object": "chat.completion.chunk",
            "created": 1234567890,
            "model": "apple-foundation-model",
            "choices": [{
                "index": 0,
                "delta": {
                    "role": "assistant",
                    "content": "Hello"
                },
                "finish_reason": null
            }]
        }"#;

        let chunk: StreamChunk = serde_json::from_str(json).unwrap();
        assert_eq!(chunk.id, "fm-123");
        assert_eq!(chunk.choices[0].delta.content.as_ref().unwrap(), "Hello");
    }

    #[test]
    fn test_final_chunk_deserialization() {
        let json = r#"{
            "id": "fm-123",
            "object": "chat.completion.chunk",
            "created": 1234567890,
            "model": "apple-foundation-model",
            "choices": [{
                "index": 0,
                "delta": {},
                "finish_reason": "stop"
            }]
        }"#;

        let chunk: StreamChunk = serde_json::from_str(json).unwrap();
        assert_eq!(chunk.choices[0].finish_reason.as_ref().unwrap(), "stop");
    }
}
