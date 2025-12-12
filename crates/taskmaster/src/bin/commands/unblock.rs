//! Unblock command (move from Blocked to Open)

use clap::Args;
use colored::Colorize;
use taskmaster::{IssueRepository, Result};

#[derive(Args)]
pub struct UnblockArgs {
    /// Issue ID
    id: String,

    /// Actor name
    #[arg(long)]
    actor: Option<String>,

    /// Output as JSON
    #[arg(long)]
    json: bool,
}

pub fn run(repo: &impl IssueRepository, args: UnblockArgs) -> Result<()> {
    let issue = repo.unblock(&args.id, args.actor.as_deref())?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&issue)?);
    } else {
        println!("{} {}", "Unblocked:".green().bold(), issue.id.cyan());
        println!("{} {}", "Status:".bold(), issue.status);
    }

    Ok(())
}
