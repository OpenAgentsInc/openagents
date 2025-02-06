use serde::{Deserialize, Serialize};

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
        }
    }
}