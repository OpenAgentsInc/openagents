//! Start command (move to InProgress)

use clap::Args;
use colored::Colorize;
use taskmaster::{IssueRepository, Result};

#[derive(Args)]
pub struct StartArgs {
    /// Issue ID
    id: String,

    /// Actor name
    #[arg(long)]
    actor: Option<String>,

    /// Output as JSON
    #[arg(long)]
    json: bool,
}

pub fn run(repo: &impl IssueRepository, args: StartArgs) -> Result<()> {
    let issue = repo.start(&args.id, args.actor.as_deref())?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&issue)?);
    } else {
        println!("{} {}", "Started:".green().bold(), issue.id.cyan());
        println!("{} {}", "Status:".bold(), issue.status);
    }

    Ok(())
}
