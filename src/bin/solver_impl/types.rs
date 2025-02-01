use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct RelevantFiles {
    pub files: Vec<FileInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileInfo {
    pub path: String,
    pub relevance_score: f32,
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Changes {
    pub changes: Vec<Change>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Change {
    pub search: String,
    pub replace: String,
    pub analysis: String,
}
