use crate::app_state::PaneLoadState;

pub mod contract;
pub mod projection;
pub mod schema;

pub const PROJECT_OPS_FEATURE_ENV: &str = "OPENAGENTS_ENABLE_PROJECT_OPS";
pub const PROJECT_OPS_SOURCE_BADGE: &str = "source: local";

#[allow(unused_imports)]
pub use contract::{
    step0_stream_specs, ProjectOpsAcceptedEvent, ProjectOpsAcceptedEventEnvelope,
    ProjectOpsAcceptedEventName, ProjectOpsActor, ProjectOpsCommand, ProjectOpsCommandEnvelope,
    ProjectOpsCommandId, ProjectOpsCommandName, ProjectOpsEditWorkItemFieldsPatch,
    ProjectOpsStreamSpec, PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID,
    PROJECT_OPS_CYCLES_STREAM_ID, PROJECT_OPS_SAVED_VIEWS_STREAM_ID,
    PROJECT_OPS_WORK_ITEMS_STREAM_ID,
};

#[allow(unused_imports)]
pub use schema::{
    PROJECT_OPS_STEP0_SCHEMA_VERSION, ProjectOpsCycleId, ProjectOpsPriority,
    ProjectOpsTeamKey, ProjectOpsWorkItem, ProjectOpsWorkItemId, ProjectOpsWorkItemStatus,
};

#[allow(unused_imports)]
pub use projection::{
    ProjectOpsActivityRow, ProjectOpsCycleRow, ProjectOpsProjectionStore, ProjectOpsSavedViewRow,
    PROJECT_OPS_PROJECTION_SCHEMA_VERSION,
};

pub fn project_ops_enabled_from_env() -> bool {
    std::env::var(PROJECT_OPS_FEATURE_ENV)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

pub struct ProjectOpsPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub feature_enabled: bool,
    pub active_saved_view: String,
    pub source_badge: String,
    pub summary: String,
    pub status_note: String,
    pub local_store: ProjectOpsProjectionStore,
}

impl Default for ProjectOpsPaneState {
    fn default() -> Self {
        let feature_enabled = project_ops_enabled_from_env();
        let local_store = if feature_enabled {
            ProjectOpsProjectionStore::load_or_bootstrap_default()
        } else {
            ProjectOpsProjectionStore::disabled()
        };
        let (load_state, last_error, last_action, source_badge, summary, status_note) =
            if feature_enabled {
                (
                    local_store.load_state,
                    local_store.last_error.clone(),
                    local_store.last_action.clone(),
                    local_store.source_badge(),
                    format!(
                        "Step 0 PM projections ready with {} work items, {} activity rows, {} cycles, and {} saved views (schema v{}).",
                        local_store.work_items.len(),
                        local_store.activity_rows.len(),
                        local_store.cycles.len(),
                        local_store.saved_views.len(),
                        PROJECT_OPS_PROJECTION_SCHEMA_VERSION,
                    ),
                    format!(
                        "Step 0 schema is frozen with workflow {} and priorities {}. PM streams are registered as {}, {}, {}, and {} with local projection documents and shared checkpoint rows ready for replay-safe apply.",
                        ProjectOpsWorkItemStatus::workflow_summary(),
                        ProjectOpsPriority::summary(),
                        PROJECT_OPS_WORK_ITEMS_STREAM_ID,
                        PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID,
                        PROJECT_OPS_CYCLES_STREAM_ID,
                        PROJECT_OPS_SAVED_VIEWS_STREAM_ID,
                    ),
                )
            } else {
                (
                    PaneLoadState::Loading,
                    None,
                    Some(format!(
                        "Project Ops disabled (set {}=1 to enable)",
                        PROJECT_OPS_FEATURE_ENV
                    )),
                    PROJECT_OPS_SOURCE_BADGE.to_string(),
                    "Project Ops is feature-gated off by default.".to_string(),
                    "Enable the gate to expose the native PM shell in the desktop pane registry and command palette."
                        .to_string(),
                )
            };

        let last_action = last_action.or_else(|| {
            feature_enabled.then(|| "Project Ops shell ready behind project_ops feature gate".to_string())
        });

        Self {
            load_state,
            last_error,
            last_action,
            feature_enabled,
            active_saved_view: "My Work".to_string(),
            source_badge,
            summary,
            status_note,
            local_store,
        }
    }
}
