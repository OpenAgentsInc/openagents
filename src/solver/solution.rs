use crate::solver::context::SolverContext;
use crate::solver::state::{SolverState, SolverStatus};
use anyhow::Result;
use std::path::PathBuf;
use tracing::info;

pub async fn handle_solution(
    title: &str,
    description: &str,
    issue_number: i32,
) -> Result<()> {
    info!("Handling solution for issue #{}", issue_number);

    let state = SolverState::new(format!("solve-{}", issue_number));
    let repo_dir = PathBuf::from("temp").into_boxed_path();
    let mut context = SolverContext::new(state, repo_dir);

    // Analyze files
    context.state.update_status(SolverStatus::Analyzing);
    context.analyze_files(title, description).await?;

    // Generate changes
    context.state.update_status(SolverStatus::Implementing);
    let changes = context.generate_changes(title, description).await?;
    info!("Generated {} changes", changes.len());

    // Apply changes
    context.state.update_status(SolverStatus::Testing);
    context.apply_changes().await?;

    // Update status
    context.state.update_status(SolverStatus::Complete);
    info!("Solution handling complete");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_handle_solution() {
        let result = handle_solution(
            "Test issue",
            "Test description",
            1,
        ).await;
        assert!(result.is_ok());
    }
}