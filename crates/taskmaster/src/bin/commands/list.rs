//! List command

use clap::Args;
use colored::Colorize;
use tabled::{Table, Tabled, settings::Style};
use taskmaster::{
    AssigneeFilter, IssueFilter, IssueRepository, IssueStatus, Priority, Result, SortPolicy,
};

#[derive(Args)]
pub struct ListArgs {
    /// Filter by status
    #[arg(short, long)]
    status: Option<String>,

    /// Filter by priority
    #[arg(short, long)]
    priority: Option<String>,

    /// Filter by type
    #[arg(short = 't', long)]
    issue_type: Option<String>,

    /// Filter by assignee
    #[arg(short, long)]
    assignee: Option<String>,

    /// Show only unassigned issues
    #[arg(long)]
    unassigned: bool,

    /// Filter by labels (comma-separated for AND, repeat for OR)
    #[arg(short, long)]
    label: Vec<String>,

    /// Sort policy (hybrid, priority, oldest, newest, recently_updated)
    #[arg(long, default_value = "hybrid")]
    sort: String,

    /// Maximum results
    #[arg(long)]
    limit: Option<usize>,

    /// Include tombstoned issues
    #[arg(long)]
    include_tombstones: bool,

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
    #[tabled(rename = "Assignee")]
    assignee: String,
    #[tabled(rename = "Labels")]
    labels: String,
}

pub fn run(repo: &impl IssueRepository, args: ListArgs) -> Result<()> {
    let mut filter = IssueFilter::new();

    if let Some(status_str) = args.status {
        let status: IssueStatus = status_str.parse().map_err(|e| {
            taskmaster::TaskmasterError::validation(format!("Invalid status: {}", e))
        })?;
        filter = filter.status(status);
    }

    if let Some(priority_str) = args.priority {
        let priority: Priority = priority_str.parse().map_err(|e| {
            taskmaster::TaskmasterError::validation(format!("Invalid priority: {}", e))
        })?;
        filter = filter.priority(priority);
    }

    if let Some(type_str) = args.issue_type {
        let issue_type: taskmaster::IssueType = type_str
            .parse()
            .map_err(|e| taskmaster::TaskmasterError::validation(format!("Invalid type: {}", e)))?;
        filter = filter.issue_type(issue_type);
    }

    if let Some(assignee) = args.assignee {
        filter = filter.assignee(AssigneeFilter::Is(assignee));
    } else if args.unassigned {
        filter = filter.assignee(AssigneeFilter::Unassigned);
    }

    if !args.label.is_empty() {
        // Parse comma-separated labels
        let labels: Vec<String> = args
            .label
            .iter()
            .flat_map(|s| s.split(',').map(|l| l.trim().to_string()))
            .collect();
        filter = filter.labels_all(labels);
    }

    let sort: SortPolicy = match args.sort.as_str() {
        "hybrid" => SortPolicy::Hybrid,
        "priority" => SortPolicy::Priority,
        "oldest" => SortPolicy::Oldest,
        "newest" => SortPolicy::Newest,
        "recently_updated" => SortPolicy::RecentlyUpdated,
        _ => {
            return Err(taskmaster::TaskmasterError::validation(
                "Invalid sort policy. Use: hybrid, priority, oldest, newest, recently_updated",
            ));
        }
    };
    filter = filter.sort(sort);

    if let Some(limit) = args.limit {
        filter = filter.limit(limit);
    }

    if args.include_tombstones {
        filter.include_tombstones = true;
    }

    let issues = repo.list(filter)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&issues)?);
        return Ok(());
    }

    if issues.is_empty() {
        println!("{}", "No issues found".yellow());
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
            assignee: issue.assignee.clone().unwrap_or_else(|| "-".to_string()),
            labels: if issue.labels.is_empty() {
                "-".to_string()
            } else {
                issue.labels.join(",")
            },
        })
        .collect();

    let mut table = Table::new(rows);
    table.with(Style::rounded());
    println!("{}", table);
    println!("\nTotal: {} issues", issues.len());

    Ok(())
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
