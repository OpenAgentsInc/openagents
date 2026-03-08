use crate::app_state::PaneLoadState;

pub mod contract;
pub mod editor;
pub mod pilot;
pub mod projection;
pub mod schema;
pub mod service;
pub mod views;

pub const PROJECT_OPS_FEATURE_ENV: &str = "OPENAGENTS_ENABLE_PROJECT_OPS";
pub const PROJECT_OPS_SOURCE_BADGE: &str = contract::PROJECT_OPS_PRIMARY_SOURCE_BADGE;

#[allow(unused_imports)]
pub use contract::{
    PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID, PROJECT_OPS_CYCLES_STREAM_ID,
    PROJECT_OPS_PRIMARY_SOURCE_BADGE, PROJECT_OPS_SAVED_VIEWS_STREAM_ID,
    PROJECT_OPS_SYNC_LIFECYCLE_SOURCE_BADGE, PROJECT_OPS_V1_CONTRACT_VERSION,
    PROJECT_OPS_WORK_ITEMS_STREAM_ID, ProjectOpsAcceptedEvent, ProjectOpsAcceptedEventContractSpec,
    ProjectOpsAcceptedEventEnvelope, ProjectOpsAcceptedEventName, ProjectOpsActor,
    ProjectOpsCheckpointRule, ProjectOpsCommand, ProjectOpsCommandContractSpec,
    ProjectOpsCommandEnvelope, ProjectOpsCommandId, ProjectOpsCommandName,
    ProjectOpsContractManifest, ProjectOpsDeliveryPhase, ProjectOpsEditWorkItemFieldsPatch,
    ProjectOpsEntityContractSpec, ProjectOpsEntityKind, ProjectOpsErrorCode,
    ProjectOpsSourceBadgeRule, ProjectOpsStreamSpec, ProjectOpsSyncContract, project_ops_error,
    project_ops_phase1_sync_contract, project_ops_required_stream_grants,
    project_ops_v1_contract_manifest, step0_stream_specs,
};

#[allow(unused_imports)]
pub use schema::{
    PROJECT_OPS_STEP0_SCHEMA_VERSION, ProjectOpsCycleId, ProjectOpsPriority, ProjectOpsTeamKey,
    ProjectOpsWorkItem, ProjectOpsWorkItemId, ProjectOpsWorkItemStatus,
};

#[allow(unused_imports)]
pub use projection::{
    PROJECT_OPS_PROJECTION_SCHEMA_VERSION, ProjectOpsActivityRow, ProjectOpsCycleRow,
    ProjectOpsProjectionStore, ProjectOpsSavedViewRow,
};

#[allow(unused_imports)]
pub use editor::{ProjectOpsDetailDraft, ProjectOpsQuickCreateDraft};

#[allow(unused_imports)]
pub use service::{ProjectOpsCommandApplyResult, ProjectOpsCommandResult, ProjectOpsService};

#[allow(unused_imports)]
pub use pilot::{PROJECT_OPS_PILOT_METRICS_SCHEMA_VERSION, ProjectOpsPilotMetricsState};

