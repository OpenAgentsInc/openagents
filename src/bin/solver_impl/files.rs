use anyhow::Result;
use octocrab::models::issues::{Issue, Comment as IssueComment};
use crate::solver_impl::types::FileInfo;

pub async fn identify_files(
    issue: &Issue,
    comments: &[IssueComment],
) -> Result<Vec<FileInfo>> {
    // For now, just return a test file
    Ok(vec![FileInfo {
        path: "src/lib.rs".to_string(),
        relevance_score: 1.0,
        reason: "Test file".to_string(),
    }])
}