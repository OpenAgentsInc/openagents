//! Issue management commands

use clap::Args;
use oanix::boot;

/// List issues arguments
#[derive(Args)]
pub struct ListArgs {
    /// Show all issues including blocked ones
    #[arg(short, long)]
    pub all: bool,
}

/// Claim issue arguments
#[derive(Args)]
pub struct ClaimArgs {
    /// Issue number to claim (if not specified, claims next available)
    pub number: Option<u32>,
}

/// Complete issue arguments
#[derive(Args)]
pub struct CompleteArgs {
    /// Issue number to complete (if not specified, completes current)
    pub number: Option<u32>,
}

/// Show issue arguments
#[derive(Args)]
pub struct ShowArgs {
    /// Issue number to show
    pub number: u32,
}

/// List open issues
pub async fn list(args: ListArgs) -> anyhow::Result<()> {
    let manifest = boot().await?;

    let workspace = manifest
        .workspace
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No .openagents/ folder found"))?;

    println!("Issues in {}", workspace.project_name.as_deref().unwrap_or("project"));
    println!();

    let issues = &workspace.issues;

    if issues.is_empty() {
        println!("  No issues found");
        return Ok(());
    }

    // Filter issues
    let filtered: Vec<_> = if args.all {
        issues.iter().collect()
    } else {
        issues
            .iter()
            .filter(|i| i.status == "open" && !i.is_blocked)
            .collect()
    };

    if filtered.is_empty() {
        if args.all {
            println!("  No issues found");
        } else {
            println!("  No actionable issues (use --all to see blocked issues)");
        }
        return Ok(());
    }

    // Group by priority
    let urgent: Vec<_> = filtered.iter().filter(|i| i.priority == "urgent").collect();
    let high: Vec<_> = filtered.iter().filter(|i| i.priority == "high").collect();
    let medium: Vec<_> = filtered.iter().filter(|i| i.priority == "medium").collect();
    let low: Vec<_> = filtered.iter().filter(|i| i.priority == "low").collect();

    if !urgent.is_empty() {
        println!("Urgent:");
        for issue in urgent {
            print_issue_line(issue);
        }
        println!();
    }

    if !high.is_empty() {
        println!("High:");
        for issue in high {
            print_issue_line(issue);
        }
        println!();
    }

    if !medium.is_empty() {
        println!("Medium:");
        for issue in medium {
            print_issue_line(issue);
        }
        println!();
    }

    if !low.is_empty() {
        println!("Low:");
        for issue in low {
            print_issue_line(issue);
        }
    }

    println!();
    println!("Total: {} issues ({} actionable)", issues.len(), filtered.len());

    Ok(())
}

fn print_issue_line(issue: &oanix::IssueSummary) {
    let status_icon = if issue.is_blocked { "[x]" } else { "[ ]" };
    println!("  {} #{}: {}", status_icon, issue.number, issue.title);
}

/// Claim an issue to work on
pub async fn claim(args: ClaimArgs) -> anyhow::Result<()> {
    let manifest = boot().await?;

    let workspace = manifest
        .workspace
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No .openagents/ folder found"))?;

    // Find the issue to claim
    let issue = if let Some(number) = args.number {
        workspace
            .issues
            .iter()
            .find(|i| i.number == number)
            .ok_or_else(|| anyhow::anyhow!("Issue #{} not found", number))?
    } else {
        // Find next available issue (highest priority, not blocked)
        workspace
            .issues
            .iter()
            .filter(|i| i.status == "open" && !i.is_blocked)
            .min_by_key(|i| match i.priority.as_str() {
                "urgent" => 0,
                "high" => 1,
                "medium" => 2,
                "low" => 3,
                _ => 4,
            })
            .ok_or_else(|| anyhow::anyhow!("No actionable issues available"))?
    };

    if issue.is_blocked {
        println!("Warning: Issue #{} is blocked", issue.number);
    }

    println!("Claiming issue #{}: {}", issue.number, issue.title);
    println!("Priority: {}", issue.priority);

    // Load and update the actual issue file
    let issues_file = workspace.root.join(".openagents/issues.json");
    if issues_file.exists() {
        let content = std::fs::read_to_string(&issues_file)?;
        let mut issues: Vec<serde_json::Value> = serde_json::from_str(&content)?;

        // Find and update the issue
        for json_issue in &mut issues {
            if json_issue.get("number").and_then(|n| n.as_u64()) == Some(issue.number as u64) {
                json_issue["claimed_by"] = serde_json::Value::String("adjutant".to_string());
                json_issue["claimed_at"] =
                    serde_json::Value::String(chrono::Utc::now().to_rfc3339());
                break;
            }
        }

        std::fs::write(&issues_file, serde_json::to_string_pretty(&issues)?)?;
        println!("Issue #{} claimed successfully", issue.number);
    } else {
        println!("Note: Could not update issues.json - file not found");
    }

    Ok(())
}

