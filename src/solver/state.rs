use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolverState {
    pub id: String,
    pub run_id: String,
    pub status: SolverStatus,
    pub analysis: String,
    pub files: Vec<FileState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileState {
    pub id: String,
    pub path: String,
    pub analysis: String,
    pub relevance_score: f32,
    pub changes: Vec<Change>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    pub search: String,
    pub replace: String,
    pub analysis: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SolverStatus {
    CollectingContext,
    Thinking,
    GeneratingCode,
    ReadyForCoding,
    Testing,
    CreatingPr,
    Complete,
    Error,
}

impl SolverState {
    pub fn new(analysis: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            run_id: Uuid::new_v4().to_string(),
            status: SolverStatus::CollectingContext,
            analysis,
            files: Vec::new(),
        }
    }

    pub fn add_file(&mut self, path: String, analysis: String, relevance_score: f32) -> &mut FileState {
        let file_state = FileState {
            id: Uuid::new_v4().to_string(),
            path,
            analysis,
            relevance_score,
            changes: Vec::new(),
        };
        self.files.push(file_state);
        self.files.last_mut().unwrap()
    }

    pub fn update_status(&mut self, status: SolverStatus) {
        self.status = status;
    }
}

impl FileState {
    pub fn add_change(&mut self, search: String, replace: String, analysis: String) {
        let change = Change {
            search,
            replace,
            analysis,
        };
        self.changes.push(change);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_solver_state_creation() {
        let state = SolverState::new("Initial analysis".to_string());
        assert_eq!(state.status, SolverStatus::CollectingContext);
        assert!(state.files.is_empty());
        assert_eq!(state.analysis, "Initial analysis");
    }

    #[test]
    fn test_add_file() {
        let mut state = SolverState::new("Initial analysis".to_string());
        let file = state.add_file(
            "src/main.rs".to_string(),
            "Main file".to_string(),
            0.9,
        );
        
        assert_eq!(file.path, "src/main.rs");
        assert_eq!(file.analysis, "Main file");
        assert_eq!(file.relevance_score, 0.9);
        assert!(file.changes.is_empty());
        assert_eq!(state.files.len(), 1);
    }

    #[test]
    fn test_add_change() {
        let mut state = SolverState::new("Initial analysis".to_string());
        let file = state.add_file(
            "src/main.rs".to_string(),
            "Main file".to_string(),
            0.9,
        );
        
        file.add_change(
            "old code".to_string(),
            "new code".to_string(),
            "Improving performance".to_string(),
        );

        assert_eq!(file.changes.len(), 1);
        assert_eq!(file.changes[0].search, "old code");
        assert_eq!(file.changes[0].replace, "new code");
        assert_eq!(file.changes[0].analysis, "Improving performance");
    }

    #[test]
    fn test_update_status() {
        let mut state = SolverState::new("Initial analysis".to_string());
        assert_eq!(state.status, SolverStatus::CollectingContext);
        
        state.update_status(SolverStatus::Thinking);
        assert_eq!(state.status, SolverStatus::Thinking);
        
        state.update_status(SolverStatus::GeneratingCode);
        assert_eq!(state.status, SolverStatus::GeneratingCode);
    }
}