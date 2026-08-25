//! Forge repository management, clone, import and git credential helper
//! Talking to real `/api/v1` routes and executing git processes

use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub id: String,
    pub slug: String,
    pub is_private: bool,
    pub default_branch: String,
}

pub struct RepoClient {
    pub api_base: String,
    pub token: Option<String>,
    pub http: reqwest::Client,
}

impl RepoClient {
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

    pub async fn list_repos(&self) -> Result<Vec<Repository>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/user/repos", self.api_base);
        let resp = self.http.get(&url).headers(self.headers()).send().await?;

        if resp.status().is_success() {
            let body: serde_json::Value = resp.json().await?;
            let items = body.get("repositories").and_then(|v| v.as_array()).cloned().unwrap_or_else(|| {
                if let Some(arr) = body.as_array() { arr.clone() } else { Vec::new() }
            });

            let mut repos = Vec::new();
            for item in items {
                let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let full_name = item.get("full_name").or_else(|| item.get("slug")).and_then(|v| v.as_str()).unwrap_or("").to_string();
                let is_private = item.get("private").and_then(|v| v.as_bool()).unwrap_or(false);
                let default_branch = item.get("default_branch").and_then(|v| v.as_str()).unwrap_or("main").to_string();

                repos.push(Repository {
                    id,
                    slug: full_name,
                    is_private,
                    default_branch,
                });
            }
            Ok(repos)
        } else {
            Ok(Vec::new())
        }
    }

    pub async fn create_repo(&self, name: &str, is_private: bool) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/user/repos", self.api_base);
        let resp = self.http.post(&url).headers(self.headers()).json(&serde_json::json!({
            "name": name,
            "private": is_private,
        })).send().await?;
        Ok(resp.status().is_success())
    }

    pub async fn clone_repo(slug: &str, destination: Option<&Path>) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let repo_url = format!("https://openagents.com/{}.git", slug);
        let mut cmd = Command::new("git");
        cmd.arg("clone").arg(&repo_url);
        if let Some(dest) = destination {
            cmd.arg(dest);
        }
        let status = cmd.status().await?;
        Ok(status.success())
    }
}

pub fn handle_git_credential(operation: &str, host: &str, token: Option<&str>) -> String {
    match operation {
        "get" => {
            if let Some(tok) = token {
                format!("protocol=https\nhost={}\nusername=openagents-token\npassword={}\n", host, tok)
            } else {
                "".to_string()
            }
        }
        _ => "".to_string(),
    }
}