/// Complete an issue
pub async fn complete(args: CompleteArgs) -> anyhow::Result<()> {
    let manifest = boot().await?;

    let workspace = manifest
        .workspace
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No .openagents/ folder found"))?;

    // Find claimed issue or specific number
    let issue = if let Some(number) = args.number {
        workspace
            .issues
            .iter()
            .find(|i| i.number == number)
            .ok_or_else(|| anyhow::anyhow!("Issue #{} not found", number))?
    } else {
        // Find currently claimed issue (would need to check claimed_by field)
        // For now, error if no number specified
        return Err(anyhow::anyhow!(
            "Please specify an issue number to complete"
        ));
    };

    println!("Completing issue #{}: {}", issue.number, issue.title);

    // Load and update the actual issue file
    let issues_file = workspace.root.join(".openagents/issues.json");
    if issues_file.exists() {
        let content = std::fs::read_to_string(&issues_file)?;
        let mut issues: Vec<serde_json::Value> = serde_json::from_str(&content)?;

        // Find and update the issue
        for json_issue in &mut issues {
            if json_issue.get("number").and_then(|n| n.as_u64()) == Some(issue.number as u64) {
                json_issue["status"] = serde_json::Value::String("completed".to_string());
                json_issue["completed_at"] =
                    serde_json::Value::String(chrono::Utc::now().to_rfc3339());
                break;
            }
        }

        std::fs::write(&issues_file, serde_json::to_string_pretty(&issues)?)?;
        println!("Issue #{} marked as completed", issue.number);
    } else {
        println!("Note: Could not update issues.json - file not found");
    }

    Ok(())
}

/// Show issue details
pub async fn show(args: ShowArgs) -> anyhow::Result<()> {
    let manifest = boot().await?;

    let workspace = manifest
        .workspace
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No .openagents/ folder found"))?;

    // Load full issue from issues.json
    let issues_file = workspace.root.join(".openagents/issues.json");
    if !issues_file.exists() {
        return Err(anyhow::anyhow!("issues.json not found"));
    }

    let content = std::fs::read_to_string(&issues_file)?;
    let issues: Vec<serde_json::Value> = serde_json::from_str(&content)?;

    let issue = issues
        .iter()
        .find(|i| i.get("number").and_then(|n| n.as_u64()) == Some(args.number as u64))
        .ok_or_else(|| anyhow::anyhow!("Issue #{} not found", args.number))?;

    println!("Issue #{}", args.number);
    println!("{}", "=".repeat(40));
    println!();

    if let Some(title) = issue.get("title").and_then(|t| t.as_str()) {
        println!("Title: {}", title);
    }

    if let Some(status) = issue.get("status").and_then(|s| s.as_str()) {
        println!("Status: {}", status);
    }

    if let Some(priority) = issue.get("priority").and_then(|p| p.as_str()) {
        println!("Priority: {}", priority);
    }

    if let Some(blocked) = issue.get("is_blocked").and_then(|b| b.as_bool()) {
        if blocked {
            println!("BLOCKED");
            if let Some(reason) = issue.get("blocked_reason").and_then(|r| r.as_str()) {
                println!("Reason: {}", reason);
            }
        }
    }

    println!();

    if let Some(description) = issue.get("description").and_then(|d| d.as_str()) {
        println!("Description:");
        println!("{}", description);
    }

    Ok(())
}
