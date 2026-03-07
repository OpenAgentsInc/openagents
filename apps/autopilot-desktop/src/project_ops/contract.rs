use serde::{Deserialize, Serialize};

use super::schema::{
    ProjectOpsCycleId, ProjectOpsPriority, ProjectOpsTeamKey, ProjectOpsWorkItem,
    ProjectOpsWorkItemId, ProjectOpsWorkItemStatus,
};

pub const PROJECT_OPS_WORK_ITEMS_STREAM_ID: &str = "stream.pm.work_items.v1";
pub const PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID: &str = "stream.pm.activity_projection.v1";
pub const PROJECT_OPS_CYCLES_STREAM_ID: &str = "stream.pm.cycles.v1";
pub const PROJECT_OPS_SAVED_VIEWS_STREAM_ID: &str = "stream.pm.saved_views.v1";
pub const PROJECT_OPS_V1_CONTRACT_VERSION: &str = "project_ops.contract.v1";
pub const PROJECT_OPS_PRIMARY_SOURCE_BADGE: &str = "source: stream.pm.work_items.v1";
pub const PROJECT_OPS_SYNC_LIFECYCLE_SOURCE_BADGE: &str = "source: spacetime.sync.lifecycle";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ProjectOpsStreamSpec {
    pub stream_id: &'static str,
    pub purpose: &'static str,
    pub projection_payload: &'static str,
    pub projection_responsibility: &'static str,
}

const STEP0_STREAM_SPECS: [ProjectOpsStreamSpec; 4] = [
    ProjectOpsStreamSpec {
        stream_id: PROJECT_OPS_WORK_ITEMS_STREAM_ID,
        purpose: "Current work-item state and list projection",
        projection_payload: "Vec<ProjectOpsWorkItem>",
        projection_responsibility: "Visible work-item list/detail truth for the native PM pane under Phase 1 local replay semantics",
    },
    ProjectOpsStreamSpec {
        stream_id: PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID,
        purpose: "Human-readable item history and state-change feed",
        projection_payload: "Vec<ProjectOpsActivityRow>",
        projection_responsibility: "Human-readable state-change history derived from accepted PM events; not money or payout authority",
    },
    ProjectOpsStreamSpec {
        stream_id: PROJECT_OPS_CYCLES_STREAM_ID,
        purpose: "Active cycle definitions and summaries",
        projection_payload: "Vec<ProjectOpsCycleRow>",
        projection_responsibility: "Cycle definitions and current-cycle summaries used by list filters and assignment validation",
    },
    ProjectOpsStreamSpec {
        stream_id: PROJECT_OPS_SAVED_VIEWS_STREAM_ID,
        purpose: "Built-in and user-defined saved views",
        projection_payload: "Vec<ProjectOpsSavedViewRow>",
        projection_responsibility: "Built-in and user-authored PM view definitions used by toolbar, search, and list context",
    },
];

