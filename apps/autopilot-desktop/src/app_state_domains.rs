use super::*;
use wgpui::RiveFitMode;

const FRAME_DEBUGGER_SAMPLE_CAPACITY: usize = 180;

pub struct LocalInferencePaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub pending_request_id: Option<String>,
    pub last_request_id: Option<String>,
    pub last_model: Option<String>,
    pub output_preview: String,
    pub output_chars: usize,
    pub last_metrics: Option<LocalInferenceExecutionMetrics>,
    pub last_provenance: Option<LocalInferenceExecutionProvenance>,
}

impl Default for LocalInferencePaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for local inference runtime snapshot".to_string()),
            pending_request_id: None,
            last_request_id: None,
            last_model: None,
            output_preview: String::new(),
            output_chars: 0,
            last_metrics: None,
            last_provenance: None,
        }
    }
}

pub struct RivePreviewPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub asset_id: String,
    pub asset_name: String,
    pub artboard_name: Option<String>,
    pub state_machine_name: Option<String>,
    pub autoplay: bool,
    pub playing: bool,
    pub fit_mode: RiveFitMode,
    pub frame_build_ms: Option<f32>,
    pub draw_call_count: u32,
    pub image_count: u32,
    pub scene_name: Option<String>,
    pub last_pointer: Option<Point>,
}

impl Default for RivePreviewPaneState {
    fn default() -> Self {
        let asset = crate::rive_assets::default_packaged_rive_asset();
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting to load packaged Rive asset".to_string()),
            asset_id: asset.id.to_string(),
            asset_name: asset.file_name.to_string(),
            artboard_name: Some(asset.default_artboard.to_string()),
            state_machine_name: Some(asset.default_scene.to_string()),
            autoplay: true,
            playing: true,
            fit_mode: RiveFitMode::Contain,
            frame_build_ms: None,
            draw_call_count: 0,
            image_count: 0,
            scene_name: None,
            last_pointer: None,
        }
    }
}

pub struct PresentationPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub asset_id: String,
    pub asset_name: String,
}

impl Default for PresentationPaneState {
    fn default() -> Self {
        let asset = crate::rive_assets::simple_fui_hud_asset();
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting to load presentation HUD asset".to_string()),
            asset_id: asset.id.to_string(),
            asset_name: asset.file_name.to_string(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct RiveCadenceSnapshot {
    pub pane_open: bool,
    pub surface_loaded: bool,
    pub needs_redraw: bool,
    pub animating: bool,
    pub settled: bool,
}

impl RiveCadenceSnapshot {
    pub fn cadence_label(&self) -> &'static str {
        if !self.pane_open {
            "closed"
        } else if !self.surface_loaded {
            "unloaded"
        } else if self.animating {
            "animating"
        } else if self.needs_redraw {
            "dirty"
        } else if self.settled {
            "settled"
        } else {
            "idle"
        }
    }

    pub fn state_summary(&self) -> String {
        format!(
            "{} // loaded={} redraw={} settled={}",
            self.cadence_label(),
            self.surface_loaded,
            self.needs_redraw,
            self.settled
        )
    }
}

#[derive(Clone, Debug, Default)]
pub struct FrameRedrawPressureSnapshot {
    pub should_redraw: bool,
    pub background_changed: bool,
    pub hotbar_flashing: bool,
    pub provider_animating: bool,
    pub chat_pending: bool,
    pub debug_probe_active: bool,
    pub text_input_focused: bool,
    pub poll_interval_ms: u32,
    pub rive_preview: RiveCadenceSnapshot,
    pub presentation: RiveCadenceSnapshot,
}

impl FrameRedrawPressureSnapshot {
    pub fn active_reason_labels(&self) -> Vec<&'static str> {
        let mut labels = Vec::new();
        if self.background_changed {
            labels.push("background");
        }
        if self.hotbar_flashing {
            labels.push("hotbar");
        }
        if self.provider_animating {
            labels.push("provider");
        }
        if self.chat_pending {
            labels.push("chat");
        }
        if self.debug_probe_active {
            labels.push("debugger");
        }
        if self.text_input_focused {
            labels.push("text_input");
        }
        if self.rive_preview.needs_redraw {
            labels.push("rive_preview");
        }
        if self.presentation.needs_redraw {
            labels.push("presentation");
        }
        labels
    }

    pub fn reason_summary(&self) -> String {
        let labels = self.active_reason_labels();
        if labels.is_empty() {
            "idle".to_string()
        } else {
            labels.join(" + ")
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct FrameRedrawReasonCounters {
    pub background_changed: u64,
    pub hotbar_flashing: u64,
    pub provider_animating: u64,
    pub chat_pending: u64,
    pub debug_probe_active: u64,
    pub text_input_focused: u64,
    pub rive_preview: u64,
    pub presentation: u64,
}

#[derive(Clone, Debug, Default)]
pub struct FrameRenderReport {
    pub scene_build_ms: f32,
    pub surface_acquire_ms: f32,
    pub prepare_cpu_ms: f32,
    pub render_cpu_ms: f32,
    pub submit_present_ms: f32,
    pub total_cpu_ms: f32,
    pub draw_calls: u32,
    pub layer_count: usize,
    pub vector_batches: u32,
    pub image_instances: u32,
    pub svg_instances: u32,
    pub svg_cache_size: usize,
}

#[derive(Clone, Debug, Default)]
pub struct FrameSample {
    pub frame_interval_ms: f32,
    pub scene_build_ms: f32,
    pub surface_acquire_ms: f32,
    pub prepare_cpu_ms: f32,
    pub render_cpu_ms: f32,
    pub submit_present_ms: f32,
    pub total_cpu_ms: f32,
    pub draw_calls: u32,
    pub layer_count: usize,
    pub vector_batches: u32,
    pub image_instances: u32,
    pub svg_instances: u32,
    pub svg_cache_size: usize,
}

pub struct FrameDebuggerPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub total_frames: u64,
    pub redraw_requests: u64,
    pub slow_frames_60hz: u64,
    pub slow_frames_30hz: u64,
    pub last_frame_interval_ms: Option<f32>,
    pub rolling_frame_interval_ms: Option<f32>,
    pub rolling_fps: Option<f32>,
    pub last_report: Option<FrameSample>,
    pub redraw_pressure: FrameRedrawPressureSnapshot,
    pub redraw_reason_counters: FrameRedrawReasonCounters,
    last_frame_completed_at: Option<Instant>,
    samples: std::collections::VecDeque<FrameSample>,
}

impl Default for FrameDebuggerPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for first desktop frame sample".to_string()),
            total_frames: 0,
            redraw_requests: 0,
            slow_frames_60hz: 0,
            slow_frames_30hz: 0,
            last_frame_interval_ms: None,
            rolling_frame_interval_ms: None,
            rolling_fps: None,
            last_report: None,
            redraw_pressure: FrameRedrawPressureSnapshot::default(),
            redraw_reason_counters: FrameRedrawReasonCounters::default(),
            last_frame_completed_at: None,
            samples: std::collections::VecDeque::with_capacity(FRAME_DEBUGGER_SAMPLE_CAPACITY),
        }
    }
}

impl FrameDebuggerPaneState {
    pub fn samples(&self) -> &std::collections::VecDeque<FrameSample> {
        &self.samples
    }

    pub fn note_redraw_pressure(&mut self, snapshot: FrameRedrawPressureSnapshot) {
        if snapshot.should_redraw {
            self.redraw_requests = self.redraw_requests.saturating_add(1);
            if snapshot.background_changed {
                self.redraw_reason_counters.background_changed = self
                    .redraw_reason_counters
                    .background_changed
                    .saturating_add(1);
            }
            if snapshot.hotbar_flashing {
                self.redraw_reason_counters.hotbar_flashing = self
                    .redraw_reason_counters
                    .hotbar_flashing
                    .saturating_add(1);
            }
            if snapshot.provider_animating {
                self.redraw_reason_counters.provider_animating = self
                    .redraw_reason_counters
                    .provider_animating
                    .saturating_add(1);
            }
            if snapshot.chat_pending {
                self.redraw_reason_counters.chat_pending =
                    self.redraw_reason_counters.chat_pending.saturating_add(1);
            }
            if snapshot.debug_probe_active {
                self.redraw_reason_counters.debug_probe_active = self
                    .redraw_reason_counters
                    .debug_probe_active
                    .saturating_add(1);
            }
            if snapshot.text_input_focused {
                self.redraw_reason_counters.text_input_focused = self
                    .redraw_reason_counters
                    .text_input_focused
                    .saturating_add(1);
            }
            if snapshot.rive_preview.needs_redraw {
                self.redraw_reason_counters.rive_preview =
                    self.redraw_reason_counters.rive_preview.saturating_add(1);
            }
            if snapshot.presentation.needs_redraw {
                self.redraw_reason_counters.presentation =
                    self.redraw_reason_counters.presentation.saturating_add(1);
            }
        }
        self.last_action = Some(format!(
            "Loop pressure {} @ {}ms poll",
            snapshot.reason_summary(),
            snapshot.poll_interval_ms
        ));
        self.redraw_pressure = snapshot;
    }

