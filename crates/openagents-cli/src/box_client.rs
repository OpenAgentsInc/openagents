//! Box sandbox management, remote execution and parallel fanout
//! Real client communicating with `/api/v1/conversations/:id/boxes`

use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoxRecord {
    pub box_id: String,
    pub label: Option<String>,
    pub state: String,
    pub setup_status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoxCommandResult {
    pub box_id: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

pub struct BoxClient {
    pub api_base: String,
    pub token: Option<String>,
    pub http: reqwest::Client,
}

impl BoxClient {
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

    pub async fn list_boxes(&self, conversation_id: &str) -> Result<Vec<BoxRecord>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/conversations/{}/boxes", self.api_base, conversation_id);
        let resp = self.http.get(&url).headers(self.headers()).send().await?;

        if resp.status().is_success() {
            let body: serde_json::Value = resp.json().await?;
            let items = body.get("boxes").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            let mut records = Vec::new();
            for item in items {
                let box_id = item.get("box_id").or_else(|| item.get("id")).and_then(|v| v.as_str()).unwrap_or("").to_string();
                let label = item.get("label").and_then(|v| v.as_str()).map(String::from);
                let state = item.get("state").and_then(|v| v.as_str()).unwrap_or("active").to_string();
                let setup_status = item.get("setup_status").and_then(|v| v.as_str()).unwrap_or("ready").to_string();
                let created_at = item.get("created_at").and_then(|v| v.as_str()).unwrap_or("").to_string();
                records.push(BoxRecord {
                    box_id,
                    label,
                    state,
                    setup_status,
                    created_at,
                });
            }
            Ok(records)
        } else {
            Ok(Vec::new())
        }
    }

    pub async fn create_box(&self, conversation_id: &str, label: Option<&str>) -> Result<Option<BoxRecord>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/conversations/{}/boxes", self.api_base, conversation_id);
        let mut payload = serde_json::json!({});
        if let Some(lbl) = label {
            payload["label"] = serde_json::json!(lbl);
        }

        let resp = self.http.post(&url).headers(self.headers()).json(&payload).send().await?;
        if resp.status().is_success() {
            let item: serde_json::Value = resp.json().await?;
            let box_id = item.get("box_id").or_else(|| item.get("id")).and_then(|v| v.as_str()).unwrap_or("").to_string();
            let state = item.get("state").and_then(|v| v.as_str()).unwrap_or("provisioning").to_string();
            let setup_status = item.get("setup_status").and_then(|v| v.as_str()).unwrap_or("pending").to_string();
            let created_at = item.get("created_at").and_then(|v| v.as_str()).unwrap_or("").to_string();
            Ok(Some(BoxRecord {
                box_id,
                label: label.map(String::from),
                state,
                setup_status,
                created_at,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn execute_command(&self, conversation_id: &str, box_id: &str, command: &str) -> Result<BoxCommandResult, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/conversations/{}/boxes/{}/exec", self.api_base, conversation_id, box_id);
        let resp = self.http.post(&url).headers(self.headers()).json(&serde_json::json!({
            "command": command
        })).send().await?;

        if resp.status().is_success() {
            let body: serde_json::Value = resp.json().await?;
            let exit_code = body.get("exit_code").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let stdout = body.get("stdout").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let stderr = body.get("stderr").and_then(|v| v.as_str()).unwrap_or("").to_string();
            Ok(BoxCommandResult {
                box_id: box_id.to_string(),
                exit_code,
                stdout,
                stderr,
            })
        } else {
            Ok(BoxCommandResult {
                box_id: box_id.to_string(),
                exit_code: 1,
                stdout: String::new(),
                stderr: format!("Box execution request failed with status {}", resp.status()),
            })
        }
    }
}
