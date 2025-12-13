//! Delete (tombstone) command

use clap::Args;
use colored::Colorize;
use taskmaster::{IssueRepository, Result};

#[derive(Args)]
pub struct DeleteArgs {
    /// Issue ID
    id: String,

    /// Reason for deletion
    #[arg(long)]
    reason: Option<String>,

    /// TTL in days before purge (default: 30)
    #[arg(long)]
    ttl_days: Option<i32>,

    /// Actor name
    #[arg(long)]
    actor: Option<String>,

    /// Output as JSON
    #[arg(long)]
    json: bool,
}

pub fn run(repo: &impl IssueRepository, args: DeleteArgs) -> Result<()> {
    repo.tombstone(&args.id, args.reason.as_deref(), args.actor.as_deref())?;

    if args.json {
        let issue = repo.get_with_tombstones(&args.id)?;
        println!("{}", serde_json::to_string_pretty(&issue)?);
    } else {
        println!("{} {}", "Deleted:".red().bold(), args.id.cyan());
        if let Some(reason) = &args.reason {
            println!("{} {}", "Reason:".bold(), reason);
        }
        println!("{} {} days", "TTL:".bold(), args.ttl_days.unwrap_or(30));
    }

    Ok(())
}