#[allow(unused_imports)]
pub use views::{
    PROJECT_OPS_BLOCKED_VIEW_ID, PROJECT_OPS_CURRENT_CYCLE_VIEW_ID, PROJECT_OPS_DEFAULT_VIEW_ID,
    PROJECT_OPS_MY_WORK_VIEW_ID, PROJECT_OPS_RECENTLY_UPDATED_VIEW_ID, ProjectOpsBoardLane,
    ProjectOpsBuiltinSavedViewSpec, builtin_saved_view_specs, current_operator_label,
    empty_state_copy_for_view, filter_chips_for_view, filter_work_items_for_view,
    project_board_lanes, view_title_for_id,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectOpsPresentationMode {
    List,
    Board,
}

impl ProjectOpsPresentationMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::List => "list",
            Self::Board => "board",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectOpsSyncStreamStatus {
    pub stream_id: String,
    pub granted: bool,
    pub checkpoint_seq: u64,
    pub resume_cursor_seq: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectOpsSyncDiagnostics {
    pub source_badge: String,
    pub bootstrap_state: String,
    pub bootstrap_note: Option<String>,
    pub bootstrap_error: Option<String>,
    pub lifecycle_state: Option<String>,
    pub last_disconnect_reason: Option<String>,
    pub stale_cursor_recovery_required: bool,
    pub replay_cursor_seq: Option<u64>,
    pub replay_target_seq: Option<u64>,
    pub required_stream_grants: Vec<String>,
    pub granted_stream_grants: Vec<String>,
    pub missing_stream_grants: Vec<String>,
    pub streams: Vec<ProjectOpsSyncStreamStatus>,
}

impl Default for ProjectOpsSyncDiagnostics {
    fn default() -> Self {
        Self {
            source_badge: PROJECT_OPS_SYNC_LIFECYCLE_SOURCE_BADGE.to_string(),
            bootstrap_state: "idle".to_string(),
            bootstrap_note: None,
            bootstrap_error: None,
            lifecycle_state: None,
            last_disconnect_reason: None,
            stale_cursor_recovery_required: false,
            replay_cursor_seq: None,
            replay_target_seq: None,
            required_stream_grants: project_ops_required_stream_grants()
                .into_iter()
                .map(ToString::to_string)
                .collect(),
            granted_stream_grants: Vec::new(),
            missing_stream_grants: project_ops_required_stream_grants()
                .into_iter()
                .map(ToString::to_string)
                .collect(),
            streams: project_ops_required_stream_grants()
                .into_iter()
                .map(|stream_id| ProjectOpsSyncStreamStatus {
                    stream_id: stream_id.to_string(),
                    granted: false,
                    checkpoint_seq: 0,
                    resume_cursor_seq: 0,
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectOpsBoardDragState {
    pub work_item_id: ProjectOpsWorkItemId,
    pub from_status: ProjectOpsWorkItemStatus,
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
    pub presentation_mode: ProjectOpsPresentationMode,
    pub board_lanes: Vec<ProjectOpsBoardLane>,
    pub board_drag_state: Option<ProjectOpsBoardDragState>,
    pub selected_work_item_id: Option<ProjectOpsWorkItemId>,
    pub empty_state_copy: String,
    pub visible_activity_rows: Vec<ProjectOpsActivityRow>,
    pub activity_empty_state: String,
    pub selection_notice: Option<String>,
    pub available_saved_views: Vec<ProjectOpsSavedViewRow>,
    pub quick_create_draft: ProjectOpsQuickCreateDraft,
    pub detail_draft: Option<ProjectOpsDetailDraft>,
    pub detail_save_status: Option<String>,
    pub pilot_metrics: ProjectOpsPilotMetricsState,
    pub source_badge: String,
    pub summary: String,
    pub status_note: String,
    pub sync_diagnostics: ProjectOpsSyncDiagnostics,
    pub local_store: ProjectOpsProjectionStore,
}

impl Default for ProjectOpsPaneState {
    fn default() -> Self {
        let feature_enabled = project_ops_enabled_from_env();
        let active_saved_view_id = PROJECT_OPS_DEFAULT_VIEW_ID.to_string();
        let projection_load_started_at = std::time::Instant::now();
        let local_store = if feature_enabled {
            ProjectOpsProjectionStore::load_or_bootstrap_default()
        } else {
            ProjectOpsProjectionStore::disabled()
        };
        let mut pilot_metrics = if feature_enabled {
            ProjectOpsPilotMetricsState::load_or_new_default()
                .unwrap_or_else(|_| ProjectOpsPilotMetricsState::disabled())
        } else {
            ProjectOpsPilotMetricsState::disabled()
        };
        if feature_enabled {
            let _ = pilot_metrics.record_projection_rebuild(
                projection_load_started_at.elapsed().as_millis() as u64,
                local_store.max_checkpoint_seq(),
            );
            let _ = pilot_metrics.record_view(active_saved_view_id.as_str());
        }
        let operator_label = current_operator_label();
        let search_query = String::new();
        let (
            active_saved_view,
            active_filter_chips,
            visible_work_items,
            selected_work_item_id,
            empty_state_copy,
            visible_activity_rows,
            activity_empty_state,
            selection_notice,
            available_saved_views,
        ) = if feature_enabled {
            derive_project_ops_view_state(
                &local_store,
                active_saved_view_id.as_str(),
                operator_label.as_str(),
                search_query.as_str(),
                None,
            )
        } else {
            (
                "My Work".to_string(),
                Vec::new(),
                Vec::new(),
                None,
                empty_state_copy_for_view(active_saved_view_id.as_str()).to_string(),
                Vec::new(),
                "Select a work item to inspect activity.".to_string(),
                None,
                Vec::new(),
            )
        };
        let contract_manifest = project_ops_v1_contract_manifest();
        let sync_contract = project_ops_phase1_sync_contract();
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
                        "Step 0 schema v{} and PM contract {} are frozen with workflow {} and priorities {} across {} entities, {} commands, {} accepted events, and {} canonical PM streams. PM streams are registered as {}, {}, {}, and {} with local projection documents, shared checkpoint rows, source badge {}, sync badge {}, and a reserved grant set of {} streams for later bootstrap wiring.",
                        PROJECT_OPS_STEP0_SCHEMA_VERSION,
                        PROJECT_OPS_V1_CONTRACT_VERSION,
                        ProjectOpsWorkItemStatus::workflow_summary(),
                        ProjectOpsPriority::summary(),
                        contract_manifest.entities.len(),
                        contract_manifest.commands.len(),
                        contract_manifest.accepted_events.len(),
                        contract_manifest.streams.len(),
                        PROJECT_OPS_WORK_ITEMS_STREAM_ID,
                        PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID,
                        PROJECT_OPS_CYCLES_STREAM_ID,
                        PROJECT_OPS_SAVED_VIEWS_STREAM_ID,
                        PROJECT_OPS_PRIMARY_SOURCE_BADGE,
                        PROJECT_OPS_SYNC_LIFECYCLE_SOURCE_BADGE,
                        sync_contract.required_stream_grants.len(),
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
            feature_enabled
                .then(|| "Project Ops shell ready behind project_ops feature gate".to_string())
        });
        let detail_draft = selected_work_item_id
            .as_ref()
            .and_then(|selected| {
                visible_work_items
                    .iter()
                    .find(|item| &item.work_item_id == selected)
                    .cloned()
            })
            .map(|item| ProjectOpsDetailDraft::from_work_item(&item));
        let board_lanes = project_board_lanes(visible_work_items.as_slice());

        let mut state = Self {
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
            presentation_mode: ProjectOpsPresentationMode::List,
            board_lanes,
            board_drag_state: None,
            selected_work_item_id,
            empty_state_copy,
            visible_activity_rows,
            activity_empty_state,
            selection_notice,
            available_saved_views,
            quick_create_draft: ProjectOpsQuickCreateDraft::default(),
            detail_draft,
            detail_save_status: None,
            pilot_metrics,
            source_badge,
            summary,
            status_note,
            sync_diagnostics: ProjectOpsSyncDiagnostics::default(),
            local_store,
        };
        if feature_enabled {
            state.sync_runtime_contract_state(None, None, &[], None);
        }
        state
    }
}

impl ProjectOpsPaneState {
    #[cfg(test)]
    pub(crate) fn from_local_store_for_tests(
        local_store: ProjectOpsProjectionStore,
        operator_label: &str,
    ) -> Self {
        let active_saved_view_id = PROJECT_OPS_DEFAULT_VIEW_ID.to_string();
        let search_query = String::new();
        let (
            active_saved_view,
            active_filter_chips,
            visible_work_items,
            selected_work_item_id,
            empty_state_copy,
            visible_activity_rows,
            activity_empty_state,
            selection_notice,
            available_saved_views,
        ) = derive_project_ops_view_state(
            &local_store,
            active_saved_view_id.as_str(),
            operator_label,
            search_query.as_str(),
            None,
        );
        let detail_draft = selected_work_item_id
            .as_ref()
            .and_then(|selected| {
                visible_work_items
                    .iter()
                    .find(|item| &item.work_item_id == selected)
                    .cloned()
            })
            .map(|item| ProjectOpsDetailDraft::from_work_item(&item));
        let board_lanes = project_board_lanes(visible_work_items.as_slice());
        let mut state = Self {
            load_state: local_store.load_state,
            last_error: local_store.last_error.clone(),
            last_action: local_store.last_action.clone(),
            feature_enabled: true,
            operator_label: operator_label.to_string(),
            active_saved_view_id,
            active_saved_view,
            search_query,
            active_filter_chips,
            visible_work_items,
            presentation_mode: ProjectOpsPresentationMode::List,
            board_lanes,
            board_drag_state: None,
            selected_work_item_id,
            empty_state_copy,
            visible_activity_rows,
            activity_empty_state,
            selection_notice,
            available_saved_views,
            quick_create_draft: ProjectOpsQuickCreateDraft::default(),
            detail_draft,
            detail_save_status: None,
            pilot_metrics: ProjectOpsPilotMetricsState::disabled(),
            source_badge: local_store.source_badge(),
            summary: "Test PM pane".to_string(),
            status_note: "Test PM pane".to_string(),
            sync_diagnostics: ProjectOpsSyncDiagnostics::default(),
            local_store,
        };
        state.sync_runtime_contract_state(None, None, &[], None);
        state
    }

    pub fn selected_work_item(&self) -> Option<&ProjectOpsWorkItem> {
        let selected_work_item_id = self.selected_work_item_id.as_ref()?;
        self.visible_work_items
            .iter()
            .find(|item| &item.work_item_id == selected_work_item_id)
    }

    pub fn set_active_saved_view(&mut self, view_id: &str) -> bool {
        let normalized = view_id.trim();
        if normalized.is_empty() || normalized == self.active_saved_view_id {
            return false;
        }
        self.active_saved_view_id = normalized.to_string();
        self.refresh_derived_view_state();
        let _ = self
            .pilot_metrics
            .record_view(self.active_saved_view_id.as_str());
        self.last_action = Some(format!("Project Ops view -> {}", self.active_saved_view));
        true
    }

    pub fn set_search_query(&mut self, query: &str) -> bool {
        let normalized = query.trim().to_string();
        if normalized == self.search_query {
            return false;
        }
        self.search_query = normalized;
        self.refresh_derived_view_state();
        self.last_action = Some(if self.search_query.is_empty() {
            "Project Ops search cleared".to_string()
        } else {
            format!("Project Ops search -> {}", self.search_query)
        });
        true
    }

    pub fn select_visible_row(&mut self, index: usize) -> bool {
        let Some(work_item_id) = self
            .visible_work_items
            .get(index)
            .map(|item| item.work_item_id.clone())
        else {
            return false;
        };
        if self.selected_work_item_id.as_ref() == Some(&work_item_id) {
            return false;
        }
        self.selected_work_item_id = Some(work_item_id.clone());
        self.sync_detail_draft_from_selection();
        self.last_action = Some(format!("Selected {}", work_item_id.as_str()));
        true
    }

    pub fn move_selection(&mut self, delta: isize) -> bool {
        if self.visible_work_items.is_empty() {
            self.selected_work_item_id = None;
            return false;
        }
        let current_index = self
            .selected_work_item_id
            .as_ref()
            .and_then(|selected| {
                self.visible_work_items
                    .iter()
                    .position(|item| &item.work_item_id == selected)
            })
            .unwrap_or(0);
        let max_index = self.visible_work_items.len().saturating_sub(1) as isize;
        let next_index = ((current_index as isize) + delta).clamp(0, max_index) as usize;
        self.select_visible_row(next_index)
    }

    pub fn set_presentation_mode(&mut self, presentation_mode: ProjectOpsPresentationMode) -> bool {
        if self.presentation_mode == presentation_mode {
            return false;
        }
        self.presentation_mode = presentation_mode;
        if presentation_mode == ProjectOpsPresentationMode::List {
            self.board_drag_state = None;
        }
        self.last_action = Some(format!(
            "Project Ops presentation -> {}",
            presentation_mode.label()
        ));
        true
    }

    pub fn start_board_drag(&mut self, work_item_id: &str) -> bool {
        let Some((dragged_work_item_id, from_status)) = self
            .visible_work_items
            .iter()
            .find(|item| item.work_item_id.as_str() == work_item_id.trim())
            .map(|item| (item.work_item_id.clone(), item.status))
        else {
            return false;
        };
        self.presentation_mode = ProjectOpsPresentationMode::Board;
        self.selected_work_item_id = Some(dragged_work_item_id.clone());
        self.sync_detail_draft_from_selection();
        self.board_drag_state = Some(ProjectOpsBoardDragState {
            work_item_id: dragged_work_item_id.clone(),
            from_status,
        });
        self.last_action = Some(format!(
            "Board drag {} from {}",
            dragged_work_item_id.as_str(),
            from_status.label()
        ));
        true
    }

    pub fn drop_board_drag(
        &mut self,
        target_status: ProjectOpsWorkItemStatus,
    ) -> Result<bool, String> {
        let Some(drag_state) = self.board_drag_state.clone() else {
            return Ok(false);
        };
        if drag_state.from_status == target_status {
            self.board_drag_state = None;
            self.last_action = Some(format!(
                "Board drag cleared for {}",
                drag_state.work_item_id.as_str()
            ));
            return Ok(false);
        }
        let moved = self.move_work_item_to_status(&drag_state.work_item_id, target_status)?;
        if moved {
            self.board_drag_state = None;
            self.presentation_mode = ProjectOpsPresentationMode::Board;
            self.last_action = Some(format!(
                "Board move {} -> {}",
                drag_state.work_item_id.as_str(),
                target_status.label()
            ));
            self.detail_save_status = Some("Board move applied".to_string());
        }
        Ok(moved)
    }

    fn refresh_derived_view_state(&mut self) {
        let (
            active_saved_view,
            active_filter_chips,
            visible_work_items,
            selected_work_item_id,
            empty_state_copy,
            visible_activity_rows,
            activity_empty_state,
            selection_notice,
            available_saved_views,
        ) = derive_project_ops_view_state(
            &self.local_store,
            self.active_saved_view_id.as_str(),
            self.operator_label.as_str(),
            self.search_query.as_str(),
            self.selected_work_item_id.as_ref(),
        );
        self.active_saved_view = active_saved_view;
        self.active_filter_chips = active_filter_chips;
        self.visible_work_items = visible_work_items;
        self.board_lanes = project_board_lanes(self.visible_work_items.as_slice());
        if self.board_drag_state.as_ref().is_some_and(|drag| {
            !self.visible_work_items.iter().any(|item| {
                item.work_item_id == drag.work_item_id && item.status == drag.from_status
            })
        }) {
            self.board_drag_state = None;
        }
        self.selected_work_item_id = selected_work_item_id;
        self.empty_state_copy = empty_state_copy;
        self.visible_activity_rows = visible_activity_rows;
        self.activity_empty_state = activity_empty_state;
        self.selection_notice = selection_notice;
        self.available_saved_views = available_saved_views;
        self.load_state = self.local_store.load_state;
        self.last_error = self.local_store.last_error.clone();
        self.refresh_sync_diagnostic_stream_rows();
        self.sync_detail_draft_from_selection();
    }

    pub fn sync_runtime_contract_state(
        &mut self,
        sync_bootstrap_note: Option<&str>,
        sync_bootstrap_error: Option<&str>,
        granted_stream_grants: &[String],
        sync_lifecycle_snapshot: Option<&crate::sync_lifecycle::RuntimeSyncHealthSnapshot>,
    ) {
        if !self.feature_enabled {
            return;
        }
        if let Err(error) = self.local_store.reload_shared_checkpoints() {
            self.sync_diagnostics.bootstrap_state = "checkpoint_error".to_string();
            self.sync_diagnostics.bootstrap_error = Some(error);
            return;
        }

        self.sync_diagnostics.bootstrap_note = sync_bootstrap_note.map(ToString::to_string);
        self.sync_diagnostics.bootstrap_error = sync_bootstrap_error.map(ToString::to_string);
        self.sync_diagnostics.lifecycle_state =
            sync_lifecycle_snapshot.map(|snapshot| snapshot.state.as_str().to_string());
        self.sync_diagnostics.last_disconnect_reason =
            sync_lifecycle_snapshot.and_then(|snapshot| {
                snapshot
                    .last_disconnect_reason
                    .map(|reason| reason.as_str().to_string())
            });
        self.sync_diagnostics.stale_cursor_recovery_required =
            sync_lifecycle_snapshot.is_some_and(|snapshot| {
                snapshot.last_disconnect_reason
                    == Some(crate::sync_lifecycle::RuntimeSyncDisconnectReason::StaleCursor)
            });
        self.sync_diagnostics.replay_cursor_seq =
            sync_lifecycle_snapshot.and_then(|snapshot| snapshot.replay_cursor_seq);
        self.sync_diagnostics.replay_target_seq =
            sync_lifecycle_snapshot.and_then(|snapshot| snapshot.replay_target_seq);
        self.sync_diagnostics.granted_stream_grants = granted_stream_grants
            .iter()
            .filter(|grant| {
                project_ops_required_stream_grants()
                    .into_iter()
                    .any(|stream_id| {
                        crate::sync_bootstrap::stream_grant_allows(grant.as_str(), stream_id)
                    })
            })
            .cloned()
            .collect();
        self.sync_diagnostics.missing_stream_grants = project_ops_required_stream_grants()
            .into_iter()
            .filter(|stream_id| {
                !granted_stream_grants.iter().any(|grant| {
                    crate::sync_bootstrap::stream_grant_allows(grant.as_str(), stream_id)
                })
            })
            .map(ToString::to_string)
            .collect();

        self.sync_diagnostics.bootstrap_state = if sync_bootstrap_error.is_some() {
            "bootstrap_error".to_string()
        } else if self.sync_diagnostics.stale_cursor_recovery_required {
            "stale_cursor_recovery_required".to_string()
        } else if sync_bootstrap_note.is_some_and(|note| note.contains("disabled")) {
            "sync_disabled".to_string()
        } else if !granted_stream_grants.is_empty()
            && !self.sync_diagnostics.missing_stream_grants.is_empty()
        {
            "missing_grants".to_string()
        } else if let Some(snapshot) = sync_lifecycle_snapshot {
            snapshot.state.as_str().to_string()
        } else {
            "idle".to_string()
        };

        self.refresh_sync_diagnostic_stream_rows();
    }

    fn refresh_sync_diagnostic_stream_rows(&mut self) {
        self.sync_diagnostics.streams = project_ops_required_stream_grants()
            .into_iter()
            .map(|stream_id| {
                let checkpoint_seq = self.local_store.checkpoint_for(stream_id).unwrap_or(0);
                ProjectOpsSyncStreamStatus {
                    stream_id: stream_id.to_string(),
                    granted: self
                        .sync_diagnostics
                        .granted_stream_grants
                        .iter()
                        .any(|grant| crate::sync_bootstrap::stream_grant_allows(grant, stream_id)),
                    checkpoint_seq,
                    resume_cursor_seq: self
                        .local_store
                        .resume_cursor_for_stream(stream_id, Some(checkpoint_seq))
                        .unwrap_or(checkpoint_seq),
                }
            })
            .collect();
    }

    pub fn set_quick_create_title(&mut self, title: &str) {
        self.quick_create_draft.title = title.trim().to_string();
    }

    pub fn set_quick_create_description(&mut self, description: &str) {
        self.quick_create_draft.description = description.trim().to_string();
    }

    pub fn set_quick_create_priority(&mut self, priority: ProjectOpsPriority) {
        self.quick_create_draft.priority = priority;
    }

    pub fn set_quick_create_area_tags(&mut self, tags: &[&str]) {
        self.quick_create_draft.area_tags = editor::normalize_area_tags(tags.iter().copied());
    }

    pub fn set_quick_create_due_at(&mut self, due_at_unix_ms: Option<u64>) {
        self.quick_create_draft.due_at_unix_ms = due_at_unix_ms;
    }

    pub fn edit_detail_title(&mut self, title: &str) {
        if let Some(detail_draft) = self.detail_draft.as_mut() {
            detail_draft.title = title.trim().to_string();
            detail_draft.dirty = true;
        }
    }

    pub fn edit_detail_description(&mut self, description: &str) {
        if let Some(detail_draft) = self.detail_draft.as_mut() {
            detail_draft.description = description.trim().to_string();
            detail_draft.dirty = true;
        }
    }

    pub fn set_detail_status(&mut self, status: ProjectOpsWorkItemStatus) {
        if let Some(detail_draft) = self.detail_draft.as_mut() {
            detail_draft.status = status;
            detail_draft.dirty = true;
        }
    }

    pub fn set_detail_priority(&mut self, priority: ProjectOpsPriority) {
        if let Some(detail_draft) = self.detail_draft.as_mut() {
            detail_draft.priority = priority;
            detail_draft.dirty = true;
        }
    }

    pub fn set_detail_assignee(&mut self, assignee: Option<&str>) {
        if let Some(detail_draft) = self.detail_draft.as_mut() {
            detail_draft.assignee = assignee
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string);
            detail_draft.dirty = true;
        }
    }

    pub fn set_detail_cycle(&mut self, cycle_id: Option<ProjectOpsCycleId>) {
        if let Some(detail_draft) = self.detail_draft.as_mut() {
            detail_draft.cycle_id = cycle_id;
            detail_draft.dirty = true;
        }
    }

    pub fn set_detail_parent(&mut self, parent_id: Option<ProjectOpsWorkItemId>) {
        if let Some(detail_draft) = self.detail_draft.as_mut() {
            detail_draft.parent_id = parent_id;
            detail_draft.dirty = true;
        }
    }

    pub fn set_detail_area_tags(&mut self, tags: &[&str]) {
        if let Some(detail_draft) = self.detail_draft.as_mut() {
            detail_draft.area_tags = editor::normalize_area_tags(tags.iter().copied());
            detail_draft.dirty = true;
        }
    }

    pub fn set_detail_blocked_reason(&mut self, blocked_reason: Option<&str>) {
        if let Some(detail_draft) = self.detail_draft.as_mut() {
            detail_draft.blocked_reason = blocked_reason
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string);
            detail_draft.dirty = true;
        }
    }

    pub fn set_detail_due_at(&mut self, due_at_unix_ms: Option<u64>) {
        if let Some(detail_draft) = self.detail_draft.as_mut() {
            detail_draft.due_at_unix_ms = due_at_unix_ms;
            detail_draft.dirty = true;
        }
    }

    pub fn apply_quick_create(&mut self) -> Result<Option<ProjectOpsWorkItemId>, String> {
        if !self.feature_enabled {
            return Ok(None);
        }
        self.quick_create_draft
            .validate()
            .map_err(|error| project_ops_error(ProjectOpsErrorCode::InvalidCommand, error))?;
        let issued_at_unix_ms = now_unix_ms();
        let work_item_id = next_work_item_id(self.local_store.work_items.as_slice())?;
        let command = ProjectOpsCommandEnvelope {
            command_id: ProjectOpsCommandId::new(format!(
                "pm-create-{issued_at_unix_ms}-{}",
                work_item_id.as_str()
            ))?,
            issued_at_unix_ms,
            actor: ProjectOpsActor {
                actor_id: None,
                actor_label: Some(self.operator_label.clone()),
            },
            command: ProjectOpsCommand::CreateWorkItem(contract::ProjectOpsCreateWorkItem {
                draft: self
                    .quick_create_draft
                    .to_work_item_draft(work_item_id.clone()),
            }),
        };
        let result = ProjectOpsService::apply_command_to_store(&mut self.local_store, command)?;
        self.refresh_derived_view_state();
        let _ = self.pilot_metrics.record_command("CreateWorkItem");
        self.quick_create_draft = ProjectOpsQuickCreateDraft::default();
        self.detail_save_status = Some("Quick create applied".to_string());
        match result {
            ProjectOpsCommandResult::Applied(result) => {
                self.selected_work_item_id = Some(result.work_item_id.clone());
                self.sync_detail_draft_from_selection();
                Ok(Some(result.work_item_id))
            }
            ProjectOpsCommandResult::DuplicateCommand { .. } => Ok(None),
        }
    }

    pub fn apply_detail_draft(&mut self) -> Result<bool, String> {
        if !self.feature_enabled {
            return Ok(false);
        }
        let Some(detail_draft) = self.detail_draft.clone() else {
            return Ok(false);
        };
        let Some(current) = self
            .local_store
            .work_items
            .iter()
            .find(|item| item.work_item_id == detail_draft.work_item_id)
            .cloned()
        else {
            return Err(project_ops_error(
                ProjectOpsErrorCode::WorkItemMissing,
                format!(
                    "selected work item {} no longer exists",
                    detail_draft.work_item_id.as_str()
                ),
            ));
        };
        let issued_at_unix_ms = now_unix_ms();
        let mut command_counter = 0u64;
        let mut applied_any = false;
        let mut applied_command_names = Vec::new();

        let mut apply_command = |command: ProjectOpsCommand| -> Result<(), String> {
            let command_name = command.name().label().to_string();
            command_counter = command_counter.saturating_add(1);
            let envelope = ProjectOpsCommandEnvelope {
                command_id: ProjectOpsCommandId::new(format!(
                    "pm-edit-{issued_at_unix_ms}-{command_counter}"
                ))?,
                issued_at_unix_ms: issued_at_unix_ms.saturating_add(command_counter),
                actor: ProjectOpsActor {
                    actor_id: None,
                    actor_label: Some(self.operator_label.clone()),
                },
                command,
            };
            let result =
                ProjectOpsService::apply_command_to_store(&mut self.local_store, envelope)?;
            applied_any |= matches!(result, ProjectOpsCommandResult::Applied(_));
            applied_command_names.push(command_name);
            Ok(())
        };

        if detail_draft.title != current.title
            || detail_draft.description != current.description
            || detail_draft.priority != current.priority
            || detail_draft.due_at_unix_ms != current.due_at_unix_ms
            || detail_draft.area_tags != current.area_tags
        {
            apply_command(ProjectOpsCommand::EditWorkItemFields(
                contract::ProjectOpsEditWorkItemFields {
                    work_item_id: detail_draft.work_item_id.clone(),
                    patch: ProjectOpsEditWorkItemFieldsPatch {
                        title: (detail_draft.title != current.title)
                            .then(|| detail_draft.title.clone()),
                        description: (detail_draft.description != current.description)
                            .then(|| detail_draft.description.clone()),
                        priority: (detail_draft.priority != current.priority)
                            .then_some(detail_draft.priority),
                        due_at_unix_ms: (detail_draft.due_at_unix_ms != current.due_at_unix_ms)
                            .then_some(detail_draft.due_at_unix_ms),
                        area_tags: (detail_draft.area_tags != current.area_tags)
                            .then(|| detail_draft.area_tags.clone()),
                    },
                },
            ))?;
        }

        if detail_draft.status != current.status {
            apply_command(ProjectOpsCommand::ChangeWorkItemStatus(
                contract::ProjectOpsChangeWorkItemStatus {
                    work_item_id: detail_draft.work_item_id.clone(),
                    status: detail_draft.status,
                },
            ))?;
        }

        if detail_draft.assignee != current.assignee {
            if let Some(assignee) = detail_draft.assignee.clone() {
                apply_command(ProjectOpsCommand::AssignWorkItem(
                    contract::ProjectOpsAssignWorkItem {
                        work_item_id: detail_draft.work_item_id.clone(),
                        assignee,
                    },
                ))?;
            } else {
                apply_command(ProjectOpsCommand::ClearAssignee(
                    contract::ProjectOpsWorkItemRef {
                        work_item_id: detail_draft.work_item_id.clone(),
                    },
                ))?;
            }
        }

        if detail_draft.cycle_id != current.cycle_id {
            if let Some(cycle_id) = detail_draft.cycle_id.clone() {
                apply_command(ProjectOpsCommand::SetWorkItemCycle(
                    contract::ProjectOpsSetWorkItemCycle {
                        work_item_id: detail_draft.work_item_id.clone(),
                        cycle_id,
                    },
                ))?;
            } else {
                apply_command(ProjectOpsCommand::ClearWorkItemCycle(
                    contract::ProjectOpsWorkItemRef {
                        work_item_id: detail_draft.work_item_id.clone(),
                    },
                ))?;
            }
        }

        if detail_draft.parent_id != current.parent_id {
            if let Some(parent_id) = detail_draft.parent_id.clone() {
                apply_command(ProjectOpsCommand::SetParentWorkItem(
                    contract::ProjectOpsSetParentWorkItem {
                        work_item_id: detail_draft.work_item_id.clone(),
                        parent_id,
                    },
                ))?;
            } else {
                apply_command(ProjectOpsCommand::ClearParentWorkItem(
                    contract::ProjectOpsWorkItemRef {
                        work_item_id: detail_draft.work_item_id.clone(),
                    },
                ))?;
            }
        }

        if detail_draft.blocked_reason != current.blocked_reason {
            if let Some(blocked_reason) = detail_draft.blocked_reason.clone() {
                apply_command(ProjectOpsCommand::SetBlockedReason(
                    contract::ProjectOpsSetBlockedReason {
                        work_item_id: detail_draft.work_item_id.clone(),
                        blocked_reason,
                    },
                ))?;
            } else {
                apply_command(ProjectOpsCommand::ClearBlockedReason(
                    contract::ProjectOpsWorkItemRef {
                        work_item_id: detail_draft.work_item_id.clone(),
                    },
                ))?;
            }
        }

        self.refresh_derived_view_state();
        let command_labels = applied_command_names
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let _ = self
            .pilot_metrics
            .record_commands(command_labels.as_slice());
        self.detail_save_status = Some(if applied_any {
            "Detail changes applied".to_string()
        } else {
            "Detail draft had no changes".to_string()
        });
        Ok(applied_any)
    }

    fn move_work_item_to_status(
        &mut self,
        work_item_id: &ProjectOpsWorkItemId,
        target_status: ProjectOpsWorkItemStatus,
    ) -> Result<bool, String> {
        if !self.feature_enabled {
            return Ok(false);
        }
        let issued_at_unix_ms = now_unix_ms();
        let command = ProjectOpsCommandEnvelope {
            command_id: ProjectOpsCommandId::new(format!(
                "pm-board-status-{issued_at_unix_ms}-{}-{}",
                work_item_id.as_str(),
                target_status.label(),
            ))?,
            issued_at_unix_ms,
            actor: ProjectOpsActor {
                actor_id: None,
                actor_label: Some(self.operator_label.clone()),
            },
            command: ProjectOpsCommand::ChangeWorkItemStatus(
                contract::ProjectOpsChangeWorkItemStatus {
                    work_item_id: work_item_id.clone(),
                    status: target_status,
                },
            ),
        };
        let result = ProjectOpsService::apply_command_to_store(&mut self.local_store, command)?;
        self.refresh_derived_view_state();
        self.selected_work_item_id = Some(work_item_id.clone());
        self.sync_detail_draft_from_selection();
        let _ = self.pilot_metrics.record_command("ChangeWorkItemStatus");
        Ok(matches!(result, ProjectOpsCommandResult::Applied(_)))
    }

    fn sync_detail_draft_from_selection(&mut self) {
        let Some(selected) = self.selected_work_item_id.as_ref() else {
            self.detail_draft = None;
            return;
        };
        let Some(current) = self
            .visible_work_items
            .iter()
            .find(|item| &item.work_item_id == selected)
        else {
            self.detail_draft = None;
            return;
        };
        if self
            .detail_draft
            .as_ref()
            .is_some_and(|draft| draft.work_item_id == current.work_item_id && draft.dirty)
        {
            return;
        }
        self.detail_draft = Some(ProjectOpsDetailDraft::from_work_item(current));
    }
}

fn derive_project_ops_view_state(
    local_store: &ProjectOpsProjectionStore,
    active_saved_view_id: &str,
    operator_label: &str,
    search_query: &str,
    selected_work_item_id: Option<&ProjectOpsWorkItemId>,
) -> (
    String,
    Vec<String>,
    Vec<ProjectOpsWorkItem>,
    Option<ProjectOpsWorkItemId>,
    String,
    Vec<ProjectOpsActivityRow>,
    String,
    Option<String>,
    Vec<ProjectOpsSavedViewRow>,
) {
    let available_saved_views = local_store.saved_views.clone();
    let active_saved_view = available_saved_views
        .iter()
        .find(|view| view.view_id == active_saved_view_id)
        .map(|view| view.title.clone())
        .or_else(|| view_title_for_id(active_saved_view_id).map(ToString::to_string))
        .unwrap_or_else(|| "Saved View".to_string());
    let active_filter_chips = filter_chips_for_view(active_saved_view_id, search_query);
    let visible_work_items = filter_work_items_for_view(
        local_store.work_items.as_slice(),
        local_store.cycles.as_slice(),
        active_saved_view_id,
        operator_label,
        search_query,
    );
    let selection_notice = selected_work_item_id.and_then(|selected| {
        if visible_work_items
            .iter()
            .any(|item| item.work_item_id == *selected)
        {
            None
        } else if local_store
            .work_items
            .iter()
            .any(|item| item.work_item_id == *selected)
        {
            Some(format!(
                "Selected item {} is filtered out of {}.",
                selected.as_str(),
                active_saved_view
            ))
        } else {
            Some(format!(
                "Selected item {} no longer exists.",
                selected.as_str()
            ))
        }
    });
    let selected_work_item_id = selected_work_item_id
        .cloned()
        .filter(|selected| {
            visible_work_items
                .iter()
                .any(|item| item.work_item_id == *selected)
        })
        .or_else(|| {
            visible_work_items
                .first()
                .map(|item| item.work_item_id.clone())
        });
    let empty_state_copy = empty_state_copy_for_view(active_saved_view_id).to_string();
    let visible_activity_rows = selected_work_item_id
        .as_ref()
        .map(|selected| {
            local_store
                .activity_rows
                .iter()
                .filter(|row| &row.work_item_id == selected)
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let activity_empty_state = if selected_work_item_id.is_none() {
        "Select a work item to inspect activity.".to_string()
    } else if visible_activity_rows.is_empty() {
        "No activity yet for the selected work item.".to_string()
    } else {
        String::new()
    };
    (
        active_saved_view,
        active_filter_chips,
        visible_work_items,
        selected_work_item_id,
        empty_state_copy,
        visible_activity_rows,
        activity_empty_state,
        selection_notice,
        available_saved_views,
    )
}

fn next_work_item_id(work_items: &[ProjectOpsWorkItem]) -> Result<ProjectOpsWorkItemId, String> {
    let max_numeric = work_items
        .iter()
        .filter_map(|item| item.work_item_id.as_str().strip_prefix("wi-"))
        .filter_map(|value| value.parse::<u64>().ok())
        .max()
        .unwrap_or(0);
    ProjectOpsWorkItemId::new(format!("wi-{}", max_numeric.saturating_add(1)))
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use serde::{Deserialize, Serialize};

    use super::{
        PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID, PROJECT_OPS_BLOCKED_VIEW_ID,
        PROJECT_OPS_CYCLES_STREAM_ID, PROJECT_OPS_MY_WORK_VIEW_ID,
        PROJECT_OPS_SAVED_VIEWS_STREAM_ID, PROJECT_OPS_WORK_ITEMS_STREAM_ID, ProjectOpsPaneState,
        ProjectOpsPilotMetricsState, ProjectOpsPresentationMode, ProjectOpsService,
        project_ops_required_stream_grants,
    };
    use crate::project_ops::contract::{
        ProjectOpsAcceptedEventEnvelope, ProjectOpsAcceptedEventName,
        ProjectOpsChangeWorkItemStatus, ProjectOpsCommand, ProjectOpsCommandEnvelope,
        ProjectOpsCommandId, ProjectOpsCreateWorkItem, ProjectOpsEditWorkItemFields,
        ProjectOpsEditWorkItemFieldsPatch, ProjectOpsSetBlockedReason, ProjectOpsSetWorkItemCycle,
        ProjectOpsWorkItemDraft,
    };
    use crate::project_ops::projection::{
        ProjectOpsActivityRow, ProjectOpsCycleRow, ProjectOpsProjectionStore,
    };
    use crate::project_ops::schema::{
        ProjectOpsCycleId, ProjectOpsPriority, ProjectOpsTeamKey, ProjectOpsWorkItem,
        ProjectOpsWorkItemId, ProjectOpsWorkItemStatus,
    };

    static UNIQUE_PATH_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn unique_temp_path(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let counter = UNIQUE_PATH_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "openagents-project-ops-pane-{name}-{nanos}-{counter}.json"
        ))
    }

    fn work_item(
        work_item_id: &str,
        status: ProjectOpsWorkItemStatus,
        assignee: Option<&str>,
        blocked_reason: Option<&str>,
        updated_offset_unix_ms: u64,
    ) -> ProjectOpsWorkItem {
        let created_at_unix_ms = 1_762_000_000_000;
        ProjectOpsWorkItem {
            work_item_id: ProjectOpsWorkItemId::new(work_item_id).expect("work item id"),
            title: format!("Task {work_item_id}"),
            description: format!("Description for {work_item_id}"),
            status,
            priority: ProjectOpsPriority::High,
            assignee: assignee.map(ToString::to_string),
            team_key: ProjectOpsTeamKey::new("desktop").expect("team key"),
            cycle_id: Some(ProjectOpsCycleId::new("2026-w10").expect("cycle id")),
            parent_id: None,
            area_tags: vec!["pm".to_string()],
            blocked_reason: blocked_reason.map(ToString::to_string),
            due_at_unix_ms: None,
            created_at_unix_ms,
            updated_at_unix_ms: created_at_unix_ms + updated_offset_unix_ms,
            archived_at_unix_ms: None,
        }
    }

    fn sample_store() -> ProjectOpsProjectionStore {
        let work_items_path = unique_temp_path("work-items");
        let activity_path = unique_temp_path("activity");
        let cycles_path = unique_temp_path("cycles");
        let saved_views_path = unique_temp_path("saved-views");
        let checkpoint_path = unique_temp_path("checkpoints");
        let mut store = ProjectOpsProjectionStore::from_paths_for_tests(
            work_items_path,
            activity_path,
            cycles_path,
            saved_views_path,
            checkpoint_path,
        );
        let cycle = ProjectOpsCycleRow {
            cycle_id: ProjectOpsCycleId::new("2026-w10").expect("cycle id"),
            title: "Week 10".to_string(),
            goal: Some("Land PM pane selection".to_string()),
            starts_at_unix_ms: 1_761_998_400_000,
            ends_at_unix_ms: 1_762_603_200_000,
            is_active: true,
        };
        store
            .apply_cycles_projection(1, vec![cycle])
            .expect("cycle projection should apply");
        store
            .apply_work_items_projection(
                1,
                vec![
                    work_item(
                        "wi-1",
                        ProjectOpsWorkItemStatus::Todo,
                        Some("cdavid"),
                        None,
                        10,
                    ),
                    work_item(
                        "wi-2",
                        ProjectOpsWorkItemStatus::InProgress,
                        Some("cdavid"),
                        Some("Waiting on upstream"),
                        30,
                    ),
                    work_item(
                        "wi-3",
                        ProjectOpsWorkItemStatus::Done,
                        Some("cdavid"),
                        None,
                        20,
                    ),
                ],
            )
            .expect("work item projection should apply");
        store
            .apply_activity_projection(
                1,
                vec![
                    ProjectOpsActivityRow {
                        event_id: "pm:activity:1".to_string(),
                        work_item_id: ProjectOpsWorkItemId::new("wi-2").expect("work item id"),
                        event_name: ProjectOpsAcceptedEventName::WorkItemStatusChanged,
                        summary: "Moved wi-2 into in_progress".to_string(),
                        actor_label: "cdavid".to_string(),
                        command_id: "cmd-1".to_string(),
                        occurred_at_unix_ms: 1_762_000_200_000,
                    },
                    ProjectOpsActivityRow {
                        event_id: "pm:activity:2".to_string(),
                        work_item_id: ProjectOpsWorkItemId::new("wi-2").expect("work item id"),
                        event_name: ProjectOpsAcceptedEventName::WorkItemBlocked,
                        summary: "Blocked wi-2 waiting on upstream".to_string(),
                        actor_label: "cdavid".to_string(),
                        command_id: "cmd-2".to_string(),
                        occurred_at_unix_ms: 1_762_000_300_000,
                    },
                    ProjectOpsActivityRow {
                        event_id: "pm:activity:3".to_string(),
                        work_item_id: ProjectOpsWorkItemId::new("wi-1").expect("work item id"),
                        event_name: ProjectOpsAcceptedEventName::WorkItemAssigned,
                        summary: "Assigned wi-1 to cdavid".to_string(),
                        actor_label: "cdavid".to_string(),
                        command_id: "cmd-3".to_string(),
                        occurred_at_unix_ms: 1_762_000_250_000,
                    },
                ],
            )
            .expect("activity projection should apply");
        store
    }

    #[derive(Clone)]
    struct ProjectOpsFixturePaths {
        work_items_path: PathBuf,
        activity_path: PathBuf,
        cycles_path: PathBuf,
        saved_views_path: PathBuf,
        checkpoint_path: PathBuf,
    }

    impl ProjectOpsFixturePaths {
        fn new(tag: &str) -> Self {
            Self {
                work_items_path: unique_temp_path(&format!("{tag}-work-items")),
                activity_path: unique_temp_path(&format!("{tag}-activity")),
                cycles_path: unique_temp_path(&format!("{tag}-cycles")),
                saved_views_path: unique_temp_path(&format!("{tag}-saved-views")),
                checkpoint_path: unique_temp_path(&format!("{tag}-checkpoints")),
            }
        }

        fn build_store(&self) -> ProjectOpsProjectionStore {
            ProjectOpsProjectionStore::from_paths_for_tests(
                self.work_items_path.clone(),
                self.activity_path.clone(),
                self.cycles_path.clone(),
                self.saved_views_path.clone(),
                self.checkpoint_path.clone(),
            )
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
    struct ProjectOpsGoldenListRow {
        work_item_id: String,
        title: String,
        status: String,
        priority: String,
        assignee: Option<String>,
        blocked_reason: Option<String>,
        cycle_id: Option<String>,
        updated_at_unix_ms: u64,
    }

    #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
    struct ProjectOpsGoldenDetail {
        work_item_id: String,
        title: String,
        description: String,
        status: String,
        priority: String,
        assignee: Option<String>,
        cycle_id: Option<String>,
        parent_id: Option<String>,
        blocked_reason: Option<String>,
        created_at_unix_ms: u64,
        updated_at_unix_ms: u64,
    }

    #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
    struct ProjectOpsGoldenActivity {
        event_id: String,
        event_name: String,
        actor_label: String,
        summary: String,
        occurred_at_unix_ms: u64,
    }

    #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
    struct ProjectOpsGoldenSnapshot {
        source_badge: String,
        active_saved_view: String,
        selected_work_item_id: Option<String>,
        checkpoints: BTreeMap<String, u64>,
        accepted_events: Vec<ProjectOpsAcceptedEventEnvelope>,
        visible_work_items: Vec<ProjectOpsGoldenListRow>,
        detail: Option<ProjectOpsGoldenDetail>,
        activity_rows: Vec<ProjectOpsGoldenActivity>,
    }

    fn pm_projection_fixture_path() -> String {
        let root = env!("CARGO_MANIFEST_DIR");
        format!("{root}/tests/goldens/project_ops_stream_projection_snapshot.json")
    }

    fn assert_or_write_pm_fixture(path: &str, snapshot: &ProjectOpsGoldenSnapshot, label: &str) {
        let actual_json =
            serde_json::to_string_pretty(snapshot).expect("PM golden snapshot should serialize");
        if std::env::var("PROJECT_OPS_UPDATE_GOLDENS").as_deref() == Ok("1") {
            if let Some(parent) = std::path::Path::new(path).parent() {
                fs::create_dir_all(parent).expect("PM fixture directory should exist");
            }
            fs::write(path, actual_json).expect("PM fixture should write");
            return;
        }

        let expected_json = fs::read_to_string(path).unwrap_or_else(|error| {
            panic!(
                "missing {label} fixture {path}: {error}\nset PROJECT_OPS_UPDATE_GOLDENS=1 to regenerate.\nactual snapshot:\n{actual_json}"
            )
        });
        let expected = serde_json::from_str::<ProjectOpsGoldenSnapshot>(&expected_json)
            .expect("PM fixture should parse");
        if expected != *snapshot {
            panic!("{label} snapshot mismatch against {path}\nactual snapshot:\n{actual_json}");
        }
    }

    fn fixture_cycle() -> ProjectOpsCycleRow {
        ProjectOpsCycleRow {
            cycle_id: ProjectOpsCycleId::new("2026-w10").expect("cycle id"),
            title: "Week 10".to_string(),
            goal: Some("Freeze PM phase 1 semantics".to_string()),
            starts_at_unix_ms: 1_761_998_400_000,
            ends_at_unix_ms: 1_762_603_200_000,
            is_active: true,
        }
    }

    fn scripted_work_item_draft(
        work_item_id: &str,
        title: &str,
        description: &str,
        status: ProjectOpsWorkItemStatus,
        priority: ProjectOpsPriority,
        assignee: Option<&str>,
    ) -> ProjectOpsWorkItemDraft {
        ProjectOpsWorkItemDraft {
            work_item_id: ProjectOpsWorkItemId::new(work_item_id).expect("work item id"),
            title: title.to_string(),
            description: description.to_string(),
            status,
            priority,
            assignee: assignee.map(ToString::to_string),
            team_key: ProjectOpsTeamKey::new("desktop").expect("team key"),
            cycle_id: None,
            parent_id: None,
            area_tags: vec!["pm".to_string()],
            blocked_reason: None,
            due_at_unix_ms: None,
        }
    }

    fn scripted_command(
        command_id: &str,
        issued_at_unix_ms: u64,
        command: ProjectOpsCommand,
    ) -> ProjectOpsCommandEnvelope {
        ProjectOpsCommandEnvelope {
            command_id: ProjectOpsCommandId::new(command_id).expect("command id"),
            issued_at_unix_ms,
            actor: crate::project_ops::ProjectOpsActor {
                actor_id: Some("npub1fixture".to_string()),
                actor_label: Some("cdavid".to_string()),
            },
            command,
        }
    }

    fn apply_scripted_pm_cycle(
        store: &mut ProjectOpsProjectionStore,
    ) -> Vec<ProjectOpsAcceptedEventEnvelope> {
        store
            .apply_cycles_projection(1, vec![fixture_cycle()])
            .expect("cycle projection should seed");

        let commands = vec![
            scripted_command(
                "pm-fixture-cmd-1",
                1_762_100_000_000,
                ProjectOpsCommand::CreateWorkItem(ProjectOpsCreateWorkItem {
                    draft: scripted_work_item_draft(
                        "wi-1",
                        "Freeze PM contract manifest",
                        "Keep the Phase 1 contract surface stable.",
                        ProjectOpsWorkItemStatus::Backlog,
                        ProjectOpsPriority::High,
                        Some("cdavid"),
                    ),
                }),
            ),
            scripted_command(
                "pm-fixture-cmd-2",
                1_762_100_001_000,
                ProjectOpsCommand::ChangeWorkItemStatus(ProjectOpsChangeWorkItemStatus {
                    work_item_id: ProjectOpsWorkItemId::new("wi-1").expect("work item id"),
                    status: ProjectOpsWorkItemStatus::Todo,
                }),
            ),
            scripted_command(
                "pm-fixture-cmd-3",
                1_762_100_002_000,
                ProjectOpsCommand::SetWorkItemCycle(ProjectOpsSetWorkItemCycle {
                    work_item_id: ProjectOpsWorkItemId::new("wi-1").expect("work item id"),
                    cycle_id: ProjectOpsCycleId::new("2026-w10").expect("cycle id"),
                }),
            ),
            scripted_command(
                "pm-fixture-cmd-4",
                1_762_100_003_000,
                ProjectOpsCommand::CreateWorkItem(ProjectOpsCreateWorkItem {
                    draft: scripted_work_item_draft(
                        "wi-2",
                        "Prove PM projection replay",
                        "Exercise restart and rebuild against one deterministic script.",
                        ProjectOpsWorkItemStatus::Todo,
                        ProjectOpsPriority::Urgent,
                        Some("cdavid"),
                    ),
                }),
            ),
            scripted_command(
                "pm-fixture-cmd-5",
                1_762_100_004_000,
                ProjectOpsCommand::ChangeWorkItemStatus(ProjectOpsChangeWorkItemStatus {
                    work_item_id: ProjectOpsWorkItemId::new("wi-2").expect("work item id"),
                    status: ProjectOpsWorkItemStatus::InProgress,
                }),
            ),
            scripted_command(
                "pm-fixture-cmd-6",
                1_762_100_005_000,
                ProjectOpsCommand::SetBlockedReason(ProjectOpsSetBlockedReason {
                    work_item_id: ProjectOpsWorkItemId::new("wi-2").expect("work item id"),
                    blocked_reason: "Waiting on relay telemetry".to_string(),
                }),
            ),
            scripted_command(
                "pm-fixture-cmd-7",
                1_762_100_006_000,
                ProjectOpsCommand::EditWorkItemFields(ProjectOpsEditWorkItemFields {
                    work_item_id: ProjectOpsWorkItemId::new("wi-1").expect("work item id"),
                    patch: ProjectOpsEditWorkItemFieldsPatch {
                        title: Some("Freeze PM sync contract".to_string()),
                        description: Some(
                            "Keep the Phase 1 badge, grant, and checkpoint contract stable."
                                .to_string(),
                        ),
                        priority: Some(ProjectOpsPriority::Urgent),
                        due_at_unix_ms: Some(Some(1_762_500_000_000)),
                        area_tags: Some(vec!["pm".to_string(), "sync".to_string()]),
                    },
                }),
            ),
        ];

        let mut accepted_events = Vec::new();
        for command in commands {
            let result = ProjectOpsService::apply_command_to_store(store, command)
                .expect("scripted PM command should apply");
            match result {
                crate::project_ops::ProjectOpsCommandResult::Applied(result) => {
                    accepted_events.extend(result.accepted_events);
                }
                crate::project_ops::ProjectOpsCommandResult::DuplicateCommand { .. } => {
                    panic!("scripted PM fixture should not emit duplicate commands");
                }
            }
        }
        accepted_events
    }

    fn snapshot_from_store(
        store: ProjectOpsProjectionStore,
        accepted_events: Vec<ProjectOpsAcceptedEventEnvelope>,
    ) -> ProjectOpsGoldenSnapshot {
        let pane = ProjectOpsPaneState::from_local_store_for_tests(store, "cdavid");
        let mut checkpoints = BTreeMap::new();
        for stream_id in [
            PROJECT_OPS_WORK_ITEMS_STREAM_ID,
            PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID,
            PROJECT_OPS_CYCLES_STREAM_ID,
            PROJECT_OPS_SAVED_VIEWS_STREAM_ID,
        ] {
            checkpoints.insert(
                stream_id.to_string(),
                pane.local_store.checkpoint_for(stream_id).unwrap_or(0),
            );
        }
        ProjectOpsGoldenSnapshot {
            source_badge: pane.source_badge,
            active_saved_view: pane.active_saved_view,
            selected_work_item_id: pane
                .selected_work_item_id
                .as_ref()
                .map(|work_item_id| work_item_id.as_str().to_string()),
            checkpoints,
            accepted_events,
            visible_work_items: pane
                .visible_work_items
                .iter()
                .map(|item| ProjectOpsGoldenListRow {
                    work_item_id: item.work_item_id.as_str().to_string(),
                    title: item.title.clone(),
                    status: item.status.label().to_string(),
                    priority: item.priority.label().to_string(),
                    assignee: item.assignee.clone(),
                    blocked_reason: item.blocked_reason.clone(),
                    cycle_id: item
                        .cycle_id
                        .as_ref()
                        .map(|cycle_id| cycle_id.as_str().to_string()),
                    updated_at_unix_ms: item.updated_at_unix_ms,
                })
                .collect(),
            detail: pane
                .detail_draft
                .as_ref()
                .map(|detail| ProjectOpsGoldenDetail {
                    work_item_id: detail.work_item_id.as_str().to_string(),
                    title: detail.title.clone(),
                    description: detail.description.clone(),
                    status: detail.status.label().to_string(),
                    priority: detail.priority.label().to_string(),
                    assignee: detail.assignee.clone(),
                    cycle_id: detail
                        .cycle_id
                        .as_ref()
                        .map(|cycle_id| cycle_id.as_str().to_string()),
                    parent_id: detail
                        .parent_id
                        .as_ref()
                        .map(|parent_id| parent_id.as_str().to_string()),
                    blocked_reason: detail.blocked_reason.clone(),
                    created_at_unix_ms: detail.created_at_unix_ms,
                    updated_at_unix_ms: detail.updated_at_unix_ms,
                }),
            activity_rows: pane
                .visible_activity_rows
                .iter()
                .map(|row| ProjectOpsGoldenActivity {
                    event_id: row.event_id.clone(),
                    event_name: row.event_name.label().to_string(),
                    actor_label: row.actor_label.clone(),
                    summary: row.summary.clone(),
                    occurred_at_unix_ms: row.occurred_at_unix_ms,
                })
                .collect(),
        }
    }

    #[test]
    fn selection_defaults_to_first_visible_row_in_stable_sort_order() {
        let pane = ProjectOpsPaneState::from_local_store_for_tests(sample_store(), "cdavid");
        assert_eq!(pane.active_saved_view_id, PROJECT_OPS_MY_WORK_VIEW_ID);
        assert_eq!(
            pane.visible_work_items
                .iter()
                .map(|item| item.work_item_id.as_str())
                .collect::<Vec<_>>(),
            vec!["wi-2", "wi-1"]
        );
        assert_eq!(
            pane.selected_work_item_id
                .as_ref()
                .map(|item| item.as_str()),
            Some("wi-2")
        );
        assert_eq!(
            pane.selected_work_item()
                .map(|item| item.work_item_id.as_str().to_string()),
            Some("wi-2".to_string())
        );
    }

    #[test]
    fn selection_can_move_and_reacts_to_view_filter_changes() {
        let mut pane = ProjectOpsPaneState::from_local_store_for_tests(sample_store(), "cdavid");
        assert!(pane.move_selection(1));
        assert_eq!(
            pane.selected_work_item_id
                .as_ref()
                .map(|item| item.as_str()),
            Some("wi-1")
        );
        assert!(!pane.move_selection(1));

        assert!(pane.set_active_saved_view(PROJECT_OPS_BLOCKED_VIEW_ID));
        assert_eq!(pane.active_saved_view, "Blocked");
        assert_eq!(
            pane.visible_work_items
                .iter()
                .map(|item| item.work_item_id.as_str())
                .collect::<Vec<_>>(),
            vec!["wi-2"]
        );
        assert_eq!(
            pane.selected_work_item_id
                .as_ref()
                .map(|item| item.as_str()),
            Some("wi-2")
        );

        assert!(pane.set_search_query("missing"));
        assert!(pane.visible_work_items.is_empty());
        assert!(pane.selected_work_item_id.is_none());
        assert_eq!(pane.empty_state_copy, "No blocked work.");
    }

    #[test]
    fn sync_runtime_contract_state_mirrors_pm_grants_and_shared_checkpoints() {
        let work_items_path = unique_temp_path("sync-work-items");
        let activity_path = unique_temp_path("sync-activity");
        let cycles_path = unique_temp_path("sync-cycles");
        let saved_views_path = unique_temp_path("sync-saved-views");
        let checkpoint_path = unique_temp_path("sync-checkpoints");

        let store = ProjectOpsProjectionStore::from_paths_for_tests(
            work_items_path,
            activity_path,
            cycles_path,
            saved_views_path,
            checkpoint_path.clone(),
        );
        let mut pane = ProjectOpsPaneState::from_local_store_for_tests(store, "cdavid");
        let mut shared = crate::sync_apply::SyncApplyEngine::load_or_new(
            checkpoint_path,
            crate::sync_apply::SyncApplyPolicy::default(),
        )
        .expect("shared checkpoint engine should initialize");
        for stream_id in project_ops_required_stream_grants() {
            shared
                .ensure_stream_registered(stream_id)
                .expect("PM stream should register");
        }
        shared
            .adopt_checkpoint_if_newer(PROJECT_OPS_WORK_ITEMS_STREAM_ID, 7)
            .expect("work item checkpoint should adopt");
        shared
            .adopt_checkpoint_if_newer(PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID, 5)
            .expect("activity checkpoint should adopt");
        shared
            .adopt_checkpoint_if_newer(PROJECT_OPS_CYCLES_STREAM_ID, 3)
            .expect("cycle checkpoint should adopt");

        let mut lifecycle = crate::sync_lifecycle::RuntimeSyncLifecycleManager::default();
        let worker_id = "desktopw:sync";
        lifecycle.mark_connecting(worker_id);
        lifecycle.mark_replay_bootstrap(worker_id, 5, Some(7));
        lifecycle.mark_live(worker_id, Some(120));
        let snapshot = lifecycle.snapshot(worker_id);

        pane.sync_runtime_contract_state(
            Some("Minted sync token and hydrated 3 remote checkpoints"),
            None,
            &["stream.pm.*".to_string()],
            snapshot.as_ref(),
        );

        assert_eq!(pane.sync_diagnostics.bootstrap_state, "live");
        assert!(
            pane.sync_diagnostics.missing_stream_grants.is_empty(),
            "wildcard PM grant should satisfy all PM streams"
        );
        assert_eq!(
            pane.local_store
                .checkpoint_for(PROJECT_OPS_WORK_ITEMS_STREAM_ID),
            Some(7)
        );
        assert_eq!(
            pane.sync_diagnostics
                .streams
                .iter()
                .find(|stream| stream.stream_id == PROJECT_OPS_WORK_ITEMS_STREAM_ID)
                .map(|stream| (
                    stream.granted,
                    stream.checkpoint_seq,
                    stream.resume_cursor_seq
                )),
            Some((true, 7, 7))
        );
    }

    #[test]
    fn sync_runtime_contract_state_keeps_missing_grants_and_stale_cursor_explicit() {
        let pane_store = sample_store();
        let mut pane = ProjectOpsPaneState::from_local_store_for_tests(pane_store, "cdavid");
        let mut lifecycle = crate::sync_lifecycle::RuntimeSyncLifecycleManager::default();
        let worker_id = "desktopw:sync";
        lifecycle.mark_connecting(worker_id);
        let _ = lifecycle.mark_disconnect(
            worker_id,
            crate::sync_lifecycle::RuntimeSyncDisconnectReason::StaleCursor,
            Some("stale_cursor; replay bootstrap required".to_string()),
        );
        let snapshot = lifecycle.snapshot(worker_id);

        pane.sync_runtime_contract_state(
            Some("Minted sync token without the full PM grant set"),
            None,
            &["stream.pm.work_items.v1".to_string()],
            snapshot.as_ref(),
        );

        assert_eq!(
            pane.sync_diagnostics.bootstrap_state,
            "stale_cursor_recovery_required"
        );
        assert!(pane.sync_diagnostics.stale_cursor_recovery_required);
        assert_eq!(
            pane.sync_diagnostics.last_disconnect_reason.as_deref(),
            Some("stale_cursor")
        );
        assert_eq!(
            pane.sync_diagnostics.missing_stream_grants,
            vec![
                PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID.to_string(),
                PROJECT_OPS_CYCLES_STREAM_ID.to_string(),
                PROJECT_OPS_SAVED_VIEWS_STREAM_ID.to_string(),
            ]
        );
    }

    #[test]
    fn quick_create_uses_service_loop_and_selects_new_item() {
        let mut pane = ProjectOpsPaneState::from_local_store_for_tests(sample_store(), "cdavid");
        pane.set_quick_create_title("Capture a new PM task");
        pane.set_quick_create_description("Quick create should add a backlog item.");
        pane.set_quick_create_priority(ProjectOpsPriority::Urgent);
        pane.set_quick_create_area_tags(&["pm", "sync"]);
        pane.set_quick_create_due_at(Some(1_762_500_000_000));

        let created_id = pane
            .apply_quick_create()
            .expect("quick create should succeed")
            .expect("quick create should return a new id");
        assert_eq!(created_id.as_str(), "wi-4");
        let created = pane
            .local_store
            .work_items
            .iter()
            .find(|item| item.work_item_id == created_id)
            .expect("created item should exist");
        assert_eq!(created.status, ProjectOpsWorkItemStatus::Backlog);
        assert_eq!(
            created.area_tags,
            vec!["pm".to_string(), "sync".to_string()]
        );
        assert_eq!(created.due_at_unix_ms, Some(1_762_500_000_000));
        assert_eq!(pane.quick_create_draft.title, "");
        assert_eq!(
            pane.detail_save_status.as_deref(),
            Some("Quick create applied")
        );
    }

    #[test]
    fn detail_draft_applies_primary_edit_fields() {
        let mut pane = ProjectOpsPaneState::from_local_store_for_tests(sample_store(), "cdavid");
        pane.edit_detail_title("Updated PM task title");
        pane.edit_detail_description("Updated description");
        pane.set_detail_status(ProjectOpsWorkItemStatus::InReview);
        pane.set_detail_priority(ProjectOpsPriority::Urgent);
        pane.set_detail_assignee(Some("teammate"));
        pane.set_detail_parent(Some(
            ProjectOpsWorkItemId::new("wi-1").expect("work item id"),
        ));
        pane.set_detail_area_tags(&["pm", "sync"]);
        pane.set_detail_blocked_reason(Some("Waiting on design"));
        pane.set_detail_due_at(Some(1_762_600_000_000));

        assert!(
            pane.apply_detail_draft()
                .expect("detail apply should succeed")
        );

        let updated = pane
            .local_store
            .work_items
            .iter()
            .find(|item| item.work_item_id.as_str() == "wi-2")
            .expect("updated item should exist");
        assert_eq!(updated.title, "Updated PM task title");
        assert_eq!(updated.description, "Updated description");
        assert_eq!(updated.status, ProjectOpsWorkItemStatus::InReview);
        assert_eq!(updated.priority, ProjectOpsPriority::Urgent);
        assert_eq!(updated.assignee.as_deref(), Some("teammate"));
        assert_eq!(
            updated
                .parent_id
                .as_ref()
                .map(|parent_id| parent_id.as_str()),
            Some("wi-1")
        );
        assert_eq!(
            updated.area_tags,
            vec!["pm".to_string(), "sync".to_string()]
        );
        assert_eq!(updated.blocked_reason.as_deref(), Some("Waiting on design"));
        assert_eq!(updated.due_at_unix_ms, Some(1_762_600_000_000));
        assert_eq!(
            pane.detail_save_status.as_deref(),
            Some("Detail changes applied")
        );
    }

    #[test]
    fn activity_rows_follow_selected_item_and_surface_filtered_selection_notice() {
        let mut pane = ProjectOpsPaneState::from_local_store_for_tests(sample_store(), "cdavid");
        assert_eq!(
            pane.visible_activity_rows
                .iter()
                .map(|row| row.event_id.as_str())
                .collect::<Vec<_>>(),
            vec!["pm:activity:2", "pm:activity:1"]
        );
        assert!(pane.activity_empty_state.is_empty());

        assert!(pane.set_search_query("wi-1"));
        assert_eq!(
            pane.selected_work_item_id
                .as_ref()
                .map(|item| item.as_str()),
            Some("wi-1")
        );
        assert!(
            pane.selection_notice
                .as_deref()
                .is_some_and(|notice| notice.contains("filtered out"))
        );
        assert_eq!(
            pane.visible_activity_rows
                .iter()
                .map(|row| row.event_id.as_str())
                .collect::<Vec<_>>(),
            vec!["pm:activity:3"]
        );
    }

    #[test]
    fn board_lane_drop_routes_through_pm_command_path() {
        let mut pane = ProjectOpsPaneState::from_local_store_for_tests(sample_store(), "cdavid");

        assert!(pane.set_presentation_mode(ProjectOpsPresentationMode::Board));
        assert_eq!(pane.presentation_mode, ProjectOpsPresentationMode::Board);
        assert!(pane.start_board_drag("wi-2"));
        assert_eq!(
            pane.board_drag_state
                .as_ref()
                .map(|drag| (drag.work_item_id.as_str(), drag.from_status)),
            Some(("wi-2", ProjectOpsWorkItemStatus::InProgress))
        );

        let work_items_seq_before = pane
            .local_store
            .checkpoint_for(PROJECT_OPS_WORK_ITEMS_STREAM_ID)
            .unwrap_or(0);
        let activity_count_before = pane.local_store.activity_rows.len();

        assert!(
            pane.drop_board_drag(ProjectOpsWorkItemStatus::InReview)
                .expect("board drop should succeed")
        );

        let updated = pane
            .local_store
            .work_items
            .iter()
            .find(|item| item.work_item_id.as_str() == "wi-2")
            .expect("board-moved item should exist");
        assert_eq!(updated.status, ProjectOpsWorkItemStatus::InReview);
        assert_eq!(
            pane.local_store
                .checkpoint_for(PROJECT_OPS_WORK_ITEMS_STREAM_ID)
                .unwrap_or(0),
            work_items_seq_before + 1
        );
        assert_eq!(
            pane.local_store.activity_rows.len(),
            activity_count_before + 1
        );
        assert_eq!(pane.board_drag_state, None);
        assert_eq!(
            pane.detail_save_status.as_deref(),
            Some("Board move applied")
        );
        assert_eq!(
            pane.pilot_metrics
                .command_counts
                .get("ChangeWorkItemStatus"),
            Some(&1)
        );
        assert!(
            pane.local_store
                .activity_rows
                .first()
                .is_some_and(|row| row.summary.contains("in_review"))
        );
    }

    #[test]
    fn board_lane_drop_rejects_invalid_transition_without_bypassing_service_rules() {
        let mut pane = ProjectOpsPaneState::from_local_store_for_tests(sample_store(), "cdavid");
        let activity_count_before = pane.local_store.activity_rows.len();
        let work_items_seq_before = pane
            .local_store
            .checkpoint_for(PROJECT_OPS_WORK_ITEMS_STREAM_ID)
            .unwrap_or(0);

        assert!(pane.set_presentation_mode(ProjectOpsPresentationMode::Board));
        assert!(pane.start_board_drag("wi-1"));
        let error = pane
            .drop_board_drag(ProjectOpsWorkItemStatus::Done)
            .expect_err("invalid board transition should reject");
        assert!(error.contains("invalid status transition"));
        assert_eq!(
            pane.board_drag_state
                .as_ref()
                .map(|drag| drag.work_item_id.as_str()),
            Some("wi-1")
        );
        assert_eq!(
            pane.local_store
                .work_items
                .iter()
                .find(|item| item.work_item_id.as_str() == "wi-1")
                .map(|item| item.status),
            Some(ProjectOpsWorkItemStatus::Todo)
        );
        assert_eq!(pane.local_store.activity_rows.len(), activity_count_before);
        assert_eq!(
            pane.local_store
                .checkpoint_for(PROJECT_OPS_WORK_ITEMS_STREAM_ID)
                .unwrap_or(0),
            work_items_seq_before
        );
    }

    #[test]
    fn scripted_pilot_cycle_records_command_and_view_usage() {
        let metrics_path = unique_temp_path("pilot-metrics");
        let mut pane = ProjectOpsPaneState::from_local_store_for_tests(sample_store(), "cdavid");
        pane.pilot_metrics =
            ProjectOpsPilotMetricsState::from_metrics_path_for_tests(metrics_path.clone())
                .expect("pilot metrics should initialize");
        pane.pilot_metrics
            .record_view(pane.active_saved_view_id.as_str())
            .expect("default view should record");

        pane.edit_detail_title("Pilot cycle edit");
        assert!(
            pane.apply_detail_draft()
                .expect("detail apply should succeed")
        );

        pane.set_quick_create_title("Pilot cycle task");
        pane.set_quick_create_description("Created during scripted pilot cycle");
        pane.set_quick_create_priority(ProjectOpsPriority::High);
        let _ = pane
            .apply_quick_create()
            .expect("quick create should succeed")
            .expect("quick create should produce an id");

        assert!(pane.set_active_saved_view(PROJECT_OPS_BLOCKED_VIEW_ID));
        pane.pilot_metrics
            .record_cycle_summary("scripted internal PM cycle completed")
            .expect("cycle summary should record");

        let restored = ProjectOpsPilotMetricsState::from_metrics_path_for_tests(metrics_path)
            .expect("pilot metrics should reload");
        assert_eq!(
            restored.view_counts.get(PROJECT_OPS_MY_WORK_VIEW_ID),
            Some(&1)
        );
        assert_eq!(
            restored.view_counts.get(PROJECT_OPS_BLOCKED_VIEW_ID),
            Some(&1)
        );
        assert_eq!(restored.command_counts.get("CreateWorkItem"), Some(&1));
        assert_eq!(restored.command_counts.get("EditWorkItemFields"), Some(&1));
        assert_eq!(
            restored.last_cycle_summary.as_deref(),
            Some("scripted internal PM cycle completed")
        );
    }

    #[test]
    fn project_ops_stream_projection_fixture_matches_golden_across_rebuild_and_restart() {
        let paths = ProjectOpsFixturePaths::new("golden-fixture");
        let mut initial_store = paths.build_store();
        let accepted_events = apply_scripted_pm_cycle(&mut initial_store);
        let initial_snapshot = snapshot_from_store(initial_store, accepted_events.clone());

        let reloaded_snapshot = snapshot_from_store(paths.build_store(), accepted_events.clone());
        assert_eq!(
            reloaded_snapshot, initial_snapshot,
            "restart reload should preserve the same PM visible snapshot"
        );

        let rebuild_paths = ProjectOpsFixturePaths::new("golden-rebuild");
        let mut rebuilt_store = rebuild_paths.build_store();
        let rebuilt_events = apply_scripted_pm_cycle(&mut rebuilt_store);
        let rebuilt_snapshot = snapshot_from_store(rebuilt_store, rebuilt_events);
        assert_eq!(
            rebuilt_snapshot, initial_snapshot,
            "rebuild from zero should reproduce the same PM visible snapshot"
        );

        assert_or_write_pm_fixture(
            pm_projection_fixture_path().as_str(),
            &initial_snapshot,
            "project_ops_stream_projection",
        );
    }
}
