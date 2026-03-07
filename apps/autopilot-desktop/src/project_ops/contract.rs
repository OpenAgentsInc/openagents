use serde::{Deserialize, Serialize};

use super::schema::{
    ProjectOpsCycleId, ProjectOpsPriority, ProjectOpsTeamKey, ProjectOpsWorkItem,
    ProjectOpsWorkItemId, ProjectOpsWorkItemStatus,
};

pub const PROJECT_OPS_WORK_ITEMS_STREAM_ID: &str = "stream.pm.work_items.v1";
pub const PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID: &str = "stream.pm.activity_projection.v1";
pub const PROJECT_OPS_CYCLES_STREAM_ID: &str = "stream.pm.cycles.v1";
pub const PROJECT_OPS_SAVED_VIEWS_STREAM_ID: &str = "stream.pm.saved_views.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectOpsStreamSpec {
    pub stream_id: &'static str,
    pub purpose: &'static str,
}

const STEP0_STREAM_SPECS: [ProjectOpsStreamSpec; 4] = [
    ProjectOpsStreamSpec {
        stream_id: PROJECT_OPS_WORK_ITEMS_STREAM_ID,
        purpose: "Current work-item state and list projection",
    },
    ProjectOpsStreamSpec {
        stream_id: PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID,
        purpose: "Human-readable item history and state-change feed",
    },
    ProjectOpsStreamSpec {
        stream_id: PROJECT_OPS_CYCLES_STREAM_ID,
        purpose: "Active cycle definitions and summaries",
    },
    ProjectOpsStreamSpec {
        stream_id: PROJECT_OPS_SAVED_VIEWS_STREAM_ID,
        purpose: "Built-in and user-defined saved views",
    },
];

pub fn step0_stream_specs() -> &'static [ProjectOpsStreamSpec] {
    &STEP0_STREAM_SPECS
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectOpsCommandId(String);

