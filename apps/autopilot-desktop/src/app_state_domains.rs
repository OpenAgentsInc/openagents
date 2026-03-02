use super::*;

pub struct CodexAccountPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub account_summary: String,
    pub requires_openai_auth: bool,
    pub auth_mode: Option<String>,
    pub pending_login_id: Option<String>,
    pub pending_login_url: Option<String>,
    pub rate_limits_summary: Option<String>,
}

impl Default for CodexAccountPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for account/read".to_string()),
            account_summary: "unknown".to_string(),
            requires_openai_auth: true,
            auth_mode: None,
            pending_login_id: None,
            pending_login_url: None,
            rate_limits_summary: None,
        }
    }
}

pub struct CodexModelCatalogEntryState {
    pub model: String,
    pub display_name: String,
    pub description: String,
    pub hidden: bool,
    pub is_default: bool,
    pub default_reasoning_effort: String,
    pub supported_reasoning_efforts: Vec<String>,
}

pub struct CodexModelsPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub include_hidden: bool,
    pub entries: Vec<CodexModelCatalogEntryState>,
    pub last_reroute: Option<String>,
}

impl Default for CodexModelsPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for model/list".to_string()),
            include_hidden: false,
            entries: Vec::new(),
            last_reroute: None,
        }
    }
}

pub struct CodexConfigPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub config_json: String,
    pub requirements_json: String,
    pub detected_external_configs: usize,
}

impl Default for CodexConfigPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for config/read".to_string()),
            config_json: "{}".to_string(),
            requirements_json: "null".to_string(),
            detected_external_configs: 0,
        }
    }
}

pub struct CodexMcpServerEntryState {
    pub name: String,
    pub auth_status: String,
    pub tool_count: usize,
    pub resource_count: usize,
    pub template_count: usize,
}

pub struct CodexMcpPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub servers: Vec<CodexMcpServerEntryState>,
    pub selected_server_index: Option<usize>,
    pub last_oauth_url: Option<String>,
    pub last_oauth_result: Option<String>,
    pub next_cursor: Option<String>,
}

impl Default for CodexMcpPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for mcpServerStatus/list".to_string()),
            servers: Vec::new(),
            selected_server_index: None,
            last_oauth_url: None,
            last_oauth_result: None,
            next_cursor: None,
        }
    }
}

pub struct CodexAppEntryState {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub is_accessible: bool,
    pub is_enabled: bool,
}

pub struct CodexAppsPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub apps: Vec<CodexAppEntryState>,
    pub selected_app_index: Option<usize>,
    pub next_cursor: Option<String>,
    pub update_count: u64,
}

impl Default for CodexAppsPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for app/list".to_string()),
            apps: Vec::new(),
            selected_app_index: None,
            next_cursor: None,
            update_count: 0,
        }
    }
}

pub struct CodexLabsPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub review_last_turn_id: Option<String>,
    pub review_last_thread_id: Option<String>,
    pub command_last_exit_code: Option<i32>,
    pub command_last_stdout: String,
    pub command_last_stderr: String,
    pub collaboration_modes_json: String,
    pub experimental_features_json: String,
    pub experimental_enabled: bool,
    pub realtime_started: bool,
    pub fuzzy_session_id: String,
    pub fuzzy_last_status: String,
    pub windows_last_status: Option<String>,
}

impl Default for CodexLabsPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Codex Labs ready".to_string()),
            review_last_turn_id: None,
            review_last_thread_id: None,
            command_last_exit_code: None,
            command_last_stdout: String::new(),
            command_last_stderr: String::new(),
            collaboration_modes_json: "[]".to_string(),
            experimental_features_json: "[]".to_string(),
            experimental_enabled: false,
            realtime_started: false,
            fuzzy_session_id: format!("labs-{}", std::process::id()),
            fuzzy_last_status: "idle".to_string(),
            windows_last_status: None,
        }
    }
}

pub struct CodexDiagnosticsMethodCountState {
    pub method: String,
    pub count: u64,
}

pub struct CodexDiagnosticsPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub notification_counts: Vec<CodexDiagnosticsMethodCountState>,
    pub server_request_counts: Vec<CodexDiagnosticsMethodCountState>,
    pub raw_events: Vec<String>,
    pub last_command_failure: Option<String>,
    pub last_snapshot_error: Option<String>,
    pub wire_log_path: String,
    pub wire_log_enabled: bool,
}

impl Default for CodexDiagnosticsPaneState {
    fn default() -> Self {
        let env_wire_log_path = std::env::var("OPENAGENTS_CODEX_WIRE_LOG_PATH").ok();
        let wire_log_path = env_wire_log_path
            .clone()
            .unwrap_or_else(|| "/tmp/openagents-codex-wire.log".to_string());
        Self {
            load_state: PaneLoadState::Ready,
            last_error: None,
            last_action: Some("Codex diagnostics idle".to_string()),
            notification_counts: Vec::new(),
            server_request_counts: Vec::new(),
            raw_events: Vec::new(),
            last_command_failure: None,
            last_snapshot_error: None,
            wire_log_path: wire_log_path.clone(),
            wire_log_enabled: env_wire_log_path.is_some(),
        }
    }
}

pub struct AgentProfileStatePaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub profile_name: String,
    pub profile_about: String,
    pub goals_summary: String,
    pub selected_goal_id: Option<String>,
    pub selected_goal_status: String,
    pub selected_goal_attempts: u32,
    pub selected_goal_selected_skills: String,
    pub selected_goal_receipt_summary: String,
    pub treasury_wallet_projection_count: usize,
    pub treasury_wallet_projection_summary: String,
    pub profile_event_id: Option<String>,
    pub state_event_id: Option<String>,
    pub goals_event_id: Option<String>,
}

impl Default for AgentProfileStatePaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for SA profile/state snapshot".to_string()),
            profile_name: "Autopilot".to_string(),
            profile_about: "Desktop sovereign agent runtime".to_string(),
            goals_summary: "Earn sats and complete queued jobs".to_string(),
            selected_goal_id: None,
            selected_goal_status: "n/a".to_string(),
            selected_goal_attempts: 0,
            selected_goal_selected_skills: "n/a".to_string(),
            selected_goal_receipt_summary: "n/a".to_string(),
            treasury_wallet_projection_count: 0,
            treasury_wallet_projection_summary: "n/a".to_string(),
            profile_event_id: None,
            state_event_id: None,
            goals_event_id: None,
        }
    }
}

pub struct AgentScheduleTickPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub heartbeat_seconds: u64,
    pub selected_goal_id: Option<String>,
    pub scheduler_mode: String,
    pub next_goal_run_epoch_seconds: Option<u64>,
    pub last_goal_run_epoch_seconds: Option<u64>,
    pub missed_run_policy: String,
    pub pending_catchup_runs: u32,
    pub last_recovery_epoch_seconds: Option<u64>,
    pub cron_expression: String,
    pub cron_timezone: String,
    pub cron_next_run_preview_epoch_seconds: Option<u64>,
    pub cron_parse_error: Option<String>,
    pub os_scheduler_enabled: bool,
    pub os_scheduler_adapter: String,
    pub os_scheduler_descriptor_path: Option<String>,
    pub os_scheduler_last_reconciled_epoch_seconds: Option<u64>,
    pub os_scheduler_last_reconcile_result: Option<String>,
    pub next_tick_reason: String,
    pub last_tick_outcome: String,
    pub schedule_event_id: Option<String>,
    pub tick_request_event_id: Option<String>,
    pub tick_result_event_id: Option<String>,
}

impl Default for AgentScheduleTickPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for SA schedule/tick snapshot".to_string()),
            heartbeat_seconds: 30,
            selected_goal_id: None,
            scheduler_mode: "manual".to_string(),
            next_goal_run_epoch_seconds: None,
            last_goal_run_epoch_seconds: None,
            missed_run_policy: "single_replay".to_string(),
            pending_catchup_runs: 0,
            last_recovery_epoch_seconds: None,
            cron_expression: "*/15 * * * *".to_string(),
            cron_timezone: "UTC".to_string(),
            cron_next_run_preview_epoch_seconds: None,
            cron_parse_error: None,
            os_scheduler_enabled: false,
            os_scheduler_adapter: "auto".to_string(),
            os_scheduler_descriptor_path: None,
            os_scheduler_last_reconciled_epoch_seconds: None,
            os_scheduler_last_reconcile_result: None,
            next_tick_reason: "manual.operator".to_string(),
            last_tick_outcome: "n/a".to_string(),
            schedule_event_id: None,
            tick_request_event_id: None,
            tick_result_event_id: None,
        }
    }
}

pub struct TrajectoryAuditPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub active_session_id: Option<String>,
    pub verified_hash: Option<String>,
    pub step_filter: String,
    pub treasury_event_ref: Option<String>,
    pub treasury_event_summary: Option<String>,
}

impl Default for TrajectoryAuditPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for trajectory session stream".to_string()),
            active_session_id: None,
            verified_hash: None,
            step_filter: "all".to_string(),
            treasury_event_ref: None,
            treasury_event_summary: None,
        }
    }
}

pub struct SkillRegistryPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub search_query: String,
    pub manifest_slug: String,
    pub manifest_version: String,
    pub manifest_a: Option<String>,
    pub manifest_event_id: Option<String>,
    pub version_event_id: Option<String>,
    pub search_result_event_id: Option<String>,
    pub source: String,
    pub repo_skills_root: Option<String>,
    pub discovered_skills: Vec<SkillRegistryDiscoveredSkill>,
    pub discovery_errors: Vec<String>,
    pub selected_skill_index: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkillRegistryDiscoveredSkill {
    pub name: String,
    pub path: String,
    pub scope: String,
    pub enabled: bool,
    pub interface_display_name: Option<String>,
    pub dependency_count: usize,
}

impl Default for SkillRegistryPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for SKL registry snapshot".to_string()),
            search_query: "summarize".to_string(),
            manifest_slug: "summarize-text".to_string(),
            manifest_version: "0.1.0".to_string(),
            manifest_a: None,
            manifest_event_id: None,
            version_event_id: None,
            search_result_event_id: None,
            source: "codex".to_string(),
            repo_skills_root: None,
            discovered_skills: Vec::new(),
            discovery_errors: Vec::new(),
            selected_skill_index: None,
        }
    }
}

pub struct SkillTrustRevocationPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub trust_tier: String,
    pub manifest_a: Option<String>,
    pub attestation_count: u32,
    pub kill_switch_active: bool,
    pub revocation_event_id: Option<String>,
}

impl Default for SkillTrustRevocationPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for SKL trust gate snapshot".to_string()),
            trust_tier: "unknown".to_string(),
            manifest_a: None,
            attestation_count: 0,
            kill_switch_active: false,
            revocation_event_id: None,
        }
    }
}

pub struct CreditDeskPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub scope: String,
    pub requested_sats: u64,
    pub offered_sats: u64,
    pub envelope_cap_sats: u64,
    pub spend_sats: u64,
    pub spend_job_id: String,
    pub intent_event_id: Option<String>,
    pub offer_event_id: Option<String>,
    pub envelope_event_id: Option<String>,
    pub spend_event_id: Option<String>,
}

impl Default for CreditDeskPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for AC credit desk snapshot".to_string()),
            scope: "skill:33400:npub1agent:summarize-text:0.1.0:constraints".to_string(),
            requested_sats: 1500,
            offered_sats: 1400,
            envelope_cap_sats: 1200,
            spend_sats: 600,
            spend_job_id: "job-credit-001".to_string(),
            intent_event_id: None,
            offer_event_id: None,
            envelope_event_id: None,
            spend_event_id: None,
        }
    }
}

pub struct CreditSettlementLedgerPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub result_event_id: String,
    pub payment_pointer: String,
    pub default_reason: String,
    pub settlement_event_id: Option<String>,
    pub default_event_id: Option<String>,
}

impl Default for CreditSettlementLedgerPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for AC settlement ledger snapshot".to_string()),
            result_event_id: "nip90:result:pending".to_string(),
            payment_pointer: "pay:pending".to_string(),
            default_reason: "settlement timeout".to_string(),
            settlement_event_id: None,
            default_event_id: None,
        }
    }
}

pub struct CadDemoPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub session_id: String,
    pub active_chat_session_id: Option<String>,
    pub chat_thread_session_bindings: std::collections::BTreeMap<String, String>,
    pub dispatch_sessions:
        std::collections::BTreeMap<String, openagents_cad::dispatch::CadDispatchState>,
    pub last_chat_intent_name: Option<String>,
    pub build_session: CadBuildSessionState,
    pub last_build_session: Option<CadBuildSessionArchiveState>,
    pub build_failure_metrics: CadBuildFailureMetricsState,
    pub document_id: String,
    pub document_revision: u64,
    pub active_variant_id: String,
    pub variant_ids: Vec<String>,
    pub active_variant_tile_index: usize,
    pub variant_viewports: Vec<CadVariantViewportState>,
    pub last_rebuild_receipt: Option<CadRebuildReceiptState>,
    pub rebuild_receipts: Vec<CadRebuildReceiptState>,
    pub eval_cache: openagents_cad::eval::EvalCacheStore,
    pub rebuild_worker: Option<crate::cad_rebuild_worker::CadBackgroundRebuildWorker>,
    pub next_rebuild_request_id: u64,
    pub pending_rebuild_request_id: Option<u64>,
    pub last_good_mesh_id: Option<String>,
    pub last_good_mesh_payload: Option<openagents_cad::mesh::CadMeshPayload>,
    pub warnings: Vec<CadDemoWarningState>,
    pub variant_warning_sets: std::collections::BTreeMap<String, Vec<CadDemoWarningState>>,
    pub warning_filter_severity: String,
    pub warning_filter_code: String,
    pub warning_hover_index: Option<usize>,
    pub focused_warning_index: Option<usize>,
    pub focused_geometry_ref: Option<String>,
    pub hovered_geometry_ref: Option<String>,
    pub selection_store: openagents_cad::selection::CadSelectionStore,
    pub analysis_snapshot: openagents_cad::contracts::CadAnalysis,
    pub variant_analysis_snapshots:
        std::collections::BTreeMap<String, openagents_cad::contracts::CadAnalysis>,
    pub measurement_tile_index: Option<usize>,
    pub measurement_points: Vec<Point>,
    pub measurement_distance_px: Option<f64>,
    pub measurement_angle_deg: Option<f64>,
    pub section_axis: Option<CadSectionAxis>,
    pub section_offset_normalized: f32,
    pub hidden_line_mode: CadHiddenLineMode,
    pub snap_toggles: CadSnapToggles,
    pub projection_mode: CadProjectionMode,
    pub hotkey_profile: String,
    pub hotkeys: CadHotkeyBindings,
    pub three_d_mouse_mode: CadThreeDMouseMode,
    pub three_d_mouse_profile: CadThreeDMouseProfile,
    pub three_d_mouse_axis_locks: CadThreeDMouseAxisLocks,
    pub three_d_mouse_event_count: u64,
    pub camera_zoom: f32,
    pub camera_pan_x: f32,
    pub camera_pan_y: f32,
    pub camera_orbit_yaw_deg: f32,
    pub camera_orbit_pitch_deg: f32,
    pub history_stack: openagents_cad::history::CadHistoryStack,
    pub timeline_rows: Vec<CadTimelineRowState>,
    pub timeline_selected_index: Option<usize>,
    pub timeline_scroll_offset: usize,
    pub selected_feature_params: Vec<(String, String)>,
    pub dimensions: Vec<CadDimensionState>,
    pub dimension_edit: Option<CadDimensionEditState>,
    pub context_menu: CadContextMenuState,
    pub cad_events: Vec<openagents_cad::events::CadEvent>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CadCameraViewSnap {
    Isometric,
    Top,
    Front,
    Right,
}

