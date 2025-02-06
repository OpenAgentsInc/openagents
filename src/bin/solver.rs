use anyhow::{Context as _, Result};
use openagents::server::services::github_issue::GitHubService;
use openagents::solver::state::{SolverState, SolverStatus};
use tracing::debug;

mod solver_impl;
use solver_impl::{files::identify_files, issue::handle_issue};

#[derive(Debug)]
pub struct Args {
    pub issue: i32,
    pub repo: String,
    pub live: bool,
}

impl Args {
    pub fn parse() -> Result<Self> {
        let mut args = std::env::args().skip(1);
        let mut issue = None;
        let mut repo = None;
        let mut live = false;

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--issue" => {
                    if let Some(val) = args.next() {
                        issue = Some(val.parse()?);
                    }
                }
                "--repo" => {
                    if let Some(val) = args.next() {
                        repo = Some(val);
                    }
                }
                "--live" => live = true,
                _ => {}
            }
        }

        Ok(Args {
            issue: issue.context("--issue argument is required")?,
            repo: repo.unwrap_or_else(|| "OpenAgentsInc/openagents".to_string()),
            live,
        })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_file(true)
        .with_line_number(true)
        .init();

    // Parse command line arguments
    let args = Args::parse().context("Failed to parse arguments")?;
    debug!("Args: {:?}", args);

    // Initialize GitHub service
    let github_token = std::env::var("GITHUB_TOKEN").context("GITHUB_TOKEN not set")?;
    let github = GitHubService::new(Some(github_token))?;

    // Parse owner/repo
    let (owner, repo) = args
        .repo
        .split_once('/')
        .context("Invalid repo format - expected owner/name")?;

    // Initialize state
    let mut state = SolverState::new(format!("{}-{}", args.issue, chrono::Utc::now().timestamp()));
    state.update_status(SolverStatus::Starting);

    // Handle issue
    let (issue, comments) = handle_issue(&github, owner, repo, args.issue).await?;
    debug!("Issue: {:?}", issue);
    debug!("Comments: {:?}", comments);

    // Identify files to modify
    let files = identify_files(&issue, &comments).await?;
    debug!("Files to modify: {:?}", files);

    // Add files to state
    for file in files {
        state.add_file(file.path, file.reason, file.relevance_score);
    }

    Ok(())
}