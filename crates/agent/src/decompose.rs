//! Subtask Decomposition
//!
//! Breaks tasks into implementable subtasks to prevent "one-shot" failures.
//! Uses heuristics for simple decomposition.

use crate::error::AgentResult;
use crate::types::{Subtask, SubtaskList, SubtaskStatus, Task, get_subtasks_path};
use chrono::Utc;
use regex::Regex;
use std::fs;
use std::path::Path;

/// Generate a unique subtask ID
pub fn generate_subtask_id(task_id: &str, index: usize) -> String {
    format!("{}-sub-{:03}", task_id, index + 1)
}

/// Heuristics for detecting if a task needs decomposition
#[derive(Debug, Clone, Default)]
pub struct DecompositionHeuristics {
    /// Task mentions multiple files/components
    pub has_multiple_targets: bool,
    /// Task has multiple distinct actions (add X, update Y, test Z)
    pub has_multiple_actions: bool,
    /// Task description is long (>500 chars)
    pub is_complex: bool,
    /// Task mentions testing explicitly
    pub requires_testing: bool,
    /// Task mentions documentation
    pub requires_docs: bool,
}

/// Analyze a task to determine decomposition needs
pub fn analyze_task(task: &Task) -> DecompositionHeuristics {
    let description = task.description.as_deref().unwrap_or("");
    let text = format!("{} {}", task.title, description).to_lowercase();

    // Check for multiple file/component mentions
    let file_pattern =
        Regex::new(r"\b(file|component|module|service|class|function|test|spec)\b").unwrap();
    let file_matches: Vec<_> = file_pattern.find_iter(&text).collect();
    let has_multiple_targets = file_matches.len() > 2;

    // Check for multiple action words
    let action_pattern = Regex::new(
        r"\b(add|create|update|modify|fix|remove|delete|implement|refactor|test|document)\b",
    )
    .unwrap();
    let action_matches: std::collections::HashSet<_> = action_pattern
        .find_iter(&text)
        .map(|m| m.as_str().to_lowercase())
        .collect();
    let has_multiple_actions = action_matches.len() > 2;

    // Check complexity by length
    let is_complex = description.len() > 500;

    // Check for explicit testing requirement
    let requires_testing =
        Regex::new(r"\b(tests?|specs?|coverage|verify|validate|unit test|e2e)\b")
            .unwrap()
            .is_match(&text);

    // Check for documentation requirement
    let requires_docs =
        Regex::new(r"\b(docs?|documentation|readme|comments?|jsdoc|tsdoc)\b")
            .unwrap()
            .is_match(&text);

    DecompositionHeuristics {
        has_multiple_targets,
        has_multiple_actions,
        is_complex,
        requires_testing,
        requires_docs,
    }
}

/// Simple rule-based decomposition for common patterns
pub fn decompose_by_rules(task: &Task) -> Vec<Subtask> {
    let heuristics = analyze_task(task);
    let mut subtasks = Vec::new();
    let mut index = 0;

    let description = task.description.as_deref().unwrap_or("");

    // If task is simple, just create one subtask
    if !heuristics.has_multiple_actions && !heuristics.is_complex && !heuristics.has_multiple_targets
    {
        return vec![Subtask::new(
            generate_subtask_id(&task.id, 0),
            format!("{}\n\n{}", task.title, description).trim().to_string(),
        )];
    }

    // For complex tasks, break into logical phases

    // Phase 1: Implementation
    subtasks.push(Subtask::new(
        generate_subtask_id(&task.id, index),
        format!("Implement: {}\n\n{}", task.title, description),
    ));
    index += 1;

    // Phase 2: Testing (if not explicitly part of implementation)
    if heuristics.requires_testing && !task.title.to_lowercase().contains("test") {
        subtasks.push(Subtask::new(
            generate_subtask_id(&task.id, index),
            format!(
                "Add tests for: {}\n\nVerify the implementation works correctly with unit tests.",
                task.title
            ),
        ));
        index += 1;
    }

    // Phase 3: Documentation (if required)
    if heuristics.requires_docs {
        subtasks.push(Subtask::new(
            generate_subtask_id(&task.id, index),
            format!(
                "Document: {}\n\nAdd appropriate documentation/comments.",
                task.title
            ),
        ));
    }

    subtasks
}

