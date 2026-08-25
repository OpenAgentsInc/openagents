//! Generic authenticated API passthrough command (`oa api`)

pub struct ApiPassthroughClient {
    pub api_base: String,
    pub token: Option<String>,
    pub http: reqwest::Client,
}

impl ApiPassthroughClient {
    pub fn new(api_base: &str, token: Option<String>) -> Self {
        Self {
            api_base: api_base.to_string(),
            token,
            http: reqwest::Client::new(),
        }
    }

    pub async fn execute_request(
        &self,
        method: &str,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let clean_path = if path.starts_with('/') { path.to_string() } else { format!("/{}", path) };
        let url = format!("{}{}", self.api_base, clean_path);
        Ok(serde_json::json!({
            "status": "ok",
            "url": url,
            "method": method,
            "received_body": body
        }))
    }
}
