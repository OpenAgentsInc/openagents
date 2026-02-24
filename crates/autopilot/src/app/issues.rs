use adjutant::IssueSummary;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WorkspaceIssueStatus {
    Open,
    InProgress,
    Completed,
    Blocked,
    Other,
}

pub(crate) fn issue_status(issue: &IssueSummary) -> WorkspaceIssueStatus {
    if issue.is_blocked {
        return WorkspaceIssueStatus::Blocked;
    }
    match issue.status.trim().to_ascii_lowercase().as_str() {
        "open" => WorkspaceIssueStatus::Open,
        "in_progress" | "in-progress" | "inprogress" => WorkspaceIssueStatus::InProgress,
        "completed" | "done" | "closed" => WorkspaceIssueStatus::Completed,
        "blocked" => WorkspaceIssueStatus::Blocked,
        _ => WorkspaceIssueStatus::Other,
    }
}

pub(crate) fn issue_status_label(issue: &IssueSummary) -> String {
    match issue_status(issue) {
        WorkspaceIssueStatus::Open => "Open".to_string(),
        WorkspaceIssueStatus::InProgress => "In progress".to_string(),
        WorkspaceIssueStatus::Completed => "Completed".to_string(),
        WorkspaceIssueStatus::Blocked => "Blocked".to_string(),
        WorkspaceIssueStatus::Other => humanize_label(&issue.status),
    }
}

pub(crate) fn issue_status_rank(issue: &IssueSummary) -> u8 {
    match issue_status(issue) {
        WorkspaceIssueStatus::Blocked => 0,
        WorkspaceIssueStatus::InProgress => 1,
        WorkspaceIssueStatus::Open => 2,
        WorkspaceIssueStatus::Completed => 3,
        WorkspaceIssueStatus::Other => 4,
    }
}

pub(crate) fn issue_priority_rank(priority: &str) -> u8 {
    match priority.trim().to_ascii_lowercase().as_str() {
        "urgent" => 0,
        "high" => 1,
        "medium" => 2,
        "low" => 3,
        _ => 4,
    }
}

pub(crate) fn issue_priority_label(priority: &str) -> String {
    humanize_label(priority)
}

pub(crate) fn sort_workspace_issues<'a>(issues: &'a [IssueSummary]) -> Vec<&'a IssueSummary> {
    let mut entries: Vec<&IssueSummary> = issues.iter().collect();
    entries.sort_by_key(|issue| {
        (
            issue_status_rank(issue),
            issue_priority_rank(&issue.priority),
            issue.number,
        )
    });
    entries
}

fn humanize_label(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "Unknown".to_string();
    }
    let lowered = trimmed
        .to_ascii_lowercase()
        .replace('_', " ")
        .replace('-', " ");
    let mut chars = lowered.chars();
    match chars.next() {
        Some(first) => {
            let mut out = String::new();
            out.push(first.to_ascii_uppercase());
            out.push_str(chars.as_str());
            out
        }
        None => "Unknown".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn issue(status: &str, priority: &str, blocked: bool) -> IssueSummary {
        IssueSummary {
            number: 1,
            title: "Test".to_string(),
            description: None,
            issue_type: None,
            status: status.to_string(),
            priority: priority.to_string(),
            is_blocked: blocked,
            blocked_reason: None,
            created_at: None,
            updated_at: None,
            last_checked: None,
        }
    }

    #[test]
    fn maps_issue_status_and_priority() {
        let open = issue("open", "high", false);
        assert_eq!(issue_status(&open), WorkspaceIssueStatus::Open);
        assert_eq!(issue_status_label(&open), "Open");
        assert_eq!(issue_priority_rank(&open.priority), 1);

        let progress = issue("in_progress", "medium", false);
        assert_eq!(issue_status(&progress), WorkspaceIssueStatus::InProgress);

        let done = issue("completed", "low", false);
        assert_eq!(issue_status(&done), WorkspaceIssueStatus::Completed);

        let blocked = issue("open", "urgent", true);
        assert_eq!(issue_status(&blocked), WorkspaceIssueStatus::Blocked);
        assert_eq!(issue_status_label(&blocked), "Blocked");
    }

    #[test]
    fn sorts_workspace_issues_by_status_then_priority_then_number() {
        let mut urgent_in_progress = issue("in_progress", "urgent", false);
        urgent_in_progress.number = 12;
        let mut blocked_low = issue("open", "low", true);
        blocked_low.number = 40;
        let mut open_high = issue("open", "high", false);
        open_high.number = 3;
        let mut open_medium = issue("open", "medium", false);
        open_medium.number = 2;
        let mut complete_urgent = issue("completed", "urgent", false);
        complete_urgent.number = 1;

        let issues = [
            open_medium.clone(),
            complete_urgent.clone(),
            blocked_low.clone(),
            urgent_in_progress.clone(),
            open_high.clone(),
        ];
        let sorted = sort_workspace_issues(&issues);
        let sorted_numbers: Vec<u32> = sorted.iter().map(|issue| issue.number).collect();
        assert_eq!(sorted_numbers, vec![40, 12, 3, 2, 1]);
    }
}