    pub fn record_frame(&mut self, report: FrameRenderReport) {
        let now = Instant::now();
        let frame_interval_ms = self
            .last_frame_completed_at
            .map(|last| now.saturating_duration_since(last).as_secs_f32() * 1_000.0)
            .unwrap_or(report.total_cpu_ms.max(0.0));
        self.last_frame_completed_at = Some(now);

        let sample = FrameSample {
            frame_interval_ms,
            scene_build_ms: report.scene_build_ms,
            surface_acquire_ms: report.surface_acquire_ms,
            prepare_cpu_ms: report.prepare_cpu_ms,
            render_cpu_ms: report.render_cpu_ms,
            submit_present_ms: report.submit_present_ms,
            total_cpu_ms: report.total_cpu_ms,
            draw_calls: report.draw_calls,
            layer_count: report.layer_count,
            vector_batches: report.vector_batches,
            image_instances: report.image_instances,
            svg_instances: report.svg_instances,
            svg_cache_size: report.svg_cache_size,
        };

        if self.samples.len() == FRAME_DEBUGGER_SAMPLE_CAPACITY {
            self.samples.pop_front();
        }
        self.samples.push_back(sample.clone());
        self.total_frames = self.total_frames.saturating_add(1);
        if sample.frame_interval_ms > 16.67 {
            self.slow_frames_60hz = self.slow_frames_60hz.saturating_add(1);
        }
        if sample.frame_interval_ms > 33.34 {
            self.slow_frames_30hz = self.slow_frames_30hz.saturating_add(1);
        }

        let rolling_interval = self
            .samples
            .iter()
            .map(|entry| entry.frame_interval_ms)
            .sum::<f32>()
            / self.samples.len().max(1) as f32;
        self.last_frame_interval_ms = Some(sample.frame_interval_ms);
        self.rolling_frame_interval_ms = Some(rolling_interval);
        self.rolling_fps = Some(if rolling_interval > f32::EPSILON {
            1_000.0 / rolling_interval
        } else {
            0.0
        });
        self.last_report = Some(sample.clone());
        self.load_state = PaneLoadState::Ready;
        self.last_error = None;
        self.last_action = Some(format!(
            "Frame {} recorded // {:.1} redraw fps rolling // {:.2} ms cpu",
            self.total_frames,
            self.rolling_fps.unwrap_or_default(),
            sample.total_cpu_ms
        ));
    }

    pub fn record_error(&mut self, error: impl Into<String>) {
        self.load_state = PaneLoadState::Error;
        self.last_error = Some(error.into());
    }
}

#[cfg(test)]
mod frame_debugger_tests {
    use super::{FrameDebuggerPaneState, FrameRedrawPressureSnapshot, FrameRenderReport};

    #[test]
    fn frame_debugger_records_samples_and_rolling_fps() {
        let mut state = FrameDebuggerPaneState::default();
        state.record_frame(FrameRenderReport {
            total_cpu_ms: 8.0,
            scene_build_ms: 3.0,
            render_cpu_ms: 2.0,
            ..FrameRenderReport::default()
        });
        state.record_frame(FrameRenderReport {
            total_cpu_ms: 10.0,
            scene_build_ms: 4.0,
            render_cpu_ms: 3.0,
            ..FrameRenderReport::default()
        });

        assert_eq!(state.total_frames, 2);
        assert_eq!(state.samples().len(), 2);
        assert!(state.rolling_fps.is_some());
        assert!(state.last_report.is_some());
    }

    #[test]
    fn redraw_pressure_reason_summary_lists_active_drivers() {
        let snapshot = FrameRedrawPressureSnapshot {
            should_redraw: true,
            background_changed: true,
            presentation: super::RiveCadenceSnapshot {
                pane_open: true,
                surface_loaded: true,
                needs_redraw: true,
                animating: true,
                settled: false,
            },
            ..FrameRedrawPressureSnapshot::default()
        };

        assert_eq!(snapshot.reason_summary(), "background + presentation");
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppleFmWorkbenchSamplingMode {
    Auto,
    Greedy,
    Random,
}

impl AppleFmWorkbenchSamplingMode {
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Auto => "AUTO",
            Self::Greedy => "GREEDY",
            Self::Random => "RANDOM",
        }
    }

    pub const fn cycle(self) -> Self {
        match self {
            Self::Auto => Self::Greedy,
            Self::Greedy => Self::Random,
            Self::Random => Self::Auto,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppleFmWorkbenchToolProfile {
    None,
    Demo,
    Failing,
}

impl AppleFmWorkbenchToolProfile {
    pub const fn label(&self) -> &'static str {
        match self {
            Self::None => "TOOLS: NONE",
            Self::Demo => "TOOLS: DEMO",
            Self::Failing => "TOOLS: FAILING",
        }
    }

    pub const fn cycle(self) -> Self {
        match self {
            Self::None => Self::Demo,
            Self::Demo => Self::Failing,
            Self::Failing => Self::None,
        }
    }
}

pub struct AppleFmWorkbenchPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub pending_request_id: Option<String>,
    pub last_request_id: Option<String>,
    pub last_operation: Option<String>,
    pub active_session_id: Option<String>,
    pub last_model: Option<String>,
    pub sampling_mode: AppleFmWorkbenchSamplingMode,
    pub tool_profile: AppleFmWorkbenchToolProfile,
    pub output_preview: String,
    pub output_chars: usize,
    pub session_preview: String,
    pub structured_preview: String,
    pub usage_preview: String,
    pub event_log: TerminalPane,
}

impl Default for AppleFmWorkbenchPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for Apple FM bridge snapshot".to_string()),
            pending_request_id: None,
            last_request_id: None,
            last_operation: None,
            active_session_id: None,
            last_model: None,
            sampling_mode: AppleFmWorkbenchSamplingMode::Auto,
            tool_profile: AppleFmWorkbenchToolProfile::None,
            output_preview: String::new(),
            output_chars: 0,
            session_preview: String::new(),
            structured_preview: String::new(),
            usage_preview: String::new(),
            event_log: TerminalPane::new().title("\\\\ EVENTS"),
        }
    }
}

pub struct CodexAccountPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub install_available: bool,
    pub install_command: Option<String>,
    pub install_version: Option<String>,
    pub readiness_summary: String,
    pub config_summary: String,
    pub config_requirements_summary: String,
    pub config_constraint_summary: Option<String>,
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
            install_available: false,
            install_command: None,
            install_version: None,
            readiness_summary: "Waiting for Codex lane readiness".to_string(),
            config_summary: "Waiting for config/read".to_string(),
            config_requirements_summary: "Waiting for configRequirements/read".to_string(),
            config_constraint_summary: None,
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
    pub summary: String,
    pub requirements_summary: String,
    pub constraint_summary: Option<String>,
    pub config_json: String,
    pub origins_json: String,
    pub layers_json: String,
    pub requirements_json: String,
    pub detected_external_configs: usize,
}

impl Default for CodexConfigPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for config/read".to_string()),
            summary: "Waiting for config/read".to_string(),
            requirements_summary: "Waiting for configRequirements/read".to_string(),
            constraint_summary: None,
            config_json: "{}".to_string(),
            origins_json: "{}".to_string(),
            layers_json: "[]".to_string(),
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

pub struct CodexRemoteState {
    pub enabled: bool,
    pub requested_bind_addr: String,
    pub listen_addr: Option<String>,
    pub base_url: Option<String>,
    pub pairing_url: Option<String>,
    pub auth_token_preview: Option<String>,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
}

impl Default for CodexRemoteState {
    fn default() -> Self {
        Self {
            enabled: false,
            requested_bind_addr: "127.0.0.1:4848".to_string(),
            listen_addr: None,
            base_url: None,
            pairing_url: None,
            auth_token_preview: None,
            last_error: None,
            last_action: Some("Remote companion disabled".to_string()),
        }
    }
}

pub struct DesktopControlState {
    pub enabled: bool,
    pub requested_bind_addr: String,
    pub listen_addr: Option<String>,
    pub base_url: Option<String>,
    pub manifest_path: Option<String>,
    pub auth_token_preview: Option<String>,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub last_command_summary: Option<String>,
    pub last_command_error: Option<String>,
    pub last_command_completed_at_epoch_ms: Option<u64>,
    pub last_snapshot_revision: u64,
    pub last_snapshot_signature: Option<String>,
    pub compute_history: DesktopControlComputeHistoryState,
}

#[derive(Clone, Debug)]
pub struct DesktopControlComputeHistoryState {
    pub provider_id: Option<String>,
    pub delivery_proofs: Vec<openagents_kernel_core::compute::DeliveryProof>,
    pub capacity_instruments: Vec<openagents_kernel_core::compute::CapacityInstrument>,
    pub structured_capacity_instruments:
        Vec<openagents_kernel_core::compute::StructuredCapacityInstrument>,
    pub validator_challenges:
        Vec<openagents_kernel_core::compute::ComputeValidatorChallengeSnapshot>,
    pub last_refreshed_at_epoch_ms: Option<u64>,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
}

impl Default for DesktopControlComputeHistoryState {
    fn default() -> Self {
        Self {
            provider_id: None,
            delivery_proofs: Vec::new(),
            capacity_instruments: Vec::new(),
            structured_capacity_instruments: Vec::new(),
            validator_challenges: Vec::new(),
            last_refreshed_at_epoch_ms: None,
            last_error: None,
            last_action: Some("Kernel proof and settlement history idle".to_string()),
        }
    }
}

impl Default for DesktopControlState {
    fn default() -> Self {
        Self {
            enabled: false,
            requested_bind_addr: "127.0.0.1:0".to_string(),
            listen_addr: None,
            base_url: None,
            manifest_path: None,
            auth_token_preview: None,
            last_error: None,
            last_action: Some("Desktop control runtime disabled".to_string()),
            last_command_summary: None,
            last_command_error: None,
            last_command_completed_at_epoch_ms: None,
            last_snapshot_revision: 0,
            last_snapshot_signature: None,
            compute_history: DesktopControlComputeHistoryState::default(),
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
    pub remote_scope: codex_client::HazelnutScope,
    pub remote_skills: Vec<SkillRegistryRemoteSkill>,
    pub last_remote_export_id: Option<String>,
    pub last_remote_export_path: Option<String>,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkillRegistryRemoteSkill {
    pub id: String,
    pub name: String,
    pub description: String,
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
            remote_scope: codex_client::HazelnutScope::Example,
            remote_skills: Vec::new(),
            last_remote_export_id: None,
            last_remote_export_path: None,
        }
    }
}

pub struct CastControlPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub source: String,
    pub prereq_status: String,
    pub last_operation: Option<String>,
    pub last_receipt_path: Option<String>,
    pub last_txid: Option<String>,
    pub last_log_path: Option<String>,
    pub broadcast_armed: bool,
    pub auto_loop_enabled: bool,
    pub auto_loop_interval: Duration,
    pub active_pid: Option<String>,
    pub loop_config_path: String,
    auto_loop_last_tick: Option<Instant>,
    pub run_count: u64,
}

