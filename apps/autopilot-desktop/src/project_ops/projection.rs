use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::app_state::PaneLoadState;
use crate::project_ops::contract::{
    project_ops_error, ProjectOpsAcceptedEventName, ProjectOpsErrorCode,
    PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID, PROJECT_OPS_CYCLES_STREAM_ID,
    PROJECT_OPS_PRIMARY_SOURCE_BADGE, PROJECT_OPS_SAVED_VIEWS_STREAM_ID,
    PROJECT_OPS_WORK_ITEMS_STREAM_ID,
};
use crate::project_ops::schema::{ProjectOpsCycleId, ProjectOpsWorkItem, ProjectOpsWorkItemId};
use crate::project_ops::views::builtin_saved_view_specs;
use crate::sync_apply::{StreamApplyDecision, SyncApplyEngine, SyncApplyPolicy};

pub const PROJECT_OPS_PROJECTION_SCHEMA_VERSION: u16 = 1;

const WORK_ITEMS_FILE_NAME: &str = "autopilot-pm-work-items-projection-v1.json";
const ACTIVITY_FILE_NAME: &str = "autopilot-pm-activity-projection-v1.json";
const CYCLES_FILE_NAME: &str = "autopilot-pm-cycles-v1.json";
const SAVED_VIEWS_FILE_NAME: &str = "autopilot-pm-saved-views-v1.json";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectOpsActivityRow {
    pub event_id: String,
    pub work_item_id: ProjectOpsWorkItemId,
    pub event_name: ProjectOpsAcceptedEventName,
    pub summary: String,
    pub actor_label: String,
    pub command_id: String,
    pub occurred_at_unix_ms: u64,
}

