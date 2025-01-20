use anyhow::Result;
use tokio::sync::broadcast;
use crate::server::services::solver::ws::types::SolverUpdate;

pub(crate) fn parse_repo_url(
    issue_url: &str,
    update_tx: &broadcast::Sender<SolverUpdate>,
) -> Result<String> {
    if issue_url.contains("/issues/") {
        Ok(issue_url
            .split("/issues/")
            .next()
            .unwrap_or(issue_url)
            .to_string())
    } else if issue_url.contains("github.com") {
        // If it's already a repo URL, use it directly
        Ok(issue_url.trim_end_matches('/').to_string())
    } else {
        let err = anyhow::anyhow!("Invalid GitHub URL format");
        let _ = update_tx.send(SolverUpdate::Error {
            message: "Invalid GitHub URL format".into(),
            details: None,
        });
        Err(err)
    }
}
