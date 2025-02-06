use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::Value;
use std::env;
use tracing::{debug, error};

use super::types::*;

const GEMINI_API_URL: &str = "https://generativelanguage.googleapis.com/v1/models/gemini-pro:generateContent";

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
}