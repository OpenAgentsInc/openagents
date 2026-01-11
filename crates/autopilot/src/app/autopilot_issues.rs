use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use issues::issue::list_issues;
use issues::{Issue, IssueType, Priority, Status};
use rusqlite::{Connection, OpenFlags};

#[derive(Clone, Debug)]
pub(crate) enum AutopilotIssuesStatus {
    Idle,
    Refreshing,
    NoWorkspace,
    MissingDatabase,
    Error(String),
}

impl AutopilotIssuesStatus {
    pub(crate) fn label(&self) -> &str {
        match self {
            AutopilotIssuesStatus::Idle => "Idle",
            AutopilotIssuesStatus::Refreshing => "Refreshing",
            AutopilotIssuesStatus::NoWorkspace => "No workspace",
            AutopilotIssuesStatus::MissingDatabase => "Database missing",
            AutopilotIssuesStatus::Error(_) => "Error",
        }
    }

    pub(crate) fn error(&self) -> Option<&str> {
        match self {
            AutopilotIssuesStatus::Error(message) => Some(message),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct IssueCounts {
    pub(crate) open: usize,
    pub(crate) in_progress: usize,
    pub(crate) done: usize,
    pub(crate) blocked: usize,
    pub(crate) auto_created: usize,
    pub(crate) claimed: usize,
}

impl IssueCounts {
    pub(crate) fn from(issues: &[Issue]) -> Self {
        let mut counts = Self::default();
        for issue in issues {
            match issue.status {
                Status::Open => counts.open += 1,
                Status::InProgress => counts.in_progress += 1,
                Status::Done => counts.done += 1,
            }
            if issue.is_blocked {
                counts.blocked += 1;
            }
            if issue.auto_created {
                counts.auto_created += 1;
            }
            if issue.claimed_by.is_some() {
                counts.claimed += 1;
            }
        }
        counts
    }
}

#[derive(Clone, Debug)]
pub(crate) struct AutopilotIssuesSnapshot {
    pub(crate) db_path: Option<PathBuf>,
    pub(crate) db_exists: bool,
    pub(crate) issues: Vec<Issue>,
}

impl AutopilotIssuesSnapshot {
    fn empty(db_path: Option<PathBuf>, db_exists: bool) -> Self {
        Self {
            db_path,
            db_exists,
            issues: Vec::new(),
        }
    }
}

pub(crate) struct AutopilotIssuesState {
    pub(crate) status: AutopilotIssuesStatus,
    pub(crate) snapshot: AutopilotIssuesSnapshot,
    pub(crate) last_refresh: Option<u64>,
}

impl AutopilotIssuesState {
    pub(crate) fn new() -> Self {
        Self {
            status: AutopilotIssuesStatus::Idle,
            snapshot: AutopilotIssuesSnapshot::empty(None, false),
            last_refresh: None,
        }
    }

    pub(crate) fn refresh(&mut self, workspace_root: Option<&Path>) {
        self.status = AutopilotIssuesStatus::Refreshing;
        let (status, snapshot) = load_snapshot(workspace_root);
        self.status = status;
        self.snapshot = snapshot;
        self.last_refresh = Some(now());
    }
}

impl Default for AutopilotIssuesState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AutopilotIssueState {
    Blocked,
    InProgress,
    Open,
    Done,
}

pub(crate) fn issue_state(issue: &Issue) -> AutopilotIssueState {
    if issue.is_blocked {
        return AutopilotIssueState::Blocked;
    }
    match issue.status {
        Status::InProgress => AutopilotIssueState::InProgress,
        Status::Done => AutopilotIssueState::Done,
        Status::Open => AutopilotIssueState::Open,
    }
}

pub(crate) fn issue_state_label(issue: &Issue) -> &'static str {
    match issue_state(issue) {
        AutopilotIssueState::Blocked => "Blocked",
        AutopilotIssueState::InProgress => "In progress",
        AutopilotIssueState::Open => "Open",
        AutopilotIssueState::Done => "Done",
    }
}

pub(crate) fn issue_state_rank(issue: &Issue) -> u8 {
    match issue_state(issue) {
        AutopilotIssueState::Blocked => 0,
        AutopilotIssueState::InProgress => 1,
        AutopilotIssueState::Open => 2,
        AutopilotIssueState::Done => 3,
    }
}

pub(crate) fn issue_priority_rank(priority: Priority) -> u8 {
    match priority {
        Priority::Urgent => 0,
        Priority::High => 1,
        Priority::Medium => 2,
        Priority::Low => 3,
    }
}

pub(crate) fn issue_priority_label(priority: Priority) -> &'static str {
    match priority {
        Priority::Urgent => "Urgent",
        Priority::High => "High",
        Priority::Medium => "Medium",
        Priority::Low => "Low",
    }
}

pub(crate) fn issue_type_label(issue_type: IssueType) -> &'static str {
    match issue_type {
        IssueType::Task => "Task",
        IssueType::Bug => "Bug",
        IssueType::Feature => "Feature",
    }
}

