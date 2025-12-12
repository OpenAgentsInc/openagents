//! Statistics command

use clap::Args;
use colored::Colorize;
use taskmaster::{IssueRepository, Result};

#[derive(Args)]
pub struct StatsArgs {
    /// Output as JSON
    #[arg(long)]
    json: bool,

    /// Include historical stats
    #[arg(long)]
    history: bool,

    /// Limit historical results
    #[arg(long)]
    limit: Option<usize>,
}

pub fn run(repo: &impl IssueRepository, args: StatsArgs) -> Result<()> {
    if args.history {
        let history = repo.stats_history(args.limit.unwrap_or(30) as u32)?;

        if args.json {
            println!("{}", serde_json::to_string_pretty(&history)?);
            return Ok(());
        }

        println!("{}", "Historical Statistics:".bold());
        for snapshot in history {
            println!("\n{} {}", "Snapshot:".bold(), snapshot.id);
            println!("{} {}", "Date:".bold(), snapshot.snapshot_date);
            println!("\n{}", "Counts:".bold());
            println!("  Total:       {}", snapshot.total_issues);
            println!("  Open:        {}", snapshot.open_count);
            println!("  In Progress: {}", snapshot.in_progress_count);
            println!("  Blocked:     {}", snapshot.blocked_count);
            println!("  Closed:      {}", snapshot.closed_count);
        }
    } else {
        let stats = repo.stats()?;

        if args.json {
            println!("{}", serde_json::to_string_pretty(&stats)?);
            return Ok(());
        }

        println!("{}", "Current Statistics:".bold().cyan());
        print_stats(&stats);
    }

    Ok(())
}

fn print_stats(stats: &taskmaster::IssueStats) {
    println!("\n{}", "By Status:".bold());
    println!("  Open:        {}", stats.by_status.open);
    println!("  In Progress: {}", stats.by_status.in_progress);
    println!("  Blocked:     {}", stats.by_status.blocked);
    println!("  Closed:      {}", stats.by_status.closed);
    println!("  Tombstone:   {}", stats.by_status.tombstone);

    println!("\n{}", "By Priority:".bold());
    println!("  P0 (Critical): {}", stats.by_priority.critical);
    println!("  P1 (High):     {}", stats.by_priority.high);
    println!("  P2 (Medium):   {}", stats.by_priority.medium);
    println!("  P3 (Low):      {}", stats.by_priority.low);
    println!("  P4 (Backlog):  {}", stats.by_priority.backlog);

    println!("\n{}", "By Type:".bold());
    println!("  Bug:     {}", stats.by_type.bug);
    println!("  Feature: {}", stats.by_type.feature);
    println!("  Task:    {}", stats.by_type.task);
    println!("  Epic:    {}", stats.by_type.epic);
    println!("  Chore:   {}", stats.by_type.chore);

    println!("\n{}", "Other:".bold());
    println!("  Total:      {}", stats.total_issues);
    println!("  Ready:      {}", stats.ready_issues);
    println!("  Tombstoned: {}", stats.tombstone_issues);
    if let Some(avg) = stats.avg_time_to_close_hours {
        println!("  Avg close:  {:.1} hours", avg);
    }
}
