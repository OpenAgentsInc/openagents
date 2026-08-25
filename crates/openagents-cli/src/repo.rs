//! Forge repository management, clone, import and git credential helper

use serde::{Deserialize, Serialize};

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
}

impl RepoClient {
    pub fn new(api_base: &str, token: Option<String>) -> Self {
        Self {
            api_base: api_base.to_string(),
            token,
        }
    }

    pub fn list_repos(&self) -> Vec<Repository> {
        vec![
            Repository {
                id: "repo_1".to_string(),
                slug: "OpenAgentsInc/openagents".to_string(),
                is_private: false,
                default_branch: "main".to_string(),
            },
            Repository {
                id: "repo_2".to_string(),
                slug: "OpenAgentsInc/openagents.com".to_string(),
                is_private: false,
                default_branch: "main".to_string(),
            },
        ]
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
