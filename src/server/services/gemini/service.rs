use anyhow::{anyhow, Result};
use futures::stream::StreamExt;
use reqwest::Client;
use serde_json::Value;
use std::env;
use tokio::sync::mpsc;
use tracing::{debug, error};

use super::types::*;

const GEMINI_API_URL: &str = "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent";
const GEMINI_STREAM_URL: &str = "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:streamGenerateContent";

pub struct GeminiService {
    client: Client,
    api_key: String,
}

impl GeminiService {
    pub fn new() -> Result<Self> {
        let api_key = env::var("GEMINI_API_KEY")
            .map_err(|_| anyhow!("GEMINI_API_KEY environment variable not set"))?;

        Ok(Self {
            client: Client::new(),
            api_key,
        })
    }

    pub async fn analyze_files(
        &self,
        issue_description: &str,
        valid_paths: &[String],
        repo_context: &str,
    ) -> Result<Value> {
        let prompt = format!(
            "You are a code analysis expert. Given a GitHub issue and repository context, identify the most relevant files that need to be modified to implement the solution.\n\n\
            Issue Description:\n{}\n\n\
            Repository Context:\n{}\n\n\
            Available Files:\n{}\n\n\
            Return a JSON object with a 'files' array containing objects with:\n\
            - 'path' (relative path, no leading slash)\n\
            - 'relevance_score' (1-10, where 10 is most relevant)\n\
            - 'reason' (detailed explanation for why this file needs changes)\n\n\
            IMPORTANT:\n\
            1. Only use paths from the Available Files list\n\
            2. Include up to 10 most relevant files\n\
            3. Provide clear reasoning for each file\n\
            4. Ensure paths are relative with no leading slash\n\
            5. Score relevance from 1-10 based on how critical the file is to the solution",
            issue_description,
            repo_context,
            valid_paths.join("\n")
        );

        let request = GeminiRequest {
            contents: vec![Content {
                parts: vec![Part { text: prompt }],
            }],
            generation_config: GenerationConfig {
                temperature: 0.4, // Lower temperature for more focused output
                max_output_tokens: 1024,
                ..Default::default()
            },
            ..Default::default()
        };

        let url = format!("{}?key={}", GEMINI_API_URL, self.api_key);
        
        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await?
            .json::<GeminiResponse>()
            .await?;

        if response.candidates.is_empty() {
            return Err(anyhow!("No response generated"));
        }

        let content = &response.candidates[0].content.parts[0].text;
        debug!("Gemini response: {}", content);

        // Extract JSON from the response text
        let json_str = if let Some(start) = content.find('{') {
            if let Some(end) = content.rfind('}') {
                &content[start..=end]
            } else {
                return Err(anyhow!("Invalid JSON in response - no closing brace"));
            }
        } else {
            return Err(anyhow!("Invalid JSON in response - no opening brace"));
        };

        serde_json::from_str(json_str).map_err(|e| {
            error!("Failed to parse JSON: {}", e);
            anyhow!("Failed to parse JSON response: {}", e)
        })
    }

    pub async fn analyze_files_stream(
        &self,
        issue_description: &str,
        valid_paths: &[String],
        repo_context: &str,
    ) -> mpsc::Receiver<StreamUpdate> {
        let (tx, rx) = mpsc::channel(100);
        let api_key = self.api_key.clone();
        let client = self.client.clone();

        let prompt = format!(
            "You are a code analysis expert. Given a GitHub issue and repository context, identify the most relevant files that need to be modified to implement the solution.\n\n\
            Issue Description:\n{}\n\n\
            Repository Context:\n{}\n\n\
            Available Files:\n{}\n\n\
            Return a JSON object with a 'files' array containing objects with:\n\
            - 'path' (relative path, no leading slash)\n\
            - 'relevance_score' (1-10, where 10 is most relevant)\n\
            - 'reason' (detailed explanation for why this file needs changes)\n\n\
            IMPORTANT:\n\
            1. Only use paths from the Available Files list\n\
            2. Include up to 10 most relevant files\n\
            3. Provide clear reasoning for each file\n\
            4. Ensure paths are relative with no leading slash\n\
            5. Score relevance from 1-10 based on how critical the file is to the solution",
            issue_description,
            repo_context,
            valid_paths.join("\n")
        );

        let request = GeminiRequest {
            contents: vec![Content {
                parts: vec![Part { text: prompt }],
            }],
            generation_config: GenerationConfig {
                temperature: 0.4,
                max_output_tokens: 1024,
                ..Default::default()
            },
            ..Default::default()
        };

        tokio::spawn(async move {
            let url = format!("{}?alt=sse&key={}", GEMINI_STREAM_URL, api_key);
            
            match client.post(&url)
                .header("Content-Type", "application/json")
                .header("Accept", "text/event-stream")
                .json(&request)
                .send()
                .await
            {
                Ok(response) => {
                    let mut stream = response.bytes_stream();
                    let mut buffer = String::new();

                    while let Some(chunk_result) = stream.next().await {
                        match chunk_result {
                            Ok(chunk) => {
                                if let Ok(text) = String::from_utf8(chunk.to_vec()) {
                                    buffer.push_str(&text);
                                    
                                    // Process SSE events
                                    for line in buffer.lines() {
                                        if line.starts_with("data: ") {
                                            let data = &line["data: ".len()..];
                                            if data == "[DONE]" {
                                                let _ = tx.send(StreamUpdate::Done).await;
                                                break;
                                            }
                                            
                                            if let Ok(response) = serde_json::from_str::<GeminiResponse>(data) {
                                                if let Some(candidate) = response.candidates.first() {
                                                    if let Some(part) = candidate.content.parts.first() {
                                                        let _ = tx.send(StreamUpdate::Content(part.text.clone())).await;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    buffer.clear();
                                }
                            }
                            Err(e) => {
                                error!("Error reading stream: {}", e);
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to start stream: {}", e);
                }
            }
        });

        rx
    }
}