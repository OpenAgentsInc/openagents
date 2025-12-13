//! Ready queue command

use clap::Args;
use colored::Colorize;
use tabled::{Table, Tabled, settings::Style};
use taskmaster::{IssueRepository, Result};

#[derive(Args)]
pub struct ReadyArgs {
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
    #[tabled(rename = "Pri")]
    priority: String,
    #[tabled(rename = "Type")]
    issue_type: String,
    #[tabled(rename = "Title")]
    title: String,
    #[tabled(rename = "Assignee")]
    assignee: String,
}

pub fn run(repo: &impl IssueRepository, args: ReadyArgs) -> Result<()> {
    let mut filter = taskmaster::IssueFilter::new();
    if let Some(limit) = args.limit {
        filter = filter.limit(limit);
    }
    let issues = repo.ready(filter)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&issues)?);
        return Ok(());
    }

    if issues.is_empty() {
        println!("{}", "No ready issues found".yellow());
        return Ok(());
    }

    let rows: Vec<IssueRow> = issues
        .iter()
        .map(|issue| IssueRow {
            id: issue.id.clone(),
            priority: format!("{}", issue.priority),
            issue_type: format!("{}", issue.issue_type),
            title: truncate(&issue.title, 50),
            assignee: issue.assignee.clone().unwrap_or_else(|| "-".to_string()),
        })
        .collect();

    let mut table = Table::new(rows);
    table.with(Style::rounded());
    println!("{}", table);
    println!("\n{} {} ready issues", "Total:".bold(), issues.len());

    Ok(())
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
