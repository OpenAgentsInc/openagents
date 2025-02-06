use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ChangeResponse {
    pub changes: Vec<ChangeBlock>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChangeBlock {
    pub path: String,
    pub search: String,
    pub replace: String,
    pub reason: String,
}