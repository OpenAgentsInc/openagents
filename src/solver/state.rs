use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SolverStatus {
    Starting,
    Thinking,
    Analyzing,
    Implementing,
    Testing,
    Complete,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolverFile {
    pub path: String,
    pub reason: String,
    pub relevance: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolverState {
    pub id: Uuid,
    pub run_id: String,
    pub status: SolverStatus,
    pub analysis: String,
    pub repo_context: String,
    pub files: HashMap<String, SolverFile>,
}

impl SolverState {
    pub fn new(run_id: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            run_id,
            status: SolverStatus::Starting,
            analysis: String::new(),
            repo_context: String::new(),
            files: HashMap::new(),
        }
    }

    pub fn add_file(&mut self, path: String, reason: String, relevance: f32) {
        self.files.insert(
            path.clone(),
            SolverFile {
                path,
                reason,
                relevance,
            },
        );
    }

    pub fn update_status(&mut self, status: SolverStatus) {
        self.status = status;
    }
}