pub(crate) fn sort_autopilot_issues<'a>(issues: &'a [Issue]) -> Vec<&'a Issue> {
    let mut entries: Vec<&Issue> = issues.iter().collect();
    entries.sort_by_key(|issue| {
        (
            issue_state_rank(issue),
            issue_priority_rank(issue.priority),
            issue.number,
        )
    });
    entries
}

fn load_snapshot(
    workspace_root: Option<&Path>,
) -> (AutopilotIssuesStatus, AutopilotIssuesSnapshot) {
    let Some(root) = workspace_root else {
        return (
            AutopilotIssuesStatus::NoWorkspace,
            AutopilotIssuesSnapshot::empty(None, false),
        );
    };

    let db_path = root.join(".openagents").join("autopilot.db");
    if !db_path.exists() {
        return (
            AutopilotIssuesStatus::MissingDatabase,
            AutopilotIssuesSnapshot::empty(Some(db_path), false),
        );
    }

    let conn = match Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_ONLY) {
        Ok(conn) => conn,
        Err(err) => {
            return (
                AutopilotIssuesStatus::Error(format!("DB open failed: {}", err)),
                AutopilotIssuesSnapshot::empty(Some(db_path), true),
            );
        }
    };

    let issues = match list_issues(&conn, None) {
        Ok(issues) => issues,
        Err(err) => {
            return (
                AutopilotIssuesStatus::Error(format!("Issue query failed: {}", err)),
                AutopilotIssuesSnapshot::empty(Some(db_path), true),
            );
        }
    };

    (
        AutopilotIssuesStatus::Idle,
        AutopilotIssuesSnapshot {
            db_path: Some(db_path),
            db_exists: true,
            issues,
        },
    )
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn issue(status: Status, priority: Priority, blocked: bool) -> Issue {
        Issue {
            id: "id-1".to_string(),
            number: 1,
            title: "Test".to_string(),
            description: None,
            status,
            priority,
            issue_type: IssueType::Task,
            agent: "claude".to_string(),
            directive_id: None,
            project_id: None,
            is_blocked: blocked,
            blocked_reason: None,
            claimed_by: None,
            claimed_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            completed_at: None,
            auto_created: false,
        }
    }

    #[test]
    fn maps_autopilot_issue_state() {
        let open = issue(Status::Open, Priority::High, false);
        assert_eq!(issue_state(&open), AutopilotIssueState::Open);
        assert_eq!(issue_state_label(&open), "Open");

        let progress = issue(Status::InProgress, Priority::Medium, false);
        assert_eq!(issue_state(&progress), AutopilotIssueState::InProgress);
        assert_eq!(issue_state_label(&progress), "In progress");

        let blocked = issue(Status::Open, Priority::Urgent, true);
        assert_eq!(issue_state(&blocked), AutopilotIssueState::Blocked);
        assert_eq!(issue_state_label(&blocked), "Blocked");
    }
}
