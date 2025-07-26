use crate::claude_code::models::{ClaudeError, ConvexSession};
use reqwest;
use serde_json::{json, Value};
use log::{debug, error, info};

pub struct ConvexClient {
    base_url: String,
    client: reqwest::Client,
}

impl ConvexClient {
    pub fn new(convex_url: &str) -> Self {
        Self {
            base_url: convex_url.to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Fetch sessions from Convex database
    pub async fn get_sessions(&self, limit: Option<usize>, user_id: Option<String>) -> Result<Vec<ConvexSession>, ClaudeError> {
        let url = format!("{}/api/query", self.base_url);
        
        // Prepare the query body
        let mut args = json!({
            "limit": limit.unwrap_or(50)
        });
        
        if let Some(uid) = user_id {
            args["userId"] = json!(uid);
        }
        
        let body = json!({
            "query": "claude:getSessions",
            "args": args
        });

        debug!("Making Convex request to: {}", url);
        debug!("Request body: {}", serde_json::to_string_pretty(&body).unwrap_or_default());

        let response = self.client
            .post(&url)
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            error!("Convex API error {}: {}", status, error_text);
            return Err(ClaudeError::Other(format!("Convex API error {}: {}", status, error_text)));
        }

        let response_body: Value = response.json().await?;
        debug!("Convex response: {}", serde_json::to_string_pretty(&response_body).unwrap_or_default());

        // Handle Convex response format
        if let Some(error) = response_body.get("error") {
            error!("Convex query error: {}", error);
            return Err(ClaudeError::Other(format!("Convex query error: {}", error)));
        }

        let sessions_value = response_body.get("result")
            .ok_or_else(|| ClaudeError::Other("No result field in Convex response".to_string()))?;

        let sessions: Vec<ConvexSession> = serde_json::from_value(sessions_value.clone())
            .map_err(|e| {
                error!("Failed to parse Convex sessions: {}", e);
                error!("Raw sessions data: {}", serde_json::to_string_pretty(sessions_value).unwrap_or_default());
                ClaudeError::JsonError(e)
            })?;

        info!("Successfully fetched {} sessions from Convex", sessions.len());
        Ok(sessions)
    }

    /// Test connection to Convex
    pub async fn test_connection(&self) -> Result<bool, ClaudeError> {
        let url = format!("{}/api/query", self.base_url);
        
        let body = json!({
            "query": "claude:getSessions",
            "args": {
                "limit": 1
            }
        });

        debug!("Testing Convex connection to: {}", url);

        let response = self.client
            .post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await?;

        let is_success = response.status().is_success();
        if is_success {
            info!("Convex connection test successful");
        } else {
            error!("Convex connection test failed with status: {}", response.status());
        }

        Ok(is_success)
    }
}