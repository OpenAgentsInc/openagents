//! Convert parsed plans to taskmaster issues

use std::collections::HashMap;

use crate::{
    Dependency, DependencyType, IdMethod, Issue, IssueCreate, IssueRepository,
    TaskmasterError,
};

use super::parser::ParsedPlan;

/// Result of converting a plan to tasks
#[derive(Debug)]
pub struct ConversionResult {
    /// Successfully created issues
    pub created: Vec<Issue>,
    /// Tasks that were skipped (with reason)
    pub skipped: Vec<(String, String)>,
}

/// Convert a parsed plan to taskmaster issues
///
/// # Arguments
/// * `parsed` - The parsed plan with extracted tasks
/// * `plan_name` - Name of the source plan file (used as label)
/// * `repo` - The taskmaster repository
/// * `prefix` - Issue ID prefix (e.g., "tm")
/// * `dry_run` - If true, don't actually create issues
///
/// # Returns
/// A `ConversionResult` with created and skipped issues
pub fn convert_to_tasks<R: IssueRepository>(
    parsed: &ParsedPlan,
    plan_name: &str,
    repo: &R,
    prefix: &str,
    dry_run: bool,
) -> Result<ConversionResult, TaskmasterError> {
    let mut result = ConversionResult {
        created: Vec::new(),
        skipped: Vec::new(),
    };

    // First pass: create all tasks without dependencies
    // and build a map of title -> issue_id
    let mut title_to_id: HashMap<String, String> = HashMap::new();
    let mut created_issues: Vec<(Issue, Vec<String>)> = Vec::new();

    for task in &parsed.tasks {
        // Validate task
        if task.title.is_empty() {
            result
                .skipped
                .push((task.title.clone(), "Empty title".to_string()));
            continue;
        }

        if task.title.len() > 500 {
            result.skipped.push((
                task.title.clone(),
                format!("Title too long: {} chars", task.title.len()),
            ));
            continue;
        }

        // Build the issue
        let mut create = IssueCreate::new(&task.title)
            .priority(task.priority_enum())
            .issue_type(task.issue_type_enum())
            .label(format!("plan:{}", plan_name));

        if let Some(desc) = &task.description {
            create = create.description(desc);
        }

        // Set acceptance_criteria directly on the struct
        if let Some(ac) = &task.acceptance_criteria {
            create.acceptance_criteria = Some(ac.clone());
        }

        if dry_run {
            // For dry run, create a mock issue
            let mock_id = format!("{}-{}", prefix, uuid::Uuid::new_v4().to_string()[..8].to_string());
            title_to_id.insert(task.title.clone(), mock_id.clone());

            let mock_issue = Issue {
                id: mock_id,
                title: task.title.clone(),
                description: task.description.clone().unwrap_or_default(),
                design: None,
                acceptance_criteria: task.acceptance_criteria.clone(),
                notes: None,
                status: crate::IssueStatus::Open,
                priority: task.priority_enum(),
                issue_type: task.issue_type_enum(),
                assignee: None,
                estimated_minutes: None,
                compaction_level: 0,
                close_reason: None,
                closed_at: None,
                external_ref: None,
                source_repo: None,
                discovered_from: None,
                content_hash: None,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                tombstoned_at: None,
                tombstone_ttl_days: None,
                tombstone_reason: None,
                execution_mode: crate::ExecutionMode::None,
                execution_state: crate::ExecutionState::Unscheduled,
                container_id: None,
                agent_id: None,
                execution_branch: None,
                execution_started_at: None,
                execution_finished_at: None,
                execution_exit_code: None,
                commits: vec![],
                labels: vec![format!("plan:{}", plan_name)],
                deps: vec![],
            };

            created_issues.push((mock_issue.clone(), task.dependencies.clone()));
            result.created.push(mock_issue);
        } else {
            // Create the issue using hash-based ID for deduplication
            match repo.create_with_id_method(create, IdMethod::Hash, prefix) {
                Ok(issue) => {
                    title_to_id.insert(task.title.clone(), issue.id.clone());
                    created_issues.push((issue.clone(), task.dependencies.clone()));
                    result.created.push(issue);
                }
                Err(TaskmasterError::AlreadyExists(msg)) => {
                    result.skipped.push((task.title.clone(), msg));
                }
                Err(e) => return Err(e),
            }
        }
    }

    // Second pass: add dependencies
    if !dry_run {
        for (issue, deps) in &created_issues {
            for dep_title in deps {
                if let Some(dep_id) = title_to_id.get(dep_title) {
                    // Add dependency: this issue is blocked by dep_id
                    let dep = Dependency::new(&issue.id, dep_id, DependencyType::Blocks);
                    if let Err(e) = repo.add_dependency(&issue.id, dep) {
                        // Log but don't fail - dependency might already exist
                        tracing::warn!(
                            "Failed to add dependency {} -> {}: {}",
                            issue.id,
                            dep_id,
                            e
                        );
                    }
                }
            }
        }
    }

    Ok(result)
}

/// Print a summary of the conversion result
pub fn print_summary(result: &ConversionResult, dry_run: bool) {
    use colored::Colorize;

    let mode = if dry_run { "[DRY RUN] " } else { "" };

    println!("\n{}Conversion Summary:", mode.yellow());
    println!("  {} tasks created", result.created.len().to_string().green());

    if !result.skipped.is_empty() {
        println!(
            "  {} tasks skipped",
            result.skipped.len().to_string().yellow()
        );
        for (title, reason) in &result.skipped {
            println!("    - {}: {}", title.dimmed(), reason);
        }
    }

    if !result.created.is_empty() {
        println!("\nCreated tasks:");
        for issue in &result.created {
            println!(
                "  {} {} [{}] {}",
                issue.id.cyan(),
                issue.priority.as_str().yellow(),
                issue.issue_type.as_str().blue(),
                issue.title
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SqliteRepository;

    #[test]
    fn test_convert_empty_plan() {
        let repo = SqliteRepository::in_memory().unwrap();
        let plan = ParsedPlan {
            title: "Empty".to_string(),
            tasks: vec![],
        };

        let result = convert_to_tasks(&plan, "test-plan", &repo, "tm", false).unwrap();
        assert!(result.created.is_empty());
        assert!(result.skipped.is_empty());
    }

    #[test]
    fn test_convert_dry_run() {
        let repo = SqliteRepository::in_memory().unwrap();
        let plan = ParsedPlan {
            title: "Test Plan".to_string(),
            tasks: vec![super::super::parser::ParsedTask {
                title: "Test task".to_string(),
                description: Some("Description".to_string()),
                priority: "P1".to_string(),
                issue_type: "feature".to_string(),
                dependencies: vec![],
                acceptance_criteria: None,
            }],
        };

        let result = convert_to_tasks(&plan, "test-plan", &repo, "tm", true).unwrap();
        assert_eq!(result.created.len(), 1);
        assert_eq!(result.created[0].title, "Test task");

        // Verify nothing was actually created
        let all = repo.all().unwrap();
        assert!(all.is_empty());
    }
}
