use crate::tasks::*;
use crate::master::{next_pending_index, start_subtask, complete_subtask, budget_hit};
use anyhow::Result;
use std::time::Duration;

fn fallback_plan_from_goal(goal: &str) -> Vec<Subtask> {
    let mut out = Vec::new();
    let sentences: Vec<&str> = goal
        .split(|c| c == '.' || c == '\n' || c == ';')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    for (i, s) in sentences.into_iter().take(10).enumerate() {
        out.push(Subtask { id: format!("s{:02}", i + 1), title: s.to_string(), status: SubtaskStatus::Pending, inputs: serde_json::json!({}), session_id: None, rollout_path: None, last_error: None, retries: 0 });
    }
    if out.is_empty() {
        out.push(Subtask { id: "s01".into(), title: goal.trim().to_string(), status: SubtaskStatus::Pending, inputs: serde_json::json!({}), session_id: None, rollout_path: None, last_error: None, retries: 0 });
    }
    out
}

/// Headless plan: use fallback planner and persist.
pub fn plan_with_fallback(task_id: &str, goal: &str) -> Result<Task> {
    let mut t = task_get(task_id)?;
    t.queue = fallback_plan_from_goal(goal);
    t.status = TaskStatus::Planned;
    task_update(t)
}

/// Headless runner: simulate token usage/turns and enforce budgets/time.
/// Returns updated task after a single budgeted run over the current subtask.
pub fn run_once_headless(task_id: &str) -> Result<Task> {
    let mut task = task_get(task_id)?;
    let idx_opt = next_pending_index(&task).or_else(||
        task.queue.iter().position(|s| matches!(s.status, SubtaskStatus::Running))
    );
    let Some(i) = idx_opt else { return Ok(task) };

    if budget_hit(&task.autonomy_budget, &task.metrics, 0, 0, 0, Duration::from_secs(0)).is_some() {
        // Already out of budget; mark paused
        task.status = TaskStatus::Paused;
        return task_update(task);
    }

    if !matches!(task.queue[i].status, SubtaskStatus::Running) {
        task = start_subtask(task, i);
    }
    // Simulate one "turn": add small token deltas and increment counters
    task.metrics.tokens_in = task.metrics.tokens_in.saturating_add(100);
    task.metrics.tokens_out = task.metrics.tokens_out.saturating_add(50);
    task = complete_subtask(task, i);
    task_update(task)
}

