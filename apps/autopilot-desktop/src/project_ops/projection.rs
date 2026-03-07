use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::app_state::PaneLoadState;
use crate::project_ops::contract::{
    project_ops_error, ProjectOpsAcceptedEventName, ProjectOpsErrorCode,
    PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID, PROJECT_OPS_CYCLES_STREAM_ID,
    PROJECT_OPS_PRIMARY_SOURCE_BADGE, PROJECT_OPS_SAVED_VIEWS_STREAM_ID,
    PROJECT_OPS_TEAMS_STREAM_ID, PROJECT_OPS_WORK_ITEMS_STREAM_ID,
};
use crate::project_ops::schema::{
    ProjectOpsCycleId, ProjectOpsTeamKey, ProjectOpsWorkItem, ProjectOpsWorkItemId,
};
use crate::project_ops::views::builtin_saved_view_specs;
use crate::sync_apply::{StreamApplyDecision, SyncApplyEngine, SyncApplyPolicy};

pub const PROJECT_OPS_PROJECTION_SCHEMA_VERSION: u16 = 1;

const WORK_ITEMS_FILE_NAME: &str = "autopilot-pm-work-items-projection-v1.json";
const ACTIVITY_FILE_NAME: &str = "autopilot-pm-activity-projection-v1.json";
const CYCLES_FILE_NAME: &str = "autopilot-pm-cycles-v1.json";
const SAVED_VIEWS_FILE_NAME: &str = "autopilot-pm-saved-views-v1.json";
const TEAMS_FILE_NAME: &str = "autopilot-pm-teams-v1.json";

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
            return Err(
                "project ops cycle ends_at_unix_ms must be >= starts_at_unix_ms".to_string(),
            );
        }
        if self
            .goal
            .as_deref()
            .is_some_and(|goal| goal.trim().is_empty())
        {
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
pub struct ProjectOpsTeamRow {
    pub team_key: ProjectOpsTeamKey,
    pub title: String,
    pub summary: Option<String>,
    pub default_saved_view_id: String,
    pub default_cycle_id: Option<ProjectOpsCycleId>,
    pub default_area_tags: Vec<String>,
    pub is_default: bool,
}

impl ProjectOpsTeamRow {
    pub fn validate(&self) -> Result<(), String> {
        if self.title.trim().is_empty() {
            return Err("project ops team title must not be empty".to_string());
        }
        if self
            .summary
            .as_deref()
            .is_some_and(|summary| summary.trim().is_empty())
        {
            return Err("project ops team summary must not be blank when present".to_string());
        }
        if self.default_saved_view_id.trim().is_empty() {
            return Err("project ops team default_saved_view_id must not be empty".to_string());
        }
        if self.default_area_tags.len() > 2 {
            return Err("project ops team default_area_tags supports at most two values".to_string());
        }
        if self
            .default_area_tags
            .iter()
            .any(|tag| tag.trim().is_empty())
        {
            return Err("project ops team default_area_tags must not contain blanks".to_string());
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProjectOpsTeamsProjectionDocumentV1 {
    schema_version: u16,
    stream_id: String,
    rows: Vec<ProjectOpsTeamRow>,
}

pub struct ProjectOpsProjectionStore {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub work_items: Vec<ProjectOpsWorkItem>,
    pub activity_rows: Vec<ProjectOpsActivityRow>,
    pub cycles: Vec<ProjectOpsCycleRow>,
    pub saved_views: Vec<ProjectOpsSavedViewRow>,
    pub teams: Vec<ProjectOpsTeamRow>,
    work_items_path: PathBuf,
    activity_path: PathBuf,
    cycles_path: PathBuf,
    saved_views_path: PathBuf,
    teams_path: PathBuf,
    checkpoint_path: PathBuf,
    checkpoints: Option<SyncApplyEngine>,
}

impl ProjectOpsProjectionStore {
    pub fn disabled() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some(
                "Project Ops local projections idle until the feature gate is enabled".to_string(),
            ),
            work_items: Vec::new(),
            activity_rows: Vec::new(),
            cycles: Vec::new(),
            saved_views: Vec::new(),
            teams: Vec::new(),
            work_items_path: PathBuf::new(),
            activity_path: PathBuf::new(),
            cycles_path: PathBuf::new(),
            saved_views_path: PathBuf::new(),
            teams_path: PathBuf::new(),
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
            teams_projection_file_path(),
            SyncApplyEngine::default_checkpoint_path(),
        )
    }

    #[cfg(test)]
    pub(crate) fn from_paths_for_tests(
        work_items_path: PathBuf,
        activity_path: PathBuf,
        cycles_path: PathBuf,
        saved_views_path: PathBuf,
        teams_path: PathBuf,
        checkpoint_path: PathBuf,
    ) -> Self {
        Self::from_paths(
            work_items_path,
            activity_path,
            cycles_path,
            saved_views_path,
            teams_path,
            checkpoint_path,
        )
    }

    fn from_paths(
        work_items_path: PathBuf,
        activity_path: PathBuf,
        cycles_path: PathBuf,
        saved_views_path: PathBuf,
        teams_path: PathBuf,
        checkpoint_path: PathBuf,
    ) -> Self {
        match Self::load_or_bootstrap(
            work_items_path.clone(),
            activity_path.clone(),
            cycles_path.clone(),
            saved_views_path.clone(),
            teams_path.clone(),
            checkpoint_path.clone(),
        ) {
            Ok((
                checkpoints,
                work_items,
                activity_rows,
                cycles,
                saved_views,
                teams,
                bootstrapped,
            )) => {
                let last_action = if bootstrapped.is_empty() {
                    Some(format!(
                        "Loaded PM projection streams ({} work items / {} activity / {} cycles / {} views / {} teams)",
                        work_items.len(),
                        activity_rows.len(),
                        cycles.len(),
                        saved_views.len(),
                        teams.len(),
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
                    teams,
                    work_items_path,
                    activity_path,
                    cycles_path,
                    saved_views_path,
                    teams_path,
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
                teams: Vec::new(),
                work_items_path,
                activity_path,
                cycles_path,
                saved_views_path,
                teams_path,
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
        teams_path: PathBuf,
        checkpoint_path: PathBuf,
    ) -> Result<
        (
            SyncApplyEngine,
            Vec<ProjectOpsWorkItem>,
            Vec<ProjectOpsActivityRow>,
            Vec<ProjectOpsCycleRow>,
            Vec<ProjectOpsSavedViewRow>,
            Vec<ProjectOpsTeamRow>,
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
        if checkpoints.ensure_stream_registered(PROJECT_OPS_TEAMS_STREAM_ID)? {
            bootstrapped.push(PROJECT_OPS_TEAMS_STREAM_ID);
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
        let (teams, teams_bootstrapped) =
            load_or_bootstrap_teams(teams_path.as_path(), work_items.as_slice(), cycles.as_slice())?;
        if teams_bootstrapped {
            bootstrapped.push(TEAMS_FILE_NAME);
        }

        Ok((
            checkpoints,
            work_items,
            activity_rows,
            cycles,
            saved_views,
            teams,
            bootstrapped,
        ))
    }

    pub fn source_badge(&self) -> String {
        PROJECT_OPS_PRIMARY_SOURCE_BADGE.to_string()
    }

    pub fn default_team(&self) -> Option<&ProjectOpsTeamRow> {
        self.teams
            .iter()
            .find(|team| team.is_default)
            .or_else(|| self.teams.first())
    }

    pub fn team_for_key(&self, team_key: &ProjectOpsTeamKey) -> Option<&ProjectOpsTeamRow> {
        self.teams.iter().find(|team| &team.team_key == team_key)
    }

    pub fn checkpoint_for(&self, stream_id: &str) -> Option<u64> {
        self.checkpoints
            .as_ref()
            .and_then(|checkpoints| checkpoints.checkpoint_for(stream_id))
    }

    pub fn reload_shared_checkpoints(&mut self) -> Result<(), String> {
        let checkpoints =
            SyncApplyEngine::load_or_new(self.checkpoint_path.clone(), SyncApplyPolicy::default())?;
        self.checkpoints = Some(checkpoints);
        Ok(())
    }

    pub fn max_checkpoint_seq(&self) -> u64 {
        self.checkpoints
            .as_ref()
            .map_or(0, SyncApplyEngine::max_checkpoint_seq)
    }

    pub fn resume_cursor_for_stream(
        &self,
        stream_id: &str,
        remote_head_seq: Option<u64>,
    ) -> Option<u64> {
        self.checkpoints
            .as_ref()
            .map(|checkpoints| checkpoints.resume_cursor_for_stream(stream_id, remote_head_seq))
    }

    pub fn rewind_stream_checkpoint(&mut self, stream_id: &str, seq: u64) -> Result<(), String> {
        self.reload_shared_checkpoints()?;
        let checkpoints = self.checkpoints.as_mut().ok_or_else(|| {
            project_ops_error(
                ProjectOpsErrorCode::CheckpointConflict,
                format!(
                    "PM checkpoints unavailable for {}",
                    self.checkpoint_path.display()
                ),
            )
        })?;
        checkpoints.rewind_stream(stream_id, seq)?;
        self.last_error = None;
        self.last_action = Some(format!("Rewound {stream_id} checkpoint to seq {seq}"));
        self.load_state = PaneLoadState::Ready;
        Ok(())
    }

    pub fn adopt_remote_checkpoint(&mut self, stream_id: &str, seq: u64) -> Result<bool, String> {
        self.reload_shared_checkpoints()?;
        let checkpoints = self.checkpoints.as_mut().ok_or_else(|| {
            project_ops_error(
                ProjectOpsErrorCode::CheckpointConflict,
                format!(
                    "PM checkpoints unavailable for {}",
                    self.checkpoint_path.display()
                ),
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

    pub fn apply_teams_projection(
        &mut self,
        seq: u64,
        rows: Vec<ProjectOpsTeamRow>,
    ) -> Result<StreamApplyDecision, String> {
        for row in &rows {
            row.validate()?;
        }
        let rows = normalize_teams(rows);
        let decision = self.apply_stream_seq(PROJECT_OPS_TEAMS_STREAM_ID, seq)?;
        match &decision {
            StreamApplyDecision::Applied { .. } => {
                persist_teams(self.teams_path.as_path(), rows.as_slice())?;
                self.teams = rows;
                self.last_action = Some(format!(
                    "Applied {} seq {} ({} rows)",
                    PROJECT_OPS_TEAMS_STREAM_ID,
                    seq,
                    self.teams.len()
                ));
                self.last_error = None;
                self.load_state = PaneLoadState::Ready;
            }
            StreamApplyDecision::Duplicate { .. } => {
                self.last_action = Some(format!(
                    "Ignored duplicate {} seq {}",
                    PROJECT_OPS_TEAMS_STREAM_ID, seq
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
                    PROJECT_OPS_TEAMS_STREAM_ID, expected_seq, received_seq
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

    pub fn upsert_personal_saved_view(
        &mut self,
        row: ProjectOpsSavedViewRow,
    ) -> Result<bool, String> {
        if row.built_in {
            return Err(project_ops_error(
                ProjectOpsErrorCode::InvalidCommand,
                format!("saved view {} must not be marked built-in", row.view_id),
            ));
        }
        row.validate()?;
        let mut next_rows = self
            .saved_views
            .iter()
            .filter(|saved_view| !saved_view.built_in && saved_view.view_id != row.view_id)
            .cloned()
            .collect::<Vec<_>>();
        next_rows.push(row);
        let seq = self
            .checkpoint_for(PROJECT_OPS_SAVED_VIEWS_STREAM_ID)
            .unwrap_or(0)
            .saturating_add(1);
        Ok(matches!(
            self.apply_saved_views_projection(seq, next_rows)?,
            StreamApplyDecision::Applied { .. }
        ))
    }

    pub fn remove_personal_saved_view(&mut self, view_id: &str) -> Result<bool, String> {
        let normalized = view_id.trim();
        if normalized.is_empty() {
            return Ok(false);
        }
        let next_rows = self
            .saved_views
            .iter()
            .filter(|saved_view| !saved_view.built_in && saved_view.view_id != normalized)
            .cloned()
            .collect::<Vec<_>>();
        let current_custom_count = self
            .saved_views
            .iter()
            .filter(|view| !view.built_in)
            .count();
        if next_rows.len() == current_custom_count {
            return Ok(false);
        }
        let seq = self
            .checkpoint_for(PROJECT_OPS_SAVED_VIEWS_STREAM_ID)
            .unwrap_or(0)
            .saturating_add(1);
        Ok(matches!(
            self.apply_saved_views_projection(seq, next_rows)?,
            StreamApplyDecision::Applied { .. }
        ))
    }

    pub fn upsert_team(&mut self, row: ProjectOpsTeamRow) -> Result<bool, String> {
        row.validate()?;
        let mut next_rows = self
            .teams
            .iter()
            .filter(|team| team.team_key != row.team_key)
            .cloned()
            .collect::<Vec<_>>();
        next_rows.push(row);
        let seq = self
            .checkpoint_for(PROJECT_OPS_TEAMS_STREAM_ID)
            .unwrap_or(0)
            .saturating_add(1);
        Ok(matches!(
            self.apply_teams_projection(seq, next_rows)?,
            StreamApplyDecision::Applied { .. }
        ))
    }

    pub fn remove_team(&mut self, team_key: &ProjectOpsTeamKey) -> Result<bool, String> {
        let next_rows = self
            .teams
            .iter()
            .filter(|team| &team.team_key != team_key)
            .cloned()
            .collect::<Vec<_>>();
        if next_rows.len() == self.teams.len() || next_rows.is_empty() {
            return Ok(false);
        }
        let seq = self
            .checkpoint_for(PROJECT_OPS_TEAMS_STREAM_ID)
            .unwrap_or(0)
            .saturating_add(1);
        Ok(matches!(
            self.apply_teams_projection(seq, next_rows)?,
            StreamApplyDecision::Applied { .. }
        ))
    }

    fn apply_stream_seq(
        &mut self,
        stream_id: &str,
        seq: u64,
    ) -> Result<StreamApplyDecision, String> {
        self.reload_shared_checkpoints()?;
        let checkpoints = self.checkpoints.as_mut().ok_or_else(|| {
            project_ops_error(
                ProjectOpsErrorCode::CheckpointConflict,
                format!(
                    "PM checkpoints unavailable for {}",
                    self.checkpoint_path.display()
                ),
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

fn teams_projection_file_path() -> PathBuf {
    openagents_dir().join(TEAMS_FILE_NAME)
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

fn normalize_teams(mut rows: Vec<ProjectOpsTeamRow>) -> Vec<ProjectOpsTeamRow> {
    rows.sort_by(|lhs, rhs| {
        rhs.is_default
            .cmp(&lhs.is_default)
            .then_with(|| lhs.title.cmp(&rhs.title))
            .then_with(|| lhs.team_key.as_str().cmp(rhs.team_key.as_str()))
    });
    let mut seen_keys = BTreeSet::new();
    rows.retain(|row| seen_keys.insert(row.team_key.as_str().to_string()));
    let mut default_index = None;
    for (index, row) in rows.iter_mut().enumerate() {
        if row.is_default && default_index.is_none() {
            default_index = Some(index);
        } else {
            row.is_default = false;
        }
    }
    if let Some(index) = default_index {
        rows[index].is_default = true;
    } else if let Some(first) = rows.first_mut() {
        first.is_default = true;
    }
    rows
}

fn builtin_saved_view_rows() -> Vec<ProjectOpsSavedViewRow> {
    builtin_saved_view_specs()
        .iter()
        .map(|spec| ProjectOpsSavedViewRow {
            view_id: spec.view_id.to_string(),
            title: spec.title.to_string(),
            query: spec.query.to_string(),
            filters: spec
                .filters
                .iter()
                .map(|filter| (*filter).to_string())
                .collect(),
            built_in: true,
        })
        .collect()
}

fn bootstrap_team_rows(
    work_items: &[ProjectOpsWorkItem],
    cycles: &[ProjectOpsCycleRow],
) -> Vec<ProjectOpsTeamRow> {
    let active_cycle_id = cycles
        .iter()
        .find(|cycle| cycle.is_active)
        .map(|cycle| cycle.cycle_id.clone());
    let mut team_keys = work_items
        .iter()
        .map(|item| item.team_key.clone())
        .collect::<Vec<_>>();
    if team_keys.is_empty() {
        team_keys.push(ProjectOpsTeamKey::new("desktop").expect("static team key"));
    }
    team_keys.sort_by(|lhs, rhs| lhs.as_str().cmp(rhs.as_str()));
    team_keys.dedup_by(|lhs, rhs| lhs.as_str() == rhs.as_str());

    let mut rows = team_keys
        .into_iter()
        .map(|team_key| {
            let team_items = work_items
                .iter()
                .filter(|item| item.team_key == team_key)
                .collect::<Vec<_>>();
            let active_count = team_items
                .iter()
                .filter(|item| !item.status.is_terminal())
                .count();
            let default_cycle_id = active_cycle_id.clone().or_else(|| {
                team_items
                    .iter()
                    .find_map(|item| item.cycle_id.clone())
            });
            let default_saved_view_id = if default_cycle_id.is_some() {
                "current-cycle"
            } else if team_items
                .iter()
                .any(|item| item.status == crate::project_ops::schema::ProjectOpsWorkItemStatus::Backlog)
            {
                "backlog"
            } else {
                "my-work"
            };
            let default_area_tags = team_items
                .iter()
                .find_map(|item| {
                    (!item.area_tags.is_empty()).then(|| {
                        item.area_tags
                            .iter()
                            .take(2)
                            .cloned()
                            .collect::<Vec<_>>()
                    })
                })
                .unwrap_or_else(|| vec!["pm".to_string()]);
            ProjectOpsTeamRow {
                title: humanize_team_title(team_key.as_str()),
                summary: Some(format!(
                    "{} active items / {} total items",
                    active_count,
                    team_items.len()
                )),
                default_saved_view_id: default_saved_view_id.to_string(),
                default_cycle_id,
                default_area_tags,
                is_default: team_key.as_str() == "desktop",
                team_key,
            }
        })
        .collect::<Vec<_>>();

    if !rows.iter().any(|row| row.is_default) {
        if let Some(first) = rows.first_mut() {
            first.is_default = true;
        }
    }
    normalize_teams(rows)
}

fn humanize_team_title(team_key: &str) -> String {
    team_key
        .split(['-', '_'])
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => {
                    let mut title = first.to_ascii_uppercase().to_string();
                    title.push_str(&chars.as_str().to_ascii_lowercase());
                    title
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
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

fn persist_teams(path: &Path, rows: &[ProjectOpsTeamRow]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create PM teams projection dir: {error}"))?;
    }
    let document = ProjectOpsTeamsProjectionDocumentV1 {
        schema_version: PROJECT_OPS_PROJECTION_SCHEMA_VERSION,
        stream_id: PROJECT_OPS_TEAMS_STREAM_ID.to_string(),
        rows: normalize_teams(rows.to_vec()),
    };
    let payload = serde_json::to_string_pretty(&document)
        .map_err(|error| format!("Failed to encode PM teams projection: {error}"))?;
    let temp_path = path.with_extension("tmp");
    fs::write(temp_path.as_path(), payload)
        .map_err(|error| format!("Failed to write PM teams projection temp file: {error}"))?;
    fs::rename(temp_path.as_path(), path)
        .map_err(|error| format!("Failed to persist PM teams projection: {error}"))?;
    Ok(())
}

fn load_or_bootstrap_teams(
    path: &Path,
    work_items: &[ProjectOpsWorkItem],
    cycles: &[ProjectOpsCycleRow],
) -> Result<(Vec<ProjectOpsTeamRow>, bool), String> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let rows = bootstrap_team_rows(work_items, cycles);
            persist_teams(path, rows.as_slice())?;
            return Ok((rows, true));
        }
        Err(error) => return Err(format!("Failed to read PM teams projection: {error}")),
    };
    let document = serde_json::from_str::<ProjectOpsTeamsProjectionDocumentV1>(&raw)
        .map_err(|error| format!("Failed to parse PM teams projection: {error}"))?;
    if document.schema_version != PROJECT_OPS_PROJECTION_SCHEMA_VERSION {
        return Err(format!(
            "Unsupported PM teams schema version: {}",
            document.schema_version
        ));
    }
    if document.stream_id != PROJECT_OPS_TEAMS_STREAM_ID {
        return Err(format!(
            "Unsupported PM teams stream id: {}",
            document.stream_id
        ));
    }
    for row in &document.rows {
        row.validate()?;
    }
    let normalized = normalize_teams(document.rows);
    persist_teams(path, normalized.as_slice())?;
    Ok((normalized, false))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::Value;

    use super::{
        ProjectOpsActivityRow, ProjectOpsCycleRow, ProjectOpsProjectionStore,
        ProjectOpsSavedViewRow, ProjectOpsTeamRow, PROJECT_OPS_PROJECTION_SCHEMA_VERSION,
    };
    use crate::app_state::PaneLoadState;
    use crate::project_ops::contract::{
        ProjectOpsAcceptedEventName, PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID,
        PROJECT_OPS_CYCLES_STREAM_ID, PROJECT_OPS_SAVED_VIEWS_STREAM_ID,
        PROJECT_OPS_TEAMS_STREAM_ID, PROJECT_OPS_WORK_ITEMS_STREAM_ID,
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

    fn sample_team_row() -> ProjectOpsTeamRow {
        ProjectOpsTeamRow {
            team_key: ProjectOpsTeamKey::new("ops").expect("team key"),
            title: "Ops".to_string(),
            summary: Some("1 active items / 1 total items".to_string()),
            default_saved_view_id: "current-cycle".to_string(),
            default_cycle_id: Some(ProjectOpsCycleId::new("2026-w10").expect("cycle id")),
            default_area_tags: vec!["pm".to_string()],
            is_default: false,
        }
    }

    #[test]
    fn bootstraps_projection_docs_and_checkpoint_rows() {
        let work_items_path = unique_temp_path("work-items");
        let activity_path = unique_temp_path("activity");
        let cycles_path = unique_temp_path("cycles");
        let saved_views_path = unique_temp_path("saved-views");
        let teams_path = unique_temp_path("teams");
        let checkpoint_path = unique_temp_path("checkpoints");

        let store = ProjectOpsProjectionStore::from_paths_for_tests(
            work_items_path.clone(),
            activity_path.clone(),
            cycles_path.clone(),
            saved_views_path.clone(),
            teams_path.clone(),
            checkpoint_path,
        );
        assert_eq!(store.load_state, PaneLoadState::Ready);
        assert_eq!(
            store.checkpoint_for(PROJECT_OPS_WORK_ITEMS_STREAM_ID),
            Some(0)
        );
        assert_eq!(
            store.checkpoint_for(PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID),
            Some(0)
        );
        assert_eq!(store.checkpoint_for(PROJECT_OPS_CYCLES_STREAM_ID), Some(0));
        assert_eq!(
            store.checkpoint_for(PROJECT_OPS_SAVED_VIEWS_STREAM_ID),
            Some(0)
        );
        assert_eq!(store.checkpoint_for(PROJECT_OPS_TEAMS_STREAM_ID), Some(0));
        assert_eq!(store.saved_views.len(), 5);
        assert_eq!(store.teams.len(), 1);

        for (path, stream_id) in [
            (&work_items_path, PROJECT_OPS_WORK_ITEMS_STREAM_ID),
            (&activity_path, PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID),
            (&cycles_path, PROJECT_OPS_CYCLES_STREAM_ID),
            (&saved_views_path, PROJECT_OPS_SAVED_VIEWS_STREAM_ID),
            (&teams_path, PROJECT_OPS_TEAMS_STREAM_ID),
        ] {
            let raw = std::fs::read_to_string(path).expect("projection doc should exist");
            let json = serde_json::from_str::<Value>(raw.as_str()).expect("json should parse");
            assert_eq!(
                json.get("schema_version").and_then(Value::as_u64),
                Some(PROJECT_OPS_PROJECTION_SCHEMA_VERSION as u64)
            );
            assert_eq!(
                json.get("stream_id").and_then(Value::as_str),
                Some(stream_id)
            );
        }
    }

    #[test]
    fn unsupported_projection_schema_versions_surface_explicit_rebuild_errors() {
        let work_items_path = unique_temp_path("unsupported-work-items");
        let activity_path = unique_temp_path("unsupported-activity");
        let cycles_path = unique_temp_path("unsupported-cycles");
        let saved_views_path = unique_temp_path("unsupported-saved-views");
        let teams_path = unique_temp_path("unsupported-teams");
        let checkpoint_path = unique_temp_path("unsupported-checkpoints");

        if let Some(parent) = work_items_path.parent() {
            std::fs::create_dir_all(parent).expect("projection dir should exist");
        }
        std::fs::write(
            &work_items_path,
            serde_json::json!({
                "schema_version": PROJECT_OPS_PROJECTION_SCHEMA_VERSION + 1,
                "stream_id": PROJECT_OPS_WORK_ITEMS_STREAM_ID,
                "rows": [],
            })
            .to_string(),
        )
        .expect("unsupported work-items projection should write");

        let store = ProjectOpsProjectionStore::from_paths_for_tests(
            work_items_path,
            activity_path,
            cycles_path,
            saved_views_path,
            teams_path,
            checkpoint_path,
        );
        assert_eq!(store.load_state, PaneLoadState::Error);
        assert!(store
            .last_error
            .as_deref()
            .is_some_and(|error| { error.contains("Unsupported PM work items schema version") }));
        assert_eq!(
            store.last_action.as_deref(),
            Some("PM projection bootstrap failed")
        );
    }

    #[test]
    fn apply_projection_updates_persist_and_reload() {
        let work_items_path = unique_temp_path("persist-work-items");
        let activity_path = unique_temp_path("persist-activity");
        let cycles_path = unique_temp_path("persist-cycles");
        let saved_views_path = unique_temp_path("persist-saved-views");
        let teams_path = unique_temp_path("persist-teams");
        let checkpoint_path = unique_temp_path("persist-checkpoints");

        let mut store = ProjectOpsProjectionStore::from_paths_for_tests(
            work_items_path.clone(),
            activity_path.clone(),
            cycles_path.clone(),
            saved_views_path.clone(),
            teams_path.clone(),
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
        assert!(store
            .upsert_team(sample_team_row())
            .expect("team row should upsert"));

        let reloaded = ProjectOpsProjectionStore::from_paths_for_tests(
            work_items_path,
            activity_path,
            cycles_path,
            saved_views_path,
            teams_path,
            checkpoint_path,
        );
        assert_eq!(reloaded.work_items.len(), 1);
        assert_eq!(reloaded.activity_rows.len(), 1);
        assert_eq!(reloaded.cycles.len(), 1);
        assert_eq!(reloaded.saved_views.len(), 6);
        assert_eq!(reloaded.teams.len(), 2);
        assert_eq!(
            reloaded.checkpoint_for(PROJECT_OPS_WORK_ITEMS_STREAM_ID),
            Some(1)
        );
        assert_eq!(
            reloaded.checkpoint_for(PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID),
            Some(1)
        );
        assert_eq!(
            reloaded.checkpoint_for(PROJECT_OPS_CYCLES_STREAM_ID),
            Some(1)
        );
        assert_eq!(
            reloaded.checkpoint_for(PROJECT_OPS_SAVED_VIEWS_STREAM_ID),
            Some(1)
        );
        assert_eq!(reloaded.checkpoint_for(PROJECT_OPS_TEAMS_STREAM_ID), Some(1));
    }

    #[test]
    fn duplicate_and_out_of_order_sequences_are_reported_without_overwriting() {
        let work_items_path = unique_temp_path("dupe-work-items");
        let activity_path = unique_temp_path("dupe-activity");
        let cycles_path = unique_temp_path("dupe-cycles");
        let saved_views_path = unique_temp_path("dupe-saved-views");
        let teams_path = unique_temp_path("dupe-teams");
        let checkpoint_path = unique_temp_path("dupe-checkpoints");

        let mut store = ProjectOpsProjectionStore::from_paths_for_tests(
            work_items_path,
            activity_path,
            cycles_path,
            saved_views_path,
            teams_path,
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
        assert!(store.last_error.as_deref().is_some_and(|error| {
            error.contains("project_ops.checkpoint_conflict:") && error.contains("Out-of-order")
        }));
        assert_eq!(store.work_items.len(), 1);
    }

    #[test]
    fn checkpoint_recovery_helpers_match_sync_apply_contract() {
        let work_items_path = unique_temp_path("recover-work-items");
        let activity_path = unique_temp_path("recover-activity");
        let cycles_path = unique_temp_path("recover-cycles");
        let saved_views_path = unique_temp_path("recover-saved-views");
        let teams_path = unique_temp_path("recover-teams");
        let checkpoint_path = unique_temp_path("recover-checkpoints");

        let mut store = ProjectOpsProjectionStore::from_paths_for_tests(
            work_items_path,
            activity_path,
            cycles_path,
            saved_views_path,
            teams_path,
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

        assert!(store
            .adopt_remote_checkpoint(PROJECT_OPS_CYCLES_STREAM_ID, 4)
            .expect("remote checkpoint should adopt"));
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
            unique_temp_path("recover-teams-reload-unused"),
            checkpoint_path,
        );
        assert_eq!(
            reloaded.checkpoint_for(PROJECT_OPS_WORK_ITEMS_STREAM_ID),
            Some(1)
        );
        assert_eq!(
            reloaded.checkpoint_for(PROJECT_OPS_CYCLES_STREAM_ID),
            Some(2)
        );
    }
}
