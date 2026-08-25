//! Real tracker client implementation for OpenAgents Issues, Projects, Comments, and Milestones
//! Talking to real `/api/v1` routes with authenticated requests

use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub body: Option<String>,
    pub author: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub body: Option<String>,
}

pub struct TrackerClient {
    pub api_base: String,
    pub token: Option<String>,
    pub http: reqwest::Client,
}

impl TrackerClient {
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

    pub async fn list_issues(&self, repo: &str) -> Result<Vec<Issue>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/repos/{}/issues", self.api_base, repo);
        let resp = self.http.get(&url).headers(self.headers()).send().await?;

        if resp.status().is_success() {
            let body: serde_json::Value = resp.json().await?;
            let items = body.get("issues").and_then(|v| v.as_array()).cloned().unwrap_or_else(|| {
                if let Some(arr) = body.as_array() { arr.clone() } else { Vec::new() }
            });

            let mut issues = Vec::new();
            for item in items {
                let number = item.get("number").and_then(|v| v.as_u64()).unwrap_or(0);
                let title = item.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let state = item.get("state").and_then(|v| v.as_str()).unwrap_or("open").to_string();
                let body_text = item.get("body").and_then(|v| v.as_str()).map(String::from);
                let author = item.get("author").and_then(|v| v.get("login")).and_then(|v| v.as_str()).map(String::from)
                    .or_else(|| item.get("user").and_then(|v| v.get("login")).and_then(|v| v.as_str()).map(String::from));
                let labels = item.get("labels").and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|l| l.get("name").and_then(|n| n.as_str()).map(String::from)).collect())
                    .unwrap_or_default();

                issues.push(Issue {
                    number,
                    title,
                    state,
                    body: body_text,
                    author,
                    labels,
                });
            }
            Ok(issues)
        } else {
            Ok(Vec::new())
        }
    }

    pub async fn get_issue(&self, repo: &str, number: u64) -> Result<Option<Issue>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/repos/{}/issues/{}", self.api_base, repo, number);
        let resp = self.http.get(&url).headers(self.headers()).send().await?;

        if resp.status().is_success() {
            let item: serde_json::Value = resp.json().await?;
            let number = item.get("number").and_then(|v| v.as_u64()).unwrap_or(number);
            let title = item.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let state = item.get("state").and_then(|v| v.as_str()).unwrap_or("open").to_string();
            let body_text = item.get("body").and_then(|v| v.as_str()).map(String::from);
            let author = item.get("author").and_then(|v| v.get("login")).and_then(|v| v.as_str()).map(String::from)
                .or_else(|| item.get("user").and_then(|v| v.get("login")).and_then(|v| v.as_str()).map(String::from));
            let labels = item.get("labels").and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|l| l.get("name").and_then(|n| n.as_str()).map(String::from)).collect())
                .unwrap_or_default();

            Ok(Some(Issue {
                number,
                title,
                state,
                body: body_text,
                author,
                labels,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn create_issue(&self, repo: &str, title: &str, body: Option<&str>) -> Result<Option<Issue>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/repos/{}/issues", self.api_base, repo);
        let mut payload = serde_json::json!({
            "title": title,
        });
        if let Some(b) = body {
            payload["body"] = serde_json::json!(b);
        }

        let resp = self.http.post(&url).headers(self.headers()).json(&payload).send().await?;
        if resp.status().is_success() {
            let item: serde_json::Value = resp.json().await?;
            let number = item.get("number").and_then(|v| v.as_u64()).unwrap_or(0);
            let state = item.get("state").and_then(|v| v.as_str()).unwrap_or("open").to_string();
            Ok(Some(Issue {
                number,
                title: title.to_string(),
                state,
                body: body.map(String::from),
                author: None,
                labels: Vec::new(),
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn close_issue(&self, repo: &str, number: u64) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/repos/{}/issues/{}", self.api_base, repo, number);
        let resp = self.http.patch(&url).headers(self.headers()).json(&serde_json::json!({
            "state": "closed"
        })).send().await?;
        Ok(resp.status().is_success())
    }

    pub async fn comment_issue(&self, repo: &str, number: u64, body: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/repos/{}/issues/{}/comments", self.api_base, repo, number);
        let resp = self.http.post(&url).headers(self.headers()).json(&serde_json::json!({
            "body": body
        })).send().await?;
        Ok(resp.status().is_success())
    }

    pub async fn list_projects(&self, repo: &str) -> Result<Vec<Project>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/repos/{}/projects", self.api_base, repo);
        let resp = self.http.get(&url).headers(self.headers()).send().await?;

        if resp.status().is_success() {
            let body: serde_json::Value = resp.json().await?;
            let items = body.get("projects").and_then(|v| v.as_array()).cloned().unwrap_or_else(|| {
                if let Some(arr) = body.as_array() { arr.clone() } else { Vec::new() }
            });

            let mut projects = Vec::new();
            for item in items {
                let number = item.get("number").and_then(|v| v.as_u64()).unwrap_or(0);
                let title = item.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let state = item.get("state").and_then(|v| v.as_str()).unwrap_or("open").to_string();
                let body_text = item.get("description").or_else(|| item.get("body")).and_then(|v| v.as_str()).map(String::from);
                projects.push(Project {
                    number,
                    title,
                    state,
                    body: body_text,
                });
            }
            Ok(projects)
        } else {
            Ok(Vec::new())
        }
    }
}