impl CadCameraViewSnap {
    pub fn orbit_degrees(self) -> (f32, f32) {
        match self {
            Self::Isometric => (45.0, 35.264),
            Self::Top => (0.0, 89.0),
            Self::Front => (0.0, 0.0),
            Self::Right => (90.0, 0.0),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CadSnapMode {
    Grid,
    Origin,
    Endpoint,
    Midpoint,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum CadThreeDMouseAxis {
    X,
    Y,
    Z,
    Rx,
    Ry,
    Rz,
}

impl CadThreeDMouseAxis {
    pub fn from_motion_axis_id(axis_id: u32) -> Option<Self> {
        match axis_id {
            0 => Some(Self::X),
            1 => Some(Self::Y),
            2 => Some(Self::Z),
            3 => Some(Self::Rx),
            4 => Some(Self::Ry),
            5 => Some(Self::Rz),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::X => "x",
            Self::Y => "y",
            Self::Z => "z",
            Self::Rx => "rx",
            Self::Ry => "ry",
            Self::Rz => "rz",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CadThreeDMouseMode {
    Translate,
    Rotate,
}

impl CadThreeDMouseMode {
    pub fn next(self) -> Self {
        match self {
            Self::Translate => Self::Rotate,
            Self::Rotate => Self::Translate,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Translate => "translate",
            Self::Rotate => "rotate",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CadThreeDMouseProfile {
    Precision,
    Balanced,
    Fast,
}

impl CadThreeDMouseProfile {
    pub fn next(self) -> Self {
        match self {
            Self::Precision => Self::Balanced,
            Self::Balanced => Self::Fast,
            Self::Fast => Self::Precision,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Precision => "precision",
            Self::Balanced => "balanced",
            Self::Fast => "fast",
        }
    }

    pub fn scalar(self) -> f32 {
        match self {
            Self::Precision => 0.6,
            Self::Balanced => 1.0,
            Self::Fast => 1.7,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CadThreeDMouseAxisLocks {
    pub x: bool,
    pub y: bool,
    pub z: bool,
    pub rx: bool,
    pub ry: bool,
    pub rz: bool,
}

impl Default for CadThreeDMouseAxisLocks {
    fn default() -> Self {
        Self {
            x: false,
            y: false,
            z: false,
            rx: false,
            ry: false,
            rz: false,
        }
    }
}

impl CadThreeDMouseAxisLocks {
    pub fn is_locked(&self, axis: CadThreeDMouseAxis) -> bool {
        match axis {
            CadThreeDMouseAxis::X => self.x,
            CadThreeDMouseAxis::Y => self.y,
            CadThreeDMouseAxis::Z => self.z,
            CadThreeDMouseAxis::Rx => self.rx,
            CadThreeDMouseAxis::Ry => self.ry,
            CadThreeDMouseAxis::Rz => self.rz,
        }
    }

    pub fn toggle(&mut self, axis: CadThreeDMouseAxis) -> bool {
        let target = match axis {
            CadThreeDMouseAxis::X => &mut self.x,
            CadThreeDMouseAxis::Y => &mut self.y,
            CadThreeDMouseAxis::Z => &mut self.z,
            CadThreeDMouseAxis::Rx => &mut self.rx,
            CadThreeDMouseAxis::Ry => &mut self.ry,
            CadThreeDMouseAxis::Rz => &mut self.rz,
        };
        *target = !*target;
        *target
    }

    pub fn summary(&self) -> String {
        format!(
            "x={} y={} z={} rx={} ry={} rz={}",
            self.x as u8, self.y as u8, self.z as u8, self.rx as u8, self.ry as u8, self.rz as u8
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum CadHotkeyAction {
    SnapTop,
    SnapFront,
    SnapRight,
    SnapIsometric,
    ToggleProjection,
    CycleRenderMode,
    ToggleSnapGrid,
    ToggleSnapOrigin,
    ToggleSnapEndpoint,
    ToggleSnapMidpoint,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CadHotkeyBindings {
    pub snap_top: String,
    pub snap_front: String,
    pub snap_right: String,
    pub snap_isometric: String,
    pub toggle_projection: String,
    pub cycle_render_mode: String,
    pub toggle_snap_grid: String,
    pub toggle_snap_origin: String,
    pub toggle_snap_endpoint: String,
    pub toggle_snap_midpoint: String,
}

impl Default for CadHotkeyBindings {
    fn default() -> Self {
        Self {
            snap_top: "t".to_string(),
            snap_front: "f".to_string(),
            snap_right: "r".to_string(),
            snap_isometric: "i".to_string(),
            toggle_projection: "p".to_string(),
            cycle_render_mode: "v".to_string(),
            toggle_snap_grid: "g".to_string(),
            toggle_snap_origin: "o".to_string(),
            toggle_snap_endpoint: "e".to_string(),
            toggle_snap_midpoint: "m".to_string(),
        }
    }
}

impl CadHotkeyBindings {
    pub fn compact_profile() -> Self {
        Self {
            snap_top: "7".to_string(),
            snap_front: "1".to_string(),
            snap_right: "3".to_string(),
            snap_isometric: "5".to_string(),
            toggle_projection: "p".to_string(),
            cycle_render_mode: "w".to_string(),
            toggle_snap_grid: "g".to_string(),
            toggle_snap_origin: "z".to_string(),
            toggle_snap_endpoint: "x".to_string(),
            toggle_snap_midpoint: "c".to_string(),
        }
    }

    pub fn key_for(&self, action: CadHotkeyAction) -> &str {
        match action {
            CadHotkeyAction::SnapTop => &self.snap_top,
            CadHotkeyAction::SnapFront => &self.snap_front,
            CadHotkeyAction::SnapRight => &self.snap_right,
            CadHotkeyAction::SnapIsometric => &self.snap_isometric,
            CadHotkeyAction::ToggleProjection => &self.toggle_projection,
            CadHotkeyAction::CycleRenderMode => &self.cycle_render_mode,
            CadHotkeyAction::ToggleSnapGrid => &self.toggle_snap_grid,
            CadHotkeyAction::ToggleSnapOrigin => &self.toggle_snap_origin,
            CadHotkeyAction::ToggleSnapEndpoint => &self.toggle_snap_endpoint,
            CadHotkeyAction::ToggleSnapMidpoint => &self.toggle_snap_midpoint,
        }
    }

    pub fn set_key(&mut self, action: CadHotkeyAction, value: &str) {
        let normalized = value.trim().to_lowercase();
        let target = match action {
            CadHotkeyAction::SnapTop => &mut self.snap_top,
            CadHotkeyAction::SnapFront => &mut self.snap_front,
            CadHotkeyAction::SnapRight => &mut self.snap_right,
            CadHotkeyAction::SnapIsometric => &mut self.snap_isometric,
            CadHotkeyAction::ToggleProjection => &mut self.toggle_projection,
            CadHotkeyAction::CycleRenderMode => &mut self.cycle_render_mode,
            CadHotkeyAction::ToggleSnapGrid => &mut self.toggle_snap_grid,
            CadHotkeyAction::ToggleSnapOrigin => &mut self.toggle_snap_origin,
            CadHotkeyAction::ToggleSnapEndpoint => &mut self.toggle_snap_endpoint,
            CadHotkeyAction::ToggleSnapMidpoint => &mut self.toggle_snap_midpoint,
        };
        *target = normalized;
    }

    pub fn validate_conflicts(&self) -> Result<(), String> {
        let mut seen = std::collections::BTreeMap::<String, CadHotkeyAction>::new();
        for action in [
            CadHotkeyAction::SnapTop,
            CadHotkeyAction::SnapFront,
            CadHotkeyAction::SnapRight,
            CadHotkeyAction::SnapIsometric,
            CadHotkeyAction::ToggleProjection,
            CadHotkeyAction::CycleRenderMode,
            CadHotkeyAction::ToggleSnapGrid,
            CadHotkeyAction::ToggleSnapOrigin,
            CadHotkeyAction::ToggleSnapEndpoint,
            CadHotkeyAction::ToggleSnapMidpoint,
        ] {
            let key = self.key_for(action).trim().to_lowercase();
            if key.is_empty() {
                return Err(format!("hotkey for {action:?} cannot be empty"));
            }
            if let Some(existing) = seen.insert(key.clone(), action) {
                return Err(format!(
                    "hotkey conflict: '{key}' already assigned to {existing:?}"
                ));
            }
        }
        Ok(())
    }

    pub fn summary(&self) -> String {
        format!(
            "top={} front={} right={} iso={} proj={} render={} grid={} origin={} endpoint={} midpoint={}",
            self.snap_top,
            self.snap_front,
            self.snap_right,
            self.snap_isometric,
            self.toggle_projection,
            self.cycle_render_mode,
            self.toggle_snap_grid,
            self.toggle_snap_origin,
            self.toggle_snap_endpoint,
            self.toggle_snap_midpoint,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CadSnapToggles {
    pub grid: bool,
    pub origin: bool,
    pub endpoint: bool,
    pub midpoint: bool,
}

impl Default for CadSnapToggles {
    fn default() -> Self {
        Self {
            grid: true,
            origin: true,
            endpoint: false,
            midpoint: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CadProjectionMode {
    Orthographic,
    Perspective,
}

impl CadProjectionMode {
    pub fn next(self) -> Self {
        match self {
            Self::Orthographic => Self::Perspective,
            Self::Perspective => Self::Orthographic,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Orthographic => "ortho",
            Self::Perspective => "perspective",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CadSectionAxis {
    X,
    Y,
    Z,
}

impl CadSectionAxis {
    pub fn next(self) -> Self {
        match self {
            Self::X => Self::Y,
            Self::Y => Self::Z,
            Self::Z => Self::X,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::X => "x",
            Self::Y => "y",
            Self::Z => "z",
        }
    }

    pub fn to_cad_section_axis(self) -> openagents_cad::section::CadSectionAxis {
        match self {
            Self::X => openagents_cad::section::CadSectionAxis::X,
            Self::Y => openagents_cad::section::CadSectionAxis::Y,
            Self::Z => openagents_cad::section::CadSectionAxis::Z,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CadHiddenLineMode {
    Shaded,
    ShadedEdges,
    Wireframe,
}

impl CadHiddenLineMode {
    pub fn next(self) -> Self {
        match self {
            Self::Shaded => Self::ShadedEdges,
            Self::ShadedEdges => Self::Wireframe,
            Self::Wireframe => Self::Shaded,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Shaded => "shaded",
            Self::ShadedEdges => "shaded+edges",
            Self::Wireframe => "wireframe",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CadBuildSessionPhase {
    Idle,
    Planning,
    Applying,
    Rebuilding,
    Summarizing,
    Done,
    Failed,
}

impl CadBuildSessionPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Planning => "planning",
            Self::Applying => "applying",
            Self::Rebuilding => "rebuilding",
            Self::Summarizing => "summarizing",
            Self::Done => "done",
            Self::Failed => "failed",
        }
    }

    fn transition_allowed(self, next: CadBuildSessionPhase) -> bool {
        use CadBuildSessionPhase::{
            Applying, Done, Failed, Idle, Planning, Rebuilding, Summarizing,
        };
        match (self, next) {
            (Idle, Planning) => true,
            (Planning, Applying | Failed) => true,
            (Applying, Rebuilding | Summarizing | Failed) => true,
            (Rebuilding, Summarizing | Failed) => true,
            (Summarizing, Done | Failed) => true,
            (Done, Idle) => true,
            (Failed, Idle) => true,
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CadBuildFailureClass {
    ToolTransport,
    IntentParseValidation,
    DispatchRebuild,
}

impl CadBuildFailureClass {
    pub fn label(self) -> &'static str {
        match self {
            Self::ToolTransport => "tool_transport",
            Self::IntentParseValidation => "intent_parse_validation",
            Self::DispatchRebuild => "dispatch_rebuild",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CadBuildFailureMetricsState {
    pub tool_transport_failures: u64,
    pub intent_parse_failures: u64,
    pub dispatch_rebuild_failures: u64,
    pub tool_transport_retries: u64,
    pub intent_parse_retries: u64,
    pub dispatch_rebuild_retries: u64,
    pub terminal_failures: u64,
}

impl CadBuildFailureMetricsState {
    pub fn record_failure(&mut self, class: CadBuildFailureClass) {
        match class {
            CadBuildFailureClass::ToolTransport => {
                self.tool_transport_failures = self.tool_transport_failures.saturating_add(1);
            }
            CadBuildFailureClass::IntentParseValidation => {
                self.intent_parse_failures = self.intent_parse_failures.saturating_add(1);
            }
            CadBuildFailureClass::DispatchRebuild => {
                self.dispatch_rebuild_failures = self.dispatch_rebuild_failures.saturating_add(1);
            }
        }
    }

    pub fn record_retry(&mut self, class: CadBuildFailureClass) {
        match class {
            CadBuildFailureClass::ToolTransport => {
                self.tool_transport_retries = self.tool_transport_retries.saturating_add(1);
            }
            CadBuildFailureClass::IntentParseValidation => {
                self.intent_parse_retries = self.intent_parse_retries.saturating_add(1);
            }
            CadBuildFailureClass::DispatchRebuild => {
                self.dispatch_rebuild_retries = self.dispatch_rebuild_retries.saturating_add(1);
            }
        }
    }
}

impl Default for CadBuildFailureMetricsState {
    fn default() -> Self {
        Self {
            tool_transport_failures: 0,
            intent_parse_failures: 0,
            dispatch_rebuild_failures: 0,
            tool_transport_retries: 0,
            intent_parse_retries: 0,
            dispatch_rebuild_retries: 0,
            terminal_failures: 0,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CadBuildSessionEventState {
    pub event_code: String,
    pub phase: CadBuildSessionPhase,
    pub detail: String,
    pub at_epoch_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CadBuildSessionState {
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    pub phase: CadBuildSessionPhase,
    pub failure_class: Option<CadBuildFailureClass>,
    pub retry_attempts: u8,
    pub retry_limit: u8,
    pub latest_tool_result: Option<String>,
    pub latest_rebuild_result: Option<String>,
    pub failure_reason: Option<String>,
    pub remediation_hint: Option<String>,
    pub events: Vec<CadBuildSessionEventState>,
}

impl Default for CadBuildSessionState {
    fn default() -> Self {
        Self {
            thread_id: None,
            turn_id: None,
            phase: CadBuildSessionPhase::Idle,
            failure_class: None,
            retry_attempts: 0,
            retry_limit: 0,
            latest_tool_result: None,
            latest_rebuild_result: None,
            failure_reason: None,
            remediation_hint: None,
            events: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CadBuildSessionArchiveState {
    pub thread_id: String,
    pub turn_id: String,
    pub terminal_phase: CadBuildSessionPhase,
    pub failure_class: Option<CadBuildFailureClass>,
    pub retry_attempts: u8,
    pub retry_limit: u8,
    pub latest_tool_result: Option<String>,
    pub latest_rebuild_result: Option<String>,
    pub failure_reason: Option<String>,
    pub remediation_hint: Option<String>,
    pub events: Vec<CadBuildSessionEventState>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CadRebuildReceiptState {
    pub event_id: String,
    pub document_revision: u64,
    pub variant_id: String,
    pub rebuild_hash: String,
    pub mesh_hash: String,
    pub duration_ms: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub cache_evictions: u64,
    pub feature_count: usize,
    pub vertex_count: usize,
    pub triangle_count: usize,
    pub edge_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CadDemoWarningState {
    pub warning_id: String,
    pub code: String,
    pub severity: String,
    pub message: String,
    pub remediation_hint: String,
    pub semantic_refs: Vec<String>,
    pub deep_link: Option<String>,
    pub feature_id: String,
    pub entity_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CadTimelineRowState {
    pub feature_id: String,
    pub feature_name: String,
    pub op_type: String,
    pub status_badge: String,
    pub provenance: String,
    pub params: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CadDimensionState {
    pub dimension_id: String,
    pub label: String,
    pub value_mm: f64,
    pub min_mm: f64,
    pub max_mm: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CadDimensionEditState {
    pub dimension_index: usize,
    pub draft_value: String,
    pub last_error: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CadContextMenuTargetKind {
    Body,
    Face,
    Edge,
}

impl CadContextMenuTargetKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Body => "body",
            Self::Face => "face",
            Self::Edge => "edge",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CadContextMenuItemState {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CadContextMenuState {
    pub is_open: bool,
    pub anchor: Point,
    pub target_kind: CadContextMenuTargetKind,
    pub target_ref: String,
    pub items: Vec<CadContextMenuItemState>,
}

impl Default for CadContextMenuState {
    fn default() -> Self {
        Self {
            is_open: false,
            anchor: Point::ZERO,
            target_kind: CadContextMenuTargetKind::Body,
            target_ref: "body.default".to_string(),
            items: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CadVariantViewportState {
    pub variant_id: String,
    pub camera_zoom: f32,
    pub camera_pan_x: f32,
    pub camera_pan_y: f32,
    pub camera_orbit_yaw_deg: f32,
    pub camera_orbit_pitch_deg: f32,
    pub selected_ref: Option<String>,
    pub hovered_ref: Option<String>,
}

impl CadVariantViewportState {
    pub fn for_variant(variant_id: &str) -> Self {
        Self {
            variant_id: variant_id.to_string(),
            camera_zoom: 1.0,
            camera_pan_x: 0.0,
            camera_pan_y: 0.0,
            camera_orbit_yaw_deg: 26.0,
            camera_orbit_pitch_deg: 18.0,
            selected_ref: None,
            hovered_ref: None,
        }
    }
}

impl Default for CadDemoPaneState {
    fn default() -> Self {
        let variant_ids = vec![
            "variant.baseline".to_string(),
            "variant.lightweight".to_string(),
            "variant.low-cost".to_string(),
            "variant.stiffness".to_string(),
        ];
        let initial_variant_id = variant_ids[0].clone();
        let variant_viewports = variant_ids
            .iter()
            .map(|variant_id| CadVariantViewportState::for_variant(variant_id))
            .collect::<Vec<_>>();
        let default_analysis = openagents_cad::contracts::CadAnalysis {
            document_revision: 0,
            variant_id: initial_variant_id.clone(),
            material_id: Some(openagents_cad::materials::DEFAULT_CAD_MATERIAL_ID.to_string()),
            volume_mm3: None,
            mass_kg: None,
            center_of_gravity_mm: None,
            estimated_cost_usd: None,
            max_deflection_mm: None,
            estimator_metadata: std::collections::BTreeMap::new(),
            objective_scores: std::collections::BTreeMap::new(),
        };
        let variant_analysis_snapshots = variant_ids
            .iter()
            .map(|variant_id| {
                let mut analysis = default_analysis.clone();
                analysis.variant_id = variant_id.clone();
                (variant_id.clone(), analysis)
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        let variant_warning_sets = variant_ids
            .iter()
            .map(|variant_id| (variant_id.clone(), Vec::new()))
            .collect::<std::collections::BTreeMap<_, _>>();
        let dimensions = vec![
            CadDimensionState {
                dimension_id: "width_mm".to_string(),
                label: "Width".to_string(),
                value_mm: 390.0,
                min_mm: 300.0,
                max_mm: 520.0,
            },
            CadDimensionState {
                dimension_id: "depth_mm".to_string(),
                label: "Depth".to_string(),
                value_mm: 226.0,
                min_mm: 140.0,
                max_mm: 320.0,
            },
            CadDimensionState {
                dimension_id: "height_mm".to_string(),
                label: "Height".to_string(),
                value_mm: 88.0,
                min_mm: 40.0,
                max_mm: 180.0,
            },
            CadDimensionState {
                dimension_id: "wall_mm".to_string(),
                label: "Wall".to_string(),
                value_mm: 6.0,
                min_mm: 2.0,
                max_mm: 20.0,
            },
        ];
        let session_id = "cad.session.local".to_string();
        let document_id = "cad.doc.demo-rack".to_string();
        let document_created_event = openagents_cad::events::CadEvent::new_with_key(
            openagents_cad::events::CadEventKind::DocumentCreated,
            session_id.clone(),
            document_id.clone(),
            0,
            Some(initial_variant_id.clone()),
            openagents_cad::events::CadEventMessage::new(
                "CAD document created",
                format!("session={} document={document_id}", session_id),
            )
            .with_key("document-created"),
        );
        Self {
            load_state: PaneLoadState::Ready,
            last_error: None,
            last_action: Some("CAD demo initialized; waiting for feature graph state".to_string()),
            session_id,
            active_chat_session_id: None,
            chat_thread_session_bindings: std::collections::BTreeMap::new(),
            dispatch_sessions: std::collections::BTreeMap::new(),
            last_chat_intent_name: None,
            build_session: CadBuildSessionState::default(),
            last_build_session: None,
            build_failure_metrics: CadBuildFailureMetricsState::default(),
            document_id,
            document_revision: 0,
            active_variant_id: initial_variant_id.clone(),
            variant_ids,
            active_variant_tile_index: 0,
            variant_viewports,
            last_rebuild_receipt: None,
            rebuild_receipts: Vec::new(),
            eval_cache: openagents_cad::eval::EvalCacheStore::new(128)
                .expect("cad eval cache capacity should be valid"),
            rebuild_worker: None,
            next_rebuild_request_id: 1,
            pending_rebuild_request_id: None,
            last_good_mesh_id: None,
            last_good_mesh_payload: None,
            warnings: Vec::new(),
            variant_warning_sets,
            warning_filter_severity: "all".to_string(),
            warning_filter_code: "all".to_string(),
            warning_hover_index: None,
            focused_warning_index: None,
            focused_geometry_ref: None,
            hovered_geometry_ref: None,
            selection_store: openagents_cad::selection::CadSelectionStore::default(),
            analysis_snapshot: default_analysis,
            variant_analysis_snapshots,
            measurement_tile_index: None,
            measurement_points: Vec::new(),
            measurement_distance_px: None,
            measurement_angle_deg: None,
            section_axis: None,
            section_offset_normalized: 0.0,
            hidden_line_mode: CadHiddenLineMode::Shaded,
            snap_toggles: CadSnapToggles::default(),
            projection_mode: CadProjectionMode::Orthographic,
            hotkey_profile: "default".to_string(),
            hotkeys: CadHotkeyBindings::default(),
            three_d_mouse_mode: CadThreeDMouseMode::Translate,
            three_d_mouse_profile: CadThreeDMouseProfile::Balanced,
            three_d_mouse_axis_locks: CadThreeDMouseAxisLocks::default(),
            three_d_mouse_event_count: 0,
            camera_zoom: 1.0,
            camera_pan_x: 0.0,
            camera_pan_y: 0.0,
            camera_orbit_yaw_deg: 26.0,
            camera_orbit_pitch_deg: 18.0,
            history_stack: openagents_cad::history::CadHistoryStack::new("cad.session.local", 128)
                .expect("cad history max_steps should be valid"),
            timeline_rows: Vec::new(),
            timeline_selected_index: None,
            timeline_scroll_offset: 0,
            selected_feature_params: Vec::new(),
            dimensions,
            dimension_edit: None,
            context_menu: CadContextMenuState::default(),
            cad_events: vec![document_created_event],
        }
    }
}

fn current_epoch_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

impl CadDemoPaneState {
    fn sync_active_variant_viewport_from_global(&mut self) {
        let Some(active) = self
            .variant_viewports
            .iter_mut()
            .find(|viewport| viewport.variant_id == self.active_variant_id)
        else {
            return;
        };
        active.camera_zoom = self.camera_zoom;
        active.camera_pan_x = self.camera_pan_x;
        active.camera_pan_y = self.camera_pan_y;
        active.camera_orbit_yaw_deg = self.camera_orbit_yaw_deg;
        active.camera_orbit_pitch_deg = self.camera_orbit_pitch_deg;
        active.selected_ref = self.focused_geometry_ref.clone();
        active.hovered_ref = self.hovered_geometry_ref.clone();
    }

    fn sync_global_from_variant_viewport(&mut self) {
        let Some(active) = self
            .variant_viewports
            .iter()
            .find(|viewport| viewport.variant_id == self.active_variant_id)
        else {
            return;
        };
        self.camera_zoom = active.camera_zoom;
        self.camera_pan_x = active.camera_pan_x;
        self.camera_pan_y = active.camera_pan_y;
        self.camera_orbit_yaw_deg = active.camera_orbit_yaw_deg;
        self.camera_orbit_pitch_deg = active.camera_orbit_pitch_deg;
        self.focused_geometry_ref = active.selected_ref.clone();
        self.hovered_geometry_ref = active.hovered_ref.clone();
    }

    fn sync_active_variant_payloads_from_maps(&mut self) {
        if let Some(analysis) = self
            .variant_analysis_snapshots
            .get(&self.active_variant_id)
            .cloned()
        {
            self.analysis_snapshot = analysis;
        }
        if let Some(warnings) = self
            .variant_warning_sets
            .get(&self.active_variant_id)
            .cloned()
        {
            self.warnings = warnings;
        }
        self.warning_hover_index = None;
        self.focused_warning_index = None;
    }

    pub fn set_active_variant_tile(&mut self, tile_index: usize) -> bool {
        if tile_index >= self.variant_viewports.len() {
            return false;
        }
        self.active_variant_tile_index = tile_index;
        self.active_variant_id = self.variant_viewports[tile_index].variant_id.clone();
        self.sync_global_from_variant_viewport();
        self.sync_active_variant_payloads_from_maps();
        true
    }

    pub fn variant_viewport(&self, tile_index: usize) -> Option<&CadVariantViewportState> {
        self.variant_viewports.get(tile_index)
    }

    pub fn set_focused_geometry_for_active_variant(&mut self, value: Option<String>) {
        self.focused_geometry_ref = value;
        self.sync_active_variant_viewport_from_global();
    }

    pub fn set_hovered_geometry_for_active_variant(&mut self, value: Option<String>) {
        self.hovered_geometry_ref = value;
        self.sync_active_variant_viewport_from_global();
    }

    pub fn begin_dimension_edit(&mut self, index: usize) -> bool {
        let Some(dimension) = self.dimensions.get(index) else {
            return false;
        };
        self.dimension_edit = Some(CadDimensionEditState {
            dimension_index: index,
            draft_value: format!("{:.3}", dimension.value_mm),
            last_error: None,
        });
        true
    }

    pub fn append_dimension_edit_char(&mut self, ch: char) -> bool {
        let Some(edit) = self.dimension_edit.as_mut() else {
            return false;
        };
        if !matches!(ch, '0'..='9' | '.' | '-') {
            return false;
        }
        if ch == '.' && edit.draft_value.contains('.') {
            return false;
        }
        if ch == '-' && !edit.draft_value.is_empty() {
            return false;
        }
        if edit.draft_value.len() >= 24 {
            return false;
        }
        edit.draft_value.push(ch);
        edit.last_error = None;
        true
    }

    pub fn backspace_dimension_edit(&mut self) -> bool {
        let Some(edit) = self.dimension_edit.as_mut() else {
            return false;
        };
        if edit.draft_value.is_empty() {
            return false;
        }
        edit.draft_value.pop();
        edit.last_error = None;
        true
    }

    pub fn cancel_dimension_edit(&mut self) -> bool {
        if self.dimension_edit.is_none() {
            return false;
        }
        self.dimension_edit = None;
        true
    }

    pub fn commit_dimension_edit(&mut self) -> Result<(String, f64, f64), String> {
        let Some(edit) = self.dimension_edit.clone() else {
            return Err("no active dimension edit session".to_string());
        };
        let Some(dimension) = self.dimensions.get_mut(edit.dimension_index) else {
            self.dimension_edit = None;
            return Err("dimension index out of range".to_string());
        };
        let parsed = edit
            .draft_value
            .trim()
            .parse::<f64>()
            .map_err(|_| "dimension input must be numeric (mm)".to_string())?;
        if !parsed.is_finite() {
            return Err("dimension input must be finite".to_string());
        }
        if parsed < dimension.min_mm || parsed > dimension.max_mm {
            return Err(format!(
                "{} must be between {:.3} and {:.3} mm",
                dimension.label, dimension.min_mm, dimension.max_mm
            ));
        }
        let previous = dimension.value_mm;
        dimension.value_mm = parsed;
        self.dimension_edit = None;
        Ok((dimension.dimension_id.clone(), previous, parsed))
    }

    pub fn dimension_value_mm(&self, dimension_id: &str) -> Option<f64> {
        self.dimensions
            .iter()
            .find(|dimension| dimension.dimension_id == dimension_id)
            .map(|dimension| dimension.value_mm)
    }

    pub fn set_variant_analysis_snapshot(
        &mut self,
        variant_id: &str,
        analysis: openagents_cad::contracts::CadAnalysis,
    ) {
        self.variant_analysis_snapshots
            .insert(variant_id.to_string(), analysis.clone());
        if self.active_variant_id == variant_id {
            self.analysis_snapshot = analysis;
        }
    }

    pub fn set_variant_warning_set(
        &mut self,
        variant_id: &str,
        warnings: Vec<CadDemoWarningState>,
    ) {
        self.variant_warning_sets
            .insert(variant_id.to_string(), warnings.clone());
        if self.active_variant_id == variant_id {
            self.warnings = warnings;
            self.warning_hover_index = None;
            self.focused_warning_index = None;
        }
    }

    pub fn begin_agent_build_session(
        &mut self,
        thread_id: &str,
        turn_id: &str,
    ) -> Result<(), String> {
        if self.build_session.phase != CadBuildSessionPhase::Idle {
            self.fail_agent_build_session(
                "cad.build.interrupted",
                format!(
                    "superseded by thread={} turn={}",
                    thread_id.trim(),
                    turn_id.trim()
                ),
                Some("retry previous CAD turn if it is still needed".to_string()),
            )?;
        }
        self.build_session = CadBuildSessionState {
            thread_id: Some(thread_id.trim().to_string()),
            turn_id: Some(turn_id.trim().to_string()),
            ..CadBuildSessionState::default()
        };
        self.transition_agent_build_phase(
            CadBuildSessionPhase::Planning,
            "cad.build.planning.start",
            format!("thread={} turn={}", thread_id.trim(), turn_id.trim()),
        )?;
        Ok(())
    }

    pub fn transition_agent_build_phase(
        &mut self,
        next: CadBuildSessionPhase,
        event_code: &str,
        detail: String,
    ) -> Result<(), String> {
        let current = self.build_session.phase;
        if !current.transition_allowed(next) {
            return Err(format!(
                "invalid CAD build phase transition {} -> {}",
                current.label(),
                next.label()
            ));
        }
        self.build_session.phase = next;
        self.push_agent_build_event(event_code, detail);
        Ok(())
    }

    pub fn record_agent_build_tool_result(&mut self, code: &str, success: bool, message: &str) {
        let status = if success { "ok" } else { "failed" };
        self.build_session.latest_tool_result = Some(format!("{status}:{code}"));
        self.push_agent_build_event(
            "cad.build.tool.result",
            format!("status={status} code={code} message={}", message.trim()),
        );
    }

    pub fn set_agent_build_failure_context(
        &mut self,
        class: CadBuildFailureClass,
        retry_attempts: u8,
        retry_limit: u8,
    ) {
        self.build_session.failure_class = Some(class);
        self.build_session.retry_attempts = retry_attempts;
        self.build_session.retry_limit = retry_limit;
        self.push_agent_build_event(
            "cad.build.failure.context",
            format!(
                "class={} retries={}/{}",
                class.label(),
                retry_attempts,
                retry_limit
            ),
        );
    }

    pub fn record_agent_build_failure_metric(&mut self, class: CadBuildFailureClass) {
        self.build_failure_metrics.record_failure(class);
        self.push_agent_build_event(
            "cad.build.failure.metric",
            format!(
                "class={} count={}",
                class.label(),
                match class {
                    CadBuildFailureClass::ToolTransport => {
                        self.build_failure_metrics.tool_transport_failures
                    }
                    CadBuildFailureClass::IntentParseValidation => {
                        self.build_failure_metrics.intent_parse_failures
                    }
                    CadBuildFailureClass::DispatchRebuild => {
                        self.build_failure_metrics.dispatch_rebuild_failures
                    }
                }
            ),
        );
    }

    pub fn record_agent_build_retry_metric(&mut self, class: CadBuildFailureClass) {
        self.build_failure_metrics.record_retry(class);
        self.push_agent_build_event(
            "cad.build.retry.metric",
            format!(
                "class={} count={}",
                class.label(),
                match class {
                    CadBuildFailureClass::ToolTransport => {
                        self.build_failure_metrics.tool_transport_retries
                    }
                    CadBuildFailureClass::IntentParseValidation => {
                        self.build_failure_metrics.intent_parse_retries
                    }
                    CadBuildFailureClass::DispatchRebuild => {
                        self.build_failure_metrics.dispatch_rebuild_retries
                    }
                }
            ),
        );
    }

    pub fn record_agent_build_rebuild_result(&mut self, trigger: &str, result: &str) {
        self.build_session.latest_rebuild_result = Some(format!(
            "trigger={} result={}",
            trigger.trim(),
            result.trim()
        ));
        self.push_agent_build_event(
            "cad.build.rebuild.result",
            format!("trigger={} result={}", trigger.trim(), result.trim()),
        );
    }

    pub fn complete_agent_build_session(&mut self, summary: String) -> Result<(), String> {
        self.transition_agent_build_phase(CadBuildSessionPhase::Done, "cad.build.done", summary)?;
        self.archive_and_reset_agent_build_session();
        Ok(())
    }

    pub fn fail_agent_build_session(
        &mut self,
        event_code: &str,
        reason: String,
        remediation_hint: Option<String>,
    ) -> Result<(), String> {
        let reason_trimmed = reason.trim().to_string();
        self.build_session.failure_reason = Some(reason_trimmed.clone());
        self.build_session.remediation_hint = remediation_hint;
        self.build_failure_metrics.terminal_failures = self
            .build_failure_metrics
            .terminal_failures
            .saturating_add(1);
        self.transition_agent_build_phase(
            CadBuildSessionPhase::Failed,
            event_code,
            reason_trimmed,
        )?;
        self.archive_and_reset_agent_build_session();
        Ok(())
    }

    fn push_agent_build_event(&mut self, event_code: &str, detail: String) {
        let phase = self.build_session.phase;
        self.build_session.events.push(CadBuildSessionEventState {
            event_code: event_code.trim().to_string(),
            phase,
            detail,
            at_epoch_ms: current_epoch_millis(),
        });
        if self.build_session.events.len() > 64 {
            let overflow = self.build_session.events.len().saturating_sub(64);
            self.build_session.events.drain(0..overflow);
        }
    }

    fn archive_and_reset_agent_build_session(&mut self) {
        let terminal_phase = self.build_session.phase;
        if !matches!(
            terminal_phase,
            CadBuildSessionPhase::Done | CadBuildSessionPhase::Failed
        ) {
            return;
        }
        let thread_id = self
            .build_session
            .thread_id
            .clone()
            .unwrap_or_else(|| "unknown-thread".to_string());
        let turn_id = self
            .build_session
            .turn_id
            .clone()
            .unwrap_or_else(|| "unknown-turn".to_string());
        let archived = CadBuildSessionArchiveState {
            thread_id,
            turn_id,
            terminal_phase,
            failure_class: self.build_session.failure_class,
            retry_attempts: self.build_session.retry_attempts,
            retry_limit: self.build_session.retry_limit,
            latest_tool_result: self.build_session.latest_tool_result.clone(),
            latest_rebuild_result: self.build_session.latest_rebuild_result.clone(),
            failure_reason: self.build_session.failure_reason.clone(),
            remediation_hint: self.build_session.remediation_hint.clone(),
            events: self.build_session.events.clone(),
        };
        if terminal_phase == CadBuildSessionPhase::Failed {
            let latest_event = archived.events.last().cloned();
            tracing::error!(
                "cad build/session failed thread_id={} turn_id={} class={} retries={}/{} tool_result={} rebuild_result={} reason={} hint={} latest_event={} latest_detail={}",
                archived.thread_id,
                archived.turn_id,
                archived
                    .failure_class
                    .map(|class| class.label().to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
                archived.retry_attempts,
                archived.retry_limit,
                archived.latest_tool_result.as_deref().unwrap_or("n/a"),
                archived.latest_rebuild_result.as_deref().unwrap_or("n/a"),
                archived.failure_reason.as_deref().unwrap_or("n/a"),
                archived.remediation_hint.as_deref().unwrap_or("n/a"),
                latest_event
                    .as_ref()
                    .map(|event| event.event_code.as_str())
                    .unwrap_or("n/a"),
                latest_event
                    .as_ref()
                    .map(|event| event.detail.as_str())
                    .unwrap_or("n/a")
            );
        }
        self.last_build_session = Some(archived);
        self.build_session = CadBuildSessionState::default();
    }

    pub fn ensure_chat_session_for_thread(&mut self, thread_id: &str) -> String {
        if let Some(existing) = self.chat_thread_session_bindings.get(thread_id) {
            self.active_chat_session_id = Some(existing.clone());
            self.session_id = existing.clone();
            return existing.clone();
        }
        let normalized = thread_id
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() {
                    ch.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect::<String>()
            .trim_matches('-')
            .to_string();
        let session_id = format!(
            "cad.session.chat.{}",
            if normalized.is_empty() {
                "thread"
            } else {
                normalized.as_str()
            }
        );
        self.chat_thread_session_bindings
            .insert(thread_id.to_string(), session_id.clone());
        self.dispatch_sessions
            .entry(session_id.clone())
            .or_default();
        self.active_chat_session_id = Some(session_id.clone());
        self.session_id = session_id.clone();
        session_id
    }

    pub fn apply_chat_intent_for_thread(
        &mut self,
        thread_id: &str,
        intent: &openagents_cad::intent::CadIntent,
    ) -> openagents_cad::CadResult<openagents_cad::dispatch::CadDispatchReceipt> {
        let session_id = self.ensure_chat_session_for_thread(thread_id);
        let dispatch_state = self
            .dispatch_sessions
            .entry(session_id.clone())
            .or_default();
        let receipt = openagents_cad::dispatch::dispatch_cad_intent(intent, dispatch_state)?;
        self.document_revision = receipt.state_revision;
        self.last_chat_intent_name = Some(intent.intent_name().to_string());
        self.last_action = Some(format!(
            "CAD chat intent {} applied to {} (rev {})",
            intent.intent_name(),
            session_id,
            receipt.state_revision
        ));
        self.last_error = None;
        Ok(receipt)
    }

    pub fn upsert_cad_event(&mut self, event: openagents_cad::events::CadEvent) -> bool {
        if let Some(existing) = self
            .cad_events
            .iter_mut()
            .find(|existing| existing.event_id == event.event_id)
        {
            *existing = event;
            return false;
        }
        self.cad_events.push(event);
        self.cad_events.sort_by(|lhs, rhs| {
            lhs.document_revision
                .cmp(&rhs.document_revision)
                .then_with(|| lhs.event_id.cmp(&rhs.event_id))
        });
        if self.cad_events.len() > 128 {
            let overflow = self.cad_events.len().saturating_sub(128);
            self.cad_events.drain(0..overflow);
        }
        true
    }

    pub fn set_hovered_geometry_for_tile_focus(
        &mut self,
        tile_index: Option<usize>,
        hovered_ref: Option<String>,
    ) -> bool {
        let mut changed = false;
        for (index, viewport) in self.variant_viewports.iter_mut().enumerate() {
            let next_hover = if Some(index) == tile_index {
                hovered_ref.clone()
            } else {
                None
            };
            if viewport.hovered_ref != next_hover {
                viewport.hovered_ref = next_hover;
                changed = true;
            }
        }

        let active_hover = tile_index
            .filter(|index| *index == self.active_variant_tile_index)
            .and(hovered_ref);
        if self.hovered_geometry_ref != active_hover {
            self.hovered_geometry_ref = active_hover;
            changed = true;
        }
        changed
    }

    pub fn record_measurement_snap_point(&mut self, tile_index: usize, snapped: Point) -> bool {
        if tile_index >= self.variant_viewports.len() {
            return false;
        }
        if self.measurement_tile_index != Some(tile_index) || self.measurement_points.len() >= 2 {
            self.measurement_tile_index = Some(tile_index);
            self.measurement_points.clear();
            self.measurement_distance_px = None;
            self.measurement_angle_deg = None;
        }
        self.measurement_points.push(snapped);
        if self.measurement_points.len() < 2 {
            return true;
        }

        let a = self.measurement_points[0];
        let b = self.measurement_points[1];
        let a3 =
            openagents_cad::measurement::CadMeasurePoint3::new(f64::from(a.x), f64::from(a.y), 0.0);
        let b3 =
            openagents_cad::measurement::CadMeasurePoint3::new(f64::from(b.x), f64::from(b.y), 0.0);
        let delta = openagents_cad::measurement::vector_between_points(a3, b3);
        let tolerance = openagents_cad::policy::resolve_tolerance_mm(None);
        self.measurement_distance_px = Some(openagents_cad::measurement::distance_between_points(
            a3, b3, tolerance,
        ));
        self.measurement_angle_deg = openagents_cad::measurement::angle_between_vectors_deg(
            delta,
            openagents_cad::measurement::CadMeasurePoint3::new(1.0, 0.0, 0.0),
            tolerance,
        );
        true
    }

    pub fn reset_camera(&mut self) {
        self.camera_zoom = 1.0;
        self.camera_pan_x = 0.0;
        self.camera_pan_y = 0.0;
        self.camera_orbit_yaw_deg = 26.0;
        self.camera_orbit_pitch_deg = 18.0;
        self.sync_active_variant_viewport_from_global();
    }

    pub fn orbit_camera_by_drag(&mut self, drag_dx: f32, drag_dy: f32) {
        const ORBIT_SENSITIVITY_DEG_PER_PX: f32 = 0.28;
        self.camera_orbit_yaw_deg += drag_dx * ORBIT_SENSITIVITY_DEG_PER_PX;
        self.camera_orbit_pitch_deg = (self.camera_orbit_pitch_deg
            - drag_dy * ORBIT_SENSITIVITY_DEG_PER_PX)
            .clamp(-89.0, 89.0);
        self.sync_active_variant_viewport_from_global();
    }

    pub fn pan_camera_by_drag(&mut self, drag_dx: f32, drag_dy: f32) {
        const PAN_SENSITIVITY: f32 = 1.0;
        self.camera_pan_x = (self.camera_pan_x + drag_dx * PAN_SENSITIVITY).clamp(-800.0, 800.0);
        self.camera_pan_y = (self.camera_pan_y + drag_dy * PAN_SENSITIVITY).clamp(-800.0, 800.0);
        self.sync_active_variant_viewport_from_global();
    }

    pub fn zoom_camera_by_scroll(&mut self, scroll_dy: f32) {
        // Negative wheel deltas (scroll up on most devices) zoom in.
        let scale = (1.0 + (-scroll_dy * 0.0018)).clamp(0.75, 1.35);
        self.camera_zoom = (self.camera_zoom * scale).clamp(0.35, 4.0);
        self.sync_active_variant_viewport_from_global();
    }

    pub fn snap_camera_to_view(&mut self, snap: CadCameraViewSnap) {
        let (yaw_deg, pitch_deg) = snap.orbit_degrees();
        self.camera_orbit_yaw_deg = yaw_deg;
        self.camera_orbit_pitch_deg = pitch_deg;
        self.camera_pan_x = 0.0;
        self.camera_pan_y = 0.0;
        self.sync_active_variant_viewport_from_global();
    }

    pub fn active_view_snap(&self) -> Option<CadCameraViewSnap> {
        const SNAP_TOLERANCE_DEG: f32 = 0.15;
        [
            CadCameraViewSnap::Isometric,
            CadCameraViewSnap::Top,
            CadCameraViewSnap::Front,
            CadCameraViewSnap::Right,
        ]
        .into_iter()
        .find(|snap| {
            let (yaw, pitch) = snap.orbit_degrees();
            (self.camera_orbit_yaw_deg - yaw).abs() <= SNAP_TOLERANCE_DEG
                && (self.camera_orbit_pitch_deg - pitch).abs() <= SNAP_TOLERANCE_DEG
        })
    }

    pub fn cycle_projection_mode(&mut self) {
        self.projection_mode = self.projection_mode.next();
    }

    pub fn cycle_section_axis(&mut self) -> Option<CadSectionAxis> {
        self.section_axis = match self.section_axis {
            None => Some(CadSectionAxis::X),
            Some(axis) if axis == CadSectionAxis::Z => None,
            Some(axis) => Some(axis.next()),
        };
        if self.section_axis.is_none() {
            self.section_offset_normalized = 0.0;
        }
        self.section_axis
    }

    pub fn step_section_offset(&mut self) -> f32 {
        const OFFSETS: [f32; 5] = [-0.4, -0.2, 0.0, 0.2, 0.4];
        if self.section_axis.is_none() {
            self.section_axis = Some(CadSectionAxis::X);
        }
        let current = self.section_offset_normalized;
        let current_index = OFFSETS
            .iter()
            .enumerate()
            .min_by(|(_, lhs), (_, rhs)| {
                (current - *lhs)
                    .abs()
                    .partial_cmp(&(current - *rhs).abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(index, _)| index)
            .unwrap_or(2);
        let next_index = (current_index + 1) % OFFSETS.len();
        self.section_offset_normalized = OFFSETS[next_index];
        self.section_offset_normalized
    }

    pub fn section_summary(&self) -> String {
        match self.section_axis {
            Some(axis) => format!("{}/{}", axis.label(), self.section_offset_normalized),
            None => "off".to_string(),
        }
    }

    pub fn section_plane(&self) -> Option<openagents_cad::section::CadSectionPlane> {
        self.section_axis.map(|axis| {
            openagents_cad::section::CadSectionPlane::new(
                axis.to_cad_section_axis(),
                self.section_offset_normalized,
            )
        })
    }

    pub fn cycle_material_preset(&mut self) -> String {
        let current = self
            .analysis_snapshot
            .material_id
            .as_deref()
            .unwrap_or(openagents_cad::materials::DEFAULT_CAD_MATERIAL_ID);
        let next = openagents_cad::materials::next_material_preset_id(current).to_string();
        self.analysis_snapshot.material_id = Some(next.clone());
        next
    }

    pub fn cycle_hotkey_profile(&mut self) -> Result<(), String> {
        let (profile, bindings) = if self.hotkey_profile == "default" {
            ("compact".to_string(), CadHotkeyBindings::compact_profile())
        } else {
            ("default".to_string(), CadHotkeyBindings::default())
        };
        bindings.validate_conflicts()?;
        self.hotkey_profile = profile;
        self.hotkeys = bindings;
        Ok(())
    }

    pub fn remap_hotkey(&mut self, action: CadHotkeyAction, key: &str) -> Result<(), String> {
        let mut candidate = self.hotkeys.clone();
        candidate.set_key(action, key);
        candidate.validate_conflicts()?;
        self.hotkeys = candidate;
        Ok(())
    }

    pub fn hotkey_matches(&self, action: CadHotkeyAction, value: &str) -> bool {
        self.hotkeys.key_for(action).eq_ignore_ascii_case(value)
    }

    pub fn cycle_three_d_mouse_profile(&mut self) {
        self.three_d_mouse_profile = self.three_d_mouse_profile.next();
    }

    pub fn toggle_three_d_mouse_mode(&mut self) {
        self.three_d_mouse_mode = self.three_d_mouse_mode.next();
    }

    pub fn toggle_three_d_mouse_axis_lock(&mut self, axis: CadThreeDMouseAxis) -> bool {
        self.three_d_mouse_axis_locks.toggle(axis)
    }

    pub fn three_d_mouse_status(&self) -> String {
        if self.three_d_mouse_event_count == 0 {
            return "absent".to_string();
        }
        format!(
            "events={} mode={} profile={} locks[{}]",
            self.three_d_mouse_event_count,
            self.three_d_mouse_mode.label(),
            self.three_d_mouse_profile.label(),
            self.three_d_mouse_axis_locks.summary()
        )
    }

    pub fn apply_three_d_mouse_motion(&mut self, axis_id: u32, value: f64) -> bool {
        const DEADZONE: f32 = 0.02;
        let Some(axis) = CadThreeDMouseAxis::from_motion_axis_id(axis_id) else {
            return false;
        };
        self.three_d_mouse_event_count = self.three_d_mouse_event_count.saturating_add(1);
        let value = value as f32;
        if value.abs() < DEADZONE || self.three_d_mouse_axis_locks.is_locked(axis) {
            return false;
        }
        let speed = self.three_d_mouse_profile.scalar();
        let before = (
            self.camera_zoom,
            self.camera_pan_x,
            self.camera_pan_y,
            self.camera_orbit_yaw_deg,
            self.camera_orbit_pitch_deg,
        );

        match self.three_d_mouse_mode {
            CadThreeDMouseMode::Translate => match axis {
                CadThreeDMouseAxis::X => self.pan_camera_by_drag(value * 18.0 * speed, 0.0),
                CadThreeDMouseAxis::Y => self.pan_camera_by_drag(0.0, value * 18.0 * speed),
                CadThreeDMouseAxis::Z => self.zoom_camera_by_scroll(value * 26.0 * speed),
                CadThreeDMouseAxis::Rx | CadThreeDMouseAxis::Ry | CadThreeDMouseAxis::Rz => {}
            },
            CadThreeDMouseMode::Rotate => match axis {
                CadThreeDMouseAxis::Rx => self.orbit_camera_by_drag(value * 20.0 * speed, 0.0),
                CadThreeDMouseAxis::Ry => self.orbit_camera_by_drag(0.0, value * 20.0 * speed),
                CadThreeDMouseAxis::Rz => self.orbit_camera_by_drag(value * 10.0 * speed, 0.0),
                CadThreeDMouseAxis::Z => self.zoom_camera_by_scroll(value * 26.0 * speed),
                CadThreeDMouseAxis::X | CadThreeDMouseAxis::Y => {}
            },
        }

        let after = (
            self.camera_zoom,
            self.camera_pan_x,
            self.camera_pan_y,
            self.camera_orbit_yaw_deg,
            self.camera_orbit_pitch_deg,
        );
        before != after
    }

    pub fn toggle_snap_mode(&mut self, mode: CadSnapMode) -> bool {
        let target = match mode {
            CadSnapMode::Grid => &mut self.snap_toggles.grid,
            CadSnapMode::Origin => &mut self.snap_toggles.origin,
            CadSnapMode::Endpoint => &mut self.snap_toggles.endpoint,
            CadSnapMode::Midpoint => &mut self.snap_toggles.midpoint,
        };
        *target = !*target;
        *target
    }

    pub fn snap_summary(&self) -> String {
        format!(
            "grid={} origin={} endpoint={} midpoint={}",
            self.snap_toggles.grid as u8,
            self.snap_toggles.origin as u8,
            self.snap_toggles.endpoint as u8,
            self.snap_toggles.midpoint as u8,
        )
    }

    pub fn apply_snap_to_viewport_point(&self, point: Point, viewport: Bounds) -> Point {
        let mut snapped = point;
        if self.snap_toggles.grid {
            const GRID_STEP: f32 = 12.0;
            let grid_x = ((snapped.x - viewport.origin.x) / GRID_STEP).round() * GRID_STEP;
            let grid_y = ((snapped.y - viewport.origin.y) / GRID_STEP).round() * GRID_STEP;
            snapped.x = viewport.origin.x + grid_x;
            snapped.y = viewport.origin.y + grid_y;
        }

        let top_left = viewport.origin;
        let top_right = Point::new(viewport.max_x(), viewport.origin.y);
        let bottom_left = Point::new(viewport.origin.x, viewport.max_y());
        let bottom_right = Point::new(viewport.max_x(), viewport.max_y());
        let center = Point::new(
            viewport.origin.x + viewport.size.width * 0.5,
            viewport.origin.y + viewport.size.height * 0.5,
        );
        let top_mid = Point::new(center.x, viewport.origin.y);
        let bottom_mid = Point::new(center.x, viewport.max_y());
        let left_mid = Point::new(viewport.origin.x, center.y);
        let right_mid = Point::new(viewport.max_x(), center.y);

        if self.snap_toggles.origin {
            snapped = snap_to_anchor_if_near(snapped, center, 16.0);
        }
        if self.snap_toggles.endpoint {
            for anchor in [top_left, top_right, bottom_left, bottom_right] {
                snapped = snap_to_anchor_if_near(snapped, anchor, 14.0);
            }
        }
        if self.snap_toggles.midpoint {
            for anchor in [top_mid, bottom_mid, left_mid, right_mid] {
                snapped = snap_to_anchor_if_near(snapped, anchor, 14.0);
            }
        }

        let min_x = viewport.origin.x.min(viewport.max_x());
        let max_x = viewport.origin.x.max(viewport.max_x());
        let min_y = viewport.origin.y.min(viewport.max_y());
        let max_y = viewport.origin.y.max(viewport.max_y());
        Point::new(snapped.x.clamp(min_x, max_x), snapped.y.clamp(min_y, max_y))
    }

    pub fn infer_context_menu_target_for_viewport_point(
        &self,
        point: Point,
        viewport: Bounds,
    ) -> (CadContextMenuTargetKind, String) {
        if viewport.size.width <= f32::EPSILON {
            return (CadContextMenuTargetKind::Body, "body.main".to_string());
        }
        let normalized_x = ((point.x - viewport.origin.x) / viewport.size.width).clamp(0.0, 1.0);
        if normalized_x < 0.33 {
            (CadContextMenuTargetKind::Body, "body.main".to_string())
        } else if normalized_x < 0.66 {
            (CadContextMenuTargetKind::Face, "face.front".to_string())
        } else {
            (CadContextMenuTargetKind::Edge, "edge.rim".to_string())
        }
    }

    pub fn open_context_menu(
        &mut self,
        anchor: Point,
        target_kind: CadContextMenuTargetKind,
        target_ref: String,
    ) {
        self.set_focused_geometry_for_active_variant(Some(target_ref.clone()));
        self.context_menu = CadContextMenuState {
            is_open: true,
            anchor,
            target_kind,
            target_ref,
            items: context_menu_items_for_target(target_kind),
        };
    }

    pub fn close_context_menu(&mut self) {
        self.context_menu.is_open = false;
        self.context_menu.items.clear();
    }

    pub fn run_context_menu_item(&mut self, index: usize) -> Option<String> {
        let item = self.context_menu.items.get(index)?;
        Some(format!(
            "CAD context action {} on {} ({})",
            item.id,
            self.context_menu.target_ref,
            self.context_menu.target_kind.label()
        ))
    }
}

fn snap_to_anchor_if_near(point: Point, anchor: Point, threshold_px: f32) -> Point {
    let dx = point.x - anchor.x;
    let dy = point.y - anchor.y;
    if (dx * dx + dy * dy) <= threshold_px * threshold_px {
        anchor
    } else {
        point
    }
}

fn context_menu_items_for_target(
    target_kind: CadContextMenuTargetKind,
) -> Vec<CadContextMenuItemState> {
    match target_kind {
        CadContextMenuTargetKind::Body => vec![
            CadContextMenuItemState {
                id: "body.isolate".to_string(),
                label: "Isolate Body".to_string(),
            },
            CadContextMenuItemState {
                id: "body.material".to_string(),
                label: "Assign Material".to_string(),
            },
            CadContextMenuItemState {
                id: "body.measure".to_string(),
                label: "Measure Mass".to_string(),
            },
        ],
        CadContextMenuTargetKind::Face => vec![
            CadContextMenuItemState {
                id: "face.inspect".to_string(),
                label: "Inspect Face".to_string(),
            },
            CadContextMenuItemState {
                id: "face.offset".to_string(),
                label: "Offset Face".to_string(),
            },
            CadContextMenuItemState {
                id: "face.fillet".to_string(),
                label: "Add Fillet".to_string(),
            },
        ],
        CadContextMenuTargetKind::Edge => vec![
            CadContextMenuItemState {
                id: "edge.inspect".to_string(),
                label: "Inspect Edge".to_string(),
            },
            CadContextMenuItemState {
                id: "edge.chamfer".to_string(),
                label: "Chamfer Edge".to_string(),
            },
            CadContextMenuItemState {
                id: "edge.fillet".to_string(),
                label: "Fillet Edge".to_string(),
            },
        ],
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentNetworkSimulationEvent {
    pub seq: u64,
    pub protocol: String,
    pub event_ref: String,
    pub summary: String,
}

pub struct AgentNetworkSimulationPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub channel_name: String,
    pub channel_event_id: Option<String>,
    pub rounds_run: u32,
    pub total_transferred_sats: u64,
    pub learned_skills: Vec<String>,
    pub auto_run_enabled: bool,
    pub auto_run_interval: Duration,
    pub events: Vec<AgentNetworkSimulationEvent>,
    next_seq: u64,
    auto_run_last_tick: Option<Instant>,
}

impl Default for AgentNetworkSimulationPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Run simulation to create NIP-28 coordination channel".to_string()),
            channel_name: "sovereign-agents-lab".to_string(),
            channel_event_id: None,
            rounds_run: 0,
            total_transferred_sats: 0,
            learned_skills: Vec::new(),
            auto_run_enabled: false,
            auto_run_interval: Duration::from_millis(120),
            events: Vec::new(),
            next_seq: 1,
            auto_run_last_tick: None,
        }
    }
}

impl AgentNetworkSimulationPaneState {
    pub fn run_round(&mut self, now_epoch_seconds: u64) -> Result<(), String> {
        let channel_event_id = match self.channel_event_id.clone() {
            Some(existing) => existing,
            None => {
                let metadata = nostr::ChannelMetadata::new(
                    self.channel_name.clone(),
                    "Public SA/SKL/AC negotiation channel",
                    "https://openagents.com/channel.png",
                )
                .with_relays(vec!["wss://relay.openagents.dev".to_string()]);
                let creation = nostr::ChannelCreateEvent::new(metadata, now_epoch_seconds);
                if let Err(error) = creation.content() {
                    self.load_state = PaneLoadState::Error;
                    self.last_error = Some(error.to_string());
                    return Err(error.to_string());
                }
                let id = format!(
                    "sim:{}:{:08x}",
                    nostr::KIND_CHANNEL_CREATION,
                    self.rounds_run + 1
                );
                self.channel_event_id = Some(id.clone());
                self.push_event(
                    "NIP-28",
                    &id,
                    format!(
                        "created channel #{} ({})",
                        self.channel_name, creation.created_at
                    ),
                );
                id
            }
        };

        let round = self.rounds_run.saturating_add(1);
        let relay = "wss://relay.openagents.dev";
        let skill_version = format!("0.{}.0", round + 1);
        let skill_ref = format!("33400:npub1beta:summarize-text:{skill_version}");
        let base = u64::from(round) * 10;

        let announce = nostr::ChannelMessageEvent::new(
            channel_event_id.clone(),
            relay,
            format!(
                "agent-alpha requests summarize-text@{skill_version} for client brief #{round}"
            ),
            now_epoch_seconds.saturating_add(base + 1),
        );
        let announce_id = format!("sim:{}:{:08x}:a", nostr::KIND_CHANNEL_MESSAGE, round);
        self.push_event(
            "NIP-28",
            &announce_id,
            format!(
                "alpha broadcast in channel ({} tags)",
                announce.to_tags().len()
            ),
        );

        let negotiation = nostr::ChannelMessageEvent::reply(
            channel_event_id,
            announce_id.clone(),
            relay,
            format!("agent-beta offers {skill_ref} with AC escrow"),
            now_epoch_seconds.saturating_add(base + 2),
        )
        .mention_pubkey("npub1alpha", Some(relay.to_string()));
        let negotiation_id = format!("sim:{}:{:08x}:b", nostr::KIND_CHANNEL_MESSAGE, round);
        self.push_event(
            "NIP-28",
            &negotiation_id,
            format!(
                "beta replied with terms ({} tags)",
                negotiation.to_tags().len()
            ),
        );

        self.push_event(
            "NIP-SKL",
            &format!("sim:{}:{:08x}", nostr::KIND_SKILL_MANIFEST, round),
            format!("beta published manifest {skill_ref}"),
        );
        self.push_event(
            "NIP-SA",
            &format!("sim:{}:{:08x}", nostr::KIND_SKILL_LICENSE, round),
            format!("alpha learned skill summarize-text@{skill_version}"),
        );
        self.push_event(
            "NIP-AC",
            &format!("sim:{}:{:08x}", nostr::KIND_CREDIT_INTENT, round),
            "opened escrow intent for skill execution".to_string(),
        );
        self.push_event(
            "NIP-AC",
            &format!("sim:{}:{:08x}", nostr::KIND_CREDIT_SETTLEMENT, round),
            "settled escrow after successful delivery".to_string(),
        );

        let transfer_sats = 250_u64.saturating_add(u64::from(round) * 35);
        self.total_transferred_sats = self.total_transferred_sats.saturating_add(transfer_sats);
        self.rounds_run = round;

        if !self.learned_skills.iter().any(|skill| skill == &skill_ref) {
            self.learned_skills.push(skill_ref);
        }

        self.load_state = PaneLoadState::Ready;
        self.last_error = None;
        self.last_action = Some(format!(
            "Round {round}: agents exchanged NIP-28 messages and settled {transfer_sats} sats"
        ));
        Ok(())
    }

    pub fn reset(&mut self) {
        self.load_state = PaneLoadState::Ready;
        self.last_error = None;
        self.last_action = Some("Simulation reset".to_string());
        self.channel_event_id = None;
        self.rounds_run = 0;
        self.total_transferred_sats = 0;
        self.learned_skills.clear();
        self.auto_run_enabled = false;
        self.events.clear();
        self.next_seq = 1;
        self.auto_run_last_tick = None;
    }

    pub fn start_auto_run(&mut self, now: Instant) {
        self.auto_run_enabled = true;
        self.auto_run_last_tick = Some(now);
        self.last_error = None;
        self.last_action = Some("Auto simulation running".to_string());
    }

    pub fn stop_auto_run(&mut self) {
        self.auto_run_enabled = false;
        self.auto_run_last_tick = None;
        self.last_action = Some("Auto simulation paused".to_string());
    }

    pub fn should_run_auto_round(&self, now: Instant) -> bool {
        if !self.auto_run_enabled {
            return false;
        }
        self.auto_run_last_tick
            .is_none_or(|last| now.duration_since(last) >= self.auto_run_interval)
    }

    pub fn mark_auto_round(&mut self, now: Instant) {
        self.auto_run_last_tick = Some(now);
    }

    fn push_event(&mut self, protocol: &str, event_ref: &str, summary: String) {
        self.events.push(AgentNetworkSimulationEvent {
            seq: self.next_seq,
            protocol: protocol.to_string(),
            event_ref: event_ref.to_string(),
            summary,
        });
        self.next_seq = self.next_seq.saturating_add(1);
        if self.events.len() > 18 {
            let overflow = self.events.len().saturating_sub(18);
            self.events.drain(0..overflow);
        }
    }
}

pub struct TreasuryExchangeSimulationPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub rounds_run: u32,
    pub order_event_id: Option<String>,
    pub mint_reference: Option<String>,
    pub wallet_connect_url: Option<String>,
    pub total_liquidity_sats: u64,
    pub trade_volume_sats: u64,
    pub auto_run_enabled: bool,
    pub auto_run_interval: Duration,
    pub events: Vec<AgentNetworkSimulationEvent>,
    next_seq: u64,
    auto_run_last_tick: Option<Instant>,
}

impl Default for TreasuryExchangeSimulationPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some(
                "Run simulation to model treasury + exchange NIP interactions".to_string(),
            ),
            rounds_run: 0,
            order_event_id: None,
            mint_reference: None,
            wallet_connect_url: None,
            total_liquidity_sats: 0,
            trade_volume_sats: 0,
            auto_run_enabled: false,
            auto_run_interval: Duration::from_millis(120),
            events: Vec::new(),
            next_seq: 1,
            auto_run_last_tick: None,
        }
    }
}

impl TreasuryExchangeSimulationPaneState {
    pub fn run_round(&mut self, now_epoch_seconds: u64) -> Result<(), String> {
        let round = self.rounds_run.saturating_add(1);
        let swap_sats = 40_000_u64.saturating_add(u64::from(round) * 5_000);

        let handler = nostr::nip89::HandlerInfo::new(
            "npub1treasury",
            nostr::nip89::HandlerType::Agent,
            nostr::nip89::HandlerMetadata::new(
                "Treasury Agent",
                "Provides FX and liquidity routing for agent payments",
            ),
        )
        .add_capability("fx.quote.btcusd")
        .add_capability("swap.cashu.lightning")
        .with_pricing(
            nostr::nip89::PricingInfo::new(25)
                .with_model("per-swap")
                .with_currency("sat"),
        );
        let handler_tags = handler.to_tags();
        self.push_event(
            "NIP-89",
            &format!("sim:{}:{:08x}", nostr::nip89::KIND_HANDLER_INFO, round),
            format!("announced treasury handler ({} tags)", handler_tags.len()),
        );

        let mint_tags = nostr::nip87::create_cashu_mint_tags(
            "mint-pubkey-alpha",
            "https://mint.openagents.dev",
            &[1, 2, 3, 4, 5, 11, 12],
            nostr::nip87::MintNetwork::Mainnet,
        );
        let mint = nostr::nip87::parse_cashu_mint(
            nostr::nip87::KIND_CASHU_MINT,
            &mint_tags,
            "{\"name\":\"OpenAgents Mint\"}",
        )
        .map_err(|error| error.to_string())?;
        self.mint_reference = Some(format!("{} ({})", mint.url, mint.network.as_str()));
        self.push_event(
            "NIP-87",
            &format!("sim:{}:{:08x}", nostr::nip87::KIND_CASHU_MINT, round),
            format!("discovered mint {} with {} nuts", mint.url, mint.nuts.len()),
        );

        let order_id = format!("order-{:04}", round);
        let expires_at = now_epoch_seconds.saturating_add(900);
        let order_event = nostr::Event {
            id: format!("sim:{}:{:08x}", nostr::nip69::P2P_ORDER_KIND, round),
            pubkey: "npub1treasury".to_string(),
            created_at: now_epoch_seconds,
            kind: nostr::nip69::P2P_ORDER_KIND,
            tags: vec![
                vec!["d".to_string(), order_id.clone()],
                vec!["k".to_string(), "sell".to_string()],
                vec!["f".to_string(), "USD".to_string()],
                vec!["s".to_string(), "pending".to_string()],
                vec!["amt".to_string(), swap_sats.to_string()],
                vec!["fa".to_string(), "1250".to_string()],
                vec!["pm".to_string(), "wire".to_string(), "cashapp".to_string()],
                vec!["premium".to_string(), "1.5".to_string()],
                vec!["network".to_string(), "bitcoin".to_string()],
                vec!["layer".to_string(), "lightning".to_string()],
                vec!["expires_at".to_string(), expires_at.to_string()],
                vec!["expiration".to_string(), expires_at.to_string()],
                vec!["y".to_string(), "openagents-exchange".to_string()],
            ],
            content: String::new(),
            sig: format!("sim-signature-{round}"),
        };
        let order = nostr::nip69::P2POrder::from_event(order_event.clone())
            .map_err(|error| error.to_string())?;
        self.order_event_id = Some(order_event.id.clone());
        self.push_event(
            "NIP-69",
            &order_event.id,
            format!(
                "published {} order {} for {} sats",
                order.order_type.as_str(),
                order.order_id,
                order.amount_sats
            ),
        );

        let token_content = nostr::nip60::TokenContent::new(
            mint.url.clone(),
            vec![nostr::nip60::CashuProof::new(
                format!("proof-{round:04}"),
                swap_sats,
                format!("secret-{round:04}"),
                format!("C-{round:04}"),
            )],
        )
        .with_unit("sat".to_string());
        let locked_sats = token_content.total_amount();
        self.total_liquidity_sats = self.total_liquidity_sats.saturating_add(locked_sats);
        self.push_event(
            "NIP-60",
            &format!("sim:{}:{:08x}", nostr::nip60::TOKEN_KIND, round),
            format!("locked {} sats in wallet token batch", locked_sats),
        );

        let nutzap_proof = nostr::nip61::NutzapProof::new(
            swap_sats,
            format!("C-{round:04}"),
            format!("proof-{round:04}"),
            format!("secret-{round:04}"),
        );
        let proof_tag =
            nostr::nip61::create_proof_tag(&nutzap_proof).map_err(|error| error.to_string())?;
        self.push_event(
            "NIP-61",
            &format!("sim:{}:{:08x}", nostr::nip61::NUTZAP_KIND, round),
            format!(
                "created nutzap settlement proof ({} fields)",
                proof_tag.len()
            ),
        );

        let wallet_connect_url = nostr::nip47::NostrWalletConnectUrl::new(
            "walletpubkey123",
            vec!["wss://relay.openagents.dev".to_string()],
            format!("secret-{round:04}"),
        )
        .with_lud16("treasury@openagents.dev")
        .to_string();
        self.wallet_connect_url = Some(wallet_connect_url.clone());
        self.push_event(
            "NIP-47",
            &format!("sim:{}:{:08x}", nostr::nip47::REQUEST_KIND, round),
            format!(
                "prepared wallet connect session ({} chars)",
                wallet_connect_url.len()
            ),
        );

        let reputation_label = nostr::nip32::LabelEvent::new(
            vec![nostr::nip32::Label::new("success", "exchange/trade")],
            vec![nostr::nip32::LabelTarget::event(
                order_event.id.clone(),
                Some("wss://relay.openagents.dev"),
            )],
        )
        .with_content("atomic swap completed within policy bounds");
        self.push_event(
            "NIP-32",
            &format!("sim:{}:{:08x}", nostr::nip32::KIND_LABEL, round),
            format!(
                "emitted trade attestation ({} tags)",
                reputation_label.to_tags().len()
            ),
        );

        self.rounds_run = round;
        self.trade_volume_sats = self.trade_volume_sats.saturating_add(swap_sats);
        self.load_state = PaneLoadState::Ready;
        self.last_error = None;
        self.last_action = Some(format!(
            "Round {round}: routed {} sats through discovery, orderbook, wallet and settlement rails",
            swap_sats
        ));
        Ok(())
    }

    pub fn reset(&mut self) {
        self.load_state = PaneLoadState::Ready;
        self.last_error = None;
        self.last_action = Some("Treasury exchange simulation reset".to_string());
        self.rounds_run = 0;
        self.order_event_id = None;
        self.mint_reference = None;
        self.wallet_connect_url = None;
        self.total_liquidity_sats = 0;
        self.trade_volume_sats = 0;
        self.auto_run_enabled = false;
        self.events.clear();
        self.next_seq = 1;
        self.auto_run_last_tick = None;
    }

    pub fn start_auto_run(&mut self, now: Instant) {
        self.auto_run_enabled = true;
        self.auto_run_last_tick = Some(now);
        self.last_error = None;
        self.last_action = Some("Auto treasury simulation running".to_string());
    }

    pub fn stop_auto_run(&mut self) {
        self.auto_run_enabled = false;
        self.auto_run_last_tick = None;
        self.last_action = Some("Auto treasury simulation paused".to_string());
    }

    pub fn should_run_auto_round(&self, now: Instant) -> bool {
        if !self.auto_run_enabled {
            return false;
        }
        self.auto_run_last_tick
            .is_none_or(|last| now.duration_since(last) >= self.auto_run_interval)
    }

    pub fn mark_auto_round(&mut self, now: Instant) {
        self.auto_run_last_tick = Some(now);
    }

    fn push_event(&mut self, protocol: &str, event_ref: &str, summary: String) {
        self.events.push(AgentNetworkSimulationEvent {
            seq: self.next_seq,
            protocol: protocol.to_string(),
            event_ref: event_ref.to_string(),
            summary,
        });
        self.next_seq = self.next_seq.saturating_add(1);
        if self.events.len() > 18 {
            let overflow = self.events.len().saturating_sub(18);
            self.events.drain(0..overflow);
        }
    }
}

pub struct RelaySecuritySimulationPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub relay_url: String,
    pub challenge: String,
    pub auth_event_id: Option<String>,
    pub rounds_run: u32,
    pub dm_relay_count: u32,
    pub sync_ranges: u32,
    pub auto_run_enabled: bool,
    pub auto_run_interval: Duration,
    pub events: Vec<AgentNetworkSimulationEvent>,
    next_seq: u64,
    auto_run_last_tick: Option<Instant>,
}

impl Default for RelaySecuritySimulationPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some(
                "Run simulation to model secure relay, auth, and sync lifecycle".to_string(),
            ),
            relay_url: "wss://relay.openagents.dev".to_string(),
            challenge: "auth-bootstrap".to_string(),
            auth_event_id: None,
            rounds_run: 0,
            dm_relay_count: 0,
            sync_ranges: 0,
            auto_run_enabled: false,
            auto_run_interval: Duration::from_millis(120),
            events: Vec::new(),
            next_seq: 1,
            auto_run_last_tick: None,
        }
    }
}

impl RelaySecuritySimulationPaneState {
    pub fn run_round(&mut self, now_epoch_seconds: u64) -> Result<(), String> {
        let round = self.rounds_run.saturating_add(1);

        let relay_doc = nostr::nip11::RelayInformationDocument {
            name: Some("OpenAgents Auth Relay".to_string()),
            description: Some("Relay exposing auth, DM, and audit sync capabilities".to_string()),
            supported_nips: Some(vec![11, 17, 42, 46, 65, 77, 98]),
            limitation: Some(nostr::nip11::RelayLimitation {
                auth_required: Some(true),
                restricted_writes: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        };
        let relay_doc_json = relay_doc.to_json().map_err(|error| error.to_string())?;
        let relay_doc_roundtrip =
            nostr::nip11::RelayInformationDocument::from_json(&relay_doc_json)
                .map_err(|error| error.to_string())?;
        self.push_event(
            "NIP-11",
            &format!("sim:30111:{:08x}", round),
            format!(
                "loaded relay document ({} advertised nips)",
                relay_doc_roundtrip
                    .supported_nips
                    .map_or(0, |nips| nips.len())
            ),
        );

        let relay_list = nostr::nip65::RelayListMetadata::new(vec![
            nostr::nip65::RelayEntry::write(self.relay_url.clone()),
            nostr::nip65::RelayEntry::read("wss://relay.backup.openagents.dev".to_string()),
        ]);
        self.dm_relay_count = relay_list.all_relays().len() as u32;
        self.push_event(
            "NIP-65",
            &format!(
                "sim:{}:{:08x}",
                nostr::nip65::RELAY_LIST_METADATA_KIND,
                round
            ),
            format!(
                "published relay list (read={} write={})",
                relay_list.read_relays().len(),
                relay_list.write_relays().len()
            ),
        );

        let challenge = format!("auth-{round:04x}");
        self.challenge.clone_from(&challenge);
        let auth_template = nostr::nip42::create_auth_event_template(&self.relay_url, &challenge);
        let auth_event = nostr::Event {
            id: format!("sim:{}:{:08x}", nostr::nip42::AUTH_KIND, round),
            pubkey: "npub1agentalpha".to_string(),
            created_at: auth_template.created_at,
            kind: auth_template.kind,
            tags: auth_template.tags,
            content: auth_template.content,
            sig: format!("sim-auth-signature-{round}"),
        };
        nostr::nip42::validate_auth_event(
            &auth_event,
            &self.relay_url,
            &challenge,
            Some(auth_event.created_at),
        )
        .map_err(|error| error.to_string())?;
        self.auth_event_id = Some(auth_event.id.clone());
        self.push_event(
            "NIP-42",
            &auth_event.id,
            "validated relay authentication event".to_string(),
        );

        let signer_request = nostr::nip46::NostrConnectRequest::get_public_key();
        let signer_request_json = signer_request
            .to_json()
            .map_err(|error| error.to_string())?;
        self.push_event(
            "NIP-46",
            &format!("sim:{}:{:08x}", nostr::nip46::KIND_NOSTR_CONNECT, round),
            format!(
                "queued remote signing request {} ({} bytes)",
                signer_request.id,
                signer_request_json.len()
            ),
        );

        let sender_sk = nostr::generate_secret_key();
        let recipient_sk = nostr::generate_secret_key();
        let recipient_pk = nostr::get_public_key_hex(&recipient_sk).map_err(|e| e.to_string())?;
        let message =
            nostr::nip17::ChatMessage::new(format!("relay-auth heartbeat {} confirmed", round))
                .add_recipient(recipient_pk.clone(), Some(self.relay_url.clone()))
                .subject("secure-ops");
        let wrapped = nostr::nip17::send_chat_message(
            &message,
            &sender_sk,
            &recipient_pk,
            now_epoch_seconds.saturating_add(3),
        )
        .map_err(|error| error.to_string())?;
        let received = nostr::nip17::receive_chat_message(&wrapped, &recipient_sk)
            .map_err(|e| e.to_string())?;
        self.push_event(
            "NIP-17",
            &format!("sim:{}:{:08x}", nostr::nip17::KIND_CHAT_MESSAGE, round),
            format!(
                "sent private chat message to {} recipient(s)",
                received.recipients.len()
            ),
        );
        self.push_event(
            "NIP-59",
            &format!("sim:{}:{:08x}", nostr::nip59::KIND_GIFT_WRAP, round),
            format!("wrapped private message event (kind {})", wrapped.kind),
        );

        let endpoint = format!("https://api.openagents.dev/v1/relay/check/{round}");
        let payload = format!(
            "{{\"challenge\":\"{}\",\"relay\":\"{}\"}}",
            challenge, self.relay_url
        );
        let payload_hash = nostr::nip98::hash_payload(payload.as_bytes());
        let http_auth =
            nostr::nip98::HttpAuth::new(endpoint.clone(), nostr::nip98::HttpMethod::Post)
                .with_payload_hash(payload_hash.clone());
        let http_auth_tags = http_auth.to_tags();
        let validation = nostr::nip98::ValidationParams::new(
            endpoint,
            nostr::nip98::HttpMethod::Post,
            now_epoch_seconds.saturating_add(4),
        )
        .with_payload_hash(payload_hash)
        .with_timestamp_window(120);
        nostr::nip98::validate_http_auth_event(
            nostr::nip98::KIND_HTTP_AUTH,
            now_epoch_seconds.saturating_add(4),
            &http_auth_tags,
            &validation,
        )
        .map_err(|error| error.to_string())?;
        self.push_event(
            "NIP-98",
            &format!("sim:{}:{:08x}", nostr::nip98::KIND_HTTP_AUTH, round),
            "validated HTTP auth request tags".to_string(),
        );

        let round_byte = (round % u32::from(u8::MAX.saturating_sub(2))).saturating_add(1) as u8;
        let mut records = vec![
            nostr::nip77::Record::new(now_epoch_seconds.saturating_add(1), [round_byte; 32]),
            nostr::nip77::Record::new(
                now_epoch_seconds.saturating_add(2),
                [round_byte.saturating_add(1); 32],
            ),
            nostr::nip77::Record::new(
                now_epoch_seconds.saturating_add(3),
                [round_byte.saturating_add(2); 32],
            ),
        ];
        nostr::nip77::sort_records(&mut records);
        let ids: Vec<nostr::nip77::EventId> = records.iter().map(|record| record.id).collect();
        let fingerprint = nostr::nip77::calculate_fingerprint(&ids);
        let negentropy =
            nostr::nip77::NegentropyMessage::new(vec![nostr::nip77::Range::fingerprint(
                nostr::nip77::Bound::infinity(),
                fingerprint,
            )]);
        let encoded = negentropy.encode_hex().map_err(|error| error.to_string())?;
        let decoded =
            nostr::nip77::NegentropyMessage::decode_hex(&encoded).map_err(|e| e.to_string())?;
        self.sync_ranges = self.sync_ranges.saturating_add(decoded.ranges.len() as u32);
        self.push_event(
            "NIP-77",
            &format!("sim:30077:{:08x}", round),
            format!(
                "reconciled negentropy message ({} hex chars)",
                encoded.len()
            ),
        );

        self.rounds_run = round;
        self.load_state = PaneLoadState::Ready;
        self.last_error = None;
        self.last_action = Some(format!(
            "Round {round}: relay auth, private messaging, HTTP auth, and negentropy sync succeeded"
        ));
        Ok(())
    }

    pub fn reset(&mut self) {
        self.load_state = PaneLoadState::Ready;
        self.last_error = None;
        self.last_action = Some("Relay security simulation reset".to_string());
        self.challenge = "auth-bootstrap".to_string();
        self.auth_event_id = None;
        self.rounds_run = 0;
        self.dm_relay_count = 0;
        self.sync_ranges = 0;
        self.auto_run_enabled = false;
        self.events.clear();
        self.next_seq = 1;
        self.auto_run_last_tick = None;
    }

    pub fn start_auto_run(&mut self, now: Instant) {
        self.auto_run_enabled = true;
        self.auto_run_last_tick = Some(now);
        self.last_error = None;
        self.last_action = Some("Auto relay security simulation running".to_string());
    }

    pub fn stop_auto_run(&mut self) {
        self.auto_run_enabled = false;
        self.auto_run_last_tick = None;
        self.last_action = Some("Auto relay security simulation paused".to_string());
    }

    pub fn should_run_auto_round(&self, now: Instant) -> bool {
        if !self.auto_run_enabled {
            return false;
        }
        self.auto_run_last_tick
            .is_none_or(|last| now.duration_since(last) >= self.auto_run_interval)
    }

    pub fn mark_auto_round(&mut self, now: Instant) {
        self.auto_run_last_tick = Some(now);
    }

    fn push_event(&mut self, protocol: &str, event_ref: &str, summary: String) {
        self.events.push(AgentNetworkSimulationEvent {
            seq: self.next_seq,
            protocol: protocol.to_string(),
            event_ref: event_ref.to_string(),
            summary,
        });
        self.next_seq = self.next_seq.saturating_add(1);
        if self.events.len() > 18 {
            let overflow = self.events.len().saturating_sub(18);
            self.events.drain(0..overflow);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StableSatsWalletMode {
    Btc,
    Usd,
}

impl StableSatsWalletMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Btc => "BTC",
            Self::Usd => "USD",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StableSatsWalletOwnerKind {
    Operator,
    SovereignAgent,
}

impl StableSatsWalletOwnerKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Operator => "operator",
            Self::SovereignAgent => "sovereign_agent",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StableSatsSimulationMode {
    Demo,
    RealBlink,
}

impl StableSatsSimulationMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Demo => "demo",
            Self::RealBlink => "real",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StableSatsAgentWalletState {
    pub agent_name: String,
    pub owner_kind: StableSatsWalletOwnerKind,
    pub owner_id: String,
    pub credential_key_name: String,
    pub credential_url_name: Option<String>,
    pub btc_balance_sats: u64,
    pub usd_balance_cents: u64,
    pub active_wallet: StableSatsWalletMode,
    pub switch_count: u32,
    pub last_switch_summary: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StableSatsTransferAsset {
    BtcSats,
    UsdCents,
}

impl StableSatsTransferAsset {
    pub const fn label(self) -> &'static str {
        match self {
            Self::BtcSats => "BTC",
            Self::UsdCents => "USD",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StableSatsTransferStatus {
    Settled,
    Failed,
}

impl StableSatsTransferStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Settled => "settled",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StableSatsTransferLedgerEntry {
    pub seq: u64,
    pub transfer_ref: String,
    pub from_wallet: String,
    pub to_wallet: String,
    pub asset: StableSatsTransferAsset,
    pub amount: u64,
    pub effective_fee: u64,
    pub status: StableSatsTransferStatus,
    pub summary: String,
    pub occurred_at_epoch_seconds: u64,
}

pub struct StableSatsSimulationPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub mode: StableSatsSimulationMode,
    pub rounds_run: u32,
    pub price_usd_cents_per_btc: u64,
    pub total_converted_sats: u64,
    pub total_converted_usd_cents: u64,
    pub last_settlement_ref: Option<String>,
    pub agents: Vec<StableSatsAgentWalletState>,
    pub price_history_usd_cents_per_btc: Vec<u64>,
    pub converted_sats_history: Vec<u64>,
    pub auto_run_enabled: bool,
    pub auto_run_interval: Duration,
    pub live_refresh_pending: bool,
    pub active_live_refresh_request_id: Option<u64>,
    pub transfer_ledger: Vec<StableSatsTransferLedgerEntry>,
    pub events: Vec<AgentNetworkSimulationEvent>,
    next_seq: u64,
    next_transfer_seq: u64,
    auto_run_last_tick: Option<Instant>,
    next_live_refresh_request_id: u64,
}

impl Default for StableSatsSimulationPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some(
                "Run simulation to model StableSats wallet switches (BTC <-> USD)".to_string(),
            ),
            mode: StableSatsSimulationMode::Demo,
            rounds_run: 0,
            price_usd_cents_per_btc: Self::BASE_PRICE_USD_CENTS_PER_BTC,
            total_converted_sats: 0,
            total_converted_usd_cents: 0,
            last_settlement_ref: None,
            agents: Self::default_agents(),
            price_history_usd_cents_per_btc: Vec::new(),
            converted_sats_history: Vec::new(),
            auto_run_enabled: false,
            auto_run_interval: Duration::from_millis(120),
            live_refresh_pending: false,
            active_live_refresh_request_id: None,
            transfer_ledger: Vec::new(),
            events: Vec::new(),
            next_seq: 1,
            next_transfer_seq: 1,
            auto_run_last_tick: None,
            next_live_refresh_request_id: 1,
        }
    }
}

impl StableSatsSimulationPaneState {
    const SATS_PER_BTC: u128 = 100_000_000;
    const BASE_PRICE_USD_CENTS_PER_BTC: u64 = 8_400_000;
    const PRICE_STEP_USD_CENTS_PER_BTC: u64 = 12_500;

    fn default_agents() -> Vec<StableSatsAgentWalletState> {
        vec![
            StableSatsAgentWalletState {
                agent_name: "agent-alpha".to_string(),
                owner_kind: StableSatsWalletOwnerKind::SovereignAgent,
                owner_id: "sa:agent-alpha".to_string(),
                credential_key_name: "BLINK_API_KEY_SA_ALPHA".to_string(),
                credential_url_name: Some("BLINK_API_URL_SA_ALPHA".to_string()),
                btc_balance_sats: 260_000,
                usd_balance_cents: 42_000,
                active_wallet: StableSatsWalletMode::Btc,
                switch_count: 0,
                last_switch_summary: "none".to_string(),
            },
            StableSatsAgentWalletState {
                agent_name: "agent-beta".to_string(),
                owner_kind: StableSatsWalletOwnerKind::SovereignAgent,
                owner_id: "sa:agent-beta".to_string(),
                credential_key_name: "BLINK_API_KEY_SA_BETA".to_string(),
                credential_url_name: Some("BLINK_API_URL_SA_BETA".to_string()),
                btc_balance_sats: 180_000,
                usd_balance_cents: 64_000,
                active_wallet: StableSatsWalletMode::Usd,
                switch_count: 0,
                last_switch_summary: "none".to_string(),
            },
            StableSatsAgentWalletState {
                agent_name: "agent-gamma".to_string(),
                owner_kind: StableSatsWalletOwnerKind::SovereignAgent,
                owner_id: "sa:agent-gamma".to_string(),
                credential_key_name: "BLINK_API_KEY_SA_GAMMA".to_string(),
                credential_url_name: Some("BLINK_API_URL_SA_GAMMA".to_string()),
                btc_balance_sats: 120_000,
                usd_balance_cents: 36_000,
                active_wallet: StableSatsWalletMode::Btc,
                switch_count: 0,
                last_switch_summary: "none".to_string(),
            },
        ]
    }

    pub fn total_btc_balance_sats(&self) -> u64 {
        self.agents.iter().map(|agent| agent.btc_balance_sats).sum()
    }

    pub fn total_usd_balance_cents(&self) -> u64 {
        self.agents
            .iter()
            .map(|agent| agent.usd_balance_cents)
            .sum()
    }

    pub fn set_mode(&mut self, mode: StableSatsSimulationMode) {
        if self.mode == mode {
            return;
        }
        self.mode = mode;
        self.load_state = PaneLoadState::Ready;
        self.auto_run_enabled = false;
        self.auto_run_last_tick = None;
        self.last_error = None;
        self.rounds_run = 0;
        self.price_usd_cents_per_btc = Self::BASE_PRICE_USD_CENTS_PER_BTC;
        self.total_converted_sats = 0;
        self.total_converted_usd_cents = 0;
        self.last_settlement_ref = None;
        self.price_history_usd_cents_per_btc.clear();
        self.converted_sats_history.clear();
        self.live_refresh_pending = false;
        self.active_live_refresh_request_id = None;
        self.transfer_ledger.clear();
        self.events.clear();
        self.next_seq = 1;
        self.next_transfer_seq = 1;
        self.next_live_refresh_request_id = 1;
        match mode {
            StableSatsSimulationMode::Demo => {
                self.agents = Self::default_agents();
                self.last_action = Some("StableSats mode switched to demo simulation".to_string());
            }
            StableSatsSimulationMode::RealBlink => {
                self.agents = Self::default_real_wallets();
                self.last_action =
                    Some("StableSats mode switched to real Blink integration".to_string());
            }
        }
    }

    pub fn begin_live_refresh(&mut self) -> Result<u64, String> {
        if self.mode != StableSatsSimulationMode::RealBlink {
            return Err("Live refresh requires StableSats real mode".to_string());
        }
        if self.live_refresh_pending {
            return Err("Live Blink refresh already in progress".to_string());
        }
        let request_id = self.next_live_refresh_request_id;
        self.next_live_refresh_request_id = self.next_live_refresh_request_id.saturating_add(1);
        self.live_refresh_pending = true;
        self.active_live_refresh_request_id = Some(request_id);
        self.load_state = PaneLoadState::Loading;
        self.last_error = None;
        self.last_action = Some(format!(
            "Refreshing live Blink balances (request #{request_id})"
        ));
        Ok(request_id)
    }

    pub fn finish_live_refresh(&mut self, request_id: u64) -> bool {
        if self.active_live_refresh_request_id != Some(request_id) {
            return false;
        }
        self.live_refresh_pending = false;
        self.active_live_refresh_request_id = None;
        true
    }

    pub fn fail_live_refresh(&mut self, request_id: u64, error: String) -> bool {
        if !self.finish_live_refresh(request_id) {
            return false;
        }
        self.load_state = PaneLoadState::Error;
        self.last_error = Some(error.clone());
        self.last_action = Some(format!("Live Blink refresh failed: {error}"));
        true
    }

    pub fn run_round(&mut self, now_epoch_seconds: u64) -> Result<(), String> {
        if self.agents.is_empty() {
            self.load_state = PaneLoadState::Error;
            let error = "No agents configured for StableSats simulation".to_string();
            self.last_error = Some(error.clone());
            return Err(error);
        }

        let round = self.rounds_run.saturating_add(1);
        let price = Self::BASE_PRICE_USD_CENTS_PER_BTC
            .saturating_add(u64::from(round) * Self::PRICE_STEP_USD_CENTS_PER_BTC);
        self.price_usd_cents_per_btc = price;

        let quote_ref = format!("sim:blink:price:{round:04}:{now_epoch_seconds}");
        self.push_event(
            "BLINK-PRICE",
            &quote_ref,
            format!("quoted BTC/USD at {}", Self::format_usd_cents(price)),
        );

        let mut converted_sats_round = 0_u64;
        let mut converted_usd_round = 0_u64;
        let mut agents_switched = 0_u32;

        for index in 0..self.agents.len() {
            let index_u64 = index as u64;
            let event_ref = format!("sim:blink:swap:{round:04}:{index:02}");
            let mut transfer_record: Option<(
                String,
                String,
                StableSatsTransferAsset,
                u64,
                String,
            )> = None;
            let event_summary = {
                let agent = &mut self.agents[index];
                match agent.active_wallet {
                    StableSatsWalletMode::Btc => {
                        let target_sats = 6_000_u64
                            .saturating_add(u64::from(round) * 500)
                            .saturating_add(index_u64 * 400);
                        let switch_sats = target_sats.min(agent.btc_balance_sats);
                        if switch_sats == 0 {
                            None
                        } else {
                            let credited_usd = Self::sats_to_usd_cents(switch_sats, price);
                            agent.btc_balance_sats =
                                agent.btc_balance_sats.saturating_sub(switch_sats);
                            agent.usd_balance_cents =
                                agent.usd_balance_cents.saturating_add(credited_usd);
                            agent.active_wallet = StableSatsWalletMode::Usd;
                            agent.switch_count = agent.switch_count.saturating_add(1);
                            agent.last_switch_summary = format!(
                                "BTC->USD {} sats -> {}",
                                switch_sats,
                                Self::format_usd_cents(credited_usd)
                            );

                            converted_sats_round = converted_sats_round.saturating_add(switch_sats);
                            converted_usd_round = converted_usd_round.saturating_add(credited_usd);
                            agents_switched = agents_switched.saturating_add(1);
                            let from_wallet = format!("{}:BTC", agent.agent_name);
                            let to_wallet = format!("{}:USD", agent.agent_name);
                            let summary = format!(
                                "{} switched {} sats to {}",
                                agent.agent_name,
                                switch_sats,
                                Self::format_usd_cents(credited_usd)
                            );
                            transfer_record = Some((
                                from_wallet,
                                to_wallet,
                                StableSatsTransferAsset::BtcSats,
                                switch_sats,
                                summary.clone(),
                            ));
                            Some(format!(
                                "{} switched {} sats to {}",
                                agent.agent_name,
                                switch_sats,
                                Self::format_usd_cents(credited_usd)
                            ))
                        }
                    }
                    StableSatsWalletMode::Usd => {
                        let target_usd = 280_u64
                            .saturating_add(u64::from(round) * 25)
                            .saturating_add(index_u64 * 20);
                        let switch_usd = target_usd.min(agent.usd_balance_cents);
                        if switch_usd == 0 {
                            None
                        } else {
                            let credited_sats = Self::usd_cents_to_sats(switch_usd, price);
                            agent.usd_balance_cents =
                                agent.usd_balance_cents.saturating_sub(switch_usd);
                            agent.btc_balance_sats =
                                agent.btc_balance_sats.saturating_add(credited_sats);
                            agent.active_wallet = StableSatsWalletMode::Btc;
                            agent.switch_count = agent.switch_count.saturating_add(1);
                            agent.last_switch_summary = format!(
                                "USD->BTC {} -> {} sats",
                                Self::format_usd_cents(switch_usd),
                                credited_sats
                            );

                            converted_sats_round =
                                converted_sats_round.saturating_add(credited_sats);
                            converted_usd_round = converted_usd_round.saturating_add(switch_usd);
                            agents_switched = agents_switched.saturating_add(1);
                            let from_wallet = format!("{}:USD", agent.agent_name);
                            let to_wallet = format!("{}:BTC", agent.agent_name);
                            let summary = format!(
                                "{} switched {} to {} sats",
                                agent.agent_name,
                                Self::format_usd_cents(switch_usd),
                                credited_sats
                            );
                            transfer_record = Some((
                                from_wallet,
                                to_wallet,
                                StableSatsTransferAsset::UsdCents,
                                switch_usd,
                                summary.clone(),
                            ));
                            Some(format!(
                                "{} switched {} to {} sats",
                                agent.agent_name,
                                Self::format_usd_cents(switch_usd),
                                credited_sats
                            ))
                        }
                    }
                }
            };
            if let Some((from_wallet, to_wallet, asset, amount, summary)) = transfer_record {
                self.push_transfer_ledger_entry(
                    now_epoch_seconds,
                    format!("sim:blink:transfer:{round:04}:{index:02}"),
                    from_wallet,
                    to_wallet,
                    asset,
                    amount,
                    0,
                    StableSatsTransferStatus::Settled,
                    summary,
                );
            }

            if let Some(summary) = event_summary {
                self.push_event("BLINK-SWAP", &event_ref, summary);
            }
        }

        self.total_converted_sats = self
            .total_converted_sats
            .saturating_add(converted_sats_round);
        self.total_converted_usd_cents = self
            .total_converted_usd_cents
            .saturating_add(converted_usd_round);
        self.rounds_run = round;
        self.last_settlement_ref = Some(format!("sim:blink:settlement:{round:04}"));
        self.price_history_usd_cents_per_btc.push(price);
        if self.price_history_usd_cents_per_btc.len() > 18 {
            let overflow = self
                .price_history_usd_cents_per_btc
                .len()
                .saturating_sub(18);
            self.price_history_usd_cents_per_btc.drain(0..overflow);
        }
        self.converted_sats_history.push(converted_sats_round);
        if self.converted_sats_history.len() > 18 {
            let overflow = self.converted_sats_history.len().saturating_sub(18);
            self.converted_sats_history.drain(0..overflow);
        }
        if let Some(settlement_ref) = self.last_settlement_ref.clone() {
            self.push_event(
                "BLINK-LEDGER",
                &settlement_ref,
                format!(
                    "settled {} wallet switches ({} total, {} total)",
                    agents_switched,
                    self.total_converted_sats,
                    Self::format_usd_cents(self.total_converted_usd_cents)
                ),
            );
        }

        self.load_state = PaneLoadState::Ready;
        self.last_error = None;
        self.last_action = Some(format!(
            "Round {round}: {} agents switched wallets at {} per BTC",
            agents_switched,
            Self::format_usd_cents(price)
        ));
        Ok(())
    }

    pub fn apply_live_wallet_snapshots(
        &mut self,
        now_epoch_seconds: u64,
        price_usd_cents_per_btc: u64,
        wallet_snapshots: &[(String, u64, u64, String)],
        wallet_failures: &[(String, String)],
    ) {
        if self.agents.len() < 3 {
            self.agents = Self::default_real_wallets();
        }
        let round = self.rounds_run.saturating_add(1);
        let mut converted_sats_round = 0_u64;
        let mut converted_usd_round = 0_u64;
        let mut refreshed_wallets = 0_u32;
        let mut failed_wallets = 0_u32;

        self.mode = StableSatsSimulationMode::RealBlink;
        self.live_refresh_pending = false;
        self.active_live_refresh_request_id = None;
        self.rounds_run = round;
        self.price_usd_cents_per_btc = price_usd_cents_per_btc.max(1);
        self.last_settlement_ref = Some(format!("blink:live:settlement:{round:04}"));

        for (owner_id, btc_balance_sats, usd_balance_cents, source_ref) in wallet_snapshots {
            let Some(wallet_index) = self
                .agents
                .iter()
                .position(|wallet| wallet.owner_id == *owner_id)
            else {
                self.push_event(
                    "BLINK-LEDGER",
                    &format!("blink:live:unmapped:{round:04}:{owner_id}"),
                    format!("received live snapshot for unmapped wallet owner_id={owner_id}"),
                );
                continue;
            };

            let wallet_name = self.agents[wallet_index].agent_name.clone();
            let prev_btc = self.agents[wallet_index].btc_balance_sats;
            let prev_usd = self.agents[wallet_index].usd_balance_cents;
            let btc_delta = prev_btc.abs_diff(*btc_balance_sats);
            let usd_delta = prev_usd.abs_diff(*usd_balance_cents);
            let switched = btc_delta > 0 || usd_delta > 0;

            {
                let wallet = &mut self.agents[wallet_index];
                wallet.btc_balance_sats = *btc_balance_sats;
                wallet.usd_balance_cents = *usd_balance_cents;
                wallet.active_wallet = if *usd_balance_cents > 0 && *btc_balance_sats == 0 {
                    StableSatsWalletMode::Usd
                } else if *btc_balance_sats > 0 && *usd_balance_cents == 0 {
                    StableSatsWalletMode::Btc
                } else {
                    wallet.active_wallet
                };
                if switched {
                    wallet.switch_count = wallet.switch_count.saturating_add(1);
                    wallet.last_switch_summary = format!(
                        "delta btc={} sats usd={}",
                        btc_delta,
                        Self::format_usd_cents(usd_delta)
                    );
                } else {
                    wallet.last_switch_summary = "no balance change".to_string();
                }
            }

            refreshed_wallets = refreshed_wallets.saturating_add(1);
            converted_sats_round = converted_sats_round.saturating_add(btc_delta);
            converted_usd_round = converted_usd_round.saturating_add(usd_delta);

            let wallet_ref = format!("blink:live:wallet:{round:04}:{wallet_index:02}");
            self.push_event(
                "BLINK-LEDGER",
                &wallet_ref,
                format!(
                    "{} balances btc={} sats usd={} ({})",
                    wallet_name,
                    btc_balance_sats,
                    Self::format_usd_cents(*usd_balance_cents),
                    source_ref
                ),
            );

            if switched {
                let (from_wallet, to_wallet, asset, amount) = if *btc_balance_sats < prev_btc {
                    (
                        format!("{wallet_name}:BTC"),
                        format!("{wallet_name}:USD"),
                        StableSatsTransferAsset::BtcSats,
                        btc_delta,
                    )
                } else {
                    (
                        format!("{wallet_name}:USD"),
                        format!("{wallet_name}:BTC"),
                        StableSatsTransferAsset::UsdCents,
                        usd_delta,
                    )
                };
                self.push_transfer_ledger_entry(
                    now_epoch_seconds,
                    format!("blink:live:transfer:{round:04}:{wallet_index:02}"),
                    from_wallet,
                    to_wallet,
                    asset,
                    amount,
                    0,
                    StableSatsTransferStatus::Settled,
                    format!(
                        "{} live delta btc={} sats usd={}",
                        wallet_name,
                        btc_delta,
                        Self::format_usd_cents(usd_delta)
                    ),
                );
            }
        }

        for (owner_id, error) in wallet_failures {
            failed_wallets = failed_wallets.saturating_add(1);
            if let Some(wallet_index) = self
                .agents
                .iter()
                .position(|wallet| wallet.owner_id == *owner_id)
            {
                let wallet_owner_id = self.agents[wallet_index].owner_id.clone();
                let wallet_name = self.agents[wallet_index].agent_name.clone();
                self.agents[wallet_index].last_switch_summary = format!("refresh failed: {error}");
                self.push_event(
                    "BLINK-LEDGER",
                    &format!("blink:live:error:{round:04}:{wallet_owner_id}"),
                    format!("{wallet_name} refresh failed: {error}"),
                );
            } else {
                self.push_event(
                    "BLINK-LEDGER",
                    &format!("blink:live:error:{round:04}:{owner_id}"),
                    format!("wallet refresh failed for unmapped owner_id={owner_id}: {error}"),
                );
            }
        }

        self.total_converted_sats = self
            .total_converted_sats
            .saturating_add(converted_sats_round);
        self.total_converted_usd_cents = self
            .total_converted_usd_cents
            .saturating_add(converted_usd_round);

        self.price_history_usd_cents_per_btc
            .push(self.price_usd_cents_per_btc);
        if self.price_history_usd_cents_per_btc.len() > 18 {
            let overflow = self
                .price_history_usd_cents_per_btc
                .len()
                .saturating_sub(18);
            self.price_history_usd_cents_per_btc.drain(0..overflow);
        }
        self.converted_sats_history.push(converted_sats_round);
        if self.converted_sats_history.len() > 18 {
            let overflow = self.converted_sats_history.len().saturating_sub(18);
            self.converted_sats_history.drain(0..overflow);
        }

        let price_ref = format!("blink:live:price:{round:04}:{now_epoch_seconds}");
        let source_summary = wallet_snapshots
            .iter()
            .map(|(owner_id, _btc, _usd, _source_ref)| owner_id.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        self.push_event(
            "BLINK-PRICE",
            &price_ref,
            format!(
                "live BTC/USD {} for [{}]",
                Self::format_usd_cents(self.price_usd_cents_per_btc),
                source_summary
            ),
        );
        let ledger_ref = self
            .last_settlement_ref
            .clone()
            .unwrap_or_else(|| "blink:live:settlement".to_string());
        self.push_event(
            "BLINK-LEDGER",
            &ledger_ref,
            format!(
                "live refresh settled wallets={} failed={} aggregate_btc={} sats aggregate_usd={}",
                refreshed_wallets,
                failed_wallets,
                self.total_btc_balance_sats(),
                Self::format_usd_cents(self.total_usd_balance_cents()),
            ),
        );

        self.load_state = if refreshed_wallets == 0 {
            PaneLoadState::Error
        } else {
            PaneLoadState::Ready
        };
        self.last_error = if failed_wallets == 0 {
            None
        } else {
            Some(format!(
                "Live refresh completed with {} wallet error(s)",
                failed_wallets
            ))
        };
        self.last_action = Some(format!(
            "Round {round}: live refresh updated {} wallet(s) with {} failure(s)",
            refreshed_wallets, failed_wallets
        ));
    }

    pub fn apply_live_snapshot(
        &mut self,
        now_epoch_seconds: u64,
        btc_balance_sats: u64,
        usd_balance_cents: u64,
        price_usd_cents_per_btc: u64,
        source_ref: &str,
    ) {
        let operator_owner_id = self
            .agents
            .iter()
            .find(|wallet| wallet.owner_kind == StableSatsWalletOwnerKind::Operator)
            .map(|wallet| wallet.owner_id.clone())
            .unwrap_or_else(|| "operator:autopilot".to_string());
        self.apply_live_wallet_snapshots(
            now_epoch_seconds,
            price_usd_cents_per_btc,
            &[(
                operator_owner_id,
                btc_balance_sats,
                usd_balance_cents,
                source_ref.to_string(),
            )],
            &[],
        );
    }

    pub fn reset(&mut self) {
        self.load_state = PaneLoadState::Ready;
        self.last_error = None;
        self.last_action = Some(format!("StableSats {} mode reset", self.mode.label()));
        self.rounds_run = 0;
        self.price_usd_cents_per_btc = Self::BASE_PRICE_USD_CENTS_PER_BTC;
        self.total_converted_sats = 0;
        self.total_converted_usd_cents = 0;
        self.last_settlement_ref = None;
        self.agents = match self.mode {
            StableSatsSimulationMode::Demo => Self::default_agents(),
            StableSatsSimulationMode::RealBlink => Self::default_real_wallets(),
        };
        self.price_history_usd_cents_per_btc.clear();
        self.converted_sats_history.clear();
        self.auto_run_enabled = false;
        self.live_refresh_pending = false;
        self.active_live_refresh_request_id = None;
        self.transfer_ledger.clear();
        self.events.clear();
        self.next_seq = 1;
        self.next_transfer_seq = 1;
        self.auto_run_last_tick = None;
        self.next_live_refresh_request_id = 1;
    }

    pub fn start_auto_run(&mut self, now: Instant) {
        self.auto_run_enabled = true;
        self.auto_run_last_tick = Some(now);
        self.last_error = None;
        self.last_action = Some("Auto StableSats simulation running".to_string());
    }

    pub fn stop_auto_run(&mut self) {
        self.auto_run_enabled = false;
        self.auto_run_last_tick = None;
        self.last_action = Some("Auto StableSats simulation paused".to_string());
    }

    pub fn should_run_auto_round(&self, now: Instant) -> bool {
        if self.mode == StableSatsSimulationMode::RealBlink {
            return false;
        }
        if !self.auto_run_enabled {
            return false;
        }
        self.auto_run_last_tick
            .is_none_or(|last| now.duration_since(last) >= self.auto_run_interval)
    }

    pub fn mark_auto_round(&mut self, now: Instant) {
        self.auto_run_last_tick = Some(now);
    }

    fn sats_to_usd_cents(sats: u64, price_usd_cents_per_btc: u64) -> u64 {
        let numerator = u128::from(sats).saturating_mul(u128::from(price_usd_cents_per_btc));
        ((numerator + (Self::SATS_PER_BTC / 2)) / Self::SATS_PER_BTC) as u64
    }

    fn usd_cents_to_sats(usd_cents: u64, price_usd_cents_per_btc: u64) -> u64 {
        if price_usd_cents_per_btc == 0 {
            return 0;
        }
        let numerator = u128::from(usd_cents).saturating_mul(Self::SATS_PER_BTC);
        ((numerator + (u128::from(price_usd_cents_per_btc) / 2))
            / u128::from(price_usd_cents_per_btc)) as u64
    }

    fn format_usd_cents(usd_cents: u64) -> String {
        format!("${}.{:02}", usd_cents / 100, usd_cents % 100)
    }

    fn push_event(&mut self, protocol: &str, event_ref: &str, summary: String) {
        self.events.push(AgentNetworkSimulationEvent {
            seq: self.next_seq,
            protocol: protocol.to_string(),
            event_ref: event_ref.to_string(),
            summary,
        });
        self.next_seq = self.next_seq.saturating_add(1);
        if self.events.len() > 24 {
            let overflow = self.events.len().saturating_sub(24);
            self.events.drain(0..overflow);
        }
    }

    fn default_real_wallets() -> Vec<StableSatsAgentWalletState> {
        vec![
            StableSatsAgentWalletState {
                agent_name: "autopilot-user".to_string(),
                owner_kind: StableSatsWalletOwnerKind::Operator,
                owner_id: "operator:autopilot".to_string(),
                credential_key_name: "BLINK_API_KEY".to_string(),
                credential_url_name: Some("BLINK_API_URL".to_string()),
                btc_balance_sats: 0,
                usd_balance_cents: 0,
                active_wallet: StableSatsWalletMode::Btc,
                switch_count: 0,
                last_switch_summary: "awaiting live refresh".to_string(),
            },
            StableSatsAgentWalletState {
                agent_name: "sa-wallet-1".to_string(),
                owner_kind: StableSatsWalletOwnerKind::SovereignAgent,
                owner_id: "sa:wallet-1".to_string(),
                credential_key_name: "BLINK_API_KEY_SA_1".to_string(),
                credential_url_name: Some("BLINK_API_URL_SA_1".to_string()),
                btc_balance_sats: 0,
                usd_balance_cents: 0,
                active_wallet: StableSatsWalletMode::Btc,
                switch_count: 0,
                last_switch_summary: "awaiting sovereign wallet sync".to_string(),
            },
            StableSatsAgentWalletState {
                agent_name: "sa-wallet-2".to_string(),
                owner_kind: StableSatsWalletOwnerKind::SovereignAgent,
                owner_id: "sa:wallet-2".to_string(),
                credential_key_name: "BLINK_API_KEY_SA_2".to_string(),
                credential_url_name: Some("BLINK_API_URL_SA_2".to_string()),
                btc_balance_sats: 0,
                usd_balance_cents: 0,
                active_wallet: StableSatsWalletMode::Usd,
                switch_count: 0,
                last_switch_summary: "awaiting sovereign wallet sync".to_string(),
            },
        ]
    }

    #[allow(clippy::too_many_arguments)]
    fn push_transfer_ledger_entry(
        &mut self,
        now_epoch_seconds: u64,
        transfer_ref: String,
        from_wallet: String,
        to_wallet: String,
        asset: StableSatsTransferAsset,
        amount: u64,
        effective_fee: u64,
        status: StableSatsTransferStatus,
        summary: String,
    ) {
        self.transfer_ledger.push(StableSatsTransferLedgerEntry {
            seq: self.next_transfer_seq,
            transfer_ref: transfer_ref.clone(),
            from_wallet,
            to_wallet,
            asset,
            amount,
            effective_fee,
            status,
            summary: summary.clone(),
            occurred_at_epoch_seconds: now_epoch_seconds,
        });
        self.next_transfer_seq = self.next_transfer_seq.saturating_add(1);
        if self.transfer_ledger.len() > 48 {
            let overflow = self.transfer_ledger.len().saturating_sub(48);
            self.transfer_ledger.drain(0..overflow);
        }
        self.push_event("BLINK-XFER", transfer_ref.as_str(), summary);
    }
}