impl ProjectOpsActivityRow {
    pub fn validate(&self) -> Result<(), String> {
        if self.event_id.trim().is_empty() {
            return Err("project ops activity event_id must not be empty".to_string());
        }
        if self.summary.trim().is_empty() {
            return Err("project ops activity summary must not be empty".to_string());
        }
        if self.actor_label.trim().is_empty() {
            return Err("project ops activity actor_label must not be empty".to_string());
        }
        if self.command_id.trim().is_empty() {
            return Err("project ops activity command_id must not be empty".to_string());
        }
        if self.occurred_at_unix_ms == 0 {
            return Err("project ops activity occurred_at_unix_ms must be > 0".to_string());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectOpsCycleRow {
    pub cycle_id: ProjectOpsCycleId,
    pub title: String,
    pub goal: Option<String>,
    pub starts_at_unix_ms: u64,
    pub ends_at_unix_ms: u64,
    pub is_active: bool,
}

impl ProjectOpsCycleRow {
    pub fn validate(&self) -> Result<(), String> {
        if self.title.trim().is_empty() {
            return Err("project ops cycle title must not be empty".to_string());
        }
        if self.starts_at_unix_ms == 0 {
            return Err("project ops cycle starts_at_unix_ms must be > 0".to_string());
        }
        if self.ends_at_unix_ms < self.starts_at_unix_ms {
            return Err("project ops cycle ends_at_unix_ms must be >= starts_at_unix_ms".to_string());
        }
        if self.goal.as_deref().is_some_and(|goal| goal.trim().is_empty()) {
            return Err("project ops cycle goal must not be blank when present".to_string());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectOpsSavedViewRow {
    pub view_id: String,
    pub title: String,
    pub query: String,
    pub filters: Vec<String>,
    pub built_in: bool,
}

impl ProjectOpsSavedViewRow {
    pub fn validate(&self) -> Result<(), String> {
        if self.view_id.trim().is_empty() {
            return Err("project ops saved view_id must not be empty".to_string());
        }
        if self.title.trim().is_empty() {
            return Err("project ops saved view title must not be empty".to_string());
        }
        if self.filters.iter().any(|filter| filter.trim().is_empty()) {
            return Err("project ops saved view filters must not contain blanks".to_string());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProjectOpsWorkItemsProjectionDocumentV1 {
    schema_version: u16,
    stream_id: String,
    rows: Vec<ProjectOpsWorkItem>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProjectOpsActivityProjectionDocumentV1 {
    schema_version: u16,
    stream_id: String,
    rows: Vec<ProjectOpsActivityRow>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProjectOpsCyclesProjectionDocumentV1 {
    schema_version: u16,
    stream_id: String,
    rows: Vec<ProjectOpsCycleRow>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProjectOpsSavedViewsProjectionDocumentV1 {
    schema_version: u16,
    stream_id: String,
    rows: Vec<ProjectOpsSavedViewRow>,
}

pub struct ProjectOpsProjectionStore {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub work_items: Vec<ProjectOpsWorkItem>,
    pub activity_rows: Vec<ProjectOpsActivityRow>,
    pub cycles: Vec<ProjectOpsCycleRow>,
    pub saved_views: Vec<ProjectOpsSavedViewRow>,
    work_items_path: PathBuf,
    activity_path: PathBuf,
    cycles_path: PathBuf,
    saved_views_path: PathBuf,
    checkpoint_path: PathBuf,
    checkpoints: Option<SyncApplyEngine>,
}

impl ProjectOpsProjectionStore {
    pub fn disabled() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Project Ops local projections idle until the feature gate is enabled".to_string()),
            work_items: Vec::new(),
            activity_rows: Vec::new(),
            cycles: Vec::new(),
            saved_views: Vec::new(),
            work_items_path: PathBuf::new(),
            activity_path: PathBuf::new(),
            cycles_path: PathBuf::new(),
            saved_views_path: PathBuf::new(),
            checkpoint_path: PathBuf::new(),
            checkpoints: None,
        }
    }

    pub fn load_or_bootstrap_default() -> Self {
        Self::from_paths(
            work_items_projection_file_path(),
            activity_projection_file_path(),
            cycles_projection_file_path(),
            saved_views_projection_file_path(),
            SyncApplyEngine::default_checkpoint_path(),
        )
    }

    #[cfg(test)]
    pub(crate) fn from_paths_for_tests(
        work_items_path: PathBuf,
        activity_path: PathBuf,
        cycles_path: PathBuf,
        saved_views_path: PathBuf,
        checkpoint_path: PathBuf,
    ) -> Self {
        Self::from_paths(
            work_items_path,
            activity_path,
            cycles_path,
            saved_views_path,
            checkpoint_path,
        )
    }

    fn from_paths(
        work_items_path: PathBuf,
        activity_path: PathBuf,
        cycles_path: PathBuf,
        saved_views_path: PathBuf,
        checkpoint_path: PathBuf,
    ) -> Self {
        match Self::load_or_bootstrap(
            work_items_path.clone(),
            activity_path.clone(),
            cycles_path.clone(),
            saved_views_path.clone(),
            checkpoint_path.clone(),
        ) {
            Ok((checkpoints, work_items, activity_rows, cycles, saved_views, bootstrapped)) => {
                let last_action = if bootstrapped.is_empty() {
                    Some(format!(
                        "Loaded PM projection streams ({} work items / {} activity / {} cycles / {} views)",
                        work_items.len(),
                        activity_rows.len(),
                        cycles.len(),
                        saved_views.len()
                    ))
                } else {
                    Some(format!(
                        "Bootstrapped PM projection docs for {}",
                        bootstrapped.join(", ")
                    ))
                };
                Self {
                    load_state: PaneLoadState::Ready,
                    last_error: None,
                    last_action,
                    work_items,
                    activity_rows,
                    cycles,
                    saved_views,
                    work_items_path,
                    activity_path,
                    cycles_path,
                    saved_views_path,
                    checkpoint_path,
                    checkpoints: Some(checkpoints),
                }
            }
            Err(error) => Self {
                load_state: PaneLoadState::Error,
                last_error: Some(error),
                last_action: Some("PM projection bootstrap failed".to_string()),
                work_items: Vec::new(),
                activity_rows: Vec::new(),
                cycles: Vec::new(),
                saved_views: Vec::new(),
                work_items_path,
                activity_path,
                cycles_path,
                saved_views_path,
                checkpoint_path,
                checkpoints: None,
            },
        }
    }

    fn load_or_bootstrap(
        work_items_path: PathBuf,
        activity_path: PathBuf,
        cycles_path: PathBuf,
        saved_views_path: PathBuf,
        checkpoint_path: PathBuf,
    ) -> Result<
        (
            SyncApplyEngine,
            Vec<ProjectOpsWorkItem>,
            Vec<ProjectOpsActivityRow>,
            Vec<ProjectOpsCycleRow>,
            Vec<ProjectOpsSavedViewRow>,
            Vec<&'static str>,
        ),
        String,
    > {
        let mut checkpoints =
            SyncApplyEngine::load_or_new(checkpoint_path, SyncApplyPolicy::default())?;
        let mut bootstrapped = Vec::new();

        if checkpoints.ensure_stream_registered(PROJECT_OPS_WORK_ITEMS_STREAM_ID)? {
            bootstrapped.push(PROJECT_OPS_WORK_ITEMS_STREAM_ID);
        }
        if checkpoints.ensure_stream_registered(PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID)? {
            bootstrapped.push(PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID);
        }
        if checkpoints.ensure_stream_registered(PROJECT_OPS_CYCLES_STREAM_ID)? {
            bootstrapped.push(PROJECT_OPS_CYCLES_STREAM_ID);
        }
        if checkpoints.ensure_stream_registered(PROJECT_OPS_SAVED_VIEWS_STREAM_ID)? {
            bootstrapped.push(PROJECT_OPS_SAVED_VIEWS_STREAM_ID);
        }

        let (work_items, work_items_bootstrapped) =
            load_or_bootstrap_work_items(work_items_path.as_path())?;
        if work_items_bootstrapped {
            bootstrapped.push(WORK_ITEMS_FILE_NAME);
        }
        let (activity_rows, activity_bootstrapped) =
            load_or_bootstrap_activity(activity_path.as_path())?;
        if activity_bootstrapped {
            bootstrapped.push(ACTIVITY_FILE_NAME);
        }
        let (cycles, cycles_bootstrapped) = load_or_bootstrap_cycles(cycles_path.as_path())?;
        if cycles_bootstrapped {
            bootstrapped.push(CYCLES_FILE_NAME);
        }
        let (saved_views, saved_views_bootstrapped) =
            load_or_bootstrap_saved_views(saved_views_path.as_path())?;
        if saved_views_bootstrapped {
            bootstrapped.push(SAVED_VIEWS_FILE_NAME);
        }

        Ok((
            checkpoints,
            work_items,
            activity_rows,
            cycles,
            saved_views,
            bootstrapped,
        ))
    }

    pub fn source_badge(&self) -> String {
        PROJECT_OPS_PRIMARY_SOURCE_BADGE.to_string()
    }

    pub fn checkpoint_for(&self, stream_id: &str) -> Option<u64> {
        self.checkpoints
            .as_ref()
            .and_then(|checkpoints| checkpoints.checkpoint_for(stream_id))
    }

    pub fn max_checkpoint_seq(&self) -> u64 {
        self.checkpoints
            .as_ref()
            .map_or(0, SyncApplyEngine::max_checkpoint_seq)
    }

    pub fn resume_cursor_for_stream(&self, stream_id: &str, remote_head_seq: Option<u64>) -> Option<u64> {
        self.checkpoints
            .as_ref()
            .map(|checkpoints| checkpoints.resume_cursor_for_stream(stream_id, remote_head_seq))
    }

    pub fn rewind_stream_checkpoint(&mut self, stream_id: &str, seq: u64) -> Result<(), String> {
        let checkpoints = self
            .checkpoints
            .as_mut()
            .ok_or_else(|| {
                project_ops_error(
                    ProjectOpsErrorCode::CheckpointConflict,
                    format!("PM checkpoints unavailable for {}", self.checkpoint_path.display()),
                )
            })?;
        checkpoints.rewind_stream(stream_id, seq)?;
        self.last_error = None;
        self.last_action = Some(format!("Rewound {stream_id} checkpoint to seq {seq}"));
        self.load_state = PaneLoadState::Ready;
        Ok(())
    }

    pub fn adopt_remote_checkpoint(&mut self, stream_id: &str, seq: u64) -> Result<bool, String> {
        let checkpoints = self
            .checkpoints
            .as_mut()
            .ok_or_else(|| {
                project_ops_error(
                    ProjectOpsErrorCode::CheckpointConflict,
                    format!("PM checkpoints unavailable for {}", self.checkpoint_path.display()),
                )
            })?;
        let adopted = checkpoints.adopt_checkpoint_if_newer(stream_id, seq)?;
        self.last_error = None;
        self.last_action = Some(if adopted {
            format!("Adopted remote {stream_id} checkpoint seq {seq}")
        } else {
            format!("Ignored stale remote {stream_id} checkpoint seq {seq}")
        });
        self.load_state = PaneLoadState::Ready;
        Ok(adopted)
    }

    pub fn apply_work_items_projection(
        &mut self,
        seq: u64,
        rows: Vec<ProjectOpsWorkItem>,
    ) -> Result<StreamApplyDecision, String> {
        for row in &rows {
            row.validate()?;
        }
        let rows = normalize_work_items(rows);
        let decision = self.apply_stream_seq(PROJECT_OPS_WORK_ITEMS_STREAM_ID, seq)?;
        match &decision {
            StreamApplyDecision::Applied { .. } => {
                persist_work_items(self.work_items_path.as_path(), rows.as_slice())?;
                self.work_items = rows;
                self.last_action = Some(format!(
                    "Applied {} seq {} ({} rows)",
                    PROJECT_OPS_WORK_ITEMS_STREAM_ID,
                    seq,
                    self.work_items.len()
                ));
                self.last_error = None;
                self.load_state = PaneLoadState::Ready;
            }
            StreamApplyDecision::Duplicate { .. } => {
                self.last_action = Some(format!(
                    "Ignored duplicate {} seq {}",
                    PROJECT_OPS_WORK_ITEMS_STREAM_ID, seq
                ));
                self.last_error = None;
                self.load_state = PaneLoadState::Ready;
            }
            StreamApplyDecision::OutOfOrder {
                expected_seq,
                received_seq,
                ..
            } => {
                let message = format!(
                    "Out-of-order {} apply: expected seq {}, received {}",
                    PROJECT_OPS_WORK_ITEMS_STREAM_ID, expected_seq, received_seq
                );
                self.last_action = Some("PM projection apply rejected".to_string());
                self.last_error = Some(project_ops_error(
                    ProjectOpsErrorCode::CheckpointConflict,
                    message,
                ));
                self.load_state = PaneLoadState::Error;
            }
        }
        Ok(decision)
    }

    pub fn apply_activity_projection(
        &mut self,
        seq: u64,
        rows: Vec<ProjectOpsActivityRow>,
    ) -> Result<StreamApplyDecision, String> {
        for row in &rows {
            row.validate()?;
        }
        let rows = normalize_activity_rows(rows);
        let decision = self.apply_stream_seq(PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID, seq)?;
        match &decision {
            StreamApplyDecision::Applied { .. } => {
                persist_activity(self.activity_path.as_path(), rows.as_slice())?;
                self.activity_rows = rows;
                self.last_action = Some(format!(
                    "Applied {} seq {} ({} rows)",
                    PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID,
                    seq,
                    self.activity_rows.len()
                ));
                self.last_error = None;
                self.load_state = PaneLoadState::Ready;
            }
            StreamApplyDecision::Duplicate { .. } => {
                self.last_action = Some(format!(
                    "Ignored duplicate {} seq {}",
                    PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID, seq
                ));
                self.last_error = None;
                self.load_state = PaneLoadState::Ready;
            }
            StreamApplyDecision::OutOfOrder {
                expected_seq,
                received_seq,
                ..
            } => {
                let message = format!(
                    "Out-of-order {} apply: expected seq {}, received {}",
                    PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID, expected_seq, received_seq
                );
                self.last_action = Some("PM projection apply rejected".to_string());
                self.last_error = Some(project_ops_error(
                    ProjectOpsErrorCode::CheckpointConflict,
                    message,
                ));
                self.load_state = PaneLoadState::Error;
            }
        }
        Ok(decision)
    }

    pub fn apply_cycles_projection(
        &mut self,
        seq: u64,
        rows: Vec<ProjectOpsCycleRow>,
    ) -> Result<StreamApplyDecision, String> {
        for row in &rows {
            row.validate()?;
        }
        let rows = normalize_cycles(rows);
        let decision = self.apply_stream_seq(PROJECT_OPS_CYCLES_STREAM_ID, seq)?;
        match &decision {
            StreamApplyDecision::Applied { .. } => {
                persist_cycles(self.cycles_path.as_path(), rows.as_slice())?;
                self.cycles = rows;
                self.last_action = Some(format!(
                    "Applied {} seq {} ({} rows)",
                    PROJECT_OPS_CYCLES_STREAM_ID,
                    seq,
                    self.cycles.len()
                ));
                self.last_error = None;
                self.load_state = PaneLoadState::Ready;
            }
            StreamApplyDecision::Duplicate { .. } => {
                self.last_action = Some(format!(
                    "Ignored duplicate {} seq {}",
                    PROJECT_OPS_CYCLES_STREAM_ID, seq
                ));
                self.last_error = None;
                self.load_state = PaneLoadState::Ready;
            }
            StreamApplyDecision::OutOfOrder {
                expected_seq,
                received_seq,
                ..
            } => {
                let message = format!(
                    "Out-of-order {} apply: expected seq {}, received {}",
                    PROJECT_OPS_CYCLES_STREAM_ID, expected_seq, received_seq
                );
                self.last_action = Some("PM projection apply rejected".to_string());
                self.last_error = Some(project_ops_error(
                    ProjectOpsErrorCode::CheckpointConflict,
                    message,
                ));
                self.load_state = PaneLoadState::Error;
            }
        }
        Ok(decision)
    }

    pub fn apply_saved_views_projection(
        &mut self,
        seq: u64,
        rows: Vec<ProjectOpsSavedViewRow>,
    ) -> Result<StreamApplyDecision, String> {
        for row in &rows {
            row.validate()?;
        }
        let rows = normalize_saved_views(rows);
        let decision = self.apply_stream_seq(PROJECT_OPS_SAVED_VIEWS_STREAM_ID, seq)?;
        match &decision {
            StreamApplyDecision::Applied { .. } => {
                persist_saved_views(self.saved_views_path.as_path(), rows.as_slice())?;
                self.saved_views = rows;
                self.last_action = Some(format!(
                    "Applied {} seq {} ({} rows)",
                    PROJECT_OPS_SAVED_VIEWS_STREAM_ID,
                    seq,
                    self.saved_views.len()
                ));
                self.last_error = None;
                self.load_state = PaneLoadState::Ready;
            }
            StreamApplyDecision::Duplicate { .. } => {
                self.last_action = Some(format!(
                    "Ignored duplicate {} seq {}",
                    PROJECT_OPS_SAVED_VIEWS_STREAM_ID, seq
                ));
                self.last_error = None;
                self.load_state = PaneLoadState::Ready;
            }
            StreamApplyDecision::OutOfOrder {
                expected_seq,
                received_seq,
                ..
            } => {
                let message = format!(
                    "Out-of-order {} apply: expected seq {}, received {}",
                    PROJECT_OPS_SAVED_VIEWS_STREAM_ID, expected_seq, received_seq
                );
                self.last_action = Some("PM projection apply rejected".to_string());
                self.last_error = Some(project_ops_error(
                    ProjectOpsErrorCode::CheckpointConflict,
                    message,
                ));
                self.load_state = PaneLoadState::Error;
            }
        }
        Ok(decision)
    }

    fn apply_stream_seq(
        &mut self,
        stream_id: &str,
        seq: u64,
    ) -> Result<StreamApplyDecision, String> {
        let checkpoints = self
            .checkpoints
            .as_mut()
            .ok_or_else(|| {
                project_ops_error(
                    ProjectOpsErrorCode::CheckpointConflict,
                    format!("PM checkpoints unavailable for {}", self.checkpoint_path.display()),
                )
            })?;
        checkpoints.apply_seq(stream_id, seq)
    }
}

fn openagents_dir() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".openagents")
}

fn work_items_projection_file_path() -> PathBuf {
    openagents_dir().join(WORK_ITEMS_FILE_NAME)
}

fn activity_projection_file_path() -> PathBuf {
    openagents_dir().join(ACTIVITY_FILE_NAME)
}

fn cycles_projection_file_path() -> PathBuf {
    openagents_dir().join(CYCLES_FILE_NAME)
}

fn saved_views_projection_file_path() -> PathBuf {
    openagents_dir().join(SAVED_VIEWS_FILE_NAME)
}

fn normalize_work_items(mut rows: Vec<ProjectOpsWorkItem>) -> Vec<ProjectOpsWorkItem> {
    rows.sort_by(|lhs, rhs| {
        rhs.updated_at_unix_ms
            .cmp(&lhs.updated_at_unix_ms)
            .then_with(|| lhs.work_item_id.as_str().cmp(rhs.work_item_id.as_str()))
    });
    let mut seen_ids = BTreeSet::new();
    rows.retain(|row| seen_ids.insert(row.work_item_id.as_str().to_string()));
    rows
}

fn normalize_activity_rows(mut rows: Vec<ProjectOpsActivityRow>) -> Vec<ProjectOpsActivityRow> {
    rows.sort_by(|lhs, rhs| {
        rhs.occurred_at_unix_ms
            .cmp(&lhs.occurred_at_unix_ms)
            .then_with(|| lhs.event_id.cmp(&rhs.event_id))
    });
    let mut seen_ids = BTreeSet::new();
    rows.retain(|row| seen_ids.insert(row.event_id.clone()));
    rows
}

fn normalize_cycles(mut rows: Vec<ProjectOpsCycleRow>) -> Vec<ProjectOpsCycleRow> {
    rows.sort_by(|lhs, rhs| {
        rhs.is_active
            .cmp(&lhs.is_active)
            .then_with(|| rhs.starts_at_unix_ms.cmp(&lhs.starts_at_unix_ms))
            .then_with(|| lhs.cycle_id.as_str().cmp(rhs.cycle_id.as_str()))
    });
    let mut seen_ids = BTreeSet::new();
    rows.retain(|row| seen_ids.insert(row.cycle_id.as_str().to_string()));
    rows
}

fn normalize_saved_views(mut rows: Vec<ProjectOpsSavedViewRow>) -> Vec<ProjectOpsSavedViewRow> {
    for spec in builtin_saved_view_rows() {
        rows.push(spec);
    }
    rows.sort_by(|lhs, rhs| {
        rhs.built_in
            .cmp(&lhs.built_in)
            .then_with(|| lhs.title.cmp(&rhs.title))
            .then_with(|| lhs.view_id.cmp(&rhs.view_id))
    });
    let mut seen_ids = BTreeSet::new();
    rows.retain(|row| seen_ids.insert(row.view_id.clone()));
    rows
}

fn builtin_saved_view_rows() -> Vec<ProjectOpsSavedViewRow> {
    builtin_saved_view_specs()
        .iter()
        .map(|spec| ProjectOpsSavedViewRow {
            view_id: spec.view_id.to_string(),
            title: spec.title.to_string(),
            query: spec.query.to_string(),
            filters: spec.filters.iter().map(|filter| (*filter).to_string()).collect(),
            built_in: true,
        })
        .collect()
}

fn persist_work_items(path: &Path, rows: &[ProjectOpsWorkItem]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create PM work items projection dir: {error}"))?;
    }
    let document = ProjectOpsWorkItemsProjectionDocumentV1 {
        schema_version: PROJECT_OPS_PROJECTION_SCHEMA_VERSION,
        stream_id: PROJECT_OPS_WORK_ITEMS_STREAM_ID.to_string(),
        rows: normalize_work_items(rows.to_vec()),
    };
    let payload = serde_json::to_string_pretty(&document)
        .map_err(|error| format!("Failed to encode PM work items projection: {error}"))?;
    let temp_path = path.with_extension("tmp");
    fs::write(temp_path.as_path(), payload)
        .map_err(|error| format!("Failed to write PM work items projection temp file: {error}"))?;
    fs::rename(temp_path.as_path(), path)
        .map_err(|error| format!("Failed to persist PM work items projection: {error}"))?;
    Ok(())
}

fn load_or_bootstrap_work_items(path: &Path) -> Result<(Vec<ProjectOpsWorkItem>, bool), String> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            persist_work_items(path, &[])?;
            return Ok((Vec::new(), true));
        }
        Err(error) => return Err(format!("Failed to read PM work items projection: {error}")),
    };
    let document = serde_json::from_str::<ProjectOpsWorkItemsProjectionDocumentV1>(&raw)
        .map_err(|error| format!("Failed to parse PM work items projection: {error}"))?;
    if document.schema_version != PROJECT_OPS_PROJECTION_SCHEMA_VERSION {
        return Err(format!(
            "Unsupported PM work items schema version: {}",
            document.schema_version
        ));
    }
    if document.stream_id != PROJECT_OPS_WORK_ITEMS_STREAM_ID {
        return Err(format!(
            "Unsupported PM work items stream id: {}",
            document.stream_id
        ));
    }
    for row in &document.rows {
        row.validate()?;
    }
    Ok((normalize_work_items(document.rows), false))
}

fn persist_activity(path: &Path, rows: &[ProjectOpsActivityRow]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create PM activity projection dir: {error}"))?;
    }
    let document = ProjectOpsActivityProjectionDocumentV1 {
        schema_version: PROJECT_OPS_PROJECTION_SCHEMA_VERSION,
        stream_id: PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID.to_string(),
        rows: normalize_activity_rows(rows.to_vec()),
    };
    let payload = serde_json::to_string_pretty(&document)
        .map_err(|error| format!("Failed to encode PM activity projection: {error}"))?;
    let temp_path = path.with_extension("tmp");
    fs::write(temp_path.as_path(), payload)
        .map_err(|error| format!("Failed to write PM activity projection temp file: {error}"))?;
    fs::rename(temp_path.as_path(), path)
        .map_err(|error| format!("Failed to persist PM activity projection: {error}"))?;
    Ok(())
}

fn load_or_bootstrap_activity(path: &Path) -> Result<(Vec<ProjectOpsActivityRow>, bool), String> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            persist_activity(path, &[])?;
            return Ok((Vec::new(), true));
        }
        Err(error) => return Err(format!("Failed to read PM activity projection: {error}")),
    };
    let document = serde_json::from_str::<ProjectOpsActivityProjectionDocumentV1>(&raw)
        .map_err(|error| format!("Failed to parse PM activity projection: {error}"))?;
    if document.schema_version != PROJECT_OPS_PROJECTION_SCHEMA_VERSION {
        return Err(format!(
            "Unsupported PM activity schema version: {}",
            document.schema_version
        ));
    }
    if document.stream_id != PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID {
        return Err(format!(
            "Unsupported PM activity stream id: {}",
            document.stream_id
        ));
    }
    for row in &document.rows {
        row.validate()?;
    }
    Ok((normalize_activity_rows(document.rows), false))
}

