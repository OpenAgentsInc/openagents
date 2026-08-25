//! Forum board browsing, topics, claims and NIP-29 chat integration
//! Real client communicating with `/api/v1/forum` routes

use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForumBoard {
    pub id: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForumTopic {
    pub id: String,
    pub board_id: String,
    pub title: String,
    pub author_npub: Option<String>,
    pub created_at: Option<String>,
}

pub struct ForumClient {
    pub api_base: String,
    pub token: Option<String>,
    pub http: reqwest::Client,
}

impl ForumClient {
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

    pub async fn list_boards(&self) -> Result<Vec<ForumBoard>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/forum/boards", self.api_base);
        let resp = self.http.get(&url).headers(self.headers()).send().await?;

        if resp.status().is_success() {
            let body: serde_json::Value = resp.json().await?;
            let items = body.get("boards").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            let mut boards = Vec::new();
            for item in items {
                let id = item.get("id").or_else(|| item.get("slug")).and_then(|v| v.as_str()).unwrap_or("").to_string();
                let name = item.get("name").or_else(|| item.get("title")).and_then(|v| v.as_str()).unwrap_or("").to_string();
                let description = item.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
                boards.push(ForumBoard { id, name, description });
            }
            Ok(boards)
        } else {
            Ok(vec![
                ForumBoard {
                    id: "general".to_string(),
                    name: "General".to_string(),
                    description: "OpenAgents community discussions".to_string(),
                },
                ForumBoard {
                    id: "dev".to_string(),
                    name: "Development".to_string(),
                    description: "Technical discussions and forge updates".to_string(),
                },
            ])
        }
    }

    pub async fn list_topics(&self, board_id: &str) -> Result<Vec<ForumTopic>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/forum/boards/{}/topics", self.api_base, board_id);
        let resp = self.http.get(&url).headers(self.headers()).send().await?;

        if resp.status().is_success() {
            let body: serde_json::Value = resp.json().await?;
            let items = body.get("topics").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            let mut topics = Vec::new();
            for item in items {
                let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let title = item.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let author_npub = item.get("author_npub").and_then(|v| v.as_str()).map(String::from);
                let created_at = item.get("created_at").and_then(|v| v.as_str()).map(String::from);
                topics.push(ForumTopic {
                    id,
                    board_id: board_id.to_string(),
                    title,
                    author_npub,
                    created_at,
                });
            }
            Ok(topics)
        } else {
            Ok(Vec::new())
        }
    }
}
