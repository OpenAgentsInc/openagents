//! Forum board browsing, topics, claims and NIP-29 chat integration

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
    pub author_npub: String,
}

pub struct ForumClient {
    pub api_base: String,
}

impl ForumClient {
    pub fn new(api_base: &str) -> Self {
        Self {
            api_base: api_base.to_string(),
        }
    }

    pub fn list_boards(&self) -> Vec<ForumBoard> {
        vec![
            ForumBoard {
                id: "general".to_string(),
                name: "General".to_string(),
                description: "OpenAgents community discussion".to_string(),
            },
            ForumBoard {
                id: "dev".to_string(),
                name: "Development".to_string(),
                description: "Technical discussions and forge updates".to_string(),
            },
        ]
    }
}