impl Default for CastControlPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for CAST controls".to_string()),
            source: "runtime+local".to_string(),
            prereq_status: "unknown".to_string(),
            last_operation: None,
            last_receipt_path: None,
            last_txid: None,
            last_log_path: None,
            broadcast_armed: false,
            auto_loop_enabled: false,
            auto_loop_interval: Duration::from_secs(45),
            active_pid: None,
            loop_config_path: "skills/cast/assets/autotrade-loop.config.example".to_string(),
            auto_loop_last_tick: None,
            run_count: 0,
        }
    }
}

impl CastControlPaneState {
    pub fn start_auto_loop(&mut self) {
        self.auto_loop_enabled = true;
        self.auto_loop_last_tick = None;
        self.last_error = None;
    }

    pub fn stop_auto_loop(&mut self) {
        self.auto_loop_enabled = false;
        self.auto_loop_last_tick = None;
    }

    pub fn should_run_auto_loop(&self, now: Instant) -> bool {
        if !self.auto_loop_enabled {
            return false;
        }
        self.auto_loop_last_tick
            .is_none_or(|last| now.duration_since(last) >= self.auto_loop_interval)
    }

    pub fn mark_auto_loop_tick(&mut self, now: Instant) {
        self.auto_loop_last_tick = Some(now);
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
    pub variant_materials: std::collections::BTreeMap<String, String>,
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
    pub sensor_visualization_mode: CadSensorVisualizationMode,
    pub snap_toggles: CadSnapToggles,
    pub projection_mode: CadProjectionMode,
    pub viewport_layout: CadViewportLayout,
    pub gripper_jaw_open: bool,
    pub grasp_simulation_seed: u64,
    pub grasp_simulation_samples: Vec<CadGraspSimulationSample>,
    pub grasp_simulation_last_updated_revision: u64,
    pub sensor_feedback_readings: Vec<CadSensorFeedbackReading>,
    pub sensor_feedback_trace: Vec<CadSensorFeedbackTracePoint>,
    pub sensor_feedback_last_updated_revision: u64,
    pub drawing_view_mode: CadDrawingViewMode,
    pub drawing_view_direction: CadDrawingViewDirection,
    pub drawing_show_hidden_lines: bool,
    pub drawing_show_dimensions: bool,
    pub drawing_zoom: f32,
    pub drawing_pan_x: f32,
    pub drawing_pan_y: f32,
    pub drawing_detail_views: Vec<CadDrawingDetailViewState>,
    pub drawing_next_detail_id: u64,
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
    pub assembly_schema: openagents_cad::assembly::CadAssemblySchema,
    pub assembly_ui_state: openagents_cad::assembly::CadAssemblyUiState,
    pub dimensions: Vec<CadDimensionState>,
    pub dimension_edit: Option<CadDimensionEditState>,
    pub context_menu: CadContextMenuState,
    pub cad_events: Vec<openagents_cad::events::CadEvent>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CadGraspObjectShape {
    Sphere,
    Cube,
    Capsule,
}

impl CadGraspObjectShape {
    pub fn label(self) -> &'static str {
        match self {
            Self::Sphere => "sphere",
            Self::Cube => "cube",
            Self::Capsule => "capsule",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CadGraspSimulationSample {
    pub shape: CadGraspObjectShape,
    pub closure_mm: f64,
    pub contact_points: u8,
    pub compliance_deflection_mm: f64,
    pub adaptation_score: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CadSensorVisualizationMode {
    Off,
    Pressure,
    Proximity,
    Combined,
}

impl CadSensorVisualizationMode {
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::Pressure,
            Self::Pressure => Self::Proximity,
            Self::Proximity => Self::Combined,
            Self::Combined => Self::Off,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Pressure => "pressure",
            Self::Proximity => "proximity",
            Self::Combined => "combined",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CadSensorFeedbackReading {
    pub digit_id: String,
    pub pressure_ratio: f64,
    pub proximity_mm: f64,
    pub contact: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CadSensorFeedbackTracePoint {
    pub document_revision: u64,
    pub pose_preset: String,
    pub average_pressure_ratio: f64,
    pub minimum_proximity_mm: f64,
    pub contact_count: usize,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CadViewportLayout {
    Single,
    Quad,
}

impl CadViewportLayout {
    pub fn next(self) -> Self {
        match self {
            Self::Single => Self::Quad,
            Self::Quad => Self::Single,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::Quad => "quad",
        }
    }
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
pub enum CadDrawingViewMode {
    ThreeD,
    TwoD,
}

impl CadDrawingViewMode {
    pub fn next(self) -> Self {
        match self {
            Self::ThreeD => Self::TwoD,
            Self::TwoD => Self::ThreeD,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::ThreeD => "3d",
            Self::TwoD => "2d",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CadDrawingViewDirection {
    Front,
    Back,
    Top,
    Bottom,
    Left,
    Right,
    Isometric,
}

impl CadDrawingViewDirection {
    pub fn next(self) -> Self {
        match self {
            Self::Front => Self::Back,
            Self::Back => Self::Top,
            Self::Top => Self::Bottom,
            Self::Bottom => Self::Left,
            Self::Left => Self::Right,
            Self::Right => Self::Isometric,
            Self::Isometric => Self::Front,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Front => "front",
            Self::Back => "back",
            Self::Top => "top",
            Self::Bottom => "bottom",
            Self::Left => "left",
            Self::Right => "right",
            Self::Isometric => "isometric",
        }
    }

    pub fn to_drafting_view_direction(self) -> openagents_cad::drafting::ViewDirection {
        match self {
            Self::Front => openagents_cad::drafting::ViewDirection::Front,
            Self::Back => openagents_cad::drafting::ViewDirection::Back,
            Self::Top => openagents_cad::drafting::ViewDirection::Top,
            Self::Bottom => openagents_cad::drafting::ViewDirection::Bottom,
            Self::Left => openagents_cad::drafting::ViewDirection::Left,
            Self::Right => openagents_cad::drafting::ViewDirection::Right,
            Self::Isometric => openagents_cad::drafting::ViewDirection::ISOMETRIC_STANDARD,
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

#[derive(Clone, Debug, PartialEq)]
pub struct CadDrawingDetailViewState {
    pub detail_id: String,
    pub label: String,
    pub center_x: f32,
    pub center_y: f32,
    pub width: f32,
    pub height: f32,
    pub scale: f32,
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
        let variant_materials = variant_ids
            .iter()
            .map(|variant_id| {
                (
                    variant_id.clone(),
                    openagents_cad::materials::DEFAULT_CAD_MATERIAL_ID.to_string(),
                )
            })
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
            CadDimensionState {
                dimension_id: "jaw_open_mm".to_string(),
                label: "Jaw Open".to_string(),
                value_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_DEFAULT_JAW_OPEN_MM,
                min_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MIN_JAW_OPEN_MM,
                max_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MAX_JAW_OPEN_MM,
            },
            CadDimensionState {
                dimension_id: "finger_length_mm".to_string(),
                label: "Finger Length".to_string(),
                value_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_DEFAULT_FINGER_LENGTH_MM,
                min_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MIN_FINGER_LENGTH_MM,
                max_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MAX_FINGER_LENGTH_MM,
            },
            CadDimensionState {
                dimension_id: "finger_thickness_mm".to_string(),
                label: "Finger Thick".to_string(),
                value_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_DEFAULT_FINGER_THICKNESS_MM,
                min_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MIN_FINGER_THICKNESS_MM,
                max_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MAX_FINGER_THICKNESS_MM,
            },
            CadDimensionState {
                dimension_id: "base_width_mm".to_string(),
                label: "Base Width".to_string(),
                value_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_DEFAULT_BASE_WIDTH_MM,
                min_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MIN_BASE_WIDTH_MM,
                max_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MAX_BASE_WIDTH_MM,
            },
            CadDimensionState {
                dimension_id: "base_depth_mm".to_string(),
                label: "Base Depth".to_string(),
                value_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_DEFAULT_BASE_DEPTH_MM,
                min_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MIN_BASE_DEPTH_MM,
                max_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MAX_BASE_DEPTH_MM,
            },
            CadDimensionState {
                dimension_id: "base_thickness_mm".to_string(),
                label: "Base Thick".to_string(),
                value_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_DEFAULT_BASE_THICKNESS_MM,
                min_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MIN_BASE_THICKNESS_MM,
                max_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MAX_BASE_THICKNESS_MM,
            },
            CadDimensionState {
                dimension_id: "servo_mount_hole_diameter_mm".to_string(),
                label: "Servo Hole".to_string(),
                value_mm:
                    openagents_cad::intent::PARALLEL_JAW_GRIPPER_DEFAULT_SERVO_MOUNT_HOLE_DIAMETER_MM,
                min_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MIN_SERVO_MOUNT_HOLE_DIAMETER_MM,
                max_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MAX_SERVO_MOUNT_HOLE_DIAMETER_MM,
            },
            CadDimensionState {
                dimension_id: "print_fit_mm".to_string(),
                label: "Print Fit".to_string(),
                value_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_DEFAULT_PRINT_FIT_MM,
                min_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MIN_PRINT_FIT_MM,
                max_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MAX_PRINT_FIT_MM,
            },
            CadDimensionState {
                dimension_id: "print_clearance_mm".to_string(),
                label: "Print Clear".to_string(),
                value_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_DEFAULT_PRINT_CLEARANCE_MM,
                min_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MIN_PRINT_CLEARANCE_MM,
                max_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MAX_PRINT_CLEARANCE_MM,
            },
            CadDimensionState {
                dimension_id: "compliant_joint_count".to_string(),
                label: "Compliant Joints".to_string(),
                value_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_DEFAULT_COMPLIANT_JOINT_COUNT
                    as f64,
                min_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MIN_COMPLIANT_JOINT_COUNT
                    as f64,
                max_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MAX_COMPLIANT_JOINT_COUNT
                    as f64,
            },
            CadDimensionState {
                dimension_id: "flexure_thickness_mm".to_string(),
                label: "Flexure Thick".to_string(),
                value_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_DEFAULT_FLEXURE_THICKNESS_MM,
                min_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MIN_FLEXURE_THICKNESS_MM,
                max_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MAX_FLEXURE_THICKNESS_MM,
            },
            CadDimensionState {
                dimension_id: "finger_count".to_string(),
                label: "Finger Count".to_string(),
                value_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_DEFAULT_FINGER_COUNT as f64,
                min_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MIN_FINGER_COUNT as f64,
                max_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MAX_FINGER_COUNT as f64,
            },
            CadDimensionState {
                dimension_id: "thumb_base_angle_deg".to_string(),
                label: "Thumb Angle".to_string(),
                value_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_DEFAULT_THUMB_BASE_ANGLE_DEG,
                min_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MIN_THUMB_BASE_ANGLE_DEG,
                max_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MAX_THUMB_BASE_ANGLE_DEG,
            },
            CadDimensionState {
                dimension_id: "tendon_channel_diameter_mm".to_string(),
                label: "Tendon Ch.".to_string(),
                value_mm:
                    openagents_cad::intent::PARALLEL_JAW_GRIPPER_DEFAULT_TENDON_CHANNEL_DIAMETER_MM,
                min_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MIN_TENDON_CHANNEL_DIAMETER_MM,
                max_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MAX_TENDON_CHANNEL_DIAMETER_MM,
            },
            CadDimensionState {
                dimension_id: "joint_min_deg".to_string(),
                label: "Joint Min".to_string(),
                value_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_DEFAULT_JOINT_MIN_DEG,
                min_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MIN_JOINT_MIN_DEG,
                max_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MAX_JOINT_MIN_DEG,
            },
            CadDimensionState {
                dimension_id: "joint_max_deg".to_string(),
                label: "Joint Max".to_string(),
                value_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_DEFAULT_JOINT_MAX_DEG,
                min_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MIN_JOINT_MAX_DEG,
                max_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MAX_JOINT_MAX_DEG,
            },
            CadDimensionState {
                dimension_id: "tendon_route_clearance_mm".to_string(),
                label: "Route Clear".to_string(),
                value_mm:
                    openagents_cad::intent::PARALLEL_JAW_GRIPPER_DEFAULT_TENDON_ROUTE_CLEARANCE_MM,
                min_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MIN_TENDON_ROUTE_CLEARANCE_MM,
                max_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MAX_TENDON_ROUTE_CLEARANCE_MM,
            },
            CadDimensionState {
                dimension_id: "tendon_bend_radius_mm".to_string(),
                label: "Bend Radius".to_string(),
                value_mm:
                    openagents_cad::intent::PARALLEL_JAW_GRIPPER_DEFAULT_TENDON_BEND_RADIUS_MM,
                min_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MIN_TENDON_BEND_RADIUS_MM,
                max_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MAX_TENDON_BEND_RADIUS_MM,
            },
            CadDimensionState {
                dimension_id: "servo_envelope_length_mm".to_string(),
                label: "Servo Len".to_string(),
                value_mm:
                    openagents_cad::intent::PARALLEL_JAW_GRIPPER_DEFAULT_SERVO_ENVELOPE_LENGTH_MM,
                min_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MIN_SERVO_ENVELOPE_LENGTH_MM,
                max_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MAX_SERVO_ENVELOPE_LENGTH_MM,
            },
            CadDimensionState {
                dimension_id: "servo_envelope_width_mm".to_string(),
                label: "Servo Wid".to_string(),
                value_mm:
                    openagents_cad::intent::PARALLEL_JAW_GRIPPER_DEFAULT_SERVO_ENVELOPE_WIDTH_MM,
                min_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MIN_SERVO_ENVELOPE_WIDTH_MM,
                max_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MAX_SERVO_ENVELOPE_WIDTH_MM,
            },
            CadDimensionState {
                dimension_id: "servo_envelope_height_mm".to_string(),
                label: "Servo Hgt".to_string(),
                value_mm:
                    openagents_cad::intent::PARALLEL_JAW_GRIPPER_DEFAULT_SERVO_ENVELOPE_HEIGHT_MM,
                min_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MIN_SERVO_ENVELOPE_HEIGHT_MM,
                max_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MAX_SERVO_ENVELOPE_HEIGHT_MM,
            },
            CadDimensionState {
                dimension_id: "servo_shaft_axis_offset_mm".to_string(),
                label: "Shaft Off".to_string(),
                value_mm:
                    openagents_cad::intent::PARALLEL_JAW_GRIPPER_DEFAULT_SERVO_SHAFT_AXIS_OFFSET_MM,
                min_mm:
                    openagents_cad::intent::PARALLEL_JAW_GRIPPER_MIN_SERVO_SHAFT_AXIS_OFFSET_MM,
                max_mm:
                    openagents_cad::intent::PARALLEL_JAW_GRIPPER_MAX_SERVO_SHAFT_AXIS_OFFSET_MM,
            },
            CadDimensionState {
                dimension_id: "servo_mount_pattern_pitch_mm".to_string(),
                label: "Mount Pitch".to_string(),
                value_mm:
                    openagents_cad::intent::PARALLEL_JAW_GRIPPER_DEFAULT_SERVO_MOUNT_PATTERN_PITCH_MM,
                min_mm:
                    openagents_cad::intent::PARALLEL_JAW_GRIPPER_MIN_SERVO_MOUNT_PATTERN_PITCH_MM,
                max_mm:
                    openagents_cad::intent::PARALLEL_JAW_GRIPPER_MAX_SERVO_MOUNT_PATTERN_PITCH_MM,
            },
            CadDimensionState {
                dimension_id: "servo_bracket_thickness_mm".to_string(),
                label: "Bracket Thk".to_string(),
                value_mm:
                    openagents_cad::intent::PARALLEL_JAW_GRIPPER_DEFAULT_SERVO_BRACKET_THICKNESS_MM,
                min_mm:
                    openagents_cad::intent::PARALLEL_JAW_GRIPPER_MIN_SERVO_BRACKET_THICKNESS_MM,
                max_mm:
                    openagents_cad::intent::PARALLEL_JAW_GRIPPER_MAX_SERVO_BRACKET_THICKNESS_MM,
            },
            CadDimensionState {
                dimension_id: "servo_housing_wall_mm".to_string(),
                label: "Housing Wall".to_string(),
                value_mm:
                    openagents_cad::intent::PARALLEL_JAW_GRIPPER_DEFAULT_SERVO_HOUSING_WALL_MM,
                min_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MIN_SERVO_HOUSING_WALL_MM,
                max_mm: openagents_cad::intent::PARALLEL_JAW_GRIPPER_MAX_SERVO_HOUSING_WALL_MM,
            },
            CadDimensionState {
                dimension_id: "servo_standoff_diameter_mm".to_string(),
                label: "Standoff Dia".to_string(),
                value_mm:
                    openagents_cad::intent::PARALLEL_JAW_GRIPPER_DEFAULT_SERVO_STANDOFF_DIAMETER_MM,
                min_mm:
                    openagents_cad::intent::PARALLEL_JAW_GRIPPER_MIN_SERVO_STANDOFF_DIAMETER_MM,
                max_mm:
                    openagents_cad::intent::PARALLEL_JAW_GRIPPER_MAX_SERVO_STANDOFF_DIAMETER_MM,
            },
            CadDimensionState {
                dimension_id: "gearbox_ratio".to_string(),
                label: "Gear Ratio".to_string(),
                value_mm: 4.5,
                min_mm: 1.0,
                max_mm: 14.0,
            },
            CadDimensionState {
                dimension_id: "gearbox_stage_diameter_mm".to_string(),
                label: "Gear Dia".to_string(),
                value_mm: 11.0,
                min_mm: 6.0,
                max_mm: 30.0,
            },
            CadDimensionState {
                dimension_id: "gearbox_stage_length_mm".to_string(),
                label: "Gear Len".to_string(),
                value_mm: 14.0,
                min_mm: 6.0,
                max_mm: 42.0,
            },
            CadDimensionState {
                dimension_id: "wiring_channel_diameter_mm".to_string(),
                label: "Wire Dia".to_string(),
                value_mm: 1.8,
                min_mm: 0.8,
                max_mm: 8.0,
            },
            CadDimensionState {
                dimension_id: "wiring_bend_radius_mm".to_string(),
                label: "Wire Bend".to_string(),
                value_mm: 2.6,
                min_mm: 0.8,
                max_mm: 16.0,
            },
            CadDimensionState {
                dimension_id: "wiring_clearance_mm".to_string(),
                label: "Wire Clear".to_string(),
                value_mm: 1.2,
                min_mm: 0.2,
                max_mm: 6.0,
            },
            CadDimensionState {
                dimension_id: "force_sensor_pad_diameter_mm".to_string(),
                label: "Pad Dia".to_string(),
                value_mm: 6.4,
                min_mm: 2.0,
                max_mm: 16.0,
            },
            CadDimensionState {
                dimension_id: "proximity_sensor_port_diameter_mm".to_string(),
                label: "Prox Port".to_string(),
                value_mm: 4.0,
                min_mm: 1.0,
                max_mm: 12.0,
            },
            CadDimensionState {
                dimension_id: "control_board_mount_width_mm".to_string(),
                label: "Board W".to_string(),
                value_mm: 34.0,
                min_mm: 10.0,
                max_mm: 90.0,
            },
            CadDimensionState {
                dimension_id: "control_board_mount_depth_mm".to_string(),
                label: "Board D".to_string(),
                value_mm: 24.0,
                min_mm: 8.0,
                max_mm: 70.0,
            },
            CadDimensionState {
                dimension_id: "control_board_mount_height_mm".to_string(),
                label: "Board H".to_string(),
                value_mm: 6.0,
                min_mm: 2.0,
                max_mm: 24.0,
            },
            CadDimensionState {
                dimension_id: "modular_mount_slot_pitch_mm".to_string(),
                label: "Slot Pitch".to_string(),
                value_mm: 8.0,
                min_mm: 3.0,
                max_mm: 20.0,
            },
            CadDimensionState {
                dimension_id: "modular_mount_slot_count".to_string(),
                label: "Slot Count".to_string(),
                value_mm: 4.0,
                min_mm: 2.0,
                max_mm: 10.0,
            },
            CadDimensionState {
                dimension_id: "electrical_clearance_mm".to_string(),
                label: "Elec Clr".to_string(),
                value_mm: 2.2,
                min_mm: 0.5,
                max_mm: 12.0,
            },
        ];
        let assembly_schema = openagents_cad::assembly::CadAssemblySchema {
            part_defs: std::collections::BTreeMap::from([
                (
                    "base".to_string(),
                    openagents_cad::assembly::CadPartDef {
                        id: "base".to_string(),
                        name: Some("Base".to_string()),
                        root: 1,
                        default_material: Some("aluminum".to_string()),
                    },
                ),
                (
                    "arm".to_string(),
                    openagents_cad::assembly::CadPartDef {
                        id: "arm".to_string(),
                        name: Some("Arm".to_string()),
                        root: 2,
                        default_material: Some("steel".to_string()),
                    },
                ),
            ]),
            instances: vec![
                openagents_cad::assembly::CadPartInstance {
                    id: "base-1".to_string(),
                    part_def_id: "base".to_string(),
                    name: Some("Base".to_string()),
                    transform: Some(openagents_cad::assembly::CadTransform3D::identity()),
                    material: None,
                },
                openagents_cad::assembly::CadPartInstance {
                    id: "arm-1".to_string(),
                    part_def_id: "arm".to_string(),
                    name: Some("Arm".to_string()),
                    transform: Some(openagents_cad::assembly::CadTransform3D {
                        translation: openagents_cad::kernel_math::Vec3::new(10.0, 0.0, 0.0),
                        rotation: openagents_cad::kernel_math::Vec3::new(0.0, 0.0, 0.0),
                        scale: openagents_cad::kernel_math::Vec3::new(1.0, 1.0, 1.0),
                    }),
                    material: None,
                },
            ],
            joints: vec![openagents_cad::assembly::CadAssemblyJoint {
                id: "joint.hinge".to_string(),
                name: Some("Hinge".to_string()),
                parent_instance_id: Some("base-1".to_string()),
                child_instance_id: "arm-1".to_string(),
                parent_anchor: openagents_cad::kernel_math::Vec3::new(0.0, 0.0, 0.0),
                child_anchor: openagents_cad::kernel_math::Vec3::new(0.0, 0.0, 0.0),
                kind: openagents_cad::assembly::CadJointKind::Revolute {
                    axis: openagents_cad::kernel_math::Vec3::new(0.0, 0.0, 1.0),
                    limits: Some((-90.0, 90.0)),
                },
                state: 0.0,
            }],
            ground_instance_id: Some("base-1".to_string()),
        };
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
            variant_materials,
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
            sensor_visualization_mode: CadSensorVisualizationMode::Off,
            snap_toggles: CadSnapToggles::default(),
            projection_mode: CadProjectionMode::Orthographic,
            viewport_layout: CadViewportLayout::Single,
            gripper_jaw_open: false,
            grasp_simulation_seed: 20_260_303,
            grasp_simulation_samples: Vec::new(),
            grasp_simulation_last_updated_revision: 0,
            sensor_feedback_readings: Vec::new(),
            sensor_feedback_trace: Vec::new(),
            sensor_feedback_last_updated_revision: 0,
            drawing_view_mode: CadDrawingViewMode::ThreeD,
            drawing_view_direction: CadDrawingViewDirection::Front,
            drawing_show_hidden_lines: true,
            drawing_show_dimensions: true,
            drawing_zoom: 1.0,
            drawing_pan_x: 0.0,
            drawing_pan_y: 0.0,
            drawing_detail_views: Vec::new(),
            drawing_next_detail_id: 1,
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
            assembly_schema,
            assembly_ui_state: openagents_cad::assembly::CadAssemblyUiState::default(),
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
    fn default_analysis_for_variant(
        &self,
        variant_id: &str,
    ) -> openagents_cad::contracts::CadAnalysis {
        openagents_cad::contracts::CadAnalysis {
            document_revision: self.document_revision,
            variant_id: variant_id.to_string(),
            material_id: self
                .variant_materials
                .get(variant_id)
                .cloned()
                .or_else(|| Some(openagents_cad::materials::DEFAULT_CAD_MATERIAL_ID.to_string())),
            volume_mm3: None,
            mass_kg: None,
            center_of_gravity_mm: None,
            estimated_cost_usd: None,
            max_deflection_mm: None,
            estimator_metadata: std::collections::BTreeMap::new(),
            objective_scores: std::collections::BTreeMap::new(),
        }
    }

    pub fn active_dispatch_state(&self) -> Option<&openagents_cad::dispatch::CadDispatchState> {
        let session_id = self
            .active_chat_session_id
            .as_ref()
            .unwrap_or(&self.session_id);
        self.dispatch_sessions.get(session_id)
    }

    pub fn active_dispatch_state_mut(
        &mut self,
    ) -> Option<&mut openagents_cad::dispatch::CadDispatchState> {
        let session_id = self
            .active_chat_session_id
            .clone()
            .unwrap_or_else(|| self.session_id.clone());
        self.dispatch_sessions.get_mut(&session_id)
    }

    pub fn active_design_profile(&self) -> openagents_cad::dispatch::CadDesignProfile {
        self.active_dispatch_state()
            .map(|state| state.design_profile)
            .unwrap_or_default()
    }

    fn profile_dimension_ids(
        profile: openagents_cad::dispatch::CadDesignProfile,
    ) -> &'static [&'static str] {
        match profile {
            openagents_cad::dispatch::CadDesignProfile::Rack => {
                &["width_mm", "depth_mm", "height_mm", "wall_mm"]
            }
            openagents_cad::dispatch::CadDesignProfile::ParallelJawGripper => &[
                "jaw_open_mm",
                "finger_length_mm",
                "finger_thickness_mm",
                "base_width_mm",
            ],
            openagents_cad::dispatch::CadDesignProfile::ParallelJawGripperUnderactuated => &[
                "jaw_open_mm",
                "finger_length_mm",
                "finger_thickness_mm",
                "base_width_mm",
                "compliant_joint_count",
                "flexure_thickness_mm",
            ],
            openagents_cad::dispatch::CadDesignProfile::ThreeFingerThumb => &[
                "jaw_open_mm",
                "finger_length_mm",
                "finger_thickness_mm",
                "base_width_mm",
                "base_depth_mm",
                "finger_count",
                "thumb_base_angle_deg",
                "tendon_channel_diameter_mm",
                "joint_min_deg",
                "joint_max_deg",
                "tendon_route_clearance_mm",
                "tendon_bend_radius_mm",
                "servo_envelope_length_mm",
                "servo_envelope_width_mm",
                "servo_envelope_height_mm",
                "servo_shaft_axis_offset_mm",
                "servo_mount_pattern_pitch_mm",
                "servo_bracket_thickness_mm",
                "servo_housing_wall_mm",
                "servo_standoff_diameter_mm",
                "gearbox_ratio",
                "gearbox_stage_diameter_mm",
                "gearbox_stage_length_mm",
                "wiring_channel_diameter_mm",
                "wiring_bend_radius_mm",
                "wiring_clearance_mm",
                "force_sensor_pad_diameter_mm",
                "proximity_sensor_port_diameter_mm",
                "control_board_mount_width_mm",
                "control_board_mount_depth_mm",
                "control_board_mount_height_mm",
                "modular_mount_slot_pitch_mm",
                "modular_mount_slot_count",
                "electrical_clearance_mm",
            ],
            openagents_cad::dispatch::CadDesignProfile::HumanoidHandV1 => &[
                "jaw_open_mm",
                "finger_length_mm",
                "finger_thickness_mm",
                "base_width_mm",
                "base_depth_mm",
                "finger_count",
                "thumb_base_angle_deg",
                "tendon_channel_diameter_mm",
                "joint_min_deg",
                "joint_max_deg",
                "tendon_route_clearance_mm",
                "tendon_bend_radius_mm",
                "servo_envelope_length_mm",
                "servo_envelope_width_mm",
                "servo_envelope_height_mm",
                "servo_shaft_axis_offset_mm",
                "servo_mount_pattern_pitch_mm",
                "servo_bracket_thickness_mm",
                "servo_housing_wall_mm",
                "servo_standoff_diameter_mm",
                "gearbox_ratio",
                "gearbox_stage_diameter_mm",
                "gearbox_stage_length_mm",
                "wiring_channel_diameter_mm",
                "wiring_bend_radius_mm",
                "wiring_clearance_mm",
                "force_sensor_pad_diameter_mm",
                "proximity_sensor_port_diameter_mm",
                "control_board_mount_width_mm",
                "control_board_mount_depth_mm",
                "control_board_mount_height_mm",
                "modular_mount_slot_pitch_mm",
                "modular_mount_slot_count",
                "electrical_clearance_mm",
            ],
        }
    }

    fn align_dimensions_for_profile(
        &mut self,
        profile: openagents_cad::dispatch::CadDesignProfile,
    ) {
        let preferred_ids = Self::profile_dimension_ids(profile);
        let mut reordered = Vec::with_capacity(self.dimensions.len());
        for dimension_id in preferred_ids {
            if let Some(index) = self
                .dimensions
                .iter()
                .position(|dimension| dimension.dimension_id == *dimension_id)
            {
                reordered.push(self.dimensions[index].clone());
            }
        }
        for dimension in &self.dimensions {
            if !reordered
                .iter()
                .any(|entry| entry.dimension_id == dimension.dimension_id)
            {
                reordered.push(dimension.clone());
            }
        }
        self.dimensions = reordered;
    }

    pub fn visible_dimension_indices(&self) -> Vec<usize> {
        let preferred_ids = Self::profile_dimension_ids(self.active_design_profile());
        self.dimensions
            .iter()
            .enumerate()
            .filter(|(_, dimension)| preferred_ids.contains(&dimension.dimension_id.as_str()))
            .map(|(index, _)| index)
            .collect()
    }

    pub fn visible_dimension_slots(&self) -> Vec<(usize, &CadDimensionState)> {
        self.visible_dimension_indices()
            .into_iter()
            .filter_map(|index| {
                self.dimensions
                    .get(index)
                    .map(|dimension| (index, dimension))
            })
            .collect()
    }

    pub fn dimension_index_for_visible_row(&self, visible_row_index: usize) -> Option<usize> {
        self.visible_dimension_indices()
            .get(visible_row_index)
            .copied()
    }

    pub fn ensure_variant_family_for_profile(
        &mut self,
        profile: openagents_cad::dispatch::CadDesignProfile,
    ) {
        let target = match profile {
            openagents_cad::dispatch::CadDesignProfile::Rack => vec![
                "variant.baseline".to_string(),
                "variant.lightweight".to_string(),
                "variant.low-cost".to_string(),
                "variant.stiffness".to_string(),
            ],
            openagents_cad::dispatch::CadDesignProfile::ParallelJawGripper => vec![
                "variant.baseline".to_string(),
                "variant.wide-jaw".to_string(),
                "variant.long-reach".to_string(),
                "variant.stiff-finger".to_string(),
            ],
            openagents_cad::dispatch::CadDesignProfile::ParallelJawGripperUnderactuated => vec![
                "variant.baseline".to_string(),
                "variant.wide-jaw".to_string(),
                "variant.long-reach".to_string(),
                "variant.stiff-finger".to_string(),
            ],
            openagents_cad::dispatch::CadDesignProfile::ThreeFingerThumb => vec![
                "variant.baseline".to_string(),
                "variant.pinch".to_string(),
                "variant.tripod".to_string(),
                "variant.wide-thumb".to_string(),
            ],
            openagents_cad::dispatch::CadDesignProfile::HumanoidHandV1 => vec![
                "variant.baseline".to_string(),
                "variant.precision".to_string(),
                "variant.power".to_string(),
                "variant.wide-spread".to_string(),
            ],
        };
        if self.variant_ids == target {
            return;
        }

        let previous_active = self.active_variant_id.clone();
        let existing_viewports = self
            .variant_viewports
            .iter()
            .map(|view| (view.variant_id.clone(), view.clone()))
            .collect::<std::collections::BTreeMap<_, _>>();
        self.variant_ids = target.clone();
        self.variant_viewports = target
            .iter()
            .map(|variant_id| {
                existing_viewports
                    .get(variant_id)
                    .cloned()
                    .unwrap_or_else(|| CadVariantViewportState::for_variant(variant_id))
            })
            .collect();

        for variant_id in &target {
            self.variant_materials
                .entry(variant_id.clone())
                .or_insert_with(|| openagents_cad::materials::DEFAULT_CAD_MATERIAL_ID.to_string());
            self.variant_warning_sets
                .entry(variant_id.clone())
                .or_default();
            if !self.variant_analysis_snapshots.contains_key(variant_id) {
                let default_analysis = self.default_analysis_for_variant(variant_id);
                self.variant_analysis_snapshots
                    .insert(variant_id.clone(), default_analysis);
            }
        }
        self.variant_materials
            .retain(|variant_id, _| target.contains(variant_id));
        self.variant_warning_sets
            .retain(|variant_id, _| target.contains(variant_id));
        self.variant_analysis_snapshots
            .retain(|variant_id, _| target.contains(variant_id));

        self.active_variant_id = if target.contains(&previous_active) {
            previous_active
        } else {
            target
                .first()
                .cloned()
                .unwrap_or_else(|| "variant.baseline".to_string())
        };
        self.active_variant_tile_index = self
            .variant_viewports
            .iter()
            .position(|viewport| viewport.variant_id == self.active_variant_id)
            .unwrap_or(0);
        self.align_dimensions_for_profile(profile);
        self.sync_global_from_variant_viewport();
        self.sync_active_variant_payloads_from_maps();
    }

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

    fn set_dimension_value_mm_if_present(&mut self, dimension_id: &str, value_mm: f64) {
        let Some(dimension) = self
            .dimensions
            .iter_mut()
            .find(|dimension| dimension.dimension_id == dimension_id)
        else {
            return;
        };
        if !value_mm.is_finite() {
            return;
        }
        dimension.value_mm = value_mm.clamp(dimension.min_mm, dimension.max_mm);
    }

    pub fn apply_parallel_jaw_gripper_spec_dimensions(
        &mut self,
        spec: &openagents_cad::intent::CreateParallelJawGripperSpecIntent,
    ) {
        self.set_dimension_value_mm_if_present("jaw_open_mm", spec.jaw_open_mm);
        self.set_dimension_value_mm_if_present("finger_length_mm", spec.finger_length_mm);
        self.set_dimension_value_mm_if_present("finger_thickness_mm", spec.finger_thickness_mm);
        self.set_dimension_value_mm_if_present("base_width_mm", spec.base_width_mm);
        self.set_dimension_value_mm_if_present("base_depth_mm", spec.base_depth_mm);
        self.set_dimension_value_mm_if_present("base_thickness_mm", spec.base_thickness_mm);
        self.set_dimension_value_mm_if_present(
            "servo_mount_hole_diameter_mm",
            spec.servo_mount_hole_diameter_mm,
        );
        self.set_dimension_value_mm_if_present("print_fit_mm", spec.print_fit_mm);
        self.set_dimension_value_mm_if_present("print_clearance_mm", spec.print_clearance_mm);
        self.set_dimension_value_mm_if_present(
            "compliant_joint_count",
            spec.compliant_joint_count as f64,
        );
        self.set_dimension_value_mm_if_present("flexure_thickness_mm", spec.flexure_thickness_mm);
        self.set_dimension_value_mm_if_present("finger_count", spec.finger_count as f64);
        self.set_dimension_value_mm_if_present("thumb_base_angle_deg", spec.thumb_base_angle_deg);
        self.set_dimension_value_mm_if_present(
            "tendon_channel_diameter_mm",
            spec.tendon_channel_diameter_mm,
        );
        self.set_dimension_value_mm_if_present("joint_min_deg", spec.joint_min_deg);
        self.set_dimension_value_mm_if_present("joint_max_deg", spec.joint_max_deg);
        self.set_dimension_value_mm_if_present(
            "tendon_route_clearance_mm",
            spec.tendon_route_clearance_mm,
        );
        self.set_dimension_value_mm_if_present("tendon_bend_radius_mm", spec.tendon_bend_radius_mm);
        self.set_dimension_value_mm_if_present(
            "servo_envelope_length_mm",
            spec.servo_envelope_length_mm,
        );
        self.set_dimension_value_mm_if_present(
            "servo_envelope_width_mm",
            spec.servo_envelope_width_mm,
        );
        self.set_dimension_value_mm_if_present(
            "servo_envelope_height_mm",
            spec.servo_envelope_height_mm,
        );
        self.set_dimension_value_mm_if_present(
            "servo_shaft_axis_offset_mm",
            spec.servo_shaft_axis_offset_mm,
        );
        self.set_dimension_value_mm_if_present(
            "servo_mount_pattern_pitch_mm",
            spec.servo_mount_pattern_pitch_mm,
        );
        self.set_dimension_value_mm_if_present(
            "servo_bracket_thickness_mm",
            spec.servo_bracket_thickness_mm,
        );
        self.set_dimension_value_mm_if_present("servo_housing_wall_mm", spec.servo_housing_wall_mm);
        self.set_dimension_value_mm_if_present(
            "servo_standoff_diameter_mm",
            spec.servo_standoff_diameter_mm,
        );
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

    pub fn select_assembly_instance(&mut self, instance_id: &str) -> Result<(), String> {
        self.assembly_ui_state
            .select_instance(&self.assembly_schema, instance_id)
            .map_err(|error| error.to_string())
    }

    pub fn select_assembly_joint(&mut self, joint_id: &str) -> Result<(), String> {
        self.assembly_ui_state
            .select_joint(&self.assembly_schema, joint_id)
            .map_err(|error| error.to_string())
    }

    pub fn rename_selected_assembly_instance(&mut self, name: String) -> Result<(), String> {
        self.assembly_ui_state
            .rename_selected_instance(&mut self.assembly_schema, name)
            .map_err(|error| error.to_string())
    }

    pub fn set_selected_assembly_joint_state(
        &mut self,
        requested_state: f64,
    ) -> Result<openagents_cad::assembly::CadJointStateSemantics, String> {
        self.assembly_ui_state
            .set_selected_joint_state(&mut self.assembly_schema, requested_state)
            .map_err(|error| error.to_string())
    }

    pub fn sync_assembly_ui_selection(&mut self) {
        self.assembly_ui_state
            .sync_with_schema(&self.assembly_schema);
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
        let profile = dispatch_state.design_profile;
        self.document_revision = receipt.state_revision;
        self.ensure_variant_family_for_profile(profile);
        if matches!(
            profile,
            openagents_cad::dispatch::CadDesignProfile::ThreeFingerThumb
                | openagents_cad::dispatch::CadDesignProfile::HumanoidHandV1
        ) {
            if self.sensor_visualization_mode == CadSensorVisualizationMode::Off {
                self.sensor_visualization_mode = CadSensorVisualizationMode::Combined;
            }
        } else {
            self.sensor_visualization_mode = CadSensorVisualizationMode::Off;
            self.sensor_feedback_readings.clear();
            self.sensor_feedback_trace.clear();
        }
        match intent {
            openagents_cad::intent::CadIntent::CreateParallelJawGripperSpec(spec) => {
                self.apply_parallel_jaw_gripper_spec_dimensions(spec);
            }
            openagents_cad::intent::CadIntent::SetMaterial(payload) => {
                self.set_active_variant_material(payload.material_id.as_str());
            }
            _ => {}
        }
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

    fn normalize_orbit_angle_deg(angle_deg: f32) -> f32 {
        let wrapped = angle_deg.rem_euclid(360.0);
        if wrapped >= 180.0 {
            wrapped - 360.0
        } else {
            wrapped
        }
    }

    fn shortest_orbit_angle_delta_deg(lhs: f32, rhs: f32) -> f32 {
        Self::normalize_orbit_angle_deg(lhs - rhs).abs()
    }

    pub fn orbit_camera_by_drag(&mut self, drag_dx: f32, drag_dy: f32) {
        const ORBIT_SENSITIVITY_DEG_PER_PX: f32 = 0.28;
        self.camera_orbit_yaw_deg = Self::normalize_orbit_angle_deg(
            self.camera_orbit_yaw_deg + drag_dx * ORBIT_SENSITIVITY_DEG_PER_PX,
        );
        self.camera_orbit_pitch_deg = Self::normalize_orbit_angle_deg(
            self.camera_orbit_pitch_deg - drag_dy * ORBIT_SENSITIVITY_DEG_PER_PX,
        );
        self.sync_active_variant_viewport_from_global();
    }

    pub fn pan_camera_by_drag(&mut self, drag_dx: f32, drag_dy: f32) {
        const PAN_SENSITIVITY: f32 = 1.0;
        self.camera_pan_x = (self.camera_pan_x + drag_dx * PAN_SENSITIVITY).clamp(-800.0, 800.0);
        self.camera_pan_y = (self.camera_pan_y + drag_dy * PAN_SENSITIVITY).clamp(-800.0, 800.0);
        self.sync_active_variant_viewport_from_global();
    }

    pub fn zoom_camera_by_scroll(&mut self, scroll_dy: f32) {
        const CAD_CAMERA_MIN_ZOOM: f32 = 0.35;
        const CAD_CAMERA_MAX_ZOOM: f32 = 1.0;
        // Negative wheel deltas (scroll up on most devices) zoom in.
        let scale = (1.0 + (-scroll_dy * 0.0018)).clamp(0.75, 1.35);
        self.camera_zoom =
            (self.camera_zoom * scale).clamp(CAD_CAMERA_MIN_ZOOM, CAD_CAMERA_MAX_ZOOM);
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
            Self::shortest_orbit_angle_delta_deg(self.camera_orbit_yaw_deg, yaw)
                <= SNAP_TOLERANCE_DEG
                && Self::shortest_orbit_angle_delta_deg(self.camera_orbit_pitch_deg, pitch)
                    <= SNAP_TOLERANCE_DEG
        })
    }

    pub fn cycle_projection_mode(&mut self) {
        self.projection_mode = self.projection_mode.next();
    }

    pub fn cycle_sensor_visualization_mode(&mut self) -> CadSensorVisualizationMode {
        self.sensor_visualization_mode = self.sensor_visualization_mode.next();
        self.sensor_visualization_mode
    }

    pub fn toggle_drawing_view_mode(&mut self) -> CadDrawingViewMode {
        self.drawing_view_mode = self.drawing_view_mode.next();
        self.drawing_view_mode
    }

    pub fn cycle_drawing_view_direction(&mut self) -> CadDrawingViewDirection {
        self.drawing_view_direction = self.drawing_view_direction.next();
        self.reset_drawing_view();
        self.drawing_view_direction
    }

    pub fn toggle_drawing_hidden_lines(&mut self) -> bool {
        self.drawing_show_hidden_lines = !self.drawing_show_hidden_lines;
        self.drawing_show_hidden_lines
    }

    pub fn toggle_drawing_dimensions(&mut self) -> bool {
        self.drawing_show_dimensions = !self.drawing_show_dimensions;
        self.drawing_show_dimensions
    }

    pub fn reset_drawing_view(&mut self) {
        self.drawing_zoom = 1.0;
        self.drawing_pan_x = 0.0;
        self.drawing_pan_y = 0.0;
    }

    pub fn zoom_drawing_view_by_scroll(&mut self, scroll_dy: f32) {
        let scale = (1.0 + (-scroll_dy * 0.002)).clamp(0.5, 1.5);
        self.drawing_zoom = (self.drawing_zoom * scale).clamp(0.1, 10.0);
    }

    pub fn pan_drawing_view_by_drag(&mut self, drag_dx: f32, drag_dy: f32) {
        self.drawing_pan_x = (self.drawing_pan_x + drag_dx).clamp(-10_000.0, 10_000.0);
        self.drawing_pan_y = (self.drawing_pan_y + drag_dy).clamp(-10_000.0, 10_000.0);
    }

    pub fn add_drawing_detail_view(&mut self) -> CadDrawingDetailViewState {
        let detail_id = format!("detail-{}", self.drawing_next_detail_id);
        self.drawing_next_detail_id = self.drawing_next_detail_id.saturating_add(1);
        let label =
            char::from_u32(u32::from(b'A') + (self.drawing_detail_views.len().min(25) as u32))
                .unwrap_or('A')
                .to_string();
        let detail = CadDrawingDetailViewState {
            detail_id,
            label,
            center_x: 0.0,
            center_y: 0.0,
            width: 40.0,
            height: 40.0,
            scale: 2.0,
        };
        self.drawing_detail_views.push(detail.clone());
        detail
    }

    pub fn clear_drawing_detail_views(&mut self) -> usize {
        let count = self.drawing_detail_views.len();
        self.drawing_detail_views.clear();
        count
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
            .variant_materials
            .get(&self.active_variant_id)
            .map(String::as_str)
            .or(self.analysis_snapshot.material_id.as_deref())
            .unwrap_or(openagents_cad::materials::DEFAULT_CAD_MATERIAL_ID);
        let next = openagents_cad::materials::next_material_preset_id(current).to_string();
        self.variant_materials
            .insert(self.active_variant_id.clone(), next.clone());
        if let Some(analysis) = self
            .variant_analysis_snapshots
            .get_mut(&self.active_variant_id)
        {
            analysis.material_id = Some(next.clone());
        }
        self.analysis_snapshot.material_id = Some(next.clone());
        next
    }

    pub fn set_active_variant_material(&mut self, material_id: &str) {
        let normalized = material_id.trim().to_string();
        if normalized.is_empty() {
            return;
        }
        self.variant_materials
            .insert(self.active_variant_id.clone(), normalized.clone());
        if let Some(analysis) = self
            .variant_analysis_snapshots
            .get_mut(&self.active_variant_id)
        {
            analysis.material_id = Some(normalized.clone());
        }
        self.analysis_snapshot.material_id = Some(normalized);
    }

    pub fn toggle_viewport_layout(&mut self) -> CadViewportLayout {
        self.viewport_layout = self.viewport_layout.next();
        self.viewport_layout
    }

    pub fn visible_variant_ids(&self) -> Vec<String> {
        match self.viewport_layout {
            CadViewportLayout::Single => vec![self.active_variant_id.clone()],
            CadViewportLayout::Quad => self.variant_ids.iter().take(4).cloned().collect(),
        }
    }

    pub fn all_variants_visible(&self) -> bool {
        self.visible_variant_ids().len() == self.variant_ids.len()
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
pub struct ProtocolTraceEvent {
    pub seq: u64,
    pub protocol: String,
    pub event_ref: String,
    pub summary: String,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StableSatsTreasuryOperationKind {
    Refresh,
    SwapQuote,
    SwapExecute,
    TransferBtc,
    TransferUsd,
    Convert,
}

impl StableSatsTreasuryOperationKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Refresh => "refresh",
            Self::SwapQuote => "swap_quote",
            Self::SwapExecute => "swap_execute",
            Self::TransferBtc => "transfer_btc",
            Self::TransferUsd => "transfer_usd",
            Self::Convert => "convert",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StableSatsTreasuryOperationStatus {
    Queued,
    Running,
    Settled,
    Failed,
    Cancelled,
}

impl StableSatsTreasuryOperationStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Settled => "settled",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StableSatsTreasuryOperationEntry {
    pub seq: u64,
    pub request_id: u64,
    pub kind: StableSatsTreasuryOperationKind,
    pub status: StableSatsTreasuryOperationStatus,
    pub detail: String,
    pub created_at_epoch_seconds: u64,
    pub updated_at_epoch_seconds: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StableSatsTreasuryReceipt {
    pub seq: u64,
    pub request_id: u64,
    pub kind: StableSatsTreasuryOperationKind,
    pub payload: serde_json::Value,
    pub occurred_at_epoch_seconds: u64,
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
    pub treasury_operations: Vec<StableSatsTreasuryOperationEntry>,
    pub treasury_receipts: Vec<StableSatsTreasuryReceipt>,
    pub events: Vec<ProtocolTraceEvent>,
    next_seq: u64,
    next_transfer_seq: u64,
    next_operation_seq: u64,
    next_receipt_seq: u64,
    auto_run_last_tick: Option<Instant>,
    next_live_refresh_request_id: u64,
}

impl Default for StableSatsSimulationPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Blink treasury wallet lane ready for live refresh".to_string()),
            mode: StableSatsSimulationMode::RealBlink,
            rounds_run: 0,
            price_usd_cents_per_btc: Self::BASE_PRICE_USD_CENTS_PER_BTC,
            total_converted_sats: 0,
            total_converted_usd_cents: 0,
            last_settlement_ref: None,
            agents: Self::default_real_wallets(),
            price_history_usd_cents_per_btc: Vec::new(),
            converted_sats_history: Vec::new(),
            auto_run_enabled: false,
            auto_run_interval: Duration::from_millis(120),
            live_refresh_pending: false,
            active_live_refresh_request_id: None,
            transfer_ledger: Vec::new(),
            treasury_operations: Vec::new(),
            treasury_receipts: Vec::new(),
            events: Vec::new(),
            next_seq: 1,
            next_transfer_seq: 1,
            next_operation_seq: 1,
            next_receipt_seq: 1,
            auto_run_last_tick: None,
            next_live_refresh_request_id: 1,
        }
    }
}

impl StableSatsSimulationPaneState {
    const SATS_PER_BTC: u128 = 100_000_000;
    const BASE_PRICE_USD_CENTS_PER_BTC: u64 = 8_400_000;
    const PRICE_STEP_USD_CENTS_PER_BTC: u64 = 12_500;

    pub fn total_btc_balance_sats(&self) -> u64 {
        self.agents.iter().map(|agent| agent.btc_balance_sats).sum()
    }

    pub fn total_usd_balance_cents(&self) -> u64 {
        self.agents
            .iter()
            .map(|agent| agent.usd_balance_cents)
            .sum()
    }

    pub fn has_settled_live_refresh(&self) -> bool {
        self.treasury_operations.iter().any(|entry| {
            entry.kind == StableSatsTreasuryOperationKind::Refresh
                && entry.status == StableSatsTreasuryOperationStatus::Settled
        })
    }

    pub fn set_mode(&mut self, mode: StableSatsSimulationMode) {
        if self.mode == mode {
            return;
        }
        self.mode = StableSatsSimulationMode::RealBlink;
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
        self.treasury_operations.clear();
        self.treasury_receipts.clear();
        self.events.clear();
        self.next_seq = 1;
        self.next_transfer_seq = 1;
        self.next_operation_seq = 1;
        self.next_receipt_seq = 1;
        self.next_live_refresh_request_id = 1;
        self.agents = Self::default_real_wallets();
        self.last_action = Some("StableSats mode switched to real Blink integration".to_string());
    }

    pub fn begin_live_refresh(&mut self) -> Result<u64, String> {
        if self.live_refresh_pending {
            return Err("Blink treasury refresh already in progress".to_string());
        }
        let request_id = self.reserve_worker_request_id();
        self.live_refresh_pending = true;
        self.active_live_refresh_request_id = Some(request_id);
        self.load_state = PaneLoadState::Loading;
        self.last_error = None;
        self.last_action = Some(format!(
            "Refreshing live Blink balances (request #{request_id})"
        ));
        let now_epoch_seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs());
        self.record_treasury_operation_queued(
            request_id,
            StableSatsTreasuryOperationKind::Refresh,
            now_epoch_seconds,
            "live wallet refresh queued".to_string(),
        );
        self.record_treasury_operation_running(
            request_id,
            StableSatsTreasuryOperationKind::Refresh,
            now_epoch_seconds,
            "live wallet refresh running".to_string(),
        );
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
        let now_epoch_seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs());
        self.record_treasury_operation_finished(
            request_id,
            StableSatsTreasuryOperationKind::Refresh,
            StableSatsTreasuryOperationStatus::Failed,
            now_epoch_seconds,
            format!("live wallet refresh failed: {error}"),
        );
        true
    }

    pub fn apply_live_wallet_snapshots(
        &mut self,
        now_epoch_seconds: u64,
        price_usd_cents_per_btc: u64,
        wallet_snapshots: &[(String, u64, u64, String)],
        wallet_failures: &[(String, String)],
    ) {
        if self.agents.is_empty() {
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
                        "delta btc={} usd={}",
                        crate::bitcoin_display::format_sats_amount(btc_delta),
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
                    "{} balances btc={} usd={} ({})",
                    wallet_name,
                    crate::bitcoin_display::format_sats_amount(*btc_balance_sats),
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
                        "{} live delta btc={} usd={}",
                        wallet_name,
                        crate::bitcoin_display::format_sats_amount(btc_delta),
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
                "live refresh settled wallets={} failed={} aggregate_btc={} aggregate_usd={}",
                refreshed_wallets,
                failed_wallets,
                crate::bitcoin_display::format_sats_amount(self.total_btc_balance_sats()),
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
        self.agents = Self::default_real_wallets();
        self.price_history_usd_cents_per_btc.clear();
        self.converted_sats_history.clear();
        self.auto_run_enabled = false;
        self.live_refresh_pending = false;
        self.active_live_refresh_request_id = None;
        self.transfer_ledger.clear();
        self.treasury_operations.clear();
        self.treasury_receipts.clear();
        self.events.clear();
        self.next_seq = 1;
        self.next_transfer_seq = 1;
        self.next_operation_seq = 1;
        self.next_receipt_seq = 1;
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
        self.events.push(ProtocolTraceEvent {
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
        vec![StableSatsAgentWalletState {
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
        }]
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

    pub fn apply_wallet_balance(
        &mut self,
        owner_id: &str,
        btc_balance_sats: u64,
        usd_balance_cents: u64,
        summary: String,
    ) {
        if let Some(wallet_index) = self
            .agents
            .iter()
            .position(|wallet| wallet.owner_id == owner_id)
        {
            let wallet = &mut self.agents[wallet_index];
            wallet.btc_balance_sats = btc_balance_sats;
            wallet.usd_balance_cents = usd_balance_cents;
            wallet.last_switch_summary = summary;
        }
    }

    pub fn record_external_transfer(
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
        self.push_transfer_ledger_entry(
            now_epoch_seconds,
            transfer_ref,
            from_wallet,
            to_wallet,
            asset,
            amount,
            effective_fee,
            status,
            summary,
        );
    }

    pub fn record_runtime_event(&mut self, protocol: &str, event_ref: String, summary: String) {
        self.push_event(protocol, event_ref.as_str(), summary);
    }

    pub fn reserve_worker_request_id(&mut self) -> u64 {
        let request_id = self.next_live_refresh_request_id;
        self.next_live_refresh_request_id = self.next_live_refresh_request_id.saturating_add(1);
        request_id
    }

    pub fn record_treasury_operation_queued(
        &mut self,
        request_id: u64,
        kind: StableSatsTreasuryOperationKind,
        now_epoch_seconds: u64,
        detail: String,
    ) {
        self.treasury_operations
            .push(StableSatsTreasuryOperationEntry {
                seq: self.next_operation_seq,
                request_id,
                kind,
                status: StableSatsTreasuryOperationStatus::Queued,
                detail,
                created_at_epoch_seconds: now_epoch_seconds,
                updated_at_epoch_seconds: now_epoch_seconds,
            });
        self.next_operation_seq = self.next_operation_seq.saturating_add(1);
        self.trim_treasury_operations();
    }

    pub fn record_treasury_operation_running(
        &mut self,
        request_id: u64,
        kind: StableSatsTreasuryOperationKind,
        now_epoch_seconds: u64,
        detail: String,
    ) {
        if let Some(entry) = self
            .treasury_operations
            .iter_mut()
            .rev()
            .find(|entry| entry.request_id == request_id && entry.kind == kind)
        {
            entry.status = StableSatsTreasuryOperationStatus::Running;
            entry.detail = detail;
            entry.updated_at_epoch_seconds = now_epoch_seconds;
        } else {
            self.treasury_operations
                .push(StableSatsTreasuryOperationEntry {
                    seq: self.next_operation_seq,
                    request_id,
                    kind,
                    status: StableSatsTreasuryOperationStatus::Running,
                    detail,
                    created_at_epoch_seconds: now_epoch_seconds,
                    updated_at_epoch_seconds: now_epoch_seconds,
                });
            self.next_operation_seq = self.next_operation_seq.saturating_add(1);
            self.trim_treasury_operations();
        }
    }

    pub fn record_treasury_operation_finished(
        &mut self,
        request_id: u64,
        kind: StableSatsTreasuryOperationKind,
        status: StableSatsTreasuryOperationStatus,
        now_epoch_seconds: u64,
        detail: String,
    ) {
        if let Some(entry) = self
            .treasury_operations
            .iter_mut()
            .rev()
            .find(|entry| entry.request_id == request_id && entry.kind == kind)
        {
            entry.status = status;
            entry.detail = detail;
            entry.updated_at_epoch_seconds = now_epoch_seconds;
        } else {
            self.treasury_operations
                .push(StableSatsTreasuryOperationEntry {
                    seq: self.next_operation_seq,
                    request_id,
                    kind,
                    status,
                    detail,
                    created_at_epoch_seconds: now_epoch_seconds,
                    updated_at_epoch_seconds: now_epoch_seconds,
                });
            self.next_operation_seq = self.next_operation_seq.saturating_add(1);
            self.trim_treasury_operations();
        }
    }

    fn trim_treasury_operations(&mut self) {
        if self.treasury_operations.len() > 96 {
            let overflow = self.treasury_operations.len().saturating_sub(96);
            self.treasury_operations.drain(0..overflow);
        }
    }

    pub fn record_treasury_receipt(
        &mut self,
        request_id: u64,
        kind: StableSatsTreasuryOperationKind,
        now_epoch_seconds: u64,
        payload: serde_json::Value,
    ) {
        self.treasury_receipts.push(StableSatsTreasuryReceipt {
            seq: self.next_receipt_seq,
            request_id,
            kind,
            payload,
            occurred_at_epoch_seconds: now_epoch_seconds,
        });
        self.next_receipt_seq = self.next_receipt_seq.saturating_add(1);
        if self.treasury_receipts.len() > 128 {
            let overflow = self.treasury_receipts.len().saturating_sub(128);
            self.treasury_receipts.drain(0..overflow);
        }
    }
}
