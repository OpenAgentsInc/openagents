use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolverState {
    pub id: String,
    pub status: SolverStatus,
    pub issue_number: i32,
    pub issue_title: String,
    pub issue_body: String,
    pub files: Vec<FileState>,
    pub repo_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileState {
    pub path: String,
    pub relevance_score: f32,
    pub reason: String,
    pub changes: Vec<Change>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    pub id: String,
    pub search: String,
    pub replace: String,
    pub analysis: String,
    #[allow(dead_code)]
    pub status: ChangeStatus, // Keeping for future use
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SolverStatus {
    Analyzing,
    GeneratingChanges,
    ApplyingChanges,
    Complete,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[allow(dead_code)] // Keeping for future use
pub enum ChangeStatus {
    Pending,
    Approved,
    Rejected,
}

impl SolverState {
    pub fn new(issue_number: i32, issue_title: String, issue_body: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            status: SolverStatus::Analyzing,
            issue_number,
            issue_title,
            issue_body,
            files: Vec::new(),
            repo_path: None,
        }
    }

    pub fn add_file(
        &mut self,
        path: String,
        relevance_score: f32,
        reason: String,
    ) -> &mut FileState {
        let file_state = FileState {
            path,
            relevance_score,
            reason,
            changes: Vec::new(),
        };
        self.files.push(file_state);
        self.files.last_mut().unwrap()
    }

    pub fn set_repo_path(&mut self, path: String) {
        self.repo_path = Some(path);
    }
}

impl FileState {
    pub fn add_change(&mut self, search: String, replace: String, analysis: String) {
        let change = Change {
            id: Uuid::new_v4().to_string(),
            search,
            replace,
            analysis,
            status: ChangeStatus::Pending,
        };
        self.changes.push(change);
    }
}
