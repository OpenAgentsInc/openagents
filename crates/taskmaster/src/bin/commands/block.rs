//! Block command (move to Blocked status)

use clap::Args;
use colored::Colorize;
use taskmaster::{IssueRepository, Result};

#[derive(Args)]
pub struct BlockArgs {
    /// Issue ID
    id: String,

    /// Actor name
    #[arg(long)]
    actor: Option<String>,

    /// Output as JSON
    #[arg(long)]
    json: bool,
}

pub fn run(repo: &impl IssueRepository, args: BlockArgs) -> Result<()> {
    let issue = repo.block(&args.id, None, args.actor.as_deref())?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&issue)?);
    } else {
        println!("{} {}", "Blocked:".yellow().bold(), issue.id.cyan());
        println!("{} {}", "Status:".bold(), issue.status);
    }

    Ok(())
}
