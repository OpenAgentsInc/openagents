use crate::tasks::*;
use std::time::Duration;

pub fn next_pending_index(task: &Task) -> Option<usize> {
    task.queue.iter().position(|s| matches!(s.status, SubtaskStatus::Pending))
}

pub fn current_running_index(task: &Task) -> Option<usize> {
    task.queue.iter().position(|s| matches!(s.status, SubtaskStatus::Running))
}

pub fn start_subtask(task: Task, i: usize) -> Task {
    let mut t = task; // take ownership
    if let Some(s) = t.queue.get_mut(i) { s.status = SubtaskStatus::Running; }
    // Reflect overall task status when any subtask starts
    t.status = TaskStatus::Running;
    t
}

pub fn complete_subtask(task: Task, i: usize) -> Task {
    let mut t = task;
    if let Some(s) = t.queue.get_mut(i) { s.status = SubtaskStatus::Done; }
    // Naively record that at least one turn occurred while advancing this subtask.
    // Detailed token/time metrics will be populated by the streaming hook.
    t.metrics.turns = t.metrics.turns.saturating_add(1);
    if next_pending_index(&t).is_none() { t.status = TaskStatus::Completed; }
    else { t.status = TaskStatus::Planned; }
    t
}

/// Returns Some(reason) if any budget limit is hit given the cumulative metrics plus in-flight counters.
pub fn budget_hit(
    budget: &AutonomyBudget,
    base_metrics: &Metrics,
    turns_this_run: u32,
    tokens_in_delta: u64,
    tokens_out_delta: u64,
    elapsed: Duration,
) -> Option<&'static str> {
    if let Some(max) = budget.max_turns { if (base_metrics.turns as u32) + turns_this_run >= max { return Some("turns"); } }
    if let Some(max) = budget.max_tokens { if base_metrics.tokens_in + base_metrics.tokens_out + tokens_in_delta + tokens_out_delta >= max as u64 { return Some("tokens"); } }
    if let Some(max) = budget.max_minutes { if (base_metrics.wall_clock_minutes + (elapsed.as_secs()/60) as u64) >= max as u64 { return Some("time"); } }
    None
}

/// Exponential backoff helper with a soft cap.
pub fn compute_backoff_delay(attempt: u32) -> Duration {
    // Base 200ms, double each attempt, cap at 2000ms
    let base_ms: u64 = 200;
    let pow = 1u64 << attempt.min(4); // 2^attempt up to 2^4
    let ms = base_ms.saturating_mul(pow).min(2000);
    Duration::from_millis(ms)
}

/// Attach protocol session metadata to a running subtask.
pub fn attach_session(task: Task, i: usize, session_id: String, rollout_path: Option<String>) -> Task {
    let mut t = task;
    if let Some(s) = t.queue.get_mut(i) {
        s.session_id = Some(session_id);
        s.rollout_path = rollout_path;
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn sample_task() -> Task {
        Task { version:1, id:"t1".into(), name:"Demo".into(), status:TaskStatus::Planned, created_at:Utc::now(), updated_at:Utc::now(), autonomy_budget:AutonomyBudget::default(), stop_conditions:StopConditions::default(), queue: vec![
            Subtask{ id:"s1".into(), title:"A".into(), status:SubtaskStatus::Pending, inputs:serde_json::json!({}), session_id:None, rollout_path:None, last_error:None, retries:0},
            Subtask{ id:"s2".into(), title:"B".into(), status:SubtaskStatus::Pending, inputs:serde_json::json!({}), session_id:None, rollout_path:None, last_error:None, retries:0},
        ], metrics: Metrics::default() }
    }

    #[test]
    fn pending_index_and_transitions() {
        let t = sample_task();
        assert_eq!(next_pending_index(&t), Some(0));
        assert_eq!(current_running_index(&t), None);
        let t = start_subtask(t, 0);
        assert!(matches!(t.queue[0].status, SubtaskStatus::Running));
        assert!(matches!(t.status, TaskStatus::Running));
        assert_eq!(current_running_index(&t), Some(0));
        let t = complete_subtask(t, 0);
        assert!(matches!(t.queue[0].status, SubtaskStatus::Done));
        // turns metric should increment when completing
        assert_eq!(t.metrics.turns, 1);
        assert_eq!(next_pending_index(&t), Some(1));
        let t = start_subtask(t, 1);
        let t = complete_subtask(t, 1);
        assert!(matches!(t.status, TaskStatus::Completed));
        assert_eq!(t.metrics.turns, 2);
    }

    #[test]
    fn attach_session_sets_fields() {
        let t = sample_task();
        let t = start_subtask(t, 0);
        let t = attach_session(t, 0, "sess-123".into(), Some("/tmp/rollout-1.jsonl".into()));
        assert_eq!(t.queue[0].session_id.as_deref(), Some("sess-123"));
        assert_eq!(t.queue[0].rollout_path.as_deref(), Some("/tmp/rollout-1.jsonl"));
    }

    #[test]
    fn budgets_turns_tokens_time() {
        let budget = AutonomyBudget { approvals: "never".into(), sandbox: "danger-full-access".into(), max_turns: Some(2), max_tokens: Some(100), max_minutes: Some(1) };
        let base = Metrics { turns: 1, tokens_in: 10, tokens_out: 10, wall_clock_minutes: 0, retries: 0 };
        // Hitting turns at +1 turn
        assert_eq!(budget_hit(&budget, &base, 1, 0, 0, Duration::from_secs(0)), Some("turns"));
        // Not yet hitting tokens
        assert_eq!(budget_hit(&budget, &base, 0, 10, 10, Duration::from_secs(0)), None);
        // Hitting tokens if we add lots
        assert_eq!(budget_hit(&budget, &base, 0, 100, 0, Duration::from_secs(0)), Some("tokens"));
        // Hitting time at >=1 minute elapsed
        assert_eq!(budget_hit(&budget, &base, 0, 0, 0, Duration::from_secs(60)), Some("time"));
    }

    #[test]
    fn backoff_grows_and_caps() {
        assert_eq!(compute_backoff_delay(0), Duration::from_millis(200));
        assert_eq!(compute_backoff_delay(1), Duration::from_millis(400));
        assert_eq!(compute_backoff_delay(2), Duration::from_millis(800));
        assert_eq!(compute_backoff_delay(3), Duration::from_millis(1600));
        assert_eq!(compute_backoff_delay(4), Duration::from_millis(2000));
        assert_eq!(compute_backoff_delay(5), Duration::from_millis(2000));
    }
}
