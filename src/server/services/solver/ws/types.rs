#[derive(Debug, Clone, PartialEq)]
pub enum SolverStage {
    Init,
    Repomap,
    Analysis,
    Solution,
    PR,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SolverUpdate {
    Progress {
        stage: SolverStage,
        message: String,
        data: Option<serde_json::Value>,
    },
    Error {
        message: String,
        details: Option<String>,
    },
    Complete {
        result: serde_json::Value,
    },
}
