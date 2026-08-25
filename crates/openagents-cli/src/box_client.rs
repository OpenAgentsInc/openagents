//! Box sandbox management, remote execution and parallel fanout

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoxSandbox {
    pub id: String,
    pub name: String,
    pub status: String,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoxRun {
    pub run_id: String,
    pub box_id: String,
    pub command: String,
    pub status: String,
    pub exit_code: Option<i32>,
}

pub struct BoxClient {
    pub api_base: String,
    pub token: Option<String>,
}

impl BoxClient {
    pub fn new(api_base: &str, token: Option<String>) -> Self {
        Self {
            api_base: api_base.to_string(),
            token,
        }
    }

    pub fn list_boxes(&self) -> Vec<BoxSandbox> {
        vec![
            BoxSandbox {
                id: "bx_main".to_string(),
                name: "primary-sandbox".to_string(),
                status: "running".to_string(),
                created_at: 1724600000,
            }
        ]
    }
}