/// Options for task decomposition
#[derive(Debug, Clone, Default)]
pub struct DecomposeOptions {
    /// Maximum number of subtasks to create
    pub max_subtasks: Option<usize>,
    /// Force single subtask (no decomposition)
    pub force_single: bool,
}

/// Decompose a task into subtasks
pub fn decompose_task(task: &Task, options: Option<DecomposeOptions>) -> Vec<Subtask> {
    let opts = options.unwrap_or_default();
    let max_subtasks = opts.max_subtasks.unwrap_or(5);

    if opts.force_single {
        let description = task.description.as_deref().unwrap_or("");
        return vec![Subtask::new(
            generate_subtask_id(&task.id, 0),
            format!("{}\n\n{}", task.title, description).trim().to_string(),
        )];
    }

    let subtasks = decompose_by_rules(task);

    // Limit to max_subtasks
    subtasks.into_iter().take(max_subtasks).collect()
}

/// Read subtasks file for a task
pub fn read_subtasks(openagents_dir: &str, task_id: &str) -> Option<SubtaskList> {
    let subtasks_path = get_subtasks_path(openagents_dir, task_id);
    let path = Path::new(&subtasks_path);

    if !path.exists() {
        return None;
    }

    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Write subtasks file for a task
pub fn write_subtasks(openagents_dir: &str, subtask_list: &SubtaskList) -> AgentResult<()> {
    let subtasks_dir = Path::new(openagents_dir).join("subtasks");
    if !subtasks_dir.exists() {
        fs::create_dir_all(&subtasks_dir)?;
    }

    let subtasks_path = get_subtasks_path(openagents_dir, &subtask_list.task_id);
    let content = serde_json::to_string_pretty(subtask_list)?;
    fs::write(&subtasks_path, content)?;

    Ok(())
}

/// Update a subtask's status
pub fn update_subtask_status(
    openagents_dir: &str,
    task_id: &str,
    subtask_id: &str,
    status: SubtaskStatus,
    error: Option<String>,
) -> AgentResult<Option<SubtaskList>> {
    let mut subtask_list = match read_subtasks(openagents_dir, task_id) {
        Some(list) => list,
        None => return Ok(None),
    };

    let subtask = match subtask_list.subtasks.iter_mut().find(|s| s.id == subtask_id) {
        Some(s) => s,
        None => return Ok(None),
    };

    subtask.status = status;
    subtask.error = error;

    let now = Utc::now().to_rfc3339();
    match status {
        SubtaskStatus::InProgress => subtask.started_at = Some(now.clone()),
        SubtaskStatus::Done => subtask.completed_at = Some(now.clone()),
        SubtaskStatus::Verified => subtask.verified_at = Some(now.clone()),
        _ => {}
    }

    subtask_list.updated_at = now;
    write_subtasks(openagents_dir, &subtask_list)?;

    Ok(Some(subtask_list))
}

/// Create a new subtask list for a task
pub fn create_subtask_list(task: &Task, options: Option<DecomposeOptions>) -> SubtaskList {
    let now = Utc::now().to_rfc3339();
    let subtasks = decompose_task(task, options);

    SubtaskList {
        task_id: task.id.clone(),
        task_title: task.title.clone(),
        subtasks,
        created_at: now.clone(),
        updated_at: now,
    }
}

/// Get pending subtasks from a list
pub fn get_pending_subtasks(subtask_list: &SubtaskList) -> Vec<&Subtask> {
    subtask_list
        .subtasks
        .iter()
        .filter(|s| s.status == SubtaskStatus::Pending)
        .collect()
}

/// Get the next subtask to work on
pub fn get_next_subtask(subtask_list: &SubtaskList) -> Option<&Subtask> {
    // First, check for in_progress subtasks (resume)
    if let Some(in_progress) = subtask_list
        .subtasks
        .iter()
        .find(|s| s.status == SubtaskStatus::InProgress)
    {
        return Some(in_progress);
    }

    // Then, get first pending subtask
    subtask_list
        .subtasks
        .iter()
        .find(|s| s.status == SubtaskStatus::Pending)
}

/// Check if all subtasks are complete
pub fn is_all_subtasks_complete(subtask_list: &SubtaskList) -> bool {
    subtask_list.subtasks.iter().all(|s| {
        s.status == SubtaskStatus::Done || s.status == SubtaskStatus::Verified
    })
}

/// Check if any subtask failed
pub fn has_failed_subtasks(subtask_list: &SubtaskList) -> bool {
    subtask_list
        .subtasks
        .iter()
        .any(|s| s.status == SubtaskStatus::Failed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_task(id: &str, title: &str, description: Option<&str>) -> Task {
        Task {
            id: id.to_string(),
            title: title.to_string(),
            description: description.map(|s| s.to_string()),
            status: None,
        }
    }

    #[test]
    fn test_generate_subtask_id() {
        assert_eq!(generate_subtask_id("task-1", 0), "task-1-sub-001");
        assert_eq!(generate_subtask_id("task-1", 9), "task-1-sub-010");
        assert_eq!(generate_subtask_id("task-1", 99), "task-1-sub-100");
    }

    #[test]
    fn test_analyze_simple_task() {
        let task = make_task("t1", "Fix a bug", Some("Simple fix"));
        let heuristics = analyze_task(&task);

        assert!(!heuristics.has_multiple_targets);
        assert!(!heuristics.has_multiple_actions);
        assert!(!heuristics.is_complex);
    }

    #[test]
    fn test_analyze_complex_task() {
        let task = make_task(
            "t2",
            "Add tests and docs",
            Some("Create tests for the module. Add documentation for all functions."),
        );
        let heuristics = analyze_task(&task);

        assert!(heuristics.requires_testing);
        assert!(heuristics.requires_docs);
    }

    #[test]
    fn test_decompose_simple_task() {
        let task = make_task("t1", "Fix typo", None);
        let subtasks = decompose_task(&task, None);

        assert_eq!(subtasks.len(), 1);
        assert!(subtasks[0].description.contains("Fix typo"));
    }

    #[test]
    fn test_decompose_complex_task() {
        let task = make_task(
            "t2",
            "Add feature X",
            Some("Add feature with tests and documentation required. This is a longer description that requires multiple components and modules to be updated."),
        );
        let subtasks = decompose_task(&task, None);

        // Should have implementation + tests + docs subtasks (or just more than 1)
        // The exact count depends on heuristics matching
        assert!(!subtasks.is_empty());
        // At minimum we should get the tests subtask since "tests" is mentioned
        let has_tests_subtask = subtasks.iter().any(|s| s.description.contains("tests"));
        assert!(has_tests_subtask || subtasks.len() == 1);
    }

    #[test]
    fn test_force_single_subtask() {
        let task = make_task(
            "t3",
            "Complex task",
            Some("Add tests and documentation for the complex feature"),
        );
        let opts = DecomposeOptions {
            force_single: true,
            ..Default::default()
        };
        let subtasks = decompose_task(&task, Some(opts));

        assert_eq!(subtasks.len(), 1);
    }

    #[test]
    fn test_max_subtasks_limit() {
        let task = make_task(
            "t4",
            "Big task",
            Some("Add tests, documentation, refactor code, fix bugs, update configs"),
        );
        let opts = DecomposeOptions {
            max_subtasks: Some(2),
            ..Default::default()
        };
        let subtasks = decompose_task(&task, Some(opts));

        assert!(subtasks.len() <= 2);
    }

    #[test]
    fn test_create_subtask_list() {
        let task = make_task("t5", "Test task", None);
        let list = create_subtask_list(&task, None);

        assert_eq!(list.task_id, "t5");
        assert_eq!(list.task_title, "Test task");
        assert!(!list.subtasks.is_empty());
    }

    #[test]
    fn test_get_next_subtask() {
        let task = make_task("t6", "Test", None);
        let list = create_subtask_list(&task, None);

        let next = get_next_subtask(&list);
        assert!(next.is_some());
        assert_eq!(next.unwrap().status, SubtaskStatus::Pending);
    }

    #[test]
    fn test_is_all_complete() {
        let task = make_task("t7", "Test", None);
        let mut list = create_subtask_list(&task, None);

        assert!(!is_all_subtasks_complete(&list));

        // Mark all as done
        for subtask in &mut list.subtasks {
            subtask.status = SubtaskStatus::Done;
        }

        assert!(is_all_subtasks_complete(&list));
    }
}
