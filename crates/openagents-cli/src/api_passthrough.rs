//! Generic authenticated API passthrough command (`oa api`)
//! Talking to real `/api/v1` routes with dynamic methods and arbitrary JSON payloads

use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponseEnvelope {
    pub status: u16,
    pub body: serde_json::Value,
}

pub struct ApiPassthroughClient {
    pub api_base: String,
    pub token: Option<String>,
    pub http: reqwest::Client,
}

impl ApiPassthroughClient {
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

    pub async fn execute_request(
        &self,
        method: &str,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let clean_path = if path.starts_with('/') { path.to_string() } else { format!("/{}", path) };
        let url = format!("{}{}", self.api_base, clean_path);

        let method_upper = method.to_uppercase();
        let mut req_builder = match method_upper.as_str() {
            "POST" => self.http.post(&url),
            "PUT" => self.http.put(&url),
            "PATCH" => self.http.patch(&url),
            "DELETE" => self.http.delete(&url),
            _ => self.http.get(&url),
        };

        req_builder = req_builder.headers(self.headers());

        if let Some(b) = body {
            req_builder = req_builder.json(&b);
        }

        let resp = req_builder.send().await?;
        let status = resp.status();
        let json_body = resp.json::<serde_json::Value>().await.unwrap_or_else(|_| serde_json::json!({
            "status": status.as_u16(),
        }));

        Ok(json_body)
    }
}
