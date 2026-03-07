use crate::app_state::PaneLoadState;

pub mod contract;
pub mod projection;
pub mod schema;
pub mod service;
pub mod views;

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

#[allow(unused_imports)]
pub use service::{ProjectOpsCommandApplyResult, ProjectOpsCommandResult, ProjectOpsService};

#[allow(unused_imports)]
pub use views::{
    builtin_saved_view_specs, current_operator_label, empty_state_copy_for_view,
    filter_chips_for_view, filter_work_items_for_view, view_title_for_id,
    ProjectOpsBuiltinSavedViewSpec, PROJECT_OPS_BLOCKED_VIEW_ID,
    PROJECT_OPS_CURRENT_CYCLE_VIEW_ID, PROJECT_OPS_DEFAULT_VIEW_ID,
    PROJECT_OPS_MY_WORK_VIEW_ID, PROJECT_OPS_RECENTLY_UPDATED_VIEW_ID,
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
    pub operator_label: String,
    pub active_saved_view_id: String,
    pub active_saved_view: String,
    pub search_query: String,
    pub active_filter_chips: Vec<String>,
    pub visible_work_items: Vec<ProjectOpsWorkItem>,
    pub selected_work_item_id: Option<ProjectOpsWorkItemId>,
    pub empty_state_copy: String,
    pub available_saved_views: Vec<ProjectOpsSavedViewRow>,
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
        let operator_label = current_operator_label();
        let active_saved_view_id = PROJECT_OPS_DEFAULT_VIEW_ID.to_string();
        let active_saved_view = view_title_for_id(active_saved_view_id.as_str())
            .unwrap_or("My Work")
            .to_string();
        let search_query = String::new();
        let active_filter_chips =
            filter_chips_for_view(active_saved_view_id.as_str(), search_query.as_str());
        let visible_work_items = if feature_enabled {
            filter_work_items_for_view(
                local_store.work_items.as_slice(),
                local_store.cycles.as_slice(),
                active_saved_view_id.as_str(),
                operator_label.as_str(),
                search_query.as_str(),
            )
        } else {
            Vec::new()
        };
        let selected_work_item_id = visible_work_items
            .first()
            .map(|item| item.work_item_id.clone());
        let empty_state_copy =
            empty_state_copy_for_view(active_saved_view_id.as_str()).to_string();
        let available_saved_views = if feature_enabled {
            local_store.saved_views.clone()
        } else {
            Vec::new()
        };
        let (load_state, last_error, last_action, source_badge, summary, status_note) =
            if feature_enabled {
                (
                    local_store.load_state,
                    local_store.last_error.clone(),
                    local_store.last_action.clone(),
                    local_store.source_badge(),
                    format!(
                        "Step 0 PM projections ready with {} total work items, {} visible in {}, {} activity rows, {} cycles, and {} saved views (schema v{}).",
                        local_store.work_items.len(),
                        visible_work_items.len(),
                        active_saved_view,
                        local_store.activity_rows.len(),
                        local_store.cycles.len(),
                        local_store.saved_views.len(),
                        PROJECT_OPS_PROJECTION_SCHEMA_VERSION,
                    ),
                    format!(
                        "Step 0 schema is frozen with workflow {} and priorities {}. PM streams are registered as {}, {}, {}, and {} with local projection documents, shared checkpoint rows, and a deterministic reducer/service loop ready for replay-safe apply.",
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
            operator_label,
            active_saved_view_id,
            active_saved_view,
            search_query,
            active_filter_chips,
            visible_work_items,
            selected_work_item_id,
            empty_state_copy,
            available_saved_views,
            source_badge,
            summary,
            status_note,
            local_store,
        }
    }
}
