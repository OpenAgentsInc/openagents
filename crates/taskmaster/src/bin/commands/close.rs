//! Close command

use clap::Args;
use colored::Colorize;
use taskmaster::{IssueRepository, Result};

#[derive(Args)]
pub struct CloseArgs {
    /// Issue ID
    id: String,

    /// Reason for closing
    #[arg(long)]
    reason: Option<String>,

    /// Actor name
    #[arg(long)]
    actor: Option<String>,

    /// Output as JSON
    #[arg(long)]
    json: bool,
}

pub fn run(repo: &impl IssueRepository, args: CloseArgs) -> Result<()> {
    let issue = repo.close(&args.id, args.reason.as_deref(), vec![], args.actor.as_deref())?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&issue)?);
    } else {
        println!("{} {}", "Closed:".green().bold(), issue.id.cyan());
        if let Some(reason) = &issue.close_reason {
            println!("{} {}", "Reason:".bold(), reason);
        }
        if let Some(closed_at) = issue.closed_at {
            println!("{} {}", "Closed at:".bold(), closed_at);
        }
    }

    Ok(())
}