fn persist_cycles(path: &Path, rows: &[ProjectOpsCycleRow]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create PM cycles projection dir: {error}"))?;
    }
    let document = ProjectOpsCyclesProjectionDocumentV1 {
        schema_version: PROJECT_OPS_PROJECTION_SCHEMA_VERSION,
        stream_id: PROJECT_OPS_CYCLES_STREAM_ID.to_string(),
        rows: normalize_cycles(rows.to_vec()),
    };
    let payload = serde_json::to_string_pretty(&document)
        .map_err(|error| format!("Failed to encode PM cycles projection: {error}"))?;
    let temp_path = path.with_extension("tmp");
    fs::write(temp_path.as_path(), payload)
        .map_err(|error| format!("Failed to write PM cycles projection temp file: {error}"))?;
    fs::rename(temp_path.as_path(), path)
        .map_err(|error| format!("Failed to persist PM cycles projection: {error}"))?;
    Ok(())
}

fn load_or_bootstrap_cycles(path: &Path) -> Result<(Vec<ProjectOpsCycleRow>, bool), String> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            persist_cycles(path, &[])?;
            return Ok((Vec::new(), true));
        }
        Err(error) => return Err(format!("Failed to read PM cycles projection: {error}")),
    };
    let document = serde_json::from_str::<ProjectOpsCyclesProjectionDocumentV1>(&raw)
        .map_err(|error| format!("Failed to parse PM cycles projection: {error}"))?;
    if document.schema_version != PROJECT_OPS_PROJECTION_SCHEMA_VERSION {
        return Err(format!(
            "Unsupported PM cycles schema version: {}",
            document.schema_version
        ));
    }
    if document.stream_id != PROJECT_OPS_CYCLES_STREAM_ID {
        return Err(format!(
            "Unsupported PM cycles stream id: {}",
            document.stream_id
        ));
    }
    for row in &document.rows {
        row.validate()?;
    }
    Ok((normalize_cycles(document.rows), false))
}

