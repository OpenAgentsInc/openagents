//! Cleanup command (remove expired tombstones)

use clap::Args;
use colored::Colorize;
use taskmaster::{IssueRepository, Result};

#[derive(Args)]
pub struct CleanupArgs {
    /// Dry run (show what would be cleaned)
    #[arg(long)]
    dry_run: bool,

    /// Output as JSON
    #[arg(long)]
    json: bool,
}

pub fn run(repo: &impl IssueRepository, args: CleanupArgs) -> Result<()> {
    if args.dry_run {
        println!(
            "{}",
            "Note: Dry run not supported yet. Use --json to see counts.".yellow()
        );
    }

    let result = repo.cleanup_tombstones()?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    println!("{}", "Cleanup Complete:".bold().green());
    println!(
        "{} {} tombstones purged",
        "Purged:".bold(),
        result.purged_count
    );
    println!(
        "{} {} tombstones retained",
        "Retained:".bold(),
        result.retained_count
    );

    if !result.errors.is_empty() {
        println!("\n{} errors:", "Encountered".red().bold());
        for error in &result.errors {
            println!("  {}", error.red());
        }
    }

    Ok(())
}
