use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    pub search: String,
    pub replace: String,
    pub analysis: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileState {
    pub path: String,
    pub analysis: String,
    pub changes: Vec<Change>,
}

impl Change {
    pub fn new(search: String, replace: String, analysis: String) -> Self {
        Self {
            search,
            replace,
            analysis,
        }
    }
}

impl FileState {
    pub fn new(path: String, analysis: String) -> Self {
        Self {
            path,
            analysis,
            changes: Vec::new(),
        }
    }

    pub fn add_change(&mut self, search: String, replace: String, analysis: String) {
        self.changes.push(Change::new(search, replace, analysis));
    }
}