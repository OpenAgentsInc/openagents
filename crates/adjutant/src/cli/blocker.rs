//! Blocker analysis - categorize why issues are blocked and suggest unblocking work.

use oanix::IssueSummary;
use std::collections::HashMap;

/// Analysis of blocked issues.
#[derive(Debug, Default)]
pub struct BlockerAnalysis {
    /// Issues that need code implemented first (empty crate, no source)
    pub needs_code: u32,
    /// Issues that need infrastructure/setup (mocks, test harness)
    pub needs_infra: u32,
    /// Issues blocked by token budget constraints
    pub token_budget: u32,
    /// Issues that need special environment (GUI, headless)
    pub needs_env: u32,
    /// Issues blocked by architectural concerns
    pub architectural: u32,
    /// Issues blocked by dependencies on other issues
    pub dependencies: u32,
    /// Suggested unblocking work
    pub suggested_work: Option<String>,
    /// Crates that appear in blocked reasons (for "needs code" issues)
    pub blocked_crates: Vec<(String, u32)>,
}

/// Analyze blocked issues and categorize them.
pub fn analyze_blockers(issues: &[&IssueSummary]) -> BlockerAnalysis {
    let mut analysis = BlockerAnalysis::default();
    let mut crate_counts: HashMap<String, u32> = HashMap::new();

    for issue in issues {
        let reason = issue.blocked_reason.as_deref().unwrap_or("").to_lowercase();

        // Categorize by reason
        if reason.contains("empty")
            || reason.contains("no source")
            || reason.contains("doesn't exist")
            || reason.contains("only cargo.toml")
        {
            analysis.needs_code += 1;

            // Try to extract crate name
            if let Some(crate_name) = extract_crate_name(&reason) {
                *crate_counts.entry(crate_name).or_insert(0) += 1;
            }
        } else if reason.contains("infrastructure")
            || reason.contains("mock")
            || reason.contains("setup")
            || reason.contains("test harness")
        {
            analysis.needs_infra += 1;
        } else if reason.contains("token")
            || reason.contains("budget")
            || reason.contains("too large")
            || reason.contains("would consume")
        {
            analysis.token_budget += 1;
        } else if reason.contains("gui")
            || reason.contains("environment")
            || reason.contains("headless")
            || reason.contains("display")
        {
            analysis.needs_env += 1;
        } else if reason.contains("refactor")
            || reason.contains("restructure")
            || reason.contains("architectural")
            || reason.contains("design")
        {
            analysis.architectural += 1;
        } else if reason.contains("depends on")
            || reason.contains("blocked by")
            || reason.contains("waiting for")
        {
            analysis.dependencies += 1;
        }
    }

    // Sort crates by count
    let mut crate_list: Vec<_> = crate_counts.into_iter().collect();
    crate_list.sort_by(|a, b| b.1.cmp(&a.1));
    analysis.blocked_crates = crate_list;

    // Generate suggestion
    analysis.suggested_work = suggest_unblocking_work(&analysis, issues);

    analysis
}

/// Extract crate name from a blocked reason.
fn extract_crate_name(reason: &str) -> Option<String> {
    // Look for patterns like "crate-name crate" or "crate-name has no source"
    let words: Vec<&str> = reason.split_whitespace().collect();

    for (i, word) in words.iter().enumerate() {
        if *word == "crate" && i > 0 {
            let name =
                words[i - 1].trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_');
            if !name.is_empty() && name != "the" && name != "this" {
                return Some(name.to_string());
            }
        }
    }

    // Look for crate names in common patterns
    for pattern in &["codex-mcp", "openagents-mcp", "pylon-ui", "wgpui"] {
        if reason.contains(pattern) {
            return Some(pattern.to_string());
        }
    }

    None
}

/// Suggest what work would unblock the most issues.
fn suggest_unblocking_work(analysis: &BlockerAnalysis, issues: &[&IssueSummary]) -> Option<String> {
    // If many issues need code, suggest implementing the most-mentioned crate
    if analysis.needs_code > 2 {
        if let Some((crate_name, count)) = analysis.blocked_crates.first() {
            return Some(format!(
                "Implement {} crate - would unblock {} issues",
                crate_name, count
            ));
        }
    }

    // If infrastructure is the main blocker
    if analysis.needs_infra > 2 {
        return Some("Set up test infrastructure (mocks, harnesses)".to_string());
    }

    // If token budget is an issue, suggest splitting
    if analysis.token_budget > 0 {
        // Find a specific issue to split
        let budget_issue = issues.iter().find(|i| {
            i.blocked_reason
                .as_ref()
                .map(|r| r.to_lowercase().contains("token") || r.to_lowercase().contains("budget"))
                .unwrap_or(false)
        });

        if let Some(issue) = budget_issue {
            return Some(format!("Split issue #{} into smaller tasks", issue.number));
        }

        return Some("Split large tasks into smaller, focused chunks".to_string());
    }

    // If architectural issues dominate
    if analysis.architectural > 2 {
        return Some("Address architectural concerns before feature work".to_string());
    }

    None
}

/// Truncate a string with ellipsis.
pub fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Print a summary of blocked issues.
pub fn print_blocker_summary(issues: &[&IssueSummary]) {
    println!("Blocked issues ({}):", issues.len());

    for issue in issues.iter().take(10) {
        let reason = issue.blocked_reason.as_deref().unwrap_or("unknown reason");
        println!(
            "  #{}: {} - {}",
            issue.number,
            truncate(&issue.title, 30),
            truncate(reason, 50)
        );
    }

    if issues.len() > 10 {
        println!("  ... and {} more", issues.len() - 10);
    }
}
