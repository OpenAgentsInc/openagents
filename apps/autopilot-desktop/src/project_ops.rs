use crate::app_state::PaneLoadState;

pub mod contract;
pub mod editor;
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
pub use editor::{ProjectOpsDetailDraft, ProjectOpsQuickCreateDraft};

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
    pub quick_create_draft: ProjectOpsQuickCreateDraft,
    pub detail_draft: Option<ProjectOpsDetailDraft>,
    pub detail_save_status: Option<String>,
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
        let search_query = String::new();
        let (
            active_saved_view,
            active_filter_chips,
            visible_work_items,
            selected_work_item_id,
            empty_state_copy,
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
            )
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
        let detail_draft = selected_work_item_id
            .as_ref()
            .and_then(|selected| {
                visible_work_items
                    .iter()
                    .find(|item| &item.work_item_id == selected)
                    .cloned()
            })
            .map(|item| ProjectOpsDetailDraft::from_work_item(&item));

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
            quick_create_draft: ProjectOpsQuickCreateDraft::default(),
            detail_draft,
            detail_save_status: None,
            source_badge,
            summary,
            status_note,
            local_store,
        }
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
        Self {
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
            selected_work_item_id,
            empty_state_copy,
            available_saved_views,
            quick_create_draft: ProjectOpsQuickCreateDraft::default(),
            detail_draft,
            detail_save_status: None,
            source_badge: local_store.source_badge(),
            summary: "Test PM pane".to_string(),
            status_note: "Test PM pane".to_string(),
            local_store,
        }
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

    fn refresh_derived_view_state(&mut self) {
        let (
            active_saved_view,
            active_filter_chips,
            visible_work_items,
            selected_work_item_id,
            empty_state_copy,
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
        self.selected_work_item_id = selected_work_item_id;
        self.empty_state_copy = empty_state_copy;
        self.available_saved_views = available_saved_views;
        self.load_state = self.local_store.load_state;
        self.last_error = self.local_store.last_error.clone();
        self.sync_detail_draft_from_selection();
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

    pub fn set_detail_blocked_reason(&mut self, blocked_reason: Option<&str>) {
        if let Some(detail_draft) = self.detail_draft.as_mut() {
            detail_draft.blocked_reason = blocked_reason
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string);
            detail_draft.dirty = true;
        }
    }

    pub fn apply_quick_create(&mut self) -> Result<Option<ProjectOpsWorkItemId>, String> {
        if !self.feature_enabled {
            return Ok(None);
        }
        self.quick_create_draft.validate()?;
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
                draft: self.quick_create_draft.to_work_item_draft(work_item_id.clone()),
            }),
        };
        let result = ProjectOpsService::apply_command_to_store(&mut self.local_store, command)?;
        self.refresh_derived_view_state();
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
            return Err(format!(
                "selected work item {} no longer exists",
                detail_draft.work_item_id.as_str()
            ));
        };
        let issued_at_unix_ms = now_unix_ms();
        let mut command_counter = 0u64;
        let mut applied_any = false;

        let mut apply_command = |command: ProjectOpsCommand| -> Result<(), String> {
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
            Ok(())
        };

        if detail_draft.title != current.title
            || detail_draft.description != current.description
            || detail_draft.priority != current.priority
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
                        due_at_unix_ms: None,
                        area_tags: None,
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
        self.detail_save_status = Some(if applied_any {
            "Detail changes applied".to_string()
        } else {
            "Detail draft had no changes".to_string()
        });
        Ok(applied_any)
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
    let selected_work_item_id = selected_work_item_id
        .cloned()
        .filter(|selected| {
            visible_work_items
                .iter()
                .any(|item| item.work_item_id == *selected)
        })
        .or_else(|| visible_work_items.first().map(|item| item.work_item_id.clone()));
    let empty_state_copy = empty_state_copy_for_view(active_saved_view_id).to_string();
    (
        active_saved_view,
        active_filter_chips,
        visible_work_items,
        selected_work_item_id,
        empty_state_copy,
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
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{
        ProjectOpsPaneState, PROJECT_OPS_BLOCKED_VIEW_ID, PROJECT_OPS_MY_WORK_VIEW_ID,
    };
    use crate::project_ops::projection::{ProjectOpsCycleRow, ProjectOpsProjectionStore};
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
        std::env::temp_dir()
            .join(format!("openagents-project-ops-pane-{name}-{nanos}-{counter}.json"))
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
                    work_item("wi-1", ProjectOpsWorkItemStatus::Todo, Some("cdavid"), None, 10),
                    work_item(
                        "wi-2",
                        ProjectOpsWorkItemStatus::InProgress,
                        Some("cdavid"),
                        Some("Waiting on upstream"),
                        30,
                    ),
                    work_item("wi-3", ProjectOpsWorkItemStatus::Done, Some("cdavid"), None, 20),
                ],
            )
            .expect("work item projection should apply");
        store
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
            pane.selected_work_item_id.as_ref().map(|item| item.as_str()),
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
            pane.selected_work_item_id.as_ref().map(|item| item.as_str()),
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
            pane.selected_work_item_id.as_ref().map(|item| item.as_str()),
            Some("wi-2")
        );

        assert!(pane.set_search_query("missing"));
        assert!(pane.visible_work_items.is_empty());
        assert!(pane.selected_work_item_id.is_none());
        assert_eq!(pane.empty_state_copy, "No blocked work.");
    }

    #[test]
    fn quick_create_uses_service_loop_and_selects_new_item() {
        let mut pane = ProjectOpsPaneState::from_local_store_for_tests(sample_store(), "cdavid");
        pane.set_quick_create_title("Capture a new PM task");
        pane.set_quick_create_description("Quick create should add a backlog item.");
        pane.set_quick_create_priority(ProjectOpsPriority::Urgent);

        let created_id = pane
            .apply_quick_create()
            .expect("quick create should succeed")
            .expect("quick create should return a new id");
        assert_eq!(created_id.as_str(), "wi-4");
        assert!(
            pane.local_store
                .work_items
                .iter()
                .any(|item| item.work_item_id == created_id && item.status == ProjectOpsWorkItemStatus::Backlog)
        );
        assert_eq!(pane.quick_create_draft.title, "");
        assert_eq!(pane.detail_save_status.as_deref(), Some("Quick create applied"));
    }

    #[test]
    fn detail_draft_applies_primary_edit_fields() {
        let mut pane = ProjectOpsPaneState::from_local_store_for_tests(sample_store(), "cdavid");
        pane.edit_detail_title("Updated PM task title");
        pane.edit_detail_description("Updated description");
        pane.set_detail_status(ProjectOpsWorkItemStatus::InReview);
        pane.set_detail_priority(ProjectOpsPriority::Urgent);
        pane.set_detail_assignee(Some("teammate"));
        pane.set_detail_blocked_reason(Some("Waiting on design"));

        assert!(pane.apply_detail_draft().expect("detail apply should succeed"));

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
        assert_eq!(updated.blocked_reason.as_deref(), Some("Waiting on design"));
        assert_eq!(pane.detail_save_status.as_deref(), Some("Detail changes applied"));
    }
}
