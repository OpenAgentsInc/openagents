use anyhow::Result;
use openagents::server::services::github_issue::GitHubService;
use openagents::solver::state::{SolverState, SolverStatus};
use std::collections::HashSet;
use tracing::{debug, info};

pub fn extract_paths_from_repomap(repo_map: &str) -> HashSet<String> {
    let mut paths = HashSet::new();
    for line in repo_map.lines() {
        if line.contains(".rs:") {
            if let Some(path) = line.split(':').next() {
                paths.insert(path.trim().to_string());
            }
        }
    }
    debug!("Found {} valid paths in repo map", paths.len());
    debug!("Valid paths: {:?}", paths);
    paths
}

pub async fn collect_context(
    state: &mut SolverState,
    github: &GitHubService,
    owner: &str,
    name: &str,
    issue_num: i32,
) -> Result<(String, HashSet<String>)> {
    info!("Collecting context...");
    state.update_status(SolverStatus::CollectingContext);

    // Fetch issue details
    info!("Fetching issue #{}", issue_num);
    let issue = github.get_issue(owner, name, issue_num).await?;
    let comments = github.get_issue_comments(owner, name, issue_num).await?;

    // Generate repository map
    info!("Generating repository map...");
    let repo_dir = std::env::current_dir()?;
    debug!("Current directory: {}", repo_dir.display());
    debug!(
        "Directory contents: {:?}",
        std::fs::read_dir(&repo_dir)?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .collect::<Vec<_>>()
    );
    let repo_map = openagents::repomap::generate_repo_map(&repo_dir);

    // Extract valid paths from repo map
    let valid_paths = extract_paths_from_repomap(&repo_map);

    // Update state with initial analysis
    state.analysis = format!(
        "Issue #{}: {}\n\nDescription: {}\n\nComments:\n{}\n\nRepository Map:\n{}\n\nValid file paths for modifications:\n{}", 
        issue_num,
        issue.title,
        issue.body.unwrap_or_default(),
        comments
            .iter()
            .map(|c| format!("- {}", c.body))
            .collect::<Vec<_>>()
            .join("\n"),
        repo_map,
        valid_paths.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n")
    );

    // Return both the repo directory and valid paths
    Ok((repo_dir.to_string_lossy().to_string(), valid_paths))
}
