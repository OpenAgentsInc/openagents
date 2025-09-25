use crate::tasks::*;
use crate::master::{next_pending_index, start_subtask, complete_subtask, budget_hit};
use anyhow::Result;
use chrono::Utc;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::PathBuf;
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
    append_log(task_id, &format!("plan start goal=\"{}\"", goal.replace('\n', " ")))?;
    t.queue = fallback_plan_from_goal(goal);
    t.status = TaskStatus::Planned;
    let t = task_update(t)?;
    append_log(task_id, &format!("plan done queue_len={}", t.queue.len()))?;
    Ok(t)
}

/// Headless runner: simulate token usage/turns and enforce budgets/time.
/// Returns updated task after a single budgeted run over the current subtask.
pub fn run_once_headless(task_id: &str) -> Result<Task> {
    let mut task = task_get(task_id)?;
    let idx_opt = next_pending_index(&task).or_else(||
        task.queue.iter().position(|s| matches!(s.status, SubtaskStatus::Running))
    );
    let Some(i) = idx_opt else { append_log(task_id, "run: no pending or running subtasks; nothing to do")?; return Ok(task) };

    if budget_hit(&task.autonomy_budget, &task.metrics, 0, 0, 0, Duration::from_secs(0)).is_some() {
        // Already out of budget; mark paused
        task.status = TaskStatus::Paused;
        append_log(task_id, "run: budget exceeded pre-check; pausing task")?;
        return task_update(task);
    }

    if !matches!(task.queue[i].status, SubtaskStatus::Running) {
        append_log(task_id, &format!("run: start subtask id={} title=\"{}\"", task.queue[i].id, task.queue[i].title))?;
        task = start_subtask(task, i);
    }
    // Simulate one "turn": add small token deltas and increment counters
    let before_in = task.metrics.tokens_in;
    let before_out = task.metrics.tokens_out;
    task.metrics.tokens_in = task.metrics.tokens_in.saturating_add(100);
    task.metrics.tokens_out = task.metrics.tokens_out.saturating_add(50);
    task = complete_subtask(task, i);
    let t = task_update(task)?;
    append_log(task_id, &format!(
        "run: completed subtask id={} turns={} tokens_in_delta={} tokens_out_delta={} next_pending={:?}",
        t.queue[i].id,
        t.metrics.turns,
        t.metrics.tokens_in.saturating_sub(before_in),
        t.metrics.tokens_out.saturating_sub(before_out),
        super::next_pending_index(&t)))?;
    Ok(t)
}

fn codex_home() -> PathBuf {
    if let Ok(val) = std::env::var("CODEX_HOME") { if !val.is_empty() { return PathBuf::from(val); } }
    dirs::home_dir().map(|mut h| { h.push(".codex"); h }).expect("home dir")
}

fn log_path(task_id: &str) -> PathBuf {
    let mut p = codex_home();
    p.push("master-tasks");
    p.push(format!("{}.log", task_id));
    p
}

fn append_log(task_id: &str, line: &str) -> Result<()> {
    let p = log_path(task_id);
    if let Some(parent) = p.parent() { let _ = fs::create_dir_all(parent); }
    let mut f = OpenOptions::new().create(true).append(true).open(&p)?;
    let ts = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    writeln!(f, "{} | {}", ts, line)?;
    Ok(())
}
