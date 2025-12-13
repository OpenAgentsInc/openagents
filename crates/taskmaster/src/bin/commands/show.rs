//! Show command

use clap::Args;
use colored::Colorize;
use taskmaster::{IssueRepository, Result};

#[derive(Args)]
pub struct ShowArgs {
    /// Issue ID
    id: String,

    /// Output as JSON
    #[arg(long)]
    json: bool,

    /// Include tombstoned issues
    #[arg(long)]
    include_tombstones: bool,
}

pub fn run(repo: &impl IssueRepository, args: ShowArgs) -> Result<()> {
    let issue = if args.include_tombstones {
        repo.get_with_tombstones(&args.id)?
    } else {
        repo.get(&args.id)?
    };

    if args.json {
        println!("{}", serde_json::to_string_pretty(&issue)?);
        return Ok(());
    }

    // Pretty print
    println!("{}", "═".repeat(60));
    println!("{} {}", "ID:".bold(), issue.id.cyan());
    println!("{} {}", "Title:".bold(), issue.title);
    println!("{}", "─".repeat(60));

    println!("{} {}", "Status:".bold(), format_status(&issue.status));
    println!(
        "{} {}",
        "Priority:".bold(),
        format_priority(&issue.priority)
    );
    println!("{} {}", "Type:".bold(), issue.issue_type);

    if let Some(assignee) = &issue.assignee {
        println!("{} {}", "Assignee:".bold(), assignee);
    }

    if !issue.description.is_empty() {
        println!("\n{}", "Description:".bold());
        println!("{}", issue.description);
    }

    if let Some(design) = &issue.design {
        println!("\n{}", "Design:".bold());
        println!("{}", design);
    }

    if let Some(ac) = &issue.acceptance_criteria {
        println!("\n{}", "Acceptance Criteria:".bold());
        println!("{}", ac);
    }

    if let Some(notes) = &issue.notes {
        println!("\n{}", "Notes:".bold());
        println!("{}", notes);
    }

    if !issue.labels.is_empty() {
        println!(
            "\n{} {}",
            "Labels:".bold(),
            issue.labels.join(", ").yellow()
        );
    }

    if !issue.deps.is_empty() {
        println!("\n{}", "Dependencies:".bold());
        for dep in &issue.deps {
            println!("  {} {} ({})", "→".cyan(), dep.id, dep.dep_type);
        }
    }

    if let Some(est) = issue.estimated_minutes {
        println!("\n{} {} minutes", "Estimated:".bold(), est);
    }

    if let Some(ext) = &issue.external_ref {
        println!("{} {}", "External Ref:".bold(), ext);
    }

    println!("\n{}", "Timestamps:".bold());
    println!("  Created:  {}", issue.created_at);
    println!("  Updated:  {}", issue.updated_at);
    if let Some(closed) = issue.closed_at {
        println!("  Closed:   {}", closed);
        if let Some(reason) = &issue.close_reason {
            println!("  Reason:   {}", reason);
        }
    }

    if issue.is_tombstone() {
        if let Some(tombstoned) = issue.tombstoned_at {
            println!("\n{}", "Tombstone:".red().bold());
            println!("  Deleted:  {}", tombstoned);
            if let Some(reason) = &issue.tombstone_reason {
                println!("  Reason:   {}", reason);
            }
            println!(
                "  TTL:      {} days",
                issue.tombstone_ttl_days.unwrap_or(30)
            );
        }
    }

    println!("{}", "═".repeat(60));

    Ok(())
}

fn format_status(status: &taskmaster::IssueStatus) -> String {
    use taskmaster::IssueStatus;
    match status {
        IssueStatus::Open => "open".green().to_string(),
        IssueStatus::InProgress => "in_progress".blue().to_string(),
        IssueStatus::Blocked => "blocked".yellow().to_string(),
        IssueStatus::Closed => "closed".bright_black().to_string(),
        IssueStatus::Tombstone => "tombstone".red().to_string(),
    }
}

fn format_priority(priority: &taskmaster::Priority) -> String {
    use taskmaster::Priority;
    match priority {
        Priority::Critical => "P0 (Critical)".red().bold().to_string(),
        Priority::High => "P1 (High)".red().to_string(),
        Priority::Medium => "P2 (Medium)".yellow().to_string(),
        Priority::Low => "P3 (Low)".green().to_string(),
        Priority::Backlog => "P4 (Backlog)".bright_black().to_string(),
    }
}