fn persist_saved_views(path: &Path, rows: &[ProjectOpsSavedViewRow]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create PM saved views projection dir: {error}"))?;
    }
    let document = ProjectOpsSavedViewsProjectionDocumentV1 {
        schema_version: PROJECT_OPS_PROJECTION_SCHEMA_VERSION,
        stream_id: PROJECT_OPS_SAVED_VIEWS_STREAM_ID.to_string(),
        rows: normalize_saved_views(rows.to_vec()),
    };
    let payload = serde_json::to_string_pretty(&document)
        .map_err(|error| format!("Failed to encode PM saved views projection: {error}"))?;
    let temp_path = path.with_extension("tmp");
    fs::write(temp_path.as_path(), payload)
        .map_err(|error| format!("Failed to write PM saved views projection temp file: {error}"))?;
    fs::rename(temp_path.as_path(), path)
        .map_err(|error| format!("Failed to persist PM saved views projection: {error}"))?;
    Ok(())
}

fn load_or_bootstrap_saved_views(
    path: &Path,
) -> Result<(Vec<ProjectOpsSavedViewRow>, bool), String> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let rows = builtin_saved_view_rows();
            persist_saved_views(path, rows.as_slice())?;
            return Ok((normalize_saved_views(rows), true));
        }
        Err(error) => return Err(format!("Failed to read PM saved views projection: {error}")),
    };
    let document = serde_json::from_str::<ProjectOpsSavedViewsProjectionDocumentV1>(&raw)
        .map_err(|error| format!("Failed to parse PM saved views projection: {error}"))?;
    if document.schema_version != PROJECT_OPS_PROJECTION_SCHEMA_VERSION {
        return Err(format!(
            "Unsupported PM saved views schema version: {}",
            document.schema_version
        ));
    }
    if document.stream_id != PROJECT_OPS_SAVED_VIEWS_STREAM_ID {
        return Err(format!(
            "Unsupported PM saved views stream id: {}",
            document.stream_id
        ));
    }
    for row in &document.rows {
        row.validate()?;
    }
    let normalized = normalize_saved_views(document.rows);
    persist_saved_views(path, normalized.as_slice())?;
    Ok((normalized, false))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::Value;

    use super::{
        ProjectOpsActivityRow, ProjectOpsCycleRow, ProjectOpsProjectionStore,
        ProjectOpsSavedViewRow, PROJECT_OPS_PROJECTION_SCHEMA_VERSION,
    };
    use crate::app_state::PaneLoadState;
    use crate::project_ops::contract::{
        ProjectOpsAcceptedEventName, PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID,
        PROJECT_OPS_CYCLES_STREAM_ID, PROJECT_OPS_SAVED_VIEWS_STREAM_ID,
        PROJECT_OPS_WORK_ITEMS_STREAM_ID,
    };
    use crate::project_ops::schema::{
        ProjectOpsCycleId, ProjectOpsPriority, ProjectOpsTeamKey, ProjectOpsWorkItem,
        ProjectOpsWorkItemId, ProjectOpsWorkItemStatus,
    };
    use crate::sync_apply::StreamApplyDecision;

    fn unique_temp_path(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        std::env::temp_dir().join(format!("openagents-project-ops-{name}-{nanos}.json"))
    }

    fn sample_work_item() -> ProjectOpsWorkItem {
        ProjectOpsWorkItem {
            work_item_id: ProjectOpsWorkItemId::new("wi-1").expect("work item id"),
            title: "Ship PM projection docs".to_string(),
            description: "Persist Step 0 PM projections under ~/.openagents.".to_string(),
            status: ProjectOpsWorkItemStatus::InProgress,
            priority: ProjectOpsPriority::High,
            assignee: Some("cdavid".to_string()),
            team_key: ProjectOpsTeamKey::new("desktop").expect("team key"),
            cycle_id: Some(ProjectOpsCycleId::new("2026-w10").expect("cycle id")),
            parent_id: None,
            area_tags: vec!["pm".to_string()],
            blocked_reason: None,
            due_at_unix_ms: None,
            created_at_unix_ms: 1_762_000_000_000,
            updated_at_unix_ms: 1_762_000_100_000,
            archived_at_unix_ms: None,
        }
    }

    fn sample_activity_row() -> ProjectOpsActivityRow {
        ProjectOpsActivityRow {
            event_id: "pm:activity:1".to_string(),
            work_item_id: ProjectOpsWorkItemId::new("wi-1").expect("work item id"),
            event_name: ProjectOpsAcceptedEventName::WorkItemStatusChanged,
            summary: "Moved wi-1 into in_progress".to_string(),
            actor_label: "cdavid".to_string(),
            command_id: "cmd-1".to_string(),
            occurred_at_unix_ms: 1_762_000_200_000,
        }
    }

    fn sample_cycle_row() -> ProjectOpsCycleRow {
        ProjectOpsCycleRow {
            cycle_id: ProjectOpsCycleId::new("2026-w10").expect("cycle id"),
            title: "Week 10".to_string(),
            goal: Some("Land the PM thin slice".to_string()),
            starts_at_unix_ms: 1_761_998_400_000,
            ends_at_unix_ms: 1_762_603_200_000,
            is_active: true,
        }
    }

    fn sample_saved_view_row() -> ProjectOpsSavedViewRow {
        ProjectOpsSavedViewRow {
            view_id: "focus".to_string(),
            title: "Focus".to_string(),
            query: "priority:high".to_string(),
            filters: vec!["priority:high".to_string(), "status:active".to_string()],
            built_in: false,
        }
    }

    #[test]
    fn bootstraps_projection_docs_and_checkpoint_rows() {
        let work_items_path = unique_temp_path("work-items");
        let activity_path = unique_temp_path("activity");
        let cycles_path = unique_temp_path("cycles");
        let saved_views_path = unique_temp_path("saved-views");
        let checkpoint_path = unique_temp_path("checkpoints");

        let store = ProjectOpsProjectionStore::from_paths_for_tests(
            work_items_path.clone(),
            activity_path.clone(),
            cycles_path.clone(),
            saved_views_path.clone(),
            checkpoint_path,
        );
        assert_eq!(store.load_state, PaneLoadState::Ready);
        assert_eq!(store.checkpoint_for(PROJECT_OPS_WORK_ITEMS_STREAM_ID), Some(0));
        assert_eq!(
            store.checkpoint_for(PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID),
            Some(0)
        );
        assert_eq!(store.checkpoint_for(PROJECT_OPS_CYCLES_STREAM_ID), Some(0));
        assert_eq!(store.checkpoint_for(PROJECT_OPS_SAVED_VIEWS_STREAM_ID), Some(0));
        assert_eq!(store.saved_views.len(), 5);

        for (path, stream_id) in [
            (&work_items_path, PROJECT_OPS_WORK_ITEMS_STREAM_ID),
            (&activity_path, PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID),
            (&cycles_path, PROJECT_OPS_CYCLES_STREAM_ID),
            (&saved_views_path, PROJECT_OPS_SAVED_VIEWS_STREAM_ID),
        ] {
            let raw = std::fs::read_to_string(path).expect("projection doc should exist");
            let json = serde_json::from_str::<Value>(raw.as_str()).expect("json should parse");
            assert_eq!(
                json.get("schema_version").and_then(Value::as_u64),
                Some(PROJECT_OPS_PROJECTION_SCHEMA_VERSION as u64)
            );
            assert_eq!(json.get("stream_id").and_then(Value::as_str), Some(stream_id));
        }
    }

    #[test]
    fn apply_projection_updates_persist_and_reload() {
        let work_items_path = unique_temp_path("persist-work-items");
        let activity_path = unique_temp_path("persist-activity");
        let cycles_path = unique_temp_path("persist-cycles");
        let saved_views_path = unique_temp_path("persist-saved-views");
        let checkpoint_path = unique_temp_path("persist-checkpoints");

        let mut store = ProjectOpsProjectionStore::from_paths_for_tests(
            work_items_path.clone(),
            activity_path.clone(),
            cycles_path.clone(),
            saved_views_path.clone(),
            checkpoint_path.clone(),
        );

        assert!(matches!(
            store
                .apply_work_items_projection(1, vec![sample_work_item()])
                .expect("work items should apply"),
            StreamApplyDecision::Applied { .. }
        ));
        assert!(matches!(
            store
                .apply_activity_projection(1, vec![sample_activity_row()])
                .expect("activity should apply"),
            StreamApplyDecision::Applied { .. }
        ));
        assert!(matches!(
            store
                .apply_cycles_projection(1, vec![sample_cycle_row()])
                .expect("cycles should apply"),
            StreamApplyDecision::Applied { .. }
        ));
        assert!(matches!(
            store
                .apply_saved_views_projection(1, vec![sample_saved_view_row()])
                .expect("saved views should apply"),
            StreamApplyDecision::Applied { .. }
        ));

        let reloaded = ProjectOpsProjectionStore::from_paths_for_tests(
            work_items_path,
            activity_path,
            cycles_path,
            saved_views_path,
            checkpoint_path,
        );
        assert_eq!(reloaded.work_items.len(), 1);
        assert_eq!(reloaded.activity_rows.len(), 1);
        assert_eq!(reloaded.cycles.len(), 1);
        assert_eq!(reloaded.saved_views.len(), 6);
        assert_eq!(reloaded.checkpoint_for(PROJECT_OPS_WORK_ITEMS_STREAM_ID), Some(1));
        assert_eq!(
            reloaded.checkpoint_for(PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID),
            Some(1)
        );
        assert_eq!(reloaded.checkpoint_for(PROJECT_OPS_CYCLES_STREAM_ID), Some(1));
        assert_eq!(reloaded.checkpoint_for(PROJECT_OPS_SAVED_VIEWS_STREAM_ID), Some(1));
    }

    #[test]
    fn duplicate_and_out_of_order_sequences_are_reported_without_overwriting() {
        let work_items_path = unique_temp_path("dupe-work-items");
        let activity_path = unique_temp_path("dupe-activity");
        let cycles_path = unique_temp_path("dupe-cycles");
        let saved_views_path = unique_temp_path("dupe-saved-views");
        let checkpoint_path = unique_temp_path("dupe-checkpoints");

        let mut store = ProjectOpsProjectionStore::from_paths_for_tests(
            work_items_path,
            activity_path,
            cycles_path,
            saved_views_path,
            checkpoint_path,
        );

        assert!(matches!(
            store
                .apply_work_items_projection(1, vec![sample_work_item()])
                .expect("first apply should succeed"),
            StreamApplyDecision::Applied { .. }
        ));
        assert!(matches!(
            store
                .apply_work_items_projection(1, vec![sample_work_item()])
                .expect("duplicate should be reported"),
            StreamApplyDecision::Duplicate { .. }
        ));
        assert!(matches!(
            store
                .apply_work_items_projection(3, vec![sample_work_item()])
                .expect("out of order should be reported"),
            StreamApplyDecision::OutOfOrder { .. }
        ));
        assert_eq!(store.load_state, PaneLoadState::Error);
        assert!(
            store
                .last_error
                .as_deref()
                .is_some_and(|error| {
                    error.contains("project_ops.checkpoint_conflict:")
                        && error.contains("Out-of-order")
                })
        );
        assert_eq!(store.work_items.len(), 1);
    }

    #[test]
    fn checkpoint_recovery_helpers_match_sync_apply_contract() {
        let work_items_path = unique_temp_path("recover-work-items");
        let activity_path = unique_temp_path("recover-activity");
        let cycles_path = unique_temp_path("recover-cycles");
        let saved_views_path = unique_temp_path("recover-saved-views");
        let checkpoint_path = unique_temp_path("recover-checkpoints");

        let mut store = ProjectOpsProjectionStore::from_paths_for_tests(
            work_items_path,
            activity_path,
            cycles_path,
            saved_views_path,
            checkpoint_path.clone(),
        );

        assert!(matches!(
            store
                .apply_work_items_projection(1, vec![sample_work_item()])
                .expect("first PM seq should apply"),
            StreamApplyDecision::Applied { .. }
        ));
        assert_eq!(store.max_checkpoint_seq(), 1);
        assert_eq!(
            store.resume_cursor_for_stream(PROJECT_OPS_WORK_ITEMS_STREAM_ID, Some(20_000)),
            Some(10_000)
        );

        assert!(
            store
                .adopt_remote_checkpoint(PROJECT_OPS_CYCLES_STREAM_ID, 4)
                .expect("remote checkpoint should adopt")
        );
        assert_eq!(store.checkpoint_for(PROJECT_OPS_CYCLES_STREAM_ID), Some(4));

        store
            .rewind_stream_checkpoint(PROJECT_OPS_CYCLES_STREAM_ID, 2)
            .expect("rewind should persist");
        assert_eq!(store.checkpoint_for(PROJECT_OPS_CYCLES_STREAM_ID), Some(2));

        let reloaded = ProjectOpsProjectionStore::from_paths_for_tests(
            unique_temp_path("recover-work-items-reload-unused"),
            unique_temp_path("recover-activity-reload-unused"),
            unique_temp_path("recover-cycles-reload-unused"),
            unique_temp_path("recover-saved-views-reload-unused"),
            checkpoint_path,
        );
        assert_eq!(reloaded.checkpoint_for(PROJECT_OPS_WORK_ITEMS_STREAM_ID), Some(1));
        assert_eq!(reloaded.checkpoint_for(PROJECT_OPS_CYCLES_STREAM_ID), Some(2));
    }
}
