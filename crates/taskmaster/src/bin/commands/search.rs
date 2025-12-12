//! Search command (full-text search)

use clap::Args;
use colored::Colorize;
use tabled::{settings::Style, Table, Tabled};
use taskmaster::{IssueRepository, Result};

#[derive(Args)]
pub struct SearchArgs {
    /// Search query
    query: String,

    /// Maximum results
    #[arg(long)]
    limit: Option<usize>,

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
    #[tabled(rename = "Type")]
    issue_type: String,
    #[tabled(rename = "Title")]
    title: String,
}

pub fn run(repo: &impl IssueRepository, args: SearchArgs) -> Result<()> {
    let mut filter = taskmaster::IssueFilter::new();
    if let Some(limit) = args.limit {
        filter = filter.limit(limit);
    }
    let issues = repo.search(&args.query, filter)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&issues)?);
        return Ok(());
    }

    if issues.is_empty() {
        println!("{} '{}'", "No issues found for".yellow(), args.query);
        return Ok(());
    }

    let rows: Vec<IssueRow> = issues
        .iter()
        .map(|issue| IssueRow {
            id: issue.id.clone(),
            status: format!("{}", issue.status),
            priority: format!("{}", issue.priority),
            issue_type: format!("{}", issue.issue_type),
            title: truncate(&issue.title, 50),
        })
        .collect();

    let mut table = Table::new(rows);
    table.with(Style::rounded());
    println!("{}", table);
    println!("\n{} {} issues matching '{}'", "Total:".bold(), issues.len(), args.query);

    Ok(())
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
