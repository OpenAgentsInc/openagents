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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Changes {
    pub changes: Vec<Change>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    pub search: String,
    pub replace: String,
    pub analysis: String,
}

impl From<Change> for openagents::solver::Change {
    fn from(change: Change) -> Self {
        openagents::solver::Change {
            search: change.search,
            replace: change.replace,
            analysis: change.analysis,
            path: String::new(), // Will be set by state.add_change
            reason: Some(change.analysis.clone()),
        }
    }
}