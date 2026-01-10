use std::cmp::Reverse;

use oanix::manifest::DirectiveSummary;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DirectiveStatus {
    Active,
    Paused,
    Completed,
    Other,
}

pub(crate) fn directive_status(directive: &DirectiveSummary) -> DirectiveStatus {
    match directive.status.trim().to_ascii_lowercase().as_str() {
        "active" => DirectiveStatus::Active,
        "paused" | "on_hold" | "on-hold" | "hold" => DirectiveStatus::Paused,
        "completed" | "done" | "closed" => DirectiveStatus::Completed,
        _ => DirectiveStatus::Other,
    }
}

pub(crate) fn directive_status_label(directive: &DirectiveSummary) -> String {
    match directive_status(directive) {
        DirectiveStatus::Active => "Active".to_string(),
        DirectiveStatus::Paused => "Paused".to_string(),
        DirectiveStatus::Completed => "Completed".to_string(),
        DirectiveStatus::Other => humanize_label(&directive.status),
    }
}

pub(crate) fn directive_status_rank(directive: &DirectiveSummary) -> u8 {
    match directive_status(directive) {
        DirectiveStatus::Active => 0,
        DirectiveStatus::Paused => 1,
        DirectiveStatus::Other => 2,
        DirectiveStatus::Completed => 3,
    }
}

pub(crate) fn directive_priority_rank(priority: Option<&str>) -> u8 {
    match priority
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "urgent" => 0,
        "high" => 1,
        "medium" => 2,
        "low" => 3,
        _ => 4,
    }
}

pub(crate) fn directive_priority_label(priority: Option<&str>) -> String {
    match priority {
        Some(value) => humanize_label(value),
        None => "None".to_string(),
    }
}

pub(crate) fn directive_id_number(id: &str) -> u32 {
    let trimmed = id.trim();
    let numeric = trimmed
        .trim_start_matches("d-")
        .trim_start_matches('d');
    numeric.parse::<u32>().unwrap_or(0)
}

pub(crate) fn sort_workspace_directives<'a>(
    directives: &'a [DirectiveSummary],
) -> Vec<&'a DirectiveSummary> {
    let mut entries: Vec<&DirectiveSummary> = directives.iter().collect();
    entries.sort_by_key(|directive| {
        (
            directive_status_rank(directive),
            directive_priority_rank(directive.priority.as_deref()),
            Reverse(directive_id_number(&directive.id)),
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

    fn directive(status: &str, priority: Option<&str>) -> DirectiveSummary {
        DirectiveSummary {
            id: "d-042".to_string(),
            title: "Test".to_string(),
            status: status.to_string(),
            priority: priority.map(|value| value.to_string()),
            progress_pct: None,
        }
    }

    #[test]
    fn maps_directive_status_and_priority() {
        let active = directive("active", Some("high"));
        assert_eq!(directive_status(&active), DirectiveStatus::Active);
        assert_eq!(directive_status_label(&active), "Active");
        assert_eq!(directive_priority_rank(active.priority.as_deref()), 1);

        let paused = directive("on_hold", Some("low"));
        assert_eq!(directive_status(&paused), DirectiveStatus::Paused);
        assert_eq!(directive_status_label(&paused), "Paused");

        let done = directive("completed", None);
        assert_eq!(directive_status(&done), DirectiveStatus::Completed);
        assert_eq!(directive_priority_label(done.priority.as_deref()), "None");
    }
}
