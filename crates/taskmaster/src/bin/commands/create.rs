//! Create command

use clap::Args;
use colored::Colorize;
use taskmaster::{IssueCreate, IssueRepository, IssueType, Priority, Result};

#[derive(Args)]
pub struct CreateArgs {
    /// Issue title
    title: String,

    /// Description
    #[arg(short, long)]
    description: Option<String>,

    /// Priority (0-4, or P0-P4, or critical/high/medium/low/backlog)
    #[arg(short, long, default_value = "medium")]
    priority: String,

    /// Issue type (bug, feature, task, epic, chore)
    #[arg(short = 't', long, default_value = "task")]
    issue_type: String,

    /// Assignee
    #[arg(short, long)]
    assignee: Option<String>,

    /// Labels (can be specified multiple times)
    #[arg(short, long)]
    label: Vec<String>,

    /// Design notes
    #[arg(long)]
    design: Option<String>,

    /// Acceptance criteria
    #[arg(long)]
    acceptance_criteria: Option<String>,

    /// Estimated minutes
    #[arg(long)]
    estimated_minutes: Option<i32>,

    /// External reference (e.g., gh-123)
    #[arg(long)]
    external_ref: Option<String>,

    /// Output as JSON
    #[arg(long)]
    json: bool,
}

pub fn run(repo: &impl IssueRepository, prefix: &str, args: CreateArgs) -> Result<()> {
    let priority: Priority = args
        .priority
        .parse()
        .map_err(|e| taskmaster::TaskmasterError::validation(format!("Invalid priority: {}", e)))?;

    let issue_type: IssueType = args.issue_type.parse().map_err(|e| {
        taskmaster::TaskmasterError::validation(format!("Invalid issue type: {}", e))
    })?;

    let mut create = IssueCreate::new(args.title)
        .priority(priority)
        .issue_type(issue_type);

    if let Some(desc) = args.description {
        create.description = Some(desc);
    }
    if let Some(assignee) = args.assignee {
        create.assignee = Some(assignee);
    }
    if let Some(design) = args.design {
        create.design = Some(design);
    }
    if let Some(ac) = args.acceptance_criteria {
        create.acceptance_criteria = Some(ac);
    }
    if let Some(est) = args.estimated_minutes {
        create.estimated_minutes = Some(est);
    }
    if let Some(ext) = args.external_ref {
        create.external_ref = Some(ext);
    }

    create.labels = args.label;

    let issue = repo.create(create, prefix)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&issue)?);
    } else {
        println!("{} {}", "Created:".green().bold(), issue.id.cyan());
        println!("{} {}", "Title:".bold(), issue.title);
        println!("{} {}", "Status:".bold(), issue.status);
        println!("{} {}", "Priority:".bold(), issue.priority);
        println!("{} {}", "Type:".bold(), issue.issue_type);
        if !issue.labels.is_empty() {
            println!("{} {}", "Labels:".bold(), issue.labels.join(", "));
        }
    }

    Ok(())
}
