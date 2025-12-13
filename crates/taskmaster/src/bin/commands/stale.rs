//! Stale issues command

use clap::Args;
use colored::Colorize;
use tabled::{Table, Tabled, settings::Style};
use taskmaster::{IssueRepository, Result};

#[derive(Args)]
pub struct StaleArgs {
    /// Days without update to be considered stale
    #[arg(short, long, default_value = "30")]
    days: i32,

    /// Output as JSON
    #[arg(long)]
    json: bool,
}

#[derive(Tabled)]
struct IssueRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Pri")]
    priority: String,
    #[tabled(rename = "Title")]
    title: String,
    #[tabled(rename = "Updated")]
    updated_at: String,
}

pub fn run(repo: &impl IssueRepository, args: StaleArgs) -> Result<()> {
    let filter = taskmaster::StaleFilter {
        days: args.days as u32,
        status: None,
        limit: None,
    };
    let issues = repo.stale(filter)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&issues)?);
        return Ok(());
    }

    if issues.is_empty() {
        println!(
            "{}",
            format!("No issues stale for {} days", args.days).yellow()
        );
        return Ok(());
    }

    let rows: Vec<IssueRow> = issues
        .iter()
        .map(|issue| IssueRow {
            id: issue.id.clone(),
            status: format!("{}", issue.status),
            priority: format!("{}", issue.priority),
            title: truncate(&issue.title, 40),
            updated_at: issue.updated_at.to_string(),
        })
        .collect();

    let mut table = Table::new(rows);
    table.with(Style::rounded());
    println!("{}", table);
    println!(
        "\n{} {} stale issues (not updated in {} days)",
        "Total:".bold(),
        issues.len(),
        args.days
    );

    Ok(())
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
