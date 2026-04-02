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
pub const PROJECT_OPS_PROMOTION_LEDGER_STREAM_ID: &str = "stream.pm.promotion_ledger.v1";
pub const PROJECT_OPS_ROLLBACK_HISTORY_STREAM_ID: &str = "stream.pm.rollback_history.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ProjectOpsStreamSpec {
    pub stream_id: &'static str,
    pub purpose: &'static str,
    pub projection_payload: &'static str,
    pub projection_responsibility: &'static str,
}

const STEP0_STREAM_SPECS: [ProjectOpsStreamSpec; 6] = [
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
    ProjectOpsStreamSpec {
        stream_id: PROJECT_OPS_PROMOTION_LEDGER_STREAM_ID,
        purpose: "Promotion ledger for admitted improvements",
        projection_payload: "Vec<PromotionLedgerEntry>",
        projection_responsibility: "Promotion ledger for admitted improvements",
    },
    ProjectOpsStreamSpec {
        stream_id: PROJECT_OPS_ROLLBACK_HISTORY_STREAM_ID,
        purpose: "Rollback history for admitted improvements",
        projection_payload: "Vec<RollbackHistoryEntry>",
        projection_responsibility: "Rollback history for admitted improvements",
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
    PromotionLedger,
    RollbackHistory,
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum ProjectOpsCommandName {
    CreateWorkItem,
    UpdateWorkItem,
    DeleteWorkItem,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum ProjectOpsAcceptedEventName {
    WorkItemCreated,
    WorkItemUpdated,
    WorkItemDeleted,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum ProjectOpsErrorCode {
    InvalidCommand,
    InvalidEvent,
    UnknownEntity,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProjectOpsActivityRow {
    pub id: ProjectOpsWorkItemId,
    pub event_name: ProjectOpsAcceptedEventName,
    pub payload: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProjectOpsCycleRow {
    pub id: ProjectOpsCycleId,
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProjectOpsSavedViewRow {
    pub id: String,
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PromotionLedgerEntry {
    pub id: String,
    pub work_item_id: ProjectOpsWorkItemId,
    pub promotion_date: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RollbackHistoryEntry {
    pub id: String,
    pub work_item_id: ProjectOpsWorkItemId,
    pub rollback_date: String,
}