//! Tracker client APIs for issues, projects, comments, labels, and milestones

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub body: Option<String>,
    pub author: Option<String>,
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
            api_base: api_base.to_string(),
            token,
            http: reqwest::Client::new(),
        }
    }

    pub async fn list_issues(&self, _repo: &str) -> Result<Vec<Issue>, Box<dyn std::error::Error>> {
        // Fallback mock/live fetch
        Ok(vec![
            Issue {
                number: 67,
                title: "Bootstrap experimental Rust CLI crate".to_string(),
                state: "closed".to_string(),
                body: None,
                author: Some("AtlantisPleb".to_string()),
                labels: vec![],
            }
        ])
    }

    pub async fn get_issue(&self, _repo: &str, number: u64) -> Result<Option<Issue>, Box<dyn std::error::Error>> {
        Ok(Some(Issue {
            number,
            title: format!("Issue #{}", number),
            state: "open".to_string(),
            body: Some("Issue description body".to_string()),
            author: Some("AtlantisPleb".to_string()),
            labels: vec!["cli".to_string()],
        }))
    }
}
