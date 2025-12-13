//! Events command (show audit trail)

use clap::Args;
use colored::Colorize;
use taskmaster::{IssueRepository, Result};

#[derive(Args)]
pub struct EventsArgs {
    /// Issue ID (optional - shows all events if not specified)
    id: Option<String>,

    /// Maximum results
    #[arg(long)]
    limit: Option<usize>,

    /// Output as JSON
    #[arg(long)]
    json: bool,
}

pub fn run(repo: &impl IssueRepository, args: EventsArgs) -> Result<()> {
    let events = if let Some(id) = &args.id {
        repo.events(id, args.limit)?
    } else {
        repo.recent_events(args.limit.unwrap_or(50))?
    };

    if args.json {
        println!("{}", serde_json::to_string_pretty(&events)?);
        return Ok(());
    }

    if events.is_empty() {
        println!("{}", "No events found".yellow());
        return Ok(());
    }

    println!("{} events:", "Event History".bold());

    for event in &events {
        println!("\n{}", "─".repeat(60));
        println!("{} {}", "Event:".bold(), event.id);
        println!("{} {}", "Issue:".bold(), event.issue_id.cyan());
        println!(
            "{} {}",
            "Type:".bold(),
            format_event_type(&event.event_type)
        );
        if let Some(actor) = &event.actor {
            println!("{} {}", "Actor:".bold(), actor);
        }
        println!("{} {}", "Timestamp:".bold(), event.created_at);

        if let Some(field) = &event.field_name {
            println!("{} {}", "Field:".bold(), field);
        }
        if let Some(old) = &event.old_value {
            println!("{} {}", "Old:".bold(), old);
        }
        if let Some(new) = &event.new_value {
            println!("{} {}", "New:".bold(), new);
        }
        if let Some(meta) = &event.metadata {
            println!("{} {}", "Metadata:".bold(), meta);
        }
    }

    println!("{}", "─".repeat(60));
    println!("\n{} {} events", "Total:".bold(), events.len());

    Ok(())
}

fn format_event_type(event_type: &taskmaster::EventType) -> String {
    use taskmaster::EventType;
    match event_type {
        EventType::Created => "Created".green().to_string(),
        EventType::Updated => "Updated".blue().to_string(),
        EventType::StatusChanged => "Status Changed".cyan().to_string(),
        EventType::Commented => "Commented".yellow().to_string(),
        EventType::Closed => "Closed".bright_black().to_string(),
        EventType::Reopened => "Reopened".green().to_string(),
        EventType::DependencyAdded => "Dependency Added".magenta().to_string(),
        EventType::DependencyRemoved => "Dependency Removed".magenta().to_string(),
        EventType::LabelAdded => "Label Added".yellow().to_string(),
        EventType::LabelRemoved => "Label Removed".yellow().to_string(),
        EventType::Compacted => "Compacted".bright_black().to_string(),
        EventType::Tombstoned => "Tombstoned".red().to_string(),
        EventType::Restored => "Restored".green().to_string(),
        EventType::Purged => "Purged".red().bold().to_string(),
        EventType::Migrated => "Migrated".blue().to_string(),
    }
}