pub fn step0_stream_specs() -> &'static [ProjectOpsStreamSpec] {
    &STEP0_STREAM_SPECS
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectOpsDeliveryPhase {
    Step0,
    Phase3,
    Phase4,
    Phase5,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectOpsEntityKind {
    WorkItem,
    Cycle,
    SavedView,
    ActivityEvent,
    Comment,
    Team,
    Project,
    AgentTask,
    Bounty,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProjectOpsEntityContractSpec {
    pub entity_kind: ProjectOpsEntityKind,
    pub step0_required: bool,
    pub projection_only: bool,
    pub deferred_until_phase: Option<ProjectOpsDeliveryPhase>,
    pub purpose: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProjectOpsCommandContractSpec {
    pub command_name: ProjectOpsCommandName,
    pub payload_shape: Vec<&'static str>,
    pub accepted_events: Vec<ProjectOpsAcceptedEventName>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProjectOpsAcceptedEventContractSpec {
    pub event_name: ProjectOpsAcceptedEventName,
    pub payload_shape: Vec<&'static str>,
    pub stream_id: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProjectOpsContractManifest {
    pub contract_version: &'static str,
    pub step0_schema_version: u16,
    pub command_envelope_fields: Vec<&'static str>,
    pub accepted_event_envelope_fields: Vec<&'static str>,
    pub workflow: Vec<ProjectOpsWorkItemStatus>,
    pub priorities: Vec<ProjectOpsPriority>,
    pub entities: Vec<ProjectOpsEntityContractSpec>,
    pub commands: Vec<ProjectOpsCommandContractSpec>,
    pub accepted_events: Vec<ProjectOpsAcceptedEventContractSpec>,
    pub streams: Vec<ProjectOpsStreamSpec>,
    pub error_codes: Vec<ProjectOpsErrorCode>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProjectOpsSourceBadgeRule {
    pub badge: &'static str,
    pub scope: &'static str,
    pub truth_rule: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProjectOpsCheckpointRule {
    pub stream_id: &'static str,
    pub duplicate_policy: &'static str,
    pub out_of_order_policy: &'static str,
    pub stale_cursor_policy: &'static str,
    pub remote_checkpoint_policy: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProjectOpsSyncContract {
    pub current_rollout_phase: &'static str,
    pub local_truth_rule: &'static str,
    pub live_truth_rule: &'static str,
    pub source_badges: Vec<ProjectOpsSourceBadgeRule>,
    pub required_stream_grants: Vec<&'static str>,
    pub checkpoint_rules: Vec<ProjectOpsCheckpointRule>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectOpsErrorCode {
    InvalidCommand,
    DuplicateCommand,
    WorkItemExists,
    WorkItemMissing,
    InvalidTransition,
    DependencyMissing,
    ArchivedMutation,
    NoopMutation,
    CheckpointConflict,
}

impl ProjectOpsErrorCode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::InvalidCommand => "invalid_command",
            Self::DuplicateCommand => "duplicate_command",
            Self::WorkItemExists => "work_item_exists",
            Self::WorkItemMissing => "work_item_missing",
            Self::InvalidTransition => "invalid_transition",
            Self::DependencyMissing => "dependency_missing",
            Self::ArchivedMutation => "archived_mutation",
            Self::NoopMutation => "noop_mutation",
            Self::CheckpointConflict => "checkpoint_conflict",
        }
    }

    pub const fn all() -> &'static [Self] {
        &[
            Self::InvalidCommand,
            Self::DuplicateCommand,
            Self::WorkItemExists,
            Self::WorkItemMissing,
            Self::InvalidTransition,
            Self::DependencyMissing,
            Self::ArchivedMutation,
            Self::NoopMutation,
            Self::CheckpointConflict,
        ]
    }
}

pub fn project_ops_error(code: ProjectOpsErrorCode, message: impl AsRef<str>) -> String {
    format!("project_ops.{}: {}", code.label(), message.as_ref())
}

const COMMAND_ENVELOPE_FIELDS: [&str; 4] = [
    "command_id",
    "issued_at_unix_ms",
    "actor.actor_id|actor.actor_label",
    "command",
];

const ACCEPTED_EVENT_ENVELOPE_FIELDS: [&str; 5] = [
    "stream_id",
    "seq",
    "command_id",
    "emitted_at_unix_ms",
    "actor.actor_id|actor.actor_label",
];

pub fn project_ops_v1_contract_manifest() -> ProjectOpsContractManifest {
    ProjectOpsContractManifest {
        contract_version: PROJECT_OPS_V1_CONTRACT_VERSION,
        step0_schema_version: super::schema::PROJECT_OPS_STEP0_SCHEMA_VERSION,
        command_envelope_fields: COMMAND_ENVELOPE_FIELDS.to_vec(),
        accepted_event_envelope_fields: ACCEPTED_EVENT_ENVELOPE_FIELDS.to_vec(),
        workflow: ProjectOpsWorkItemStatus::workflow().to_vec(),
        priorities: ProjectOpsPriority::all().to_vec(),
        entities: vec![
            ProjectOpsEntityContractSpec {
                entity_kind: ProjectOpsEntityKind::WorkItem,
                step0_required: true,
                projection_only: false,
                deferred_until_phase: None,
                purpose: "Primary planning and execution record owned by the native PM command/event loop",
            },
            ProjectOpsEntityContractSpec {
                entity_kind: ProjectOpsEntityKind::Cycle,
                step0_required: true,
                projection_only: false,
                deferred_until_phase: None,
                purpose: "Timeboxed commitment bucket used for current-cycle filtering and assignment",
            },
            ProjectOpsEntityContractSpec {
                entity_kind: ProjectOpsEntityKind::SavedView,
                step0_required: true,
                projection_only: false,
                deferred_until_phase: None,
                purpose: "Reusable toolbar/list context definitions for built-in and user-authored PM views",
            },
            ProjectOpsEntityContractSpec {
                entity_kind: ProjectOpsEntityKind::ActivityEvent,
                step0_required: true,
                projection_only: true,
                deferred_until_phase: None,
                purpose: "Human-readable PM history derived from accepted events and used by the activity timeline",
            },
            ProjectOpsEntityContractSpec {
                entity_kind: ProjectOpsEntityKind::Comment,
                step0_required: false,
                projection_only: false,
                deferred_until_phase: Some(ProjectOpsDeliveryPhase::Phase3),
                purpose: "Append-only collaborative discussion records; deferred until multi-user PM is earned",
            },
            ProjectOpsEntityContractSpec {
                entity_kind: ProjectOpsEntityKind::Team,
                step0_required: false,
                projection_only: false,
                deferred_until_phase: Some(ProjectOpsDeliveryPhase::Phase3),
                purpose: "Shared planning scope and defaults across multiple operators or sub-groups",
            },
            ProjectOpsEntityContractSpec {
                entity_kind: ProjectOpsEntityKind::Project,
                step0_required: false,
                projection_only: false,
                deferred_until_phase: Some(ProjectOpsDeliveryPhase::Phase3),
                purpose: "Higher-level grouping for work items and cycles once the pilot proves it is needed",
            },
            ProjectOpsEntityContractSpec {
                entity_kind: ProjectOpsEntityKind::AgentTask,
                step0_required: false,
                projection_only: false,
                deferred_until_phase: Some(ProjectOpsDeliveryPhase::Phase4),
                purpose: "Execution-oriented task records that connect PM items to local agent/task runtimes",
            },
            ProjectOpsEntityContractSpec {
                entity_kind: ProjectOpsEntityKind::Bounty,
                step0_required: false,
                projection_only: false,
                deferred_until_phase: Some(ProjectOpsDeliveryPhase::Phase5),
                purpose: "Non-authoritative PM reference to funding and payout-related workflows; money truth stays elsewhere",
            },
        ],
        commands: vec![
            ProjectOpsCommandContractSpec {
                command_name: ProjectOpsCommandName::CreateWorkItem,
                payload_shape: vec![
                    "draft.work_item_id",
                    "draft.title",
                    "draft.description",
                    "draft.status",
                    "draft.priority",
                    "draft.assignee?",
                    "draft.team_key",
                    "draft.cycle_id?",
                    "draft.parent_id?",
                    "draft.area_tags[]",
                    "draft.blocked_reason?",
                    "draft.due_at_unix_ms?",
                ],
                accepted_events: vec![ProjectOpsAcceptedEventName::WorkItemCreated],
            },
            ProjectOpsCommandContractSpec {
                command_name: ProjectOpsCommandName::EditWorkItemFields,
                payload_shape: vec![
                    "work_item_id",
                    "patch.title?",
                    "patch.description?",
                    "patch.priority?",
                    "patch.due_at_unix_ms?",
                    "patch.area_tags[]?",
                ],
                accepted_events: vec![ProjectOpsAcceptedEventName::WorkItemFieldsEdited],
            },
            ProjectOpsCommandContractSpec {
                command_name: ProjectOpsCommandName::ChangeWorkItemStatus,
                payload_shape: vec!["work_item_id", "status"],
                accepted_events: vec![ProjectOpsAcceptedEventName::WorkItemStatusChanged],
            },
            ProjectOpsCommandContractSpec {
                command_name: ProjectOpsCommandName::AssignWorkItem,
                payload_shape: vec!["work_item_id", "assignee"],
                accepted_events: vec![ProjectOpsAcceptedEventName::WorkItemAssigned],
            },
            ProjectOpsCommandContractSpec {
                command_name: ProjectOpsCommandName::ClearAssignee,
                payload_shape: vec!["work_item_id"],
                accepted_events: vec![ProjectOpsAcceptedEventName::WorkItemAssigneeCleared],
            },
            ProjectOpsCommandContractSpec {
                command_name: ProjectOpsCommandName::SetWorkItemCycle,
                payload_shape: vec!["work_item_id", "cycle_id"],
                accepted_events: vec![ProjectOpsAcceptedEventName::WorkItemCycleSet],
            },
            ProjectOpsCommandContractSpec {
                command_name: ProjectOpsCommandName::ClearWorkItemCycle,
                payload_shape: vec!["work_item_id"],
                accepted_events: vec![ProjectOpsAcceptedEventName::WorkItemCycleCleared],
            },
            ProjectOpsCommandContractSpec {
                command_name: ProjectOpsCommandName::SetBlockedReason,
                payload_shape: vec!["work_item_id", "blocked_reason"],
                accepted_events: vec![ProjectOpsAcceptedEventName::WorkItemBlocked],
            },
            ProjectOpsCommandContractSpec {
                command_name: ProjectOpsCommandName::ClearBlockedReason,
                payload_shape: vec!["work_item_id"],
                accepted_events: vec![ProjectOpsAcceptedEventName::WorkItemUnblocked],
            },
            ProjectOpsCommandContractSpec {
                command_name: ProjectOpsCommandName::SetParentWorkItem,
                payload_shape: vec!["work_item_id", "parent_id"],
                accepted_events: vec![ProjectOpsAcceptedEventName::WorkItemParentSet],
            },
            ProjectOpsCommandContractSpec {
                command_name: ProjectOpsCommandName::ClearParentWorkItem,
                payload_shape: vec!["work_item_id"],
                accepted_events: vec![ProjectOpsAcceptedEventName::WorkItemParentCleared],
            },
            ProjectOpsCommandContractSpec {
                command_name: ProjectOpsCommandName::ArchiveWorkItem,
                payload_shape: vec!["work_item_id"],
                accepted_events: vec![ProjectOpsAcceptedEventName::WorkItemArchived],
            },
            ProjectOpsCommandContractSpec {
                command_name: ProjectOpsCommandName::UnarchiveWorkItem,
                payload_shape: vec!["work_item_id"],
                accepted_events: vec![ProjectOpsAcceptedEventName::WorkItemUnarchived],
            },
        ],
        accepted_events: vec![
            ProjectOpsAcceptedEventContractSpec {
                event_name: ProjectOpsAcceptedEventName::WorkItemCreated,
                payload_shape: vec!["work_item"],
                stream_id: PROJECT_OPS_WORK_ITEMS_STREAM_ID,
            },
            ProjectOpsAcceptedEventContractSpec {
                event_name: ProjectOpsAcceptedEventName::WorkItemFieldsEdited,
                payload_shape: vec!["work_item_id", "patch"],
                stream_id: PROJECT_OPS_WORK_ITEMS_STREAM_ID,
            },
            ProjectOpsAcceptedEventContractSpec {
                event_name: ProjectOpsAcceptedEventName::WorkItemStatusChanged,
                payload_shape: vec!["work_item_id", "status"],
                stream_id: PROJECT_OPS_WORK_ITEMS_STREAM_ID,
            },
            ProjectOpsAcceptedEventContractSpec {
                event_name: ProjectOpsAcceptedEventName::WorkItemAssigned,
                payload_shape: vec!["work_item_id", "assignee"],
                stream_id: PROJECT_OPS_WORK_ITEMS_STREAM_ID,
            },
            ProjectOpsAcceptedEventContractSpec {
                event_name: ProjectOpsAcceptedEventName::WorkItemAssigneeCleared,
                payload_shape: vec!["work_item_id"],
                stream_id: PROJECT_OPS_WORK_ITEMS_STREAM_ID,
            },
            ProjectOpsAcceptedEventContractSpec {
                event_name: ProjectOpsAcceptedEventName::WorkItemCycleSet,
                payload_shape: vec!["work_item_id", "cycle_id"],
                stream_id: PROJECT_OPS_WORK_ITEMS_STREAM_ID,
            },
            ProjectOpsAcceptedEventContractSpec {
                event_name: ProjectOpsAcceptedEventName::WorkItemCycleCleared,
                payload_shape: vec!["work_item_id"],
                stream_id: PROJECT_OPS_WORK_ITEMS_STREAM_ID,
            },
            ProjectOpsAcceptedEventContractSpec {
                event_name: ProjectOpsAcceptedEventName::WorkItemBlocked,
                payload_shape: vec!["work_item_id", "blocked_reason"],
                stream_id: PROJECT_OPS_WORK_ITEMS_STREAM_ID,
            },
            ProjectOpsAcceptedEventContractSpec {
                event_name: ProjectOpsAcceptedEventName::WorkItemUnblocked,
                payload_shape: vec!["work_item_id"],
                stream_id: PROJECT_OPS_WORK_ITEMS_STREAM_ID,
            },
            ProjectOpsAcceptedEventContractSpec {
                event_name: ProjectOpsAcceptedEventName::WorkItemParentSet,
                payload_shape: vec!["work_item_id", "parent_id"],
                stream_id: PROJECT_OPS_WORK_ITEMS_STREAM_ID,
            },
            ProjectOpsAcceptedEventContractSpec {
                event_name: ProjectOpsAcceptedEventName::WorkItemParentCleared,
                payload_shape: vec!["work_item_id"],
                stream_id: PROJECT_OPS_WORK_ITEMS_STREAM_ID,
            },
            ProjectOpsAcceptedEventContractSpec {
                event_name: ProjectOpsAcceptedEventName::WorkItemArchived,
                payload_shape: vec!["work_item_id", "archived_at_unix_ms"],
                stream_id: PROJECT_OPS_WORK_ITEMS_STREAM_ID,
            },
            ProjectOpsAcceptedEventContractSpec {
                event_name: ProjectOpsAcceptedEventName::WorkItemUnarchived,
                payload_shape: vec!["work_item_id"],
                stream_id: PROJECT_OPS_WORK_ITEMS_STREAM_ID,
            },
        ],
        streams: step0_stream_specs().to_vec(),
        error_codes: ProjectOpsErrorCode::all().to_vec(),
    }
}

pub fn project_ops_required_stream_grants() -> Vec<&'static str> {
    step0_stream_specs()
        .iter()
        .map(|spec| spec.stream_id)
        .collect()
}

pub fn project_ops_phase1_sync_contract() -> ProjectOpsSyncContract {
    ProjectOpsSyncContract {
        current_rollout_phase: "phase_1_mirror_proxy",
        local_truth_rule: "Project Ops visible state is sourced from replay-safe local PM projection streams and shared SyncApplyEngine checkpoints, not live PM reducers",
        live_truth_rule: "Only sync lifecycle/bootstrap health may use spacetime.sync.lifecycle in Phase 1; live PM collaboration truth requires a later ADR-approved Phase 2 cutover",
        source_badges: vec![
            ProjectOpsSourceBadgeRule {
                badge: PROJECT_OPS_PRIMARY_SOURCE_BADGE,
                scope: "Project Ops pane list/detail state and work-item activity rendered from local PM projections",
                truth_rule: "Use while Project Ops is reading replay-safe local PM projection documents keyed by canonical PM stream ids",
            },
            ProjectOpsSourceBadgeRule {
                badge: PROJECT_OPS_SYNC_LIFECYCLE_SOURCE_BADGE,
                scope: "Sync/bootstrap diagnostics such as lifecycle state, grant failures, rebootstrap status, and checkpoint hydration telemetry",
                truth_rule: "Use only for sync lifecycle health or bootstrap state; do not label PM work-item values as live Spacetime authority in Phase 1",
            },
        ],
        required_stream_grants: project_ops_required_stream_grants(),
        checkpoint_rules: project_ops_required_stream_grants()
            .into_iter()
            .map(|stream_id| ProjectOpsCheckpointRule {
                stream_id,
                duplicate_policy: "drop duplicate seq <= local checkpoint",
                out_of_order_policy: "surface out_of_order and require deterministic rebootstrap or rewind before apply continues",
                stale_cursor_policy: "resume from max(local_checkpoint, remote_head - stale_clamp_window)",
                remote_checkpoint_policy: "adopt newer remote checkpoint only when it advances the local checkpoint",
            })
            .collect(),
    }
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
        project_ops_phase1_sync_contract, project_ops_required_stream_grants,
        project_ops_v1_contract_manifest, step0_stream_specs, ProjectOpsAcceptedEvent,
        ProjectOpsAcceptedEventEnvelope, ProjectOpsAcceptedEventName, ProjectOpsActor,
        ProjectOpsCommand, ProjectOpsCommandEnvelope, ProjectOpsCommandId, ProjectOpsCommandName,
        ProjectOpsCreateWorkItem, ProjectOpsDeliveryPhase, ProjectOpsEditWorkItemFields,
        ProjectOpsEditWorkItemFieldsPatch, ProjectOpsEntityKind, ProjectOpsErrorCode,
        ProjectOpsSetBlockedReason,
        ProjectOpsWorkItemArchived, ProjectOpsWorkItemAssigned, ProjectOpsWorkItemDraft,
        ProjectOpsWorkItemRef, PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID,
        PROJECT_OPS_CYCLES_STREAM_ID, PROJECT_OPS_PRIMARY_SOURCE_BADGE,
        PROJECT_OPS_SAVED_VIEWS_STREAM_ID, PROJECT_OPS_SYNC_LIFECYCLE_SOURCE_BADGE,
        PROJECT_OPS_V1_CONTRACT_VERSION, PROJECT_OPS_WORK_ITEMS_STREAM_ID,
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
    fn v1_contract_manifest_freezes_entities_commands_events_and_streams() {
        let manifest = project_ops_v1_contract_manifest();
        assert_eq!(manifest.contract_version, PROJECT_OPS_V1_CONTRACT_VERSION);
        assert_eq!(
            manifest.command_envelope_fields,
            vec![
                "command_id",
                "issued_at_unix_ms",
                "actor.actor_id|actor.actor_label",
                "command",
            ]
        );
        assert_eq!(
            manifest.accepted_event_envelope_fields,
            vec![
                "stream_id",
                "seq",
                "command_id",
                "emitted_at_unix_ms",
                "actor.actor_id|actor.actor_label",
            ]
        );
        assert_eq!(manifest.workflow, ProjectOpsWorkItemStatus::workflow().to_vec());
        assert_eq!(manifest.priorities, ProjectOpsPriority::all().to_vec());
        assert_eq!(manifest.commands.len(), ProjectOpsCommandName::all().len());
        assert_eq!(
            manifest.accepted_events.len(),
            ProjectOpsAcceptedEventName::all().len()
        );
        assert_eq!(manifest.streams, step0_stream_specs().to_vec());
        assert_eq!(manifest.error_codes, ProjectOpsErrorCode::all().to_vec());
    }

    #[test]
    fn v1_contract_manifest_keeps_step0_entities_and_deferred_placeholders() {
        let manifest = project_ops_v1_contract_manifest();
        let step0_entities = manifest
            .entities
            .iter()
            .filter(|entity| entity.step0_required)
            .map(|entity| entity.entity_kind)
            .collect::<Vec<_>>();
        assert_eq!(
            step0_entities,
            vec![
                ProjectOpsEntityKind::WorkItem,
                ProjectOpsEntityKind::Cycle,
                ProjectOpsEntityKind::SavedView,
                ProjectOpsEntityKind::ActivityEvent,
            ]
        );

        let deferred = manifest
            .entities
            .iter()
            .filter_map(|entity| entity.deferred_until_phase.map(|phase| (entity.entity_kind, phase)))
            .collect::<Vec<_>>();
        assert_eq!(
            deferred,
            vec![
                (ProjectOpsEntityKind::Comment, ProjectOpsDeliveryPhase::Phase3),
                (ProjectOpsEntityKind::Team, ProjectOpsDeliveryPhase::Phase3),
                (ProjectOpsEntityKind::Project, ProjectOpsDeliveryPhase::Phase3),
                (ProjectOpsEntityKind::AgentTask, ProjectOpsDeliveryPhase::Phase4),
                (ProjectOpsEntityKind::Bounty, ProjectOpsDeliveryPhase::Phase5),
            ]
        );
    }

    #[test]
    fn phase1_sync_contract_keeps_badges_truthful_and_grants_complete() {
        let contract = project_ops_phase1_sync_contract();
        assert_eq!(contract.current_rollout_phase, "phase_1_mirror_proxy");
        assert_eq!(
            contract.required_stream_grants,
            project_ops_required_stream_grants()
        );
        assert_eq!(contract.source_badges.len(), 2);
        assert_eq!(contract.source_badges[0].badge, PROJECT_OPS_PRIMARY_SOURCE_BADGE);
        assert_eq!(
            contract.source_badges[1].badge,
            PROJECT_OPS_SYNC_LIFECYCLE_SOURCE_BADGE
        );
        assert!(contract.source_badges[0]
            .truth_rule
            .contains("local PM projection documents"));
        assert!(contract.source_badges[1]
            .truth_rule
            .contains("do not label PM work-item values as live Spacetime authority"));
    }

    #[test]
    fn phase1_sync_contract_applies_checkpoint_rules_to_every_pm_stream() {
        let contract = project_ops_phase1_sync_contract();
        let stream_ids = contract
            .checkpoint_rules
            .iter()
            .map(|rule| rule.stream_id)
            .collect::<Vec<_>>();
        assert_eq!(stream_ids, project_ops_required_stream_grants());
        assert!(contract
            .checkpoint_rules
            .iter()
            .all(|rule| rule.duplicate_policy == "drop duplicate seq <= local checkpoint"));
        assert!(contract
            .checkpoint_rules
            .iter()
            .all(|rule| rule.out_of_order_policy.contains("rebootstrap")));
        assert!(contract
            .checkpoint_rules
            .iter()
            .all(|rule| rule.stale_cursor_policy.contains("stale_clamp_window")));
        assert!(contract
            .checkpoint_rules
            .iter()
            .all(|rule| rule.remote_checkpoint_policy.contains("adopt newer remote checkpoint")));
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
