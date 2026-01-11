//! Directive-based work fallback.
//!
//! When no issues are actionable, fall back to working on the active directive.

use crate::{Adjutant, Task};
use oanix::WorkspaceManifest;
use std::path::Path;

/// Build a task from a directive file and print directive context.
pub fn build_directive_task(
    workspace: &WorkspaceManifest,
    directive_id: &str,
) -> anyhow::Result<Task> {
    // Find the directive
    let directive = workspace
        .directives
        .iter()
        .find(|d| d.id == directive_id)
        .ok_or_else(|| anyhow::anyhow!("Directive {} not found", directive_id))?;

    println!();
    println!("Working on directive: {} - {}", directive.id, directive.title);

    if let Some(progress) = directive.progress_pct {
        println!("Current progress: {}%", progress);
    }

    // Read the directive file to understand what needs to be done
    let directive_path = workspace
        .root
        .join(".openagents/directives")
        .join(format!("{}.md", directive_id));

    let content = read_directive_file(&directive_path)?;

    println!();

    // Create task from directive
    Ok(Task::new(
        directive_id,
        &directive.title,
        format!(
            "Continue work on directive {}:\n\n{}",
            directive_id, content
        ),
    ))
}

/// Work on the active directive when no issues are available.
pub async fn work_on_directive(
    workspace: &WorkspaceManifest,
    directive_id: &str,
    adjutant: &mut Adjutant,
) -> anyhow::Result<()> {
    let task = build_directive_task(workspace, directive_id)?;

    let result = adjutant.execute(&task).await?;

    // Print result
    println!();
    println!("{}", "=".repeat(55));
    if result.success {
        println!("Directive work completed successfully");
    } else {
        println!("Directive work encountered issues");
    }
    println!();
    println!("Summary: {}", result.summary);

    Ok(())
}

/// Read a directive file, returning a summary if it's too long.
fn read_directive_file(path: &Path) -> anyhow::Result<String> {
    let content = std::fs::read_to_string(path)?;

    // If the file is very long, extract key sections
    if content.len() > 5000 {
        // Try to extract the first section (usually goals/overview)
        let lines: Vec<&str> = content.lines().collect();
        let mut summary = String::new();
        let mut in_section = false;
        let mut section_count = 0;

        for line in lines {
            if line.starts_with("## ") {
                section_count += 1;
                if section_count > 2 {
                    break;
                }
                in_section = true;
            }
            if in_section || section_count == 0 {
                summary.push_str(line);
                summary.push('\n');
            }
        }

        if !summary.is_empty() {
            summary.push_str("\n[... truncated for context, see full directive file ...]");
            return Ok(summary);
        }
    }

    Ok(content)
}

/// Find unblocking work based on blocked issues.
///
/// Returns a description of work that would unblock other issues.
pub fn find_unblocking_work(workspace: &WorkspaceManifest) -> Option<String> {
    let blocked: Vec<_> = workspace
        .issues
        .iter()
        .filter(|i| i.is_blocked)
        .collect();

    if blocked.is_empty() {
        return None;
    }

    // Count patterns in blocked reasons
    let mut needs_code = 0;
    let mut crate_mentions: std::collections::HashMap<String, u32> = std::collections::HashMap::new();

    for issue in &blocked {
        let reason = issue
            .blocked_reason
            .as_deref()
            .unwrap_or("")
            .to_lowercase();

        if reason.contains("empty")
            || reason.contains("no source")
            || reason.contains("doesn't exist")
        {
            needs_code += 1;

            // Extract crate names
            for pattern in &["codex-mcp", "openagents-mcp", "pylon-ui", "wgpui"] {
                if reason.contains(pattern) {
                    *crate_mentions.entry(pattern.to_string()).or_insert(0) += 1;
                }
            }
        }
    }

    // Suggest implementing the most-mentioned crate
    if needs_code > 0 {
        if let Some((crate_name, count)) = crate_mentions.iter().max_by_key(|(_, c)| *c) {
            return Some(format!(
                "Implement {} crate (would unblock {} issues)",
                crate_name, count
            ));
        }
    }

    None
}
