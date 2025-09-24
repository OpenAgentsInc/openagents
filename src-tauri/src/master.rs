use crate::tasks::*;

pub fn next_pending_index(task: &Task) -> Option<usize> {
    task.queue.iter().position(|s| matches!(s.status, SubtaskStatus::Pending))
}

pub fn start_subtask(task: Task, i: usize) -> Task {
    let mut t = task; // take ownership
    if let Some(s) = t.queue.get_mut(i) { s.status = SubtaskStatus::Running; }
    t
}

pub fn complete_subtask(task: Task, i: usize) -> Task {
    let mut t = task;
    if let Some(s) = t.queue.get_mut(i) { s.status = SubtaskStatus::Done; }
    if next_pending_index(&t).is_none() { t.status = TaskStatus::Completed; }
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn sample_task() -> Task {
        Task { version:1, id:"t1".into(), name:"Demo".into(), status:TaskStatus::Planned, created_at:Utc::now(), updated_at:Utc::now(), autonomy_budget:AutonomyBudget::default(), stop_conditions:StopConditions::default(), queue: vec![
            Subtask{ id:"s1".into(), title:"A".into(), status:SubtaskStatus::Pending, inputs:serde_json::json!({}), session_id:None, rollout_path:None, last_error:None},
            Subtask{ id:"s2".into(), title:"B".into(), status:SubtaskStatus::Pending, inputs:serde_json::json!({}), session_id:None, rollout_path:None, last_error:None},
        ], metrics: Metrics::default() }
    }

    #[test]
    fn pending_index_and_transitions() {
        let t = sample_task();
        assert_eq!(next_pending_index(&t), Some(0));
        let t = start_subtask(t, 0);
        assert!(matches!(t.queue[0].status, SubtaskStatus::Running));
        let t = complete_subtask(t, 0);
        assert!(matches!(t.queue[0].status, SubtaskStatus::Done));
        assert_eq!(next_pending_index(&t), Some(1));
        let t = start_subtask(t, 1);
        let t = complete_subtask(t, 1);
        assert!(matches!(t.status, TaskStatus::Completed));
    }
}
