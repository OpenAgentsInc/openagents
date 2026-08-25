//! Real memory client for account-level knowledge and learned corrections
//! Communicates with `POST /api/v1/memories`, `GET /api/v1/memories`, `DELETE /api/v1/memories/:id`

use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecord {
    pub id: String,
    pub bucket: String,
    pub body: String,
    pub source_ref: Option<String>,
    pub superseded_by: Option<String>,
    pub created_at: String,
}

pub struct MemoryClient {
    pub api_base: String,
    pub token: Option<String>,
    pub http: reqwest::Client,
}

impl MemoryClient {
    pub fn new(api_base: &str, token: Option<String>) -> Self {
        Self {
            api_base: api_base.trim_end_matches('/').to_string(),
            token,
            http: reqwest::Client::new(),
        }
    }

    fn headers(&self) -> HeaderMap {
        let mut map = HeaderMap::new();
        map.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        if let Some(tok) = &self.token {
            if let Ok(val) = HeaderValue::from_str(&format!("Bearer {}", tok)) {
                map.insert(AUTHORIZATION, val);
            }
        }
        map
    }

    pub async fn list_memories(&self, bucket: Option<&str>) -> Result<Vec<MemoryRecord>, Box<dyn std::error::Error + Send + Sync>> {
        let mut url = format!("{}/memories", self.api_base);
        if let Some(b) = bucket {
            url.push_str(&format!("?bucket={}", b));
        }

        let resp = self.http.get(&url).headers(self.headers()).send().await?;
        if resp.status().is_success() {
            let body: serde_json::Value = resp.json().await?;
            let items = body.get("memories").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            let mut records = Vec::new();
            for item in items {
                let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let bucket = item.get("bucket").and_then(|v| v.as_str()).unwrap_or("user").to_string();
                let body = item.get("body").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let source_ref = item.get("source_ref").and_then(|v| v.as_str()).map(String::from);
                let superseded_by = item.get("superseded_by").and_then(|v| v.as_str()).map(String::from);
                let created_at = item.get("created_at").and_then(|v| v.as_str()).unwrap_or("").to_string();
                records.push(MemoryRecord {
                    id,
                    bucket,
                    body,
                    source_ref,
                    superseded_by,
                    created_at,
                });
            }
            Ok(records)
        } else {
            Ok(Vec::new())
        }
    }

    pub async fn add_memory(&self, body_text: &str, bucket: Option<&str>) -> Result<Option<MemoryRecord>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/memories", self.api_base);
        let resp = self.http.post(&url).headers(self.headers()).json(&serde_json::json!({
            "body": body_text,
            "bucket": bucket.unwrap_or("user")
        })).send().await?;

        if resp.status().is_success() {
            let item: serde_json::Value = resp.json().await?;
            let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let created_at = item.get("created_at").and_then(|v| v.as_str()).unwrap_or("").to_string();
            Ok(Some(MemoryRecord {
                id,
                bucket: bucket.unwrap_or("user").to_string(),
                body: body_text.to_string(),
                source_ref: None,
                superseded_by: None,
                created_at,
            }))
        } else {
            Ok(None)
        }
    }
}
