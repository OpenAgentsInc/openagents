use serde::{Deserialize, Serialize};

pub const PROJECT_OPS_STEP0_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectOpsWorkItemId(String);

impl ProjectOpsWorkItemId {
    pub fn new(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        let normalized = value.trim();
        if normalized.is_empty() {
            return Err("work item id must not be empty".to_string());
        }
        Ok(Self(normalized.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectOpsCycleId(String);

impl ProjectOpsCycleId {
    pub fn new(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        let normalized = value.trim();
        if normalized.is_empty() {
            return Err("cycle id must not be empty".to_string());
        }
        Ok(Self(normalized.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectOpsTeamKey(String);

impl ProjectOpsTeamKey {
    pub fn new(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        let normalized = value.trim();
        if normalized.is_empty() {
            return Err("team key must not be empty".to_string());
        }
        Ok(Self(normalized.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectOpsProjectId(String);

impl ProjectOpsProjectId {
    pub fn new(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        let normalized = value.trim();
        if normalized.is_empty() {
            return Err("project id must not be empty".to_string());
        }
        Ok(Self(normalized.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectOpsWorkItemStatus {
    Backlog,
    Todo,
    InProgress,
    InReview,
    Done,
    Cancelled,
}

impl ProjectOpsWorkItemStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Backlog => "backlog",
            Self::Todo => "todo",
            Self::InProgress => "in_progress",
            Self::InReview => "in_review",
            Self::Done => "done",
            Self::Cancelled => "cancelled",
        }
    }

    pub const fn workflow() -> &'static [Self] {
        &[
            Self::Backlog,
            Self::Todo,
            Self::InProgress,
            Self::InReview,
            Self::Done,
            Self::Cancelled,
        ]
    }

    pub fn workflow_summary() -> String {
        Self::workflow()
            .iter()
            .map(|status| status.label())
            .collect::<Vec<_>>()
            .join(" -> ")
    }

    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Done | Self::Cancelled)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectOpsPriority {
    Urgent,
    High,
    Medium,
    Low,
    None,
}

impl ProjectOpsPriority {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Urgent => "urgent",
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
            Self::None => "none",
        }
    }

    pub const fn all() -> &'static [Self] {
        &[
            Self::Urgent,
            Self::High,
            Self::Medium,
            Self::Low,
            Self::None,
        ]
    }

    pub fn summary() -> String {
        Self::all()
            .iter()
            .map(|priority| priority.label())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectOpsWorkItem {
    pub work_item_id: ProjectOpsWorkItemId,
    pub title: String,
    pub description: String,
    pub status: ProjectOpsWorkItemStatus,
    pub priority: ProjectOpsPriority,
    pub assignee: Option<String>,
    pub team_key: ProjectOpsTeamKey,
    pub project_id: Option<ProjectOpsProjectId>,
    pub cycle_id: Option<ProjectOpsCycleId>,
    pub parent_id: Option<ProjectOpsWorkItemId>,
    pub area_tags: Vec<String>,
    pub blocked_reason: Option<String>,
    pub due_at_unix_ms: Option<u64>,
    pub created_at_unix_ms: u64,
    pub updated_at_unix_ms: u64,
    pub archived_at_unix_ms: Option<u64>,
}

impl ProjectOpsWorkItem {
    pub fn validate(&self) -> Result<(), String> {
        if self.title.trim().is_empty() {
            return Err("work item title must not be empty".to_string());
        }
        if self.description.trim().is_empty() {
            return Err("work item description must not be empty".to_string());
        }
        if self.created_at_unix_ms == 0 {
            return Err("created_at_unix_ms must be > 0".to_string());
        }
        if self.updated_at_unix_ms < self.created_at_unix_ms {
            return Err("updated_at_unix_ms must be >= created_at_unix_ms".to_string());
        }
        if self.area_tags.len() > 2 {
            return Err("Step 0 supports at most two area_tags".to_string());
        }
        if self.area_tags.iter().any(|tag| tag.trim().is_empty()) {
            return Err("area_tags must not contain empty values".to_string());
        }
        if self.due_at_unix_ms == Some(0) {
            return Err("due_at_unix_ms must be > 0 when present".to_string());
        }
        if self
            .blocked_reason
            .as_deref()
            .is_some_and(|reason| reason.trim().is_empty())
        {
            return Err("blocked_reason must not be blank when present".to_string());
        }
        if self
            .assignee
            .as_deref()
            .is_some_and(|assignee| assignee.trim().is_empty())
        {
            return Err("assignee must not be blank when present".to_string());
        }
        if self
            .archived_at_unix_ms
            .is_some_and(|value| value < self.updated_at_unix_ms)
        {
            return Err("archived_at_unix_ms must be >= updated_at_unix_ms".to_string());
        }
        Ok(())
    }

    pub fn is_blocked(&self) -> bool {
        self.blocked_reason
            .as_deref()
            .is_some_and(|reason| !reason.trim().is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PROJECT_OPS_STEP0_SCHEMA_VERSION, ProjectOpsCycleId, ProjectOpsPriority,
        ProjectOpsProjectId, ProjectOpsTeamKey, ProjectOpsWorkItem, ProjectOpsWorkItemId,
        ProjectOpsWorkItemStatus,
    };

    fn sample_work_item() -> ProjectOpsWorkItem {
        ProjectOpsWorkItem {
            work_item_id: ProjectOpsWorkItemId::new("wi-1").expect("id"),
            title: "Ship the Step 0 PM shell".to_string(),
            description: "Add the first native PM pane behind a feature gate.".to_string(),
            status: ProjectOpsWorkItemStatus::Backlog,
            priority: ProjectOpsPriority::High,
            assignee: Some("cdavid".to_string()),
            team_key: ProjectOpsTeamKey::new("desktop").expect("team"),
            project_id: Some(ProjectOpsProjectId::new("desktop-pm").expect("project")),
            cycle_id: Some(ProjectOpsCycleId::new("2026-w10").expect("cycle")),
            parent_id: None,
            area_tags: vec!["pm".to_string(), "desktop".to_string()],
            blocked_reason: None,
            due_at_unix_ms: None,
            created_at_unix_ms: 1_762_000_000_000,
            updated_at_unix_ms: 1_762_000_000_000,
            archived_at_unix_ms: None,
        }
    }

    #[test]
    fn step0_schema_version_is_one() {
        assert_eq!(PROJECT_OPS_STEP0_SCHEMA_VERSION, 1);
    }

    #[test]
    fn workflow_summary_matches_step0_plan() {
        assert_eq!(
            ProjectOpsWorkItemStatus::workflow_summary(),
            "backlog -> todo -> in_progress -> in_review -> done -> cancelled"
        );
        assert!(ProjectOpsWorkItemStatus::Done.is_terminal());
        assert!(ProjectOpsWorkItemStatus::Cancelled.is_terminal());
        assert!(!ProjectOpsWorkItemStatus::InProgress.is_terminal());
    }

    #[test]
    fn priority_summary_matches_step0_scale() {
        assert_eq!(
            ProjectOpsPriority::summary(),
            "urgent, high, medium, low, none"
        );
    }

    #[test]
    fn work_item_validation_accepts_step0_shape() {
        let item = sample_work_item();
        assert_eq!(item.validate(), Ok(()));
        assert!(!item.is_blocked());
    }

    #[test]
    fn work_item_validation_rejects_blank_required_fields() {
        let mut item = sample_work_item();
        item.title = "   ".to_string();
        assert_eq!(
            item.validate(),
            Err("work item title must not be empty".to_string())
        );

        item = sample_work_item();
        item.description = String::new();
        assert_eq!(
            item.validate(),
            Err("work item description must not be empty".to_string())
        );
    }

    #[test]
    fn work_item_validation_rejects_blank_blocked_reason_and_too_many_tags() {
        let mut item = sample_work_item();
        item.blocked_reason = Some("   ".to_string());
        assert_eq!(
            item.validate(),
            Err("blocked_reason must not be blank when present".to_string())
        );

        item = sample_work_item();
        item.area_tags.push("extra".to_string());
        assert_eq!(
            item.validate(),
            Err("Step 0 supports at most two area_tags".to_string())
        );

        let mut item = sample_work_item();
        item.due_at_unix_ms = Some(0);
        assert_eq!(
            item.validate(),
            Err("due_at_unix_ms must be > 0 when present".to_string())
        );
    }

    #[test]
    fn identifier_types_reject_blank_values() {
        assert_eq!(
            ProjectOpsWorkItemId::new("   "),
            Err("work item id must not be empty".to_string())
        );
        assert_eq!(
            ProjectOpsCycleId::new(""),
            Err("cycle id must not be empty".to_string())
        );
        assert_eq!(
            ProjectOpsTeamKey::new(""),
            Err("team key must not be empty".to_string())
        );
        assert_eq!(
            ProjectOpsProjectId::new(""),
            Err("project id must not be empty".to_string())
        );
    }
}