impl ProjectOpsCommandId {
    pub fn new(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        let normalized = value.trim();
        if normalized.is_empty() {
            return Err("project ops command_id must not be empty".to_string());
        }
        Ok(Self(normalized.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectOpsActor {
    pub actor_id: Option<String>,
    pub actor_label: Option<String>,
}

impl ProjectOpsActor {
    pub fn validate(&self) -> Result<(), String> {
        let actor_id = self.actor_id.as_deref().map(str::trim);
        let actor_label = self.actor_label.as_deref().map(str::trim);
        if actor_id.is_some_and(str::is_empty) {
            return Err("project ops actor_id must not be blank when present".to_string());
        }
        if actor_label.is_some_and(str::is_empty) {
            return Err("project ops actor_label must not be blank when present".to_string());
        }
        if actor_id.is_none() && actor_label.is_none() {
            return Err("project ops actor requires actor_id or actor_label".to_string());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectOpsCommandName {
    CreateWorkItem,
    EditWorkItemFields,
    ChangeWorkItemStatus,
    AssignWorkItem,
    ClearAssignee,
    SetWorkItemCycle,
    ClearWorkItemCycle,
    SetBlockedReason,
    ClearBlockedReason,
    SetParentWorkItem,
    ClearParentWorkItem,
    ArchiveWorkItem,
    UnarchiveWorkItem,
}

impl ProjectOpsCommandName {
    pub const fn label(self) -> &'static str {
        match self {
            Self::CreateWorkItem => "CreateWorkItem",
            Self::EditWorkItemFields => "EditWorkItemFields",
            Self::ChangeWorkItemStatus => "ChangeWorkItemStatus",
            Self::AssignWorkItem => "AssignWorkItem",
            Self::ClearAssignee => "ClearAssignee",
            Self::SetWorkItemCycle => "SetWorkItemCycle",
            Self::ClearWorkItemCycle => "ClearWorkItemCycle",
            Self::SetBlockedReason => "SetBlockedReason",
            Self::ClearBlockedReason => "ClearBlockedReason",
            Self::SetParentWorkItem => "SetParentWorkItem",
            Self::ClearParentWorkItem => "ClearParentWorkItem",
            Self::ArchiveWorkItem => "ArchiveWorkItem",
            Self::UnarchiveWorkItem => "UnarchiveWorkItem",
        }
    }

    pub const fn all() -> &'static [Self] {
        &[
            Self::CreateWorkItem,
            Self::EditWorkItemFields,
            Self::ChangeWorkItemStatus,
            Self::AssignWorkItem,
            Self::ClearAssignee,
            Self::SetWorkItemCycle,
            Self::ClearWorkItemCycle,
            Self::SetBlockedReason,
            Self::ClearBlockedReason,
            Self::SetParentWorkItem,
            Self::ClearParentWorkItem,
            Self::ArchiveWorkItem,
            Self::UnarchiveWorkItem,
        ]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectOpsAcceptedEventName {
    WorkItemCreated,
    WorkItemFieldsEdited,
    WorkItemStatusChanged,
    WorkItemAssigned,
    WorkItemAssigneeCleared,
    WorkItemCycleSet,
    WorkItemCycleCleared,
    WorkItemBlocked,
    WorkItemUnblocked,
    WorkItemParentSet,
    WorkItemParentCleared,
    WorkItemArchived,
    WorkItemUnarchived,
}

impl ProjectOpsAcceptedEventName {
    pub const fn label(self) -> &'static str {
        match self {
            Self::WorkItemCreated => "WorkItemCreated",
            Self::WorkItemFieldsEdited => "WorkItemFieldsEdited",
            Self::WorkItemStatusChanged => "WorkItemStatusChanged",
            Self::WorkItemAssigned => "WorkItemAssigned",
            Self::WorkItemAssigneeCleared => "WorkItemAssigneeCleared",
            Self::WorkItemCycleSet => "WorkItemCycleSet",
            Self::WorkItemCycleCleared => "WorkItemCycleCleared",
            Self::WorkItemBlocked => "WorkItemBlocked",
            Self::WorkItemUnblocked => "WorkItemUnblocked",
            Self::WorkItemParentSet => "WorkItemParentSet",
            Self::WorkItemParentCleared => "WorkItemParentCleared",
            Self::WorkItemArchived => "WorkItemArchived",
            Self::WorkItemUnarchived => "WorkItemUnarchived",
        }
    }

    pub const fn all() -> &'static [Self] {
        &[
            Self::WorkItemCreated,
            Self::WorkItemFieldsEdited,
            Self::WorkItemStatusChanged,
            Self::WorkItemAssigned,
            Self::WorkItemAssigneeCleared,
            Self::WorkItemCycleSet,
            Self::WorkItemCycleCleared,
            Self::WorkItemBlocked,
            Self::WorkItemUnblocked,
            Self::WorkItemParentSet,
            Self::WorkItemParentCleared,
            Self::WorkItemArchived,
            Self::WorkItemUnarchived,
        ]
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectOpsWorkItemDraft {
    pub work_item_id: ProjectOpsWorkItemId,
    pub title: String,
    pub description: String,
    pub status: ProjectOpsWorkItemStatus,
    pub priority: ProjectOpsPriority,
    pub assignee: Option<String>,
    pub team_key: ProjectOpsTeamKey,
    pub cycle_id: Option<ProjectOpsCycleId>,
    pub parent_id: Option<ProjectOpsWorkItemId>,
    pub area_tags: Vec<String>,
    pub blocked_reason: Option<String>,
    pub due_at_unix_ms: Option<u64>,
}

impl ProjectOpsWorkItemDraft {
    pub fn validate(&self) -> Result<(), String> {
        let synthetic = ProjectOpsWorkItem {
            work_item_id: self.work_item_id.clone(),
            title: self.title.clone(),
            description: self.description.clone(),
            status: self.status,
            priority: self.priority,
            assignee: self.assignee.clone(),
            team_key: self.team_key.clone(),
            cycle_id: self.cycle_id.clone(),
            parent_id: self.parent_id.clone(),
            area_tags: self.area_tags.clone(),
            blocked_reason: self.blocked_reason.clone(),
            due_at_unix_ms: self.due_at_unix_ms,
            created_at_unix_ms: 1,
            updated_at_unix_ms: 1,
            archived_at_unix_ms: None,
        };
        synthetic.validate()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectOpsEditWorkItemFieldsPatch {
    pub title: Option<String>,
    pub description: Option<String>,
    pub priority: Option<ProjectOpsPriority>,
    pub due_at_unix_ms: Option<Option<u64>>,
    pub area_tags: Option<Vec<String>>,
}

impl ProjectOpsEditWorkItemFieldsPatch {
    pub fn validate(&self) -> Result<(), String> {
        if self.title.is_none()
            && self.description.is_none()
            && self.priority.is_none()
            && self.due_at_unix_ms.is_none()
            && self.area_tags.is_none()
        {
            return Err("project ops edit patch must include at least one field change".to_string());
        }
        if self.title.as_deref().is_some_and(|value| value.trim().is_empty()) {
            return Err("project ops edit patch title must not be blank".to_string());
        }
        if self
            .description
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err("project ops edit patch description must not be blank".to_string());
        }
        if self.area_tags.as_ref().is_some_and(|tags| tags.len() > 2) {
            return Err("project ops edit patch supports at most two area_tags".to_string());
        }
        if self
            .area_tags
            .as_ref()
            .is_some_and(|tags| tags.iter().any(|tag| tag.trim().is_empty()))
        {
            return Err("project ops edit patch area_tags must not contain blanks".to_string());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectOpsCreateWorkItem {
    pub draft: ProjectOpsWorkItemDraft,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectOpsEditWorkItemFields {
    pub work_item_id: ProjectOpsWorkItemId,
    pub patch: ProjectOpsEditWorkItemFieldsPatch,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectOpsChangeWorkItemStatus {
    pub work_item_id: ProjectOpsWorkItemId,
    pub status: ProjectOpsWorkItemStatus,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectOpsAssignWorkItem {
    pub work_item_id: ProjectOpsWorkItemId,
    pub assignee: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectOpsWorkItemRef {
    pub work_item_id: ProjectOpsWorkItemId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectOpsSetWorkItemCycle {
    pub work_item_id: ProjectOpsWorkItemId,
    pub cycle_id: ProjectOpsCycleId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectOpsSetBlockedReason {
    pub work_item_id: ProjectOpsWorkItemId,
    pub blocked_reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectOpsSetParentWorkItem {
    pub work_item_id: ProjectOpsWorkItemId,
    pub parent_id: ProjectOpsWorkItemId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProjectOpsCommand {
    CreateWorkItem(ProjectOpsCreateWorkItem),
    EditWorkItemFields(ProjectOpsEditWorkItemFields),
    ChangeWorkItemStatus(ProjectOpsChangeWorkItemStatus),
    AssignWorkItem(ProjectOpsAssignWorkItem),
    ClearAssignee(ProjectOpsWorkItemRef),
    SetWorkItemCycle(ProjectOpsSetWorkItemCycle),
    ClearWorkItemCycle(ProjectOpsWorkItemRef),
    SetBlockedReason(ProjectOpsSetBlockedReason),
    ClearBlockedReason(ProjectOpsWorkItemRef),
    SetParentWorkItem(ProjectOpsSetParentWorkItem),
    ClearParentWorkItem(ProjectOpsWorkItemRef),
    ArchiveWorkItem(ProjectOpsWorkItemRef),
    UnarchiveWorkItem(ProjectOpsWorkItemRef),
}

impl ProjectOpsCommand {
    pub const fn name(&self) -> ProjectOpsCommandName {
        match self {
            Self::CreateWorkItem(_) => ProjectOpsCommandName::CreateWorkItem,
            Self::EditWorkItemFields(_) => ProjectOpsCommandName::EditWorkItemFields,
            Self::ChangeWorkItemStatus(_) => ProjectOpsCommandName::ChangeWorkItemStatus,
            Self::AssignWorkItem(_) => ProjectOpsCommandName::AssignWorkItem,
            Self::ClearAssignee(_) => ProjectOpsCommandName::ClearAssignee,
            Self::SetWorkItemCycle(_) => ProjectOpsCommandName::SetWorkItemCycle,
            Self::ClearWorkItemCycle(_) => ProjectOpsCommandName::ClearWorkItemCycle,
            Self::SetBlockedReason(_) => ProjectOpsCommandName::SetBlockedReason,
            Self::ClearBlockedReason(_) => ProjectOpsCommandName::ClearBlockedReason,
            Self::SetParentWorkItem(_) => ProjectOpsCommandName::SetParentWorkItem,
            Self::ClearParentWorkItem(_) => ProjectOpsCommandName::ClearParentWorkItem,
            Self::ArchiveWorkItem(_) => ProjectOpsCommandName::ArchiveWorkItem,
            Self::UnarchiveWorkItem(_) => ProjectOpsCommandName::UnarchiveWorkItem,
        }
    }

    pub const fn expected_events(&self) -> &'static [ProjectOpsAcceptedEventName] {
        match self {
            Self::CreateWorkItem(_) => &[ProjectOpsAcceptedEventName::WorkItemCreated],
            Self::EditWorkItemFields(_) => &[ProjectOpsAcceptedEventName::WorkItemFieldsEdited],
            Self::ChangeWorkItemStatus(_) => &[ProjectOpsAcceptedEventName::WorkItemStatusChanged],
            Self::AssignWorkItem(_) => &[ProjectOpsAcceptedEventName::WorkItemAssigned],
            Self::ClearAssignee(_) => &[ProjectOpsAcceptedEventName::WorkItemAssigneeCleared],
            Self::SetWorkItemCycle(_) => &[ProjectOpsAcceptedEventName::WorkItemCycleSet],
            Self::ClearWorkItemCycle(_) => &[ProjectOpsAcceptedEventName::WorkItemCycleCleared],
            Self::SetBlockedReason(_) => &[ProjectOpsAcceptedEventName::WorkItemBlocked],
            Self::ClearBlockedReason(_) => &[ProjectOpsAcceptedEventName::WorkItemUnblocked],
            Self::SetParentWorkItem(_) => &[ProjectOpsAcceptedEventName::WorkItemParentSet],
            Self::ClearParentWorkItem(_) => &[ProjectOpsAcceptedEventName::WorkItemParentCleared],
            Self::ArchiveWorkItem(_) => &[ProjectOpsAcceptedEventName::WorkItemArchived],
            Self::UnarchiveWorkItem(_) => &[ProjectOpsAcceptedEventName::WorkItemUnarchived],
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::CreateWorkItem(command) => command.draft.validate(),
            Self::EditWorkItemFields(command) => command.patch.validate(),
            Self::ChangeWorkItemStatus(_) => Ok(()),
            Self::AssignWorkItem(command) => {
                if command.assignee.trim().is_empty() {
                    return Err("project ops assignee must not be blank".to_string());
                }
                Ok(())
            }
            Self::ClearAssignee(_)
            | Self::SetWorkItemCycle(_)
            | Self::ClearWorkItemCycle(_)
            | Self::SetParentWorkItem(_)
            | Self::ClearParentWorkItem(_)
            | Self::ArchiveWorkItem(_)
            | Self::UnarchiveWorkItem(_) => Ok(()),
            Self::SetBlockedReason(command) => {
                if command.blocked_reason.trim().is_empty() {
                    return Err("project ops blocked_reason must not be blank".to_string());
                }
                Ok(())
            }
            Self::ClearBlockedReason(_) => Ok(()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectOpsCommandEnvelope {
    pub command_id: ProjectOpsCommandId,
    pub issued_at_unix_ms: u64,
    pub actor: ProjectOpsActor,
    pub command: ProjectOpsCommand,
}

impl ProjectOpsCommandEnvelope {
    pub fn validate(&self) -> Result<(), String> {
        if self.issued_at_unix_ms == 0 {
            return Err("project ops issued_at_unix_ms must be > 0".to_string());
        }
        self.actor.validate()?;
        self.command.validate()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectOpsWorkItemFieldsEdited {
    pub work_item_id: ProjectOpsWorkItemId,
    pub patch: ProjectOpsEditWorkItemFieldsPatch,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectOpsWorkItemStatusChanged {
    pub work_item_id: ProjectOpsWorkItemId,
    pub status: ProjectOpsWorkItemStatus,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectOpsWorkItemAssigned {
    pub work_item_id: ProjectOpsWorkItemId,
    pub assignee: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectOpsWorkItemCycleSet {
    pub work_item_id: ProjectOpsWorkItemId,
    pub cycle_id: ProjectOpsCycleId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectOpsWorkItemBlocked {
    pub work_item_id: ProjectOpsWorkItemId,
    pub blocked_reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectOpsWorkItemParentSet {
    pub work_item_id: ProjectOpsWorkItemId,
    pub parent_id: ProjectOpsWorkItemId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectOpsWorkItemArchived {
    pub work_item_id: ProjectOpsWorkItemId,
    pub archived_at_unix_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProjectOpsAcceptedEvent {
    WorkItemCreated { work_item: ProjectOpsWorkItem },
    WorkItemFieldsEdited(ProjectOpsWorkItemFieldsEdited),
    WorkItemStatusChanged(ProjectOpsWorkItemStatusChanged),
    WorkItemAssigned(ProjectOpsWorkItemAssigned),
    WorkItemAssigneeCleared(ProjectOpsWorkItemRef),
    WorkItemCycleSet(ProjectOpsWorkItemCycleSet),
    WorkItemCycleCleared(ProjectOpsWorkItemRef),
    WorkItemBlocked(ProjectOpsWorkItemBlocked),
    WorkItemUnblocked(ProjectOpsWorkItemRef),
    WorkItemParentSet(ProjectOpsWorkItemParentSet),
    WorkItemParentCleared(ProjectOpsWorkItemRef),
    WorkItemArchived(ProjectOpsWorkItemArchived),
    WorkItemUnarchived(ProjectOpsWorkItemRef),
}

impl ProjectOpsAcceptedEvent {
    pub const fn name(&self) -> ProjectOpsAcceptedEventName {
        match self {
            Self::WorkItemCreated { .. } => ProjectOpsAcceptedEventName::WorkItemCreated,
            Self::WorkItemFieldsEdited(_) => ProjectOpsAcceptedEventName::WorkItemFieldsEdited,
            Self::WorkItemStatusChanged(_) => ProjectOpsAcceptedEventName::WorkItemStatusChanged,
            Self::WorkItemAssigned(_) => ProjectOpsAcceptedEventName::WorkItemAssigned,
            Self::WorkItemAssigneeCleared(_) => ProjectOpsAcceptedEventName::WorkItemAssigneeCleared,
            Self::WorkItemCycleSet(_) => ProjectOpsAcceptedEventName::WorkItemCycleSet,
            Self::WorkItemCycleCleared(_) => ProjectOpsAcceptedEventName::WorkItemCycleCleared,
            Self::WorkItemBlocked(_) => ProjectOpsAcceptedEventName::WorkItemBlocked,
            Self::WorkItemUnblocked(_) => ProjectOpsAcceptedEventName::WorkItemUnblocked,
            Self::WorkItemParentSet(_) => ProjectOpsAcceptedEventName::WorkItemParentSet,
            Self::WorkItemParentCleared(_) => ProjectOpsAcceptedEventName::WorkItemParentCleared,
            Self::WorkItemArchived(_) => ProjectOpsAcceptedEventName::WorkItemArchived,
            Self::WorkItemUnarchived(_) => ProjectOpsAcceptedEventName::WorkItemUnarchived,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::WorkItemCreated { work_item } => work_item.validate(),
            Self::WorkItemFieldsEdited(event) => event.patch.validate(),
            Self::WorkItemAssigned(event) => {
                if event.assignee.trim().is_empty() {
                    return Err("project ops accepted event assignee must not be blank".to_string());
                }
                Ok(())
            }
            Self::WorkItemBlocked(event) => {
                if event.blocked_reason.trim().is_empty() {
                    return Err(
                        "project ops accepted event blocked_reason must not be blank".to_string()
                    );
                }
                Ok(())
            }
            Self::WorkItemArchived(event) => {
                if event.archived_at_unix_ms == 0 {
                    return Err(
                        "project ops accepted event archived_at_unix_ms must be > 0".to_string()
                    );
                }
                Ok(())
            }
            Self::WorkItemStatusChanged(_)
            | Self::WorkItemAssigneeCleared(_)
            | Self::WorkItemCycleSet(_)
            | Self::WorkItemCycleCleared(_)
            | Self::WorkItemUnblocked(_)
            | Self::WorkItemParentSet(_)
            | Self::WorkItemParentCleared(_)
            | Self::WorkItemUnarchived(_) => Ok(()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectOpsAcceptedEventEnvelope {
    pub stream_id: String,
    pub seq: u64,
    pub command_id: ProjectOpsCommandId,
    pub emitted_at_unix_ms: u64,
    pub actor: ProjectOpsActor,
    pub event: ProjectOpsAcceptedEvent,
}

impl ProjectOpsAcceptedEventEnvelope {
    pub fn validate(&self) -> Result<(), String> {
        if self.stream_id != PROJECT_OPS_WORK_ITEMS_STREAM_ID {
            return Err(format!(
                "project ops accepted events must use {}",
                PROJECT_OPS_WORK_ITEMS_STREAM_ID
            ));
        }
        if self.seq == 0 {
            return Err("project ops accepted event seq must be > 0".to_string());
        }
        if self.emitted_at_unix_ms == 0 {
            return Err("project ops emitted_at_unix_ms must be > 0".to_string());
        }
        self.actor.validate()?;
        self.event.validate()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        step0_stream_specs, ProjectOpsAcceptedEvent, ProjectOpsAcceptedEventEnvelope,
        ProjectOpsAcceptedEventName, ProjectOpsActor, ProjectOpsCommand, ProjectOpsCommandEnvelope,
        ProjectOpsCommandId, ProjectOpsCommandName, ProjectOpsCreateWorkItem,
        ProjectOpsEditWorkItemFields, ProjectOpsEditWorkItemFieldsPatch, ProjectOpsSetBlockedReason,
        ProjectOpsWorkItemArchived, ProjectOpsWorkItemAssigned, ProjectOpsWorkItemDraft,
        ProjectOpsWorkItemRef, PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID,
        PROJECT_OPS_CYCLES_STREAM_ID, PROJECT_OPS_SAVED_VIEWS_STREAM_ID,
        PROJECT_OPS_WORK_ITEMS_STREAM_ID,
    };
    use crate::project_ops::schema::{
        ProjectOpsCycleId, ProjectOpsPriority, ProjectOpsTeamKey, ProjectOpsWorkItem,
        ProjectOpsWorkItemId, ProjectOpsWorkItemStatus,
    };

    fn actor() -> ProjectOpsActor {
        ProjectOpsActor {
            actor_id: Some("npub1actor".to_string()),
            actor_label: Some("cdavid".to_string()),
        }
    }

    fn work_item_id() -> ProjectOpsWorkItemId {
        ProjectOpsWorkItemId::new("wi-1").expect("work item id")
    }

    fn team_key() -> ProjectOpsTeamKey {
        ProjectOpsTeamKey::new("desktop").expect("team key")
    }

    fn cycle_id() -> ProjectOpsCycleId {
        ProjectOpsCycleId::new("2026-w10").expect("cycle id")
    }

    fn draft() -> ProjectOpsWorkItemDraft {
        ProjectOpsWorkItemDraft {
            work_item_id: work_item_id(),
            title: "Ship the PM command contract".to_string(),
            description: "Freeze Step 0 commands and events.".to_string(),
            status: ProjectOpsWorkItemStatus::Backlog,
            priority: ProjectOpsPriority::High,
            assignee: Some("cdavid".to_string()),
            team_key: team_key(),
            cycle_id: Some(cycle_id()),
            parent_id: None,
            area_tags: vec!["pm".to_string()],
            blocked_reason: None,
            due_at_unix_ms: None,
        }
    }

    fn work_item() -> ProjectOpsWorkItem {
        ProjectOpsWorkItem {
            work_item_id: work_item_id(),
            title: "Ship the PM command contract".to_string(),
            description: "Freeze Step 0 commands and events.".to_string(),
            status: ProjectOpsWorkItemStatus::Backlog,
            priority: ProjectOpsPriority::High,
            assignee: Some("cdavid".to_string()),
            team_key: team_key(),
            cycle_id: Some(cycle_id()),
            parent_id: None,
            area_tags: vec!["pm".to_string()],
            blocked_reason: None,
            due_at_unix_ms: None,
            created_at_unix_ms: 1_762_000_000_000,
            updated_at_unix_ms: 1_762_000_000_000,
            archived_at_unix_ms: None,
        }
    }

    #[test]
    fn step0_stream_catalog_matches_docs() {
        let actual = step0_stream_specs()
            .iter()
            .map(|spec| (spec.stream_id, spec.purpose))
            .collect::<Vec<_>>();
        assert_eq!(
            actual,
            vec![
                (
                    PROJECT_OPS_WORK_ITEMS_STREAM_ID,
                    "Current work-item state and list projection"
                ),
                (
                    PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID,
                    "Human-readable item history and state-change feed"
                ),
                (
                    PROJECT_OPS_CYCLES_STREAM_ID,
                    "Active cycle definitions and summaries"
                ),
                (
                    PROJECT_OPS_SAVED_VIEWS_STREAM_ID,
                    "Built-in and user-defined saved views"
                ),
            ]
        );
    }

    #[test]
    fn command_catalog_matches_step0_docs() {
        let labels = ProjectOpsCommandName::all()
            .iter()
            .map(|name| name.label())
            .collect::<Vec<_>>();
        assert_eq!(
            labels,
            vec![
                "CreateWorkItem",
                "EditWorkItemFields",
                "ChangeWorkItemStatus",
                "AssignWorkItem",
                "ClearAssignee",
                "SetWorkItemCycle",
                "ClearWorkItemCycle",
                "SetBlockedReason",
                "ClearBlockedReason",
                "SetParentWorkItem",
                "ClearParentWorkItem",
                "ArchiveWorkItem",
                "UnarchiveWorkItem",
            ]
        );
    }

    #[test]
    fn accepted_event_catalog_matches_step0_docs() {
        let labels = ProjectOpsAcceptedEventName::all()
            .iter()
            .map(|name| name.label())
            .collect::<Vec<_>>();
        assert_eq!(
            labels,
            vec![
                "WorkItemCreated",
                "WorkItemFieldsEdited",
                "WorkItemStatusChanged",
                "WorkItemAssigned",
                "WorkItemAssigneeCleared",
                "WorkItemCycleSet",
                "WorkItemCycleCleared",
                "WorkItemBlocked",
                "WorkItemUnblocked",
                "WorkItemParentSet",
                "WorkItemParentCleared",
                "WorkItemArchived",
                "WorkItemUnarchived",
            ]
        );
    }

    #[test]
    fn command_envelope_requires_timestamp_and_actor_metadata() {
        let envelope = ProjectOpsCommandEnvelope {
            command_id: ProjectOpsCommandId::new("cmd-1").expect("command id"),
            issued_at_unix_ms: 0,
            actor: ProjectOpsActor {
                actor_id: None,
                actor_label: None,
            },
            command: ProjectOpsCommand::CreateWorkItem(ProjectOpsCreateWorkItem { draft: draft() }),
        };
        assert_eq!(
            envelope.validate(),
            Err("project ops issued_at_unix_ms must be > 0".to_string())
        );

        let envelope = ProjectOpsCommandEnvelope {
            command_id: ProjectOpsCommandId::new("cmd-1").expect("command id"),
            issued_at_unix_ms: 1_762_000_000_000,
            actor: ProjectOpsActor {
                actor_id: None,
                actor_label: None,
            },
            command: ProjectOpsCommand::CreateWorkItem(ProjectOpsCreateWorkItem { draft: draft() }),
        };
        assert_eq!(
            envelope.validate(),
            Err("project ops actor requires actor_id or actor_label".to_string())
        );
    }

    #[test]
    fn edit_patch_requires_at_least_one_real_change() {
        let command = ProjectOpsCommand::EditWorkItemFields(ProjectOpsEditWorkItemFields {
            work_item_id: work_item_id(),
            patch: ProjectOpsEditWorkItemFieldsPatch {
                title: None,
                description: None,
                priority: None,
                due_at_unix_ms: None,
                area_tags: None,
            },
        });
        assert_eq!(
            command.validate(),
            Err("project ops edit patch must include at least one field change".to_string())
        );
    }

    #[test]
    fn command_expected_events_are_explicit() {
        let command = ProjectOpsCommand::AssignWorkItem(super::ProjectOpsAssignWorkItem {
            work_item_id: work_item_id(),
            assignee: "cdavid".to_string(),
        });
        assert_eq!(command.name(), ProjectOpsCommandName::AssignWorkItem);
        assert_eq!(
            command.expected_events(),
            &[ProjectOpsAcceptedEventName::WorkItemAssigned]
        );

        let command = ProjectOpsCommand::ClearBlockedReason(ProjectOpsWorkItemRef {
            work_item_id: work_item_id(),
        });
        assert_eq!(
            command.expected_events(),
            &[ProjectOpsAcceptedEventName::WorkItemUnblocked]
        );
    }

    #[test]
    fn accepted_event_envelope_requires_work_item_stream_and_seq() {
        let envelope = ProjectOpsAcceptedEventEnvelope {
            stream_id: PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID.to_string(),
            seq: 1,
            command_id: ProjectOpsCommandId::new("cmd-1").expect("command id"),
            emitted_at_unix_ms: 1_762_000_000_000,
            actor: actor(),
            event: ProjectOpsAcceptedEvent::WorkItemAssigned(ProjectOpsWorkItemAssigned {
                work_item_id: work_item_id(),
                assignee: "cdavid".to_string(),
            }),
        };
        assert_eq!(
            envelope.validate(),
            Err(format!(
                "project ops accepted events must use {}",
                PROJECT_OPS_WORK_ITEMS_STREAM_ID
            ))
        );

        let envelope = ProjectOpsAcceptedEventEnvelope {
            stream_id: PROJECT_OPS_WORK_ITEMS_STREAM_ID.to_string(),
            seq: 0,
            command_id: ProjectOpsCommandId::new("cmd-1").expect("command id"),
            emitted_at_unix_ms: 1_762_000_000_000,
            actor: actor(),
            event: ProjectOpsAcceptedEvent::WorkItemAssigned(ProjectOpsWorkItemAssigned {
                work_item_id: work_item_id(),
                assignee: "cdavid".to_string(),
            }),
        };
        assert_eq!(
            envelope.validate(),
            Err("project ops accepted event seq must be > 0".to_string())
        );
    }

    #[test]
    fn accepted_event_name_matches_payload_and_validation() {
        let event = ProjectOpsAcceptedEvent::WorkItemBlocked(super::ProjectOpsWorkItemBlocked {
            work_item_id: work_item_id(),
            blocked_reason: "Waiting on upstream schema review".to_string(),
        });
        assert_eq!(event.name(), ProjectOpsAcceptedEventName::WorkItemBlocked);
        assert_eq!(event.validate(), Ok(()));

        let event = ProjectOpsAcceptedEvent::WorkItemArchived(ProjectOpsWorkItemArchived {
            work_item_id: work_item_id(),
            archived_at_unix_ms: 0,
        });
        assert_eq!(
            event.validate(),
            Err("project ops accepted event archived_at_unix_ms must be > 0".to_string())
        );
    }

    #[test]
    fn create_work_item_command_and_event_validate_against_schema_contract() {
        let command = ProjectOpsCommandEnvelope {
            command_id: ProjectOpsCommandId::new("cmd-42").expect("command id"),
            issued_at_unix_ms: 1_762_000_000_000,
            actor: actor(),
            command: ProjectOpsCommand::CreateWorkItem(ProjectOpsCreateWorkItem { draft: draft() }),
        };
        assert_eq!(command.validate(), Ok(()));

        let event = ProjectOpsAcceptedEventEnvelope {
            stream_id: PROJECT_OPS_WORK_ITEMS_STREAM_ID.to_string(),
            seq: 1,
            command_id: ProjectOpsCommandId::new("cmd-42").expect("command id"),
            emitted_at_unix_ms: 1_762_000_000_000,
            actor: actor(),
            event: ProjectOpsAcceptedEvent::WorkItemCreated { work_item: work_item() },
        };
        assert_eq!(event.validate(), Ok(()));
    }

    #[test]
    fn blocked_and_assignee_contracts_reject_blank_values() {
        let command = ProjectOpsCommand::SetBlockedReason(ProjectOpsSetBlockedReason {
            work_item_id: work_item_id(),
            blocked_reason: "   ".to_string(),
        });
        assert_eq!(
            command.validate(),
            Err("project ops blocked_reason must not be blank".to_string())
        );

        let event = ProjectOpsAcceptedEvent::WorkItemAssigned(ProjectOpsWorkItemAssigned {
            work_item_id: work_item_id(),
            assignee: " ".to_string(),
        });
        assert_eq!(
            event.validate(),
            Err("project ops accepted event assignee must not be blank".to_string())
        );
    }
}
