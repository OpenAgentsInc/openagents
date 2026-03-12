use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::{cell::RefCell, rc::Rc};

use chrono::{Datelike, Local, TimeZone, Utc};
use nostr::{Event, NostrIdentity};
use openagents_kernel_core::ids::sha256_prefixed_text;
use openagents_kernel_core::receipts::EvidenceRef;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wgpui::components::TextInput;
use wgpui::components::hud::{CommandPalette, Hotbar, PaneFrame, ResizablePane, ResizeEdge};
use wgpui::components::sections::{TerminalLine, TerminalPane, TerminalStream};
use wgpui::renderer::Renderer;
use wgpui::{Bounds, EventContext, Modifiers, Point, TextSystem, theme};
use winit::window::Window;

use crate::apple_fm_bridge::{AppleFmBridgeCommand, AppleFmBridgeSnapshot, AppleFmBridgeWorker};
use crate::bitcoin_display::format_mission_control_amount;
use crate::labor_orchestrator::{
    CodexLaborApprovalEvent, CodexLaborBinding, CodexLaborClaimState, CodexLaborSubmissionState,
    CodexLaborVerdictState, CodexRunClassification,
};
use crate::local_inference_runtime::{
    LocalInferenceExecutionMetrics, LocalInferenceExecutionProvenance,
    LocalInferenceExecutionSnapshot, LocalInferenceRuntime, LocalInferenceRuntimeCommand,
};
use crate::provider_nip90_lane::{
    ProviderNip90AuthIdentity, ProviderNip90LaneCommand, ProviderNip90LaneSnapshot,
    ProviderNip90LaneWorker,
};
use crate::runtime_lanes::{
    AcCreditCommand, AcLaneSnapshot, AcLaneWorker, RuntimeCommandResponse, SaLaneSnapshot,
    SaLaneWorker, SaLifecycleCommand, SklDiscoveryTrustCommand, SklLaneSnapshot, SklLaneWorker,
};
use crate::state::autopilot_goals::GoalLaborLinkage;
use crate::{
    codex_lane::{
        CodexLaneCommand, CodexLaneCommandResponse, CodexLaneNotification, CodexLaneSnapshot,
        CodexLaneWorker,
    },
    spark_wallet::{SparkPaneState, SparkWalletCommand, SparkWalletWorker},
    stablesats_blink_worker::StableSatsBlinkWorker,
};

#[path = "app_state_domains.rs"]
mod app_state_domains;
mod chat_projection;
mod credentials_state;
mod direct_messages;
pub use crate::project_ops::ProjectOpsPaneState;
pub use app_state_domains::*;
pub use chat_projection::*;
pub use credentials_state::CredentialsState;
pub use direct_messages::*;

pub const WINDOW_TITLE: &str = "Autopilot";
pub const WINDOW_WIDTH: f64 = 1280.0;
pub const WINDOW_HEIGHT: f64 = 800.0;

pub struct App {
    pub state: Option<RenderState>,
    pub cursor_position: Point,
}

impl Default for App {
    fn default() -> Self {
        Self {
            state: None,
            cursor_position: Point::ZERO,
        }
    }
}

const PANE_SIZE_MEMORY_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum PaneKind {
    Empty,
    AutopilotChat,
    ProjectOps,
    CodexAccount,
    CodexModels,
    CodexConfig,
    CodexMcp,
    CodexApps,
    CodexLabs,
    CodexDiagnostics,
    GoOnline,
    ProviderStatus,
    LocalInference,
    AppleFmWorkbench,
    EarningsScoreboard,
    RelayConnections,
    SyncHealth,
    NetworkRequests,
    StarterJobs,
    ReciprocalLoop,
    ActivityFeed,
    AlertsRecovery,
    Settings,
    Credentials,
    JobInbox,
    ActiveJob,
    JobHistory,
    BuyModePayments,
    NostrIdentity,
    SparkWallet,
    SparkCreateInvoice,
    SparkPayInvoice,
    AgentProfileState,
    AgentScheduleTick,
    TrajectoryAudit,
    CastControl,
    SkillRegistry,
    SkillTrustRevocation,
    CreditDesk,
    CreditSettlementLedger,
    Calculator,
    CadDemo,
}

#[derive(Clone, Copy)]
pub enum PaneDragMode {
    Moving {
        pane_id: u64,
        start_mouse: Point,
        start_bounds: Bounds,
    },
    Resizing {
        pane_id: u64,
        edge: ResizeEdge,
        start_mouse: Point,
        start_bounds: Bounds,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CadCameraDragMode {
    Orbit,
    Pan,
}

#[derive(Clone, Copy, Debug)]
pub struct CadCameraDragState {
    pub pane_id: u64,
    pub tile_index: usize,
    pub mode: CadCameraDragMode,
    pub last_mouse: Point,
    pub moved: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct ChatTranscriptSelectionDragState {
    pub message_id: u64,
    pub anchor_byte_offset: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct ChatTranscriptSelectionState {
    pub message_id: u64,
    pub start_byte_offset: usize,
    pub end_byte_offset: usize,
}

pub struct DesktopPane {
    pub id: u64,
    pub title: String,
    pub kind: PaneKind,
    pub bounds: Bounds,
    pub z_index: i32,
    pub frame: PaneFrame,
    pub presentation: PanePresentation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PanePresentation {
    Windowed,
    Fullscreen,
}

impl PanePresentation {
    pub const fn uses_window_chrome(self) -> bool {
        matches!(self, Self::Windowed)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PaneSizeMemoryDocumentV1 {
    schema_version: u32,
    panes: Vec<PaneSizeMemoryRecord>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct PaneSizeMemoryRecord {
    kind: PaneKind,
    width: f32,
    height: f32,
}

pub struct PaneSizeMemory {
    file_path: PathBuf,
    panes: Vec<PaneSizeMemoryRecord>,
    dirty: bool,
}

impl Default for PaneSizeMemory {
    fn default() -> Self {
        Self {
            file_path: Self::default_file_path(),
            panes: Vec::new(),
            dirty: false,
        }
    }
}

impl PaneSizeMemory {
    pub fn load_or_default() -> Self {
        Self::load_or_default_at(Self::default_file_path())
    }

    fn load_or_default_at(file_path: PathBuf) -> Self {
        let mut memory = Self {
            file_path,
            panes: Vec::new(),
            dirty: false,
        };
        let raw = match std::fs::read_to_string(memory.file_path.as_path()) {
            Ok(raw) => raw,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return memory,
            Err(_) => return memory,
        };
        let Ok(document) = serde_json::from_str::<PaneSizeMemoryDocumentV1>(&raw) else {
            return memory;
        };
        if document.schema_version != PANE_SIZE_MEMORY_SCHEMA_VERSION {
            return memory;
        }

        for pane in document.panes {
            if !pane.width.is_finite()
                || !pane.height.is_finite()
                || pane.width <= 0.0
                || pane.height <= 0.0
            {
                continue;
            }
            memory.panes.retain(|existing| existing.kind != pane.kind);
            memory.panes.push(pane);
        }
        memory
    }

    fn default_file_path() -> PathBuf {
        std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".openagents")
            .join("autopilot-pane-sizes-v1.json")
    }

    pub fn size_for(&self, kind: PaneKind) -> Option<wgpui::Size> {
        self.panes
            .iter()
            .find(|pane| pane.kind == kind)
            .map(|pane| wgpui::Size::new(pane.width, pane.height))
    }

    pub fn remember(&mut self, kind: PaneKind, size: wgpui::Size) {
        if !size.width.is_finite()
            || !size.height.is_finite()
            || size.width <= 0.0
            || size.height <= 0.0
        {
            return;
        }

        let mut changed = true;
        if let Some(existing) = self.panes.iter_mut().find(|pane| pane.kind == kind) {
            changed = (existing.width - size.width).abs() > f32::EPSILON
                || (existing.height - size.height).abs() > f32::EPSILON;
            if changed {
                existing.width = size.width;
                existing.height = size.height;
            }
        } else {
            self.panes.push(PaneSizeMemoryRecord {
                kind,
                width: size.width,
                height: size.height,
            });
        }

        if changed {
            self.dirty = true;
        }
    }

    pub fn persist_if_dirty(&mut self) -> Result<(), String> {
        if !self.dirty {
            return Ok(());
        }
        if let Some(parent) = self.file_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("Failed to create pane size dir: {error}"))?;
        }
        let document = PaneSizeMemoryDocumentV1 {
            schema_version: PANE_SIZE_MEMORY_SCHEMA_VERSION,
            panes: self.panes.clone(),
        };
        let payload = serde_json::to_string_pretty(&document)
            .map_err(|error| format!("Failed to encode pane sizes: {error}"))?;
        let temp_path = self.file_path.with_extension("tmp");
        std::fs::write(&temp_path, payload)
            .map_err(|error| format!("Failed to write pane size temp file: {error}"))?;
        std::fs::rename(&temp_path, self.file_path.as_path())
            .map_err(|error| format!("Failed to persist pane sizes: {error}"))?;
        self.dirty = false;
        Ok(())
    }
}

#[cfg(test)]
mod pane_size_memory_tests {
    use super::{PaneKind, PaneSizeMemory};
    use std::time::{SystemTime, UNIX_EPOCH};
    use wgpui::Size;

    #[test]
    fn pane_size_memory_round_trips_to_disk() {
        let now_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("autopilot-pane-sizes-{now_nanos}.json"));
        let mut memory = PaneSizeMemory::load_or_default_at(path.clone());
        memory.remember(PaneKind::GoOnline, Size::new(704.0, 436.0));
        memory
            .persist_if_dirty()
            .expect("pane size memory should persist");

        let loaded = PaneSizeMemory::load_or_default_at(path.clone());
        let remembered = loaded
            .size_for(PaneKind::GoOnline)
            .expect("remembered pane size should reload");
        assert_eq!(remembered.width, 704.0);
        assert_eq!(remembered.height, 436.0);

        let _ = std::fs::remove_file(path);
    }
}

pub struct SparkPaneInputs {
    pub invoice_amount: TextInput,
    pub send_request: TextInput,
    pub send_amount: TextInput,
}

impl Default for SparkPaneInputs {
    fn default() -> Self {
        Self {
            invoice_amount: TextInput::new()
                .value("1000")
                .placeholder("Lightning invoice sats"),
            send_request: TextInput::new()
                .placeholder("Lightning invoice / payment request")
                .mono(true),
            send_amount: TextInput::new().placeholder("Send sats (optional)"),
        }
    }
}

pub struct PayInvoicePaneInputs {
    pub payment_request: TextInput,
    pub amount_sats: TextInput,
}

impl Default for PayInvoicePaneInputs {
    fn default() -> Self {
        Self {
            payment_request: TextInput::new()
                .placeholder("Lightning invoice / payment request")
                .mono(true),
            amount_sats: TextInput::new().placeholder("Send sats (optional)"),
        }
    }
}

pub struct CreateInvoicePaneInputs {
    pub amount_sats: TextInput,
    pub description: TextInput,
    pub expiry_seconds: TextInput,
}

impl Default for CreateInvoicePaneInputs {
    fn default() -> Self {
        Self {
            amount_sats: TextInput::new()
                .value("1000")
                .placeholder("Lightning invoice sats"),
            description: TextInput::new().placeholder("Description (optional)"),
            expiry_seconds: TextInput::new().value("3600").placeholder("Expiry seconds"),
        }
    }
}

pub struct RelayConnectionsPaneInputs {
    pub relay_url: TextInput,
}

impl Default for RelayConnectionsPaneInputs {
    fn default() -> Self {
        Self {
            relay_url: TextInput::new()
                .value("wss://relay.example.com")
                .placeholder("wss://relay.example.com"),
        }
    }
}

pub struct NetworkRequestsPaneInputs {
    pub compute_family: TextInput,
    pub preferred_backend: TextInput,
    pub capability_constraints: TextInput,
    pub quantity: TextInput,
    pub delivery_start_minutes: TextInput,
    pub window_minutes: TextInput,
    pub max_price_sats: TextInput,
}

impl Default for NetworkRequestsPaneInputs {
    fn default() -> Self {
        Self {
            compute_family: TextInput::new()
                .value("inference")
                .placeholder("Compute family"),
            preferred_backend: TextInput::new()
                .value("psionic")
                .placeholder("Preferred backend (optional)"),
            capability_constraints: TextInput::new()
                .placeholder("Capability envelope constraints (JSON or key=value list)"),
            quantity: TextInput::new()
                .value("1")
                .placeholder("Requested quantity"),
            delivery_start_minutes: TextInput::new()
                .value("0")
                .placeholder("Delivery start in minutes"),
            window_minutes: TextInput::new()
                .value("15")
                .placeholder("Delivery window minutes"),
            max_price_sats: TextInput::new().value("34").placeholder("Max price sats"),
        }
    }
}

pub struct LocalInferencePaneInputs {
    pub prompt: TextInput,
    pub requested_model: TextInput,
    pub max_tokens: TextInput,
    pub temperature: TextInput,
    pub top_k: TextInput,
    pub top_p: TextInput,
}

impl Default for LocalInferencePaneInputs {
    fn default() -> Self {
        Self {
            prompt: TextInput::new()
                .value("Describe the current GPT-OSS local runtime in one short paragraph.")
                .placeholder("Prompt"),
            requested_model: TextInput::new().placeholder("Optional model override"),
            max_tokens: TextInput::new().value("128").placeholder("Max tokens"),
            temperature: TextInput::new().value("0.2").placeholder("Temperature"),
            top_k: TextInput::new().placeholder("Top-k (optional)"),
            top_p: TextInput::new().placeholder("Top-p (optional)"),
        }
    }
}

pub struct AppleFmWorkbenchPaneInputs {
    pub instructions: TextInput,
    pub prompt: TextInput,
    pub model: TextInput,
    pub session_id: TextInput,
    pub max_tokens: TextInput,
    pub temperature: TextInput,
    pub top: TextInput,
    pub probability_threshold: TextInput,
    pub seed: TextInput,
    pub schema_json: TextInput,
    pub transcript_json: TextInput,
}

impl Default for AppleFmWorkbenchPaneInputs {
    fn default() -> Self {
        Self {
            instructions: TextInput::new()
                .value("You are the OpenAgents Apple FM workbench. Be concise and literal.")
                .placeholder("Optional session instructions"),
            prompt: TextInput::new()
                .value("Say hello from the Apple FM workbench in one short sentence.")
                .placeholder("Prompt to send to Apple FM"),
            model: TextInput::new().placeholder("Optional Apple FM model override"),
            session_id: TextInput::new().placeholder("Bridge session id"),
            max_tokens: TextInput::new().value("128").placeholder("Max tokens"),
            temperature: TextInput::new().value("0.2").placeholder("Temperature"),
            top: TextInput::new().placeholder("Random sampling top-k"),
            probability_threshold: TextInput::new()
                .placeholder("Random sampling probability threshold"),
            seed: TextInput::new().placeholder("Random sampling seed"),
            schema_json: TextInput::new()
                .value(
                    "{\n  \"type\": \"object\",\n  \"properties\": {\n    \"summary\": { \"type\": \"string\" },\n    \"confidence\": { \"type\": \"number\" }\n  },\n  \"required\": [\"summary\", \"confidence\"]\n}",
                )
                .placeholder("Structured-generation JSON schema")
                .mono(true),
            transcript_json: TextInput::new()
                .placeholder("Transcript JSON for export / restore")
                .mono(true),
        }
    }
}

pub struct SettingsPaneInputs {
    pub relay_url: TextInput,
    pub wallet_default_send_sats: TextInput,
    pub provider_max_queue_depth: TextInput,
}

impl Default for SettingsPaneInputs {
    fn default() -> Self {
        Self {
            relay_url: TextInput::new()
                .value("wss://relay.damus.io")
                .placeholder("wss://relay.example.com"),
            wallet_default_send_sats: TextInput::new()
                .value("1000")
                .placeholder("Default send sats"),
            provider_max_queue_depth: TextInput::new()
                .value("4")
                .placeholder("Provider max queue depth"),
        }
    }
}

pub struct CredentialsPaneInputs {
    pub variable_name: TextInput,
    pub variable_value: TextInput,
}

impl Default for CredentialsPaneInputs {
    fn default() -> Self {
        Self {
            variable_name: TextInput::new().placeholder("ENV_VAR_NAME"),
            variable_value: TextInput::new().placeholder("Value (stored in secure keychain)"),
        }
    }
}

pub struct JobHistoryPaneInputs {
    pub search_job_id: TextInput,
}

impl Default for JobHistoryPaneInputs {
    fn default() -> Self {
        Self {
            search_job_id: TextInput::new().placeholder("Search job id"),
        }
    }
}

pub struct ChatPaneInputs {
    pub composer: TextInput,
    pub thread_search: TextInput,
}

impl Default for ChatPaneInputs {
    fn default() -> Self {
        Self {
            composer: TextInput::new()
                .placeholder("Write a message, ask for analysis, or command your Autopilot...")
                .border_color_focused(theme::border::FOCUS),
            thread_search: TextInput::new()
                .placeholder("Filter thread history...")
                .border_color_focused(theme::border::FOCUS),
        }
    }
}

pub struct CalculatorPaneInputs {
    pub expression: TextInput,
}

impl Default for CalculatorPaneInputs {
    fn default() -> Self {
        Self {
            expression: TextInput::new()
                .placeholder("e.g. (8 + 2) * 3 - 4 / 2")
                .mono(true),
        }
    }
}

pub struct MissionControlPaneState {
    pub log_stream: TerminalPane,
    pub load_funds_amount_sats: TextInput,
    pub send_invoice: TextInput,
    pub withdraw_invoice: TextInput,
    pub buy_mode_loop_enabled: bool,
    pub buy_mode_next_dispatch_at: Option<Instant>,
    pub buy_mode_last_dispatch_at: Option<Instant>,
    pub local_fm_summary_pending_request_id: Option<String>,
    pub local_fm_summary_text: String,
    pub last_action: Option<String>,
    pub last_error: Option<String>,
    wallet_refresh_icon_clicked_at_epoch_ms: u64,
    log_copy_icon_clicked_at_epoch_ms: u64,
    sell_scroll_offset_px: f32,
    earnings_scroll_offset_px: f32,
    wallet_scroll_offset_px: f32,
    actions_scroll_offset_px: f32,
    load_funds_scroll_offset_px: f32,
    active_jobs_scroll_offset_px: f32,
    /// Logical (stream, message) for equality check; display lines include HH:MM:SS prefix.
    rendered_log_content: Vec<(TerminalStream, String)>,
    last_mirrored_trace_id: u64,
}

impl Default for MissionControlPaneState {
    fn default() -> Self {
        Self {
            log_stream: TerminalPane::new()
                .title("\\ LOG STREAM")
                .show_frame(false)
                .code_block_style(true),
            load_funds_amount_sats: TextInput::new()
                .value("1000")
                .placeholder("Lightning sats")
                .font_size(wgpui::theme::font_size::SM - 2.0)
                .mono(true)
                .background(wgpui::theme::bg::APP)
                .border_color(wgpui::Hsla::from_hex(0x5A4730))
                .border_color_focused(wgpui::Hsla::from_hex(0xFF6A00))
                .text_color(wgpui::Hsla::from_hex(0xE8E3D7))
                .placeholder_color(wgpui::Hsla::from_hex(0x7F776D)),
            send_invoice: TextInput::new()
                .placeholder("Paste Lightning invoice to send")
                .font_size(wgpui::theme::font_size::SM - 2.0)
                .mono(true)
                .background(wgpui::theme::bg::APP)
                .border_color(wgpui::Hsla::from_hex(0x5A4730))
                .border_color_focused(wgpui::Hsla::from_hex(0xFF6A00))
                .text_color(wgpui::Hsla::from_hex(0xE8E3D7))
                .placeholder_color(wgpui::Hsla::from_hex(0x7F776D)),
            withdraw_invoice: TextInput::new()
                .placeholder("Paste Lightning invoice to withdraw")
                .font_size(wgpui::theme::font_size::SM - 2.0)
                .mono(true)
                .background(wgpui::theme::bg::APP)
                .border_color(wgpui::Hsla::from_hex(0x5A4730))
                .border_color_focused(wgpui::Hsla::from_hex(0xFF6A00))
                .text_color(wgpui::Hsla::from_hex(0xE8E3D7))
                .placeholder_color(wgpui::Hsla::from_hex(0x7F776D)),
            buy_mode_loop_enabled: false,
            buy_mode_next_dispatch_at: None,
            buy_mode_last_dispatch_at: None,
            local_fm_summary_pending_request_id: None,
            local_fm_summary_text: String::new(),
            last_action: Some("Mission Control ready".to_string()),
            last_error: None,
            wallet_refresh_icon_clicked_at_epoch_ms: 0,
            log_copy_icon_clicked_at_epoch_ms: 0,
            sell_scroll_offset_px: 0.0,
            earnings_scroll_offset_px: 0.0,
            wallet_scroll_offset_px: 0.0,
            actions_scroll_offset_px: 0.0,
            load_funds_scroll_offset_px: 0.0,
            active_jobs_scroll_offset_px: 0.0,
            rendered_log_content: Vec::new(),
            last_mirrored_trace_id: 0,
        }
    }
}

impl MissionControlPaneState {
    const ICON_CLICK_FEEDBACK_DURATION_MS: u64 = 650;

    fn icon_click_feedback_intensity(clicked_at_epoch_ms: u64, now_epoch_ms: u64) -> f32 {
        if clicked_at_epoch_ms == 0 {
            return 0.0;
        }
        let elapsed = now_epoch_ms.saturating_sub(clicked_at_epoch_ms);
        if elapsed >= Self::ICON_CLICK_FEEDBACK_DURATION_MS {
            0.0
        } else {
            1.0 - (elapsed as f32 / Self::ICON_CLICK_FEEDBACK_DURATION_MS as f32)
        }
    }

    pub fn mark_wallet_refresh_icon_clicked(&mut self) {
        self.wallet_refresh_icon_clicked_at_epoch_ms = current_epoch_millis_for_state();
    }

    pub fn mark_log_copy_icon_clicked(&mut self) {
        self.log_copy_icon_clicked_at_epoch_ms = current_epoch_millis_for_state();
    }

    pub fn wallet_refresh_icon_click_feedback(&self, now_epoch_ms: u64) -> f32 {
        Self::icon_click_feedback_intensity(self.wallet_refresh_icon_clicked_at_epoch_ms, now_epoch_ms)
    }

    pub fn log_copy_icon_click_feedback(&self, now_epoch_ms: u64) -> f32 {
        Self::icon_click_feedback_intensity(self.log_copy_icon_clicked_at_epoch_ms, now_epoch_ms)
    }

    fn clamp_scroll_offset(offset: &mut f32, max_scroll: f32) -> f32 {
        let clamped = offset.clamp(0.0, max_scroll.max(0.0));
        *offset = clamped;
        clamped
    }

    pub fn scroll_sell_by(&mut self, dy: f32) {
        self.sell_scroll_offset_px = (self.sell_scroll_offset_px + dy).max(0.0);
    }

    pub fn scroll_earnings_by(&mut self, dy: f32) {
        self.earnings_scroll_offset_px = (self.earnings_scroll_offset_px + dy).max(0.0);
    }

    pub fn scroll_wallet_by(&mut self, dy: f32) {
        self.wallet_scroll_offset_px = (self.wallet_scroll_offset_px + dy).max(0.0);
    }

    pub fn scroll_actions_by(&mut self, dy: f32) {
        self.actions_scroll_offset_px = (self.actions_scroll_offset_px + dy).max(0.0);
    }

    pub fn scroll_load_funds_by(&mut self, dy: f32) {
        self.load_funds_scroll_offset_px = (self.load_funds_scroll_offset_px + dy).max(0.0);
    }

    pub fn scroll_active_jobs_by(&mut self, dy: f32) {
        self.active_jobs_scroll_offset_px = (self.active_jobs_scroll_offset_px + dy).max(0.0);
    }

    pub fn clamp_sell_scroll_offset(&mut self, max_scroll: f32) -> f32 {
        Self::clamp_scroll_offset(&mut self.sell_scroll_offset_px, max_scroll)
    }

    pub fn clamp_earnings_scroll_offset(&mut self, max_scroll: f32) -> f32 {
        Self::clamp_scroll_offset(&mut self.earnings_scroll_offset_px, max_scroll)
    }

    pub fn clamp_wallet_scroll_offset(&mut self, max_scroll: f32) -> f32 {
        Self::clamp_scroll_offset(&mut self.wallet_scroll_offset_px, max_scroll)
    }

    pub fn clamp_actions_scroll_offset(&mut self, max_scroll: f32) -> f32 {
        Self::clamp_scroll_offset(&mut self.actions_scroll_offset_px, max_scroll)
    }

    pub fn clamp_load_funds_scroll_offset(&mut self, max_scroll: f32) -> f32 {
        Self::clamp_scroll_offset(&mut self.load_funds_scroll_offset_px, max_scroll)
    }

    pub fn clamp_active_jobs_scroll_offset(&mut self, max_scroll: f32) -> f32 {
        Self::clamp_scroll_offset(&mut self.active_jobs_scroll_offset_px, max_scroll)
    }

    pub fn actions_scroll_offset(&self) -> f32 {
        self.actions_scroll_offset_px
    }

    pub fn load_funds_scroll_offset(&self) -> f32 {
        self.load_funds_scroll_offset_px
    }

    fn push_persisted_log_line(&mut self, line: TerminalLine) {
        crate::runtime_log::record_mission_control_line(
            line.stream.clone(),
            line.text.clone(),
            line.key.as_deref(),
        );
        self.log_stream.push_line(line);
    }

    fn build_runtime_terminal_line(
        &self,
        stream: TerminalStream,
        text: impl Into<String>,
        key: Option<String>,
    ) -> TerminalLine {
        let line = TerminalLine::new(
            stream,
            format!(
                "{}  {}",
                mission_control_log_timestamp(now_epoch_seconds()),
                text.into()
            ),
        );
        if let Some(key) = key {
            line.with_key(key)
        } else {
            line
        }
    }

    pub fn toggle_buy_mode_loop(&mut self, now: Instant) -> bool {
        self.buy_mode_loop_enabled = !self.buy_mode_loop_enabled;
        if self.buy_mode_loop_enabled {
            self.buy_mode_next_dispatch_at = Some(now);
        } else {
            self.buy_mode_next_dispatch_at = None;
        }
        self.buy_mode_loop_enabled
    }

    pub fn buy_mode_dispatch_due(&self, now: Instant) -> bool {
        self.buy_mode_loop_enabled
            && self
                .buy_mode_next_dispatch_at
                .map(|next_due_at| now >= next_due_at)
                .unwrap_or(true)
    }

    pub fn schedule_next_buy_mode_dispatch(&mut self, now: Instant) {
        self.buy_mode_last_dispatch_at = Some(now);
        self.buy_mode_next_dispatch_at = Some(now + MISSION_CONTROL_BUY_MODE_INTERVAL);
    }

    pub fn schedule_buy_mode_retry(&mut self, now: Instant) {
        self.buy_mode_next_dispatch_at = Some(now + MISSION_CONTROL_BUY_MODE_INTERVAL);
    }

    pub fn buy_mode_next_dispatch_countdown_millis(&self, now: Instant) -> Option<u64> {
        let next_due_at = self.buy_mode_next_dispatch_at?;
        if now >= next_due_at {
            return Some(0);
        }
        let wait = next_due_at.saturating_duration_since(now);
        Some(wait.as_millis().min(u128::from(u64::MAX)) as u64)
    }

    pub fn buy_mode_next_dispatch_countdown_seconds(&self, now: Instant) -> Option<u64> {
        let wait_millis = self.buy_mode_next_dispatch_countdown_millis(now)?;
        if wait_millis == 0 {
            return Some(0);
        }
        Some(wait_millis.saturating_add(999) / 1_000)
    }

    pub fn buy_mode_next_dispatch_countdown_label(&self, now: Instant) -> Option<String> {
        let wait_millis = self.buy_mode_next_dispatch_countdown_millis(now)?;
        Some(if wait_millis == 0 {
            "now".to_string()
        } else if wait_millis < 1_000 {
            format!("{wait_millis}ms")
        } else {
            let seconds = wait_millis.saturating_add(999) / 1_000;
            format!("{seconds}s")
        })
    }

    pub fn record_action(&mut self, action: impl Into<String>) {
        self.last_action = Some(action.into());
        self.last_error = None;
    }

    pub fn record_error(&mut self, error: impl Into<String>) {
        self.last_error = Some(error.into());
    }

    pub fn begin_local_fm_summary(&mut self, request_id: impl Into<String>, detail: &str) {
        let request_id = request_id.into();
        self.local_fm_summary_pending_request_id = Some(request_id.clone());
        self.local_fm_summary_text.clear();
        self.push_runtime_log_line(
            TerminalStream::Stdout,
            format!("Local FM summary queued [{request_id}] // {detail}"),
        );
    }

    pub fn local_fm_summary_is_pending(&self) -> bool {
        self.local_fm_summary_pending_request_id.is_some()
    }

    pub fn push_runtime_log_line(&mut self, stream: TerminalStream, text: impl Into<String>) {
        self.push_persisted_log_line(self.build_runtime_terminal_line(stream, text, None));
    }

    pub fn upsert_runtime_log_line(
        &mut self,
        key: impl Into<String>,
        stream: TerminalStream,
        text: impl Into<String>,
    ) {
        self.push_persisted_log_line(self.build_runtime_terminal_line(
            stream,
            text,
            Some(key.into()),
        ));
    }

    pub fn has_pending_mirrored_trace_logs(&self) -> bool {
        crate::logging::latest_mirrored_log_id() > self.last_mirrored_trace_id
    }

    pub fn sync_log_stream(
        &mut self,
        desktop_shell_mode: crate::desktop_shell::DesktopShellMode,
        provider_runtime: &crate::state::provider_runtime::ProviderRuntimeState,
        local_inference_runtime: &crate::local_inference_runtime::LocalInferenceExecutionSnapshot,
        provider_blockers: &[crate::state::provider_runtime::ProviderBlocker],
        earn_job_lifecycle_projection: &EarnJobLifecycleProjectionState,
        spark_wallet: &SparkPaneState,
        network_requests: &NetworkRequestsState,
        job_inbox: &JobInboxState,
        active_job: &ActiveJobState,
    ) {
        let (lines, content) = build_mission_control_log_lines(
            self.last_action.as_deref(),
            self.last_error.as_deref(),
            desktop_shell_mode,
            provider_runtime,
            local_inference_runtime,
            provider_blockers,
            earn_job_lifecycle_projection,
            spark_wallet,
            network_requests,
            job_inbox,
            active_job,
        );
        for (line, entry) in lines.into_iter().zip(content.into_iter()) {
            if !self.rendered_log_content.contains(&entry) {
                self.push_persisted_log_line(line);
                self.rendered_log_content.push(entry);
            }
        }
        for entry in crate::logging::mirrored_logs_after(self.last_mirrored_trace_id) {
            let stream = match entry.level {
                tracing::Level::ERROR | tracing::Level::WARN => TerminalStream::Stderr,
                tracing::Level::INFO | tracing::Level::DEBUG | tracing::Level::TRACE => {
                    TerminalStream::Stdout
                }
            };
            self.log_stream.push_line(TerminalLine::new(
                stream,
                format!(
                    "{}  {}",
                    mission_control_log_timestamp(entry.at_epoch_seconds),
                    entry.line
                ),
            ));
            self.last_mirrored_trace_id = entry.id;
        }
    }
}

pub struct BuyModePaymentsPaneState {
    pub ledger: TerminalPane,
    rendered_rows: Vec<(TerminalStream, String)>,
}

impl Default for BuyModePaymentsPaneState {
    fn default() -> Self {
        Self {
            ledger: TerminalPane::new()
                .title("\\ BUY MODE PAYMENTS")
                .show_frame(false)
                .code_block_style(true),
            rendered_rows: Vec::new(),
        }
    }
}

impl BuyModePaymentsPaneState {
    pub fn sync_rows(
        &mut self,
        network_requests: &crate::state::operations::NetworkRequestsState,
        spark_wallet: &SparkPaneState,
    ) {
        let rows = build_buy_mode_payment_rows(network_requests, spark_wallet);
        if self.rendered_rows == rows {
            return;
        }

        self.ledger.clear();
        for (stream, text) in &rows {
            self.ledger
                .push_line(TerminalLine::new(stream.clone(), text.clone()));
        }
        self.rendered_rows = rows;
    }
}

fn buy_mode_payment_history_requests(
    network_requests: &crate::state::operations::NetworkRequestsState,
) -> Vec<&crate::state::operations::SubmittedNetworkRequest> {
    network_requests
        .submitted
        .iter()
        .rev()
        .filter(|request| request.request_type == MISSION_CONTROL_BUY_MODE_REQUEST_TYPE)
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BuyModePaymentLedgerEntry {
    timestamp: Option<u64>,
    sort_epoch_seconds: u64,
    stream: TerminalStream,
    status: String,
    amount_sats: u64,
    fees_sats: Option<u64>,
    total_debit_sats: Option<u64>,
    wallet_status: String,
    wallet_method: String,
    provider_pubkey: String,
    request_id: String,
    payment_pointer: String,
    request_event_id: String,
    result_event_id: String,
    payment_hash: String,
    destination_pubkey: String,
    htlc_status: String,
    htlc_expiry_epoch_seconds: Option<u64>,
    wallet_detail: Option<String>,
    wallet_description: Option<String>,
    wallet_invoice: Option<String>,
    pending_bolt11: Option<String>,
    payment_error: Option<String>,
    payment_notice: Option<String>,
    source: &'static str,
}

fn buy_mode_payment_ledger_entries(
    network_requests: &crate::state::operations::NetworkRequestsState,
    spark_wallet: &SparkPaneState,
) -> Vec<BuyModePaymentLedgerEntry> {
    let mut matched_payment_pointers = HashSet::<String>::new();
    let mut entries = Vec::<BuyModePaymentLedgerEntry>::new();

    for (index, request) in buy_mode_payment_history_requests(network_requests)
        .into_iter()
        .enumerate()
    {
        let wallet_payment = buy_mode_wallet_payment(request, spark_wallet);
        if let Some(payment) = wallet_payment {
            matched_payment_pointers.insert(payment.id.clone());
        }
        entries.push(buy_mode_request_ledger_entry(
            index,
            request,
            wallet_payment,
        ));
    }

    for payment in &spark_wallet.recent_payments {
        if matched_payment_pointers.contains(payment.id.as_str()) {
            continue;
        }
        if let Some(entry) = buy_mode_wallet_backfill_entry(payment) {
            entries.push(entry);
        }
    }

    entries.sort_by(|left, right| {
        right
            .sort_epoch_seconds
            .cmp(&left.sort_epoch_seconds)
            .then_with(|| right.payment_pointer.cmp(&left.payment_pointer))
    });
    entries
}

fn buy_mode_request_ledger_entry(
    index: usize,
    request: &crate::state::operations::SubmittedNetworkRequest,
    wallet_payment: Option<&openagents_spark::PaymentSummary>,
) -> BuyModePaymentLedgerEntry {
    let timestamp = wallet_payment
        .map(|payment| payment.timestamp)
        .or(request.payment_sent_at_epoch_seconds)
        .or(request.payment_failed_at_epoch_seconds)
        .or(request.payment_required_at_epoch_seconds);
    let amount_sats = wallet_payment
        .map(|payment| payment.amount_sats)
        .unwrap_or(request.budget_sats);
    let provider_pubkey = request
        .winning_provider_pubkey
        .as_deref()
        .or(request.last_provider_pubkey.as_deref())
        .unwrap_or("-")
        .to_string();
    let wallet_status = wallet_payment
        .map(|payment| buy_mode_wallet_state_label(request, Some(payment)))
        .unwrap_or_else(|| buy_mode_wallet_state_label(request, None));
    let request_event_id = request
        .published_request_event_id
        .as_deref()
        .unwrap_or("-")
        .to_string();
    let result_event_id = request
        .winning_result_event_id
        .as_deref()
        .or(request.last_result_event_id.as_deref())
        .unwrap_or("-")
        .to_string();
    let payment_pointer = request
        .last_payment_pointer
        .as_deref()
        .unwrap_or("-")
        .to_string();
    let stream = if request.status == crate::state::operations::NetworkRequestStatus::Failed
        || request.payment_failed_at_epoch_seconds.is_some()
        || request.payment_error.is_some()
    {
        TerminalStream::Stderr
    } else {
        TerminalStream::Stdout
    };
    BuyModePaymentLedgerEntry {
        timestamp,
        sort_epoch_seconds: timestamp.unwrap_or(u64::MAX.saturating_sub(index as u64)),
        stream,
        status: request.status.label().to_string(),
        amount_sats,
        fees_sats: wallet_payment.map(|payment| payment.fees_sats),
        total_debit_sats: wallet_payment.map(crate::spark_wallet::wallet_payment_total_debit_sats),
        wallet_status,
        wallet_method: wallet_payment
            .map(|payment| payment.method.clone())
            .unwrap_or_else(|| "-".to_string()),
        provider_pubkey,
        request_id: request.request_id.clone(),
        payment_pointer,
        request_event_id,
        result_event_id,
        payment_hash: wallet_payment
            .and_then(|payment| payment.payment_hash.clone())
            .unwrap_or_else(|| "-".to_string()),
        destination_pubkey: wallet_payment
            .and_then(|payment| payment.destination_pubkey.clone())
            .unwrap_or_else(|| "-".to_string()),
        htlc_status: wallet_payment
            .and_then(|payment| payment.htlc_status.clone())
            .unwrap_or_else(|| "-".to_string()),
        htlc_expiry_epoch_seconds: wallet_payment
            .and_then(|payment| payment.htlc_expiry_epoch_seconds),
        wallet_detail: wallet_payment.and_then(|payment| payment.status_detail.clone()),
        wallet_description: wallet_payment.and_then(|payment| payment.description.clone()),
        wallet_invoice: wallet_payment
            .and_then(|payment| payment.invoice.as_deref().map(compact_payment_invoice)),
        pending_bolt11: request
            .pending_bolt11
            .as_deref()
            .map(compact_payment_invoice),
        payment_error: request.payment_error.clone(),
        payment_notice: request.payment_notice.clone(),
        source: "request",
    }
}

fn buy_mode_wallet_backfill_entry(
    payment: &openagents_spark::PaymentSummary,
) -> Option<BuyModePaymentLedgerEntry> {
    if !buy_mode_wallet_backfill_candidate(payment) {
        return None;
    }

    let wallet_status = buy_mode_wallet_status_for_wallet_backfill(payment).to_string();
    let stream = if matches!(wallet_status.as_str(), "failed" | "returned") {
        TerminalStream::Stderr
    } else {
        TerminalStream::Stdout
    };
    let request_hint = buy_mode_wallet_request_hint(payment)
        .map(|hint| format!("wallet-inferred:{hint}"))
        .unwrap_or_else(|| "wallet-inferred".to_string());

    Some(BuyModePaymentLedgerEntry {
        timestamp: Some(payment.timestamp),
        sort_epoch_seconds: payment.timestamp,
        stream,
        status: "wallet-backfill".to_string(),
        amount_sats: payment.amount_sats,
        fees_sats: Some(payment.fees_sats),
        total_debit_sats: Some(crate::spark_wallet::wallet_payment_total_debit_sats(
            payment,
        )),
        wallet_status,
        wallet_method: payment.method.clone(),
        provider_pubkey: payment
            .destination_pubkey
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        request_id: request_hint,
        payment_pointer: payment.id.clone(),
        request_event_id: "-".to_string(),
        result_event_id: "-".to_string(),
        payment_hash: payment
            .payment_hash
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        destination_pubkey: payment
            .destination_pubkey
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        htlc_status: payment
            .htlc_status
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        htlc_expiry_epoch_seconds: payment.htlc_expiry_epoch_seconds,
        wallet_detail: payment.status_detail.clone(),
        wallet_description: payment.description.clone(),
        wallet_invoice: payment.invoice.as_deref().map(compact_payment_invoice),
        pending_bolt11: None,
        payment_error: None,
        payment_notice: None,
        source: "wallet-backfill",
    })
}

fn buy_mode_wallet_backfill_candidate(payment: &openagents_spark::PaymentSummary) -> bool {
    payment.direction.eq_ignore_ascii_case("send")
        && payment.description.as_deref().is_some_and(|description| {
            description
                .trim()
                .to_ascii_lowercase()
                .starts_with("dvm textgen")
        })
}

fn buy_mode_wallet_request_hint(payment: &openagents_spark::PaymentSummary) -> Option<String> {
    let description = payment.description.as_deref()?.trim();
    let candidate = description.split_whitespace().last()?;
    if candidate.eq_ignore_ascii_case("textgen") {
        return None;
    }
    let trimmed = candidate.trim_matches(|ch: char| !ch.is_ascii_hexdigit());
    if trimmed.is_empty() || !trimmed.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }
    Some(trimmed.to_string())
}

fn buy_mode_wallet_status_for_wallet_backfill(
    payment: &openagents_spark::PaymentSummary,
) -> &'static str {
    if payment.is_returned_htlc_failure() {
        "returned"
    } else if crate::spark_wallet::is_settled_wallet_payment_status(payment.status.as_str()) {
        "sent"
    } else if crate::spark_wallet::is_terminal_wallet_payment_status(payment.status.as_str()) {
        "failed"
    } else {
        "pending"
    }
}

pub(crate) fn buy_mode_payments_summary_text(
    network_requests: &crate::state::operations::NetworkRequestsState,
    spark_wallet: &SparkPaneState,
) -> String {
    crate::nip90_compute_flow::buy_mode_payments_summary_text(network_requests, spark_wallet)
}

pub(crate) fn buy_mode_payments_status_lines(
    mission_control: &MissionControlPaneState,
    network_requests: &crate::state::operations::NetworkRequestsState,
    now: Instant,
) -> Vec<String> {
    crate::nip90_compute_flow::buy_mode_payments_status_lines(
        mission_control,
        network_requests,
        &SparkPaneState::default(),
        now,
    )
}

pub(crate) fn buy_mode_payments_clipboard_text(
    mission_control: &MissionControlPaneState,
    network_requests: &crate::state::operations::NetworkRequestsState,
    spark_wallet: &SparkPaneState,
) -> String {
    crate::nip90_compute_flow::buy_mode_payments_clipboard_text(
        mission_control,
        network_requests,
        spark_wallet,
    )
}

fn build_buy_mode_payment_rows(
    network_requests: &crate::state::operations::NetworkRequestsState,
    spark_wallet: &SparkPaneState,
) -> Vec<(TerminalStream, String)> {
    crate::nip90_compute_flow::build_buy_mode_payment_rows(network_requests, spark_wallet)
}

fn push_buy_mode_payment_entry_rows(
    rows: &mut Vec<(TerminalStream, String)>,
    entry: &BuyModePaymentLedgerEntry,
) {
    rows.push((
        entry.stream.clone(),
        format!(
            "{}  status={}  amount={} sats  fee={}  total_debit={}  wallet_status={}  wallet_method={}  provider_pubkey={}",
            buy_mode_payment_timestamp_label(entry.timestamp),
            entry.status,
            entry.amount_sats,
            buy_mode_optional_sats_label(entry.fees_sats),
            buy_mode_optional_sats_label(entry.total_debit_sats),
            entry.wallet_status,
            entry.wallet_method,
            entry.provider_pubkey,
        ),
    ));
    rows.push((
        entry.stream.clone(),
        format!(
            "request_id={}  payment_pointer={}  request_event_id={}  result_event_id={}  payment_hash={}  source={}",
            entry.request_id,
            entry.payment_pointer,
            entry.request_event_id,
            entry.result_event_id,
            entry.payment_hash,
            entry.source,
        ),
    ));
    if entry.destination_pubkey != "-"
        || entry.htlc_status != "-"
        || entry.htlc_expiry_epoch_seconds.is_some()
    {
        rows.push((
            entry.stream.clone(),
            format!(
                "destination_pubkey={}  htlc_status={}  htlc_expiry={}",
                entry.destination_pubkey,
                entry.htlc_status,
                buy_mode_payment_timestamp_label(entry.htlc_expiry_epoch_seconds),
            ),
        ));
    }
    if let Some(detail) = entry.wallet_detail.as_deref() {
        rows.push((entry.stream.clone(), format!("wallet_detail={detail}")));
    }
    if let Some(description) = entry.wallet_description.as_deref() {
        rows.push((
            TerminalStream::Stdout,
            format!("wallet_description={description}"),
        ));
    }
    if let Some(invoice) = entry.wallet_invoice.as_deref() {
        rows.push((TerminalStream::Stdout, format!("wallet_invoice={invoice}")));
    }
    if let Some(invoice) = entry.pending_bolt11.as_deref() {
        rows.push((TerminalStream::Stdout, format!("pending_bolt11={invoice}")));
    }
    if let Some(error) = entry.payment_error.as_deref() {
        rows.push((TerminalStream::Stderr, format!("payment_error={error}")));
    }
    if let Some(notice) = entry.payment_notice.as_deref() {
        rows.push((TerminalStream::Stderr, format!("payment_notice={notice}")));
    }
    rows.push((TerminalStream::Stdout, String::new()));
}

fn compact_buy_mode_request_id(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() <= 18 {
        trimmed.to_string()
    } else {
        format!("{}..{}", &trimmed[..12], &trimmed[trimmed.len() - 6..])
    }
}

fn buy_mode_payment_timestamp_label(epoch_seconds: Option<u64>) -> String {
    epoch_seconds
        .and_then(|value| Local.timestamp_opt(value as i64, 0).single())
        .map(|value| value.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "timestamp=-".to_string())
}

fn buy_mode_optional_sats_label(value: Option<u64>) -> String {
    value
        .map(|amount| format!("{amount} sats"))
        .unwrap_or_else(|| "-".to_string())
}

pub(crate) fn buy_mode_wallet_payment<'a>(
    request: &crate::state::operations::SubmittedNetworkRequest,
    spark_wallet: &'a SparkPaneState,
) -> Option<&'a openagents_spark::PaymentSummary> {
    crate::nip90_compute_flow::buy_mode_wallet_payment(request, spark_wallet)
}

pub(crate) fn buy_mode_wallet_state_label(
    request: &crate::state::operations::SubmittedNetworkRequest,
    wallet_payment: Option<&openagents_spark::PaymentSummary>,
) -> String {
    crate::nip90_compute_flow::buy_mode_wallet_state_label(request, wallet_payment)
}

pub(crate) fn compact_payment_invoice(invoice: &str) -> String {
    crate::nip90_compute_flow::compact_payment_invoice(invoice)
}

fn mission_control_log_timestamp(epoch_secs: u64) -> String {
    Local
        .timestamp_opt(epoch_secs as i64, 0)
        .single()
        .map(|t| t.format("%H:%M:%S").to_string())
        .unwrap_or_else(|| Local::now().format("%H:%M:%S").to_string())
}

fn now_epoch_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn build_mission_control_log_lines(
    mission_action: Option<&str>,
    mission_error: Option<&str>,
    desktop_shell_mode: crate::desktop_shell::DesktopShellMode,
    provider_runtime: &crate::state::provider_runtime::ProviderRuntimeState,
    local_inference_runtime: &crate::local_inference_runtime::LocalInferenceExecutionSnapshot,
    provider_blockers: &[crate::state::provider_runtime::ProviderBlocker],
    earn_job_lifecycle_projection: &EarnJobLifecycleProjectionState,
    spark_wallet: &SparkPaneState,
    network_requests: &NetworkRequestsState,
    job_inbox: &JobInboxState,
    active_job: &ActiveJobState,
) -> (Vec<TerminalLine>, Vec<(TerminalStream, String)>) {
    let now_epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let unsupported_sell_platform_offline = provider_runtime.mode
        == crate::state::provider_runtime::ProviderMode::Offline
        && !mission_control_sell_compute_supported(desktop_shell_mode, local_inference_runtime);
    type LogEntry = (u64, TerminalStream, String);
    let mut entries: Vec<LogEntry> = Vec::new();
    let mut seen = HashSet::<String>::new();
    let mut push_entry = |stream: TerminalStream, text: String, at_epoch: Option<u64>| {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return;
        }
        if seen.insert(trimmed.to_string()) {
            entries.push((at_epoch.unwrap_or(now_epoch), stream, trimmed.to_string()));
        }
    };
    let compute_flow_snapshot = crate::nip90_compute_flow::build_nip90_compute_flow_snapshot(
        network_requests,
        spark_wallet,
        active_job,
        earn_job_lifecycle_projection,
    );

    let mode_line = match provider_runtime.mode {
        crate::state::provider_runtime::ProviderMode::Offline => {
            if mission_control_sell_compute_supported(desktop_shell_mode, local_inference_runtime) {
                "Provider offline. Click GO ONLINE to accept jobs.".to_string()
            } else {
                "Provider offline. Platform not supported for selling compute.".to_string()
            }
        }
        crate::state::provider_runtime::ProviderMode::Connecting => {
            "Provider connecting to relays and preparing runtime.".to_string()
        }
        crate::state::provider_runtime::ProviderMode::Online => {
            "Provider online. Heartbeat and relay intake are active.".to_string()
        }
        crate::state::provider_runtime::ProviderMode::Degraded => {
            "Provider degraded. Review blockers and wallet or relay health.".to_string()
        }
    };
    push_entry(TerminalStream::Stdout, mode_line, None);

    if !unsupported_sell_platform_offline {
        if provider_blockers.is_empty() {
            push_entry(TerminalStream::Stdout, "Preflight clear.".to_string(), None);
        } else {
            for blocker in provider_blockers.iter().take(3) {
                push_entry(
                    TerminalStream::Stderr,
                    format!(
                        "Preflight blocker [{}]: {}",
                        blocker.code(),
                        blocker.detail()
                    ),
                    None,
                );
            }
        }
    }

    let (model_status_stream, model_status) = if unsupported_sell_platform_offline {
        (TerminalStream::Stdout, String::new())
    } else {
        match mission_control_local_runtime_lane(desktop_shell_mode, local_inference_runtime) {
            Some(MissionControlLocalRuntimeLane::AppleFoundationModels) => {
                if provider_runtime.apple_fm.is_ready() {
                    (
                        TerminalStream::Stdout,
                        format!(
                            "Apple Foundation Models ready via Swift bridge ({}).",
                            provider_runtime
                                .apple_fm
                                .ready_model
                                .as_deref()
                                .unwrap_or("apple-foundation-model")
                        ),
                    )
                } else if let Some(message) = provider_runtime
                    .apple_fm
                    .availability_error_message()
                    .as_deref()
                {
                    (
                        TerminalStream::Stderr,
                        format!("Apple Foundation Models unavailable: {message}"),
                    )
                } else if provider_runtime.apple_fm.reachable {
                    (
                        TerminalStream::Stdout,
                        "Apple Foundation Models bridge reachable but not ready yet. Enable Apple Intelligence: System Settings → Apple Intelligence (sidebar) → turn on Apple Intelligence.".to_string(),
                    )
                } else {
                    (
                        TerminalStream::Stderr,
                        "Apple Foundation Models bridge is not running.".to_string(),
                    )
                }
            }
            Some(MissionControlLocalRuntimeLane::NvidiaGptOss) => {
                if local_inference_runtime.is_ready() {
                    (
                        TerminalStream::Stdout,
                        "NVIDIA local model ready. Manage GPT-OSS in the separate workbench pane."
                            .to_string(),
                    )
                } else {
                    (
                        TerminalStream::Stdout,
                        "Open the separate GPT-OSS workbench to load and validate the NVIDIA local model."
                            .to_string(),
                    )
                }
            }
            None => (
                TerminalStream::Stderr,
                "Mission Control has no supported local runtime. Apple Foundation Models is required for the release path."
                    .to_string(),
            ),
        }
    };
    push_entry(model_status_stream, model_status, None);

    if let Some(action) = mission_action {
        push_entry(TerminalStream::Stdout, format!("UI: {action}"), None);
    }
    if let Some(error) = mission_error {
        push_entry(TerminalStream::Stderr, format!("UI error: {error}"), None);
    }
    if let Some(result) = provider_runtime.last_result.as_deref() {
        if unsupported_sell_platform_offline && result.starts_with("Relay preview active") {
            push_entry(
                TerminalStream::Stdout,
                result.replacen("Relay preview", "Buyer relays", 1),
                None,
            );
        } else if unsupported_sell_platform_offline
            && (result.starts_with("Buyer relay transport")
                || result.starts_with("Buyer response relay tracking"))
        {
            push_entry(TerminalStream::Stdout, result.to_string(), None);
        } else if !unsupported_sell_platform_offline {
            push_entry(TerminalStream::Stdout, format!("Provider: {result}"), None);
        }
    }
    if let Some(error) = provider_runtime.last_error_detail.as_deref() {
        push_entry(
            TerminalStream::Stderr,
            format!("Provider error: {error}"),
            None,
        );
    }
    if let Some(action) = provider_runtime.inventory_last_action.as_deref() {
        push_entry(TerminalStream::Stdout, format!("Inventory: {action}"), None);
    }
    if let Some(error) = provider_runtime.inventory_last_error.as_deref() {
        push_entry(
            TerminalStream::Stderr,
            format!("Inventory error: {error}"),
            None,
        );
    }

    if let Some(action) = provider_runtime.apple_fm.last_action.as_deref() {
        push_entry(TerminalStream::Stdout, format!("Apple FM: {action}"), None);
    }
    if let Some(error) = provider_runtime.apple_fm.last_error.as_deref() {
        push_entry(
            TerminalStream::Stderr,
            format!("Apple FM error: {error}"),
            None,
        );
    }

    if let Some(action) = spark_wallet.last_action.as_deref() {
        push_entry(TerminalStream::Stdout, format!("Wallet: {action}"), None);
    }
    if let Some(error) = spark_wallet.last_error.as_deref() {
        push_entry(
            TerminalStream::Stderr,
            format!("Wallet error: {error}"),
            None,
        );
    }

    for request in compute_flow_snapshot.recent_requests.iter().take(4) {
        push_entry(
            mission_control_log_stream_for_request_status(request.status),
            request.mission_control_log_line(),
            None,
        );
    }

    if !unsupported_sell_platform_offline {
        if job_inbox.requests.is_empty() {
            push_entry(
                TerminalStream::Stdout,
                if provider_runtime.mode == crate::state::provider_runtime::ProviderMode::Offline {
                    "Relay preview idle. Observed market activity will appear here before you go online."
                        .to_string()
                } else {
                    "Watching relays for matching jobs.".to_string()
                },
                None,
            );
        } else {
            let request_count = job_inbox.requests.len();
            push_entry(
                TerminalStream::Stdout,
                if provider_runtime.mode == crate::state::provider_runtime::ProviderMode::Offline {
                    format!("Relay preview: {request_count} observed jobs while offline.")
                } else {
                    format!("Relay intake: {request_count} observed jobs available.")
                },
                None,
            );
        }
    }
    if !unsupported_sell_platform_offline && let Some(action) = job_inbox.last_action.as_deref() {
        push_entry(TerminalStream::Stdout, format!("Inbox: {action}"), None);
    }

    if let Some(job) = compute_flow_snapshot.active_job.as_ref() {
        let mut line = format!(
            "Active {} -> {} [{}] {} auth={} phase={} next={}",
            job.job_id,
            job.capability,
            job.stage.label(),
            format_mission_control_amount(job.quoted_price_sats),
            job.authority.as_str(),
            job.phase.as_str(),
            job.next_expected_event,
        );
        if let Some(amount) = job.settlement_amount_sats {
            line.push_str(" settlement_sats=");
            line.push_str(amount.to_string().as_str());
        }
        if let Some(fees) = job.settlement_fees_sats {
            line.push_str(" settlement_fee_sats=");
            line.push_str(fees.to_string().as_str());
        }
        if let Some(delta) = job.settlement_net_wallet_delta_sats {
            line.push_str(" wallet_delta_sats=");
            line.push_str(delta.to_string().as_str());
        }
        push_entry(mission_control_log_stream_for_stage(job.stage), line, None);
    }
    if let Some(action) = active_job.last_action.as_deref() {
        push_entry(
            TerminalStream::Stdout,
            format!("Active job: {action}"),
            None,
        );
    }
    if let Some(error) = active_job.last_error.as_deref() {
        push_entry(
            TerminalStream::Stderr,
            format!("Active job error: {error}"),
            None,
        );
    }

    const LOG_STREAM_EARN_WINDOW_SECS: u64 = 900;
    let earn_cutoff = now_epoch.saturating_sub(LOG_STREAM_EARN_WINDOW_SECS);
    for row in earn_job_lifecycle_projection
        .rows
        .iter()
        .rev()
        .filter(|row| row.occurred_at_epoch_seconds >= earn_cutoff)
        .take(8)
    {
        let source = if row.source_tag.to_ascii_lowercase().contains("starter") {
            "STARTER"
        } else {
            "OPEN"
        };
        push_entry(
            mission_control_log_stream_for_stage(row.stage),
            format!(
                "[{source}] {} {} {}",
                row.stage.label(),
                row.job_id,
                format_mission_control_amount(row.quoted_price_sats)
            ),
            Some(row.occurred_at_epoch_seconds),
        );
    }

    entries.sort_by_key(|e| e.0);
    let content: Vec<(TerminalStream, String)> = entries
        .iter()
        .map(|(_, stream, text)| (stream.clone(), text.clone()))
        .collect();
    let lines: Vec<TerminalLine> = entries
        .into_iter()
        .map(|(epoch, stream, text)| {
            let timestamp = mission_control_log_timestamp(epoch);
            TerminalLine::new(stream, format!("{timestamp}  {text}"))
        })
        .collect();

    if lines.is_empty() {
        let fallback = "Mission Control log waiting for provider and wallet state.";
        (
            vec![TerminalLine::new(
                TerminalStream::Stdout,
                format!("{}  {fallback}", mission_control_log_timestamp(now_epoch)),
            )],
            vec![(TerminalStream::Stdout, fallback.to_string())],
        )
    } else {
        (lines, content)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MissionControlLocalRuntimeLane {
    AppleFoundationModels,
    NvidiaGptOss,
}

pub(crate) const MISSION_CONTROL_BUY_MODE_REQUEST_TYPE: &str = "mission_control.buy_mode.5050";
pub(crate) const MISSION_CONTROL_BUY_MODE_REQUEST_KIND: u16 =
    nostr::nip90::KIND_JOB_TEXT_GENERATION;
pub(crate) const MISSION_CONTROL_BUY_MODE_BUDGET_SATS: u64 = 2;
pub(crate) const MISSION_CONTROL_BUY_MODE_INTERVAL_MILLIS: u64 = 100;
pub(crate) const MISSION_CONTROL_BUY_MODE_INTERVAL: Duration =
    Duration::from_millis(MISSION_CONTROL_BUY_MODE_INTERVAL_MILLIS);
pub(crate) const MISSION_CONTROL_BUY_MODE_TIMEOUT_SECONDS: u64 = 75;

pub(crate) fn mission_control_buy_mode_interval_label() -> String {
    if MISSION_CONTROL_BUY_MODE_INTERVAL_MILLIS.is_multiple_of(1_000) {
        format!("{}s", MISSION_CONTROL_BUY_MODE_INTERVAL_MILLIS / 1_000)
    } else {
        format!("{}ms", MISSION_CONTROL_BUY_MODE_INTERVAL_MILLIS)
    }
}

pub(crate) fn mission_control_buy_mode_available_balance_sats(
    spark_wallet: &crate::spark_wallet::SparkPaneState,
) -> Option<u64> {
    spark_wallet
        .balance
        .as_ref()
        .map(openagents_spark::Balance::total_sats)
}

pub(crate) fn mission_control_buy_mode_start_block_reason(
    spark_wallet: &crate::spark_wallet::SparkPaneState,
) -> Option<String> {
    if let Some(error) = spark_wallet.last_error.as_deref() {
        return Some(format!(
            "Buy Mode requires a healthy Spark wallet ({error})"
        ));
    }

    match mission_control_buy_mode_available_balance_sats(spark_wallet) {
        Some(balance_sats) if balance_sats >= MISSION_CONTROL_BUY_MODE_BUDGET_SATS => None,
        Some(balance_sats) => Some(format!(
            "Buy Mode requires at least {} sats in Spark wallet (balance: {} sats)",
            MISSION_CONTROL_BUY_MODE_BUDGET_SATS, balance_sats
        )),
        None => Some(format!(
            "Buy Mode requires at least {} sats in Spark wallet (balance unavailable)",
            MISSION_CONTROL_BUY_MODE_BUDGET_SATS
        )),
    }
}

pub(crate) const fn mission_control_uses_apple_fm() -> bool {
    cfg!(target_os = "macos")
}

pub(crate) fn mission_control_supports_cuda_gpt_oss(
    local_inference_runtime: &crate::local_inference_runtime::LocalInferenceExecutionSnapshot,
) -> bool {
    !mission_control_uses_apple_fm()
        && local_inference_runtime
            .backend_label
            .trim()
            .eq_ignore_ascii_case("cuda")
}

fn mission_control_sell_compute_supported_for_platform(
    apple_fm_supported: bool,
    desktop_shell_mode: crate::desktop_shell::DesktopShellMode,
    local_inference_runtime: &crate::local_inference_runtime::LocalInferenceExecutionSnapshot,
) -> bool {
    apple_fm_supported
        || (desktop_shell_mode.is_dev()
            && local_inference_runtime
                .backend_label
                .trim()
                .eq_ignore_ascii_case("cuda"))
}

pub(crate) fn mission_control_sell_compute_supported(
    desktop_shell_mode: crate::desktop_shell::DesktopShellMode,
    local_inference_runtime: &crate::local_inference_runtime::LocalInferenceExecutionSnapshot,
) -> bool {
    mission_control_sell_compute_supported_for_platform(
        mission_control_uses_apple_fm(),
        desktop_shell_mode,
        local_inference_runtime,
    )
}

pub(crate) fn mission_control_local_runtime_lane(
    desktop_shell_mode: crate::desktop_shell::DesktopShellMode,
    local_inference_runtime: &crate::local_inference_runtime::LocalInferenceExecutionSnapshot,
) -> Option<MissionControlLocalRuntimeLane> {
    mission_control_local_runtime_lane_for_platform(
        mission_control_uses_apple_fm(),
        desktop_shell_mode,
        local_inference_runtime,
    )
}

fn mission_control_local_runtime_lane_for_platform(
    apple_fm_supported: bool,
    desktop_shell_mode: crate::desktop_shell::DesktopShellMode,
    local_inference_runtime: &crate::local_inference_runtime::LocalInferenceExecutionSnapshot,
) -> Option<MissionControlLocalRuntimeLane> {
    if apple_fm_supported {
        Some(MissionControlLocalRuntimeLane::AppleFoundationModels)
    } else if desktop_shell_mode.is_dev()
        && mission_control_supports_cuda_gpt_oss(local_inference_runtime)
    {
        Some(MissionControlLocalRuntimeLane::NvidiaGptOss)
    } else {
        None
    }
}

pub(crate) fn mission_control_local_runtime_is_ready(
    desktop_shell_mode: crate::desktop_shell::DesktopShellMode,
    provider_runtime: &crate::state::provider_runtime::ProviderRuntimeState,
    local_inference_runtime: &crate::local_inference_runtime::LocalInferenceExecutionSnapshot,
) -> bool {
    match mission_control_local_runtime_lane(desktop_shell_mode, local_inference_runtime) {
        Some(MissionControlLocalRuntimeLane::AppleFoundationModels) => {
            provider_runtime.apple_fm.is_ready()
        }
        Some(MissionControlLocalRuntimeLane::NvidiaGptOss) => local_inference_runtime.is_ready(),
        None => false,
    }
}

/// True when Mission Control should show the local-model button (Refresh/Start/Open Apple FM or Open GPT-OSS).
pub(crate) fn mission_control_show_local_model_button(
    desktop_shell_mode: crate::desktop_shell::DesktopShellMode,
    provider_runtime: &crate::state::provider_runtime::ProviderRuntimeState,
    local_inference_runtime: &crate::local_inference_runtime::LocalInferenceExecutionSnapshot,
) -> bool {
    match mission_control_local_runtime_lane(desktop_shell_mode, local_inference_runtime) {
        Some(MissionControlLocalRuntimeLane::AppleFoundationModels) => {
            provider_runtime.apple_fm.bridge_status.as_deref() != Some("starting")
        }
        Some(MissionControlLocalRuntimeLane::NvidiaGptOss) => true,
        None => false,
    }
}

fn mission_control_log_stream_for_stage(stage: JobLifecycleStage) -> TerminalStream {
    match stage {
        JobLifecycleStage::Failed => TerminalStream::Stderr,
        JobLifecycleStage::Received
        | JobLifecycleStage::Accepted
        | JobLifecycleStage::Running
        | JobLifecycleStage::Delivered
        | JobLifecycleStage::Paid => TerminalStream::Stdout,
    }
}

fn mission_control_log_stream_for_request_status(
    status: crate::state::operations::NetworkRequestStatus,
) -> TerminalStream {
    if status == crate::state::operations::NetworkRequestStatus::Failed {
        TerminalStream::Stderr
    } else {
        TerminalStream::Stdout
    }
}

fn mission_control_log_short_id(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() <= 12 {
        trimmed.to_string()
    } else {
        format!("{}..", &trimmed[..12])
    }
}

fn mission_control_network_request_log_line(
    request: &crate::state::operations::SubmittedNetworkRequest,
    spark_wallet: &SparkPaneState,
) -> String {
    crate::nip90_compute_flow::build_buyer_request_flow_snapshot(request, spark_wallet)
        .mission_control_log_line()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AutopilotRole {
    User,
    Codex,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AutopilotMessageStatus {
    Queued,
    Running,
    Done,
    Error,
}

#[derive(Clone)]
pub struct AutopilotMessage {
    pub id: u64,
    pub role: AutopilotRole,
    pub status: AutopilotMessageStatus,
    pub content: String,
    pub structured: Option<AutopilotStructuredMessage>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AutopilotProgressRow {
    pub label: String,
    pub value: String,
    pub tone: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AutopilotProgressBlock {
    pub kind: String,
    pub title: String,
    pub status: String,
    pub rows: Vec<AutopilotProgressRow>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AutopilotStructuredMessage {
    pub reasoning: String,
    pub answer: String,
    pub events: Vec<String>,
    pub status: Option<String>,
    pub progress_blocks: Vec<AutopilotProgressBlock>,
}

impl AutopilotStructuredMessage {
    fn rendered_content(&self) -> String {
        let reasoning = self.reasoning.trim_end();
        let answer = self.answer.trim_end();
        if reasoning.is_empty() {
            return answer.to_string();
        }
        if answer.is_empty() {
            return reasoning.to_string();
        }
        format!("{reasoning}\n\n{answer}")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AutopilotDeltaSignature {
    turn_id: String,
    item_id: String,
    delta: String,
}

pub struct AutopilotTokenUsage {
    pub input_tokens: i64,
    pub cached_input_tokens: i64,
    pub output_tokens: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AutopilotTurnPlanStep {
    pub step: String,
    pub status: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AutopilotPlanArtifact {
    pub thread_id: String,
    pub source_turn_id: String,
    pub explanation: Option<String>,
    pub steps: Vec<AutopilotTurnPlanStep>,
    pub workspace_cwd: Option<String>,
    pub workspace_path: Option<String>,
    pub workspace_root: Option<String>,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub git_branch: Option<String>,
    pub git_dirty: Option<bool>,
    pub updated_at_epoch_ms: u64,
    pub restored_from_thread_read: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AutopilotDiffFileArtifact {
    pub path: String,
    pub added_line_count: u32,
    pub removed_line_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AutopilotDiffArtifact {
    pub thread_id: String,
    pub source_turn_id: String,
    pub files: Vec<AutopilotDiffFileArtifact>,
    pub added_line_count: u32,
    pub removed_line_count: u32,
    pub raw_diff: String,
    pub workspace_root: Option<String>,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub git_branch: Option<String>,
    pub git_dirty: Option<bool>,
    pub updated_at_epoch_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AutopilotReviewArtifact {
    pub thread_id: String,
    pub source_thread_id: String,
    pub source_turn_id: String,
    pub review_thread_id: String,
    pub delivery: String,
    pub target: String,
    pub summary: Option<String>,
    pub status: String,
    pub updated_at_epoch_ms: u64,
    pub restored_from_thread_read: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AutopilotCompactionArtifact {
    pub thread_id: String,
    pub source_turn_id: String,
    pub updated_at_epoch_ms: u64,
    pub restored_from_thread_read: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AutopilotProjectDefaults {
    pub model: Option<String>,
    pub service_tier: AutopilotChatServiceTier,
    pub reasoning_effort: Option<String>,
    pub approval_policy: Option<codex_client::AskForApproval>,
    pub sandbox_mode: Option<codex_client::SandboxMode>,
    pub personality: AutopilotChatPersonality,
    pub collaboration_mode: AutopilotChatCollaborationMode,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AutopilotProjectIdentity {
    pub project_id: String,
    pub project_name: String,
    pub workspace_root: String,
    pub git_branch: Option<String>,
    pub git_dirty: Option<bool>,
    pub thread_ids: Vec<String>,
    pub defaults: AutopilotProjectDefaults,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AutopilotTerminalSessionStatus {
    Pending,
    Running,
    Exited,
    Failed,
    Closed,
}

impl AutopilotTerminalSessionStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Exited => "exited",
            Self::Failed => "failed",
            Self::Closed => "closed",
        }
    }

    pub const fn is_active(self) -> bool {
        matches!(self, Self::Pending | Self::Running)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AutopilotTerminalSession {
    pub thread_id: String,
    pub workspace_root: String,
    pub shell: String,
    pub pid: Option<u32>,
    pub cols: u16,
    pub rows: u16,
    pub status: AutopilotTerminalSessionStatus,
    pub exit_code: Option<i32>,
    pub lines: Vec<TerminalLine>,
    pub created_at_epoch_ms: u64,
    pub updated_at_epoch_ms: u64,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AutopilotTurnMetadata {
    pub submission_seq: u64,
    pub thread_id: String,
    pub run_classification: CodexRunClassification,
    pub labor_binding: Option<CodexLaborBinding>,
    pub is_cad_turn: bool,
    pub classifier_reason: String,
    pub submitted_at_epoch_ms: u64,
    pub selected_skill_names: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AutopilotPendingSteerSubmission {
    command_seq: u64,
    thread_id: String,
    prompt: String,
}

#[derive(Clone)]
pub struct AutopilotApprovalRequest {
    pub request_id: codex_client::AppServerRequestId,
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub reason: Option<String>,
    pub command: Option<String>,
    pub cwd: Option<String>,
}

#[derive(Clone)]
pub struct AutopilotFileChangeApprovalRequest {
    pub request_id: codex_client::AppServerRequestId,
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub reason: Option<String>,
    pub grant_root: Option<String>,
}

#[derive(Clone)]
pub struct AutopilotToolCallRequest {
    pub request_id: codex_client::AppServerRequestId,
    pub thread_id: String,
    pub turn_id: String,
    pub call_id: String,
    pub tool: String,
    pub arguments: String,
}

#[derive(Clone)]
pub struct AutopilotToolUserInputQuestion {
    pub id: String,
    pub header: String,
    pub question: String,
    pub options: Vec<String>,
}

#[derive(Clone)]
pub struct AutopilotToolUserInputRequest {
    pub request_id: codex_client::AppServerRequestId,
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub questions: Vec<AutopilotToolUserInputQuestion>,
}

#[derive(Clone)]
pub struct AutopilotAuthRefreshRequest {
    pub request_id: codex_client::AppServerRequestId,
    pub reason: String,
    pub previous_account_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AutopilotChatServiceTier {
    #[default]
    Default,
    Fast,
    Flex,
}

impl AutopilotChatServiceTier {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Fast => "fast",
            Self::Flex => "flex",
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::Default => Self::Fast,
            Self::Fast => Self::Flex,
            Self::Flex => Self::Default,
        }
    }

    pub const fn request_value(self) -> Option<Option<codex_client::ServiceTier>> {
        match self {
            Self::Default => Some(None),
            Self::Fast => Some(Some(codex_client::ServiceTier::Fast)),
            Self::Flex => Some(Some(codex_client::ServiceTier::Flex)),
        }
    }

    pub const fn from_response(value: Option<codex_client::ServiceTier>) -> Self {
        match value {
            Some(codex_client::ServiceTier::Fast) => Self::Fast,
            Some(codex_client::ServiceTier::Flex) => Self::Flex,
            None => Self::Default,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AutopilotChatPersonality {
    #[default]
    Auto,
    Friendly,
    Pragmatic,
    None,
}

impl AutopilotChatPersonality {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Friendly => "friendly",
            Self::Pragmatic => "pragmatic",
            Self::None => "none",
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::Auto => Self::Friendly,
            Self::Friendly => Self::Pragmatic,
            Self::Pragmatic => Self::None,
            Self::None => Self::Auto,
        }
    }

    pub const fn request_value(self) -> Option<codex_client::Personality> {
        match self {
            Self::Auto => None,
            Self::Friendly => Some(codex_client::Personality::Friendly),
            Self::Pragmatic => Some(codex_client::Personality::Pragmatic),
            Self::None => Some(codex_client::Personality::None),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AutopilotChatCollaborationMode {
    #[default]
    Off,
    Default,
    Plan,
}

impl AutopilotChatCollaborationMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Default => "default",
            Self::Plan => "plan",
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::Off => Self::Default,
            Self::Default => Self::Plan,
            Self::Plan => Self::Off,
        }
    }
}

#[derive(Clone)]
pub struct AutopilotThreadMetadata {
    pub thread_name: Option<String>,
    pub preview: Option<String>,
    pub status: Option<String>,
    pub loaded: bool,
    pub cwd: Option<String>,
    pub path: Option<String>,
    pub workspace_root: Option<String>,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub git_branch: Option<String>,
    pub git_dirty: Option<bool>,
    pub created_at: Option<i64>,
    pub updated_at: Option<i64>,
    pub model: Option<String>,
    pub service_tier: AutopilotChatServiceTier,
    pub reasoning_effort: Option<String>,
    pub approval_policy: Option<codex_client::AskForApproval>,
    pub sandbox_mode: Option<codex_client::SandboxMode>,
    pub personality: AutopilotChatPersonality,
    pub collaboration_mode: AutopilotChatCollaborationMode,
}

impl Default for AutopilotThreadMetadata {
    fn default() -> Self {
        Self {
            thread_name: None,
            preview: None,
            status: None,
            loaded: false,
            cwd: None,
            path: None,
            workspace_root: None,
            project_id: None,
            project_name: None,
            git_branch: None,
            git_dirty: None,
            created_at: None,
            updated_at: None,
            model: None,
            service_tier: AutopilotChatServiceTier::Default,
            reasoning_effort: Some("medium".to_string()),
            approval_policy: Some(codex_client::AskForApproval::Never),
            sandbox_mode: Some(codex_client::SandboxMode::DangerFullAccess),
            personality: AutopilotChatPersonality::Auto,
            collaboration_mode: AutopilotChatCollaborationMode::Off,
        }
    }
}

pub struct AutopilotThreadResumeTarget {
    pub thread_id: String,
    pub cwd: Option<String>,
    pub path: Option<String>,
}

#[derive(Clone)]
pub struct AutopilotThreadListEntry {
    pub thread_id: String,
    pub thread_name: Option<String>,
    pub preview: String,
    pub status: Option<String>,
    pub loaded: bool,
    pub cwd: Option<String>,
    pub path: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManagedChatChannelRailRow {
    Category {
        category_id: String,
        label: String,
        collapsed: bool,
        channel_count: usize,
        unread_count: usize,
        mention_count: usize,
    },
    Channel {
        channel_id: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChatBrowseMode {
    Autopilot,
    Managed,
    DirectMessages,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChatWorkspaceSelection {
    Autopilot,
    ManagedGroup(String),
    DirectMessages,
}

pub struct AutopilotChatState {
    pub connection_status: String,
    pub models: Vec<String>,
    pub selected_model: usize,
    pub reasoning_effort: Option<String>,
    pub service_tier: AutopilotChatServiceTier,
    pub personality: AutopilotChatPersonality,
    pub collaboration_mode: AutopilotChatCollaborationMode,
    pub approval_mode: codex_client::AskForApproval,
    pub sandbox_mode: codex_client::SandboxMode,
    pub threads: Vec<String>,
    pub thread_metadata: std::collections::HashMap<String, AutopilotThreadMetadata>,
    pub project_registry: std::collections::HashMap<String, AutopilotProjectIdentity>,
    pub terminal_sessions: std::collections::HashMap<String, AutopilotTerminalSession>,
    pub thread_transcript_cache: std::collections::HashMap<String, Vec<AutopilotMessage>>,
    thread_plan_artifacts: std::collections::HashMap<String, AutopilotPlanArtifact>,
    thread_diff_artifacts: std::collections::HashMap<String, Vec<AutopilotDiffArtifact>>,
    thread_review_artifacts: std::collections::HashMap<String, AutopilotReviewArtifact>,
    thread_compaction_artifacts: std::collections::HashMap<String, AutopilotCompactionArtifact>,
    review_thread_source_map: std::collections::HashMap<String, String>,
    thread_composer_drafts: std::collections::HashMap<String, String>,
    detached_composer_draft: String,
    thread_submission_history: std::collections::HashMap<String, VecDeque<String>>,
    pub active_thread_id: Option<String>,
    pub selected_workspace: ChatWorkspaceSelection,
    pub managed_chat_projection: ManagedChatProjectionState,
    pub direct_message_projection: DirectMessageProjectionState,
    pub startup_new_thread_bootstrap_pending: bool,
    pub startup_new_thread_bootstrap_sent: bool,
    pub messages: Vec<AutopilotMessage>,
    pub next_message_id: u64,
    pub active_turn_id: Option<String>,
    pub active_assistant_message_id: Option<u64>,
    pub pending_assistant_message_ids: VecDeque<u64>,
    pub turn_assistant_message_ids: std::collections::HashMap<String, u64>,
    pub next_turn_submission_seq: u64,
    pub pending_turn_metadata: VecDeque<AutopilotTurnMetadata>,
    pending_steer_submissions: VecDeque<AutopilotPendingSteerSubmission>,
    pub turn_metadata_by_turn_id: std::collections::HashMap<String, AutopilotTurnMetadata>,
    pub last_submitted_turn_metadata: Option<AutopilotTurnMetadata>,
    last_agent_item_ids: std::collections::HashMap<String, String>,
    last_reasoning_item_ids: std::collections::HashMap<String, String>,
    last_agent_delta_signature: Option<AutopilotDeltaSignature>,
    last_reasoning_delta_signature: Option<AutopilotDeltaSignature>,
    pub last_turn_status: Option<String>,
    pub token_usage: Option<AutopilotTokenUsage>,
    pub turn_plan_explanation: Option<String>,
    pub turn_plan: Vec<AutopilotTurnPlanStep>,
    pub turn_diff: Option<String>,
    pub turn_timeline: Vec<String>,
    pub pending_command_approvals: Vec<AutopilotApprovalRequest>,
    pub pending_file_change_approvals: Vec<AutopilotFileChangeApprovalRequest>,
    pub pending_tool_calls: Vec<AutopilotToolCallRequest>,
    pub pending_tool_user_input: Vec<AutopilotToolUserInputRequest>,
    pub pending_auth_refresh: Vec<AutopilotAuthRefreshRequest>,
    pub auth_refresh_access_token: String,
    pub auth_refresh_account_id: String,
    pub auth_refresh_plan_type: String,
    pub thread_filter_archived: Option<bool>,
    pub thread_filter_sort_key: codex_client::ThreadSortKey,
    pub thread_filter_source_kind: Option<codex_client::ThreadSourceKind>,
    pub thread_filter_model_provider: Option<String>,
    pub thread_filter_search_term: String,
    pub thread_rename_counter: u64,
    pub transcript_scroll_offset: f32,
    pub transcript_follow_tail: bool,
    pub transcript_selection: Option<ChatTranscriptSelectionState>,
    pub last_error: Option<String>,
    pub copy_notice: Option<String>,
    pub copy_notice_until: Option<Instant>,
    pub buy_mode_last_targeted_peer_pubkey: Option<String>,
    artifact_projection_file_path: PathBuf,
}

impl Default for AutopilotChatState {
    fn default() -> Self {
        let artifact_projection_file_path = codex_artifact_projection_file_path();
        let (
            thread_diff_artifacts,
            thread_review_artifacts,
            thread_compaction_artifacts,
            review_thread_source_map,
            artifact_load_error,
        ) = match load_codex_artifact_projection(artifact_projection_file_path.as_path()) {
            Ok(projection) => (
                projection.thread_diff_artifacts,
                projection.thread_review_artifacts,
                projection.thread_compaction_artifacts,
                projection.review_thread_source_map,
                None,
            ),
            Err(error) => (
                HashMap::new(),
                HashMap::new(),
                HashMap::new(),
                HashMap::new(),
                Some(error),
            ),
        };
        Self {
            connection_status: "ready".to_string(),
            // "auto" means "let app-server pick the current default model".
            models: vec!["auto".to_string()],
            selected_model: 0,
            reasoning_effort: Some("medium".to_string()),
            service_tier: AutopilotChatServiceTier::Default,
            personality: AutopilotChatPersonality::Auto,
            collaboration_mode: AutopilotChatCollaborationMode::Off,
            approval_mode: codex_client::AskForApproval::Never,
            sandbox_mode: codex_client::SandboxMode::DangerFullAccess,
            threads: Vec::new(),
            thread_metadata: std::collections::HashMap::new(),
            project_registry: std::collections::HashMap::new(),
            terminal_sessions: std::collections::HashMap::new(),
            thread_transcript_cache: std::collections::HashMap::new(),
            thread_plan_artifacts: std::collections::HashMap::new(),
            thread_diff_artifacts,
            thread_review_artifacts,
            thread_compaction_artifacts,
            review_thread_source_map,
            thread_composer_drafts: std::collections::HashMap::new(),
            detached_composer_draft: String::new(),
            thread_submission_history: std::collections::HashMap::new(),
            active_thread_id: None,
            selected_workspace: ChatWorkspaceSelection::Autopilot,
            managed_chat_projection: ManagedChatProjectionState::default(),
            direct_message_projection: DirectMessageProjectionState::default(),
            startup_new_thread_bootstrap_pending: true,
            startup_new_thread_bootstrap_sent: false,
            messages: Vec::new(),
            next_message_id: 1,
            active_turn_id: None,
            active_assistant_message_id: None,
            pending_assistant_message_ids: VecDeque::new(),
            turn_assistant_message_ids: std::collections::HashMap::new(),
            next_turn_submission_seq: 1,
            pending_turn_metadata: VecDeque::new(),
            pending_steer_submissions: VecDeque::new(),
            turn_metadata_by_turn_id: std::collections::HashMap::new(),
            last_submitted_turn_metadata: None,
            last_agent_item_ids: std::collections::HashMap::new(),
            last_reasoning_item_ids: std::collections::HashMap::new(),
            last_agent_delta_signature: None,
            last_reasoning_delta_signature: None,
            last_turn_status: None,
            token_usage: None,
            turn_plan_explanation: None,
            turn_plan: Vec::new(),
            turn_diff: None,
            turn_timeline: Vec::new(),
            pending_command_approvals: Vec::new(),
            pending_file_change_approvals: Vec::new(),
            pending_tool_calls: Vec::new(),
            pending_tool_user_input: Vec::new(),
            pending_auth_refresh: Vec::new(),
            auth_refresh_access_token: std::env::var("OPENAI_ACCESS_TOKEN").unwrap_or_default(),
            auth_refresh_account_id: std::env::var("OPENAI_CHATGPT_ACCOUNT_ID").unwrap_or_default(),
            auth_refresh_plan_type: std::env::var("OPENAI_CHATGPT_PLAN_TYPE").unwrap_or_default(),
            thread_filter_archived: Some(false),
            thread_filter_sort_key: codex_client::ThreadSortKey::UpdatedAt,
            thread_filter_source_kind: None,
            thread_filter_model_provider: None,
            thread_filter_search_term: String::new(),
            thread_rename_counter: 1,
            transcript_scroll_offset: 0.0,
            transcript_follow_tail: true,
            transcript_selection: None,
            last_error: artifact_load_error,
            copy_notice: None,
            copy_notice_until: None,
            buy_mode_last_targeted_peer_pubkey: None,
            artifact_projection_file_path,
        }
    }
}

const CODEX_ARTIFACT_PROJECTION_SCHEMA_VERSION: u16 = 1;
const CODEX_ARTIFACT_PROJECTION_STREAM_ID: &str = "stream.codex_artifacts.v1";
const CODEX_DIFF_ARTIFACT_LIMIT_PER_THREAD: usize = 8;

#[derive(Debug, Serialize, Deserialize)]
struct CodexArtifactProjectionDocumentV1 {
    schema_version: u16,
    stream_id: String,
    diff_artifacts: Vec<AutopilotDiffArtifact>,
    review_artifacts: Vec<AutopilotReviewArtifact>,
    compaction_artifacts: Vec<AutopilotCompactionArtifact>,
}

struct LoadedCodexArtifactProjection {
    thread_diff_artifacts: HashMap<String, Vec<AutopilotDiffArtifact>>,
    thread_review_artifacts: HashMap<String, AutopilotReviewArtifact>,
    thread_compaction_artifacts: HashMap<String, AutopilotCompactionArtifact>,
    review_thread_source_map: HashMap<String, String>,
}

fn codex_artifact_projection_file_path() -> PathBuf {
    if let Ok(override_path) = std::env::var("OPENAGENTS_CODEX_ARTIFACT_PROJECTION_PATH") {
        if !override_path.trim().is_empty() {
            return PathBuf::from(override_path);
        }
    }
    #[cfg(test)]
    {
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT_TEST_PROJECTION_ID: AtomicU64 = AtomicU64::new(1);
        return std::env::temp_dir().join(format!(
            "openagents-codex-artifacts-test-{}-{}.json",
            std::process::id(),
            NEXT_TEST_PROJECTION_ID.fetch_add(1, Ordering::Relaxed)
        ));
    }
    #[cfg(not(test))]
    {
        std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".openagents")
            .join("autopilot-codex-artifacts-v1.json")
    }
}

fn review_artifact_thread_keys(source_thread_id: &str, review_thread_id: &str) -> Vec<String> {
    if source_thread_id == review_thread_id {
        vec![source_thread_id.to_string()]
    } else {
        vec![source_thread_id.to_string(), review_thread_id.to_string()]
    }
}

fn normalized_review_target(value: Option<String>) -> String {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "review".to_string())
}

fn normalized_review_delivery(
    value: Option<String>,
    source_thread_id: &str,
    review_thread_id: &str,
) -> String {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            if source_thread_id == review_thread_id {
                "inline".to_string()
            } else {
                "detached".to_string()
            }
        })
}

fn normalize_codex_diff_artifacts(
    mut artifacts: Vec<AutopilotDiffArtifact>,
) -> Vec<AutopilotDiffArtifact> {
    artifacts.sort_by(|lhs, rhs| {
        rhs.updated_at_epoch_ms
            .cmp(&lhs.updated_at_epoch_ms)
            .then_with(|| lhs.thread_id.cmp(&rhs.thread_id))
            .then_with(|| lhs.source_turn_id.cmp(&rhs.source_turn_id))
    });
    let mut seen_keys = HashSet::new();
    artifacts.retain(|artifact| {
        seen_keys.insert(format!(
            "{}|{}",
            artifact.thread_id, artifact.source_turn_id
        ))
    });
    let mut thread_counts = HashMap::<String, usize>::new();
    artifacts.retain(|artifact| {
        let count = thread_counts.entry(artifact.thread_id.clone()).or_insert(0);
        if *count >= CODEX_DIFF_ARTIFACT_LIMIT_PER_THREAD {
            return false;
        }
        *count += 1;
        true
    });
    artifacts
}

fn normalize_codex_review_artifacts(
    mut artifacts: Vec<AutopilotReviewArtifact>,
) -> Vec<AutopilotReviewArtifact> {
    artifacts.sort_by(|lhs, rhs| {
        rhs.updated_at_epoch_ms
            .cmp(&lhs.updated_at_epoch_ms)
            .then_with(|| lhs.thread_id.cmp(&rhs.thread_id))
    });
    let mut seen_thread_ids = HashSet::new();
    artifacts.retain(|artifact| seen_thread_ids.insert(artifact.thread_id.clone()));
    artifacts
}

fn normalize_codex_compaction_artifacts(
    mut artifacts: Vec<AutopilotCompactionArtifact>,
) -> Vec<AutopilotCompactionArtifact> {
    artifacts.sort_by(|lhs, rhs| {
        rhs.updated_at_epoch_ms
            .cmp(&lhs.updated_at_epoch_ms)
            .then_with(|| lhs.thread_id.cmp(&rhs.thread_id))
    });
    let mut seen_thread_ids = HashSet::new();
    artifacts.retain(|artifact| seen_thread_ids.insert(artifact.thread_id.clone()));
    artifacts
}

fn persist_codex_artifact_projection(
    path: &Path,
    diff_artifacts: &HashMap<String, Vec<AutopilotDiffArtifact>>,
    review_artifacts: &HashMap<String, AutopilotReviewArtifact>,
    compaction_artifacts: &HashMap<String, AutopilotCompactionArtifact>,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create Codex artifact dir: {error}"))?;
    }
    let document = CodexArtifactProjectionDocumentV1 {
        schema_version: CODEX_ARTIFACT_PROJECTION_SCHEMA_VERSION,
        stream_id: CODEX_ARTIFACT_PROJECTION_STREAM_ID.to_string(),
        diff_artifacts: normalize_codex_diff_artifacts(
            diff_artifacts
                .values()
                .flat_map(|artifacts| artifacts.iter().cloned())
                .collect(),
        ),
        review_artifacts: normalize_codex_review_artifacts(
            review_artifacts.values().cloned().collect(),
        ),
        compaction_artifacts: normalize_codex_compaction_artifacts(
            compaction_artifacts.values().cloned().collect(),
        ),
    };
    let payload = serde_json::to_string_pretty(&document)
        .map_err(|error| format!("Failed to encode Codex artifacts: {error}"))?;
    let temp_path = path.with_extension("tmp");
    std::fs::write(&temp_path, payload)
        .map_err(|error| format!("Failed to write Codex artifact temp file: {error}"))?;
    std::fs::rename(&temp_path, path)
        .map_err(|error| format!("Failed to persist Codex artifacts: {error}"))?;
    Ok(())
}

fn load_codex_artifact_projection(path: &Path) -> Result<LoadedCodexArtifactProjection, String> {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(LoadedCodexArtifactProjection {
                thread_diff_artifacts: HashMap::new(),
                thread_review_artifacts: HashMap::new(),
                thread_compaction_artifacts: HashMap::new(),
                review_thread_source_map: HashMap::new(),
            });
        }
        Err(error) => return Err(format!("Failed to read Codex artifacts: {error}")),
    };
    let document = serde_json::from_str::<CodexArtifactProjectionDocumentV1>(&raw)
        .map_err(|error| format!("Failed to parse Codex artifacts: {error}"))?;
    if document.schema_version != CODEX_ARTIFACT_PROJECTION_SCHEMA_VERSION {
        return Err(format!(
            "Unsupported Codex artifact schema version: {}",
            document.schema_version
        ));
    }
    if document.stream_id != CODEX_ARTIFACT_PROJECTION_STREAM_ID {
        return Err(format!(
            "Unsupported Codex artifact stream id: {}",
            document.stream_id
        ));
    }

    let mut thread_diff_artifacts = HashMap::<String, Vec<AutopilotDiffArtifact>>::new();
    for artifact in normalize_codex_diff_artifacts(document.diff_artifacts) {
        thread_diff_artifacts
            .entry(artifact.thread_id.clone())
            .or_default()
            .push(artifact);
    }

    let mut thread_review_artifacts = HashMap::<String, AutopilotReviewArtifact>::new();
    let mut review_thread_source_map = HashMap::<String, String>::new();
    for artifact in normalize_codex_review_artifacts(document.review_artifacts) {
        review_thread_source_map.insert(
            artifact.review_thread_id.clone(),
            artifact.source_thread_id.clone(),
        );
        thread_review_artifacts.insert(artifact.thread_id.clone(), artifact);
    }

    let mut thread_compaction_artifacts = HashMap::<String, AutopilotCompactionArtifact>::new();
    for artifact in normalize_codex_compaction_artifacts(document.compaction_artifacts) {
        thread_compaction_artifacts.insert(artifact.thread_id.clone(), artifact);
    }

    Ok(LoadedCodexArtifactProjection {
        thread_diff_artifacts,
        thread_review_artifacts,
        thread_compaction_artifacts,
        review_thread_source_map,
    })
}

fn parse_diff_file_artifacts(raw_diff: &str) -> Vec<AutopilotDiffFileArtifact> {
    let mut files = Vec::<AutopilotDiffFileArtifact>::new();
    let mut current_file_index = None;
    for line in raw_diff.lines() {
        if let Some(path) = line.strip_prefix("+++ b/") {
            current_file_index = Some(ensure_diff_file_entry(&mut files, path.trim()));
            continue;
        }
        if let Some(path) = line.strip_prefix("rename to ") {
            current_file_index = Some(ensure_diff_file_entry(&mut files, path.trim()));
            continue;
        }
        if line.starts_with("diff --git ") {
            current_file_index = None;
            continue;
        }
        if line.starts_with("@@")
            || line.starts_with("--- ")
            || line.starts_with("index ")
            || line.starts_with("new file mode ")
            || line.starts_with("deleted file mode ")
        {
            continue;
        }
        if let Some(index) = current_file_index {
            if line.starts_with('+') {
                files[index].added_line_count = files[index].added_line_count.saturating_add(1);
            } else if line.starts_with('-') {
                files[index].removed_line_count = files[index].removed_line_count.saturating_add(1);
            }
        }
    }
    files
}

fn ensure_diff_file_entry(files: &mut Vec<AutopilotDiffFileArtifact>, path: &str) -> usize {
    if let Some(index) = files.iter().position(|file| file.path == path) {
        return index;
    }
    files.push(AutopilotDiffFileArtifact {
        path: path.to_string(),
        added_line_count: 0,
        removed_line_count: 0,
    });
    files.len().saturating_sub(1)
}

fn normalized_optional_path(value: Option<&str>) -> Option<PathBuf> {
    let trimmed = value?.trim();
    if trimmed.is_empty() {
        return None;
    }
    let path = PathBuf::from(trimmed);
    Some(std::fs::canonicalize(&path).unwrap_or(path))
}

fn workspace_root_for_thread_paths(cwd: Option<&str>, path: Option<&str>) -> Option<String> {
    let base = normalized_optional_path(cwd).or_else(|| {
        normalized_optional_path(path).and_then(|path| path.parent().map(Path::to_path_buf))
    })?;
    git_workspace_root(base.as_path())
        .or_else(|| Some(base.display().to_string()))
        .filter(|value| !value.trim().is_empty())
}

fn git_command_output(cwd: &Path, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Some(stdout)
}

fn git_workspace_root(start: &Path) -> Option<String> {
    git_command_output(start, &["rev-parse", "--show-toplevel"])
        .and_then(|value| (!value.is_empty()).then_some(value))
}

fn git_branch_for_workspace_root(workspace_root: &str) -> Option<String> {
    let root = Path::new(workspace_root);
    git_command_output(root, &["branch", "--show-current"])
        .and_then(|value| (!value.is_empty()).then_some(value))
        .or_else(|| {
            git_command_output(root, &["rev-parse", "--short", "HEAD"])
                .and_then(|value| (!value.is_empty()).then_some(value))
        })
}

fn git_dirty_for_workspace_root(workspace_root: &str) -> Option<bool> {
    let root = Path::new(workspace_root);
    git_command_output(root, &["status", "--porcelain", "--untracked-files=normal"])
        .map(|value| !value.is_empty())
}

fn project_name_for_workspace_root(workspace_root: &str) -> String {
    Path::new(workspace_root)
        .file_name()
        .and_then(|value| value.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| workspace_root.to_string())
}

fn current_epoch_millis_for_state() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn project_defaults_from_thread_metadata(
    metadata: &AutopilotThreadMetadata,
) -> AutopilotProjectDefaults {
    AutopilotProjectDefaults {
        model: metadata.model.clone(),
        service_tier: metadata.service_tier,
        reasoning_effort: metadata.reasoning_effort.clone(),
        approval_policy: metadata.approval_policy,
        sandbox_mode: metadata.sandbox_mode,
        personality: metadata.personality,
        collaboration_mode: metadata.collaboration_mode,
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SidebarState {
    pub width: f32,
    pub is_open: bool,
    pub is_pressed: bool,
    pub is_dragging: bool,
    pub drag_start_x: f32,
    pub drag_start_width: f32,
    pub settings_hover: bool,
    pub settings_tooltip_t: f32,
}

impl Default for SidebarState {
    fn default() -> Self {
        Self {
            width: 300.0,
            is_open: false,
            is_pressed: false,
            is_dragging: false,
            drag_start_x: 0.0,
            drag_start_width: 300.0,
            settings_hover: false,
            settings_tooltip_t: 0.0,
        }
    }
}

impl AutopilotChatState {
    #[cfg(test)]
    fn from_artifact_projection_path_for_tests(artifact_projection_file_path: PathBuf) -> Self {
        let mut state = Self::default();
        state.artifact_projection_file_path = artifact_projection_file_path.clone();
        match load_codex_artifact_projection(artifact_projection_file_path.as_path()) {
            Ok(projection) => {
                state.thread_diff_artifacts = projection.thread_diff_artifacts;
                state.thread_review_artifacts = projection.thread_review_artifacts;
                state.thread_compaction_artifacts = projection.thread_compaction_artifacts;
                state.review_thread_source_map = projection.review_thread_source_map;
                state.last_error = None;
            }
            Err(error) => {
                state.thread_diff_artifacts.clear();
                state.thread_review_artifacts.clear();
                state.thread_compaction_artifacts.clear();
                state.review_thread_source_map.clear();
                state.last_error = Some(error);
            }
        }
        state
    }

    pub fn reset_transcript_scroll(&mut self) {
        self.transcript_scroll_offset = 0.0;
        self.transcript_follow_tail = true;
    }

    pub fn clear_transcript_selection(&mut self) {
        self.transcript_selection = None;
    }

    pub fn transcript_effective_scroll_offset(&self, max_scroll: f32) -> f32 {
        let max_scroll = max_scroll.max(0.0);
        if self.transcript_follow_tail {
            max_scroll
        } else {
            self.transcript_scroll_offset.clamp(0.0, max_scroll)
        }
    }

    pub fn scroll_transcript_by(&mut self, delta: f32, max_scroll: f32) {
        let max_scroll = max_scroll.max(0.0);
        if max_scroll <= 0.0 {
            self.reset_transcript_scroll();
            return;
        }

        let mut offset = self.transcript_effective_scroll_offset(max_scroll) + delta;
        if !offset.is_finite() {
            offset = max_scroll;
        }
        offset = offset.clamp(0.0, max_scroll);
        self.transcript_scroll_offset = offset;
        self.transcript_follow_tail = (max_scroll - offset).abs() <= 1.0;
    }

    pub fn current_model(&self) -> &str {
        self.models
            .get(self.selected_model)
            .map(String::as_str)
            .unwrap_or("auto")
    }

    pub fn selected_model_override(&self) -> Option<String> {
        let value = self.current_model().trim();
        if value.is_empty() || value.eq_ignore_ascii_case("auto") {
            None
        } else {
            Some(value.to_string())
        }
    }

    pub fn active_thread_cwd(&self) -> Option<&str> {
        let thread_id = self.active_thread_id.as_ref()?;
        self.thread_metadata
            .get(thread_id)
            .and_then(|metadata| metadata.cwd.as_deref())
    }

    pub fn active_thread_workspace_root(&self) -> Option<&str> {
        let thread_id = self.active_thread_id.as_ref()?;
        self.thread_metadata
            .get(thread_id)
            .and_then(|metadata| metadata.workspace_root.as_deref())
    }

    pub fn active_thread_project_name(&self) -> Option<&str> {
        let thread_id = self.active_thread_id.as_ref()?;
        self.thread_metadata
            .get(thread_id)
            .and_then(|metadata| metadata.project_name.as_deref())
    }

    pub fn active_thread_git_branch(&self) -> Option<&str> {
        let thread_id = self.active_thread_id.as_ref()?;
        self.thread_metadata
            .get(thread_id)
            .and_then(|metadata| metadata.git_branch.as_deref())
    }

    pub fn active_thread_git_dirty(&self) -> Option<bool> {
        let thread_id = self.active_thread_id.as_ref()?;
        self.thread_metadata
            .get(thread_id)
            .and_then(|metadata| metadata.git_dirty)
    }

    pub fn project_for_thread(&self, thread_id: &str) -> Option<&AutopilotProjectIdentity> {
        let project_id = self
            .thread_metadata
            .get(thread_id)
            .and_then(|metadata| metadata.project_id.as_deref())?;
        self.project_registry.get(project_id)
    }

    pub fn active_project(&self) -> Option<&AutopilotProjectIdentity> {
        self.active_thread_id
            .as_deref()
            .and_then(|thread_id| self.project_for_thread(thread_id))
    }

    fn rebuild_project_registry(&mut self) {
        let previous_registry = self.project_registry.clone();
        let active_thread_id = self.active_thread_id.clone();
        let mut project_threads = HashMap::<String, Vec<String>>::new();
        let mut git_state_by_project = HashMap::<String, (Option<String>, Option<bool>)>::new();

        for (thread_id, metadata) in self.thread_metadata.iter_mut() {
            let workspace_root =
                workspace_root_for_thread_paths(metadata.cwd.as_deref(), metadata.path.as_deref());
            metadata.workspace_root = workspace_root.clone();
            metadata.project_id = workspace_root.clone();
            metadata.project_name = workspace_root
                .as_deref()
                .map(project_name_for_workspace_root);
            if let Some(project_id) = metadata.project_id.as_ref() {
                project_threads
                    .entry(project_id.clone())
                    .or_default()
                    .push(thread_id.clone());
            } else {
                metadata.git_branch = None;
                metadata.git_dirty = None;
            }
        }

        for project_id in project_threads.keys() {
            let git_branch = git_branch_for_workspace_root(project_id);
            let git_dirty = git_dirty_for_workspace_root(project_id);
            git_state_by_project.insert(project_id.clone(), (git_branch, git_dirty));
        }

        for metadata in self.thread_metadata.values_mut() {
            if let Some(project_id) = metadata.project_id.as_ref() {
                let (git_branch, git_dirty) = git_state_by_project
                    .get(project_id)
                    .cloned()
                    .unwrap_or((None, None));
                metadata.git_branch = git_branch;
                metadata.git_dirty = git_dirty;
            }
        }

        let mut next_registry = HashMap::<String, AutopilotProjectIdentity>::new();
        for (project_id, mut thread_ids) in project_threads {
            thread_ids.sort();
            let metadata = thread_ids
                .iter()
                .filter_map(|thread_id| self.thread_metadata.get(thread_id))
                .find(|metadata| metadata.workspace_root.is_some());
            let Some(metadata) = metadata else {
                continue;
            };
            let defaults = if active_thread_id
                .as_deref()
                .is_some_and(|thread_id| thread_ids.iter().any(|candidate| candidate == thread_id))
            {
                active_thread_id
                    .as_deref()
                    .and_then(|thread_id| self.thread_metadata.get(thread_id))
                    .map(project_defaults_from_thread_metadata)
                    .unwrap_or_default()
            } else {
                previous_registry
                    .get(&project_id)
                    .map(|entry| entry.defaults.clone())
                    .unwrap_or_else(|| project_defaults_from_thread_metadata(metadata))
            };
            let (git_branch, git_dirty) = git_state_by_project
                .get(&project_id)
                .cloned()
                .unwrap_or((None, None));
            next_registry.insert(
                project_id.clone(),
                AutopilotProjectIdentity {
                    project_id,
                    project_name: metadata
                        .project_name
                        .clone()
                        .unwrap_or_else(|| "workspace".to_string()),
                    workspace_root: metadata.workspace_root.clone().unwrap_or_default(),
                    git_branch,
                    git_dirty,
                    thread_ids,
                    defaults,
                },
            );
        }
        self.project_registry = next_registry;
    }

    fn sync_project_defaults_for_thread(&mut self, thread_id: &str) {
        let Some(metadata) = self.thread_metadata.get(thread_id) else {
            return;
        };
        let Some(project_id) = metadata.project_id.as_deref() else {
            return;
        };
        let Some(project) = self.project_registry.get_mut(project_id) else {
            return;
        };
        project.defaults = project_defaults_from_thread_metadata(metadata);
    }

    pub fn refresh_project_registry(&mut self) {
        self.rebuild_project_registry();
    }

    pub fn set_thread_workspace_location(
        &mut self,
        thread_id: &str,
        cwd: Option<String>,
        path: Option<String>,
    ) {
        let Some(metadata) = self.thread_metadata.get_mut(thread_id) else {
            return;
        };
        if let Some(cwd) = cwd {
            let trimmed = cwd.trim();
            if !trimmed.is_empty() {
                metadata.cwd = Some(trimmed.to_string());
            }
        }
        if let Some(path) = path {
            let trimmed = path.trim();
            if !trimmed.is_empty() {
                metadata.path = Some(trimmed.to_string());
            }
        }
        self.rebuild_project_registry();
    }

    pub fn active_terminal_session(&self) -> Option<&AutopilotTerminalSession> {
        let thread_id = self.active_thread_id.as_deref()?;
        self.terminal_sessions.get(thread_id)
    }

    pub fn terminal_session_inventory(&self) -> Vec<&AutopilotTerminalSession> {
        let mut sessions = self.terminal_sessions.values().collect::<Vec<_>>();
        sessions.sort_by(|left, right| {
            right
                .updated_at_epoch_ms
                .cmp(&left.updated_at_epoch_ms)
                .then_with(|| left.thread_id.cmp(&right.thread_id))
        });
        sessions
    }

    pub fn terminal_session_for_thread(
        &self,
        thread_id: &str,
    ) -> Option<&AutopilotTerminalSession> {
        self.terminal_sessions.get(thread_id)
    }

    pub fn prepare_terminal_session(
        &mut self,
        thread_id: &str,
        workspace_root: String,
        shell: String,
        cols: u16,
        rows: u16,
    ) {
        let now = current_epoch_millis_for_state();
        let session = self
            .terminal_sessions
            .entry(thread_id.to_string())
            .or_insert_with(|| AutopilotTerminalSession {
                thread_id: thread_id.to_string(),
                workspace_root: workspace_root.clone(),
                shell: shell.clone(),
                pid: None,
                cols,
                rows,
                status: AutopilotTerminalSessionStatus::Pending,
                exit_code: None,
                lines: Vec::new(),
                created_at_epoch_ms: now,
                updated_at_epoch_ms: now,
                last_error: None,
            });
        session.workspace_root = workspace_root;
        session.shell = shell;
        session.cols = cols;
        session.rows = rows;
        session.status = AutopilotTerminalSessionStatus::Pending;
        session.exit_code = None;
        session.updated_at_epoch_ms = now;
        session.last_error = None;
    }

    pub fn record_terminal_session_opened(
        &mut self,
        thread_id: &str,
        workspace_root: String,
        shell: String,
        pid: u32,
        cols: u16,
        rows: u16,
    ) {
        let now = current_epoch_millis_for_state();
        let session = self
            .terminal_sessions
            .entry(thread_id.to_string())
            .or_insert_with(|| AutopilotTerminalSession {
                thread_id: thread_id.to_string(),
                workspace_root: workspace_root.clone(),
                shell: shell.clone(),
                pid: Some(pid),
                cols,
                rows,
                status: AutopilotTerminalSessionStatus::Running,
                exit_code: None,
                lines: Vec::new(),
                created_at_epoch_ms: now,
                updated_at_epoch_ms: now,
                last_error: None,
            });
        session.workspace_root = workspace_root;
        session.shell = shell;
        session.pid = Some(pid);
        session.cols = cols;
        session.rows = rows;
        session.status = AutopilotTerminalSessionStatus::Running;
        session.exit_code = None;
        session.updated_at_epoch_ms = now;
        session.last_error = None;
    }

    pub fn append_terminal_session_output(
        &mut self,
        thread_id: &str,
        stream: TerminalStream,
        text: impl Into<String>,
    ) {
        let now = current_epoch_millis_for_state();
        let text = text.into();
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return;
        }
        let fallback_workspace = self
            .thread_metadata
            .get(thread_id)
            .and_then(|metadata| metadata.workspace_root.clone())
            .or_else(|| {
                self.thread_metadata
                    .get(thread_id)
                    .and_then(|metadata| metadata.cwd.clone())
            })
            .unwrap_or_default();
        let session = self
            .terminal_sessions
            .entry(thread_id.to_string())
            .or_insert_with(|| AutopilotTerminalSession {
                thread_id: thread_id.to_string(),
                workspace_root: fallback_workspace,
                shell: String::new(),
                pid: None,
                cols: 120,
                rows: 32,
                status: AutopilotTerminalSessionStatus::Running,
                exit_code: None,
                lines: Vec::new(),
                created_at_epoch_ms: now,
                updated_at_epoch_ms: now,
                last_error: None,
            });
        session
            .lines
            .push(TerminalLine::new(stream, trimmed.to_string()));
        if session.lines.len() > 256 {
            let drop_count = session.lines.len().saturating_sub(256);
            session.lines.drain(0..drop_count);
        }
        session.updated_at_epoch_ms = now;
    }

    pub fn resize_terminal_session(&mut self, thread_id: &str, cols: u16, rows: u16) {
        let Some(session) = self.terminal_sessions.get_mut(thread_id) else {
            return;
        };
        session.cols = cols;
        session.rows = rows;
        session.updated_at_epoch_ms = current_epoch_millis_for_state();
    }

    pub fn clear_terminal_session_output(&mut self, thread_id: &str) {
        let Some(session) = self.terminal_sessions.get_mut(thread_id) else {
            return;
        };
        session.lines.clear();
        session.updated_at_epoch_ms = current_epoch_millis_for_state();
    }

    pub fn record_terminal_session_closed(
        &mut self,
        thread_id: &str,
        exit_code: Option<i32>,
        reason: Option<String>,
    ) {
        let Some(session) = self.terminal_sessions.get_mut(thread_id) else {
            return;
        };
        session.pid = None;
        session.exit_code = exit_code;
        session.status = if reason.is_some() {
            AutopilotTerminalSessionStatus::Failed
        } else if exit_code.is_some() {
            AutopilotTerminalSessionStatus::Exited
        } else {
            AutopilotTerminalSessionStatus::Closed
        };
        session.last_error = reason.clone();
        session.updated_at_epoch_ms = current_epoch_millis_for_state();
        if let Some(reason) = reason {
            session
                .lines
                .push(TerminalLine::new(TerminalStream::Stderr, reason));
        }
    }

    pub fn record_terminal_session_failure(&mut self, thread_id: &str, error: String) {
        let now = current_epoch_millis_for_state();
        let fallback_workspace = self
            .thread_metadata
            .get(thread_id)
            .and_then(|metadata| metadata.workspace_root.clone())
            .or_else(|| {
                self.thread_metadata
                    .get(thread_id)
                    .and_then(|metadata| metadata.cwd.clone())
            })
            .unwrap_or_default();
        let session = self
            .terminal_sessions
            .entry(thread_id.to_string())
            .or_insert_with(|| AutopilotTerminalSession {
                thread_id: thread_id.to_string(),
                workspace_root: fallback_workspace,
                shell: String::new(),
                pid: None,
                cols: 120,
                rows: 32,
                status: AutopilotTerminalSessionStatus::Failed,
                exit_code: None,
                lines: Vec::new(),
                created_at_epoch_ms: now,
                updated_at_epoch_ms: now,
                last_error: Some(error.clone()),
            });
        session.status = AutopilotTerminalSessionStatus::Failed;
        session.last_error = Some(error.clone());
        session.updated_at_epoch_ms = now;
        session
            .lines
            .push(TerminalLine::new(TerminalStream::Stderr, error));
        if session.lines.len() > 256 {
            let drop_count = session.lines.len().saturating_sub(256);
            session.lines.drain(0..drop_count);
        }
    }

    pub fn remove_inactive_terminal_sessions(&mut self) -> usize {
        let before = self.terminal_sessions.len();
        self.terminal_sessions
            .retain(|_, session| session.status.is_active());
        before.saturating_sub(self.terminal_sessions.len())
    }

    pub fn active_plan_artifact(&self) -> Option<&AutopilotPlanArtifact> {
        self.active_thread_id
            .as_deref()
            .and_then(|thread_id| self.thread_plan_artifacts.get(thread_id))
    }

    pub fn active_diff_artifact(&self) -> Option<&AutopilotDiffArtifact> {
        self.active_thread_id
            .as_deref()
            .and_then(|thread_id| self.thread_diff_artifacts.get(thread_id))
            .and_then(|artifacts| artifacts.first())
    }

    pub fn active_review_artifact(&self) -> Option<&AutopilotReviewArtifact> {
        self.active_thread_id
            .as_deref()
            .and_then(|thread_id| self.thread_review_artifacts.get(thread_id))
    }

    pub fn active_compaction_artifact(&self) -> Option<&AutopilotCompactionArtifact> {
        self.active_thread_id
            .as_deref()
            .and_then(|thread_id| self.thread_compaction_artifacts.get(thread_id))
    }

    pub fn record_composer_draft(&mut self, draft: impl Into<String>) {
        let draft = draft.into();
        if let Some(thread_id) = self.active_thread_id.clone() {
            if draft.is_empty() {
                self.thread_composer_drafts.remove(&thread_id);
            } else {
                self.thread_composer_drafts.insert(thread_id, draft);
            }
            return;
        }
        self.detached_composer_draft = draft;
    }

    pub fn active_composer_draft(&self) -> &str {
        if let Some(thread_id) = self.active_thread_id.as_ref() {
            return self
                .thread_composer_drafts
                .get(thread_id)
                .map(String::as_str)
                .unwrap_or("");
        }
        self.detached_composer_draft.as_str()
    }

    pub fn adopt_detached_composer_draft(&mut self, thread_id: &str) {
        if self.detached_composer_draft.trim().is_empty() {
            return;
        }
        let draft = self.detached_composer_draft.clone();
        self.thread_composer_drafts
            .entry(thread_id.to_string())
            .or_insert(draft);
    }

    pub fn remember_submission_draft(&mut self, thread_id: &str, draft: impl Into<String>) {
        let draft = draft.into();
        if draft.trim().is_empty() {
            return;
        }
        let history = self
            .thread_submission_history
            .entry(thread_id.to_string())
            .or_default();
        history.push_front(draft);
        if history.len() > 16 {
            history.truncate(16);
        }
    }

    pub fn last_submission_draft(&self, thread_id: &str) -> Option<&str> {
        self.thread_submission_history
            .get(thread_id)
            .and_then(|history| history.front())
            .map(String::as_str)
    }

    pub fn enqueue_pending_steer_submission(
        &mut self,
        command_seq: u64,
        thread_id: impl Into<String>,
        prompt: impl Into<String>,
    ) {
        self.pending_steer_submissions
            .push_back(AutopilotPendingSteerSubmission {
                command_seq,
                thread_id: thread_id.into(),
                prompt: prompt.into(),
            });
        if self.pending_steer_submissions.len() > 32 {
            let overflow = self.pending_steer_submissions.len().saturating_sub(32);
            self.pending_steer_submissions.drain(0..overflow);
        }
    }

    pub fn take_pending_steer_submission(&mut self, command_seq: u64) -> Option<(String, String)> {
        let index = self
            .pending_steer_submissions
            .iter()
            .position(|pending| pending.command_seq == command_seq)?;
        let pending = self.pending_steer_submissions.remove(index)?;
        Some((pending.thread_id, pending.prompt))
    }

    pub fn set_models(&mut self, models: Vec<String>, default_model: Option<String>) {
        let sanitized = models
            .into_iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        if sanitized.is_empty() {
            return;
        }

        let previous_model = self.models.get(self.selected_model).cloned();
        self.models = sanitized;

        // Preserve an explicit prior user selection first.
        if let Some(previous_model) = previous_model.as_ref()
            && !previous_model.eq_ignore_ascii_case("auto")
            && let Some(index) = self.models.iter().position(|model| model == previous_model)
        {
            self.selected_model = index;
            self.last_error = None;
            return;
        }

        // Prefer current high-quality coding models when available.
        let preferred_models = [
            "gpt-5.2-codex",
            "gpt-5.1-codex-max",
            "gpt-5.2",
            "gpt-5.3-codex",
            "gpt-5.3-codex-spark",
        ];
        if let Some(index) = preferred_models
            .iter()
            .find_map(|model| self.models.iter().position(|value| value == model))
        {
            self.selected_model = index;
            self.last_error = None;
            return;
        }

        if let Some(default_model) = default_model.as_ref()
            && let Some(index) = self.models.iter().position(|model| model == default_model)
        {
            self.selected_model = index;
            self.last_error = None;
            return;
        }

        self.selected_model = 0;
        self.last_error = None;
    }

    pub fn cycle_model(&mut self) {
        if self.models.is_empty() {
            return;
        }
        self.selected_model = (self.selected_model + 1) % self.models.len();
        self.record_active_session_preferences();
        self.last_error = None;
    }

    pub fn select_or_insert_model(&mut self, model: impl AsRef<str>) {
        let value = model.as_ref().trim();
        if value.is_empty() {
            return;
        }
        if let Some(index) = self.models.iter().position(|candidate| candidate == value) {
            self.selected_model = index;
        } else {
            self.models.push(value.to_string());
            self.selected_model = self.models.len().saturating_sub(1);
        }
        self.record_active_session_preferences();
        self.last_error = None;
    }

    pub fn set_reasoning_effort(&mut self, reasoning_effort: Option<String>) {
        self.reasoning_effort = reasoning_effort
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty());
        self.record_active_session_preferences();
        self.last_error = None;
    }

    pub fn cycle_reasoning_effort(&mut self, supported_efforts: &[String]) {
        let fallback = [
            "minimal".to_string(),
            "low".to_string(),
            "medium".to_string(),
            "high".to_string(),
        ];
        let options = if supported_efforts.is_empty() {
            fallback.as_slice()
        } else {
            supported_efforts
        };
        if options.is_empty() {
            return;
        }
        let current = self
            .reasoning_effort
            .as_deref()
            .unwrap_or("medium")
            .trim()
            .to_ascii_lowercase();
        let next_index = options
            .iter()
            .position(|value| value.eq_ignore_ascii_case(current.as_str()))
            .map(|index| (index + 1) % options.len())
            .unwrap_or(0);
        self.set_reasoning_effort(options.get(next_index).cloned());
    }

    pub fn cycle_service_tier(&mut self) {
        self.service_tier = self.service_tier.next();
        self.record_active_session_preferences();
        self.last_error = None;
    }

    pub fn set_service_tier(&mut self, service_tier: AutopilotChatServiceTier) {
        self.service_tier = service_tier;
        self.record_active_session_preferences();
        self.last_error = None;
    }

    pub fn cycle_personality(&mut self) {
        self.personality = self.personality.next();
        self.record_active_session_preferences();
        self.last_error = None;
    }

    pub fn set_personality(&mut self, personality: AutopilotChatPersonality) {
        self.personality = personality;
        self.record_active_session_preferences();
        self.last_error = None;
    }

    pub fn cycle_collaboration_mode(&mut self) {
        self.collaboration_mode = self.collaboration_mode.next();
        self.record_active_session_preferences();
        self.last_error = None;
    }

    pub fn set_collaboration_mode(&mut self, collaboration_mode: AutopilotChatCollaborationMode) {
        self.collaboration_mode = collaboration_mode;
        self.record_active_session_preferences();
        self.last_error = None;
    }

    pub fn cycle_approval_mode(&mut self) {
        self.approval_mode = match self.approval_mode {
            codex_client::AskForApproval::Never => codex_client::AskForApproval::OnFailure,
            codex_client::AskForApproval::OnFailure => codex_client::AskForApproval::OnRequest,
            codex_client::AskForApproval::OnRequest => codex_client::AskForApproval::UnlessTrusted,
            codex_client::AskForApproval::UnlessTrusted
            | codex_client::AskForApproval::Reject { .. } => codex_client::AskForApproval::Never,
        };
        self.record_active_session_preferences();
        self.last_error = None;
    }

    pub fn set_approval_mode(&mut self, approval_mode: codex_client::AskForApproval) {
        self.approval_mode = approval_mode;
        self.record_active_session_preferences();
        self.last_error = None;
    }

    pub fn cycle_sandbox_mode(&mut self) {
        self.sandbox_mode = match self.sandbox_mode {
            codex_client::SandboxMode::DangerFullAccess => {
                codex_client::SandboxMode::WorkspaceWrite
            }
            codex_client::SandboxMode::WorkspaceWrite => codex_client::SandboxMode::ReadOnly,
            codex_client::SandboxMode::ReadOnly => codex_client::SandboxMode::DangerFullAccess,
        };
        self.record_active_session_preferences();
        self.last_error = None;
    }

    pub fn set_sandbox_mode(&mut self, sandbox_mode: codex_client::SandboxMode) {
        self.sandbox_mode = sandbox_mode;
        self.record_active_session_preferences();
        self.last_error = None;
    }

    fn record_active_session_preferences(&mut self) {
        let Some(thread_id) = self.active_thread_id.clone() else {
            return;
        };
        let selected_model = self.selected_model_override();
        let reasoning_effort = self.reasoning_effort.clone();
        let service_tier = self.service_tier;
        let approval_mode = self.approval_mode;
        let sandbox_mode = self.sandbox_mode;
        let personality = self.personality;
        let collaboration_mode = self.collaboration_mode;
        if let Some(metadata) = self.thread_metadata.get_mut(&thread_id) {
            metadata.model = selected_model;
            metadata.service_tier = service_tier;
            metadata.reasoning_effort = reasoning_effort;
            metadata.approval_policy = Some(approval_mode);
            metadata.sandbox_mode = Some(sandbox_mode);
            metadata.personality = personality;
            metadata.collaboration_mode = collaboration_mode;
        }
        self.sync_project_defaults_for_thread(&thread_id);
    }

    pub fn apply_thread_session_configuration(
        &mut self,
        thread_id: &str,
        model: Option<String>,
        cwd: Option<String>,
        approval_policy: Option<codex_client::AskForApproval>,
        sandbox_mode: Option<codex_client::SandboxMode>,
        service_tier: Option<codex_client::ServiceTier>,
        reasoning_effort: Option<String>,
    ) {
        let service_tier_selection = AutopilotChatServiceTier::from_response(service_tier);
        let normalized_model = model
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        let normalized_effort = reasoning_effort
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_ascii_lowercase());
        let cwd_changed = cwd.is_some();

        {
            let metadata = self
                .thread_metadata
                .entry(thread_id.to_string())
                .or_default();
            if let Some(value) = normalized_model.as_ref() {
                metadata.model = Some(value.clone());
            }
            if let Some(value) = cwd {
                metadata.cwd = Some(value);
            }
            if let Some(value) = normalized_effort.as_ref() {
                metadata.reasoning_effort = Some(value.clone());
            }
            if let Some(value) = approval_policy {
                metadata.approval_policy = Some(value);
            }
            if let Some(value) = sandbox_mode {
                metadata.sandbox_mode = Some(value);
            }
            metadata.service_tier = service_tier_selection;
        }

        if cwd_changed {
            self.rebuild_project_registry();
        } else {
            self.sync_project_defaults_for_thread(thread_id);
        }

        if self.is_active_thread(thread_id) {
            if let Some(value) = normalized_model.as_ref() {
                self.select_or_insert_model(value);
            }
            self.service_tier = service_tier_selection;
            if let Some(value) = normalized_effort {
                self.reasoning_effort = Some(value);
            }
            if let Some(value) = approval_policy {
                self.approval_mode = value;
            }
            if let Some(value) = sandbox_mode {
                self.sandbox_mode = value;
            }
            self.record_active_session_preferences();
        }
    }

    pub fn restore_session_preferences_from_thread(&mut self, thread_id: &str) {
        let Some(metadata) = self.thread_metadata.get(thread_id).cloned() else {
            return;
        };
        if let Some(model) = metadata.model.as_ref() {
            self.select_or_insert_model(model);
        }
        self.service_tier = metadata.service_tier;
        self.reasoning_effort = metadata.reasoning_effort.clone();
        if let Some(value) = metadata.approval_policy {
            self.approval_mode = value;
        }
        if let Some(value) = metadata.sandbox_mode {
            self.sandbox_mode = value;
        }
        self.personality = metadata.personality;
        self.collaboration_mode = metadata.collaboration_mode;
        self.record_active_session_preferences();
    }

    pub fn set_connection_status(&mut self, status: impl Into<String>) {
        self.connection_status = status.into();
    }

    pub fn has_managed_chat_browseable_content(&self) -> bool {
        !self.managed_chat_projection.snapshot.groups.is_empty()
            && !self.managed_chat_projection.snapshot.channels.is_empty()
    }

    /// Auto-selects the first managed chat workspace when no workspace is selected yet.
    /// No-op if the user has already selected a workspace or if there is no content.
    pub fn maybe_auto_select_default_nip28_channel(&mut self) -> bool {
        if self.selected_workspace != ChatWorkspaceSelection::Autopilot {
            return false;
        }
        if !self.has_managed_chat_browseable_content() {
            return false;
        }
        self.select_chat_workspace_by_index(0)
    }

    pub fn has_direct_message_browseable_content(&self) -> bool {
        !self.direct_message_projection.snapshot.rooms.is_empty()
    }

    pub fn chat_has_browseable_content(&self) -> bool {
        self.has_managed_chat_browseable_content() || self.has_direct_message_browseable_content()
    }

    pub fn chat_browse_mode(&self) -> ChatBrowseMode {
        match &self.selected_workspace {
            ChatWorkspaceSelection::ManagedGroup(group_id)
                if self
                    .managed_chat_projection
                    .snapshot
                    .groups
                    .iter()
                    .any(|group| group.group_id == *group_id) =>
            {
                return ChatBrowseMode::Managed;
            }
            ChatWorkspaceSelection::DirectMessages
                if self.has_direct_message_browseable_content() =>
            {
                return ChatBrowseMode::DirectMessages;
            }
            ChatWorkspaceSelection::Autopilot => {}
            _ => {}
        }

        if self.has_managed_chat_browseable_content() {
            ChatBrowseMode::Managed
        } else if self.has_direct_message_browseable_content() {
            ChatBrowseMode::DirectMessages
        } else {
            ChatBrowseMode::Autopilot
        }
    }

    pub fn chat_workspace_entries(&self) -> Vec<ChatWorkspaceSelection> {
        let mut entries = self
            .managed_chat_projection
            .snapshot
            .groups
            .iter()
            .map(|group| ChatWorkspaceSelection::ManagedGroup(group.group_id.clone()))
            .collect::<Vec<_>>();
        if self.has_direct_message_browseable_content() {
            entries.push(ChatWorkspaceSelection::DirectMessages);
        }
        if entries.is_empty() {
            entries.push(ChatWorkspaceSelection::Autopilot);
        }
        entries
    }

    pub fn select_chat_workspace_by_index(&mut self, index: usize) -> bool {
        let Some(selection) = self.chat_workspace_entries().get(index).cloned() else {
            return false;
        };

        match selection {
            ChatWorkspaceSelection::ManagedGroup(group_id) => {
                match self.managed_chat_projection.set_selected_group(&group_id) {
                    Ok(()) => {
                        self.selected_workspace = ChatWorkspaceSelection::ManagedGroup(group_id);
                        self.reset_transcript_scroll();
                        self.last_error = None;
                        true
                    }
                    Err(error) => {
                        self.last_error = Some(error);
                        false
                    }
                }
            }
            ChatWorkspaceSelection::DirectMessages => {
                let room_id = self
                    .direct_message_projection
                    .local_state
                    .selected_room_id
                    .clone()
                    .filter(|room_id| {
                        self.direct_message_projection
                            .snapshot
                            .rooms
                            .iter()
                            .any(|room| room.room_id == *room_id)
                    })
                    .or_else(|| {
                        self.direct_message_projection
                            .snapshot
                            .rooms
                            .first()
                            .map(|room| room.room_id.clone())
                    });
                if let Some(room_id) = room_id {
                    match self.direct_message_projection.set_selected_room(&room_id) {
                        Ok(()) => {
                            self.selected_workspace = ChatWorkspaceSelection::DirectMessages;
                            self.reset_transcript_scroll();
                            self.last_error = None;
                            true
                        }
                        Err(error) => {
                            self.last_error = Some(error);
                            false
                        }
                    }
                } else {
                    self.selected_workspace = ChatWorkspaceSelection::DirectMessages;
                    self.reset_transcript_scroll();
                    self.last_error = None;
                    true
                }
            }
            ChatWorkspaceSelection::Autopilot => {
                self.selected_workspace = ChatWorkspaceSelection::Autopilot;
                self.reset_transcript_scroll();
                self.last_error = None;
                true
            }
        }
    }

    pub fn active_managed_chat_group(&self) -> Option<&ManagedChatGroupProjection> {
        if self.chat_browse_mode() != ChatBrowseMode::Managed {
            return None;
        }
        let selected_workspace_group_id = match &self.selected_workspace {
            ChatWorkspaceSelection::ManagedGroup(group_id) => Some(group_id.as_str()),
            _ => None,
        };
        if let Some(selected_group_id) = selected_workspace_group_id
            && let Some(group) = self
                .managed_chat_projection
                .snapshot
                .groups
                .iter()
                .find(|group| group.group_id == selected_group_id)
        {
            return Some(group);
        }
        if let Some(selected_group_id) = self
            .managed_chat_projection
            .local_state
            .selected_group_id
            .as_deref()
            && let Some(group) = self
                .managed_chat_projection
                .snapshot
                .groups
                .iter()
                .find(|group| group.group_id == selected_group_id)
        {
            return Some(group);
        }
        self.managed_chat_projection.snapshot.groups.first()
    }

    pub fn managed_chat_local_pubkey(&self) -> Option<&str> {
        self.managed_chat_projection.local_pubkey()
    }

    pub fn active_managed_chat_local_member(&self) -> Option<&ManagedChatMemberProjection> {
        let group = self.active_managed_chat_group()?;
        let local_pubkey = self.managed_chat_local_pubkey()?;
        group
            .members
            .iter()
            .find(|member| member.pubkey == local_pubkey)
    }

    pub fn active_managed_chat_local_is_admin(&self) -> bool {
        self.active_managed_chat_local_member()
            .is_some_and(|member| member.is_admin)
    }

    pub fn managed_chat_member_is_locally_muted(&self, pubkey: &str) -> bool {
        self.managed_chat_projection.is_pubkey_muted(pubkey)
    }

    pub fn active_managed_chat_channel(&self) -> Option<&ManagedChatChannelProjection> {
        if self.chat_browse_mode() != ChatBrowseMode::Managed {
            return None;
        }
        let active_group = self.active_managed_chat_group()?;
        if let Some(selected_channel_id) = self
            .managed_chat_projection
            .local_state
            .selected_channel_id
            .as_deref()
            && let Some(channel) =
                self.managed_chat_projection
                    .snapshot
                    .channels
                    .iter()
                    .find(|channel| {
                        channel.channel_id == selected_channel_id
                            && channel.group_id == active_group.group_id
                    })
        {
            return Some(channel);
        }
        active_group.channel_ids.first().and_then(|channel_id| {
            self.managed_chat_projection
                .snapshot
                .channels
                .iter()
                .find(|channel| channel.channel_id == *channel_id)
        })
    }

    pub fn active_managed_chat_channels(&self) -> Vec<&ManagedChatChannelProjection> {
        if self.chat_browse_mode() != ChatBrowseMode::Managed {
            return Vec::new();
        }
        let Some(active_group) = self.active_managed_chat_group() else {
            return Vec::new();
        };
        active_group
            .channel_ids
            .iter()
            .filter_map(|channel_id| {
                self.managed_chat_projection
                    .snapshot
                    .channels
                    .iter()
                    .find(|channel| channel.channel_id == *channel_id)
            })
            .collect()
    }

    pub fn active_managed_chat_messages(&self) -> Vec<&ManagedChatMessageProjection> {
        if self.chat_browse_mode() != ChatBrowseMode::Managed {
            return Vec::new();
        }
        let Some(active_channel) = self.active_managed_chat_channel() else {
            return Vec::new();
        };
        active_channel
            .message_ids
            .iter()
            .filter_map(|message_id| {
                self.managed_chat_projection
                    .snapshot
                    .messages
                    .get(message_id)
            })
            .collect()
    }

    pub fn configured_main_managed_chat_channel(
        &self,
        config: &DefaultNip28ChannelConfig,
    ) -> Option<&ManagedChatChannelProjection> {
        if !config.is_valid() {
            return None;
        }
        self.managed_chat_projection
            .snapshot
            .channels
            .iter()
            .find(|channel| channel.channel_id == config.channel_id)
    }

    pub(crate) fn autopilot_peer_roster(
        &self,
        now_epoch_seconds: u64,
    ) -> Vec<crate::autopilot_peer_roster::AutopilotPeerRosterRow> {
        self.autopilot_peer_roster_with_config(
            &DefaultNip28ChannelConfig::from_env_or_default(),
            now_epoch_seconds,
        )
    }

    pub(crate) fn autopilot_peer_roster_with_config(
        &self,
        config: &DefaultNip28ChannelConfig,
        now_epoch_seconds: u64,
    ) -> Vec<crate::autopilot_peer_roster::AutopilotPeerRosterRow> {
        crate::autopilot_peer_roster::build_autopilot_peer_roster(
            &self.managed_chat_projection.snapshot,
            &self.managed_chat_projection.local_state,
            self.managed_chat_local_pubkey(),
            config,
            now_epoch_seconds,
        )
    }

    pub(crate) fn select_autopilot_buy_mode_target(
        &self,
        now_epoch_seconds: u64,
    ) -> crate::autopilot_peer_roster::AutopilotBuyModeTargetSelection {
        self.select_autopilot_buy_mode_target_with_config(
            &DefaultNip28ChannelConfig::from_env_or_default(),
            now_epoch_seconds,
        )
    }

    pub(crate) fn select_autopilot_buy_mode_target_with_config(
        &self,
        config: &DefaultNip28ChannelConfig,
        now_epoch_seconds: u64,
    ) -> crate::autopilot_peer_roster::AutopilotBuyModeTargetSelection {
        crate::autopilot_peer_roster::select_autopilot_buy_mode_target_with_policy(
            &self.managed_chat_projection.snapshot,
            &self.managed_chat_projection.local_state,
            self.managed_chat_local_pubkey(),
            config,
            now_epoch_seconds,
            self.buy_mode_last_targeted_peer_pubkey.as_deref(),
        )
    }

    pub(crate) fn note_buy_mode_target_dispatch(&mut self, provider_pubkey: &str) {
        let normalized = provider_pubkey.trim();
        if normalized.is_empty() {
            return;
        }
        self.buy_mode_last_targeted_peer_pubkey = Some(normalized.to_string());
    }

    pub fn active_managed_chat_channel_rail_rows(&self) -> Vec<ManagedChatChannelRailRow> {
        if self.chat_browse_mode() != ChatBrowseMode::Managed {
            return Vec::new();
        }
        let Some(active_group) = self.active_managed_chat_group() else {
            return Vec::new();
        };
        let active_channels = self.active_managed_chat_channels();
        let mut rows = Vec::new();
        let mut current_category_id = None::<String>;

        for channel in active_channels {
            let category_id = channel
                .hints
                .category_id
                .clone()
                .unwrap_or_else(|| MANAGED_CHAT_UNCATEGORIZED_CATEGORY_ID.to_string());
            if current_category_id.as_deref() != Some(category_id.as_str()) {
                let channel_count = active_group
                    .channel_ids
                    .iter()
                    .filter_map(|channel_id| {
                        self.managed_chat_projection
                            .snapshot
                            .channels
                            .iter()
                            .find(|channel| channel.channel_id == *channel_id)
                    })
                    .filter(|candidate| {
                        candidate
                            .hints
                            .category_id
                            .as_deref()
                            .unwrap_or(MANAGED_CHAT_UNCATEGORIZED_CATEGORY_ID)
                            == category_id
                    })
                    .count();
                let unread_count = active_group
                    .channel_ids
                    .iter()
                    .filter_map(|channel_id| {
                        self.managed_chat_projection
                            .snapshot
                            .channels
                            .iter()
                            .find(|channel| channel.channel_id == *channel_id)
                    })
                    .filter(|candidate| {
                        candidate
                            .hints
                            .category_id
                            .as_deref()
                            .unwrap_or(MANAGED_CHAT_UNCATEGORIZED_CATEGORY_ID)
                            == category_id
                    })
                    .map(|channel| channel.unread_count)
                    .sum();
                let mention_count = active_group
                    .channel_ids
                    .iter()
                    .filter_map(|channel_id| {
                        self.managed_chat_projection
                            .snapshot
                            .channels
                            .iter()
                            .find(|channel| channel.channel_id == *channel_id)
                    })
                    .filter(|candidate| {
                        candidate
                            .hints
                            .category_id
                            .as_deref()
                            .unwrap_or(MANAGED_CHAT_UNCATEGORIZED_CATEGORY_ID)
                            == category_id
                    })
                    .map(|channel| channel.mention_count)
                    .sum();
                rows.push(ManagedChatChannelRailRow::Category {
                    label: channel
                        .hints
                        .category_label
                        .clone()
                        .filter(|value| !value.trim().is_empty())
                        .unwrap_or_else(|| {
                            if category_id == MANAGED_CHAT_UNCATEGORIZED_CATEGORY_ID {
                                "General".to_string()
                            } else {
                                category_id.clone()
                            }
                        }),
                    category_id: category_id.clone(),
                    collapsed: self
                        .managed_chat_projection
                        .category_is_collapsed(&active_group.group_id, &category_id),
                    channel_count,
                    unread_count,
                    mention_count,
                });
                current_category_id = Some(category_id.clone());
            }

            if !self
                .managed_chat_projection
                .category_is_collapsed(&active_group.group_id, &category_id)
            {
                rows.push(ManagedChatChannelRailRow::Channel {
                    channel_id: channel.channel_id.clone(),
                });
            }
        }

        rows
    }

    pub fn active_managed_chat_retryable_message(&self) -> Option<&ManagedChatMessageProjection> {
        if self.chat_browse_mode() != ChatBrowseMode::Managed {
            return None;
        }
        self.active_managed_chat_messages()
            .into_iter()
            .rev()
            .find(|message| message.delivery_state.is_retryable())
    }

    pub fn managed_chat_can_send(&self, composer_value: &str) -> bool {
        if !composer_value.trim().is_empty() {
            return self
                .active_managed_chat_channel()
                .and_then(|channel| channel.relay_url.as_deref())
                .is_some();
        }
        self.active_managed_chat_retryable_message().is_some()
    }

    pub fn active_direct_message_room(&self) -> Option<&DirectMessageRoomProjection> {
        if self.chat_browse_mode() != ChatBrowseMode::DirectMessages {
            return None;
        }
        if let Some(selected_room_id) = self
            .direct_message_projection
            .local_state
            .selected_room_id
            .as_deref()
            && let Some(room) = self
                .direct_message_projection
                .snapshot
                .rooms
                .iter()
                .find(|room| room.room_id == selected_room_id)
        {
            return Some(room);
        }
        self.direct_message_projection.snapshot.rooms.first()
    }

    pub fn active_direct_message_rooms(&self) -> Vec<&DirectMessageRoomProjection> {
        if self.chat_browse_mode() != ChatBrowseMode::DirectMessages {
            return Vec::new();
        }
        self.direct_message_projection
            .snapshot
            .rooms
            .iter()
            .collect()
    }

    pub fn active_direct_message_messages(&self) -> Vec<&DirectMessageMessageProjection> {
        let Some(active_room) = self.active_direct_message_room() else {
            return Vec::new();
        };
        active_room
            .message_ids
            .iter()
            .filter_map(|message_id| {
                self.direct_message_projection
                    .snapshot
                    .messages
                    .get(message_id)
            })
            .collect()
    }

    pub fn active_direct_message_retryable_message(
        &self,
    ) -> Option<&DirectMessageMessageProjection> {
        self.active_direct_message_messages()
            .into_iter()
            .rev()
            .find(|message| message.delivery_state.is_retryable())
    }

    pub fn direct_message_can_send(&self, composer_value: &str) -> bool {
        if !composer_value.trim().is_empty() {
            return true;
        }
        self.active_direct_message_retryable_message().is_some()
    }

    pub fn select_managed_chat_group_by_index(&mut self, index: usize) -> bool {
        let Some(group_id) = self
            .managed_chat_projection
            .snapshot
            .groups
            .get(index)
            .map(|group| group.group_id.clone())
        else {
            return false;
        };
        match self.managed_chat_projection.set_selected_group(&group_id) {
            Ok(()) => {
                self.selected_workspace = ChatWorkspaceSelection::ManagedGroup(group_id);
                self.reset_transcript_scroll();
                self.last_error = None;
                true
            }
            Err(error) => {
                self.last_error = Some(error);
                false
            }
        }
    }

    pub fn select_managed_chat_group_by_id(&mut self, group_id: &str) -> bool {
        match self.managed_chat_projection.set_selected_group(group_id) {
            Ok(()) => {
                self.selected_workspace =
                    ChatWorkspaceSelection::ManagedGroup(group_id.to_string());
                self.reset_transcript_scroll();
                self.last_error = None;
                true
            }
            Err(error) => {
                self.last_error = Some(error);
                false
            }
        }
    }

    pub fn select_managed_chat_channel_by_index(&mut self, index: usize) -> bool {
        let Some(group_id) = self
            .active_managed_chat_group()
            .map(|group| group.group_id.clone())
        else {
            return false;
        };
        let Some(channel_id) = self
            .active_managed_chat_channels()
            .get(index)
            .map(|channel| channel.channel_id.clone())
        else {
            return false;
        };
        match self
            .managed_chat_projection
            .set_selected_channel(&group_id, &channel_id)
        {
            Ok(()) => {
                self.selected_workspace = ChatWorkspaceSelection::ManagedGroup(group_id);
                self.reset_transcript_scroll();
                self.last_error = None;
                true
            }
            Err(error) => {
                self.last_error = Some(error);
                false
            }
        }
    }

    pub fn select_managed_chat_channel_row_by_index(&mut self, index: usize) -> bool {
        let Some(group_id) = self
            .active_managed_chat_group()
            .map(|group| group.group_id.clone())
        else {
            return false;
        };
        let Some(channel_id) = self
            .active_managed_chat_channel_rail_rows()
            .get(index)
            .and_then(|row| match row {
                ManagedChatChannelRailRow::Channel { channel_id } => Some(channel_id.clone()),
                ManagedChatChannelRailRow::Category { .. } => None,
            })
        else {
            return false;
        };
        match self
            .managed_chat_projection
            .set_selected_channel(&group_id, &channel_id)
        {
            Ok(()) => {
                self.selected_workspace = ChatWorkspaceSelection::ManagedGroup(group_id);
                self.reset_transcript_scroll();
                self.last_error = None;
                true
            }
            Err(error) => {
                self.last_error = Some(error);
                false
            }
        }
    }

    pub fn toggle_managed_chat_category_by_row_index(&mut self, index: usize) -> bool {
        let Some(group_id) = self
            .active_managed_chat_group()
            .map(|group| group.group_id.clone())
        else {
            return false;
        };
        let Some(category_id) = self
            .active_managed_chat_channel_rail_rows()
            .get(index)
            .and_then(|row| match row {
                ManagedChatChannelRailRow::Category { category_id, .. } => {
                    Some(category_id.clone())
                }
                ManagedChatChannelRailRow::Channel { .. } => None,
            })
        else {
            return false;
        };
        match self
            .managed_chat_projection
            .toggle_category_collapsed(&group_id, &category_id)
        {
            Ok(()) => {
                self.last_error = None;
                true
            }
            Err(error) => {
                self.last_error = Some(error);
                false
            }
        }
    }

    pub fn select_direct_message_room_by_index(&mut self, index: usize) -> bool {
        let Some(room_id) = self
            .direct_message_projection
            .snapshot
            .rooms
            .get(index)
            .map(|room| room.room_id.clone())
        else {
            return false;
        };
        match self.direct_message_projection.set_selected_room(&room_id) {
            Ok(()) => {
                self.selected_workspace = ChatWorkspaceSelection::DirectMessages;
                self.reset_transcript_scroll();
                self.last_error = None;
                true
            }
            Err(error) => {
                self.last_error = Some(error);
                false
            }
        }
    }

    pub fn set_thread_entries(&mut self, entries: Vec<AutopilotThreadListEntry>) {
        let previous_active_thread_id = self.active_thread_id.clone();
        let previous_metadata = self.thread_metadata.clone();
        self.threads = entries
            .iter()
            .map(|entry| entry.thread_id.clone())
            .collect();
        self.thread_metadata.clear();
        for entry in entries {
            let previous = previous_metadata
                .get(&entry.thread_id)
                .cloned()
                .unwrap_or_default();
            self.thread_metadata.insert(
                entry.thread_id.clone(),
                AutopilotThreadMetadata {
                    thread_name: entry.thread_name,
                    preview: if entry.preview.trim().is_empty() {
                        previous.preview.clone()
                    } else {
                        Some(entry.preview)
                    },
                    status: entry.status,
                    loaded: entry.loaded,
                    cwd: entry.cwd.or(previous.cwd.clone()),
                    path: entry.path.or(previous.path.clone()),
                    workspace_root: previous.workspace_root.clone(),
                    project_id: previous.project_id.clone(),
                    project_name: previous.project_name.clone(),
                    git_branch: previous.git_branch.clone(),
                    git_dirty: previous.git_dirty,
                    created_at: (entry.created_at > 0)
                        .then_some(entry.created_at)
                        .or(previous.created_at),
                    updated_at: (entry.updated_at > 0)
                        .then_some(entry.updated_at)
                        .or(previous.updated_at),
                    model: previous.model.clone(),
                    service_tier: previous.service_tier,
                    reasoning_effort: previous.reasoning_effort.clone(),
                    approval_policy: previous.approval_policy,
                    sandbox_mode: previous.sandbox_mode,
                    personality: previous.personality,
                    collaboration_mode: previous.collaboration_mode,
                },
            );
        }
        if let Some(active_id) = previous_active_thread_id {
            if !self.threads.iter().any(|thread_id| thread_id == &active_id) {
                self.threads.insert(0, active_id.clone());
                self.thread_metadata.insert(
                    active_id.clone(),
                    previous_metadata
                        .get(&active_id)
                        .cloned()
                        .unwrap_or_default(),
                );
            }
            self.active_thread_id = Some(active_id);
        } else {
            self.active_thread_id = self.threads.first().cloned();
        }
        self.rebuild_project_registry();
    }

    fn cache_active_thread_transcript(&mut self) {
        let Some(thread_id) = self.active_thread_id.clone() else {
            return;
        };
        if self.messages.is_empty() {
            return;
        }
        self.thread_transcript_cache
            .insert(thread_id, self.messages.clone());
    }

    fn restore_cached_thread_transcript(&mut self, thread_id: &str) {
        let Some(messages) = self.thread_transcript_cache.get(thread_id).cloned() else {
            self.set_active_thread_transcript(thread_id, Vec::new());
            return;
        };
        self.apply_active_thread_messages(thread_id, messages);
    }

    pub fn cache_thread_transcript(
        &mut self,
        thread_id: &str,
        messages: Vec<(AutopilotRole, String)>,
    ) {
        let mut cached_messages = Vec::new();
        let mut next_message_id = 1u64;
        for (role, content) in messages {
            if content.trim().is_empty() {
                continue;
            }
            cached_messages.push(AutopilotMessage {
                id: next_message_id,
                role,
                status: AutopilotMessageStatus::Done,
                content,
                structured: None,
            });
            next_message_id = next_message_id.saturating_add(1);
        }
        self.thread_transcript_cache
            .insert(thread_id.to_string(), cached_messages);
    }

    pub fn select_thread_by_index(&mut self, index: usize) -> Option<AutopilotThreadResumeTarget> {
        let thread_id = self.threads.get(index).cloned()?;
        self.cache_active_thread_transcript();
        self.active_thread_id = Some(thread_id.clone());
        self.reset_transcript_scroll();
        self.last_error = None;
        self.restore_cached_thread_transcript(&thread_id);
        let metadata = self.thread_metadata.get(&thread_id).cloned();
        Some(AutopilotThreadResumeTarget {
            thread_id,
            cwd: metadata.as_ref().and_then(|value| value.cwd.clone()),
            path: metadata.and_then(|value| value.path),
        })
    }

    pub fn remember_thread(&mut self, thread_id: impl Into<String>) {
        let thread_id = thread_id.into();
        if !self.threads.iter().any(|existing| existing == &thread_id) {
            self.threads.insert(0, thread_id.clone());
        }
        self.thread_metadata.entry(thread_id.clone()).or_default();
        if self.active_thread_id.is_none() {
            self.active_thread_id = Some(thread_id);
        }
    }

    pub fn ensure_thread(&mut self, thread_id: String) {
        self.remember_thread(thread_id.clone());
        self.active_thread_id = Some(thread_id);
        self.reset_transcript_scroll();
    }

    pub fn is_active_thread(&self, thread_id: &str) -> bool {
        self.active_thread_id.as_deref() == Some(thread_id)
    }

    pub fn remove_thread(&mut self, thread_id: &str) {
        self.threads.retain(|value| value != thread_id);
        self.thread_metadata.remove(thread_id);
        self.terminal_sessions.remove(thread_id);
        self.thread_transcript_cache.remove(thread_id);
        self.thread_plan_artifacts.remove(thread_id);
        self.thread_diff_artifacts.remove(thread_id);
        self.thread_review_artifacts.remove(thread_id);
        self.thread_compaction_artifacts.remove(thread_id);
        self.review_thread_source_map.remove(thread_id);
        self.review_thread_source_map
            .retain(|_, source_thread_id| source_thread_id != thread_id);
        self.thread_composer_drafts.remove(thread_id);
        self.thread_submission_history.remove(thread_id);
        self.pending_turn_metadata
            .retain(|metadata| metadata.thread_id != thread_id);
        self.pending_steer_submissions
            .retain(|pending| pending.thread_id != thread_id);
        self.turn_metadata_by_turn_id
            .retain(|_, metadata| metadata.thread_id != thread_id);
        if self
            .last_submitted_turn_metadata
            .as_ref()
            .is_some_and(|metadata| metadata.thread_id == thread_id)
        {
            self.last_submitted_turn_metadata = None;
        }
        if self.active_thread_id.as_deref() == Some(thread_id) {
            self.active_thread_id = self.threads.first().cloned();
            if let Some(next_thread_id) = self.active_thread_id.clone() {
                self.restore_cached_thread_transcript(&next_thread_id);
            } else {
                self.apply_active_thread_messages("", Vec::new());
            }
        }
        self.persist_codex_artifact_projection();
    }

    fn apply_active_thread_messages(&mut self, thread_id: &str, messages: Vec<AutopilotMessage>) {
        if !thread_id.is_empty() && !self.is_active_thread(thread_id) {
            return;
        }

        self.messages = messages;
        self.next_message_id = self
            .messages
            .iter()
            .map(|message| message.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1)
            .max(1);
        if !thread_id.is_empty() {
            self.thread_transcript_cache
                .insert(thread_id.to_string(), self.messages.clone());
        }

        self.active_turn_id = None;
        self.active_assistant_message_id = None;
        self.pending_assistant_message_ids.clear();
        self.turn_assistant_message_ids.clear();
        self.pending_turn_metadata
            .retain(|metadata| metadata.thread_id != thread_id);
        self.turn_metadata_by_turn_id
            .retain(|_, metadata| metadata.thread_id != thread_id);
        if self
            .last_submitted_turn_metadata
            .as_ref()
            .is_some_and(|metadata| metadata.thread_id == thread_id)
        {
            self.last_submitted_turn_metadata = None;
        }
        self.last_agent_item_ids.clear();
        self.last_reasoning_item_ids.clear();
        self.last_agent_delta_signature = None;
        self.last_reasoning_delta_signature = None;
        self.last_turn_status = None;
        self.token_usage = None;
        self.turn_plan_explanation = None;
        self.turn_plan.clear();
        self.turn_diff = None;
        self.turn_timeline.clear();
        self.transcript_selection = None;
        self.last_error = None;
        if !thread_id.is_empty() {
            self.restore_active_plan_state(thread_id);
            self.restore_active_diff_state(thread_id);
        }
    }

    pub fn set_active_thread_transcript(
        &mut self,
        thread_id: &str,
        messages: Vec<(AutopilotRole, String)>,
    ) {
        if !self.is_active_thread(thread_id) {
            return;
        }

        let mut active_messages = Vec::new();
        let mut next_message_id = 1u64;
        for (role, content) in messages {
            if content.trim().is_empty() {
                continue;
            }
            active_messages.push(AutopilotMessage {
                id: next_message_id,
                role,
                status: AutopilotMessageStatus::Done,
                content,
                structured: None,
            });
            next_message_id = next_message_id.saturating_add(1);
        }
        self.apply_active_thread_messages(thread_id, active_messages);
    }

    pub fn submit_prompt(&mut self, prompt: String) {
        self.last_error = None;
        self.transcript_selection = None;
        let trimmed = prompt.trim();
        if trimmed.is_empty() {
            self.last_error = Some("Prompt cannot be empty".to_string());
            self.messages.push(AutopilotMessage {
                id: self.next_message_id,
                role: AutopilotRole::Codex,
                status: AutopilotMessageStatus::Error,
                content: "Cannot run empty prompt".to_string(),
                structured: None,
            });
            self.next_message_id = self.next_message_id.saturating_add(1);
            return;
        }

        self.messages.push(AutopilotMessage {
            id: self.next_message_id,
            role: AutopilotRole::User,
            status: AutopilotMessageStatus::Done,
            content: trimmed.to_string(),
            structured: None,
        });
        self.next_message_id = self.next_message_id.saturating_add(1);

        let assistant_message_id = self.next_message_id;
        self.messages.push(AutopilotMessage {
            id: assistant_message_id,
            role: AutopilotRole::Codex,
            status: AutopilotMessageStatus::Queued,
            content: String::new(),
            structured: Some(AutopilotStructuredMessage::default()),
        });
        self.next_message_id = self.next_message_id.saturating_add(1);
        self.pending_assistant_message_ids
            .push_back(assistant_message_id);
        self.active_assistant_message_id = Some(assistant_message_id);
    }

    pub fn submit_steer_prompt(&mut self, prompt: String) {
        self.last_error = None;
        self.transcript_selection = None;
        let trimmed = prompt.trim();
        if trimmed.is_empty() {
            return;
        }
        self.messages.push(AutopilotMessage {
            id: self.next_message_id,
            role: AutopilotRole::User,
            status: AutopilotMessageStatus::Done,
            content: trimmed.to_string(),
            structured: None,
        });
        self.next_message_id = self.next_message_id.saturating_add(1);
        self.last_turn_status = Some("inProgress".to_string());
    }

    pub fn append_local_exchange(
        &mut self,
        prompt: impl Into<String>,
        response: impl Into<String>,
        is_error: bool,
    ) {
        self.transcript_selection = None;
        let prompt = prompt.into();
        let response = response.into();
        let trimmed_prompt = prompt.trim();
        let trimmed_response = response.trim();
        if !trimmed_prompt.is_empty() {
            self.messages.push(AutopilotMessage {
                id: self.next_message_id,
                role: AutopilotRole::User,
                status: AutopilotMessageStatus::Done,
                content: trimmed_prompt.to_string(),
                structured: None,
            });
            self.next_message_id = self.next_message_id.saturating_add(1);
        }
        if !trimmed_response.is_empty() {
            self.messages.push(AutopilotMessage {
                id: self.next_message_id,
                role: AutopilotRole::Codex,
                status: if is_error {
                    AutopilotMessageStatus::Error
                } else {
                    AutopilotMessageStatus::Done
                },
                content: trimmed_response.to_string(),
                structured: None,
            });
            self.next_message_id = self.next_message_id.saturating_add(1);
        }
        if let Some(thread_id) = self.active_thread_id.clone() {
            self.thread_transcript_cache
                .insert(thread_id, self.messages.clone());
        }
        self.last_error = is_error.then(|| trimmed_response.to_string());
    }

    pub fn record_turn_submission_metadata(
        &mut self,
        thread_id: &str,
        run_classification: CodexRunClassification,
        labor_binding: Option<CodexLaborBinding>,
        is_cad_turn: bool,
        classifier_reason: impl Into<String>,
        submitted_at_epoch_ms: u64,
        selected_skill_names: Vec<String>,
    ) {
        let mut selected_skill_names = selected_skill_names
            .into_iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        selected_skill_names.sort();
        selected_skill_names.dedup();
        let metadata = AutopilotTurnMetadata {
            submission_seq: self.next_turn_submission_seq,
            thread_id: thread_id.to_string(),
            run_classification,
            labor_binding,
            is_cad_turn,
            classifier_reason: classifier_reason.into(),
            submitted_at_epoch_ms,
            selected_skill_names,
        };
        self.next_turn_submission_seq = self.next_turn_submission_seq.saturating_add(1);
        self.last_submitted_turn_metadata = Some(metadata.clone());
        self.pending_turn_metadata.push_back(metadata);
        if self.pending_turn_metadata.len() > 64 {
            let overflow = self.pending_turn_metadata.len().saturating_sub(64);
            self.pending_turn_metadata.drain(0..overflow);
        }
    }

    pub fn set_last_pending_turn_selected_skills(&mut self, selected_skill_names: Vec<String>) {
        let mut selected_skill_names = selected_skill_names
            .into_iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        selected_skill_names.sort();
        selected_skill_names.dedup();
        if let Some(last) = self.pending_turn_metadata.back_mut() {
            last.selected_skill_names = selected_skill_names.clone();
            if let Some(binding) = last.labor_binding.as_mut() {
                binding.set_selected_skill_names(selected_skill_names.clone());
            }
        }
        if let Some(last) = self.last_submitted_turn_metadata.as_mut() {
            last.selected_skill_names = selected_skill_names.clone();
            if let Some(binding) = last.labor_binding.as_mut() {
                binding.set_selected_skill_names(selected_skill_names);
            }
        }
    }

    pub fn turn_metadata_for(&self, turn_id: &str) -> Option<&AutopilotTurnMetadata> {
        self.turn_metadata_by_turn_id.get(turn_id)
    }

    pub fn active_turn_metadata(&self) -> Option<&AutopilotTurnMetadata> {
        self.active_turn_id
            .as_deref()
            .and_then(|turn_id| self.turn_metadata_for(turn_id))
            .or(self.last_submitted_turn_metadata.as_ref())
    }

    pub fn turn_labor_binding_for(&self, turn_id: &str) -> Option<&CodexLaborBinding> {
        self.turn_metadata_for(turn_id)
            .and_then(|metadata| metadata.labor_binding.as_ref())
    }

    pub fn turn_labor_submission_for(&self, turn_id: &str) -> Option<&CodexLaborSubmissionState> {
        self.turn_labor_binding_for(turn_id)
            .and_then(|binding| binding.submission.as_ref())
    }

    pub fn turn_labor_verdict_for(&self, turn_id: &str) -> Option<&CodexLaborVerdictState> {
        self.turn_labor_binding_for(turn_id)
            .and_then(|binding| binding.verdict.as_ref())
    }

    pub fn turn_labor_linkage_for(&self, turn_id: &str) -> Option<GoalLaborLinkage> {
        let binding = self.turn_labor_binding_for(turn_id)?;
        let mut labor = GoalLaborLinkage {
            work_unit_id: Some(binding.work_unit_id.clone()),
            contract_id: Some(binding.contract_id.clone()),
            submission_id: binding
                .submission
                .as_ref()
                .map(|submission| submission.submission.submission_id.clone()),
            verdict_id: binding
                .verdict
                .as_ref()
                .map(|verdict| verdict.verdict.verdict_id.clone()),
            claim_id: binding.trace.claim_id.clone(),
            claim_state: binding.claim_runtime_state_label().map(str::to_string),
            remedy_kind: binding
                .claim
                .as_ref()
                .and_then(|claim| claim.remedy.as_ref())
                .map(|remedy| remedy.outcome.clone()),
            settlement_id: None,
            settlement_ready: Some(binding.is_settlement_ready()),
            tool_evidence_refs: binding
                .provenance
                .tool_invocations
                .iter()
                .map(|invocation| {
                    labor_tool_evidence_ref(
                        binding.work_unit_id.as_str(),
                        invocation.request_id.as_str(),
                        invocation.call_id.as_str(),
                        invocation.tool_name.as_str(),
                        invocation.response_code.as_deref().unwrap_or("pending"),
                        invocation.success.unwrap_or(false),
                        invocation
                            .response_message_digest
                            .as_deref()
                            .unwrap_or("sha256:pending"),
                    )
                })
                .collect(),
            submission_evidence_refs: binding
                .submission
                .as_ref()
                .map(|submission| submission.evidence_refs.clone())
                .unwrap_or_default(),
            verdict_evidence_refs: binding
                .verdict
                .as_ref()
                .map(|verdict| verdict.evidence_refs.clone())
                .unwrap_or_default(),
            claim_evidence_refs: binding
                .claim
                .as_ref()
                .map(|claim| claim.evidence_refs.clone())
                .unwrap_or_default(),
            incident_evidence_refs: binding.incident_evidence_refs.clone(),
            remedy_evidence_refs: binding
                .claim
                .as_ref()
                .and_then(|claim| claim.remedy.as_ref())
                .map(|remedy| remedy.evidence_refs.clone())
                .unwrap_or_default(),
            settlement_evidence_refs: Vec::new(),
        };
        labor.tool_evidence_refs.sort_by(|left, right| {
            left.kind
                .cmp(&right.kind)
                .then_with(|| left.uri.cmp(&right.uri))
                .then_with(|| left.digest.cmp(&right.digest))
        });
        Some(labor)
    }

    pub fn turn_labor_scope_payload(&self, turn_id: &str) -> Option<Value> {
        self.turn_labor_binding_for(turn_id)
            .map(CodexLaborBinding::scope_payload)
    }

    pub fn turn_labor_requirements_payload(&self, turn_id: &str) -> Option<Value> {
        self.turn_labor_binding_for(turn_id)
            .map(CodexLaborBinding::requirements_payload)
    }

    pub fn turn_labor_evidence_payload(&self, turn_id: &str) -> Option<Value> {
        self.turn_labor_binding_for(turn_id)
            .map(CodexLaborBinding::evidence_payload)
    }

    pub fn turn_labor_claim_payload(&self, turn_id: &str) -> Option<Value> {
        self.turn_labor_binding_for(turn_id)
            .map(CodexLaborBinding::claim_payload)
    }

    pub fn attach_turn_labor_evidence(
        &mut self,
        turn_id: &str,
        evidence: EvidenceRef,
        incident: bool,
    ) -> Result<Option<Value>, String> {
        let Some(binding) = self.turn_labor_binding_mut(turn_id) else {
            return Ok(None);
        };
        binding.attach_evidence_ref(evidence, incident)?;
        Ok(Some(binding.evidence_payload()))
    }

    pub fn assemble_turn_labor_submission(
        &mut self,
        turn_id: &str,
        created_at_epoch_ms: u64,
    ) -> Result<Option<CodexLaborSubmissionState>, String> {
        let Some(binding) = self.turn_labor_binding_mut(turn_id) else {
            return Ok(None);
        };
        Ok(Some(binding.assemble_submission(created_at_epoch_ms)))
    }

    pub fn finalize_turn_labor_verdict(
        &mut self,
        turn_id: &str,
        verified_at_epoch_ms: u64,
    ) -> Result<Option<CodexLaborVerdictState>, String> {
        let Some(binding) = self.turn_labor_binding_mut(turn_id) else {
            return Ok(None);
        };
        binding.finalize_verdict(verified_at_epoch_ms).map(Some)
    }

    pub fn open_turn_labor_claim(
        &mut self,
        turn_id: &str,
        opened_at_epoch_ms: u64,
        reason_code: Option<&str>,
        note: Option<&str>,
    ) -> Result<Option<CodexLaborClaimState>, String> {
        let Some(binding) = self.turn_labor_binding_mut(turn_id) else {
            return Ok(None);
        };
        binding
            .open_claim(opened_at_epoch_ms, reason_code, note)
            .map(Some)
    }

    pub fn review_turn_labor_claim(
        &mut self,
        turn_id: &str,
        reviewed_at_epoch_ms: u64,
        note: Option<&str>,
    ) -> Result<Option<CodexLaborClaimState>, String> {
        let Some(binding) = self.turn_labor_binding_mut(turn_id) else {
            return Ok(None);
        };
        binding
            .move_claim_under_review(reviewed_at_epoch_ms, note)
            .map(Some)
    }

    pub fn issue_turn_labor_remedy(
        &mut self,
        turn_id: &str,
        issued_at_epoch_ms: u64,
        outcome: &str,
        note: Option<&str>,
    ) -> Result<Option<CodexLaborClaimState>, String> {
        let Some(binding) = self.turn_labor_binding_mut(turn_id) else {
            return Ok(None);
        };
        binding
            .issue_claim_remedy(issued_at_epoch_ms, outcome, note)
            .map(Some)
    }

    pub fn deny_turn_labor_claim(
        &mut self,
        turn_id: &str,
        denied_at_epoch_ms: u64,
        reason_code: Option<&str>,
        note: Option<&str>,
    ) -> Result<Option<CodexLaborClaimState>, String> {
        let Some(binding) = self.turn_labor_binding_mut(turn_id) else {
            return Ok(None);
        };
        binding
            .deny_claim(denied_at_epoch_ms, reason_code, note)
            .map(Some)
    }

    pub fn resolve_turn_labor_claim(
        &mut self,
        turn_id: &str,
        resolved_at_epoch_ms: u64,
        note: Option<&str>,
    ) -> Result<Option<CodexLaborClaimState>, String> {
        let Some(binding) = self.turn_labor_binding_mut(turn_id) else {
            return Ok(None);
        };
        binding.resolve_claim(resolved_at_epoch_ms, note).map(Some)
    }

    pub fn turn_labor_settlement_ready(&self, turn_id: &str) -> Option<bool> {
        self.turn_metadata_for(turn_id)
            .and_then(|metadata| metadata.labor_binding.as_ref())
            .map(CodexLaborBinding::is_settlement_ready)
    }

    pub fn record_turn_command_approval_requested(
        &mut self,
        turn_id: &str,
        item_id: &str,
        reason: Option<&str>,
        command: Option<&str>,
        cwd: Option<&str>,
        recorded_at_epoch_ms: u64,
    ) {
        self.record_turn_approval_event(
            turn_id,
            CodexLaborApprovalEvent {
                kind: "command_request".to_string(),
                item_id: item_id.to_string(),
                decision: None,
                reason: reason.map(str::to_string),
                command: command.map(str::to_string),
                cwd: cwd.map(str::to_string),
                grant_root: None,
                recorded_at_epoch_ms,
            },
        );
    }

    pub fn record_turn_command_approval_response(
        &mut self,
        turn_id: &str,
        item_id: &str,
        decision: &str,
        recorded_at_epoch_ms: u64,
    ) {
        self.record_turn_approval_event(
            turn_id,
            CodexLaborApprovalEvent {
                kind: "command_response".to_string(),
                item_id: item_id.to_string(),
                decision: Some(decision.to_string()),
                reason: None,
                command: None,
                cwd: None,
                grant_root: None,
                recorded_at_epoch_ms,
            },
        );
    }

    pub fn record_turn_file_change_approval_requested(
        &mut self,
        turn_id: &str,
        item_id: &str,
        reason: Option<&str>,
        grant_root: Option<&str>,
        recorded_at_epoch_ms: u64,
    ) {
        self.record_turn_approval_event(
            turn_id,
            CodexLaborApprovalEvent {
                kind: "file_change_request".to_string(),
                item_id: item_id.to_string(),
                decision: None,
                reason: reason.map(str::to_string),
                command: None,
                cwd: None,
                grant_root: grant_root.map(str::to_string),
                recorded_at_epoch_ms,
            },
        );
    }

    pub fn record_turn_file_change_approval_response(
        &mut self,
        turn_id: &str,
        item_id: &str,
        decision: &str,
        recorded_at_epoch_ms: u64,
    ) {
        self.record_turn_approval_event(
            turn_id,
            CodexLaborApprovalEvent {
                kind: "file_change_response".to_string(),
                item_id: item_id.to_string(),
                decision: Some(decision.to_string()),
                reason: None,
                command: None,
                cwd: None,
                grant_root: None,
                recorded_at_epoch_ms,
            },
        );
    }

    pub fn record_turn_tool_request(
        &mut self,
        turn_id: &str,
        request_id: &str,
        call_id: &str,
        tool_name: &str,
        arguments: &str,
        recorded_at_epoch_ms: u64,
    ) {
        if let Some(binding) = self.turn_labor_binding_mut(turn_id) {
            binding.record_tool_request(
                request_id,
                call_id,
                tool_name,
                arguments,
                recorded_at_epoch_ms,
            );
        }
    }

    pub fn record_turn_tool_result(
        &mut self,
        turn_id: &str,
        request_id: &str,
        call_id: &str,
        tool_name: &str,
        response_code: &str,
        success: bool,
        response_message: &str,
        recorded_at_epoch_ms: u64,
    ) {
        if let Some(binding) = self.turn_labor_binding_mut(turn_id) {
            binding.record_tool_result(
                request_id,
                call_id,
                tool_name,
                response_code,
                success,
                response_message,
                recorded_at_epoch_ms,
            );
        }
    }

    fn record_turn_approval_event(&mut self, turn_id: &str, event: CodexLaborApprovalEvent) {
        if let Some(binding) = self.turn_labor_binding_mut(turn_id) {
            binding.record_approval_event(event);
        }
    }

    fn turn_labor_binding_mut(&mut self, turn_id: &str) -> Option<&mut CodexLaborBinding> {
        self.turn_metadata_by_turn_id
            .get_mut(turn_id)
            .and_then(|metadata| metadata.labor_binding.as_mut())
    }

    fn capture_turn_output_snapshot(&mut self, turn_id: &str, content: &str) {
        if let Some(binding) = self.turn_labor_binding_mut(turn_id) {
            binding.record_output_snapshot(content);
        }
    }

    pub fn mark_turn_started(&mut self, turn_id: String) {
        self.active_turn_id = Some(turn_id.clone());
        if let Some(mut metadata) = self.pending_turn_metadata.pop_front() {
            if let Some(binding) = metadata.labor_binding.as_mut() {
                binding.record_turn_started(turn_id.as_str());
            }
            self.turn_metadata_by_turn_id
                .insert(turn_id.clone(), metadata);
            if self.turn_metadata_by_turn_id.len() > 64 {
                let mut sorted = self
                    .turn_metadata_by_turn_id
                    .iter()
                    .map(|(key, value)| (key.clone(), value.submission_seq))
                    .collect::<Vec<_>>();
                sorted.sort_by_key(|(_, seq)| *seq);
                let overflow = sorted.len().saturating_sub(64);
                for (key, _) in sorted.into_iter().take(overflow) {
                    self.turn_metadata_by_turn_id.remove(&key);
                }
            }
        }
        self.last_turn_status = Some("inProgress".to_string());
        if let Some(assistant_message_id) = self.bind_turn_to_assistant_message(&turn_id)
            && let Some(message) = self
                .messages
                .iter_mut()
                .find(|message| message.id == assistant_message_id)
        {
            message.status = AutopilotMessageStatus::Running;
            self.active_assistant_message_id = Some(assistant_message_id);
        }
    }

    #[allow(dead_code)]
    pub fn append_turn_delta(&mut self, delta: &str) {
        let Some(turn_id) = self.active_turn_id.clone() else {
            return;
        };
        self.append_turn_delta_for_turn(&turn_id, delta);
    }

    pub fn append_turn_delta_for_turn(&mut self, turn_id: &str, delta: &str) {
        let assistant_message_id = self
            .turn_assistant_message_ids
            .get(turn_id)
            .copied()
            .or_else(|| self.bind_turn_to_assistant_message(turn_id));
        if let Some(assistant_message_id) = assistant_message_id
            && let Some(message) = self
                .messages
                .iter_mut()
                .find(|message| message.id == assistant_message_id)
        {
            if self.active_turn_id.as_deref() == Some(turn_id) {
                message.status = AutopilotMessageStatus::Running;
            }
            if let Some(structured) = message.structured.as_mut() {
                structured.answer.push_str(delta);
                structured
                    .events
                    .push(format!("answer+{}", delta.chars().count()));
                message.content = structured.rendered_content();
            } else {
                message.content.push_str(delta);
            }
        }
    }

    pub fn append_turn_reasoning_delta_for_turn(&mut self, turn_id: &str, delta: &str) {
        if delta.is_empty() {
            return;
        }
        let assistant_message_id = self
            .turn_assistant_message_ids
            .get(turn_id)
            .copied()
            .or_else(|| self.bind_turn_to_assistant_message(turn_id));
        if let Some(assistant_message_id) = assistant_message_id
            && let Some(message) = self
                .messages
                .iter_mut()
                .find(|message| message.id == assistant_message_id)
        {
            if self.active_turn_id.as_deref() == Some(turn_id) {
                message.status = AutopilotMessageStatus::Running;
            }
            if let Some(structured) = message.structured.as_mut() {
                // Explicit typed ordering rule: ignore late reasoning after answer streaming starts.
                if !structured.answer.trim().is_empty() {
                    return;
                }
                structured.reasoning.push_str(delta);
                structured
                    .events
                    .push(format!("reasoning+{}", delta.chars().count()));
                message.content = structured.rendered_content();
            } else {
                message.content.push_str(delta);
            }
        }
    }

    pub fn set_turn_message_for_turn(&mut self, turn_id: &str, content: &str) {
        let content = content.trim_end();
        if content.trim().is_empty() {
            return;
        }
        let mut captured_output = None::<String>;
        let assistant_message_id = self
            .turn_assistant_message_ids
            .get(turn_id)
            .copied()
            .or_else(|| self.bind_turn_to_assistant_message(turn_id));
        if let Some(assistant_message_id) = assistant_message_id
            && let Some(message) = self
                .messages
                .iter_mut()
                .find(|message| message.id == assistant_message_id)
        {
            if self.active_turn_id.as_deref() == Some(turn_id) {
                message.status = AutopilotMessageStatus::Running;
            }
            if let Some(structured) = message.structured.as_mut() {
                if structured.answer == content {
                    return;
                }
                structured.answer = content.to_string();
                structured.status = Some("answer".to_string());
                let rendered = structured.rendered_content();
                if message.content != rendered {
                    message.content = rendered;
                }
                self.capture_turn_output_snapshot(turn_id, content);
                return;
            }
            if message.content != content {
                message.content = content.to_string();
            }
            captured_output = Some(content.to_string());
        }
        if let Some(captured_output) = captured_output {
            self.capture_turn_output_snapshot(turn_id, captured_output.as_str());
        }
    }

    pub fn set_turn_progress_blocks_for_turn(
        &mut self,
        turn_id: &str,
        progress_blocks: Vec<AutopilotProgressBlock>,
    ) {
        let assistant_message_id = self
            .turn_assistant_message_ids
            .get(turn_id)
            .copied()
            .or_else(|| self.bind_turn_to_assistant_message(turn_id));
        if let Some(assistant_message_id) = assistant_message_id
            && let Some(message) = self
                .messages
                .iter_mut()
                .find(|message| message.id == assistant_message_id)
        {
            let structured = message
                .structured
                .get_or_insert_with(AutopilotStructuredMessage::default);
            if structured.progress_blocks == progress_blocks {
                return;
            }
            structured.progress_blocks = progress_blocks;
            message.content = structured.rendered_content();
        }
    }

    #[allow(dead_code)]
    pub fn mark_turn_completed(&mut self) {
        let Some(turn_id) = self.active_turn_id.clone() else {
            return;
        };
        self.mark_turn_completed_for(&turn_id);
    }

    pub fn mark_turn_completed_for(&mut self, turn_id: &str) {
        let mut captured_output = None::<String>;
        let assistant_message_id = self
            .turn_assistant_message_ids
            .get(turn_id)
            .copied()
            .or_else(|| self.bind_turn_to_assistant_message(turn_id));
        if let Some(assistant_message_id) = assistant_message_id
            && let Some(message) = self
                .messages
                .iter_mut()
                .find(|message| message.id == assistant_message_id)
        {
            message.status = AutopilotMessageStatus::Done;
            if !message.content.trim().is_empty() {
                captured_output = Some(message.content.clone());
            }
            if self.active_assistant_message_id == Some(assistant_message_id) {
                self.active_assistant_message_id = None;
            }
        }
        self.last_turn_status = Some("completed".to_string());
        if self.active_turn_id.as_deref() == Some(turn_id) {
            self.active_turn_id = None;
        }
        if let Some(captured_output) = captured_output {
            self.capture_turn_output_snapshot(turn_id, captured_output.as_str());
        }
    }

    #[allow(dead_code)]
    pub fn mark_turn_error(&mut self, error: impl Into<String>) {
        let error = error.into();
        if let Some(turn_id) = self.active_turn_id.clone() {
            self.mark_turn_error_for(&turn_id, error);
            return;
        }
        self.mark_turn_error_for("unknown-turn", error);
    }

    pub fn mark_turn_error_for(&mut self, turn_id: &str, error: impl Into<String>) {
        let error = error.into();
        let mut captured_output = None::<String>;
        let assistant_message_id = self
            .turn_assistant_message_ids
            .get(turn_id)
            .copied()
            .or_else(|| self.bind_turn_to_assistant_message(turn_id))
            .or(self.active_assistant_message_id);
        if let Some(assistant_message_id) = assistant_message_id
            && let Some(message) = self
                .messages
                .iter_mut()
                .find(|message| message.id == assistant_message_id)
        {
            message.status = AutopilotMessageStatus::Error;
            if message.content.trim().is_empty() {
                message.content.clone_from(&error);
            }
            if !message.content.trim().is_empty() {
                captured_output = Some(message.content.clone());
            }
            if self.active_assistant_message_id == Some(assistant_message_id) {
                self.active_assistant_message_id = None;
            }
        }
        self.last_turn_status = Some("failed".to_string());
        self.last_error = Some(error);
        if self.active_turn_id.as_deref() == Some(turn_id) {
            self.active_turn_id = None;
        }
        if let Some(captured_output) = captured_output {
            self.capture_turn_output_snapshot(turn_id, captured_output.as_str());
        }
    }

    pub fn mark_pending_turn_dispatch_failed(&mut self, error: impl Into<String>) {
        let error = error.into();
        let _ = self.pending_turn_metadata.pop_front();
        if let Some(assistant_message_id) = self.active_assistant_message_id
            && let Some(message) = self
                .messages
                .iter_mut()
                .find(|message| message.id == assistant_message_id)
        {
            message.status = AutopilotMessageStatus::Error;
            message.content.clone_from(&error);
        }
        self.pending_assistant_message_ids
            .retain(|id| Some(*id) != self.active_assistant_message_id);
        self.last_error = Some(error);
        self.last_turn_status = Some("failed".to_string());
        self.active_turn_id = None;
        self.active_assistant_message_id = None;
    }

    pub fn set_turn_status(&mut self, status: Option<String>) {
        self.last_turn_status = status;
    }

    pub fn set_token_usage(
        &mut self,
        input_tokens: i64,
        cached_input_tokens: i64,
        output_tokens: i64,
    ) {
        self.token_usage = Some(AutopilotTokenUsage {
            input_tokens,
            cached_input_tokens,
            output_tokens,
        });
    }

    pub fn set_copy_notice(&mut self, now: Instant, message: String) {
        self.copy_notice = Some(message);
        self.copy_notice_until = Some(now + Duration::from_secs(3));
    }

    pub fn expire_copy_notice(&mut self, now: Instant) -> bool {
        if self.copy_notice_until.is_some_and(|until| until <= now) {
            self.copy_notice = None;
            self.copy_notice_until = None;
            return true;
        }
        false
    }

    pub fn set_turn_plan(&mut self, explanation: Option<String>, plan: Vec<AutopilotTurnPlanStep>) {
        self.turn_plan_explanation = explanation;
        self.turn_plan = plan;
    }

    pub fn clear_plan_artifact(&mut self, thread_id: &str) {
        self.thread_plan_artifacts.remove(thread_id);
        if self.is_active_thread(thread_id) {
            self.turn_plan_explanation = None;
            self.turn_plan.clear();
        }
    }

    pub fn set_plan_artifact(
        &mut self,
        thread_id: &str,
        source_turn_id: impl Into<String>,
        explanation: Option<String>,
        steps: Vec<AutopilotTurnPlanStep>,
        updated_at_epoch_ms: u64,
        restored_from_thread_read: bool,
    ) {
        let metadata = self.thread_metadata.get(thread_id);
        let artifact = AutopilotPlanArtifact {
            thread_id: thread_id.to_string(),
            source_turn_id: source_turn_id.into(),
            explanation,
            steps,
            workspace_cwd: metadata.and_then(|value| value.cwd.clone()),
            workspace_path: metadata.and_then(|value| value.path.clone()),
            workspace_root: metadata.and_then(|value| value.workspace_root.clone()),
            project_id: metadata.and_then(|value| value.project_id.clone()),
            project_name: metadata.and_then(|value| value.project_name.clone()),
            git_branch: metadata.and_then(|value| value.git_branch.clone()),
            git_dirty: metadata.and_then(|value| value.git_dirty),
            updated_at_epoch_ms,
            restored_from_thread_read,
        };
        if self.is_active_thread(thread_id) {
            self.turn_plan_explanation = artifact.explanation.clone();
            self.turn_plan = artifact.steps.clone();
        }
        self.thread_plan_artifacts
            .insert(thread_id.to_string(), artifact);
    }

    pub fn restore_plan_artifact_from_text(
        &mut self,
        thread_id: &str,
        source_turn_id: impl Into<String>,
        plan_text: &str,
        updated_at_epoch_ms: u64,
    ) {
        let (explanation, steps) = Self::parse_plan_artifact_text(plan_text);
        if explanation.is_none() && steps.is_empty() {
            self.clear_plan_artifact(thread_id);
            return;
        }
        self.set_plan_artifact(
            thread_id,
            source_turn_id,
            explanation,
            steps,
            updated_at_epoch_ms,
            true,
        );
    }

    fn restore_active_plan_state(&mut self, thread_id: &str) {
        let Some(artifact) = self.thread_plan_artifacts.get(thread_id).cloned() else {
            self.turn_plan_explanation = None;
            self.turn_plan.clear();
            return;
        };
        self.turn_plan_explanation = artifact.explanation;
        self.turn_plan = artifact.steps;
    }

    fn restore_active_diff_state(&mut self, thread_id: &str) {
        self.turn_diff = self
            .thread_diff_artifacts
            .get(thread_id)
            .and_then(|artifacts| artifacts.first())
            .map(|artifact| artifact.raw_diff.clone());
    }

    fn parse_plan_artifact_text(plan_text: &str) -> (Option<String>, Vec<AutopilotTurnPlanStep>) {
        let mut explanation_lines = Vec::new();
        let mut steps = Vec::new();
        let mut saw_step = false;
        for raw_line in plan_text.lines() {
            let trimmed = raw_line.trim();
            if trimmed.is_empty() {
                if !saw_step && !explanation_lines.is_empty() {
                    explanation_lines.push(String::new());
                }
                continue;
            }
            if let Some(step) = Self::parse_plan_step_line(trimmed) {
                saw_step = true;
                steps.push(step);
            } else if !saw_step {
                explanation_lines.push(trimmed.to_string());
            }
        }
        if steps.is_empty() {
            steps = plan_text
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(|line| AutopilotTurnPlanStep {
                    step: line.to_string(),
                    status: "pending".to_string(),
                })
                .collect();
            explanation_lines.clear();
        }
        let explanation = if explanation_lines.is_empty() {
            None
        } else {
            Some(explanation_lines.join("\n").trim().to_string())
        }
        .filter(|value| !value.is_empty());
        (explanation, steps)
    }

    fn parse_plan_step_line(line: &str) -> Option<AutopilotTurnPlanStep> {
        let bracket_prefixes = [
            ("[x] ", "completed"),
            ("[X] ", "completed"),
            ("[~] ", "inProgress"),
            ("[-] ", "inProgress"),
            ("[ ] ", "pending"),
        ];
        for (prefix, status) in bracket_prefixes {
            if let Some(step) = line.strip_prefix(prefix).map(str::trim) {
                if !step.is_empty() {
                    return Some(AutopilotTurnPlanStep {
                        step: step.to_string(),
                        status: status.to_string(),
                    });
                }
            }
            let bullet_prefix = format!("- {prefix}");
            if let Some(step) = line.strip_prefix(&bullet_prefix).map(str::trim) {
                if !step.is_empty() {
                    return Some(AutopilotTurnPlanStep {
                        step: step.to_string(),
                        status: status.to_string(),
                    });
                }
            }
        }

        for prefix in ["- ", "* ", "+ "] {
            if let Some(step) = line.strip_prefix(prefix).map(str::trim) {
                if !step.is_empty() {
                    return Some(AutopilotTurnPlanStep {
                        step: step.to_string(),
                        status: "pending".to_string(),
                    });
                }
            }
        }

        let digit_count = line.chars().take_while(|ch| ch.is_ascii_digit()).count();
        if digit_count > 0 {
            let suffix = line[digit_count..].trim_start();
            if let Some(step) = suffix
                .strip_prefix('.')
                .or_else(|| suffix.strip_prefix(')'))
                .map(str::trim)
                && !step.is_empty()
            {
                return Some(AutopilotTurnPlanStep {
                    step: step.to_string(),
                    status: "pending".to_string(),
                });
            }
        }

        None
    }

    pub fn set_diff_artifact(
        &mut self,
        thread_id: &str,
        source_turn_id: impl Into<String>,
        raw_diff: String,
        updated_at_epoch_ms: u64,
    ) {
        let trimmed_diff = raw_diff.trim();
        if trimmed_diff.is_empty() {
            return;
        }
        let source_turn_id = source_turn_id.into();
        let files = parse_diff_file_artifacts(trimmed_diff);
        let added_line_count = files.iter().map(|file| file.added_line_count).sum();
        let removed_line_count = files.iter().map(|file| file.removed_line_count).sum();
        let artifact = AutopilotDiffArtifact {
            thread_id: thread_id.to_string(),
            source_turn_id: source_turn_id.clone(),
            files,
            added_line_count,
            removed_line_count,
            raw_diff: trimmed_diff.to_string(),
            workspace_root: self
                .thread_metadata
                .get(thread_id)
                .and_then(|metadata| metadata.workspace_root.clone()),
            project_id: self
                .thread_metadata
                .get(thread_id)
                .and_then(|metadata| metadata.project_id.clone()),
            project_name: self
                .thread_metadata
                .get(thread_id)
                .and_then(|metadata| metadata.project_name.clone()),
            git_branch: self
                .thread_metadata
                .get(thread_id)
                .and_then(|metadata| metadata.git_branch.clone()),
            git_dirty: self
                .thread_metadata
                .get(thread_id)
                .and_then(|metadata| metadata.git_dirty),
            updated_at_epoch_ms,
        };
        let artifacts = self
            .thread_diff_artifacts
            .entry(thread_id.to_string())
            .or_default();
        if let Some(existing) = artifacts
            .iter_mut()
            .find(|existing| existing.source_turn_id == source_turn_id)
        {
            *existing = artifact.clone();
        } else {
            artifacts.push(artifact.clone());
        }
        *artifacts = normalize_codex_diff_artifacts(std::mem::take(artifacts));
        if self.is_active_thread(thread_id) {
            self.turn_diff = Some(artifact.raw_diff.clone());
        }
        self.persist_codex_artifact_projection();
    }

    pub fn begin_review_artifact(
        &mut self,
        thread_id: &str,
        source_turn_id: impl Into<String>,
        review_thread_id: impl Into<String>,
        delivery: impl Into<String>,
        target: impl Into<String>,
        updated_at_epoch_ms: u64,
    ) {
        let source_turn_id = source_turn_id.into();
        let review_thread_id = review_thread_id.into();
        let delivery =
            normalized_review_delivery(Some(delivery.into()), thread_id, review_thread_id.as_str());
        let target = normalized_review_target(Some(target.into()));
        self.review_thread_source_map
            .insert(review_thread_id.clone(), thread_id.to_string());

        for artifact_thread_id in review_artifact_thread_keys(thread_id, review_thread_id.as_str())
        {
            self.thread_review_artifacts.insert(
                artifact_thread_id.clone(),
                AutopilotReviewArtifact {
                    thread_id: artifact_thread_id,
                    source_thread_id: thread_id.to_string(),
                    source_turn_id: source_turn_id.clone(),
                    review_thread_id: review_thread_id.clone(),
                    delivery: delivery.clone(),
                    target: target.clone(),
                    summary: None,
                    status: "running".to_string(),
                    updated_at_epoch_ms,
                    restored_from_thread_read: false,
                },
            );
        }
        self.persist_codex_artifact_projection();
    }

    pub fn complete_review_artifact(
        &mut self,
        thread_id: &str,
        source_turn_id: impl Into<String>,
        review_text: &str,
        updated_at_epoch_ms: u64,
        restored_from_thread_read: bool,
    ) {
        let source_turn_id = source_turn_id.into();
        let existing = self.thread_review_artifacts.get(thread_id).cloned();
        let source_thread_id = existing
            .as_ref()
            .map(|artifact| artifact.source_thread_id.clone())
            .or_else(|| self.review_thread_source_map.get(thread_id).cloned())
            .unwrap_or_else(|| thread_id.to_string());
        let review_thread_id = existing
            .as_ref()
            .map(|artifact| artifact.review_thread_id.clone())
            .unwrap_or_else(|| thread_id.to_string());
        let delivery = normalized_review_delivery(
            existing.as_ref().map(|artifact| artifact.delivery.clone()),
            source_thread_id.as_str(),
            review_thread_id.as_str(),
        );
        let target =
            normalized_review_target(existing.as_ref().map(|artifact| artifact.target.clone()));
        self.review_thread_source_map
            .insert(review_thread_id.clone(), source_thread_id.clone());
        let summary = review_text.trim();
        let summary = (!summary.is_empty()).then(|| summary.to_string());

        for artifact_thread_id in
            review_artifact_thread_keys(source_thread_id.as_str(), review_thread_id.as_str())
        {
            self.thread_review_artifacts.insert(
                artifact_thread_id.clone(),
                AutopilotReviewArtifact {
                    thread_id: artifact_thread_id,
                    source_thread_id: source_thread_id.clone(),
                    source_turn_id: source_turn_id.clone(),
                    review_thread_id: review_thread_id.clone(),
                    delivery: delivery.clone(),
                    target: target.clone(),
                    summary: summary.clone(),
                    status: "completed".to_string(),
                    updated_at_epoch_ms,
                    restored_from_thread_read,
                },
            );
        }
        self.persist_codex_artifact_projection();
    }

    pub fn restore_review_artifact_from_text(
        &mut self,
        thread_id: &str,
        source_turn_id: impl Into<String>,
        review_text: &str,
        updated_at_epoch_ms: u64,
    ) {
        self.complete_review_artifact(
            thread_id,
            source_turn_id,
            review_text,
            updated_at_epoch_ms,
            true,
        );
    }

    pub fn set_compaction_artifact(
        &mut self,
        thread_id: &str,
        source_turn_id: impl Into<String>,
        updated_at_epoch_ms: u64,
        restored_from_thread_read: bool,
    ) {
        self.thread_compaction_artifacts.insert(
            thread_id.to_string(),
            AutopilotCompactionArtifact {
                thread_id: thread_id.to_string(),
                source_turn_id: source_turn_id.into(),
                updated_at_epoch_ms,
                restored_from_thread_read,
            },
        );
        self.persist_codex_artifact_projection();
    }

    pub fn restore_compaction_artifact(
        &mut self,
        thread_id: &str,
        source_turn_id: impl Into<String>,
        updated_at_epoch_ms: u64,
    ) {
        self.set_compaction_artifact(thread_id, source_turn_id, updated_at_epoch_ms, true);
    }

    fn persist_codex_artifact_projection(&mut self) {
        if let Err(error) = persist_codex_artifact_projection(
            self.artifact_projection_file_path.as_path(),
            &self.thread_diff_artifacts,
            &self.thread_review_artifacts,
            &self.thread_compaction_artifacts,
        ) {
            tracing::warn!("failed to persist codex artifacts: {error}");
        }
    }

    pub fn set_turn_diff(&mut self, diff: Option<String>) {
        self.turn_diff = diff;
    }

    pub fn record_turn_timeline_event(&mut self, event: impl Into<String>) {
        self.turn_timeline.push(event.into());
        if self.turn_timeline.len() > 64 {
            let overflow = self.turn_timeline.len().saturating_sub(64);
            self.turn_timeline.drain(0..overflow);
        }
    }

    pub fn set_thread_loaded_ids(&mut self, loaded_thread_ids: &[String]) {
        let loaded_set: std::collections::HashSet<&str> =
            loaded_thread_ids.iter().map(String::as_str).collect();
        for (thread_id, metadata) in &mut self.thread_metadata {
            metadata.loaded = loaded_set.contains(thread_id.as_str());
        }
    }

    pub fn set_thread_status(&mut self, thread_id: &str, status: Option<String>) {
        if let Some(metadata) = self.thread_metadata.get_mut(thread_id) {
            metadata.status = status;
            return;
        }
        let mut metadata = AutopilotThreadMetadata::default();
        metadata.status = status;
        self.thread_metadata.insert(thread_id.to_string(), metadata);
    }

    pub fn set_thread_name(&mut self, thread_id: &str, thread_name: Option<String>) {
        if let Some(metadata) = self.thread_metadata.get_mut(thread_id) {
            metadata.thread_name = thread_name;
            return;
        }
        let mut metadata = AutopilotThreadMetadata::default();
        metadata.thread_name = thread_name;
        self.thread_metadata.insert(thread_id.to_string(), metadata);
    }

    pub fn active_thread_status(&self) -> Option<&str> {
        let thread_id = self.active_thread_id.as_ref()?;
        self.thread_metadata
            .get(thread_id)
            .and_then(|metadata| metadata.status.as_deref())
    }

    pub fn active_thread_loaded(&self) -> Option<bool> {
        let thread_id = self.active_thread_id.as_ref()?;
        self.thread_metadata
            .get(thread_id)
            .map(|metadata| metadata.loaded)
    }

    pub fn active_thread_preview(&self) -> Option<&str> {
        let thread_id = self.active_thread_id.as_ref()?;
        self.thread_metadata
            .get(thread_id)
            .and_then(|metadata| metadata.preview.as_deref())
    }

    pub fn active_thread_path(&self) -> Option<&str> {
        let thread_id = self.active_thread_id.as_ref()?;
        self.thread_metadata
            .get(thread_id)
            .and_then(|metadata| metadata.path.as_deref())
    }

    pub fn active_thread_updated_at(&self) -> Option<i64> {
        let thread_id = self.active_thread_id.as_ref()?;
        self.thread_metadata
            .get(thread_id)
            .and_then(|metadata| metadata.updated_at)
    }

    pub fn suggested_thread_name(&self, thread_id: &str) -> Option<String> {
        let active_candidate = if self.is_active_thread(thread_id) {
            self.messages
                .iter()
                .find(|message| message.role == AutopilotRole::User)
                .map(|message| message.content.as_str())
        } else {
            None
        };
        active_candidate
            .and_then(Self::sanitized_thread_title_candidate)
            .or_else(|| {
                self.thread_transcript_cache
                    .get(thread_id)
                    .and_then(|messages| {
                        messages
                            .iter()
                            .find(|message| message.role == AutopilotRole::User)
                            .and_then(|message| {
                                Self::sanitized_thread_title_candidate(message.content.as_str())
                            })
                    })
            })
            .or_else(|| {
                self.thread_metadata
                    .get(thread_id)
                    .and_then(|metadata| metadata.preview.as_deref())
                    .and_then(Self::sanitized_thread_title_candidate)
            })
    }

    fn sanitized_thread_title_candidate(value: &str) -> Option<String> {
        let normalized = value
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty() && !line.starts_with("```"))?
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if normalized.is_empty() {
            return None;
        }
        let mut candidate = normalized.chars().take(64).collect::<String>();
        if normalized.chars().count() > 64 {
            candidate = candidate
                .trim_end_matches([' ', ',', '.', ':', ';'])
                .to_string();
        }
        (!candidate.is_empty()).then_some(candidate)
    }

    pub fn thread_label(&self, thread_id: &str) -> String {
        let short_id = if thread_id.len() > 16 {
            &thread_id[..16]
        } else {
            thread_id
        };
        if let Some(metadata) = self.thread_metadata.get(thread_id)
            && let Some(name) = metadata.thread_name.as_deref()
            && !name.trim().is_empty()
        {
            return format!("{name} [{short_id}]");
        }
        short_id.to_string()
    }

    pub fn cycle_thread_filter_archived(&mut self) {
        self.thread_filter_archived = match self.thread_filter_archived {
            Some(false) => Some(true),
            Some(true) => None,
            None => Some(false),
        };
    }

    pub fn cycle_thread_filter_sort_key(&mut self) {
        self.thread_filter_sort_key = match self.thread_filter_sort_key {
            codex_client::ThreadSortKey::CreatedAt => codex_client::ThreadSortKey::UpdatedAt,
            codex_client::ThreadSortKey::UpdatedAt => codex_client::ThreadSortKey::CreatedAt,
        };
    }

    pub fn cycle_thread_filter_source_kind(&mut self) {
        self.thread_filter_source_kind = match self.thread_filter_source_kind {
            None => Some(codex_client::ThreadSourceKind::AppServer),
            Some(codex_client::ThreadSourceKind::AppServer) => {
                Some(codex_client::ThreadSourceKind::Cli)
            }
            Some(codex_client::ThreadSourceKind::Cli) => Some(codex_client::ThreadSourceKind::Exec),
            Some(codex_client::ThreadSourceKind::Exec) | Some(_) => None,
        };
    }

    pub fn cycle_thread_filter_model_provider(&mut self) {
        self.thread_filter_model_provider = match self.thread_filter_model_provider.as_deref() {
            None => Some("openai".to_string()),
            Some("openai") => Some("azure-openai".to_string()),
            _ => None,
        };
    }

    pub fn build_thread_list_params(&self, cwd: Option<String>) -> codex_client::ThreadListParams {
        codex_client::ThreadListParams {
            cwd,
            cursor: None,
            limit: Some(100),
            sort_key: Some(self.thread_filter_sort_key),
            model_providers: self
                .thread_filter_model_provider
                .as_ref()
                .map(|provider| vec![provider.clone()]),
            source_kinds: self.thread_filter_source_kind.map(|value| vec![value]),
            archived: self.thread_filter_archived,
            search_term: if self.thread_filter_search_term.trim().is_empty() {
                None
            } else {
                Some(self.thread_filter_search_term.trim().to_string())
            },
        }
    }

    pub fn next_thread_name(&mut self) -> String {
        let value = format!("Thread {}", self.thread_rename_counter);
        self.thread_rename_counter = self.thread_rename_counter.saturating_add(1);
        value
    }

    pub fn enqueue_command_approval(&mut self, request: AutopilotApprovalRequest) {
        self.pending_command_approvals.push(request);
    }

    pub fn enqueue_file_change_approval(&mut self, request: AutopilotFileChangeApprovalRequest) {
        self.pending_file_change_approvals.push(request);
    }

    pub fn enqueue_tool_call(&mut self, request: AutopilotToolCallRequest) {
        self.pending_tool_calls.push(request);
    }

    pub fn enqueue_tool_user_input(&mut self, request: AutopilotToolUserInputRequest) {
        self.pending_tool_user_input.push(request);
    }

    pub fn enqueue_auth_refresh(&mut self, request: AutopilotAuthRefreshRequest) {
        self.pending_auth_refresh.push(request);
    }

    pub fn pop_command_approval(&mut self) -> Option<AutopilotApprovalRequest> {
        if self.pending_command_approvals.is_empty() {
            None
        } else {
            Some(self.pending_command_approvals.remove(0))
        }
    }

    pub fn pop_file_change_approval(&mut self) -> Option<AutopilotFileChangeApprovalRequest> {
        if self.pending_file_change_approvals.is_empty() {
            None
        } else {
            Some(self.pending_file_change_approvals.remove(0))
        }
    }

    pub fn pop_tool_call(&mut self) -> Option<AutopilotToolCallRequest> {
        if self.pending_tool_calls.is_empty() {
            None
        } else {
            Some(self.pending_tool_calls.remove(0))
        }
    }

    pub fn pop_tool_user_input(&mut self) -> Option<AutopilotToolUserInputRequest> {
        if self.pending_tool_user_input.is_empty() {
            None
        } else {
            Some(self.pending_tool_user_input.remove(0))
        }
    }

    pub fn pop_auth_refresh(&mut self) -> Option<AutopilotAuthRefreshRequest> {
        if self.pending_auth_refresh.is_empty() {
            None
        } else {
            Some(self.pending_auth_refresh.remove(0))
        }
    }

    pub fn has_pending_messages(&self) -> bool {
        self.messages.iter().any(|message| {
            matches!(
                message.status,
                AutopilotMessageStatus::Queued | AutopilotMessageStatus::Running
            )
        })
    }

    pub fn turn_has_visible_output(&self, turn_id: &str) -> bool {
        let Some(assistant_message_id) = self
            .turn_assistant_message_ids
            .get(turn_id)
            .copied()
            .or(self.active_assistant_message_id)
        else {
            return false;
        };
        self.messages
            .iter()
            .find(|message| message.id == assistant_message_id)
            .is_some_and(|message| !message.content.trim().is_empty())
    }

    fn bind_turn_to_assistant_message(&mut self, turn_id: &str) -> Option<u64> {
        if let Some(existing) = self.turn_assistant_message_ids.get(turn_id).copied() {
            return Some(existing);
        }
        let assistant_message_id = self
            .pending_assistant_message_ids
            .pop_front()
            .or(self.active_assistant_message_id)?;
        self.turn_assistant_message_ids
            .insert(turn_id.to_string(), assistant_message_id);
        Some(assistant_message_id)
    }

    pub fn is_duplicate_agent_delta(&mut self, turn_id: &str, item_id: &str, delta: &str) -> bool {
        Self::is_duplicate_delta(
            turn_id,
            item_id,
            delta,
            &mut self.last_agent_item_ids,
            &mut self.last_agent_delta_signature,
            |value| value.is_empty() || value == "event-agent-message" || value == "n/a",
        )
    }

    pub fn is_duplicate_reasoning_delta(
        &mut self,
        turn_id: &str,
        item_id: &str,
        delta: &str,
    ) -> bool {
        Self::is_duplicate_delta(
            turn_id,
            item_id,
            delta,
            &mut self.last_reasoning_item_ids,
            &mut self.last_reasoning_delta_signature,
            |value| value.is_empty() || value == "event-reasoning" || value == "n/a",
        )
    }

    fn is_duplicate_delta(
        turn_id: &str,
        item_id: &str,
        delta: &str,
        item_ids_by_turn: &mut std::collections::HashMap<String, String>,
        last_signature: &mut Option<AutopilotDeltaSignature>,
        is_fallback_item: impl Fn(&str) -> bool,
    ) -> bool {
        if delta.is_empty() {
            return false;
        }

        let canonical_item_id = if is_fallback_item(item_id) {
            item_ids_by_turn
                .get(turn_id)
                .cloned()
                .unwrap_or_else(|| item_id.to_string())
        } else {
            item_ids_by_turn.insert(turn_id.to_string(), item_id.to_string());
            item_id.to_string()
        };

        let candidate = AutopilotDeltaSignature {
            turn_id: turn_id.to_string(),
            item_id: canonical_item_id,
            delta: delta.to_string(),
        };
        let duplicate = last_signature
            .as_ref()
            .is_some_and(|last| *last == candidate);
        *last_signature = Some(candidate);
        duplicate
    }
}

fn labor_tool_evidence_ref(
    work_unit_id: &str,
    request_id: &str,
    call_id: &str,
    tool_name: &str,
    response_code: &str,
    success: bool,
    response_message_digest: &str,
) -> EvidenceRef {
    let uri = format!("oa://autopilot/codex/{work_unit_id}/tools/{request_id}/{call_id}");
    let digest = sha256_prefixed_text(
        format!(
            "{work_unit_id}:{request_id}:{call_id}:{tool_name}:{response_code}:{success}:{response_message_digest}"
        )
        .as_str(),
    );
    let mut evidence = EvidenceRef::new("codex_tool_invocation", uri, digest);
    evidence.meta.insert(
        "tool_name".to_string(),
        Value::String(tool_name.to_string()),
    );
    evidence.meta.insert(
        "response_code".to_string(),
        Value::String(response_code.to_string()),
    );
    evidence
        .meta
        .insert("success".to_string(), Value::Bool(success));
    evidence
}

pub use crate::state::provider_runtime::{
    EarnFailureClass, ProviderBlocker, ProviderInventoryProductToggleTarget, ProviderMode,
    ProviderRuntimeState,
};
#[allow(unused_imports)]
pub use crate::state::{
    alerts_recovery::{
        AlertDomain, AlertLifecycle, AlertSeverity, AlertsRecoveryState, RecoveryAlertRow,
    },
    job_inbox::{
        JobDemandSource, JobInboxDecision, JobInboxNetworkRequest, JobInboxRequest, JobInboxState,
        JobInboxValidation,
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaneLoadState {
    Loading,
    Ready,
    Error,
}

impl PaneLoadState {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Loading => "loading",
            Self::Ready => "ready",
            Self::Error => "error",
        }
    }
}

pub trait PaneStatusAccess {
    fn pane_load_state_mut(&mut self) -> &mut PaneLoadState;
    fn pane_last_error_mut(&mut self) -> &mut Option<String>;
    fn pane_last_action_mut(&mut self) -> &mut Option<String>;

    fn pane_set_ready(&mut self, action: impl Into<String>) {
        *self.pane_load_state_mut() = PaneLoadState::Ready;
        *self.pane_last_error_mut() = None;
        *self.pane_last_action_mut() = Some(action.into());
    }

    fn pane_set_error(&mut self, error: impl Into<String>) -> String {
        let error = error.into();
        *self.pane_load_state_mut() = PaneLoadState::Error;
        *self.pane_last_error_mut() = Some(error.clone());
        error
    }

    fn pane_clear_error(&mut self) {
        *self.pane_last_error_mut() = None;
    }
}

macro_rules! impl_pane_status_access {
    ($($state:ty),+ $(,)?) => {
        $(
            impl PaneStatusAccess for $state {
                fn pane_load_state_mut(&mut self) -> &mut PaneLoadState {
                    &mut self.load_state
                }

                fn pane_last_error_mut(&mut self) -> &mut Option<String> {
                    &mut self.last_error
                }

                fn pane_last_action_mut(&mut self) -> &mut Option<String> {
                    &mut self.last_action
                }
            }
        )+
    };
}

#[allow(unused_imports)]
pub use crate::state::operations::{
    AcceptedForwardComputeOrder, AcceptedSpotComputeOrder, BuyerResolutionMode,
    BuyerResolutionReason, ComputeQuoteMode, ForwardComputeQuoteCandidate, ForwardComputeRfqDraft,
    NetworkRequestStatus, NetworkRequestSubmission, NetworkRequestsState, ReciprocalLoopDirection,
    ReciprocalLoopFailureClass, ReciprocalLoopFailureDisposition, ReciprocalLoopState,
    RelayConnectionRow, RelayConnectionStatus, RelayConnectionsState,
    SpotComputeCapabilityConstraints, SpotComputeQuoteCandidate, SpotComputeRfqDraft,
    StarterJobRow, StarterJobStatus, StarterJobsState, SubmittedNetworkRequest, SyncHealthState,
    SyncRecoveryPhase,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ActivityEventDomain {
    Chat,
    Cad,
    Job,
    Wallet,
    Network,
    Sync,
    Sa,
    Skl,
    Ac,
}

impl ActivityEventDomain {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Cad => "cad",
            Self::Job => "job",
            Self::Wallet => "wallet",
            Self::Network => "network",
            Self::Sync => "sync",
            Self::Sa => "sa",
            Self::Skl => "skl",
            Self::Ac => "ac",
        }
    }

    pub const fn source_tag(self) -> &'static str {
        match self {
            Self::Chat => "chat.lane",
            Self::Cad => "cad.events",
            Self::Job => "provider.runtime",
            Self::Wallet => "spark.wallet",
            Self::Network => "nip90.network",
            Self::Sync => "spacetime.sync",
            Self::Sa => "nostr.sa",
            Self::Skl => "nostr.skl",
            Self::Ac => "nostr.ac",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivityFeedFilter {
    All,
    Chat,
    Cad,
    Job,
    Wallet,
    Network,
    Sync,
    Sa,
    Skl,
    Ac,
    Nip90,
}

impl ActivityFeedFilter {
    pub const fn all() -> [Self; 11] {
        [
            Self::All,
            Self::Chat,
            Self::Cad,
            Self::Job,
            Self::Wallet,
            Self::Network,
            Self::Sync,
            Self::Sa,
            Self::Skl,
            Self::Ac,
            Self::Nip90,
        ]
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Chat => "chat",
            Self::Cad => "cad",
            Self::Job => "job",
            Self::Wallet => "wallet",
            Self::Network => "network",
            Self::Sync => "sync",
            Self::Sa => "sa",
            Self::Skl => "skl",
            Self::Ac => "ac",
            Self::Nip90 => "nip90",
        }
    }

    pub fn matches_row(self, row: &ActivityEventRow) -> bool {
        if !self.matches_domain(row.domain) {
            return false;
        }
        match self {
            Self::Nip90 => row.source_tag.starts_with("nip90."),
            _ => true,
        }
    }

    pub fn matches_domain(self, domain: ActivityEventDomain) -> bool {
        match self {
            Self::All => true,
            Self::Chat => domain == ActivityEventDomain::Chat,
            Self::Cad => domain == ActivityEventDomain::Cad,
            Self::Job => domain == ActivityEventDomain::Job,
            Self::Wallet => domain == ActivityEventDomain::Wallet,
            Self::Network => domain == ActivityEventDomain::Network,
            Self::Sync => domain == ActivityEventDomain::Sync,
            Self::Sa => domain == ActivityEventDomain::Sa,
            Self::Skl => domain == ActivityEventDomain::Skl,
            Self::Ac => domain == ActivityEventDomain::Ac,
            Self::Nip90 => domain == ActivityEventDomain::Network,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ActivityEventRow {
    pub event_id: String,
    pub domain: ActivityEventDomain,
    pub source_tag: String,
    pub occurred_at_epoch_seconds: u64,
    pub summary: String,
    pub detail: String,
}

const ACTIVITY_PROJECTION_SCHEMA_VERSION: u16 = 1;
const ACTIVITY_PROJECTION_STREAM_ID: &str = "stream.activity_projection.v1";
const ACTIVITY_PROJECTION_ROW_LIMIT: usize = 256;
const ACTIVITY_FEED_PAGE_SIZE: usize = 8;
const ACTIVITY_FEED_NIP90_WINDOW_SIZE: usize = 50;
const ACTIVITY_FEED_SCROLL_NOTCH_PIXELS: f32 = 24.0;

#[derive(Debug, Serialize, Deserialize)]
struct ActivityProjectionDocumentV1 {
    schema_version: u16,
    stream_id: String,
    rows: Vec<ActivityEventRow>,
}

pub struct ActivityFeedState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub active_filter: ActivityFeedFilter,
    pub page: usize,
    pub rows: Vec<ActivityEventRow>,
    pub selected_event_id: Option<String>,
    pub detail_scroll_line_offset: usize,
    pub projection_stream_id: String,
    projection_file_path: PathBuf,
}

impl Default for ActivityFeedState {
    fn default() -> Self {
        let projection_file_path = activity_projection_file_path();
        Self::from_projection_file_path(projection_file_path)
    }
}

impl ActivityFeedState {
    fn from_projection_file_path(projection_file_path: PathBuf) -> Self {
        let (rows, load_state, last_error, last_action) =
            match load_activity_projection_rows(projection_file_path.as_path()) {
                Ok(rows) => (
                    rows,
                    PaneLoadState::Ready,
                    None,
                    Some("Loaded activity projection stream".to_string()),
                ),
                Err(error) => (
                    Vec::new(),
                    PaneLoadState::Error,
                    Some(error),
                    Some("Activity projection stream load failed".to_string()),
                ),
            };
        let selected_event_id = rows.first().map(|row| row.event_id.clone());
        Self {
            load_state,
            last_error,
            last_action: last_action.map(|action| format!("{action} ({} rows)", rows.len())),
            active_filter: ActivityFeedFilter::All,
            page: 0,
            rows,
            selected_event_id,
            detail_scroll_line_offset: 0,
            projection_stream_id: ACTIVITY_PROJECTION_STREAM_ID.to_string(),
            projection_file_path,
        }
    }

    #[cfg(test)]
    fn from_projection_path_for_tests(projection_file_path: PathBuf) -> Self {
        Self::from_projection_file_path(projection_file_path)
    }

    pub fn visible_rows(&self) -> Vec<&ActivityEventRow> {
        let filtered = self.filtered_rows();
        let page = self.page.min(self.total_pages().saturating_sub(1));
        let start = page.saturating_mul(ACTIVITY_FEED_PAGE_SIZE);
        let end = (start + ACTIVITY_FEED_PAGE_SIZE).min(filtered.len());
        filtered[start..end].to_vec()
    }

    fn filtered_rows(&self) -> Vec<&ActivityEventRow> {
        let mut rows = self
            .rows
            .iter()
            .filter(|row| self.active_filter.matches_row(row))
            .collect::<Vec<_>>();
        if self.active_filter == ActivityFeedFilter::Nip90 {
            rows.truncate(ACTIVITY_FEED_NIP90_WINDOW_SIZE);
        }
        rows
    }

    pub fn filtered_row_count(&self) -> usize {
        self.filtered_rows().len()
    }

    pub fn total_pages(&self) -> usize {
        let filtered = self.filtered_rows();
        ((filtered.len() + ACTIVITY_FEED_PAGE_SIZE.saturating_sub(1))
            / ACTIVITY_FEED_PAGE_SIZE.max(1))
        .max(1)
    }

    pub fn previous_page(&mut self) {
        if self.page > 0 {
            self.page -= 1;
            self.ensure_selected_visible();
            self.reset_detail_scroll();
            self.pane_set_ready(format!(
                "Activity page -> {}/{}",
                self.page.saturating_add(1),
                self.total_pages()
            ));
        }
    }

    pub fn next_page(&mut self) {
        let total_pages = self.total_pages();
        if self.page + 1 < total_pages {
            self.page += 1;
            self.ensure_selected_visible();
            self.reset_detail_scroll();
            self.pane_set_ready(format!(
                "Activity page -> {}/{}",
                self.page.saturating_add(1),
                total_pages
            ));
        }
    }

    fn clamp_page(&mut self) {
        self.page = self.page.min(self.total_pages().saturating_sub(1));
    }

    fn ensure_selected_visible(&mut self) {
        let (selected_visible, first_visible_id) = {
            let visible = self.visible_rows();
            let selected_visible = self
                .selected_event_id
                .as_deref()
                .is_some_and(|selected| visible.iter().any(|row| row.event_id == selected));
            let first_visible_id = visible.first().map(|row| row.event_id.clone());
            (selected_visible, first_visible_id)
        };
        if !selected_visible {
            if self.selected_event_id != first_visible_id {
                self.reset_detail_scroll();
            }
            self.selected_event_id = first_visible_id;
        }
    }

    pub fn selected(&self) -> Option<&ActivityEventRow> {
        let selected = self.selected_event_id.as_deref()?;
        self.rows.iter().find(|row| row.event_id == selected)
    }

    pub fn select_visible_row(&mut self, index: usize) -> bool {
        let Some(event_id) = self
            .visible_rows()
            .get(index)
            .map(|row| row.event_id.clone())
        else {
            return false;
        };
        self.reset_detail_scroll();
        self.selected_event_id = Some(event_id);
        self.pane_clear_error();
        true
    }

    pub fn set_filter(&mut self, filter: ActivityFeedFilter) {
        self.active_filter = filter;
        self.page = 0;
        self.reset_detail_scroll();
        self.ensure_selected_visible();
        self.pane_set_ready(format!("Activity filter -> {}", filter.label()));
    }

    pub fn detail_scroll_offset_for(&self, total_lines: usize, visible_lines: usize) -> usize {
        let visible_lines = visible_lines.max(1);
        let max_start = total_lines.saturating_sub(visible_lines);
        self.detail_scroll_line_offset.min(max_start)
    }

    pub fn scroll_detail_lines_by(
        &mut self,
        delta_pixels: f32,
        total_lines: usize,
        visible_lines: usize,
    ) -> bool {
        if !delta_pixels.is_finite() || delta_pixels.abs() <= f32::EPSILON {
            return false;
        }
        let visible_lines = visible_lines.max(1);
        let max_start = total_lines.saturating_sub(visible_lines);
        if max_start == 0 {
            if self.detail_scroll_line_offset != 0 {
                self.detail_scroll_line_offset = 0;
                return true;
            }
            return false;
        }

        let mut line_delta = (delta_pixels / ACTIVITY_FEED_SCROLL_NOTCH_PIXELS).round() as isize;
        if line_delta == 0 {
            line_delta = if delta_pixels.is_sign_positive() {
                1
            } else {
                -1
            };
        }

        let next = (self
            .detail_scroll_offset_for(total_lines, visible_lines)
            .saturating_add_signed(line_delta))
        .min(max_start);
        if next == self.detail_scroll_line_offset {
            return false;
        }
        self.detail_scroll_line_offset = next;
        true
    }

    pub fn reset_detail_scroll(&mut self) {
        self.detail_scroll_line_offset = 0;
    }

    pub fn upsert_event(&mut self, row: ActivityEventRow) {
        if let Some(existing) = self
            .rows
            .iter_mut()
            .find(|existing| existing.event_id == row.event_id)
        {
            *existing = row;
        } else {
            self.rows.push(row);
        }
        self.rows = normalize_activity_projection_rows(std::mem::take(&mut self.rows));
        self.clamp_page();
        self.ensure_selected_visible();

        if let Err(error) = persist_activity_projection_rows(
            self.projection_file_path.as_path(),
            self.rows.as_slice(),
        ) {
            self.last_error = Some(error);
            self.load_state = PaneLoadState::Error;
        } else {
            self.load_state = PaneLoadState::Ready;
            self.last_error = None;
        }
    }

    pub fn reload_projection(&mut self) -> Result<(), String> {
        let rows = load_activity_projection_rows(self.projection_file_path.as_path())
            .map_err(|error| self.pane_set_error(error))?;
        self.rows = rows;
        self.clamp_page();
        self.ensure_selected_visible();
        self.pane_set_ready(format!(
            "Activity projection reloaded ({} events)",
            self.rows.len()
        ));
        Ok(())
    }
}

fn activity_projection_file_path() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".openagents")
        .join("autopilot-activity-projection-v1.json")
}

fn normalize_activity_projection_rows(mut rows: Vec<ActivityEventRow>) -> Vec<ActivityEventRow> {
    rows.sort_by(|lhs, rhs| {
        rhs.occurred_at_epoch_seconds
            .cmp(&lhs.occurred_at_epoch_seconds)
            .then_with(|| lhs.event_id.cmp(&rhs.event_id))
    });
    let mut seen_event_ids = HashSet::new();
    rows.retain(|row| seen_event_ids.insert(row.event_id.clone()));
    rows.truncate(ACTIVITY_PROJECTION_ROW_LIMIT);
    rows
}

fn persist_activity_projection_rows(path: &Path, rows: &[ActivityEventRow]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create activity projection dir: {error}"))?;
    }
    let document = ActivityProjectionDocumentV1 {
        schema_version: ACTIVITY_PROJECTION_SCHEMA_VERSION,
        stream_id: ACTIVITY_PROJECTION_STREAM_ID.to_string(),
        rows: normalize_activity_projection_rows(rows.to_vec()),
    };
    let payload = serde_json::to_string_pretty(&document)
        .map_err(|error| format!("Failed to encode activity projection rows: {error}"))?;
    let temp_path = path.with_extension("tmp");
    std::fs::write(&temp_path, payload)
        .map_err(|error| format!("Failed to write activity projection temp file: {error}"))?;
    std::fs::rename(&temp_path, path)
        .map_err(|error| format!("Failed to persist activity projection rows: {error}"))?;
    Ok(())
}

fn load_activity_projection_rows(path: &Path) -> Result<Vec<ActivityEventRow>, String> {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("Failed to read activity projection rows: {error}")),
    };
    let document = serde_json::from_str::<ActivityProjectionDocumentV1>(&raw)
        .map_err(|error| format!("Failed to parse activity projection rows: {error}"))?;
    if document.schema_version != ACTIVITY_PROJECTION_SCHEMA_VERSION {
        return Err(format!(
            "Unsupported activity projection schema version: {}",
            document.schema_version
        ));
    }
    if document.stream_id != ACTIVITY_PROJECTION_STREAM_ID {
        return Err(format!(
            "Unsupported activity projection stream id: {}",
            document.stream_id
        ));
    }
    Ok(normalize_activity_projection_rows(document.rows))
}

pub(crate) const DEFAULT_NEXUS_PRIMARY_RELAY_URL: &str = "wss://nexus.openagents.com/";
const DEFAULT_PUBLIC_BACKUP_RELAY_URLS: [&str; 2] =
    ["wss://relay.primal.net", "wss://relay.damus.io"];

pub(crate) const ENV_DEFAULT_NIP28_RELAY_URL: &str = "OA_DEFAULT_NIP28_RELAY_URL";
pub(crate) const ENV_DEFAULT_NIP28_CHANNEL_ID: &str = "OA_DEFAULT_NIP28_CHANNEL_ID";
const DEFAULT_NIP28_RELAY_URL: &str = "wss://relay.damus.io";
const DEFAULT_NIP28_CHANNEL_ID: &str =
    "ebf2e35092632ecb81b0f7da7d3b25b4c1b0e8e7eb98d7d766ef584e9edd68c8";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultNip28ChannelConfig {
    pub relay_url: String,
    pub channel_id: String,
}

impl DefaultNip28ChannelConfig {
    pub fn from_env_or_default() -> Self {
        Self {
            relay_url: std::env::var(ENV_DEFAULT_NIP28_RELAY_URL)
                .unwrap_or_else(|_| DEFAULT_NIP28_RELAY_URL.to_string()),
            channel_id: std::env::var(ENV_DEFAULT_NIP28_CHANNEL_ID)
                .unwrap_or_else(|_| DEFAULT_NIP28_CHANNEL_ID.to_string()),
        }
    }

    pub fn is_valid(&self) -> bool {
        let id = &self.channel_id;
        id.len() == 64 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettingsDocumentV1 {
    pub schema_version: u16,
    pub primary_relay_url: String,
    pub backup_relay_urls: Vec<String>,
    pub identity_path: String,
    pub wallet_default_send_sats: u64,
    pub provider_max_queue_depth: u32,
    pub reconnect_required: bool,
}

impl Default for SettingsDocumentV1 {
    fn default() -> Self {
        Self {
            schema_version: 2,
            primary_relay_url: DEFAULT_NEXUS_PRIMARY_RELAY_URL.to_string(),
            backup_relay_urls: DEFAULT_PUBLIC_BACKUP_RELAY_URLS
                .iter()
                .map(|value| value.to_string())
                .collect(),
            identity_path: settings_identity_path(),
            wallet_default_send_sats: 1000,
            provider_max_queue_depth: 1,
            reconnect_required: false,
        }
    }
}

impl SettingsDocumentV1 {
    pub fn configured_relay_urls(&self) -> Vec<String> {
        let mut relays = Vec::with_capacity(self.backup_relay_urls.len().saturating_add(1));
        let primary = self.primary_relay_url.trim();
        if !primary.is_empty() {
            relays.push(primary.to_string());
        }
        relays.extend(
            self.backup_relay_urls
                .iter()
                .map(|relay| relay.trim())
                .filter(|relay| !relay.is_empty())
                .map(ToString::to_string),
        );
        dedupe_relay_urls(relays)
    }
}

pub struct SettingsState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub document: SettingsDocumentV1,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Ready,
            last_error: None,
            last_action: Some("Settings loaded from migration-safe defaults".to_string()),
            document: SettingsDocumentV1::default(),
        }
    }
}

impl SettingsState {
    pub fn load_from_disk() -> Self {
        let path = settings_file_path();
        let mut state = Self::default();
        match std::fs::read_to_string(&path) {
            Ok(raw) => match parse_settings_document(&raw) {
                Ok(document) => {
                    state.document = document;
                    state.pane_set_ready(format!("Settings loaded from {}", path.display()));
                }
                Err(error) => {
                    let _ = state.pane_set_error(format!("Settings parse error: {error}"));
                    *state.pane_last_action_mut() =
                        Some("Using migration-safe defaults".to_string());
                }
            },
            Err(error) => {
                if error.kind() != std::io::ErrorKind::NotFound {
                    let _ = state.pane_set_error(format!("Settings read error: {error}"));
                }
            }
        }

        state
    }

    pub fn apply_updates(
        &mut self,
        primary_relay_url: &str,
        wallet_default_send_sats: &str,
        provider_max_queue_depth: &str,
    ) -> Result<(), String> {
        self.apply_updates_internal(
            primary_relay_url,
            wallet_default_send_sats,
            provider_max_queue_depth,
            true,
        )
    }

    fn apply_updates_internal(
        &mut self,
        primary_relay_url: &str,
        wallet_default_send_sats: &str,
        provider_max_queue_depth: &str,
        persist: bool,
    ) -> Result<(), String> {
        let primary_relay_url = primary_relay_url.trim();
        if primary_relay_url.is_empty() {
            return Err(self.pane_set_error("Relay URL is required"));
        }
        if !primary_relay_url.starts_with("wss://") {
            return Err(self.pane_set_error("Relay URL must start with wss://"));
        }

        let wallet_default_send_sats = wallet_default_send_sats
            .trim()
            .parse::<u64>()
            .map_err(|error| format!("Wallet default send sats must be an integer: {error}"))?;
        if wallet_default_send_sats == 0 || wallet_default_send_sats > 10_000_000 {
            return Err(
                self.pane_set_error("Wallet default send sats must be between 1 and 10,000,000")
            );
        }

        let provider_max_queue_depth = provider_max_queue_depth
            .trim()
            .parse::<u32>()
            .map_err(|error| format!("Provider max queue depth must be an integer: {error}"))?;
        if provider_max_queue_depth != 1 {
            return Err(
                self.pane_set_error("MVP currently supports exactly 1 inflight provider job")
            );
        }

        let reconnect_required = primary_relay_url != self.document.primary_relay_url
            || provider_max_queue_depth != self.document.provider_max_queue_depth;
        if primary_relay_url != self.document.primary_relay_url {
            let previous_primary = self.document.primary_relay_url.clone();
            let mut backup_relay_urls = self.document.backup_relay_urls.clone();
            backup_relay_urls.retain(|relay| relay != primary_relay_url);
            if !previous_primary.trim().is_empty() && previous_primary != primary_relay_url {
                backup_relay_urls.insert(0, previous_primary);
            }
            self.document.primary_relay_url = primary_relay_url.to_string();
            self.document.backup_relay_urls = dedupe_relay_urls(backup_relay_urls);
        }
        self.document.wallet_default_send_sats = wallet_default_send_sats;
        self.document.provider_max_queue_depth = provider_max_queue_depth;
        self.document.reconnect_required = reconnect_required;

        if persist {
            self.persist_to_disk()?;
        }

        self.pane_set_ready(if reconnect_required {
            "Saved settings. Relay/provider changes require reconnect.".to_string()
        } else {
            "Saved settings.".to_string()
        });
        Ok(())
    }

    pub fn reset_defaults(&mut self) -> Result<(), String> {
        self.reset_defaults_internal(true)
    }

    fn reset_defaults_internal(&mut self, persist: bool) -> Result<(), String> {
        self.document = SettingsDocumentV1::default();
        if persist {
            self.persist_to_disk()?;
        }
        self.pane_set_ready("Reset settings to schema defaults.");
        Ok(())
    }

    pub fn add_backup_relay(&mut self, relay_url: &str, persist: bool) -> Result<(), String> {
        let relay_url = relay_url.trim();
        if relay_url.is_empty() {
            return Err(self.pane_set_error("Relay URL cannot be empty"));
        }
        if !relay_url.starts_with("wss://") {
            return Err(self.pane_set_error("Relay URL must start with wss://"));
        }
        if relay_url == self.document.primary_relay_url {
            return Err(self.pane_set_error("Relay is already configured as primary"));
        }
        if self
            .document
            .backup_relay_urls
            .iter()
            .any(|existing| existing == relay_url)
        {
            return Err(self.pane_set_error("Relay already configured"));
        }

        self.document.backup_relay_urls.push(relay_url.to_string());
        self.document.backup_relay_urls =
            dedupe_relay_urls(self.document.backup_relay_urls.clone());
        self.document.reconnect_required = true;
        if persist {
            self.persist_to_disk()?;
        }
        self.pane_set_ready(format!("Added backup relay {relay_url}"));
        Ok(())
    }

    pub fn remove_configured_relay(
        &mut self,
        relay_url: &str,
        persist: bool,
    ) -> Result<String, String> {
        let relay_url = relay_url.trim();
        if relay_url.is_empty() {
            return Err(self.pane_set_error("Select a relay first"));
        }

        let message = if relay_url == self.document.primary_relay_url {
            if self.document.backup_relay_urls.is_empty() {
                return Err(self.pane_set_error("At least one relay must remain configured"));
            }
            let next_primary = self.document.backup_relay_urls.remove(0);
            self.document.primary_relay_url = next_primary.clone();
            format!("Removed primary relay {relay_url}; promoted {next_primary}")
        } else {
            let before = self.document.backup_relay_urls.len();
            self.document
                .backup_relay_urls
                .retain(|relay| relay != relay_url);
            if self.document.backup_relay_urls.len() == before {
                return Err(self.pane_set_error("Selected relay no longer exists"));
            }
            format!("Removed backup relay {relay_url}")
        };

        self.document.backup_relay_urls =
            dedupe_relay_urls(self.document.backup_relay_urls.clone());
        self.document.reconnect_required = true;
        if persist {
            self.persist_to_disk()?;
        }
        self.pane_set_ready(message.clone());
        Ok(message)
    }

    fn persist_to_disk(&mut self) -> Result<(), String> {
        let path = settings_file_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("Failed to create settings dir: {error}"))?;
        }
        std::fs::write(&path, serialize_settings_document(&self.document))
            .map_err(|error| format!("Failed to persist settings: {error}"))?;
        Ok(())
    }
}

impl SettingsPaneInputs {
    pub fn from_state(settings: &SettingsState) -> Self {
        Self {
            relay_url: TextInput::new()
                .value(settings.document.primary_relay_url.clone())
                .placeholder("wss://relay.example.com"),
            wallet_default_send_sats: TextInput::new()
                .value(settings.document.wallet_default_send_sats.to_string())
                .placeholder("Default send sats"),
            provider_max_queue_depth: TextInput::new()
                .value(settings.document.provider_max_queue_depth.to_string())
                .placeholder("Provider max queue depth"),
        }
    }

    pub fn sync_from_state(&mut self, settings: &SettingsState) {
        self.relay_url
            .set_value(settings.document.primary_relay_url.clone());
        self.wallet_default_send_sats
            .set_value(settings.document.wallet_default_send_sats.to_string());
        self.provider_max_queue_depth
            .set_value(settings.document.provider_max_queue_depth.to_string());
    }
}

fn settings_file_path() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".openagents")
        .join("autopilot-settings-v1.conf")
}

fn settings_identity_path() -> String {
    nostr::identity_mnemonic_path()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "~/.openagents/pylon/identity.mnemonic".to_string())
}

fn serialize_settings_document(document: &SettingsDocumentV1) -> String {
    format!(
        "schema_version={}\nprimary_relay_url={}\nbackup_relay_urls={}\nidentity_path={}\nwallet_default_send_sats={}\nprovider_max_queue_depth={}\nreconnect_required={}\n",
        document.schema_version,
        document.primary_relay_url,
        document.backup_relay_urls.join(","),
        document.identity_path,
        document.wallet_default_send_sats,
        document.provider_max_queue_depth,
        document.reconnect_required,
    )
}

fn parse_settings_document(raw: &str) -> Result<SettingsDocumentV1, String> {
    let mut document = SettingsDocumentV1::default();
    let mut legacy_relay_url = None::<String>;
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            return Err(format!("Invalid settings line: {trimmed}"));
        };
        match key.trim() {
            "schema_version" => {
                document.schema_version = value
                    .trim()
                    .parse::<u16>()
                    .map_err(|error| format!("Invalid schema version: {error}"))?;
            }
            "primary_relay_url" => document.primary_relay_url = value.trim().to_string(),
            "backup_relay_urls" => {
                document.backup_relay_urls = value
                    .split(',')
                    .map(str::trim)
                    .filter(|relay| !relay.is_empty())
                    .map(ToOwned::to_owned)
                    .collect();
            }
            "relay_url" => legacy_relay_url = Some(value.trim().to_string()),
            "identity_path" => document.identity_path = value.trim().to_string(),
            "wallet_default_send_sats" => {
                document.wallet_default_send_sats = value
                    .trim()
                    .parse::<u64>()
                    .map_err(|error| format!("Invalid wallet_default_send_sats: {error}"))?;
            }
            "provider_max_queue_depth" => {
                document.provider_max_queue_depth = value
                    .trim()
                    .parse::<u32>()
                    .map_err(|error| format!("Invalid provider_max_queue_depth: {error}"))?;
            }
            "reconnect_required" => {
                document.reconnect_required = value
                    .trim()
                    .parse::<bool>()
                    .map_err(|error| format!("Invalid reconnect_required: {error}"))?;
            }
            _ => {}
        }
    }

    if document.schema_version == 1
        && let Some(legacy_relay_url) = legacy_relay_url
    {
        document.primary_relay_url = legacy_relay_url;
        document.backup_relay_urls.clear();
        document.schema_version = 2;
    }

    if document.schema_version != 2 {
        return Err(format!(
            "Unsupported schema version {}, expected 2",
            document.schema_version
        ));
    }

    if document.primary_relay_url.trim().is_empty() {
        document.primary_relay_url = DEFAULT_NEXUS_PRIMARY_RELAY_URL.to_string();
    }
    document
        .backup_relay_urls
        .retain(|relay| relay != &document.primary_relay_url);
    document.backup_relay_urls = dedupe_relay_urls(document.backup_relay_urls);

    // Identity path authority is the resolved mnemonic path.
    document.identity_path = settings_identity_path();
    document.provider_max_queue_depth = 1;

    Ok(document)
}

fn dedupe_relay_urls(relays: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::<String>::new();
    relays
        .into_iter()
        .map(|relay| relay.trim().to_string())
        .filter(|relay| !relay.is_empty())
        .filter(|relay| seen.insert(relay.clone()))
        .collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum JobLifecycleStage {
    Received,
    Accepted,
    Running,
    Delivered,
    Paid,
    Failed,
}

impl JobLifecycleStage {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Received => "received",
            Self::Accepted => "accepted",
            Self::Running => "running",
            Self::Delivered => "delivered",
            Self::Paid => "paid",
            Self::Failed => "failed",
        }
    }

    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Paid | Self::Failed)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveJobEvent {
    pub seq: u64,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveJobRecord {
    pub job_id: String,
    pub request_id: String,
    pub requester: String,
    pub demand_source: JobDemandSource,
    pub request_kind: u16,
    pub capability: String,
    pub execution_input: Option<String>,
    pub execution_prompt: Option<String>,
    pub execution_params: Vec<crate::state::job_inbox::JobExecutionParam>,
    pub requested_model: Option<String>,
    pub execution_provenance: Option<LocalInferenceExecutionProvenance>,
    pub skill_scope_id: Option<String>,
    pub skl_manifest_a: Option<String>,
    pub skl_manifest_event_id: Option<String>,
    pub sa_tick_request_event_id: Option<String>,
    pub sa_tick_result_event_id: Option<String>,
    pub sa_trajectory_session_id: Option<String>,
    pub ac_envelope_event_id: Option<String>,
    pub ac_settlement_event_id: Option<String>,
    pub ac_default_event_id: Option<String>,
    pub compute_product_id: Option<String>,
    pub capacity_lot_id: Option<String>,
    pub capacity_instrument_id: Option<String>,
    pub delivery_proof_id: Option<String>,
    pub delivery_metering_rule_id: Option<String>,
    pub delivery_proof_status_label: Option<String>,
    pub delivery_metered_quantity: Option<u64>,
    pub delivery_accepted_quantity: Option<u64>,
    pub delivery_variance_reason_label: Option<String>,
    pub delivery_rejection_reason_label: Option<String>,
    pub quoted_price_sats: u64,
    pub ttl_seconds: u64,
    pub stage: JobLifecycleStage,
    pub invoice_id: Option<String>,
    pub payment_id: Option<String>,
    pub failure_reason: Option<String>,
    pub events: Vec<ActiveJobEvent>,
}

pub struct ActiveJobState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub scroll_offset_px: f32,
    pub runtime_supports_abort: bool,
    pub execution_thread_id: Option<String>,
    pub execution_turn_id: Option<String>,
    pub execution_output: Option<String>,
    pub execution_turn_completed: bool,
    pub execution_backend_request_id: Option<String>,
    pub execution_thread_start_command_seq: Option<u64>,
    pub execution_turn_start_command_seq: Option<u64>,
    pub execution_turn_interrupt_command_seq: Option<u64>,
    pub execution_deadline_epoch_seconds: Option<u64>,
    pub result_publish_in_flight: bool,
    pub pending_result_publish_event_id: Option<String>,
    pub pending_result_publish_event: Option<Event>,
    pub result_publish_attempt_count: u32,
    pub result_publish_first_queued_epoch_seconds: Option<u64>,
    pub result_publish_last_queued_epoch_seconds: Option<u64>,
    pub payment_required_invoice_requested: bool,
    pub payment_required_feedback_in_flight: bool,
    pub payment_required_failed: bool,
    pub next_payment_evidence_refresh_at: Option<Instant>,
    pub pending_bolt11_created_at_epoch_seconds: Option<u64>,
    pub pending_bolt11: Option<String>,
    pub job: Option<ActiveJobRecord>,
    next_event_seq: u64,
}

impl Default for ActiveJobState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for active job lane snapshot".to_string()),
            scroll_offset_px: 0.0,
            runtime_supports_abort: false,
            execution_thread_id: None,
            execution_turn_id: None,
            execution_output: None,
            execution_turn_completed: false,
            execution_backend_request_id: None,
            execution_thread_start_command_seq: None,
            execution_turn_start_command_seq: None,
            execution_turn_interrupt_command_seq: None,
            execution_deadline_epoch_seconds: None,
            result_publish_in_flight: false,
            pending_result_publish_event_id: None,
            pending_result_publish_event: None,
            result_publish_attempt_count: 0,
            result_publish_first_queued_epoch_seconds: None,
            result_publish_last_queued_epoch_seconds: None,
            payment_required_invoice_requested: false,
            payment_required_feedback_in_flight: false,
            payment_required_failed: false,
            next_payment_evidence_refresh_at: None,
            pending_bolt11_created_at_epoch_seconds: None,
            pending_bolt11: None,
            job: None,
            next_event_seq: 1,
        }
    }
}

impl ActiveJobState {
    pub fn inflight_job_count(&self) -> u32 {
        self.job
            .as_ref()
            .filter(|job| !job.stage.is_terminal())
            .map_or(0, |_| 1)
    }

    pub fn start_from_request(&mut self, request: &JobInboxRequest) {
        let job_id = format!("job-{}", request.request_id);
        self.job = Some(ActiveJobRecord {
            job_id,
            request_id: request.request_id.clone(),
            requester: request.requester.clone(),
            demand_source: request.demand_source,
            request_kind: request.request_kind,
            capability: request.capability.clone(),
            execution_input: request.execution_input.clone(),
            execution_prompt: request.execution_prompt.clone(),
            execution_params: request.execution_params.clone(),
            requested_model: request.requested_model.clone(),
            execution_provenance: None,
            skill_scope_id: request.skill_scope_id.clone(),
            skl_manifest_a: request.skl_manifest_a.clone(),
            skl_manifest_event_id: request.skl_manifest_event_id.clone(),
            sa_tick_request_event_id: request.sa_tick_request_event_id.clone(),
            sa_tick_result_event_id: request.sa_tick_result_event_id.clone(),
            sa_trajectory_session_id: Some(format!("traj:{}", request.request_id)),
            ac_envelope_event_id: request.ac_envelope_event_id.clone(),
            ac_settlement_event_id: None,
            ac_default_event_id: None,
            compute_product_id: None,
            capacity_lot_id: None,
            capacity_instrument_id: None,
            delivery_proof_id: None,
            delivery_metering_rule_id: None,
            delivery_proof_status_label: None,
            delivery_metered_quantity: None,
            delivery_accepted_quantity: None,
            delivery_variance_reason_label: None,
            delivery_rejection_reason_label: None,
            quoted_price_sats: request.price_sats,
            ttl_seconds: request.ttl_seconds,
            stage: JobLifecycleStage::Accepted,
            invoice_id: None,
            payment_id: None,
            failure_reason: None,
            events: Vec::new(),
        });
        self.next_event_seq = 1;
        self.runtime_supports_abort = false;
        self.execution_thread_id = None;
        self.execution_turn_id = None;
        self.execution_output = None;
        self.execution_turn_completed = false;
        self.execution_backend_request_id = None;
        self.execution_thread_start_command_seq = None;
        self.execution_turn_start_command_seq = None;
        self.execution_turn_interrupt_command_seq = None;
        self.execution_deadline_epoch_seconds = None;
        self.result_publish_in_flight = false;
        self.pending_result_publish_event_id = None;
        self.pending_result_publish_event = None;
        self.result_publish_attempt_count = 0;
        self.result_publish_first_queued_epoch_seconds = None;
        self.result_publish_last_queued_epoch_seconds = None;
        self.payment_required_invoice_requested = false;
        self.payment_required_feedback_in_flight = false;
        self.payment_required_failed = false;
        self.next_payment_evidence_refresh_at = None;
        self.pending_bolt11_created_at_epoch_seconds = None;
        self.pending_bolt11 = None;
        self.append_event("received request from inbox");
        self.append_event("accepted request and queued runtime execution");
        self.load_state = PaneLoadState::Ready;
        self.last_error = None;
        self.last_action = Some(format!("Selected {} as active job", request.request_id));
        self.scroll_offset_px = 0.0;
    }

    pub fn append_event(&mut self, message: impl Into<String>) {
        let Some(job) = self.job.as_mut() else {
            return;
        };
        job.events.push(ActiveJobEvent {
            seq: self.next_event_seq,
            message: message.into(),
        });
        self.next_event_seq = self.next_event_seq.saturating_add(1);
    }

    pub fn scroll_by(&mut self, dy: f32) {
        self.scroll_offset_px = (self.scroll_offset_px + dy).max(0.0);
    }

    pub fn advance_stage(&mut self) -> Result<JobLifecycleStage, String> {
        let Some(job) = self.job.as_mut() else {
            self.last_error = Some("No active job selected".to_string());
            self.load_state = PaneLoadState::Error;
            return Err("No active job selected".to_string());
        };

        match job.stage {
            JobLifecycleStage::Accepted => {
                if job.sa_tick_request_event_id.is_none() {
                    self.last_error =
                        Some("Cannot mark running without authoritative running event".to_string());
                    self.load_state = PaneLoadState::Error;
                    return Err(
                        "missing authoritative running event (sa_tick_request_event_id)"
                            .to_string(),
                    );
                }
            }
            JobLifecycleStage::Running => {
                if job.sa_tick_result_event_id.is_none() {
                    self.last_error = Some(
                        "Cannot mark delivered without authoritative delivery event".to_string(),
                    );
                    self.load_state = PaneLoadState::Error;
                    return Err(
                        "missing authoritative delivered event (sa_tick_result_event_id)"
                            .to_string(),
                    );
                }
            }
            JobLifecycleStage::Delivered => {
                if !is_authoritative_payment_pointer(job.payment_id.as_deref()) {
                    self.last_error = Some(
                        "Cannot mark paid without wallet-authoritative payment pointer".to_string(),
                    );
                    self.load_state = PaneLoadState::Error;
                    return Err(
                        "missing authoritative payment pointer for paid transition".to_string()
                    );
                }
            }
            JobLifecycleStage::Received | JobLifecycleStage::Paid | JobLifecycleStage::Failed => {}
        }

        let next_stage = match job.stage {
            JobLifecycleStage::Received => JobLifecycleStage::Accepted,
            JobLifecycleStage::Accepted => JobLifecycleStage::Running,
            JobLifecycleStage::Running => JobLifecycleStage::Delivered,
            JobLifecycleStage::Delivered => JobLifecycleStage::Paid,
            JobLifecycleStage::Paid | JobLifecycleStage::Failed => {
                self.last_error = Some("Active job already terminal".to_string());
                self.load_state = PaneLoadState::Error;
                return Err("Active job already terminal".to_string());
            }
        };

        job.stage = next_stage;
        let authority_ref = match next_stage {
            JobLifecycleStage::Accepted => Some(job.request_id.clone()),
            JobLifecycleStage::Running => job.sa_tick_request_event_id.clone(),
            JobLifecycleStage::Delivered => job.sa_tick_result_event_id.clone(),
            JobLifecycleStage::Paid => job
                .payment_id
                .clone()
                .or_else(|| job.ac_settlement_event_id.clone()),
            JobLifecycleStage::Received | JobLifecycleStage::Failed => None,
        };
        self.append_event(format!(
            "stage advanced to {} (authority={})",
            next_stage.label(),
            authority_ref.unwrap_or_else(|| "none".to_string())
        ));
        self.last_error = None;
        self.load_state = PaneLoadState::Ready;
        self.last_action = Some(format!("Advanced active job to {}", next_stage.label()));
        Ok(next_stage)
    }

    pub fn abort_job(&mut self, reason: &str) -> Result<(), String> {
        if !self.runtime_supports_abort {
            self.last_error =
                Some("Abort unavailable: runtime lane does not support cancel".to_string());
            self.load_state = PaneLoadState::Error;
            return Err("Abort unavailable".to_string());
        }
        self.mark_failed(reason, "Aborted active job")
    }

    pub fn mark_failed(&mut self, reason: &str, action_label: &str) -> Result<(), String> {
        let Some(job) = self.job.as_mut() else {
            self.last_error = Some("No active job selected".to_string());
            self.load_state = PaneLoadState::Error;
            return Err("No active job selected".to_string());
        };

        let reason_text = reason.trim().to_string();
        job.stage = JobLifecycleStage::Failed;
        job.failure_reason = Some(reason_text.clone());
        self.append_event(format!("job failed: {reason_text}"));
        self.last_error = None;
        self.load_state = PaneLoadState::Ready;
        self.runtime_supports_abort = false;
        self.execution_turn_interrupt_command_seq = None;
        self.pending_result_publish_event_id = None;
        self.pending_result_publish_event = None;
        self.result_publish_attempt_count = 0;
        self.result_publish_first_queued_epoch_seconds = None;
        self.result_publish_last_queued_epoch_seconds = None;
        self.payment_required_invoice_requested = false;
        self.payment_required_feedback_in_flight = false;
        self.payment_required_failed = false;
        self.next_payment_evidence_refresh_at = None;
        self.pending_bolt11_created_at_epoch_seconds = None;
        self.pending_bolt11 = None;
        self.last_action = Some(action_label.to_string());
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobHistoryStatus {
    Succeeded,
    Failed,
}

impl JobHistoryStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobHistoryStatusFilter {
    All,
    Succeeded,
    Failed,
}

impl JobHistoryStatusFilter {
    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }

    pub const fn cycle(self) -> Self {
        match self {
            Self::All => Self::Succeeded,
            Self::Succeeded => Self::Failed,
            Self::Failed => Self::All,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobHistoryTimeRange {
    All,
    Last24h,
    Last7d,
}

impl JobHistoryTimeRange {
    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "all-time",
            Self::Last24h => "24h",
            Self::Last7d => "7d",
        }
    }

    pub const fn max_age_seconds(self) -> Option<u64> {
        match self {
            Self::All => None,
            Self::Last24h => Some(86_400),
            Self::Last7d => Some(604_800),
        }
    }

    pub const fn cycle(self) -> Self {
        match self {
            Self::All => Self::Last24h,
            Self::Last24h => Self::Last7d,
            Self::Last7d => Self::All,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JobHistoryReceiptRow {
    pub job_id: String,
    pub status: JobHistoryStatus,
    pub demand_source: JobDemandSource,
    pub completed_at_epoch_seconds: u64,
    pub skill_scope_id: Option<String>,
    pub skl_manifest_a: Option<String>,
    pub skl_manifest_event_id: Option<String>,
    pub sa_tick_result_event_id: Option<String>,
    pub sa_trajectory_session_id: Option<String>,
    pub ac_envelope_event_id: Option<String>,
    pub ac_settlement_event_id: Option<String>,
    pub ac_default_event_id: Option<String>,
    pub delivery_proof_id: Option<String>,
    pub delivery_metering_rule_id: Option<String>,
    pub delivery_proof_status_label: Option<String>,
    pub delivery_metered_quantity: Option<u64>,
    pub delivery_accepted_quantity: Option<u64>,
    pub delivery_variance_reason_label: Option<String>,
    pub delivery_rejection_reason_label: Option<String>,
    pub payout_sats: u64,
    pub result_hash: String,
    pub payment_pointer: String,
    pub failure_reason: Option<String>,
    pub execution_provenance: Option<LocalInferenceExecutionProvenance>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WalletReconciledPayoutRow {
    pub job_id: String,
    pub payout_sats: u64,
    pub payment_pointer: String,
    pub completed_at_epoch_seconds: u64,
    pub wallet_received_at_epoch_seconds: u64,
}

pub struct JobHistoryState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub rows: Vec<JobHistoryReceiptRow>,
    pub status_filter: JobHistoryStatusFilter,
    pub time_range: JobHistoryTimeRange,
    pub page: usize,
    pub page_size: usize,
    pub search_job_id: String,
    pub reference_epoch_seconds: u64,
}

impl Default for JobHistoryState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for receipt lane snapshot".to_string()),
            rows: Vec::new(),
            status_filter: JobHistoryStatusFilter::All,
            time_range: JobHistoryTimeRange::All,
            page: 0,
            page_size: 6,
            search_job_id: String::new(),
            reference_epoch_seconds: 1_761_920_000,
        }
    }
}

impl JobHistoryState {
    pub fn set_search_job_id(&mut self, value: String) {
        self.search_job_id = value;
        self.page = 0;
    }

    pub fn cycle_status_filter(&mut self) {
        self.status_filter = self.status_filter.cycle();
        self.page = 0;
        self.last_error = None;
        self.last_action = Some(format!("Status filter -> {}", self.status_filter.label()));
    }

    pub fn cycle_time_range(&mut self) {
        self.time_range = self.time_range.cycle();
        self.page = 0;
        self.last_error = None;
        self.last_action = Some(format!("Time range -> {}", self.time_range.label()));
    }

    pub fn previous_page(&mut self) {
        if self.page > 0 {
            self.page -= 1;
        }
    }

    pub fn next_page(&mut self) {
        let pages = self.total_pages();
        if self.page + 1 < pages {
            self.page += 1;
        }
    }

    pub fn total_pages(&self) -> usize {
        let filtered = self.filtered_rows();
        ((filtered.len() + self.page_size.saturating_sub(1)) / self.page_size.max(1)).max(1)
    }

    pub fn paged_rows(&self) -> Vec<&JobHistoryReceiptRow> {
        let filtered = self.filtered_rows();
        let page = self.page.min(self.total_pages().saturating_sub(1));
        let start = page.saturating_mul(self.page_size.max(1));
        let end = (start + self.page_size.max(1)).min(filtered.len());
        filtered[start..end].to_vec()
    }

    pub fn upsert_row(&mut self, row: JobHistoryReceiptRow) {
        if let Some(existing) = self
            .rows
            .iter_mut()
            .find(|existing| existing.job_id == row.job_id)
        {
            *existing = row;
        } else {
            self.rows.push(row);
        }
        self.rows.sort_by(|lhs, rhs| {
            rhs.completed_at_epoch_seconds
                .cmp(&lhs.completed_at_epoch_seconds)
                .then_with(|| lhs.job_id.cmp(&rhs.job_id))
        });
    }

    pub fn record_from_active_job(&mut self, job: &ActiveJobRecord, status: JobHistoryStatus) {
        let completed = self
            .reference_epoch_seconds
            .saturating_add(self.rows.len() as u64 * 17);
        let payment_pointer = job
            .payment_id
            .clone()
            .or_else(|| job.invoice_id.clone())
            .unwrap_or_else(|| format!("pending:{}", job.request_id));
        let authoritative_settlement =
            is_authoritative_payment_pointer(Some(payment_pointer.as_str()));
        let converted_to_failed =
            status == JobHistoryStatus::Succeeded && !authoritative_settlement;
        let settled_success = status == JobHistoryStatus::Succeeded && authoritative_settlement;
        let status = if settled_success {
            JobHistoryStatus::Succeeded
        } else if converted_to_failed {
            JobHistoryStatus::Failed
        } else {
            status
        };
        let failure_reason = if settled_success {
            None
        } else if converted_to_failed && job.failure_reason.is_none() {
            Some("payment settlement not wallet-confirmed".to_string())
        } else {
            job.failure_reason.clone()
        };
        self.upsert_row(JobHistoryReceiptRow {
            job_id: job.job_id.clone(),
            status,
            demand_source: job.demand_source,
            completed_at_epoch_seconds: completed,
            skill_scope_id: job.skill_scope_id.clone(),
            skl_manifest_a: job.skl_manifest_a.clone(),
            skl_manifest_event_id: job.skl_manifest_event_id.clone(),
            sa_tick_result_event_id: job.sa_tick_result_event_id.clone(),
            sa_trajectory_session_id: job.sa_trajectory_session_id.clone(),
            ac_envelope_event_id: job.ac_envelope_event_id.clone(),
            ac_settlement_event_id: job.ac_settlement_event_id.clone(),
            ac_default_event_id: job.ac_default_event_id.clone(),
            delivery_proof_id: job.delivery_proof_id.clone(),
            delivery_metering_rule_id: job.delivery_metering_rule_id.clone(),
            delivery_proof_status_label: job.delivery_proof_status_label.clone(),
            delivery_metered_quantity: job.delivery_metered_quantity,
            delivery_accepted_quantity: job.delivery_accepted_quantity,
            delivery_variance_reason_label: job.delivery_variance_reason_label.clone(),
            delivery_rejection_reason_label: job.delivery_rejection_reason_label.clone(),
            payout_sats: if settled_success {
                job.quoted_price_sats
            } else {
                0
            },
            result_hash: format!("sha256:{}-{}", job.request_id, job.stage.label()),
            payment_pointer,
            failure_reason,
            execution_provenance: job.execution_provenance.clone(),
        });
        self.page = 0;
        self.last_error = None;
        self.load_state = PaneLoadState::Ready;
        self.last_action = Some(format!("Recorded history receipt for {}", job.job_id));
    }

    pub fn wallet_reconciled_payout_rows(
        &self,
        spark_wallet: &SparkPaneState,
    ) -> Vec<WalletReconciledPayoutRow> {
        let settled_receive_by_id = spark_wallet
            .recent_payments
            .iter()
            .filter(|payment| {
                payment.direction.eq_ignore_ascii_case("receive")
                    && is_settled_wallet_payment_status(payment.status.as_str())
            })
            .map(|payment| (payment.id.as_str(), payment))
            .collect::<HashMap<_, _>>();

        let mut rows = Vec::<WalletReconciledPayoutRow>::new();
        let mut seen_payment_pointers = HashSet::<String>::new();
        for row in &self.rows {
            let payment_pointer = row.payment_pointer.trim();
            if !is_authoritative_payment_pointer(Some(payment_pointer)) {
                continue;
            }
            let Some(payment) = settled_receive_by_id.get(payment_pointer) else {
                continue;
            };
            if !seen_payment_pointers.insert(payment_pointer.to_string()) {
                continue;
            }
            rows.push(WalletReconciledPayoutRow {
                job_id: row.job_id.clone(),
                payout_sats: payment.amount_sats,
                payment_pointer: payment_pointer.to_string(),
                completed_at_epoch_seconds: row.completed_at_epoch_seconds,
                wallet_received_at_epoch_seconds: payment.timestamp,
            });
        }
        rows.sort_by(|left, right| {
            right
                .wallet_received_at_epoch_seconds
                .cmp(&left.wallet_received_at_epoch_seconds)
                .then_with(|| {
                    right
                        .completed_at_epoch_seconds
                        .cmp(&left.completed_at_epoch_seconds)
                })
                .then_with(|| left.job_id.cmp(&right.job_id))
        });
        rows
    }

    fn filtered_rows(&self) -> Vec<&JobHistoryReceiptRow> {
        let search = self.search_job_id.trim().to_lowercase();
        self.rows
            .iter()
            .filter(|row| match self.status_filter {
                JobHistoryStatusFilter::All => true,
                JobHistoryStatusFilter::Succeeded => row.status == JobHistoryStatus::Succeeded,
                JobHistoryStatusFilter::Failed => row.status == JobHistoryStatus::Failed,
            })
            .filter(|row| {
                if let Some(max_age) = self.time_range.max_age_seconds() {
                    let age = self
                        .reference_epoch_seconds
                        .saturating_sub(row.completed_at_epoch_seconds);
                    age <= max_age
                } else {
                    true
                }
            })
            .filter(|row| {
                if search.is_empty() {
                    true
                } else {
                    row.job_id.to_lowercase().contains(&search)
                }
            })
            .collect()
    }
}

fn is_authoritative_payment_pointer(pointer: Option<&str>) -> bool {
    let Some(pointer) = pointer else {
        return false;
    };
    let pointer = pointer.trim();
    !pointer.is_empty()
        && !pointer.starts_with("pending:")
        && !pointer.starts_with("pay:")
        && !pointer.starts_with("inv-")
        && !pointer.starts_with("pay-req-")
}

fn is_settled_wallet_payment_status(status: &str) -> bool {
    matches!(
        status.to_ascii_lowercase().as_str(),
        "succeeded" | "success" | "settled" | "completed" | "confirmed"
    )
}

const EARN_JOB_LIFECYCLE_PROJECTION_SCHEMA_VERSION: u16 = 1;
const EARN_JOB_LIFECYCLE_PROJECTION_STREAM_ID: &str = "stream.earn_job_lifecycle_projection.v1";
const EARN_JOB_LIFECYCLE_PROJECTION_AUTHORITY: &str = "non-authoritative";
const EARN_JOB_LIFECYCLE_PROJECTION_ROW_LIMIT: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EarnJobLifecycleProjectionRow {
    pub stream_seq: u64,
    pub event_id: String,
    pub job_id: String,
    pub request_id: String,
    pub stage: JobLifecycleStage,
    pub source_tag: String,
    pub occurred_at_epoch_seconds: u64,
    pub quoted_price_sats: u64,
    pub payment_pointer: Option<String>,
    pub settlement_authority: String,
    pub settlement_authoritative: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct EarnJobLifecycleProjectionDocumentV1 {
    schema_version: u16,
    stream_id: String,
    authority: String,
    rows: Vec<EarnJobLifecycleProjectionRow>,
}

pub struct EarnJobLifecycleProjectionState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub stream_id: String,
    pub authority: String,
    pub rows: Vec<EarnJobLifecycleProjectionRow>,
    projection_file_path: PathBuf,
}

impl Default for EarnJobLifecycleProjectionState {
    fn default() -> Self {
        let projection_file_path = earn_job_lifecycle_projection_file_path();
        Self::from_projection_file_path(projection_file_path)
    }
}

impl EarnJobLifecycleProjectionState {
    fn from_projection_file_path(projection_file_path: PathBuf) -> Self {
        let (rows, load_state, last_error, last_action) =
            match load_earn_job_lifecycle_projection_rows(projection_file_path.as_path()) {
                Ok(rows) => (
                    rows,
                    PaneLoadState::Ready,
                    None,
                    Some("Loaded earn lifecycle projection stream".to_string()),
                ),
                Err(error) => (
                    Vec::new(),
                    PaneLoadState::Error,
                    Some(error),
                    Some("Earn lifecycle projection stream load failed".to_string()),
                ),
            };
        Self {
            load_state,
            last_error,
            last_action: last_action.map(|action| format!("{action} ({} rows)", rows.len())),
            stream_id: EARN_JOB_LIFECYCLE_PROJECTION_STREAM_ID.to_string(),
            authority: EARN_JOB_LIFECYCLE_PROJECTION_AUTHORITY.to_string(),
            rows,
            projection_file_path,
        }
    }

    #[cfg(test)]
    fn from_projection_path_for_tests(projection_file_path: PathBuf) -> Self {
        Self::from_projection_file_path(projection_file_path)
    }

    pub fn record_ingress_request(
        &mut self,
        request: &JobInboxNetworkRequest,
        occurred_at_epoch_seconds: u64,
        source_tag: &str,
    ) {
        let job_id = format!("job-{}", request.request_id);
        let authority_key = request.request_id.as_str();
        let (settlement_authority, settlement_authoritative) = settle_authority_for_pointer(None);
        let row = EarnJobLifecycleProjectionRow {
            stream_seq: 0,
            event_id: earn_job_lifecycle_event_id(
                job_id.as_str(),
                JobLifecycleStage::Received,
                authority_key,
            ),
            job_id,
            request_id: request.request_id.clone(),
            stage: JobLifecycleStage::Received,
            source_tag: source_tag.to_string(),
            occurred_at_epoch_seconds,
            quoted_price_sats: request.price_sats,
            payment_pointer: None,
            settlement_authority,
            settlement_authoritative,
        };
        self.upsert_row(row);
    }

    pub fn record_active_job_stage(
        &mut self,
        job: &ActiveJobRecord,
        stage: JobLifecycleStage,
        occurred_at_epoch_seconds: u64,
        source_tag: &str,
    ) {
        let payment_pointer = job.payment_id.clone().or_else(|| job.invoice_id.clone());
        let (settlement_authority, settlement_authoritative) =
            settle_authority_for_pointer(payment_pointer.as_deref());
        let effective_stage = if stage == JobLifecycleStage::Paid && !settlement_authoritative {
            JobLifecycleStage::Delivered
        } else {
            stage
        };
        let authority_key = match effective_stage {
            JobLifecycleStage::Received | JobLifecycleStage::Accepted => job.request_id.as_str(),
            JobLifecycleStage::Running => job
                .sa_tick_request_event_id
                .as_deref()
                .unwrap_or(job.request_id.as_str()),
            JobLifecycleStage::Delivered => job
                .sa_tick_result_event_id
                .as_deref()
                .unwrap_or(job.request_id.as_str()),
            JobLifecycleStage::Paid => payment_pointer
                .as_deref()
                .unwrap_or(job.request_id.as_str()),
            JobLifecycleStage::Failed => job
                .ac_default_event_id
                .as_deref()
                .or(job.failure_reason.as_deref())
                .unwrap_or(job.request_id.as_str()),
        };
        let row = EarnJobLifecycleProjectionRow {
            stream_seq: 0,
            event_id: earn_job_lifecycle_event_id(
                job.job_id.as_str(),
                effective_stage,
                authority_key,
            ),
            job_id: job.job_id.clone(),
            request_id: job.request_id.clone(),
            stage: effective_stage,
            source_tag: source_tag.to_string(),
            occurred_at_epoch_seconds,
            quoted_price_sats: job.quoted_price_sats,
            payment_pointer,
            settlement_authority,
            settlement_authoritative,
        };
        self.upsert_row(row);
    }

    pub fn record_history_receipt(
        &mut self,
        row: &JobHistoryReceiptRow,
        occurred_at_epoch_seconds: u64,
        source_tag: &str,
    ) {
        let stage = if row.status == JobHistoryStatus::Succeeded {
            JobLifecycleStage::Paid
        } else {
            JobLifecycleStage::Failed
        };
        let payment_pointer = Some(row.payment_pointer.clone());
        let (settlement_authority, settlement_authoritative) =
            settle_authority_for_pointer(payment_pointer.as_deref());
        let authority_key = if stage == JobLifecycleStage::Paid {
            row.payment_pointer.as_str()
        } else {
            row.result_hash.as_str()
        };
        let request_id = infer_request_id_from_job_id(row.job_id.as_str());
        let projection_row = EarnJobLifecycleProjectionRow {
            stream_seq: 0,
            event_id: earn_job_lifecycle_event_id(row.job_id.as_str(), stage, authority_key),
            job_id: row.job_id.clone(),
            request_id,
            stage,
            source_tag: source_tag.to_string(),
            occurred_at_epoch_seconds,
            quoted_price_sats: row.payout_sats,
            payment_pointer,
            settlement_authority,
            settlement_authoritative,
        };
        self.upsert_row(projection_row);
    }

    fn upsert_row(&mut self, mut row: EarnJobLifecycleProjectionRow) {
        if let Some(existing) = self
            .rows
            .iter_mut()
            .find(|existing| existing.event_id == row.event_id)
        {
            row.stream_seq = existing.stream_seq;
            *existing = row;
        } else {
            row.stream_seq = self
                .rows
                .iter()
                .map(|existing| existing.stream_seq)
                .max()
                .unwrap_or(0)
                .saturating_add(1);
            self.rows.push(row);
        }
        self.rows = normalize_earn_job_lifecycle_projection_rows(std::mem::take(&mut self.rows));
        if let Some(latest) = self.rows.first() {
            self.last_action = Some(format!(
                "Projected {} stage {} ({})",
                latest.job_id,
                latest.stage.label(),
                latest.source_tag
            ));
        }
        if let Err(error) = persist_earn_job_lifecycle_projection_rows(
            self.projection_file_path.as_path(),
            self.rows.as_slice(),
        ) {
            self.last_error = Some(error);
            self.load_state = PaneLoadState::Error;
        } else {
            self.last_error = None;
            self.load_state = PaneLoadState::Ready;
        }
    }
}

fn earn_job_lifecycle_projection_file_path() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".openagents")
        .join("autopilot-earn-job-lifecycle-projection-v1.json")
}

fn earn_job_lifecycle_event_id(
    job_id: &str,
    stage: JobLifecycleStage,
    authority_key: &str,
) -> String {
    format!(
        "earn.lifecycle:{}:{}:{}",
        job_id.trim(),
        stage.label(),
        normalize_projection_key(authority_key)
    )
}

fn normalize_projection_key(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|ch| if ch.is_whitespace() { '_' } else { ch })
        .collect()
}

fn infer_request_id_from_job_id(job_id: &str) -> String {
    job_id
        .strip_prefix("job-")
        .map(ToString::to_string)
        .unwrap_or_else(|| job_id.to_string())
}

fn settle_authority_for_pointer(payment_pointer: Option<&str>) -> (String, bool) {
    if is_authoritative_payment_pointer(payment_pointer) {
        ("wallet.reconciliation".to_string(), true)
    } else {
        ("projection.non_authoritative".to_string(), false)
    }
}

fn normalize_earn_job_lifecycle_projection_rows(
    mut rows: Vec<EarnJobLifecycleProjectionRow>,
) -> Vec<EarnJobLifecycleProjectionRow> {
    rows.sort_by(|lhs, rhs| {
        rhs.stream_seq
            .cmp(&lhs.stream_seq)
            .then_with(|| lhs.event_id.cmp(&rhs.event_id))
    });
    let mut seen_event_ids = HashSet::new();
    rows.retain(|row| seen_event_ids.insert(row.event_id.clone()));
    rows.truncate(EARN_JOB_LIFECYCLE_PROJECTION_ROW_LIMIT);
    rows
}

fn persist_earn_job_lifecycle_projection_rows(
    path: &Path,
    rows: &[EarnJobLifecycleProjectionRow],
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create earn lifecycle projection dir: {error}"))?;
    }
    let document = EarnJobLifecycleProjectionDocumentV1 {
        schema_version: EARN_JOB_LIFECYCLE_PROJECTION_SCHEMA_VERSION,
        stream_id: EARN_JOB_LIFECYCLE_PROJECTION_STREAM_ID.to_string(),
        authority: EARN_JOB_LIFECYCLE_PROJECTION_AUTHORITY.to_string(),
        rows: normalize_earn_job_lifecycle_projection_rows(rows.to_vec()),
    };
    let payload = serde_json::to_string_pretty(&document)
        .map_err(|error| format!("Failed to encode earn lifecycle projection rows: {error}"))?;
    let temp_path = path.with_extension("tmp");
    std::fs::write(&temp_path, payload)
        .map_err(|error| format!("Failed to write earn lifecycle projection temp file: {error}"))?;
    std::fs::rename(&temp_path, path)
        .map_err(|error| format!("Failed to persist earn lifecycle projection rows: {error}"))?;
    Ok(())
}

fn load_earn_job_lifecycle_projection_rows(
    path: &Path,
) -> Result<Vec<EarnJobLifecycleProjectionRow>, String> {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(format!(
                "Failed to read earn lifecycle projection rows: {error}"
            ));
        }
    };
    let document = serde_json::from_str::<EarnJobLifecycleProjectionDocumentV1>(&raw)
        .map_err(|error| format!("Failed to parse earn lifecycle projection rows: {error}"))?;
    if document.schema_version != EARN_JOB_LIFECYCLE_PROJECTION_SCHEMA_VERSION {
        return Err(format!(
            "Unsupported earn lifecycle projection schema version: {}",
            document.schema_version
        ));
    }
    if document.stream_id != EARN_JOB_LIFECYCLE_PROJECTION_STREAM_ID {
        return Err(format!(
            "Unsupported earn lifecycle projection stream id: {}",
            document.stream_id
        ));
    }
    if document.authority != EARN_JOB_LIFECYCLE_PROJECTION_AUTHORITY {
        return Err(format!(
            "Unsupported earn lifecycle projection authority marker: {}",
            document.authority
        ));
    }
    Ok(normalize_earn_job_lifecycle_projection_rows(document.rows))
}

pub struct EarningsScoreboardState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub sats_today: u64,
    pub sats_this_month: u64,
    pub lifetime_sats: u64,
    pub jobs_today: u64,
    pub last_job_result: String,
    pub online_uptime_seconds: u64,
    pub first_job_latency_seconds: Option<u64>,
    pub completion_ratio_bps: Option<u16>,
    pub payout_success_ratio_bps: Option<u16>,
    pub avg_wallet_confirmation_latency_seconds: Option<u64>,
    pub stale_after: Duration,
    pub last_refreshed_at: Option<Instant>,
    tracked_online_since: Option<Instant>,
    first_completed_since_online: Option<Instant>,
}

impl Default for EarningsScoreboardState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for wallet + job receipts".to_string()),
            sats_today: 0,
            sats_this_month: 0,
            lifetime_sats: 0,
            jobs_today: 0,
            last_job_result: "none".to_string(),
            online_uptime_seconds: 0,
            first_job_latency_seconds: None,
            completion_ratio_bps: None,
            payout_success_ratio_bps: None,
            avg_wallet_confirmation_latency_seconds: None,
            stale_after: Duration::from_secs(12),
            last_refreshed_at: None,
            tracked_online_since: None,
            first_completed_since_online: None,
        }
    }
}

impl EarningsScoreboardState {
    pub fn refresh_from_sources(
        &mut self,
        now: Instant,
        provider_runtime: &ProviderRuntimeState,
        job_history: &JobHistoryState,
        spark_wallet: &SparkPaneState,
    ) {
        self.last_refreshed_at = Some(now);
        self.online_uptime_seconds = provider_runtime.uptime_seconds(now);
        self.last_error = None;

        if self.tracked_online_since != provider_runtime.online_since {
            self.tracked_online_since = provider_runtime.online_since;
            self.first_completed_since_online = None;
        }
        if self.tracked_online_since.is_none() {
            self.first_job_latency_seconds = None;
            self.first_completed_since_online = None;
        } else {
            if self.first_completed_since_online.is_none()
                && let Some(last_completed) = provider_runtime.last_completed_job_at
                && self
                    .tracked_online_since
                    .is_some_and(|online_since| last_completed >= online_since)
            {
                self.first_completed_since_online = Some(last_completed);
            }
            self.first_job_latency_seconds = self.tracked_online_since.and_then(|online_since| {
                let end = self.first_completed_since_online.unwrap_or(now);
                end.checked_duration_since(online_since)
                    .map(|duration| duration.as_secs())
            });
        }

        if let Some(error) = spark_wallet.last_error.as_deref() {
            self.load_state = PaneLoadState::Error;
            self.last_error = Some(format!("Wallet source error: {error}"));
            self.last_action = Some("Scoreboard degraded due to wallet error".to_string());
        } else if spark_wallet.balance.is_none() {
            self.load_state = PaneLoadState::Loading;
            self.last_action = Some("Scoreboard waiting for first wallet refresh".to_string());
        } else {
            self.load_state = PaneLoadState::Ready;
            self.last_action =
                Some("Scoreboard refreshed from reconciled wallet evidence".to_string());
        }

        let reconciled_payout_rows = job_history.wallet_reconciled_payout_rows(spark_wallet);
        let threshold = job_history.reference_epoch_seconds.saturating_sub(86_400);
        self.jobs_today = reconciled_payout_rows
            .iter()
            .filter(|row| row.wallet_received_at_epoch_seconds >= threshold)
            .count() as u64;
        self.sats_today = reconciled_payout_rows
            .iter()
            .filter(|row| row.wallet_received_at_epoch_seconds >= threshold)
            .map(|row| row.payout_sats)
            .sum();
        self.sats_this_month = reconciled_payout_rows
            .iter()
            .filter(|row| {
                wallet_receipt_is_in_reference_month(
                    row.wallet_received_at_epoch_seconds,
                    job_history.reference_epoch_seconds,
                )
            })
            .map(|row| row.payout_sats)
            .sum();
        self.lifetime_sats = reconciled_payout_rows
            .iter()
            .map(|row| row.payout_sats)
            .sum();
        let total_terminal_jobs = job_history.rows.len() as u64;
        let completed_jobs = job_history
            .rows
            .iter()
            .filter(|row| row.status == JobHistoryStatus::Succeeded)
            .count() as u64;
        self.completion_ratio_bps = ratio_bps(completed_jobs, total_terminal_jobs);
        self.payout_success_ratio_bps =
            ratio_bps(reconciled_payout_rows.len() as u64, completed_jobs);
        self.avg_wallet_confirmation_latency_seconds = if reconciled_payout_rows.is_empty() {
            None
        } else {
            Some(
                reconciled_payout_rows
                    .iter()
                    .map(|row| {
                        row.wallet_received_at_epoch_seconds
                            .saturating_sub(row.completed_at_epoch_seconds)
                    })
                    .sum::<u64>()
                    / reconciled_payout_rows.len() as u64,
            )
        };

        self.last_job_result = job_history
            .rows
            .first()
            .map(|row| {
                if let Some(reason) = row.failure_reason.as_deref() {
                    format!("{} ({reason})", row.status.label())
                } else {
                    row.status.label().to_string()
                }
            })
            .unwrap_or_else(|| "none".to_string());
    }

    pub fn is_stale(&self, now: Instant) -> bool {
        self.last_refreshed_at
            .is_none_or(|refresh| now.duration_since(refresh) > self.stale_after)
    }
}

fn ratio_bps(numerator: u64, denominator: u64) -> Option<u16> {
    if denominator == 0 {
        return None;
    }
    let ratio = ((numerator as u128) * 10_000u128 / (denominator as u128)).min(10_000u128);
    Some(ratio as u16)
}

fn wallet_receipt_is_in_reference_month(
    wallet_received_at_epoch_seconds: u64,
    reference_epoch_seconds: u64,
) -> bool {
    let Some(reference) = Utc
        .timestamp_opt(reference_epoch_seconds as i64, 0)
        .single()
    else {
        return false;
    };
    let Some(receipt) = Utc
        .timestamp_opt(wallet_received_at_epoch_seconds as i64, 0)
        .single()
    else {
        return false;
    };
    receipt.year() == reference.year() && receipt.month() == reference.month()
}

pub struct NetworkAggregateCountersState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub source_tag: String,
    pub providers_online_source_tag: String,
    pub providers_online_source_detail: Option<String>,
    pub providers_online: u64,
    pub jobs_completed: u64,
    pub sats_paid: u64,
    pub global_earnings_today_sats: u64,
    pub stale_after: Duration,
    pub last_refreshed_at: Option<Instant>,
}

impl Default for NetworkAggregateCountersState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for aggregate counters service refresh".to_string()),
            source_tag: "aggregate.pending".to_string(),
            providers_online_source_tag: "spacetime.presence.pending".to_string(),
            providers_online_source_detail: None,
            providers_online: 0,
            jobs_completed: 0,
            sats_paid: 0,
            global_earnings_today_sats: 0,
            stale_after: Duration::from_secs(12),
            last_refreshed_at: None,
        }
    }
}

impl NetworkAggregateCountersState {
    pub fn refresh_from_sources(
        &mut self,
        now: Instant,
        spacetime_presence: &crate::spacetime_presence::SpacetimePresenceSnapshot,
        job_history: &JobHistoryState,
        spark_wallet: &SparkPaneState,
    ) {
        self.last_refreshed_at = Some(now);
        self.providers_online = spacetime_presence.providers_online;
        self.providers_online_source_tag = format!(
            "spacetime.presence.{}",
            spacetime_presence.counter_cardinality
        );
        self.providers_online_source_detail = None;

        let presence_issue = if let Some(error) = spacetime_presence.last_error.as_deref() {
            self.providers_online_source_tag = "spacetime.presence.degraded".to_string();
            self.providers_online_source_detail = Some(error.to_string());
            Some(format!("Spacetime presence error: {error}"))
        } else if spacetime_presence.node_status == "unregistered" {
            self.providers_online_source_tag = "spacetime.presence.unavailable".to_string();
            self.providers_online_source_detail =
                Some("Provider presence not registered".to_string());
            Some("Spacetime presence unavailable".to_string())
        } else if spacetime_presence.node_offline_reason.as_deref() == Some("ttl_expired") {
            self.providers_online_source_tag = "spacetime.presence.stale".to_string();
            self.providers_online_source_detail = Some("Provider presence TTL expired".to_string());
            None
        } else {
            None
        };

        let reconciled_payout_rows = job_history.wallet_reconciled_payout_rows(spark_wallet);
        let threshold = job_history.reference_epoch_seconds.saturating_sub(86_400);
        self.jobs_completed = reconciled_payout_rows.len() as u64;
        self.sats_paid = reconciled_payout_rows
            .iter()
            .map(|row| row.payout_sats)
            .sum();
        self.global_earnings_today_sats = reconciled_payout_rows
            .iter()
            .filter(|row| row.wallet_received_at_epoch_seconds >= threshold)
            .map(|row| row.payout_sats)
            .sum();

        self.last_error = None;
        if let Some(error) = spark_wallet.last_error.as_deref() {
            self.load_state = PaneLoadState::Error;
            self.last_error = Some(format!("Wallet source error: {error}"));
            self.last_action = Some("Aggregate counters degraded due to wallet error".to_string());
            self.source_tag = "aggregate.degraded.wallet".to_string();
        } else if spark_wallet.balance.is_none() {
            self.load_state = PaneLoadState::Loading;
            self.last_action = Some("Aggregate counters waiting for wallet refresh".to_string());
            self.source_tag = "aggregate.pending.wallet".to_string();
        } else {
            self.load_state = PaneLoadState::Ready;
            self.last_action = Some(
                "Aggregate counters refreshed from wallet-reconciled payouts and Spacetime presence"
                    .to_string(),
            );
            self.source_tag = "aggregate.wallet-reconciled.spacetime-presence".to_string();
        }

        if let Some(issue) = presence_issue {
            if self.load_state == PaneLoadState::Ready {
                self.load_state = PaneLoadState::Error;
                self.source_tag = "aggregate.degraded.spacetime-presence".to_string();
            }

            self.last_error = match self.last_error.take() {
                Some(existing) => Some(format!("{existing}; {issue}")),
                None => Some(issue),
            };
            self.last_action =
                Some("Aggregate counters degraded due to Spacetime presence source".to_string());
        } else if self.providers_online_source_tag == "spacetime.presence.stale" {
            self.source_tag = "aggregate.stale.spacetime-presence".to_string();
            self.last_action = Some(
                "Aggregate counters refreshed; providers_online source is stale (TTL expiry)"
                    .to_string(),
            );
        }
    }

    pub fn is_stale(&self, now: Instant) -> bool {
        self.last_refreshed_at
            .is_none_or(|refresh| now.duration_since(refresh) > self.stale_after)
    }
}

pub struct NostrSecretState {
    pub reveal_duration: Duration,
    pub revealed_until: Option<Instant>,
    pub copy_notice: Option<String>,
    pub copy_notice_until: Option<Instant>,
}

impl Default for NostrSecretState {
    fn default() -> Self {
        Self {
            reveal_duration: Duration::from_secs(12),
            revealed_until: None,
            copy_notice: None,
            copy_notice_until: None,
        }
    }
}

impl NostrSecretState {
    pub fn is_revealed(&self, now: Instant) -> bool {
        self.revealed_until.is_some_and(|until| until > now)
    }

    pub fn toggle_reveal(&mut self, now: Instant) {
        if self.is_revealed(now) {
            self.revealed_until = None;
        } else {
            self.revealed_until = Some(now + self.reveal_duration);
        }
    }

    pub fn set_copy_notice(&mut self, now: Instant, message: String) {
        self.copy_notice = Some(message);
        self.copy_notice_until = Some(now + Duration::from_secs(4));
    }

    pub fn expire(&mut self, now: Instant) -> bool {
        let mut changed = false;

        if self.revealed_until.is_some_and(|until| until <= now) {
            self.revealed_until = None;
            changed = true;
        }

        if self.copy_notice_until.is_some_and(|until| until <= now) {
            self.copy_notice = None;
            self.copy_notice_until = None;
            changed = true;
        }

        changed
    }
}

impl_pane_status_access!(
    ProjectOpsPaneState,
    CodexAccountPaneState,
    CodexModelsPaneState,
    CodexConfigPaneState,
    CodexMcpPaneState,
    CodexAppsPaneState,
    CodexLabsPaneState,
    CodexDiagnosticsPaneState,
    RelayConnectionsState,
    SyncHealthState,
    NetworkRequestsState,
    StarterJobsState,
    ReciprocalLoopState,
    ActivityFeedState,
    AlertsRecoveryState,
    SettingsState,
    CredentialsState,
    JobInboxState,
    ActiveJobState,
    JobHistoryState,
    AgentProfileStatePaneState,
    AgentScheduleTickPaneState,
    TrajectoryAuditPaneState,
    CastControlPaneState,
    SkillRegistryPaneState,
    SkillTrustRevocationPaneState,
    CadDemoPaneState,
    CreditDeskPaneState,
    CreditSettlementLedgerPaneState,
    StableSatsSimulationPaneState,
);

pub struct CastControlProcess {
    pub child: std::process::Child,
    pub operation: String,
    pub receipt_path: String,
    pub log_path: String,
}

pub struct RenderState {
    pub window: Arc<Window>,
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub renderer: Renderer,
    pub text_system: TextSystem,
    pub scale_factor: f32,
    pub desktop_shell_mode: crate::desktop_shell::DesktopShellMode,
    pub buy_mode_enabled: bool,
    pub hotbar: Hotbar,
    pub hotbar_bounds: Bounds,
    pub cursor_position: Point,
    pub event_context: EventContext,
    pub input_modifiers: Modifiers,
    pub panes: Vec<DesktopPane>,
    pub pane_size_memory: PaneSizeMemory,
    pub nostr_identity: Option<NostrIdentity>,
    pub nostr_identity_error: Option<String>,
    pub nostr_secret_state: NostrSecretState,
    pub spark_wallet: SparkPaneState,
    pub spark_worker: SparkWalletWorker,
    pub stable_sats_blink_worker: StableSatsBlinkWorker,
    pub spark_inputs: SparkPaneInputs,
    pub pay_invoice_inputs: PayInvoicePaneInputs,
    pub create_invoice_inputs: CreateInvoicePaneInputs,
    pub relay_connections_inputs: RelayConnectionsPaneInputs,
    pub network_requests_inputs: NetworkRequestsPaneInputs,
    pub local_inference_inputs: LocalInferencePaneInputs,
    pub apple_fm_workbench_inputs: AppleFmWorkbenchPaneInputs,
    pub settings_inputs: SettingsPaneInputs,
    pub credentials_inputs: CredentialsPaneInputs,
    pub job_history_inputs: JobHistoryPaneInputs,
    pub chat_inputs: ChatPaneInputs,
    pub calculator_inputs: CalculatorPaneInputs,
    pub mission_control: MissionControlPaneState,
    pub buy_mode_payments: BuyModePaymentsPaneState,
    pub autopilot_chat: AutopilotChatState,
    pub project_ops: ProjectOpsPaneState,
    pub chat_transcript_selection_drag: Option<ChatTranscriptSelectionDragState>,
    pub codex_account: CodexAccountPaneState,
    pub codex_models: CodexModelsPaneState,
    pub codex_config: CodexConfigPaneState,
    pub codex_mcp: CodexMcpPaneState,
    pub codex_apps: CodexAppsPaneState,
    pub codex_labs: CodexLabsPaneState,
    pub desktop_control: DesktopControlState,
    pub codex_remote: CodexRemoteState,
    pub codex_diagnostics: CodexDiagnosticsPaneState,
    pub codex_lane: CodexLaneSnapshot,
    pub codex_lane_config: crate::codex_lane::CodexLaneConfig,
    pub codex_lane_worker: CodexLaneWorker,
    pub codex_command_responses: Vec<CodexLaneCommandResponse>,
    pub codex_notifications: Vec<CodexLaneNotification>,
    pub next_codex_command_seq: u64,
    pub sa_lane: SaLaneSnapshot,
    pub skl_lane: SklLaneSnapshot,
    pub ac_lane: AcLaneSnapshot,
    pub sa_lane_worker: SaLaneWorker,
    pub skl_lane_worker: SklLaneWorker,
    pub ac_lane_worker: AcLaneWorker,
    pub provider_nip90_lane: ProviderNip90LaneSnapshot,
    pub provider_nip90_lane_worker: ProviderNip90LaneWorker,
    pub nip28_chat_lane_worker: crate::nip28_chat_lane::Nip28ChatLaneWorker,
    pub apple_fm_execution: AppleFmBridgeSnapshot,
    pub apple_fm_execution_worker: AppleFmBridgeWorker,
    pub ollama_execution: LocalInferenceExecutionSnapshot,
    pub local_inference_runtime: Box<dyn LocalInferenceRuntime>,
    pub runtime_command_responses: Vec<RuntimeCommandResponse>,
    pub next_runtime_command_seq: u64,
    pub provider_runtime: ProviderRuntimeState,
    pub local_inference: LocalInferencePaneState,
    pub apple_fm_workbench: AppleFmWorkbenchPaneState,
    pub provider_admin_runtime: Option<crate::provider_admin::DesktopProviderAdminRuntime>,
    pub provider_admin_listen_addr: Option<String>,
    pub provider_admin_last_error: Option<String>,
    pub provider_admin_last_sync_signature: Option<String>,
    pub provider_admin_last_sync_at: Option<Instant>,
    pub desktop_control_runtime: Option<crate::desktop_control::DesktopControlRuntime>,
    pub desktop_control_last_sync_signature: Option<String>,
    pub desktop_control_last_sync_at: Option<Instant>,
    pub codex_remote_runtime: Option<crate::codex_remote::DesktopCodexRemoteRuntime>,
    pub codex_remote_last_sync_signature: Option<String>,
    pub codex_remote_last_sync_at: Option<Instant>,
    pub earnings_scoreboard: EarningsScoreboardState,
    pub network_aggregate_counters: NetworkAggregateCountersState,
    pub relay_connections: RelayConnectionsState,
    pub sync_health: SyncHealthState,
    pub sync_bootstrap_note: Option<String>,
    pub sync_bootstrap_error: Option<String>,
    pub sync_bootstrap_stream_grants: Vec<String>,
    pub hosted_control_base_url: Option<String>,
    pub hosted_control_bearer_token: Option<String>,
    pub kernel_projection_worker: crate::kernel_control::KernelProjectionWorker,
    pub sync_apply_engine: crate::sync_apply::SyncApplyEngine,
    pub sync_lifecycle_worker_id: String,
    pub sync_lifecycle: crate::sync_lifecycle::RuntimeSyncLifecycleManager,
    pub sync_lifecycle_snapshot: Option<crate::sync_lifecycle::RuntimeSyncHealthSnapshot>,
    pub spacetime_presence: crate::spacetime_presence::SpacetimePresenceRuntime,
    pub spacetime_presence_snapshot: crate::spacetime_presence::SpacetimePresenceSnapshot,
    pub network_requests: NetworkRequestsState,
    pub starter_jobs: StarterJobsState,
    pub reciprocal_loop: ReciprocalLoopState,
    pub activity_feed: ActivityFeedState,
    pub alerts_recovery: AlertsRecoveryState,
    pub settings: SettingsState,
    pub credentials: CredentialsState,
    pub job_inbox: JobInboxState,
    pub active_job: ActiveJobState,
    pub job_history: JobHistoryState,
    pub earn_job_lifecycle_projection: EarnJobLifecycleProjectionState,
    pub earn_kernel_receipts: crate::state::earn_kernel_receipts::EarnKernelReceiptState,
    pub economy_snapshot: crate::state::economy_snapshot::EconomySnapshotState,
    pub agent_profile_state: AgentProfileStatePaneState,
    pub agent_schedule_tick: AgentScheduleTickPaneState,
    pub trajectory_audit: TrajectoryAuditPaneState,
    pub cast_control: CastControlPaneState,
    pub cast_control_process: Option<CastControlProcess>,
    pub skill_registry: SkillRegistryPaneState,
    pub skill_trust_revocation: SkillTrustRevocationPaneState,
    pub credit_desk: CreditDeskPaneState,
    pub credit_settlement_ledger: CreditSettlementLedgerPaneState,
    pub chat_terminal_worker: crate::chat_terminal::ChatTerminalWorker,
    pub cad_demo: CadDemoPaneState,
    pub stable_sats_simulation: StableSatsSimulationPaneState,
    pub autopilot_goals: crate::state::autopilot_goals::AutopilotGoalsState,
    pub goal_loop_executor: crate::state::goal_loop_executor::GoalLoopExecutorState,
    pub goal_restart_recovery_ran: bool,
    pub sidebar: SidebarState,
    pub next_pane_id: u64,
    pub next_z_index: i32,
    pub pane_drag_mode: Option<PaneDragMode>,
    pub cad_camera_drag_state: Option<CadCameraDragState>,
    pub pane_resizer: ResizablePane,
    pub hotbar_flash_was_active: bool,
    pub command_palette: CommandPalette,
    pub command_palette_actions: Rc<RefCell<Vec<String>>>,
}

impl RenderState {
    pub const fn dev_mode_enabled(&self) -> bool {
        self.desktop_shell_mode.is_dev()
    }

    pub const fn mission_control_buy_mode_enabled(&self) -> bool {
        self.buy_mode_enabled
    }

    pub fn mission_control_buy_mode_toggle_enabled(&self) -> bool {
        self.mission_control.buy_mode_loop_enabled
            || mission_control_buy_mode_start_block_reason(&self.spark_wallet).is_none()
    }

    fn allocate_runtime_command_seq(&mut self) -> u64 {
        let seq = self.next_runtime_command_seq;
        self.next_runtime_command_seq = self.next_runtime_command_seq.saturating_add(1);
        seq
    }

    pub fn reserve_runtime_command_seq(&mut self) -> u64 {
        self.allocate_runtime_command_seq()
    }

    pub fn queue_sa_command(&mut self, command: SaLifecycleCommand) -> Result<u64, String> {
        let seq = self.allocate_runtime_command_seq();
        self.sa_lane_worker.enqueue(seq, command).map(|()| seq)
    }

    fn allocate_codex_command_seq(&mut self) -> u64 {
        let seq = self.next_codex_command_seq;
        self.next_codex_command_seq = self.next_codex_command_seq.saturating_add(1);
        seq
    }

    pub fn queue_codex_command(&mut self, command: CodexLaneCommand) -> Result<u64, String> {
        let seq = self.allocate_codex_command_seq();
        self.codex_lane_worker.enqueue(seq, command).map(|()| seq)
    }

    pub fn restart_codex_lane(&mut self) {
        tracing::info!("codex lane restart requested");
        let replacement = CodexLaneWorker::spawn(self.codex_lane_config.clone());
        let mut previous = std::mem::replace(&mut self.codex_lane_worker, replacement);
        previous.shutdown_async();
        self.codex_lane = CodexLaneSnapshot::default();
        self.autopilot_chat.set_connection_status("starting");
        tracing::info!("codex lane restart dispatched (non-blocking shutdown)");
    }

    pub fn sync_credentials_runtime(&mut self, restart_codex: bool) {
        let codex_scope = crate::credentials::CREDENTIAL_SCOPE_CODEX
            | crate::credentials::CREDENTIAL_SCOPE_SKILLS
            | crate::credentials::CREDENTIAL_SCOPE_GLOBAL;
        let spark_scope = crate::credentials::CREDENTIAL_SCOPE_SPARK
            | crate::credentials::CREDENTIAL_SCOPE_GLOBAL;

        match self.credentials.resolve_env_for_scope(codex_scope) {
            Ok(codex_env) => {
                let changed = self.codex_lane_config.env != codex_env;
                self.codex_lane_config.env = codex_env;
                if restart_codex && changed {
                    self.restart_codex_lane();
                    self.autopilot_chat
                        .set_connection_status("restarting (credential env updated)");
                }
            }
            Err(error) => {
                self.credentials.last_error = Some(error);
                self.credentials.load_state = PaneLoadState::Error;
            }
        }

        match self.credentials.resolve_env_for_scope(spark_scope) {
            Ok(spark_env) => {
                if let Err(error) = self
                    .spark_worker
                    .enqueue(SparkWalletCommand::ConfigureEnv { vars: spark_env })
                {
                    self.credentials.last_error = Some(error);
                    self.credentials.load_state = PaneLoadState::Error;
                } else {
                    let _ = self.spark_worker.enqueue(SparkWalletCommand::Refresh);
                }
            }
            Err(error) => {
                self.credentials.last_error = Some(error);
                self.credentials.load_state = PaneLoadState::Error;
            }
        }
    }

    pub fn record_codex_command_response(&mut self, response: CodexLaneCommandResponse) {
        self.codex_command_responses.push(response);
        if self.codex_command_responses.len() > 128 {
            let overflow = self.codex_command_responses.len().saturating_sub(128);
            self.codex_command_responses.drain(0..overflow);
        }
    }

    pub fn record_codex_notification(&mut self, notification: CodexLaneNotification) {
        self.codex_notifications.push(notification);
        if self.codex_notifications.len() > 256 {
            let overflow = self.codex_notifications.len().saturating_sub(256);
            self.codex_notifications.drain(0..overflow);
        }
    }

    pub fn queue_skl_command(&mut self, command: SklDiscoveryTrustCommand) -> Result<u64, String> {
        let seq = self.allocate_runtime_command_seq();
        self.skl_lane_worker.enqueue(seq, command).map(|()| seq)
    }

    pub fn queue_ac_command(&mut self, command: AcCreditCommand) -> Result<u64, String> {
        let seq = self.allocate_runtime_command_seq();
        self.ac_lane_worker.enqueue(seq, command).map(|()| seq)
    }

    pub fn queue_provider_nip90_lane_command(
        &mut self,
        command: ProviderNip90LaneCommand,
    ) -> Result<(), String> {
        self.provider_nip90_lane_worker.enqueue(command)
    }

    pub fn queue_local_inference_runtime_command(
        &mut self,
        command: LocalInferenceRuntimeCommand,
    ) -> Result<(), String> {
        self.local_inference_runtime.enqueue(command)
    }

    pub fn queue_apple_fm_bridge_command(
        &mut self,
        command: AppleFmBridgeCommand,
    ) -> Result<(), String> {
        self.apple_fm_execution_worker.enqueue(command)
    }

    pub fn mission_control_local_runtime_ready(&self) -> bool {
        mission_control_local_runtime_is_ready(
            self.desktop_shell_mode,
            &self.provider_runtime,
            &self.ollama_execution,
        )
    }

    pub fn mission_control_go_online_enabled(&self) -> bool {
        !matches!(
            self.provider_runtime.mode,
            ProviderMode::Offline | ProviderMode::Degraded
        ) || self.mission_control_local_runtime_ready()
    }

    pub fn configured_provider_relay_urls(&self) -> Vec<String> {
        let relays = self.settings.document.configured_relay_urls();
        if relays.is_empty() {
            return self
                .relay_connections
                .relays
                .iter()
                .map(|row| row.url.trim())
                .filter(|url| !url.is_empty())
                .map(ToString::to_string)
                .collect();
        }
        relays
    }

    pub fn sync_provider_nip90_lane_relays(&mut self) -> Result<(), String> {
        let relays = self.configured_provider_relay_urls();
        self.queue_provider_nip90_lane_command(ProviderNip90LaneCommand::ConfigureRelays { relays })
    }

    pub fn sync_provider_nip90_lane_identity(&mut self) -> Result<(), String> {
        let identity = self
            .nostr_identity
            .as_ref()
            .map(|identity| ProviderNip90AuthIdentity {
                npub: identity.npub.clone(),
                public_key_hex: identity.public_key_hex.clone(),
                private_key_hex: identity.private_key_hex.clone(),
            });
        self.queue_provider_nip90_lane_command(ProviderNip90LaneCommand::ConfigureIdentity {
            identity,
        })
    }

    pub fn sync_chat_identities(&mut self) {
        self.autopilot_chat
            .managed_chat_projection
            .set_local_pubkey(
                self.nostr_identity
                    .as_ref()
                    .map(|identity| identity.public_key_hex.as_str()),
            );
        self.autopilot_chat
            .direct_message_projection
            .set_identity(self.nostr_identity.as_ref());
    }

    pub fn record_runtime_command_response(&mut self, response: RuntimeCommandResponse) {
        self.runtime_command_responses.push(response);
        if self.runtime_command_responses.len() > 128 {
            let overflow = self.runtime_command_responses.len().saturating_sub(128);
            self.runtime_command_responses.drain(0..overflow);
        }
    }

    pub fn provider_blockers(&self) -> Vec<ProviderBlocker> {
        let mut blockers = Vec::new();
        if self.nostr_identity.is_none() {
            blockers.push(ProviderBlocker::IdentityMissing);
        }
        if self.spark_wallet.last_error.is_some() {
            blockers.push(ProviderBlocker::WalletError);
        }
        if !mission_control_sell_compute_supported(self.desktop_shell_mode, &self.ollama_execution)
        {
            return blockers;
        }
        match mission_control_local_runtime_lane(self.desktop_shell_mode, &self.ollama_execution) {
            Some(MissionControlLocalRuntimeLane::AppleFoundationModels) => {
                if !self.provider_runtime.apple_fm.reachable {
                    blockers.push(ProviderBlocker::AppleFoundationModelsUnavailable);
                } else if !self.provider_runtime.apple_fm.is_ready() {
                    blockers.push(ProviderBlocker::AppleFoundationModelsModelUnavailable);
                }
            }
            Some(MissionControlLocalRuntimeLane::NvidiaGptOss) => {
                if !self.provider_runtime.ollama.reachable {
                    blockers.push(ProviderBlocker::OllamaUnavailable);
                } else if !self.provider_runtime.ollama.is_ready() {
                    blockers.push(ProviderBlocker::OllamaModelUnavailable);
                }
            }
            None => {}
        }
        blockers
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ActiveJobState, ActivityEventDomain, ActivityEventRow, ActivityFeedFilter,
        ActivityFeedState, AlertDomain, AlertLifecycle, AlertsRecoveryState, AutopilotChatState,
        AutopilotMessageStatus, AutopilotRole, AutopilotTerminalSessionStatus,
        AutopilotTurnPlanStep, BuyerResolutionMode, BuyerResolutionReason, CadBuildFailureClass,
        CadBuildSessionPhase, CadCameraViewSnap, CadContextMenuTargetKind, CadDemoPaneState,
        CadDemoWarningState, CadDrawingViewDirection, CadDrawingViewMode, CadHiddenLineMode,
        CadHotkeyAction, CadProjectionMode, CadSectionAxis, CadSnapMode, CadThreeDMouseAxis,
        CadThreeDMouseMode, CadThreeDMouseProfile, EarnJobLifecycleProjectionRow,
        EarnJobLifecycleProjectionState, EarningsScoreboardState, JobDemandSource, JobHistoryState,
        JobHistoryStatus, JobHistoryStatusFilter, JobHistoryTimeRange, JobInboxDecision,
        JobInboxNetworkRequest, JobInboxState, JobInboxValidation, JobLifecycleStage,
        MissionControlPaneState, NetworkAggregateCountersState, NetworkRequestStatus,
        NetworkRequestSubmission, NetworkRequestsState, NostrSecretState, ProviderBlocker,
        ProviderMode, ProviderRuntimeState, ReciprocalLoopDirection, ReciprocalLoopFailureClass,
        ReciprocalLoopFailureDisposition, ReciprocalLoopState, RecoveryAlertRow,
        RelayConnectionStatus, RelayConnectionsState, SettingsState, SidebarState, SparkPaneState,
        StableSatsSimulationPaneState, StarterJobRow, StarterJobStatus, StarterJobsState,
        SubmittedNetworkRequest, SyncHealthState, SyncRecoveryPhase,
    };
    use chrono::TimeZone;
    use wgpui::components::sections::TerminalStream;

    #[test]
    fn sidebar_defaults_to_collapsed() {
        let sidebar = SidebarState::default();
        assert!(!sidebar.is_open);
        assert_eq!(sidebar.width, 300.0);
    }

    fn unique_codex_artifact_projection_path(label: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "openagents-codex-artifacts-{label}-{}-{nanos}.json",
            std::process::id()
        ))
    }

    fn unique_temp_dir(label: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!(
            "openagents-cx7-{label}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("temporary directory should be created");
        path
    }

    fn init_git_workspace(label: &str) -> std::path::PathBuf {
        let repo = unique_temp_dir(label);
        let init_status = std::process::Command::new("git")
            .arg("init")
            .arg(&repo)
            .status()
            .expect("git init should launch");
        assert!(init_status.success(), "git init should succeed");
        let config_email = std::process::Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["config", "user.email", "autopilot@example.com"])
            .status()
            .expect("git config email should launch");
        assert!(config_email.success(), "git config email should succeed");
        let config_name = std::process::Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["config", "user.name", "Autopilot"])
            .status()
            .expect("git config name should launch");
        assert!(config_name.success(), "git config name should succeed");
        std::fs::create_dir_all(repo.join("src")).expect("git repo src dir should exist");
        std::fs::write(repo.join("README.md"), "hello\n").expect("initial file should write");
        let add_status = std::process::Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["add", "README.md"])
            .status()
            .expect("git add should launch");
        assert!(add_status.success(), "git add should succeed");
        let commit_status = std::process::Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["commit", "-m", "init"])
            .status()
            .expect("git commit should launch");
        assert!(commit_status.success(), "git commit should succeed");
        repo
    }

    fn fixture_inbox_request(
        request_id: &str,
        capability: &str,
        price_sats: u64,
        ttl_seconds: u64,
        validation: JobInboxValidation,
    ) -> JobInboxNetworkRequest {
        fixture_inbox_request_with_source(
            request_id,
            capability,
            price_sats,
            ttl_seconds,
            validation,
            JobDemandSource::OpenNetwork,
        )
    }

    fn fixture_inbox_request_with_source(
        request_id: &str,
        capability: &str,
        price_sats: u64,
        ttl_seconds: u64,
        validation: JobInboxValidation,
        demand_source: JobDemandSource,
    ) -> JobInboxNetworkRequest {
        JobInboxNetworkRequest {
            request_id: request_id.to_string(),
            requester: format!("npub1{request_id}"),
            demand_source,
            request_kind: 5050,
            capability: capability.to_string(),
            execution_input: Some(format!(
                "Execute capability `{capability}` for request `{request_id}`."
            )),
            execution_prompt: Some(format!("Prompt for {request_id}")),
            execution_params: Vec::new(),
            requested_model: Some("llama3.2:latest".to_string()),
            requested_output_mime: Some("text/plain".to_string()),
            target_provider_pubkeys: Vec::new(),
            encrypted: false,
            encrypted_payload: None,
            parsed_event_shape: None,
            raw_event_json: None,
            skill_scope_id: None,
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_request_event_id: Some(format!("req-event:{request_id}")),
            sa_tick_result_event_id: Some(format!("result-event:{request_id}")),
            ac_envelope_event_id: None,
            price_sats,
            ttl_seconds,
            validation,
        }
    }

    fn seed_job_inbox(requests: Vec<JobInboxNetworkRequest>) -> JobInboxState {
        let mut inbox = JobInboxState::default();
        for request in requests {
            inbox.upsert_network_request(request);
        }
        inbox
    }

    fn fixture_history_row(
        job_id: &str,
        status: JobHistoryStatus,
        completed_at_epoch_seconds: u64,
        payout_sats: u64,
    ) -> super::JobHistoryReceiptRow {
        super::JobHistoryReceiptRow {
            job_id: job_id.to_string(),
            status,
            demand_source: JobDemandSource::OpenNetwork,
            completed_at_epoch_seconds,
            skill_scope_id: None,
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_result_event_id: None,
            sa_trajectory_session_id: None,
            ac_envelope_event_id: None,
            ac_settlement_event_id: None,
            ac_default_event_id: None,
            delivery_proof_id: None,
            delivery_metering_rule_id: None,
            delivery_proof_status_label: None,
            delivery_metered_quantity: None,
            delivery_accepted_quantity: None,
            delivery_variance_reason_label: None,
            delivery_rejection_reason_label: None,
            payout_sats,
            result_hash: format!("sha256:{job_id}"),
            payment_pointer: format!("pay:{job_id}"),
            failure_reason: if status == JobHistoryStatus::Failed {
                Some("failure".to_string())
            } else {
                None
            },
            execution_provenance: None,
        }
    }

    fn seed_job_history(rows: Vec<super::JobHistoryReceiptRow>) -> JobHistoryState {
        let mut history = JobHistoryState::default();
        for row in rows {
            history.upsert_row(row);
        }
        history
    }

    fn fixture_presence_snapshot(
        providers_online: u64,
        node_status: &str,
        last_error: Option<&str>,
        offline_reason: Option<&str>,
    ) -> crate::spacetime_presence::SpacetimePresenceSnapshot {
        crate::spacetime_presence::SpacetimePresenceSnapshot {
            providers_online,
            counter_source: "spacetime.presence".to_string(),
            counter_cardinality: "identity".to_string(),
            node_id: "device:test".to_string(),
            session_id: "sess:test".to_string(),
            node_status: node_status.to_string(),
            node_last_seen_unix_ms: Some(1_761_920_000_000),
            node_offline_reason: offline_reason.map(ToString::to_string),
            last_error: last_error.map(ToString::to_string),
            last_action: Some("fixture presence snapshot".to_string()),
        }
    }

    fn fixture_starter_job(
        job_id: &str,
        payout_sats: u64,
        eligible: bool,
        status: StarterJobStatus,
    ) -> StarterJobRow {
        StarterJobRow {
            job_id: job_id.to_string(),
            summary: "Process starter job".to_string(),
            payout_sats,
            eligible,
            status,
            payout_pointer: None,
            start_confirm_by_unix_ms: None,
            execution_started_at_unix_ms: None,
            execution_expires_at_unix_ms: None,
            last_heartbeat_at_unix_ms: None,
            next_heartbeat_due_at_unix_ms: None,
        }
    }

    fn fixture_loop_submitted_request(
        request_id: &str,
        status: NetworkRequestStatus,
        target_peer_pubkey: &str,
        skill_scope_id: &str,
    ) -> SubmittedNetworkRequest {
        SubmittedNetworkRequest {
            request_id: request_id.to_string(),
            published_request_event_id: Some(request_id.to_string()),
            request_type: "loop.pingpong.10sat".to_string(),
            payload: "{}".to_string(),
            resolution_mode: BuyerResolutionMode::Race,
            target_provider_pubkeys: vec![target_peer_pubkey.to_string()],
            last_provider_pubkey: Some(target_peer_pubkey.to_string()),
            result_provider_pubkey: Some(target_peer_pubkey.to_string()),
            invoice_provider_pubkey: Some(target_peer_pubkey.to_string()),
            last_feedback_status: None,
            last_feedback_event_id: None,
            last_result_event_id: None,
            last_payment_pointer: Some(format!("wallet:{request_id}")),
            payment_required_at_epoch_seconds: Some(1_762_800_000),
            payment_sent_at_epoch_seconds: Some(1_762_800_001),
            payment_failed_at_epoch_seconds: None,
            payment_error: None,
            payment_notice: None,
            pending_bolt11: None,
            skill_scope_id: Some(skill_scope_id.to_string()),
            credit_envelope_ref: Some("ac:envelope:test".to_string()),
            budget_sats: 10,
            timeout_seconds: 90,
            response_stream_id: format!("stream:{request_id}"),
            status,
            authority_command_seq: 1,
            authority_status: Some("accepted".to_string()),
            authority_event_id: Some(request_id.to_string()),
            authority_error_class: None,
            winning_provider_pubkey: Some(target_peer_pubkey.to_string()),
            winning_result_event_id: Some(format!("result:{request_id}")),
            resolution_reason_code: Some(
                BuyerResolutionReason::FirstValidResult.code().to_string(),
            ),
            duplicate_outcomes: Vec::new(),
            resolution_feedbacks: Vec::new(),
            observed_buyer_event_ids: Vec::new(),
            provider_observations: Vec::new(),
        }
    }

    #[derive(Default)]
    struct DeterministicRelayFixture {
        targeted_ingress: std::collections::BTreeMap<String, std::collections::VecDeque<String>>,
        feedback_event_ids: Vec<String>,
        result_event_ids: Vec<String>,
    }

    impl DeterministicRelayFixture {
        fn queue_targeted_request(&mut self, request_id: &str, target_pubkey: &str) {
            self.targeted_ingress
                .entry(target_pubkey.to_string())
                .or_default()
                .push_back(request_id.to_string());
        }

        fn take_next_for(&mut self, target_pubkey: &str) -> Option<String> {
            self.targeted_ingress
                .get_mut(target_pubkey)
                .and_then(std::collections::VecDeque::pop_front)
        }

        fn publish_feedback(
            &mut self,
            request_id: &str,
            from_pubkey: &str,
            to_pubkey: &str,
        ) -> String {
            let event_id = format!("feedback:{request_id}:{from_pubkey}:{to_pubkey}");
            self.feedback_event_ids.push(event_id.clone());
            event_id
        }

        fn publish_result(
            &mut self,
            request_id: &str,
            from_pubkey: &str,
            to_pubkey: &str,
        ) -> String {
            let event_id = format!("result:{request_id}:{from_pubkey}:{to_pubkey}");
            self.result_event_ids.push(event_id.clone());
            event_id
        }
    }

    fn fixture_activity_event(
        event_id: &str,
        domain: ActivityEventDomain,
        occurred_at_epoch_seconds: u64,
    ) -> ActivityEventRow {
        ActivityEventRow {
            event_id: event_id.to_string(),
            domain,
            source_tag: domain.source_tag().to_string(),
            occurred_at_epoch_seconds,
            summary: format!("summary {event_id}"),
            detail: format!("detail {event_id}"),
        }
    }

    fn activity_feed_projection_test_path(name: &str) -> std::path::PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "openagents-activity-feed-{name}-{}-{nonce}.json",
            std::process::id()
        ))
    }

    fn activity_feed_state_for_tests(name: &str) -> ActivityFeedState {
        let path = activity_feed_projection_test_path(name);
        let _ = std::fs::remove_file(path.as_path());
        ActivityFeedState::from_projection_path_for_tests(path)
    }

    fn earn_projection_test_path(name: &str) -> std::path::PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "openagents-earn-projection-{name}-{}-{nonce}.json",
            std::process::id()
        ))
    }

    fn fixture_alert(
        alert_id: &str,
        domain: AlertDomain,
        severity: super::AlertSeverity,
        lifecycle: AlertLifecycle,
    ) -> RecoveryAlertRow {
        RecoveryAlertRow {
            alert_id: alert_id.to_string(),
            domain,
            severity,
            lifecycle,
            summary: format!("summary {alert_id}"),
            remediation: format!("remediation {alert_id}"),
            last_transition_epoch_seconds: 1_761_920_080,
        }
    }

    #[test]
    fn nostr_reveal_state_expires() {
        let mut state = NostrSecretState::default();
        let now = std::time::Instant::now();
        state.toggle_reveal(now);
        assert!(state.is_revealed(now));

        let expired_at = now + state.reveal_duration + std::time::Duration::from_millis(1);
        assert!(state.expire(expired_at));
        assert!(!state.is_revealed(expired_at));
    }

    #[test]
    fn nostr_copy_notice_expires() {
        let mut state = NostrSecretState::default();
        let now = std::time::Instant::now();
        state.set_copy_notice(now, "Copied".to_string());
        assert_eq!(state.copy_notice.as_deref(), Some("Copied"));

        let expired_at = now + std::time::Duration::from_secs(5);
        assert!(state.expire(expired_at));
        assert!(state.copy_notice.is_none());
    }

    #[test]
    fn chat_state_tracks_codex_turn_lifecycle() {
        let mut chat = AutopilotChatState::default();
        chat.ensure_thread("thread-1".to_string());
        chat.submit_prompt("ping".to_string());
        assert!(
            chat.messages
                .iter()
                .any(|message| message.status == AutopilotMessageStatus::Queued)
        );

        chat.mark_turn_started("turn-1".to_string());
        assert!(
            chat.messages
                .iter()
                .any(|message| message.status == AutopilotMessageStatus::Running)
        );

        chat.append_turn_delta("pong");
        chat.mark_turn_completed();
        assert!(!chat.has_pending_messages());
        assert!(
            chat.messages
                .iter()
                .any(|message| message.content.contains("pong"))
        );
    }

    #[test]
    fn chat_state_records_turn_metadata_and_binds_to_started_turn() {
        let mut chat = AutopilotChatState::default();
        chat.ensure_thread("thread-1".to_string());
        chat.submit_prompt("design a rack".to_string());
        chat.record_turn_submission_metadata(
            "thread-1",
            crate::labor_orchestrator::CodexRunClassification::PersonalAgent,
            None,
            true,
            "keyword-pair:design+rack",
            1000,
            Vec::new(),
        );

        let pending = chat
            .active_turn_metadata()
            .expect("latest submitted metadata should be available");
        assert_eq!(
            pending.run_classification,
            crate::labor_orchestrator::CodexRunClassification::PersonalAgent
        );
        assert!(pending.is_cad_turn);
        assert_eq!(pending.classifier_reason, "keyword-pair:design+rack");

        chat.mark_turn_started("turn-1".to_string());
        let bound = chat
            .turn_metadata_for("turn-1")
            .expect("turn metadata should bind when turn starts");
        assert_eq!(
            bound.run_classification,
            crate::labor_orchestrator::CodexRunClassification::PersonalAgent
        );
        assert!(bound.is_cad_turn);
        assert_eq!(bound.classifier_reason, "keyword-pair:design+rack");
        assert_eq!(chat.active_turn_metadata(), Some(bound));
    }

    #[test]
    fn chat_state_updates_pending_turn_selected_skills_for_audit_capture() {
        let mut chat = AutopilotChatState::default();
        chat.ensure_thread("thread-1".to_string());
        chat.submit_prompt("earn bitcoin".to_string());
        chat.record_turn_submission_metadata(
            "thread-1",
            crate::labor_orchestrator::CodexRunClassification::AutonomousGoal {
                goal_id: "goal-earn".to_string(),
                goal_title: "Earn bitcoin".to_string(),
            },
            None,
            false,
            "no-cad-signals",
            1000,
            Vec::new(),
        );

        chat.set_last_pending_turn_selected_skills(vec![
            "blink".to_string(),
            "l402".to_string(),
            "blink".to_string(),
        ]);

        let pending = chat
            .pending_turn_metadata
            .back()
            .expect("pending metadata should remain queued");
        assert_eq!(
            pending.run_classification,
            crate::labor_orchestrator::CodexRunClassification::AutonomousGoal {
                goal_id: "goal-earn".to_string(),
                goal_title: "Earn bitcoin".to_string(),
            }
        );
        assert_eq!(
            pending.selected_skill_names,
            vec!["blink".to_string(), "l402".to_string()]
        );
    }

    fn fixture_goal_labor_binding() -> crate::labor_orchestrator::CodexLaborBinding {
        crate::labor_orchestrator::orchestrate_codex_turn(
            crate::labor_orchestrator::CodexTurnExecutionRequest {
                trigger: crate::labor_orchestrator::CodexRunTrigger::AutonomousGoal {
                    goal_id: "goal-earn".to_string(),
                    goal_title: "Earn bitcoin".to_string(),
                },
                submitted_at_epoch_ms: 1_000,
                thread_id: "thread-1".to_string(),
                input: vec![codex_client::UserInput::Text {
                    text: "earn bitcoin".to_string(),
                    text_elements: Vec::new(),
                }],
                cwd: Some(std::path::PathBuf::from("/repo")),
                approval_policy: Some(codex_client::AskForApproval::Never),
                sandbox_policy: Some(codex_client::SandboxPolicy::DangerFullAccess),
                model: Some("gpt-5.2-codex".to_string()),
                service_tier: None,
                effort: None,
                personality: None,
                collaboration_mode: None,
            },
        )
        .labor_binding
        .expect("goal runs should create labor binding")
    }

    #[test]
    fn chat_state_updates_labor_binding_with_turn_and_output_provenance() {
        let mut chat = AutopilotChatState::default();
        chat.ensure_thread("thread-1".to_string());
        chat.submit_prompt("earn bitcoin".to_string());
        let labor_binding = crate::labor_orchestrator::orchestrate_codex_turn(
            crate::labor_orchestrator::CodexTurnExecutionRequest {
                trigger: crate::labor_orchestrator::CodexRunTrigger::AutonomousGoal {
                    goal_id: "goal-earn".to_string(),
                    goal_title: "Earn bitcoin".to_string(),
                },
                submitted_at_epoch_ms: 1_000,
                thread_id: "thread-1".to_string(),
                input: vec![
                    codex_client::UserInput::Text {
                        text: "earn bitcoin".to_string(),
                        text_elements: Vec::new(),
                    },
                    codex_client::UserInput::Skill {
                        name: "blink".to_string(),
                        path: std::path::PathBuf::from("/repo/skills/blink/SKILL.md"),
                    },
                ],
                cwd: Some(std::path::PathBuf::from("/repo")),
                approval_policy: Some(codex_client::AskForApproval::Never),
                sandbox_policy: Some(codex_client::SandboxPolicy::DangerFullAccess),
                model: Some("gpt-5.2-codex".to_string()),
                service_tier: None,
                effort: None,
                personality: None,
                collaboration_mode: None,
            },
        )
        .labor_binding
        .expect("goal runs should create labor binding");
        chat.record_turn_submission_metadata(
            "thread-1",
            crate::labor_orchestrator::CodexRunClassification::AutonomousGoal {
                goal_id: "goal-earn".to_string(),
                goal_title: "Earn bitcoin".to_string(),
            },
            Some(labor_binding),
            false,
            "no-cad-signals",
            1000,
            Vec::new(),
        );

        chat.set_last_pending_turn_selected_skills(vec!["blink".to_string(), "blink".to_string()]);
        chat.mark_turn_started("turn-1".to_string());
        chat.set_turn_message_for_turn("turn-1", "final answer");

        let bound = chat
            .turn_metadata_for("turn-1")
            .expect("turn metadata should bind");
        let labor_binding = bound
            .labor_binding
            .as_ref()
            .expect("labor binding should remain attached");
        assert_eq!(labor_binding.provenance.turn_id.as_deref(), Some("turn-1"));
        assert_eq!(
            labor_binding.provenance.selected_skill_names,
            vec!["blink".to_string()]
        );
        assert!(labor_binding.provenance.final_output_digest.is_some());
        assert!(labor_binding.provenance.transcript_digest.is_some());
    }

    #[test]
    fn chat_state_records_labor_approval_and_tool_events() {
        let mut chat = AutopilotChatState::default();
        chat.ensure_thread("thread-1".to_string());
        chat.submit_prompt("earn bitcoin".to_string());
        let labor_binding = crate::labor_orchestrator::orchestrate_codex_turn(
            crate::labor_orchestrator::CodexTurnExecutionRequest {
                trigger: crate::labor_orchestrator::CodexRunTrigger::AutonomousGoal {
                    goal_id: "goal-earn".to_string(),
                    goal_title: "Earn bitcoin".to_string(),
                },
                submitted_at_epoch_ms: 1_000,
                thread_id: "thread-1".to_string(),
                input: vec![codex_client::UserInput::Text {
                    text: "earn bitcoin".to_string(),
                    text_elements: Vec::new(),
                }],
                cwd: Some(std::path::PathBuf::from("/repo")),
                approval_policy: Some(codex_client::AskForApproval::Never),
                sandbox_policy: Some(codex_client::SandboxPolicy::DangerFullAccess),
                model: Some("gpt-5.2-codex".to_string()),
                service_tier: None,
                effort: None,
                personality: None,
                collaboration_mode: None,
            },
        )
        .labor_binding
        .expect("goal runs should create labor binding");
        chat.record_turn_submission_metadata(
            "thread-1",
            crate::labor_orchestrator::CodexRunClassification::AutonomousGoal {
                goal_id: "goal-earn".to_string(),
                goal_title: "Earn bitcoin".to_string(),
            },
            Some(labor_binding),
            false,
            "no-cad-signals",
            1000,
            Vec::new(),
        );
        chat.mark_turn_started("turn-1".to_string());

        chat.record_turn_command_approval_requested(
            "turn-1",
            "item-1",
            Some("needs command"),
            Some("git status"),
            Some("/repo"),
            1010,
        );
        chat.record_turn_command_approval_response("turn-1", "item-1", "Never", 1020);
        chat.record_turn_tool_request(
            "turn-1",
            "request-1",
            "call-1",
            "openagents.files.read",
            "{\"path\":\"README.md\"}",
            1030,
        );
        chat.record_turn_tool_result(
            "turn-1",
            "request-1",
            "call-1",
            "openagents.files.read",
            "OK",
            true,
            "read completed",
            1040,
        );

        let bound = chat
            .turn_metadata_for("turn-1")
            .expect("turn metadata should bind");
        let labor_binding = bound
            .labor_binding
            .as_ref()
            .expect("labor binding should remain attached");
        assert_eq!(labor_binding.provenance.approval_events.len(), 2);
        assert_eq!(labor_binding.provenance.tool_invocations.len(), 1);
        assert_eq!(
            labor_binding.provenance.tool_invocations[0]
                .response_code
                .as_deref(),
            Some("OK")
        );
    }

    #[test]
    fn labor_submission_can_exist_without_verdict() {
        let mut chat = AutopilotChatState::default();
        chat.ensure_thread("thread-1".to_string());
        chat.submit_prompt("earn bitcoin".to_string());
        chat.record_turn_submission_metadata(
            "thread-1",
            crate::labor_orchestrator::CodexRunClassification::AutonomousGoal {
                goal_id: "goal-earn".to_string(),
                goal_title: "Earn bitcoin".to_string(),
            },
            Some(fixture_goal_labor_binding()),
            false,
            "no-cad-signals",
            1_000,
            Vec::new(),
        );
        chat.mark_turn_started("turn-1".to_string());
        chat.set_turn_message_for_turn("turn-1", "verified later");
        chat.mark_turn_completed_for("turn-1");

        let submission = chat
            .assemble_turn_labor_submission("turn-1", 2_000)
            .expect("submission assembly should not fail")
            .expect("labor-bound turn should produce submission");

        assert_eq!(
            submission.submission.status,
            openagents_kernel_core::labor::SubmissionStatus::Received
        );
        assert_eq!(
            submission.verifier_path,
            crate::labor_orchestrator::CodexLaborVerifierPath::DeterministicOutputGate
        );
        assert!(chat.turn_labor_submission_for("turn-1").is_some());
        assert!(chat.turn_labor_verdict_for("turn-1").is_none());
        assert_eq!(chat.turn_labor_settlement_ready("turn-1"), Some(false));
    }

    #[test]
    fn labor_verdict_pass_marks_settlement_ready() {
        let mut chat = AutopilotChatState::default();
        chat.ensure_thread("thread-1".to_string());
        chat.submit_prompt("earn bitcoin".to_string());
        chat.record_turn_submission_metadata(
            "thread-1",
            crate::labor_orchestrator::CodexRunClassification::AutonomousGoal {
                goal_id: "goal-earn".to_string(),
                goal_title: "Earn bitcoin".to_string(),
            },
            Some(fixture_goal_labor_binding()),
            false,
            "no-cad-signals",
            1_000,
            Vec::new(),
        );
        chat.mark_turn_started("turn-1".to_string());
        chat.set_turn_message_for_turn("turn-1", "final answer");
        chat.mark_turn_completed_for("turn-1");
        chat.assemble_turn_labor_submission("turn-1", 2_000)
            .expect("submission assembly should succeed");

        let verdict = chat
            .finalize_turn_labor_verdict("turn-1", 3_000)
            .expect("verdict finalization should succeed")
            .expect("labor-bound turn should produce verdict");

        assert_eq!(verdict.outcome_label(), "pass");
        assert!(verdict.settlement_ready);
        assert_eq!(
            chat.turn_labor_submission_for("turn-1")
                .expect("submission should remain attached")
                .submission
                .status,
            openagents_kernel_core::labor::SubmissionStatus::Accepted
        );
        assert_eq!(chat.turn_labor_settlement_ready("turn-1"), Some(true));
    }

    #[test]
    fn labor_verdict_fail_withholds_settlement() {
        let mut chat = AutopilotChatState::default();
        chat.ensure_thread("thread-1".to_string());
        chat.submit_prompt("earn bitcoin".to_string());
        chat.record_turn_submission_metadata(
            "thread-1",
            crate::labor_orchestrator::CodexRunClassification::AutonomousGoal {
                goal_id: "goal-earn".to_string(),
                goal_title: "Earn bitcoin".to_string(),
            },
            Some(fixture_goal_labor_binding()),
            false,
            "no-cad-signals",
            1_000,
            Vec::new(),
        );
        chat.mark_turn_started("turn-1".to_string());
        chat.record_turn_tool_request(
            "turn-1",
            "request-1",
            "call-1",
            "openagents.files.write",
            "{\"path\":\"README.md\"}",
            1_050,
        );
        chat.record_turn_tool_result(
            "turn-1",
            "request-1",
            "call-1",
            "openagents.files.write",
            "FAILED",
            false,
            "write denied",
            1_060,
        );
        chat.set_turn_message_for_turn("turn-1", "attempted answer");
        chat.mark_turn_completed_for("turn-1");
        chat.assemble_turn_labor_submission("turn-1", 2_000)
            .expect("submission assembly should succeed");

        let verdict = chat
            .finalize_turn_labor_verdict("turn-1", 3_000)
            .expect("verdict finalization should succeed")
            .expect("labor-bound turn should produce verdict");

        assert_eq!(verdict.outcome_label(), "fail");
        assert!(!verdict.settlement_ready);
        assert_eq!(
            chat.turn_labor_submission_for("turn-1")
                .expect("submission should remain attached")
                .submission
                .status,
            openagents_kernel_core::labor::SubmissionStatus::Rejected
        );
        assert_eq!(chat.turn_labor_settlement_ready("turn-1"), Some(false));
    }

    #[test]
    fn labor_verifier_failure_blocks_settlement_ready() {
        let mut chat = AutopilotChatState::default();
        chat.ensure_thread("thread-1".to_string());
        chat.submit_prompt("earn bitcoin".to_string());
        chat.record_turn_submission_metadata(
            "thread-1",
            crate::labor_orchestrator::CodexRunClassification::AutonomousGoal {
                goal_id: "goal-earn".to_string(),
                goal_title: "Earn bitcoin".to_string(),
            },
            Some(fixture_goal_labor_binding()),
            false,
            "no-cad-signals",
            1_000,
            Vec::new(),
        );
        chat.mark_turn_started("turn-1".to_string());
        chat.mark_turn_completed_for("turn-1");
        chat.assemble_turn_labor_submission("turn-1", 2_000)
            .expect("submission assembly should succeed");

        let error = chat
            .finalize_turn_labor_verdict("turn-1", 3_000)
            .expect_err("verifier should reject missing output evidence");

        assert_eq!(
            error,
            "codex labor verifier requires a final output reference".to_string()
        );
        let binding = chat
            .turn_metadata_for("turn-1")
            .and_then(|metadata| metadata.labor_binding.as_ref())
            .expect("labor binding should remain attached");
        assert_eq!(
            binding
                .verifier_failure
                .as_ref()
                .map(|failure| failure.code.as_str()),
            Some("codex_submission_output_missing")
        );
        assert!(binding.verdict.is_none());
        assert_eq!(chat.turn_labor_settlement_ready("turn-1"), Some(false));
    }

    #[test]
    fn labor_evidence_attachment_updates_scope_and_payload() {
        let mut chat = AutopilotChatState::default();
        chat.ensure_thread("thread-1".to_string());
        chat.submit_prompt("earn bitcoin".to_string());
        chat.record_turn_submission_metadata(
            "thread-1",
            crate::labor_orchestrator::CodexRunClassification::AutonomousGoal {
                goal_id: "goal-earn".to_string(),
                goal_title: "Earn bitcoin".to_string(),
            },
            Some(fixture_goal_labor_binding()),
            false,
            "no-cad-signals",
            1_000,
            Vec::new(),
        );
        chat.mark_turn_started("turn-1".to_string());

        let scope = chat
            .turn_labor_scope_payload("turn-1")
            .expect("scope should be available");
        let artifact_scope_root = scope
            .get("artifact_scope_root")
            .and_then(|value| value.as_str())
            .expect("artifact scope root should exist")
            .to_string();

        let payload = chat
            .attach_turn_labor_evidence(
                "turn-1",
                openagents_kernel_core::receipts::EvidenceRef::new(
                    "tool_log",
                    format!("{artifact_scope_root}tool-log"),
                    "sha256:tool-log",
                ),
                false,
            )
            .expect("evidence attach should succeed")
            .expect("labor-bound turn should return payload");
        assert_eq!(
            payload
                .get("attached")
                .and_then(|value| value.as_array())
                .map(Vec::len),
            Some(1)
        );

        let incident_payload = chat
            .attach_turn_labor_evidence(
                "turn-1",
                openagents_kernel_core::receipts::EvidenceRef::new(
                    "incident_note",
                    format!("{artifact_scope_root}incidents/note-1"),
                    "sha256:incident-note",
                ),
                true,
            )
            .expect("incident attach should succeed")
            .expect("labor-bound turn should return payload");
        assert_eq!(
            incident_payload
                .get("incident")
                .and_then(|value| value.as_array())
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(
            chat.turn_labor_requirements_payload("turn-1")
                .and_then(|payload| payload.get("evidence_gaps").cloned())
                .and_then(|value| value.as_array().cloned())
                .map(|gaps| gaps.len()),
            Some(2)
        );
    }

    #[test]
    fn labor_claim_lifecycle_updates_payload_and_goal_linkage() {
        let mut chat = AutopilotChatState::default();
        chat.ensure_thread("thread-1".to_string());
        chat.submit_prompt("earn bitcoin".to_string());
        chat.record_turn_submission_metadata(
            "thread-1",
            crate::labor_orchestrator::CodexRunClassification::AutonomousGoal {
                goal_id: "goal-earn".to_string(),
                goal_title: "Earn bitcoin".to_string(),
            },
            Some(fixture_goal_labor_binding()),
            false,
            "no-cad-signals",
            1_000,
            Vec::new(),
        );
        chat.mark_turn_started("turn-1".to_string());
        chat.record_turn_tool_request(
            "turn-1",
            "request-1",
            "call-1",
            "openagents.files.write",
            "{\"path\":\"README.md\"}",
            1_050,
        );
        chat.record_turn_tool_result(
            "turn-1",
            "request-1",
            "call-1",
            "openagents.files.write",
            "FAILED",
            false,
            "write denied",
            1_060,
        );
        chat.set_turn_message_for_turn("turn-1", "attempted answer");
        chat.mark_turn_completed_for("turn-1");

        let artifact_scope_root = chat
            .turn_labor_binding_for("turn-1")
            .expect("labor binding should exist")
            .artifact_scope_root();
        chat.attach_turn_labor_evidence(
            "turn-1",
            openagents_kernel_core::receipts::EvidenceRef::new(
                "incident_note",
                format!("{artifact_scope_root}incidents/note-1"),
                "sha256:incident-note",
            ),
            true,
        )
        .expect("incident attach should succeed");

        chat.assemble_turn_labor_submission("turn-1", 2_000)
            .expect("submission assembly should succeed");
        chat.finalize_turn_labor_verdict("turn-1", 3_000)
            .expect("verdict finalization should succeed");
        chat.open_turn_labor_claim("turn-1", 3_100, None, Some("operator requested review"))
            .expect("claim open should succeed");
        chat.review_turn_labor_claim("turn-1", 3_200, Some("checking failure details"))
            .expect("claim review should succeed");
        chat.issue_turn_labor_remedy("turn-1", 3_300, "rework_credit", Some("issue credit"))
            .expect("remedy issuance should succeed");

        let payload = chat
            .turn_labor_claim_payload("turn-1")
            .expect("claim payload should exist");
        assert_eq!(
            payload.pointer("/claim_state"),
            Some(&serde_json::json!("remedy_issued"))
        );
        assert_eq!(
            payload.pointer("/claim/remedy/outcome"),
            Some(&serde_json::json!("rework_credit"))
        );

        let linkage = chat
            .turn_labor_linkage_for("turn-1")
            .expect("goal linkage should exist");
        assert_eq!(linkage.claim_state.as_deref(), Some("remedy_issued"));
        assert_eq!(linkage.remedy_kind.as_deref(), Some("rework_credit"));
        assert!(!linkage.claim_evidence_refs.is_empty());
        assert_eq!(linkage.incident_evidence_refs.len(), 1);
        assert_eq!(linkage.remedy_evidence_refs.len(), 1);

        chat.resolve_turn_labor_claim("turn-1", 3_400, Some("claim closed"))
            .expect("claim resolution should succeed");
        assert_eq!(
            chat.turn_labor_linkage_for("turn-1")
                .and_then(|labor| labor.claim_state),
            Some("claim_resolved".to_string())
        );
    }

    #[test]
    fn chat_state_binds_turn_metadata_in_submission_order() {
        let mut chat = AutopilotChatState::default();
        chat.ensure_thread("thread-1".to_string());

        chat.submit_prompt("summarize commits".to_string());
        chat.record_turn_submission_metadata(
            "thread-1",
            crate::labor_orchestrator::CodexRunClassification::PersonalAgent,
            None,
            false,
            "no-cad-signals",
            1010,
            Vec::new(),
        );
        chat.submit_prompt("design wall mount bracket".to_string());
        chat.record_turn_submission_metadata(
            "thread-1",
            crate::labor_orchestrator::CodexRunClassification::LaborMarket {
                work_unit_id: "wu-2".to_string(),
                contract_id: Some("contract-2".to_string()),
            },
            None,
            true,
            "keyword-pair:design+bracket",
            1020,
            Vec::new(),
        );

        chat.mark_turn_started("turn-a".to_string());
        chat.mark_turn_started("turn-b".to_string());

        let first = chat
            .turn_metadata_for("turn-a")
            .expect("first turn metadata should bind");
        assert_eq!(
            first.run_classification,
            crate::labor_orchestrator::CodexRunClassification::PersonalAgent
        );
        assert!(!first.is_cad_turn);
        assert_eq!(first.classifier_reason, "no-cad-signals");

        let second = chat
            .turn_metadata_for("turn-b")
            .expect("second turn metadata should bind");
        assert_eq!(
            second.run_classification,
            crate::labor_orchestrator::CodexRunClassification::LaborMarket {
                work_unit_id: "wu-2".to_string(),
                contract_id: Some("contract-2".to_string()),
            }
        );
        assert!(second.is_cad_turn);
        assert_eq!(second.classifier_reason, "keyword-pair:design+bracket");
    }

    #[test]
    fn chat_state_drops_pending_metadata_when_dispatch_fails() {
        let mut chat = AutopilotChatState::default();
        chat.ensure_thread("thread-1".to_string());
        chat.submit_prompt("design fixture".to_string());
        chat.record_turn_submission_metadata(
            "thread-1",
            crate::labor_orchestrator::CodexRunClassification::PersonalAgent,
            None,
            true,
            "keyword-pair:design+fixture",
            1200,
            Vec::new(),
        );
        assert_eq!(chat.pending_turn_metadata.len(), 1);

        chat.mark_pending_turn_dispatch_failed("queue failed");
        assert!(chat.pending_turn_metadata.is_empty());
    }

    #[test]
    fn chat_copy_notice_expires() {
        let mut chat = AutopilotChatState::default();
        let now = std::time::Instant::now();
        chat.set_copy_notice(now, "Copied message".to_string());
        assert_eq!(chat.copy_notice.as_deref(), Some("Copied message"));

        let expired_at = now + std::time::Duration::from_secs(4);
        assert!(chat.expire_copy_notice(expired_at));
        assert!(chat.copy_notice.is_none());
    }

    #[test]
    fn chat_state_replaces_transcript_for_active_thread() {
        let mut chat = AutopilotChatState::default();
        chat.ensure_thread("thread-a".to_string());
        chat.set_active_thread_transcript(
            "thread-a",
            vec![
                (AutopilotRole::User, "hello".to_string()),
                (AutopilotRole::Codex, "world".to_string()),
            ],
        );
        assert_eq!(chat.messages.len(), 2);
        assert_eq!(chat.messages[0].content, "hello");
        assert_eq!(chat.messages[1].content, "world");

        chat.remember_thread("thread-b");
        chat.set_active_thread_transcript(
            "thread-b",
            vec![(AutopilotRole::User, "ignored".to_string())],
        );
        assert_eq!(chat.messages[0].content, "hello");

        chat.ensure_thread("thread-b".to_string());
        chat.set_active_thread_transcript(
            "thread-b",
            vec![(AutopilotRole::User, "new thread".to_string())],
        );
        assert_eq!(chat.messages.len(), 1);
        assert_eq!(chat.messages[0].content, "new thread");
    }

    #[test]
    fn chat_state_binds_turns_to_assistant_slots_in_submission_order() {
        let mut chat = AutopilotChatState::default();
        chat.ensure_thread("thread-1".to_string());
        chat.submit_prompt("first".to_string());
        chat.submit_prompt("second".to_string());
        let queued_assistant_ids = chat
            .messages
            .iter()
            .filter(|message| {
                message.role == AutopilotRole::Codex
                    && message.status == AutopilotMessageStatus::Queued
            })
            .map(|message| message.id)
            .collect::<Vec<_>>();
        assert_eq!(queued_assistant_ids.len(), 2);

        // The first turn starts after a second prompt was already queued.
        chat.mark_turn_started("turn-1".to_string());
        chat.append_turn_delta_for_turn("turn-1", "resp-first");
        chat.mark_turn_completed_for("turn-1");

        // First assistant slot should carry first turn output.
        assert_eq!(
            chat.messages
                .iter()
                .find(|message| message.id == queued_assistant_ids[0])
                .map(|message| message.content.as_str()),
            Some("resp-first")
        );
        assert_eq!(
            chat.messages
                .iter()
                .find(|message| message.id == queued_assistant_ids[1])
                .map(|message| message.status),
            Some(AutopilotMessageStatus::Queued)
        );

        chat.mark_turn_started("turn-2".to_string());
        chat.append_turn_delta_for_turn("turn-2", "resp-second");
        chat.mark_turn_completed_for("turn-2");

        assert_eq!(
            chat.messages
                .iter()
                .find(|message| message.id == queued_assistant_ids[1])
                .map(|message| message.content.as_str()),
            Some("resp-second")
        );
        assert!(!chat.has_pending_messages());
    }

    #[test]
    fn chat_state_preserves_turn_binding_after_completion_for_late_events() {
        let mut chat = AutopilotChatState::default();
        chat.ensure_thread("thread-1".to_string());
        chat.submit_prompt("first".to_string());
        let first_assistant_id = chat
            .messages
            .iter()
            .rev()
            .find(|message| {
                message.role == AutopilotRole::Codex
                    && message.status == AutopilotMessageStatus::Queued
            })
            .map(|message| message.id)
            .unwrap_or_default();

        chat.mark_turn_started("turn-1".to_string());
        chat.append_turn_delta_for_turn("turn-1", "resp");
        chat.mark_turn_completed_for("turn-1");

        // Queue another prompt so there is a fresh pending assistant slot.
        chat.submit_prompt("second".to_string());
        chat.append_turn_delta_for_turn("turn-1", "-late");

        assert_eq!(
            chat.messages
                .iter()
                .find(|message| message.id == first_assistant_id)
                .map(|message| message.content.as_str()),
            Some("resp-late")
        );
    }

    #[test]
    fn chat_state_dedupes_agent_deltas_with_fallback_item_ids() {
        let mut chat = AutopilotChatState::default();
        chat.ensure_thread("thread-1".to_string());
        chat.submit_prompt("hello".to_string());
        chat.mark_turn_started("turn-1".to_string());

        assert!(!chat.is_duplicate_agent_delta("turn-1", "item-1", "abc"));
        assert!(chat.is_duplicate_agent_delta("turn-1", "item-1", "abc"));
        assert!(chat.is_duplicate_agent_delta("turn-1", "event-agent-message", "abc"));
        assert!(!chat.is_duplicate_agent_delta("turn-1", "item-1", "def"));
    }

    #[test]
    fn chat_state_dedupes_reasoning_deltas_with_na_item_id() {
        let mut chat = AutopilotChatState::default();
        chat.ensure_thread("thread-1".to_string());
        chat.submit_prompt("hello".to_string());
        chat.mark_turn_started("turn-1".to_string());

        assert!(!chat.is_duplicate_reasoning_delta("turn-1", "rs-1", "plan"));
        assert!(chat.is_duplicate_reasoning_delta("turn-1", "rs-1", "plan"));
        assert!(chat.is_duplicate_reasoning_delta("turn-1", "n/a", "plan"));
        assert!(!chat.is_duplicate_reasoning_delta("turn-1", "rs-1", "next"));
    }

    #[test]
    fn chat_state_preserves_preformatted_reasoning_answer_messages() {
        let mut chat = AutopilotChatState::default();
        chat.ensure_thread("thread-1".to_string());
        chat.submit_prompt("hello".to_string());
        chat.mark_turn_started("turn-1".to_string());

        let combined = "Reasoning:\n**Check**\n\nplan\n\nAnswer:\nDone.";
        chat.set_turn_message_for_turn("turn-1", combined);
        let assistant_id = *chat
            .turn_assistant_message_ids
            .get("turn-1")
            .expect("turn should bind to an assistant message");
        let codex = chat
            .messages
            .iter()
            .find(|message| message.id == assistant_id)
            .expect("codex message should exist");
        assert_eq!(codex.content, combined);
    }

    #[test]
    fn chat_state_prefers_coding_model_over_server_default() {
        let mut chat = AutopilotChatState::default();
        chat.set_models(
            vec![
                "gpt-5.3-codex".to_string(),
                "gpt-5.2-codex".to_string(),
                "gpt-5.2".to_string(),
            ],
            Some("gpt-5.3-codex".to_string()),
        );
        assert_eq!(chat.current_model(), "gpt-5.2-codex");
    }

    #[test]
    fn chat_state_preserves_explicit_user_model_selection() {
        let mut chat = AutopilotChatState::default();
        chat.set_models(
            vec![
                "gpt-5.3-codex".to_string(),
                "gpt-5.2-codex".to_string(),
                "gpt-5.2".to_string(),
            ],
            Some("gpt-5.3-codex".to_string()),
        );
        // Simulate user cycling to a specific model.
        chat.selected_model = 0; // gpt-5.3-codex
        chat.set_models(
            vec![
                "gpt-5.3-codex".to_string(),
                "gpt-5.2-codex".to_string(),
                "gpt-5.2".to_string(),
            ],
            Some("gpt-5.3-codex".to_string()),
        );
        assert_eq!(chat.current_model(), "gpt-5.3-codex");
    }

    #[test]
    fn chat_state_preserves_active_thread_when_list_omits_it() {
        let mut chat = AutopilotChatState::default();
        chat.ensure_thread("thread-active".to_string());
        chat.set_thread_name("thread-active", Some("Active".to_string()));
        chat.set_thread_status("thread-active", Some("active".to_string()));

        chat.set_thread_entries(vec![super::AutopilotThreadListEntry {
            thread_id: "thread-other".to_string(),
            thread_name: Some("Other".to_string()),
            preview: "other preview".to_string(),
            status: Some("idle".to_string()),
            loaded: false,
            cwd: Some("/tmp/other".to_string()),
            path: None,
            created_at: 1_700_000_000,
            updated_at: 1_700_000_100,
        }]);

        assert_eq!(chat.active_thread_id.as_deref(), Some("thread-active"));
        assert!(
            chat.threads
                .iter()
                .any(|thread_id| thread_id == "thread-active")
        );
        assert_eq!(
            chat.thread_metadata
                .get("thread-active")
                .and_then(|metadata| metadata.thread_name.as_deref()),
            Some("Active")
        );
    }

    #[test]
    fn chat_state_selecting_thread_clears_previous_transcript() {
        let mut chat = AutopilotChatState::default();
        chat.ensure_thread("thread-a".to_string());
        chat.set_active_thread_transcript(
            "thread-a",
            vec![(AutopilotRole::User, "previous thread".to_string())],
        );
        chat.remember_thread("thread-b");
        let selected_index = chat
            .threads
            .iter()
            .position(|thread_id| thread_id == "thread-b")
            .expect("thread-b should exist");

        let selected = chat
            .select_thread_by_index(selected_index)
            .expect("known thread should select");

        assert_eq!(selected.thread_id, "thread-b");
        assert!(chat.messages.is_empty());
    }

    #[test]
    fn chat_state_tracks_plan_artifacts_per_thread() {
        let mut chat = AutopilotChatState::default();
        chat.set_thread_entries(vec![
            super::AutopilotThreadListEntry {
                thread_id: "thread-a".to_string(),
                thread_name: Some("Alpha".to_string()),
                preview: "first preview".to_string(),
                status: Some("idle".to_string()),
                loaded: false,
                cwd: Some("/tmp/a".to_string()),
                path: Some("/tmp/a.jsonl".to_string()),
                created_at: 1_700_000_000,
                updated_at: 1_700_000_100,
            },
            super::AutopilotThreadListEntry {
                thread_id: "thread-b".to_string(),
                thread_name: Some("Beta".to_string()),
                preview: "second preview".to_string(),
                status: Some("idle".to_string()),
                loaded: false,
                cwd: Some("/tmp/b".to_string()),
                path: Some("/tmp/b.jsonl".to_string()),
                created_at: 1_700_000_200,
                updated_at: 1_700_000_300,
            },
        ]);

        chat.set_plan_artifact(
            "thread-a",
            "turn-a",
            Some("Plan A".to_string()),
            vec![AutopilotTurnPlanStep {
                step: "Do A".to_string(),
                status: "pending".to_string(),
            }],
            1_700_000_100,
            false,
        );
        assert_eq!(
            chat.active_plan_artifact()
                .map(|artifact| artifact.source_turn_id.as_str()),
            Some("turn-a")
        );

        let selected_index = chat
            .threads
            .iter()
            .position(|thread_id| thread_id == "thread-b")
            .expect("thread-b should exist");
        let _ = chat.select_thread_by_index(selected_index);
        assert!(chat.active_plan_artifact().is_none());

        chat.set_plan_artifact(
            "thread-b",
            "turn-b",
            Some("Plan B".to_string()),
            vec![AutopilotTurnPlanStep {
                step: "Do B".to_string(),
                status: "inProgress".to_string(),
            }],
            1_700_000_300,
            false,
        );
        assert_eq!(
            chat.active_plan_artifact()
                .map(|artifact| artifact.source_turn_id.as_str()),
            Some("turn-b")
        );
        assert_eq!(chat.turn_plan_explanation.as_deref(), Some("Plan B"));
    }

    #[test]
    fn chat_state_restores_plan_artifact_from_thread_read_text() {
        let mut chat = AutopilotChatState::default();
        chat.set_thread_entries(vec![super::AutopilotThreadListEntry {
            thread_id: "thread-a".to_string(),
            thread_name: Some("Alpha".to_string()),
            preview: "first preview".to_string(),
            status: Some("idle".to_string()),
            loaded: false,
            cwd: Some("/tmp/a".to_string()),
            path: Some("/tmp/a.jsonl".to_string()),
            created_at: 1_700_000_000,
            updated_at: 1_700_000_100,
        }]);

        chat.restore_plan_artifact_from_text(
            "thread-a",
            "turn-a",
            "Plan the rollout.\n\n- [ ] add tests\n- [x] update docs",
            1_700_000_100,
        );

        let artifact = chat.active_plan_artifact().expect("artifact");
        assert_eq!(artifact.explanation.as_deref(), Some("Plan the rollout."));
        assert_eq!(artifact.steps.len(), 2);
        assert_eq!(artifact.steps[0].status, "pending");
        assert_eq!(artifact.steps[1].status, "completed");
        assert!(artifact.restored_from_thread_read);
        assert_eq!(artifact.workspace_cwd.as_deref(), Some("/tmp/a"));
        assert_eq!(artifact.workspace_root.as_deref(), Some("/tmp/a"));
        assert_eq!(artifact.project_name.as_deref(), Some("a"));
    }

    #[test]
    fn chat_state_tracks_diff_artifacts_per_thread() {
        let projection_path = unique_codex_artifact_projection_path("diff-track");
        let mut chat =
            AutopilotChatState::from_artifact_projection_path_for_tests(projection_path.clone());
        chat.set_thread_entries(vec![
            super::AutopilotThreadListEntry {
                thread_id: "thread-a".to_string(),
                thread_name: Some("Alpha".to_string()),
                preview: "first preview".to_string(),
                status: Some("idle".to_string()),
                loaded: false,
                cwd: Some("/tmp/a".to_string()),
                path: Some("/tmp/a.jsonl".to_string()),
                created_at: 1_700_000_000,
                updated_at: 1_700_000_100,
            },
            super::AutopilotThreadListEntry {
                thread_id: "thread-b".to_string(),
                thread_name: Some("Beta".to_string()),
                preview: "second preview".to_string(),
                status: Some("idle".to_string()),
                loaded: false,
                cwd: Some("/tmp/b".to_string()),
                path: Some("/tmp/b.jsonl".to_string()),
                created_at: 1_700_000_200,
                updated_at: 1_700_000_300,
            },
        ]);

        chat.set_diff_artifact(
            "thread-a",
            "turn-a",
            "diff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n@@\n-old\n+new\n"
                .to_string(),
            1_700_000_100,
        );
        assert_eq!(
            chat.active_diff_artifact()
                .map(|artifact| artifact.source_turn_id.as_str()),
            Some("turn-a")
        );
        assert!(
            chat.turn_diff
                .as_deref()
                .is_some_and(|diff| diff.contains("+new"))
        );
        let diff = chat.active_diff_artifact().expect("thread-a diff");
        assert_eq!(diff.workspace_root.as_deref(), Some("/tmp/a"));
        assert_eq!(diff.project_name.as_deref(), Some("a"));

        let selected_index = chat
            .threads
            .iter()
            .position(|thread_id| thread_id == "thread-b")
            .expect("thread-b should exist");
        let _ = chat.select_thread_by_index(selected_index);
        assert!(chat.active_diff_artifact().is_none());

        chat.set_diff_artifact(
            "thread-b",
            "turn-b",
            "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@\n-removed\n+added\n"
                .to_string(),
            1_700_000_300,
        );
        assert_eq!(
            chat.active_diff_artifact()
                .map(|artifact| artifact.source_turn_id.as_str()),
            Some("turn-b")
        );

        let _ = std::fs::remove_file(projection_path);
    }

    #[test]
    fn chat_state_persists_codex_review_diff_and_compaction_artifacts() {
        let projection_path = unique_codex_artifact_projection_path("artifact-persist");
        let mut chat =
            AutopilotChatState::from_artifact_projection_path_for_tests(projection_path.clone());
        chat.set_thread_entries(vec![super::AutopilotThreadListEntry {
            thread_id: "thread-a".to_string(),
            thread_name: Some("Alpha".to_string()),
            preview: "first preview".to_string(),
            status: Some("idle".to_string()),
            loaded: false,
            cwd: Some("/tmp/a".to_string()),
            path: Some("/tmp/a.jsonl".to_string()),
            created_at: 1_700_000_000,
            updated_at: 1_700_000_100,
        }]);

        chat.set_diff_artifact(
            "thread-a",
            "turn-diff",
            "diff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n@@\n-old\n+new\n"
                .to_string(),
            1_700_000_100,
        );
        chat.begin_review_artifact(
            "thread-a",
            "turn-review",
            "review-thread-1",
            "detached",
            "uncommitted changes",
            1_700_000_120,
        );
        chat.complete_review_artifact(
            "review-thread-1",
            "turn-review",
            "Looks solid overall.\n- Keep the tests close to the behavior change.",
            1_700_000_140,
            false,
        );
        chat.set_compaction_artifact("thread-a", "turn-compact", 1_700_000_160, false);

        let reloaded =
            AutopilotChatState::from_artifact_projection_path_for_tests(projection_path.clone());
        let reloaded_diff = reloaded
            .thread_diff_artifacts
            .get("thread-a")
            .and_then(|artifacts| artifacts.first())
            .expect("reloaded diff artifact");
        assert_eq!(reloaded_diff.source_turn_id, "turn-diff");
        assert_eq!(reloaded_diff.added_line_count, 1);
        assert_eq!(reloaded_diff.removed_line_count, 1);
        assert_eq!(reloaded_diff.files.len(), 1);
        assert_eq!(reloaded_diff.workspace_root.as_deref(), Some("/tmp/a"));
        assert_eq!(reloaded_diff.project_name.as_deref(), Some("a"));

        let reloaded_review = reloaded
            .thread_review_artifacts
            .get("thread-a")
            .expect("reloaded review artifact");
        assert_eq!(reloaded_review.review_thread_id, "review-thread-1");
        assert_eq!(reloaded_review.delivery, "detached");
        assert_eq!(reloaded_review.status, "completed");
        assert_eq!(
            reloaded_review.summary.as_deref(),
            Some("Looks solid overall.\n- Keep the tests close to the behavior change.")
        );
        assert_eq!(
            reloaded
                .thread_review_artifacts
                .get("review-thread-1")
                .expect("reloaded detached review artifact")
                .source_thread_id,
            "thread-a"
        );
        assert_eq!(
            reloaded.review_thread_source_map.get("review-thread-1"),
            Some(&"thread-a".to_string())
        );
        assert_eq!(
            reloaded
                .thread_compaction_artifacts
                .get("thread-a")
                .expect("reloaded compaction artifact")
                .source_turn_id,
            "turn-compact"
        );

        let _ = std::fs::remove_file(projection_path);
    }

    #[test]
    fn chat_state_tracks_thread_metadata_and_filters() {
        let mut chat = AutopilotChatState::default();
        chat.set_thread_entries(vec![
            super::AutopilotThreadListEntry {
                thread_id: "thread-a".to_string(),
                thread_name: Some("Alpha".to_string()),
                preview: "first preview".to_string(),
                status: Some("idle".to_string()),
                loaded: false,
                cwd: Some("/tmp/a".to_string()),
                path: Some("/tmp/a.jsonl".to_string()),
                created_at: 1_700_000_000,
                updated_at: 1_700_000_100,
            },
            super::AutopilotThreadListEntry {
                thread_id: "thread-b".to_string(),
                thread_name: None,
                preview: "second preview".to_string(),
                status: Some("active:waitingOnApproval".to_string()),
                loaded: false,
                cwd: Some("/tmp/b".to_string()),
                path: None,
                created_at: 1_700_000_200,
                updated_at: 1_700_000_300,
            },
        ]);

        chat.set_thread_loaded_ids(&["thread-b".to_string()]);
        assert_eq!(chat.active_thread_id.as_deref(), Some("thread-a"));
        assert_eq!(chat.thread_label("thread-a"), "Alpha [thread-a]");
        assert_eq!(chat.thread_metadata["thread-a"].loaded, false);
        assert_eq!(chat.thread_metadata["thread-b"].loaded, true);
        assert_eq!(
            chat.thread_metadata["thread-a"].preview.as_deref(),
            Some("first preview")
        );
        assert_eq!(
            chat.thread_metadata["thread-a"].workspace_root.as_deref(),
            Some("/tmp/a")
        );
        assert_eq!(
            chat.thread_metadata["thread-a"].project_name.as_deref(),
            Some("a")
        );
        assert_eq!(
            chat.project_registry
                .get("/tmp/a")
                .map(|project| project.project_name.as_str()),
            Some("a")
        );
        assert_eq!(chat.active_thread_updated_at(), Some(1_700_000_100));

        chat.cycle_thread_filter_archived();
        chat.cycle_thread_filter_sort_key();
        chat.cycle_thread_filter_source_kind();
        chat.cycle_thread_filter_model_provider();
        chat.thread_filter_search_term = "alpha".to_string();
        let params = chat.build_thread_list_params(Some("/workspace".to_string()));
        assert_eq!(params.archived, Some(true));
        assert_eq!(
            params.sort_key,
            Some(codex_client::ThreadSortKey::CreatedAt)
        );
        assert_eq!(
            params.source_kinds,
            Some(vec![codex_client::ThreadSourceKind::AppServer])
        );
        assert_eq!(params.model_providers, Some(vec!["openai".to_string()]));
        assert_eq!(params.search_term.as_deref(), Some("alpha"));
        assert_eq!(params.cwd.as_deref(), Some("/workspace"));
    }

    #[test]
    fn chat_state_detects_git_workspace_identity_and_dirty_status() {
        let repo = init_git_workspace("git-identity");
        std::fs::write(repo.join("README.md"), "hello\nworld\n")
            .expect("modified repo file should write");
        let cwd = repo.join("src").display().to_string();

        let mut chat = AutopilotChatState::default();
        chat.set_thread_entries(vec![super::AutopilotThreadListEntry {
            thread_id: "thread-git".to_string(),
            thread_name: Some("Git thread".to_string()),
            preview: "git preview".to_string(),
            status: Some("idle".to_string()),
            loaded: false,
            cwd: Some(cwd),
            path: None,
            created_at: 1_700_000_000,
            updated_at: 1_700_000_100,
        }]);

        let repo_root = std::fs::canonicalize(&repo)
            .expect("git repo root should canonicalize")
            .display()
            .to_string();
        let metadata = chat
            .thread_metadata
            .get("thread-git")
            .expect("git thread metadata");
        assert_eq!(metadata.workspace_root.as_deref(), Some(repo_root.as_str()));
        assert_eq!(
            metadata.project_name.as_deref(),
            repo.file_name().and_then(|value| value.to_str())
        );
        assert!(
            metadata
                .git_branch
                .as_deref()
                .is_some_and(|value| !value.is_empty())
        );
        assert_eq!(metadata.git_dirty, Some(true));

        let project = chat
            .project_registry
            .get(repo_root.as_str())
            .expect("project registry entry for git workspace");
        assert_eq!(project.workspace_root, repo_root);
        assert_eq!(project.git_dirty, Some(true));
        assert_eq!(project.thread_ids, vec!["thread-git".to_string()]);

        let _ = std::fs::remove_dir_all(repo);
    }

    #[test]
    fn chat_state_tracks_project_defaults_from_active_thread_preferences() {
        let workspace = unique_temp_dir("project-defaults");
        let workspace_root = workspace.display().to_string();
        let mut chat = AutopilotChatState::default();
        chat.set_thread_entries(vec![super::AutopilotThreadListEntry {
            thread_id: "thread-a".to_string(),
            thread_name: Some("Alpha".to_string()),
            preview: "first preview".to_string(),
            status: Some("idle".to_string()),
            loaded: false,
            cwd: Some(workspace_root.clone()),
            path: None,
            created_at: 1_700_000_000,
            updated_at: 1_700_000_100,
        }]);

        chat.apply_thread_session_configuration(
            "thread-a",
            Some("gpt-5.2-codex".to_string()),
            None,
            Some(codex_client::AskForApproval::OnRequest),
            Some(codex_client::SandboxMode::ReadOnly),
            Some(codex_client::ServiceTier::Flex),
            Some("low".to_string()),
        );
        chat.cycle_personality();
        chat.cycle_collaboration_mode();

        let project = chat
            .active_project()
            .expect("project defaults should exist for active thread");
        assert_eq!(project.defaults.model.as_deref(), Some("gpt-5.2-codex"));
        assert_eq!(
            project.defaults.service_tier,
            super::AutopilotChatServiceTier::Flex
        );
        assert_eq!(project.defaults.reasoning_effort.as_deref(), Some("low"));
        assert_eq!(
            project.defaults.approval_policy,
            Some(codex_client::AskForApproval::OnRequest)
        );
        assert_eq!(
            project.defaults.sandbox_mode,
            Some(codex_client::SandboxMode::ReadOnly)
        );
        assert_eq!(
            project.defaults.personality,
            super::AutopilotChatPersonality::Friendly
        );
        assert_eq!(
            project.defaults.collaboration_mode,
            super::AutopilotChatCollaborationMode::Default
        );

        let _ = std::fs::remove_dir_all(workspace);
    }

    #[test]
    fn chat_state_restores_cached_transcript_when_switching_back() {
        let mut chat = AutopilotChatState::default();
        chat.ensure_thread("thread-a".to_string());
        chat.set_active_thread_transcript(
            "thread-a",
            vec![(AutopilotRole::User, "alpha".to_string())],
        );
        chat.remember_thread("thread-b");

        let thread_b_index = chat
            .threads
            .iter()
            .position(|thread_id| thread_id == "thread-b")
            .expect("thread-b should exist");
        chat.select_thread_by_index(thread_b_index)
            .expect("thread-b should select");
        chat.set_active_thread_transcript(
            "thread-b",
            vec![(AutopilotRole::User, "beta".to_string())],
        );

        let thread_a_index = chat
            .threads
            .iter()
            .position(|thread_id| thread_id == "thread-a")
            .expect("thread-a should exist");
        chat.select_thread_by_index(thread_a_index)
            .expect("thread-a should select");

        assert_eq!(chat.active_thread_id.as_deref(), Some("thread-a"));
        assert_eq!(chat.messages.len(), 1);
        assert_eq!(chat.messages[0].content, "alpha");
    }

    #[test]
    fn chat_state_restores_session_preferences_per_thread() {
        let mut chat = AutopilotChatState::default();
        chat.ensure_thread("thread-a".to_string());
        chat.select_or_insert_model("gpt-5.2-codex");
        chat.set_reasoning_effort(Some("high".to_string()));
        chat.cycle_service_tier();
        chat.cycle_personality();
        chat.cycle_collaboration_mode();
        chat.cycle_approval_mode();
        chat.cycle_sandbox_mode();

        let thread_a = chat
            .thread_metadata
            .get("thread-a")
            .cloned()
            .expect("thread-a metadata should exist");
        assert_eq!(thread_a.model.as_deref(), Some("gpt-5.2-codex"));
        assert_eq!(thread_a.service_tier, super::AutopilotChatServiceTier::Fast);
        assert_eq!(thread_a.reasoning_effort.as_deref(), Some("high"));
        assert_eq!(
            thread_a.approval_policy,
            Some(codex_client::AskForApproval::OnFailure)
        );
        assert_eq!(
            thread_a.sandbox_mode,
            Some(codex_client::SandboxMode::WorkspaceWrite)
        );
        assert_eq!(
            thread_a.personality,
            super::AutopilotChatPersonality::Friendly
        );
        assert_eq!(
            thread_a.collaboration_mode,
            super::AutopilotChatCollaborationMode::Default
        );

        chat.ensure_thread("thread-b".to_string());
        chat.apply_thread_session_configuration(
            "thread-b",
            Some("gpt-5.3-codex".to_string()),
            Some("/tmp/thread-b".to_string()),
            Some(codex_client::AskForApproval::OnRequest),
            Some(codex_client::SandboxMode::ReadOnly),
            Some(codex_client::ServiceTier::Flex),
            Some("low".to_string()),
        );
        chat.cycle_personality();
        chat.cycle_collaboration_mode();

        assert_eq!(chat.current_model(), "gpt-5.3-codex");
        assert_eq!(chat.service_tier, super::AutopilotChatServiceTier::Flex);
        assert_eq!(chat.reasoning_effort.as_deref(), Some("low"));
        assert_eq!(chat.approval_mode, codex_client::AskForApproval::OnRequest);
        assert_eq!(chat.sandbox_mode, codex_client::SandboxMode::ReadOnly);
        assert_eq!(chat.personality, super::AutopilotChatPersonality::Pragmatic);
        assert_eq!(
            chat.collaboration_mode,
            super::AutopilotChatCollaborationMode::Plan
        );

        chat.ensure_thread("thread-a".to_string());
        chat.restore_session_preferences_from_thread("thread-a");
        assert_eq!(chat.current_model(), "gpt-5.2-codex");
        assert_eq!(chat.service_tier, super::AutopilotChatServiceTier::Fast);
        assert_eq!(chat.reasoning_effort.as_deref(), Some("high"));
        assert_eq!(chat.approval_mode, codex_client::AskForApproval::OnFailure);
        assert_eq!(chat.sandbox_mode, codex_client::SandboxMode::WorkspaceWrite);
        assert_eq!(chat.personality, super::AutopilotChatPersonality::Friendly);
        assert_eq!(
            chat.collaboration_mode,
            super::AutopilotChatCollaborationMode::Default
        );
    }

    #[test]
    fn chat_state_tracks_composer_drafts_per_thread_and_detached() {
        let mut chat = AutopilotChatState::default();
        chat.record_composer_draft("detached draft".to_string());
        assert_eq!(chat.active_composer_draft(), "detached draft");

        chat.ensure_thread("thread-a".to_string());
        chat.adopt_detached_composer_draft("thread-a");
        assert_eq!(chat.active_composer_draft(), "detached draft");

        chat.record_composer_draft("draft for a".to_string());
        assert_eq!(chat.active_composer_draft(), "draft for a");

        chat.ensure_thread("thread-b".to_string());
        assert_eq!(chat.active_composer_draft(), "");
        chat.record_composer_draft("draft for b".to_string());

        chat.ensure_thread("thread-a".to_string());
        assert_eq!(chat.active_composer_draft(), "draft for a");

        chat.active_thread_id = None;
        assert_eq!(chat.active_composer_draft(), "detached draft");
    }

    #[test]
    fn chat_state_tracks_submission_history_and_pending_steers() {
        let mut chat = AutopilotChatState::default();
        chat.ensure_thread("thread-a".to_string());
        chat.remember_submission_draft("thread-a", "first draft".to_string());
        chat.remember_submission_draft("thread-a", "second draft".to_string());
        assert_eq!(chat.last_submission_draft("thread-a"), Some("second draft"));

        chat.enqueue_pending_steer_submission(11, "thread-a", "continue".to_string());
        assert_eq!(
            chat.take_pending_steer_submission(11),
            Some(("thread-a".to_string(), "continue".to_string()))
        );
        assert_eq!(chat.take_pending_steer_submission(11), None);
    }

    #[test]
    fn chat_state_tracks_terminal_sessions_per_thread() {
        let mut chat = AutopilotChatState::default();
        chat.ensure_thread("thread-a".to_string());
        chat.prepare_terminal_session(
            "thread-a",
            "/tmp/project-a".to_string(),
            "/bin/zsh".to_string(),
            120,
            32,
        );
        chat.record_terminal_session_opened(
            "thread-a",
            "/tmp/project-a".to_string(),
            "/bin/zsh".to_string(),
            4242,
            120,
            32,
        );
        chat.append_terminal_session_output("thread-a", TerminalStream::Stdout, "cargo test");
        chat.append_terminal_session_output("thread-a", TerminalStream::Stderr, "warning");

        let session = chat.active_terminal_session().expect("terminal session");
        assert_eq!(session.status, AutopilotTerminalSessionStatus::Running);
        assert_eq!(session.pid, Some(4242));
        assert_eq!(session.lines.len(), 2);

        chat.record_terminal_session_closed(
            "thread-a",
            Some(0),
            Some("shell exited from /tmp/project-a with status 0".to_string()),
        );
        let session = chat
            .active_terminal_session()
            .expect("closed terminal session");
        assert_eq!(session.status, AutopilotTerminalSessionStatus::Failed);
        assert!(session.last_error.is_some());
    }

    #[test]
    fn chat_state_cleans_inactive_terminal_sessions() {
        let mut chat = AutopilotChatState::default();
        chat.prepare_terminal_session(
            "thread-a",
            "/tmp/project-a".to_string(),
            "/bin/zsh".to_string(),
            120,
            32,
        );
        chat.record_terminal_session_opened(
            "thread-a",
            "/tmp/project-a".to_string(),
            "/bin/zsh".to_string(),
            1,
            120,
            32,
        );
        chat.prepare_terminal_session(
            "thread-b",
            "/tmp/project-b".to_string(),
            "/bin/zsh".to_string(),
            120,
            32,
        );
        chat.record_terminal_session_failure("thread-b", "failed to start".to_string());

        assert_eq!(chat.remove_inactive_terminal_sessions(), 1);
        assert!(chat.terminal_session_for_thread("thread-a").is_some());
        assert!(chat.terminal_session_for_thread("thread-b").is_none());
    }

    #[test]
    fn chat_state_submit_steer_prompt_adds_user_message_without_assistant_placeholder() {
        let mut chat = AutopilotChatState::default();
        chat.ensure_thread("thread-a".to_string());
        chat.submit_prompt("initial".to_string());
        let pending_before = chat.pending_assistant_message_ids.len();

        chat.submit_steer_prompt("follow up".to_string());

        assert_eq!(
            chat.messages.last().map(|message| message.content.as_str()),
            Some("follow up")
        );
        assert_eq!(chat.pending_assistant_message_ids.len(), pending_before);
    }

    #[test]
    fn chat_state_browses_managed_chat_projection_channels_read_only() {
        fn repeated_hex(ch: char, len: usize) -> String {
            std::iter::repeat_n(ch, len).collect()
        }

        fn signed_event(
            id_ch: char,
            pubkey_ch: char,
            created_at: u64,
            kind: u16,
            tags: Vec<Vec<String>>,
            content: String,
        ) -> nostr::Event {
            nostr::Event {
                id: repeated_hex(id_ch, 64),
                pubkey: repeated_hex(pubkey_ch, 64),
                created_at,
                kind,
                tags,
                content,
                sig: repeated_hex('f', 128),
            }
        }

        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("managed-chat.json");
        let mut chat = AutopilotChatState::default();
        chat.managed_chat_projection =
            super::ManagedChatProjectionState::from_projection_path_for_tests(path);

        let group_metadata = nostr::GroupMetadataEvent::new(
            "oa-main",
            nostr::GroupMetadata::new().with_name("Ops"),
            10,
        )
        .expect("group metadata");
        let channel_alpha = nostr::ManagedChannelCreateEvent::new(
            "oa-main",
            nostr::ChannelMetadata::new("alpha", "", ""),
            20,
        )
        .expect("channel alpha")
        .with_hints(
            nostr::ManagedChannelHints::new()
                .with_channel_type(nostr::ManagedChannelType::Ops)
                .with_position(1),
        )
        .expect("alpha hints");
        let channel_beta = nostr::ManagedChannelCreateEvent::new(
            "oa-main",
            nostr::ChannelMetadata::new("beta", "", ""),
            21,
        )
        .expect("channel beta")
        .with_hints(
            nostr::ManagedChannelHints::new()
                .with_channel_type(nostr::ManagedChannelType::Ops)
                .with_position(2),
        )
        .expect("beta hints");
        let beta_message = nostr::ManagedChannelMessageEvent::new(
            "oa-main",
            repeated_hex('c', 64),
            "wss://relay.openagents.test",
            "history",
            30,
        )
        .expect("beta message");

        chat.managed_chat_projection.record_relay_events(vec![
            signed_event('a', '1', 10, 39000, group_metadata.to_tags(), String::new()),
            signed_event(
                'b',
                '2',
                20,
                40,
                channel_alpha.to_tags().expect("alpha tags"),
                channel_alpha.content().expect("alpha content"),
            ),
            signed_event(
                'c',
                '3',
                21,
                40,
                channel_beta.to_tags().expect("beta tags"),
                channel_beta.content().expect("beta content"),
            ),
            signed_event(
                'd',
                '4',
                30,
                42,
                beta_message.to_tags().expect("beta message tags"),
                "history".to_string(),
            ),
        ]);

        assert!(chat.has_managed_chat_browseable_content());
        assert_eq!(
            chat.active_managed_chat_group()
                .map(|group| group.group_id.as_str()),
            Some("oa-main")
        );
        assert_eq!(
            chat.active_managed_chat_channel()
                .map(|channel| channel.metadata.name.as_str()),
            Some("alpha")
        );

        assert!(chat.select_managed_chat_channel_by_index(1));
        assert_eq!(
            chat.active_managed_chat_channel()
                .map(|channel| channel.metadata.name.as_str()),
            Some("beta")
        );
        assert_eq!(chat.active_managed_chat_messages().len(), 1);
    }

    #[test]
    fn chat_state_auto_selects_default_nip28_channel_once_content_exists() {
        fn repeated_hex(ch: char, len: usize) -> String {
            std::iter::repeat_n(ch, len).collect()
        }

        fn signed_event(
            id_ch: char,
            pubkey_ch: char,
            created_at: u64,
            kind: u16,
            tags: Vec<Vec<String>>,
            content: String,
        ) -> nostr::Event {
            nostr::Event {
                id: repeated_hex(id_ch, 64),
                pubkey: repeated_hex(pubkey_ch, 64),
                created_at,
                kind,
                tags,
                content,
                sig: repeated_hex('f', 128),
            }
        }

        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("managed-chat.json");
        let mut chat = AutopilotChatState::default();
        chat.managed_chat_projection =
            super::ManagedChatProjectionState::from_projection_path_for_tests(path);

        let group_metadata = nostr::GroupMetadataEvent::new(
            "oa-main",
            nostr::GroupMetadata::new().with_name("Ops"),
            10,
        )
        .expect("group metadata");
        let channel = nostr::ManagedChannelCreateEvent::new(
            "oa-main",
            nostr::ChannelMetadata::new("alpha", "", ""),
            20,
        )
        .expect("channel");

        chat.managed_chat_projection.record_relay_events(vec![
            signed_event('a', '1', 10, 39000, group_metadata.to_tags(), String::new()),
            signed_event(
                'b',
                '2',
                20,
                40,
                channel.to_tags().expect("channel tags"),
                channel.content().expect("channel content"),
            ),
        ]);

        assert_eq!(
            chat.selected_workspace,
            super::ChatWorkspaceSelection::Autopilot
        );
        assert!(chat.maybe_auto_select_default_nip28_channel());
        assert_eq!(
            chat.selected_workspace,
            super::ChatWorkspaceSelection::ManagedGroup("oa-main".to_string())
        );
        assert!(!chat.maybe_auto_select_default_nip28_channel());
    }

    #[test]
    fn chat_state_groups_managed_channels_by_category_and_persists_collapse() {
        fn repeated_hex(ch: char, len: usize) -> String {
            std::iter::repeat_n(ch, len).collect()
        }

        fn signed_event(
            id_ch: char,
            pubkey_ch: char,
            created_at: u64,
            kind: u16,
            tags: Vec<Vec<String>>,
            content: String,
        ) -> nostr::Event {
            nostr::Event {
                id: repeated_hex(id_ch, 64),
                pubkey: repeated_hex(pubkey_ch, 64),
                created_at,
                kind,
                tags,
                content,
                sig: repeated_hex('f', 128),
            }
        }

        fn channel_event(
            id_ch: char,
            pubkey_ch: char,
            created_at: u64,
            name: &str,
            category_id: &str,
            category_label: &str,
            position: u32,
        ) -> nostr::Event {
            let channel = nostr::ManagedChannelCreateEvent::new(
                "oa-main",
                nostr::ChannelMetadata::new(name, "", ""),
                created_at,
            )
            .expect("channel")
            .with_hints(
                nostr::ManagedChannelHints::new()
                    .with_channel_type(nostr::ManagedChannelType::Ops)
                    .with_category_id(category_id)
                    .with_category_label(category_label)
                    .with_position(position),
            )
            .expect("channel hints");
            signed_event(
                id_ch,
                pubkey_ch,
                created_at,
                40,
                channel.to_tags().expect("channel tags"),
                channel.content().expect("channel content"),
            )
        }

        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("managed-chat.json");
        let mut chat = AutopilotChatState::default();
        chat.managed_chat_projection =
            super::ManagedChatProjectionState::from_projection_path_for_tests(path);

        let group_metadata = nostr::GroupMetadataEvent::new(
            "oa-main",
            nostr::GroupMetadata::new().with_name("Ops"),
            10,
        )
        .expect("group metadata");
        chat.managed_chat_projection.record_relay_events(vec![
            signed_event('a', '1', 10, 39000, group_metadata.to_tags(), String::new()),
            channel_event('b', '2', 20, "ops-alpha", "ops", "Operations", 1),
            channel_event('c', '3', 21, "ops-beta", "ops", "Operations", 2),
            channel_event('d', '4', 22, "dev-alpha", "dev", "Development", 1),
        ]);

        let rows = chat.active_managed_chat_channel_rail_rows();
        assert_eq!(rows.len(), 5);
        let ops_row_index = rows
            .iter()
            .position(|row| {
                matches!(
                    row,
                    super::ManagedChatChannelRailRow::Category { category_id, .. }
                        if category_id == "ops"
                )
            })
            .expect("ops category row");

        assert!(chat.toggle_managed_chat_category_by_row_index(ops_row_index));
        let collapsed_rows = chat.active_managed_chat_channel_rail_rows();
        assert_eq!(collapsed_rows.len(), 3);
        assert!(matches!(
            collapsed_rows.iter().find(|row| matches!(
                row,
                super::ManagedChatChannelRailRow::Category { category_id, .. }
                    if category_id == "ops"
            )),
            Some(super::ManagedChatChannelRailRow::Category {
                category_id,
                collapsed,
                ..
            }) if category_id == "ops" && *collapsed
        ));
    }

    #[test]
    fn chat_state_browses_direct_message_rooms_and_switches_workspace() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("direct-messages.json");
        let mut chat = AutopilotChatState::default();
        chat.direct_message_projection =
            super::DirectMessageProjectionState::from_projection_path_for_tests(path);
        chat.direct_message_projection
            .queue_outbound_message(super::DirectMessageOutboundMessage {
                room_id: super::direct_message_room_id(None, &["11".repeat(32), "33".repeat(32)]),
                message_id: "44".repeat(32),
                author_pubkey: "11".repeat(32),
                participant_pubkeys: vec!["11".repeat(32), "33".repeat(32)],
                recipient_pubkeys: vec!["33".repeat(32)],
                recipient_relay_hints: std::collections::BTreeMap::from([(
                    "33".repeat(32),
                    vec!["wss://relay.example".to_string()],
                )]),
                content: "hello".to_string(),
                created_at: 50,
                reply_to_event_id: None,
                subject: None,
                wrapped_events: Vec::new(),
                delivery_state: super::ManagedChatDeliveryState::Publishing,
                attempt_count: 1,
                last_error: None,
            })
            .expect("queue direct outbound");

        assert!(chat.has_direct_message_browseable_content());
        assert_eq!(chat.chat_workspace_entries().len(), 1);
        assert!(chat.select_chat_workspace_by_index(0));
        assert_eq!(
            chat.chat_browse_mode(),
            super::ChatBrowseMode::DirectMessages
        );
        assert!(chat.select_direct_message_room_by_index(0));
        assert_eq!(chat.active_direct_message_messages().len(), 1);
        assert_eq!(
            chat.active_direct_message_room()
                .map(|room| room.participant_pubkeys.len()),
            Some(2)
        );
    }

    #[test]
    fn job_inbox_upsert_collapses_duplicate_request_ids() {
        let inbox = seed_job_inbox(vec![
            fixture_inbox_request("req-dup", "cap.one", 11, 60, JobInboxValidation::Pending),
            fixture_inbox_request("req-dup", "cap.one", 22, 120, JobInboxValidation::Valid),
        ]);

        let duplicates = inbox
            .requests
            .iter()
            .filter(|request| request.request_id == "req-dup")
            .count();
        assert_eq!(duplicates, 1);
        let request = inbox
            .requests
            .iter()
            .find(|request| request.request_id == "req-dup")
            .expect("request should exist");
        assert_eq!(request.price_sats, 22);
        assert_eq!(request.ttl_seconds, 120);
    }

    #[test]
    fn job_inbox_accept_updates_selected_request_decision() {
        let mut inbox = seed_job_inbox(vec![fixture_inbox_request(
            "req-accept",
            "summarize.text",
            900,
            120,
            JobInboxValidation::Valid,
        )]);
        assert!(inbox.select_by_index(0));
        let request_id = inbox
            .selected_request()
            .expect("selection should exist")
            .request_id
            .clone();

        let decided = inbox
            .decide_selected(true, "valid + priced")
            .expect("decision should succeed");
        assert_eq!(decided, request_id);
        let selected = inbox.selected_request().expect("selected request remains");
        assert!(matches!(
            selected.decision,
            JobInboxDecision::Accepted { ref reason } if reason == "valid + priced"
        ));
    }

    #[test]
    fn active_job_advance_stage_updates_lifecycle() {
        let mut inbox = seed_job_inbox(vec![fixture_inbox_request(
            "req-active",
            "summarize.text",
            1500,
            300,
            JobInboxValidation::Valid,
        )]);
        assert!(inbox.select_by_index(0));
        let request = inbox
            .selected_request()
            .expect("request should exist")
            .clone();

        let mut active = ActiveJobState::default();
        active.start_from_request(&request);
        let stage = active.advance_stage().expect("advance should succeed");
        assert_eq!(stage, JobLifecycleStage::Running);
        let stage = active
            .advance_stage()
            .expect("second advance should succeed");
        assert_eq!(stage, JobLifecycleStage::Delivered);
        let current = active.job.as_ref().expect("active job exists");
        assert_eq!(current.stage, JobLifecycleStage::Delivered);
        assert!(current.invoice_id.is_none());
        assert!(current.events.iter().any(|event| {
            event
                .message
                .contains("running (authority=req-event:req-active)")
        }));
        assert!(current.events.iter().any(|event| {
            event
                .message
                .contains("delivered (authority=result-event:req-active)")
        }));
    }

    #[test]
    fn active_job_inflight_count_drops_to_zero_for_terminal_stages() {
        let mut inbox = seed_job_inbox(vec![fixture_inbox_request(
            "req-inflight",
            "summarize.text",
            1500,
            300,
            JobInboxValidation::Valid,
        )]);
        assert!(inbox.select_by_index(0));
        let request = inbox
            .selected_request()
            .expect("request should exist")
            .clone();

        let mut active = ActiveJobState::default();
        active.start_from_request(&request);
        assert_eq!(active.inflight_job_count(), 1);
        active
            .advance_stage()
            .expect("accepted->running should succeed");
        assert_eq!(active.inflight_job_count(), 1);
        active
            .advance_stage()
            .expect("running->delivered should succeed");
        active.job.as_mut().expect("active job exists").payment_id =
            Some("wallet:payment:req-inflight".to_string());
        active
            .advance_stage()
            .expect("delivered->paid should succeed");
        assert_eq!(active.inflight_job_count(), 0);
    }

    #[test]
    fn active_job_start_copies_execution_input_and_ttl() {
        let mut inbox = seed_job_inbox(vec![fixture_inbox_request(
            "req-exec",
            "summarize.text",
            1500,
            91,
            JobInboxValidation::Valid,
        )]);
        assert!(inbox.select_by_index(0));
        let request = inbox
            .selected_request()
            .expect("request should exist")
            .clone();

        let mut active = ActiveJobState::default();
        active.start_from_request(&request);
        let job = active.job.as_ref().expect("active job should exist");
        assert_eq!(job.ttl_seconds, 91);
        assert_eq!(
            job.execution_input.as_deref(),
            Some("Execute capability `summarize.text` for request `req-exec`.")
        );
        assert!(active.execution_thread_id.is_none());
        assert!(!active.runtime_supports_abort);
    }

    #[test]
    fn job_history_rejects_unconfirmed_success_settlement_from_active_job() {
        let mut inbox = seed_job_inbox(vec![fixture_inbox_request(
            "req-unconfirmed",
            "summarize.text",
            1500,
            300,
            JobInboxValidation::Valid,
        )]);
        assert!(inbox.select_by_index(0));
        let request = inbox
            .selected_request()
            .expect("request should exist")
            .clone();
        let mut active = ActiveJobState::default();
        active.start_from_request(&request);
        active
            .advance_stage()
            .expect("accepted->running should succeed");
        active
            .advance_stage()
            .expect("running->delivered should succeed");
        let paid_transition = active.advance_stage();
        assert!(paid_transition.is_err());
        assert!(
            paid_transition
                .err()
                .as_deref()
                .is_some_and(|error| error.contains("payment pointer"))
        );
        let job = active.job.as_ref().expect("active job exists");
        assert!(job.payment_id.is_none());
        assert_eq!(job.stage, JobLifecycleStage::Delivered);

        let mut history = JobHistoryState::default();
        history.record_from_active_job(job, JobHistoryStatus::Succeeded);
        let row = history
            .rows
            .first()
            .expect("history row should be recorded");
        assert_eq!(row.status, JobHistoryStatus::Failed);
        assert_eq!(row.payout_sats, 0);
        assert!(
            row.failure_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("not wallet-confirmed"))
        );
    }

    #[test]
    fn active_job_rejects_running_without_authoritative_request_event() {
        let mut request = fixture_inbox_request(
            "req-missing-running-authority",
            "summarize.text",
            500,
            90,
            JobInboxValidation::Valid,
        );
        request.sa_tick_request_event_id = None;

        let mut active = ActiveJobState::default();
        let mut inbox = seed_job_inbox(vec![request]);
        assert!(inbox.select_by_index(0));
        let selected = inbox
            .selected_request()
            .expect("request should exist")
            .clone();
        active.start_from_request(&selected);

        let outcome = active.advance_stage();
        assert!(outcome.is_err());
        assert!(
            outcome
                .err()
                .as_deref()
                .is_some_and(|error| error.contains("running event"))
        );
        assert_eq!(
            active.job.as_ref().expect("job should exist").stage,
            JobLifecycleStage::Accepted
        );
    }

    #[test]
    fn active_job_rejects_delivered_without_authoritative_result_event() {
        let mut request = fixture_inbox_request(
            "req-missing-delivered-authority",
            "summarize.text",
            500,
            90,
            JobInboxValidation::Valid,
        );
        request.sa_tick_result_event_id = None;

        let mut active = ActiveJobState::default();
        let mut inbox = seed_job_inbox(vec![request]);
        assert!(inbox.select_by_index(0));
        let selected = inbox
            .selected_request()
            .expect("request should exist")
            .clone();
        active.start_from_request(&selected);
        assert_eq!(
            active
                .advance_stage()
                .expect("accepted->running should succeed"),
            JobLifecycleStage::Running
        );

        let outcome = active.advance_stage();
        assert!(outcome.is_err());
        assert!(
            outcome
                .err()
                .as_deref()
                .is_some_and(|error| error.contains("delivered event"))
        );
        assert_eq!(
            active.job.as_ref().expect("job should exist").stage,
            JobLifecycleStage::Running
        );
    }

    #[test]
    fn active_job_rejects_paid_without_wallet_authority() {
        let request = fixture_inbox_request(
            "req-missing-payment-authority",
            "summarize.text",
            500,
            90,
            JobInboxValidation::Valid,
        );
        let mut active = ActiveJobState::default();
        let mut inbox = seed_job_inbox(vec![request]);
        assert!(inbox.select_by_index(0));
        let selected = inbox
            .selected_request()
            .expect("request should exist")
            .clone();
        active.start_from_request(&selected);
        assert_eq!(
            active
                .advance_stage()
                .expect("accepted->running should succeed"),
            JobLifecycleStage::Running
        );
        assert_eq!(
            active
                .advance_stage()
                .expect("running->delivered should succeed"),
            JobLifecycleStage::Delivered
        );

        let outcome = active.advance_stage();
        assert!(outcome.is_err());
        assert!(
            outcome
                .err()
                .as_deref()
                .is_some_and(|error| error.contains("payment pointer"))
        );
        assert_eq!(
            active.job.as_ref().expect("job should exist").stage,
            JobLifecycleStage::Delivered
        );
    }

    #[test]
    fn earn_job_lifecycle_projection_coerces_nonwallet_paid_stage_to_delivered() {
        let request = fixture_inbox_request(
            "req-projection-nonwallet-paid",
            "summarize.text",
            500,
            90,
            JobInboxValidation::Valid,
        );
        let mut active = ActiveJobState::default();
        let mut inbox = seed_job_inbox(vec![request]);
        assert!(inbox.select_by_index(0));
        let selected = inbox
            .selected_request()
            .expect("request should exist")
            .clone();
        active.start_from_request(&selected);
        let job = active.job.as_mut().expect("job should exist");
        job.stage = JobLifecycleStage::Paid;
        job.sa_tick_result_event_id = Some("result-projection-nonwallet-paid".to_string());
        job.ac_settlement_event_id = Some("feedback-projection-nonwallet-paid".to_string());

        let mut projection = EarnJobLifecycleProjectionState::default();
        projection.record_active_job_stage(
            job,
            JobLifecycleStage::Paid,
            1_762_000_000,
            "earn.active_job.invalid_paid_projection",
        );

        let row = projection.rows.first().expect("projection row");
        assert_eq!(row.stage, JobLifecycleStage::Delivered);
        assert_eq!(row.payment_pointer, None);
        assert!(!row.settlement_authoritative);
        assert_eq!(row.settlement_authority, "projection.non_authoritative");
    }

    #[test]
    fn mission_control_earn_loop_wallet_confirmed_end_to_end() {
        let mut provider = ProviderRuntimeState::default();
        provider.mode = ProviderMode::Online;
        provider.online_since = Some(std::time::Instant::now());

        let mut inbox = seed_job_inbox(vec![fixture_inbox_request(
            "req-e2e",
            "run_model",
            50,
            120,
            JobInboxValidation::Valid,
        )]);
        assert!(inbox.select_by_index(0));
        let selected_request_id = inbox
            .decide_selected(true, "capability matched")
            .expect("accept request");
        assert_eq!(selected_request_id, "req-e2e");
        let request = inbox
            .selected_request()
            .expect("selected request exists")
            .clone();

        let mut active = ActiveJobState::default();
        active.start_from_request(&request);
        assert_eq!(
            active.job.as_ref().expect("active job").stage,
            JobLifecycleStage::Accepted
        );
        assert_eq!(
            active
                .advance_stage()
                .expect("accepted->running transition"),
            JobLifecycleStage::Running
        );
        assert_eq!(
            active
                .advance_stage()
                .expect("running->delivered transition"),
            JobLifecycleStage::Delivered
        );
        active.job.as_mut().expect("active job").payment_id =
            Some("wallet-payment-001".to_string());
        assert_eq!(
            active.advance_stage().expect("delivered->paid transition"),
            JobLifecycleStage::Paid
        );
        let terminal_job = active.job.clone().expect("terminal active job");

        let mut history = JobHistoryState::default();
        history.record_from_active_job(&terminal_job, JobHistoryStatus::Succeeded);
        let row = history.rows.first().expect("history row recorded");
        assert_eq!(row.status, JobHistoryStatus::Succeeded);
        assert_eq!(row.payout_sats, 50);
        assert_eq!(row.payment_pointer, "wallet-payment-001");

        let mut wallet = SparkPaneState::default();
        wallet.balance = Some(openagents_spark::Balance {
            spark_sats: 10_000,
            lightning_sats: 0,
            onchain_sats: 0,
        });
        wallet
            .recent_payments
            .push(openagents_spark::PaymentSummary {
                id: "wallet-payment-001".to_string(),
                direction: "receive".to_string(),
                status: "succeeded".to_string(),
                amount_sats: 50,
                timestamp: history.reference_epoch_seconds,
                ..Default::default()
            });

        let swap_receipts = Vec::<crate::state::swap_contract::GoalSwapExecutionReceipt>::new();
        let reconciliation = crate::state::wallet_reconciliation::reconcile_wallet_events_for_goal(
            history.reference_epoch_seconds.saturating_sub(60),
            9_950,
            10_000,
            "goal-e2e",
            &history,
            &wallet,
            &swap_receipts,
        );
        assert_eq!(reconciliation.earned_wallet_delta_sats, 50);
        assert_eq!(reconciliation.unattributed_receive_sats, 0);

        let mut score = EarningsScoreboardState::default();
        score.refresh_from_sources(std::time::Instant::now(), &provider, &history, &wallet);
        assert_eq!(score.load_state, super::PaneLoadState::Ready);
        assert!(score.sats_today >= 50);
        assert_eq!(score.jobs_today, 1);
    }

    #[test]
    fn reciprocal_loop_two_identity_relay_harness_runs_bidirectional_paid_cycles() {
        let pubkey_a = "11".repeat(32);
        let pubkey_b = "22".repeat(32);

        let mut loop_a = ReciprocalLoopState::default();
        loop_a.set_local_pubkey(Some(pubkey_a.as_str()));
        loop_a.set_peer_pubkey(Some(pubkey_b.as_str()));
        loop_a.start().expect("loop A should start");

        let mut loop_b = ReciprocalLoopState::default();
        loop_b.set_local_pubkey(Some(pubkey_b.as_str()));
        loop_b.set_peer_pubkey(Some(pubkey_a.as_str()));
        loop_b.start().expect("loop B should start");

        let mut relay = DeterministicRelayFixture::default();
        let mut submitted_a = Vec::<SubmittedNetworkRequest>::new();
        let mut submitted_b = Vec::<SubmittedNetworkRequest>::new();
        let mut history_a = Vec::<super::JobHistoryReceiptRow>::new();
        let mut history_b = Vec::<super::JobHistoryReceiptRow>::new();
        let mut request_routes = Vec::<(String, char)>::new();
        let cycles = 4u64;
        let mut now_epoch_seconds = 1_762_900_000u64;

        for cycle in 0..cycles {
            let a_ready = loop_a.ready_to_dispatch();
            let b_ready = loop_b.ready_to_dispatch();
            assert_ne!(
                a_ready, b_ready,
                "exactly one loop side must be dispatch-ready per cycle"
            );

            if a_ready {
                let request_id = format!("req-a-to-b-{:02}", cycle + 1);
                request_routes.push((request_id.clone(), 'a'));
                loop_a.register_outbound_dispatch(request_id.as_str(), now_epoch_seconds);
                relay.queue_targeted_request(request_id.as_str(), pubkey_b.as_str());
                assert_eq!(
                    relay.take_next_for(pubkey_b.as_str()).as_deref(),
                    Some(request_id.as_str())
                );
                assert!(
                    relay.take_next_for(pubkey_a.as_str()).is_none(),
                    "targeted relay ingestion should not deliver sender's own request"
                );

                let mut outbound = fixture_loop_submitted_request(
                    request_id.as_str(),
                    NetworkRequestStatus::Paid,
                    pubkey_b.as_str(),
                    loop_a.skill_scope_id.as_str(),
                );
                outbound.last_payment_pointer = Some(format!("wallet:pay:{request_id}"));
                submitted_a.push(outbound);
                assert!(
                    loop_a.reconcile_outbound_terminal_statuses(submitted_a.as_slice()),
                    "outbound paid status should reconcile once for A->B cycle"
                );

                let job_id = format!("job-{request_id}");
                let mut inbound = fixture_history_row(
                    job_id.as_str(),
                    JobHistoryStatus::Succeeded,
                    now_epoch_seconds,
                    10,
                );
                inbound.skill_scope_id = Some(loop_b.skill_scope_id.clone());
                inbound.payment_pointer = format!("wallet:recv:{request_id}");
                history_b.push(inbound);
                assert!(
                    loop_b.reconcile_inbound_history(history_b.as_slice()),
                    "inbound paid receipt should reconcile once for A->B cycle"
                );

                let feedback = relay.publish_feedback(
                    request_id.as_str(),
                    pubkey_b.as_str(),
                    pubkey_a.as_str(),
                );
                let result =
                    relay.publish_result(request_id.as_str(), pubkey_b.as_str(), pubkey_a.as_str());
                assert!(feedback.contains(request_id.as_str()));
                assert!(result.contains(request_id.as_str()));
            } else {
                let request_id = format!("req-b-to-a-{:02}", cycle + 1);
                request_routes.push((request_id.clone(), 'b'));
                loop_b.register_outbound_dispatch(request_id.as_str(), now_epoch_seconds);
                relay.queue_targeted_request(request_id.as_str(), pubkey_a.as_str());
                assert_eq!(
                    relay.take_next_for(pubkey_a.as_str()).as_deref(),
                    Some(request_id.as_str())
                );
                assert!(
                    relay.take_next_for(pubkey_b.as_str()).is_none(),
                    "targeted relay ingestion should not deliver sender's own request"
                );

                let mut outbound = fixture_loop_submitted_request(
                    request_id.as_str(),
                    NetworkRequestStatus::Paid,
                    pubkey_a.as_str(),
                    loop_b.skill_scope_id.as_str(),
                );
                outbound.last_payment_pointer = Some(format!("wallet:pay:{request_id}"));
                submitted_b.push(outbound);
                assert!(
                    loop_b.reconcile_outbound_terminal_statuses(submitted_b.as_slice()),
                    "outbound paid status should reconcile once for B->A cycle"
                );

                let job_id = format!("job-{request_id}");
                let mut inbound = fixture_history_row(
                    job_id.as_str(),
                    JobHistoryStatus::Succeeded,
                    now_epoch_seconds,
                    10,
                );
                inbound.skill_scope_id = Some(loop_a.skill_scope_id.clone());
                inbound.payment_pointer = format!("wallet:recv:{request_id}");
                history_a.push(inbound);
                assert!(
                    loop_a.reconcile_inbound_history(history_a.as_slice()),
                    "inbound paid receipt should reconcile once for B->A cycle"
                );

                let feedback = relay.publish_feedback(
                    request_id.as_str(),
                    pubkey_a.as_str(),
                    pubkey_b.as_str(),
                );
                let result =
                    relay.publish_result(request_id.as_str(), pubkey_a.as_str(), pubkey_b.as_str());
                assert!(feedback.contains(request_id.as_str()));
                assert!(result.contains(request_id.as_str()));
            }
            now_epoch_seconds = now_epoch_seconds.saturating_add(1);
        }

        assert_eq!(relay.feedback_event_ids.len() as u64, cycles);
        assert_eq!(relay.result_event_ids.len() as u64, cycles);
        assert_eq!(loop_a.local_to_peer_paid, 2);
        assert_eq!(loop_a.peer_to_local_paid, 2);
        assert_eq!(loop_b.local_to_peer_paid, 2);
        assert_eq!(loop_b.peer_to_local_paid, 2);
        assert_eq!(loop_a.sats_sent, 20);
        assert_eq!(loop_a.sats_received, 20);
        assert_eq!(loop_b.sats_sent, 20);
        assert_eq!(loop_b.sats_received, 20);

        for (request_id, sender) in request_routes {
            match sender {
                'a' => {
                    let outbound = submitted_a
                        .iter()
                        .find(|request| request.request_id == request_id)
                        .expect("A outbound request should be tracked");
                    assert_eq!(outbound.status, NetworkRequestStatus::Paid);
                    assert!(
                        outbound
                            .last_payment_pointer
                            .as_deref()
                            .is_some_and(|pointer| pointer.starts_with("wallet:pay:")),
                        "A outbound payment must be wallet-authoritative"
                    );
                    let inbound = history_b
                        .iter()
                        .find(|row| row.job_id == format!("job-{request_id}"))
                        .expect("B inbound history row should correlate to outbound request");
                    assert_eq!(inbound.status, JobHistoryStatus::Succeeded);
                    assert!(
                        inbound.payment_pointer.starts_with("wallet:recv:"),
                        "B inbound payout pointer must be wallet-authoritative"
                    );
                }
                'b' => {
                    let outbound = submitted_b
                        .iter()
                        .find(|request| request.request_id == request_id)
                        .expect("B outbound request should be tracked");
                    assert_eq!(outbound.status, NetworkRequestStatus::Paid);
                    assert!(
                        outbound
                            .last_payment_pointer
                            .as_deref()
                            .is_some_and(|pointer| pointer.starts_with("wallet:pay:")),
                        "B outbound payment must be wallet-authoritative"
                    );
                    let inbound = history_a
                        .iter()
                        .find(|row| row.job_id == format!("job-{request_id}"))
                        .expect("A inbound history row should correlate to outbound request");
                    assert_eq!(inbound.status, JobHistoryStatus::Succeeded);
                    assert!(
                        inbound.payment_pointer.starts_with("wallet:recv:"),
                        "A inbound payout pointer must be wallet-authoritative"
                    );
                }
                _ => panic!("unexpected sender tag"),
            }
        }

        let dispatches_before_stop = loop_a.local_to_peer_dispatched;
        loop_a.stop("operator stop test");
        assert!(loop_a.kill_switch_active);
        assert!(!loop_a.ready_to_dispatch());
        if loop_a.ready_to_dispatch() {
            loop_a.register_outbound_dispatch("req-after-stop", now_epoch_seconds);
        }
        assert_eq!(
            loop_a.local_to_peer_dispatched, dispatches_before_stop,
            "stop should halt post-stop dispatches"
        );
        loop_a
            .start()
            .expect("loop should restart after explicit operator start");
        assert!(!loop_a.kill_switch_active);
    }

    #[test]
    fn starter_provenance_propagates_from_inbox_to_history_receipt() {
        let mut inbox = seed_job_inbox(vec![fixture_inbox_request_with_source(
            "req-starter-provenance",
            "starter.quest.dispatch",
            75,
            120,
            JobInboxValidation::Valid,
            JobDemandSource::StarterDemand,
        )]);
        assert!(inbox.select_by_index(0));
        let request = inbox
            .selected_request()
            .expect("starter request should exist")
            .clone();
        assert_eq!(request.demand_source, JobDemandSource::StarterDemand);

        let mut active = ActiveJobState::default();
        active.start_from_request(&request);
        assert_eq!(
            active
                .job
                .as_ref()
                .expect("active job should start")
                .demand_source,
            JobDemandSource::StarterDemand
        );
        assert_eq!(
            active
                .advance_stage()
                .expect("accepted->running should succeed"),
            JobLifecycleStage::Running
        );
        assert_eq!(
            active
                .advance_stage()
                .expect("running->delivered should succeed"),
            JobLifecycleStage::Delivered
        );
        active.job.as_mut().expect("active job").payment_id =
            Some("wallet-payment-starter-provenance".to_string());
        assert_eq!(
            active
                .advance_stage()
                .expect("delivered->paid should succeed"),
            JobLifecycleStage::Paid
        );

        let terminal = active.job.clone().expect("terminal job should exist");
        let mut history = JobHistoryState::default();
        history.record_from_active_job(&terminal, JobHistoryStatus::Succeeded);
        let row = history.rows.first().expect("history row should exist");
        assert_eq!(row.demand_source, JobDemandSource::StarterDemand);
    }

    #[test]
    fn job_history_filters_search_status_and_time() {
        let mut history = seed_job_history(vec![
            fixture_history_row(
                "job-bootstrap-000",
                JobHistoryStatus::Succeeded,
                1_761_919_970,
                2100,
            ),
            fixture_history_row(
                "job-bootstrap-001",
                JobHistoryStatus::Failed,
                1_761_919_940,
                0,
            ),
        ]);
        history.status_filter = JobHistoryStatusFilter::Succeeded;
        history.time_range = JobHistoryTimeRange::All;
        history.set_search_job_id("bootstrap-000".to_string());

        let rows = history.paged_rows();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, JobHistoryStatus::Succeeded);
        assert!(rows[0].job_id.contains("bootstrap-000"));
    }

    #[test]
    fn job_history_upsert_keeps_single_row_per_job_id() {
        let mut history = seed_job_history(vec![fixture_history_row(
            "job-bootstrap-000",
            JobHistoryStatus::Succeeded,
            1_761_920_000,
            1200,
        )]);
        let before = history.rows.len();
        history.upsert_row(super::JobHistoryReceiptRow {
            job_id: "job-bootstrap-000".to_string(),
            status: JobHistoryStatus::Failed,
            demand_source: JobDemandSource::OpenNetwork,
            completed_at_epoch_seconds: history.reference_epoch_seconds + 10,
            skill_scope_id: None,
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_result_event_id: None,
            sa_trajectory_session_id: None,
            ac_envelope_event_id: None,
            ac_settlement_event_id: None,
            ac_default_event_id: None,
            delivery_proof_id: None,
            delivery_metering_rule_id: None,
            delivery_proof_status_label: None,
            delivery_metered_quantity: None,
            delivery_accepted_quantity: None,
            delivery_variance_reason_label: None,
            delivery_rejection_reason_label: None,
            payout_sats: 0,
            result_hash: "sha256:updated".to_string(),
            payment_pointer: "pay:updated".to_string(),
            failure_reason: Some("updated".to_string()),
            execution_provenance: None,
        });

        assert_eq!(history.rows.len(), before);
        let row = history
            .rows
            .iter()
            .find(|row| row.job_id == "job-bootstrap-000")
            .expect("row should exist");
        assert_eq!(row.result_hash, "sha256:updated");
    }

    #[test]
    fn relay_connections_add_retry_remove_flow() {
        let mut relays = RelayConnectionsState::default();
        assert!(relays.add_relay("wss://relay.new.example").is_ok());
        assert_eq!(
            relays.selected().map(|row| row.url.as_str()),
            Some("wss://relay.new.example")
        );

        assert!(relays.retry_selected().is_ok());
        assert_eq!(
            relays.selected().map(|row| row.status),
            Some(RelayConnectionStatus::Connecting)
        );

        assert!(relays.remove_selected().is_ok());
        assert!(
            relays
                .relays
                .iter()
                .all(|row| row.url != "wss://relay.new.example")
        );
    }

    #[test]
    fn sync_health_detects_stale_cursor_and_rebootstrap() {
        let worker_id = "desktopw:test:sync";
        let mut lifecycle = crate::sync_lifecycle::RuntimeSyncLifecycleManager::default();
        lifecycle.mark_connecting(worker_id);
        lifecycle.mark_replay_bootstrap(worker_id, 42, Some(42));
        lifecycle.mark_live(worker_id, Some(120));
        let mut sync = SyncHealthState::default();
        sync.last_applied_event_seq = 42;
        let snapshot = lifecycle
            .snapshot(worker_id)
            .expect("snapshot should exist");

        let now = std::time::Instant::now();
        sync.refresh_from_lifecycle(now, Some(&snapshot));
        sync.refresh_from_lifecycle(
            now + std::time::Duration::from_secs(sync.cursor_stale_after_seconds + 5),
            Some(&snapshot),
        );
        assert_eq!(sync.load_state, super::PaneLoadState::Error);
        assert_eq!(sync.recovery_phase, SyncRecoveryPhase::Reconnecting);
        assert!(sync.stale_cursor_reason.is_some());

        sync.rebootstrap();
        assert_eq!(sync.load_state, super::PaneLoadState::Ready);
        assert_eq!(sync.recovery_phase, SyncRecoveryPhase::Replaying);
        assert_eq!(sync.cursor_last_advanced_seconds_ago, 0);
    }

    #[test]
    fn sync_health_stays_idle_without_lifecycle_snapshot() {
        let mut sync = SyncHealthState::default();
        sync.refresh_from_lifecycle(std::time::Instant::now(), None);
        assert_eq!(sync.load_state, super::PaneLoadState::Loading);
        assert_eq!(sync.recovery_phase, SyncRecoveryPhase::Idle);
        assert_eq!(sync.last_error, None);
        assert_eq!(sync.cursor_last_advanced_seconds_ago, 0);
    }

    #[test]
    fn sync_health_marks_resubscribing_when_lifecycle_enters_backoff() {
        let worker_id = "desktopw:test:backoff";
        let mut lifecycle = crate::sync_lifecycle::RuntimeSyncLifecycleManager::default();
        lifecycle.mark_connecting(worker_id);
        lifecycle.mark_live(worker_id, Some(30));
        let _ = lifecycle.mark_disconnect(
            worker_id,
            crate::sync_lifecycle::RuntimeSyncDisconnectReason::Network,
            Some("relay dropped connection".to_string()),
        );
        let snapshot = lifecycle
            .snapshot(worker_id)
            .expect("snapshot should exist");
        let mut sync = SyncHealthState::default();

        sync.refresh_from_lifecycle(std::time::Instant::now(), Some(&snapshot));
        assert_eq!(sync.subscription_state, "resubscribing");
        assert_eq!(sync.recovery_phase, SyncRecoveryPhase::Reconnecting);
        assert_eq!(sync.disconnect_reason.as_deref(), Some("network"));
        assert!(
            sync.reconnect_posture.starts_with("backoff"),
            "reconnect posture should expose backoff details"
        );
    }

    #[test]
    fn network_requests_submit_validates_and_records_stream_link() {
        let mut requests = NetworkRequestsState::default();
        let request_id = requests
            .queue_request_submission(NetworkRequestSubmission {
                request_id: None,
                request_type: "translate.text".to_string(),
                payload: "{\"text\":\"hola\"}".to_string(),
                resolution_mode: BuyerResolutionMode::Race,
                target_provider_pubkeys: vec!["npub1provider".to_string()],
                skill_scope_id: Some("33400:npub1agent:summarize-text:0.1.0".to_string()),
                credit_envelope_ref: Some("ac:39242:00000001".to_string()),
                budget_sats: 1200,
                timeout_seconds: 90,
                authority_command_seq: 44,
            })
            .expect("request should be accepted");
        let first = requests
            .submitted
            .first()
            .expect("new request should be inserted at head");
        assert_eq!(first.request_id, request_id);
        assert_eq!(first.response_stream_id, format!("stream:{request_id}"));
        assert_eq!(first.status, NetworkRequestStatus::Submitted);
        assert_eq!(first.authority_command_seq, 44);
    }

    #[test]
    fn network_requests_track_buyer_feedback_and_result_correlation() {
        let mut requests = NetworkRequestsState::default();
        let request_id = requests
            .queue_request_submission(NetworkRequestSubmission {
                request_id: Some("req-001".to_string()),
                request_type: "summarize.text".to_string(),
                payload: "{\"prompt\":\"hello\"}".to_string(),
                resolution_mode: BuyerResolutionMode::Race,
                target_provider_pubkeys: Vec::new(),
                skill_scope_id: None,
                credit_envelope_ref: None,
                budget_sats: 10,
                timeout_seconds: 60,
                authority_command_seq: 9,
            })
            .expect("request should queue");
        let provider_pubkey = "22".repeat(32);
        let feedback_action = requests.apply_nip90_buyer_feedback_event(
            request_id.as_str(),
            provider_pubkey.as_str(),
            "feedback-001",
            Some("payment-required"),
            Some("pay invoice"),
            Some(10_000),
            Some("lnbc1feedback001"),
        );
        assert_eq!(feedback_action, None);
        let after_feedback = requests
            .submitted
            .iter()
            .find(|request| request.request_id == request_id)
            .expect("request should exist after feedback");
        assert_eq!(after_feedback.status, NetworkRequestStatus::PaymentRequired);
        assert_eq!(
            after_feedback.last_feedback_status.as_deref(),
            Some("payment-required")
        );
        assert_eq!(
            after_feedback.last_feedback_event_id.as_deref(),
            Some("feedback-001")
        );
        assert_eq!(
            after_feedback.last_provider_pubkey.as_deref(),
            Some(provider_pubkey.as_str())
        );

        let result_action = requests.apply_nip90_buyer_result_event(
            request_id.as_str(),
            provider_pubkey.as_str(),
            "result-001",
            Some("success"),
        );
        assert_eq!(result_action, None);
        let after_result = requests
            .submitted
            .iter()
            .find(|request| request.request_id == request_id)
            .expect("request should exist after result");
        assert_eq!(after_result.status, NetworkRequestStatus::ResultReceived);
        assert_eq!(
            after_result.last_result_event_id.as_deref(),
            Some("result-001")
        );
        assert_eq!(
            after_result.winning_provider_pubkey.as_deref(),
            Some(provider_pubkey.as_str())
        );
        assert_eq!(
            after_result.winning_result_event_id.as_deref(),
            Some("result-001")
        );
        assert_eq!(
            after_result.resolution_reason_code.as_deref(),
            Some(BuyerResolutionReason::FirstValidResult.code())
        );
    }

    #[test]
    fn network_requests_ignore_feedback_from_untargeted_provider() {
        let mut requests = NetworkRequestsState::default();
        let target_provider = "11".repeat(32);
        let untargeted_provider = "22".repeat(32);
        let request_id = requests
            .queue_request_submission(NetworkRequestSubmission {
                request_id: Some("req-targeted-feedback".to_string()),
                request_type: "summarize.text".to_string(),
                payload: "{\"prompt\":\"hello\"}".to_string(),
                resolution_mode: BuyerResolutionMode::Race,
                target_provider_pubkeys: vec![target_provider.clone()],
                skill_scope_id: None,
                credit_envelope_ref: None,
                budget_sats: 10,
                timeout_seconds: 60,
                authority_command_seq: 10,
            })
            .expect("request should queue");

        let action = requests.apply_nip90_buyer_feedback_event(
            request_id.as_str(),
            untargeted_provider.as_str(),
            "feedback-untargeted",
            Some("payment-required"),
            Some("invoice ready"),
            Some(10_000),
            Some("lnbc1untargeted"),
        );
        assert_eq!(action, None);

        let request = requests
            .submitted
            .iter()
            .find(|request| request.request_id == request_id)
            .expect("request should exist");
        assert!(request.observed_buyer_event_ids.is_empty());
        assert!(request.provider_observations.is_empty());
        assert_eq!(request.status, NetworkRequestStatus::Submitted);
        assert!(request.last_feedback_event_id.is_none());
    }

    #[test]
    fn network_requests_ignore_result_from_untargeted_provider() {
        let mut requests = NetworkRequestsState::default();
        let target_provider = "11".repeat(32);
        let untargeted_provider = "22".repeat(32);
        let request_id = requests
            .queue_request_submission(NetworkRequestSubmission {
                request_id: Some("req-targeted-result".to_string()),
                request_type: "summarize.text".to_string(),
                payload: "{\"prompt\":\"hello\"}".to_string(),
                resolution_mode: BuyerResolutionMode::Race,
                target_provider_pubkeys: vec![target_provider.clone()],
                skill_scope_id: None,
                credit_envelope_ref: None,
                budget_sats: 10,
                timeout_seconds: 60,
                authority_command_seq: 11,
            })
            .expect("request should queue");

        let action = requests.apply_nip90_buyer_result_event(
            request_id.as_str(),
            untargeted_provider.as_str(),
            "result-untargeted",
            Some("success"),
        );
        assert_eq!(action, None);

        let request = requests
            .submitted
            .iter()
            .find(|request| request.request_id == request_id)
            .expect("request should exist");
        assert!(request.observed_buyer_event_ids.is_empty());
        assert!(request.provider_observations.is_empty());
        assert_eq!(request.status, NetworkRequestStatus::Submitted);
        assert!(request.last_result_event_id.is_none());
        assert!(request.winning_provider_pubkey.is_none());
    }

    #[test]
    fn network_requests_record_auto_payment_pointer_and_timestamps() {
        let mut requests = NetworkRequestsState::default();
        let request_id = requests
            .queue_request_submission(NetworkRequestSubmission {
                request_id: Some("req-pay-001".to_string()),
                request_type: "summarize.text".to_string(),
                payload: "{\"prompt\":\"hello\"}".to_string(),
                resolution_mode: BuyerResolutionMode::Race,
                target_provider_pubkeys: vec!["11".repeat(32)],
                skill_scope_id: None,
                credit_envelope_ref: None,
                budget_sats: 10,
                timeout_seconds: 60,
                authority_command_seq: 12,
            })
            .expect("request should queue");
        requests.apply_nip90_buyer_feedback_event(
            request_id.as_str(),
            "22".repeat(32).as_str(),
            "feedback-pay-001",
            Some("payment-required"),
            Some("pay to continue"),
            Some(10_000),
            Some("lnbc1paymentrequired"),
        );

        let prepared = requests
            .prepare_auto_payment_attempt(
                request_id.as_str(),
                "lnbc1paymentrequired",
                Some(10_000),
                1_762_700_010,
            )
            .expect("auto-payment should prepare");
        assert_eq!(prepared.0, "lnbc1paymentrequired");
        assert_eq!(prepared.1, Some(10));
        assert_eq!(
            requests.pending_auto_payment_request_id.as_deref(),
            Some(request_id.as_str())
        );

        requests.mark_auto_payment_sent(
            request_id.as_str(),
            "wallet-payment-req-pay-001",
            1_762_700_012,
        );

        let row = requests
            .submitted
            .iter()
            .find(|request| request.request_id == request_id)
            .expect("request row should remain present");
        assert_eq!(row.status, NetworkRequestStatus::Paid);
        assert_eq!(
            row.last_payment_pointer.as_deref(),
            Some("wallet-payment-req-pay-001")
        );
        assert_eq!(row.payment_required_at_epoch_seconds, Some(1_762_700_010));
        assert_eq!(row.payment_sent_at_epoch_seconds, Some(1_762_700_012));
        assert_eq!(row.payment_failed_at_epoch_seconds, None);
        assert_eq!(row.payment_error, None);
    }

    fn queue_buy_mode_request_for_tests(
        requests: &mut NetworkRequestsState,
        request_id: &str,
        authority_command_seq: u64,
    ) -> String {
        requests
            .queue_request_submission(NetworkRequestSubmission {
                request_id: Some(request_id.to_string()),
                request_type: crate::app_state::MISSION_CONTROL_BUY_MODE_REQUEST_TYPE.to_string(),
                payload: "Reply with the exact text BUY MODE OK.".to_string(),
                resolution_mode: BuyerResolutionMode::Race,
                target_provider_pubkeys: Vec::new(),
                skill_scope_id: None,
                credit_envelope_ref: None,
                budget_sats: crate::app_state::MISSION_CONTROL_BUY_MODE_BUDGET_SATS,
                timeout_seconds: crate::app_state::MISSION_CONTROL_BUY_MODE_TIMEOUT_SECONDS,
                authority_command_seq,
            })
            .expect("buy mode request should queue")
    }

    #[test]
    fn buy_mode_payments_pane_sync_rows_include_pubkey_and_payment_pointer() {
        let mut requests = NetworkRequestsState::default();
        let request_id = queue_buy_mode_request_for_tests(&mut requests, "req-buy-ledger-001", 21);
        let provider_pubkey = "23".repeat(32);
        requests.apply_nip90_request_publish_outcome(
            request_id.as_str(),
            "event-buy-ledger-001",
            2,
            0,
            None,
        );
        requests.apply_nip90_buyer_feedback_event(
            request_id.as_str(),
            provider_pubkey.as_str(),
            "feedback-buy-ledger-001",
            Some("payment-required"),
            Some("invoice required"),
            Some(2_000),
            Some("lnbc1buyledgerinvoice"),
        );
        requests.prepare_auto_payment_attempt(
            request_id.as_str(),
            "lnbc1buyledgerinvoice",
            Some(2_000),
            1_762_700_111,
        );
        requests.apply_nip90_buyer_result_event(
            request_id.as_str(),
            provider_pubkey.as_str(),
            "result-buy-ledger-001",
            Some("success"),
        );
        requests.mark_auto_payment_sent(
            request_id.as_str(),
            "wallet-payment-buy-ledger-001",
            1_762_700_123,
        );

        let mut wallet = SparkPaneState::default();
        wallet
            .recent_payments
            .push(openagents_spark::PaymentSummary {
                id: "wallet-payment-buy-ledger-001".to_string(),
                direction: "send".to_string(),
                status: "succeeded".to_string(),
                amount_sats: 2,
                fees_sats: 1,
                method: "lightning".to_string(),
                destination_pubkey: Some(provider_pubkey.clone()),
                payment_hash: Some("hash-buy-ledger-001".to_string()),
                timestamp: 1_762_700_123,
                ..Default::default()
            });

        let mut pane = super::BuyModePaymentsPaneState::default();
        pane.sync_rows(&requests, &wallet);
        let lines = pane.ledger.recent_lines(6);

        assert!(lines.iter().any(|line| {
            line.text.contains("status=paid")
                && line.text.contains("fee=1 sats")
                && line.text.contains("total_debit=3 sats")
                && line.text.contains("wallet_status=sent")
                && line.text.contains("wallet_method=lightning")
                && line.text.contains(provider_pubkey.as_str())
        }));
        assert!(lines.iter().any(|line| {
            line.text.contains(request_id.as_str())
                && line.text.contains("wallet-payment-buy-ledger-001")
                && line.text.contains("event-buy-ledger-001")
                && line.text.contains("result-buy-ledger-001")
                && line.text.contains("hash-buy-ledger-001")
        }));
    }

    #[test]
    fn buy_mode_payments_pane_surfaces_selected_payable_and_loser_summary() {
        let mut requests = NetworkRequestsState::default();
        let request_id = queue_buy_mode_request_for_tests(&mut requests, "req-buy-ledger-3388", 31);
        let payable_provider = "31".repeat(32);
        let losing_provider = "41".repeat(32);
        requests.apply_nip90_request_publish_outcome(
            request_id.as_str(),
            "event-buy-ledger-3388",
            3,
            1,
            None,
        );
        requests.apply_nip90_buyer_feedback_event(
            request_id.as_str(),
            payable_provider.as_str(),
            "feedback-buy-ledger-3388",
            Some("payment-required"),
            Some("invoice ready"),
            Some(2_000),
            Some("lnbc1buyledger3388"),
        );
        requests.apply_nip90_buyer_result_event(
            request_id.as_str(),
            payable_provider.as_str(),
            "result-buy-ledger-3388",
            Some("success"),
        );
        requests.apply_nip90_buyer_result_event(
            request_id.as_str(),
            losing_provider.as_str(),
            "result-buy-ledger-loser-3388",
            Some("success"),
        );
        requests.apply_nip90_buyer_feedback_event(
            request_id.as_str(),
            losing_provider.as_str(),
            "feedback-buy-ledger-loser-3388",
            Some("processing"),
            Some("still working"),
            None,
            None,
        );

        let mut pane = super::BuyModePaymentsPaneState::default();
        pane.sync_rows(&requests, &SparkPaneState::default());
        let lines = pane.ledger.recent_lines(12);

        assert!(lines.iter().any(|line| {
            line.text.contains("selected_provider=")
                && line.text.contains(losing_provider.as_str())
                && line.text.contains("payable_provider=")
                && line.text.contains(payable_provider.as_str())
                && line.text.contains("losers=1")
        }));
        assert!(lines.iter().any(|line| {
            line.text.contains("loser_summary=")
                && line.text.contains("no invoice")
                && line.text.contains("late result")
                && line.text.contains("non-winning provider noise ignored")
        }));
    }

    #[test]
    fn buy_mode_payments_pane_surfaces_blockers_without_payable_winner() {
        let mut requests = NetworkRequestsState::default();
        let request_id =
            queue_buy_mode_request_for_tests(&mut requests, "req-buy-ledger-blocker-001", 32);
        requests.apply_nip90_request_publish_outcome(
            request_id.as_str(),
            "event-buy-ledger-blocker-001",
            3,
            1,
            None,
        );
        requests.apply_nip90_buyer_result_event(
            request_id.as_str(),
            "77".repeat(32).as_str(),
            "result-buy-ledger-blocker-001",
            Some("success"),
        );
        requests.apply_nip90_buyer_feedback_event(
            request_id.as_str(),
            "88".repeat(32).as_str(),
            "feedback-buy-ledger-blocker-001",
            Some("payment-required"),
            Some("invoice ready"),
            Some(25_000),
            Some("lnbc250n1buyledgerblocker"),
        );
        requests.apply_nip90_buyer_feedback_event(
            request_id.as_str(),
            "99".repeat(32).as_str(),
            "feedback-buy-ledger-blocker-missing-001",
            Some("payment-required"),
            Some("invoice missing"),
            Some(2_000),
            None,
        );

        let mut pane = super::BuyModePaymentsPaneState::default();
        pane.sync_rows(&requests, &SparkPaneState::default());
        let lines = pane.ledger.recent_lines(12);

        assert!(lines.iter().any(|line| {
            line.text.contains("blockers=result_without_invoice,invoice_without_result,invoice_over_budget,invoice_missing_bolt11")
        }));
        assert!(lines.iter().any(|line| {
            line.text.contains("payment_blocker=")
                && line.text.contains(
                    "invoice provider 888888..8888 requested 25 sats above approved budget 2",
                )
        }));
        assert!(lines.iter().any(|line| {
            line.text.contains("payment_blocker=")
                && line
                    .text
                    .contains("provider 999999..9999 sent payment-required without bolt11 invoice")
        }));
    }

    #[test]
    fn buy_mode_payments_pane_surfaces_wallet_failure_detail() {
        let mut requests = NetworkRequestsState::default();
        let request_id =
            queue_buy_mode_request_for_tests(&mut requests, "req-buy-fail-ledger-001", 22);
        let provider_pubkey = "24".repeat(32);
        requests.apply_nip90_buyer_feedback_event(
            request_id.as_str(),
            provider_pubkey.as_str(),
            "feedback-buy-fail-ledger-001",
            Some("payment-required"),
            Some("invoice required"),
            Some(2_000),
            Some("lnbc1buyfailinvoice"),
        );
        requests
            .prepare_auto_payment_attempt(
                request_id.as_str(),
                "lnbc1buyfailinvoice",
                Some(2_000),
                1_762_700_140,
            )
            .expect("payment-required invoice should prepare");
        requests
            .record_auto_payment_pointer(request_id.as_str(), "wallet-payment-buy-fail-ledger-001");
        requests.mark_auto_payment_failed(
            request_id.as_str(),
            "Spark payment wallet-payment-buy-fail-ledger-001 for req-buy-fail-ledger-001 failed: lightning send failed before preimage settlement; see Mission Control log for Breez terminal detail",
            1_762_700_141,
        );

        let mut wallet = SparkPaneState::default();
        wallet.recent_payments.push(openagents_spark::PaymentSummary {
            id: "wallet-payment-buy-fail-ledger-001".to_string(),
            direction: "send".to_string(),
            status: "failed".to_string(),
            amount_sats: 2,
            fees_sats: 1,
            timestamp: 1_762_700_141,
            method: "lightning".to_string(),
            destination_pubkey: Some(provider_pubkey.clone()),
            payment_hash: Some("hash-buy-fail-ledger-001".to_string()),
            status_detail: Some(
                "lightning send failed before preimage settlement; see Mission Control log for Breez terminal detail"
                    .to_string(),
            ),
            ..Default::default()
        });

        let mut pane = super::BuyModePaymentsPaneState::default();
        pane.sync_rows(&requests, &wallet);
        let lines = pane.ledger.recent_lines(8);

        assert!(lines.iter().any(|line| {
            line.text.contains("status=failed")
                && line.text.contains("fee=1 sats")
                && line.text.contains("total_debit=3 sats")
                && line.text.contains("wallet_status=failed")
                && line.text.contains("wallet_method=lightning")
        }));
        assert!(
            lines
                .iter()
                .any(|line| { line.text.contains("payment_hash=hash-buy-fail-ledger-001") })
        );
        assert!(lines.iter().any(|line| {
            line.text.contains("destination_pubkey=")
                && line.text.contains(provider_pubkey.as_str())
        }));
        assert!(lines.iter().any(|line| {
            line.text
                .contains("wallet_detail=lightning send failed before preimage settlement")
        }));
    }

    #[test]
    fn buy_mode_payments_pane_includes_published_request_before_payment_evidence() {
        let mut requests = NetworkRequestsState::default();
        let request_id =
            queue_buy_mode_request_for_tests(&mut requests, "req-buy-pending-ledger-001", 25);
        requests.apply_nip90_request_publish_outcome(
            request_id.as_str(),
            "event-buy-pending-ledger-001",
            3,
            1,
            None,
        );

        let mut pane = super::BuyModePaymentsPaneState::default();
        pane.sync_rows(&requests, &SparkPaneState::default());
        let lines = pane.ledger.recent_lines(6);

        assert!(lines.iter().any(|line| {
            line.text.contains("status=streaming")
                && line.text.contains("wallet_status=idle")
                && line.text.contains("amount=2 sats")
        }));
        assert!(lines.iter().any(|line| {
            line.text.contains(request_id.as_str())
                && line.text.contains("event-buy-pending-ledger-001")
        }));
    }

    #[test]
    fn buy_mode_payments_pane_backfills_wallet_only_buy_mode_sends() {
        let mut requests = NetworkRequestsState::default();
        let request_id = queue_buy_mode_request_for_tests(&mut requests, "req-buy-live-001", 26);
        requests.apply_nip90_request_publish_outcome(
            request_id.as_str(),
            "event-buy-live-001",
            3,
            1,
            None,
        );

        let provider_pubkey = "25".repeat(32);
        let mut wallet = SparkPaneState::default();
        wallet
            .recent_payments
            .push(openagents_spark::PaymentSummary {
                id: "wallet-payment-history-001".to_string(),
                direction: "send".to_string(),
                status: "succeeded".to_string(),
                amount_sats: 2,
                fees_sats: 3,
                method: "lightning".to_string(),
                description: Some("DVM textgen 6872f65774d7e233".to_string()),
                destination_pubkey: Some(provider_pubkey.clone()),
                payment_hash: Some("hash-wallet-history-001".to_string()),
                timestamp: 1_762_700_200,
                ..Default::default()
            });
        wallet
            .recent_payments
            .push(openagents_spark::PaymentSummary {
                id: "wallet-payment-non-buy-001".to_string(),
                direction: "send".to_string(),
                status: "succeeded".to_string(),
                amount_sats: 1000,
                method: "lightning".to_string(),
                description: Some("Mission Control load funds".to_string()),
                destination_pubkey: Some("99".repeat(32)),
                payment_hash: Some("hash-wallet-non-buy-001".to_string()),
                timestamp: 1_762_700_199,
                ..Default::default()
            });

        let mut pane = super::BuyModePaymentsPaneState::default();
        pane.sync_rows(&requests, &wallet);
        let lines = pane.ledger.recent_lines(20);
        let summary = super::buy_mode_payments_summary_text(&requests, &wallet);

        assert_eq!(
            summary,
            "2 rows  //  1 live  //  1 wallet-backfill  //  1 sent  //  1 pending  //  0 returned  //  0 failed  //  2 sats  //  3 fee sats  //  5 wallet debit sats"
        );
        assert!(
            lines
                .iter()
                .any(|line| line.text.contains("LIVE BUY MODE REQUESTS"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.text.contains("WALLET-BACKFILL HISTORY"))
        );
        assert!(lines.iter().any(|line| {
            line.text
                .contains("request_id=wallet-inferred:6872f65774d7e233")
                && line.text.contains("wallet-payment-history-001")
                && line.text.contains("source=wallet-backfill")
        }));
        assert!(lines.iter().any(|line| {
            line.text.contains("fee=3 sats") && line.text.contains("total_debit=5 sats")
        }));
        assert!(lines.iter().any(|line| {
            line.text.contains("provider_pubkey=") && line.text.contains(provider_pubkey.as_str())
        }));
        assert!(
            !lines
                .iter()
                .any(|line| { line.text.contains("wallet-payment-non-buy-001") })
        );
    }

    #[test]
    fn buy_mode_payments_clipboard_text_includes_nonterminal_payment_notice() {
        let mut requests = NetworkRequestsState::default();
        let request_id =
            queue_buy_mode_request_for_tests(&mut requests, "req-buy-notice-ledger-001", 23);
        requests.record_auto_payment_notice(
            request_id.as_str(),
            "provider returned payment-required without bolt11 invoice; waiting for a valid invoice event",
            1_762_700_145,
        );

        let clipboard = super::buy_mode_payments_clipboard_text(
            &MissionControlPaneState::default(),
            &requests,
            &SparkPaneState::default(),
        );
        assert!(clipboard.contains("Buy Mode Payments"));
        assert!(clipboard.contains("1 rows"));
        assert!(clipboard.contains("Dispatch loop: off"));
        assert!(clipboard.contains(request_id.as_str()));
        assert!(clipboard.contains(
            "payment_notice=provider returned payment-required without bolt11 invoice; waiting for a valid invoice event"
        ));
    }

    #[test]
    fn buy_mode_payments_status_lines_explain_single_flight_blocking() {
        let mut requests = NetworkRequestsState::default();
        let request_id = queue_buy_mode_request_for_tests(&mut requests, "req-buy-status-001", 27);
        requests.apply_nip90_request_publish_outcome(
            request_id.as_str(),
            "event-buy-status-001",
            3,
            1,
            None,
        );

        let mut mission_control = MissionControlPaneState::default();
        mission_control.buy_mode_loop_enabled = true;
        let lines = super::buy_mode_payments_status_lines(
            &mission_control,
            &requests,
            std::time::Instant::now(),
        );

        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("policy=single-flight"));
        assert!(lines[0].contains("blocked by req-buy-stat"));
        assert!(lines[0].contains("[streaming]"));
        assert!(lines[1].contains("Recent live request statuses:"));
        assert!(lines[1].contains("req-buy-stat"));
    }

    #[test]
    fn network_requests_buy_mode_requires_payment_settlement_for_terminal_success() {
        let mut requests = NetworkRequestsState::default();
        let request_id = queue_buy_mode_request_for_tests(&mut requests, "req-buy-smoke-001", 17);
        let provider_pubkey = "33".repeat(32);

        requests.apply_nip90_request_publish_outcome(
            request_id.as_str(),
            "event-buy-smoke-001",
            2,
            0,
            None,
        );
        requests.apply_nip90_buyer_feedback_event(
            request_id.as_str(),
            provider_pubkey.as_str(),
            "feedback-buy-smoke-001",
            Some("payment-required"),
            Some("pay invoice"),
            Some(crate::app_state::MISSION_CONTROL_BUY_MODE_BUDGET_SATS * 1000),
            Some("lnbc1buysmoke"),
        );
        let prepared = requests
            .prepare_auto_payment_attempt(
                request_id.as_str(),
                "lnbc1buysmoke",
                Some(crate::app_state::MISSION_CONTROL_BUY_MODE_BUDGET_SATS * 1000),
                1_762_700_030,
            )
            .expect("buy mode payment should prepare");
        assert_eq!(
            prepared.1,
            Some(crate::app_state::MISSION_CONTROL_BUY_MODE_BUDGET_SATS)
        );

        requests.apply_nip90_buyer_result_event(
            request_id.as_str(),
            provider_pubkey.as_str(),
            "result-buy-smoke-001",
            Some("success"),
        );

        let before_payment = requests
            .submitted
            .iter()
            .find(|request| request.request_id == request_id)
            .expect("request should exist after result");
        assert_eq!(before_payment.status, NetworkRequestStatus::ResultReceived);
        assert!(before_payment.last_payment_pointer.is_none());
        assert!(
            requests.has_in_flight_request_by_type(
                crate::app_state::MISSION_CONTROL_BUY_MODE_REQUEST_TYPE
            ),
            "buy mode should stay in-flight until Spark settles the invoice"
        );

        requests.mark_auto_payment_sent(
            request_id.as_str(),
            "wallet-payment-buy-smoke-001",
            1_762_700_031,
        );

        let after_payment = requests
            .submitted
            .iter()
            .find(|request| request.request_id == request_id)
            .expect("request should exist after payment");
        assert_eq!(after_payment.status, NetworkRequestStatus::Paid);
        assert_eq!(
            after_payment.last_payment_pointer.as_deref(),
            Some("wallet-payment-buy-smoke-001")
        );
        assert!(
            !requests.has_in_flight_request_by_type(
                crate::app_state::MISSION_CONTROL_BUY_MODE_REQUEST_TYPE
            ),
            "buy mode should become terminal only after Spark reports payment sent"
        );
        assert!(
            requests
                .prepare_auto_payment_attempt(
                    request_id.as_str(),
                    "lnbc1buysmoke-second",
                    Some(crate::app_state::MISSION_CONTROL_BUY_MODE_BUDGET_SATS * 1000),
                    1_762_700_032,
                )
                .is_none(),
            "settled buy mode request must not queue a second payment"
        );
    }

    #[test]
    fn network_requests_missing_bolt11_marks_payment_failure() {
        let mut requests = NetworkRequestsState::default();
        let request_id =
            queue_buy_mode_request_for_tests(&mut requests, "req-pay-missing-bolt11", 16);

        let prepared =
            requests.prepare_auto_payment_attempt(request_id.as_str(), "", None, 1_762_700_020);
        assert!(
            prepared.is_none(),
            "missing bolt11 should block payment attempt"
        );

        let row = requests
            .submitted
            .iter()
            .find(|request| request.request_id == request_id)
            .expect("request row should remain present");
        assert_eq!(row.status, NetworkRequestStatus::Failed);
        assert_eq!(
            row.payment_error.as_deref(),
            Some("provider feedback is missing bolt11 invoice")
        );
        assert_eq!(row.payment_failed_at_epoch_seconds, Some(1_762_700_020));
    }

    #[test]
    fn network_requests_payment_notice_keeps_request_nonterminal() {
        let mut requests = NetworkRequestsState::default();
        let request_id = queue_buy_mode_request_for_tests(&mut requests, "req-pay-notice-001", 24);

        requests.record_auto_payment_notice(
            request_id.as_str(),
            "provider returned payment-required without bolt11 invoice; waiting for a valid invoice event",
            1_762_700_021,
        );

        let row = requests
            .submitted
            .iter()
            .find(|request| request.request_id == request_id)
            .expect("request row should remain present");
        assert_eq!(row.status, NetworkRequestStatus::PaymentRequired);
        assert_eq!(row.payment_required_at_epoch_seconds, Some(1_762_700_021));
        assert_eq!(row.payment_failed_at_epoch_seconds, None);
        assert_eq!(row.payment_error, None);
        assert_eq!(
            row.payment_notice.as_deref(),
            Some(
                "provider returned payment-required without bolt11 invoice; waiting for a valid invoice event"
            )
        );
        assert!(
            requests.has_in_flight_request_by_type(
                crate::app_state::MISSION_CONTROL_BUY_MODE_REQUEST_TYPE
            ),
            "request should remain in-flight while waiting for a valid invoice"
        );
    }

    #[test]
    fn network_requests_auto_payment_accepts_under_budget_bolt11_only_invoice() {
        let mut requests = NetworkRequestsState::default();
        let request_id =
            queue_buy_mode_request_for_tests(&mut requests, "req-pay-budget-ok-001", 25);
        let provider_pubkey = "55".repeat(32);

        requests.apply_nip90_buyer_feedback_event(
            request_id.as_str(),
            provider_pubkey.as_str(),
            "feedback-pay-budget-ok-001",
            Some("payment-required"),
            Some("invoice required"),
            None,
            Some("lnbc20n1budgetok"),
        );
        requests.apply_nip90_buyer_result_event(
            request_id.as_str(),
            provider_pubkey.as_str(),
            "result-pay-budget-ok-001",
            Some("success"),
        );

        let prepared = requests
            .prepare_auto_payment_attempt_for_provider(
                request_id.as_str(),
                provider_pubkey.as_str(),
                1_762_700_040,
            )
            .expect("under-budget invoice should prepare for payment");
        assert_eq!(prepared.0, "lnbc20n1budgetok");
        assert_eq!(
            prepared.1,
            Some(crate::app_state::MISSION_CONTROL_BUY_MODE_BUDGET_SATS)
        );

        let row = requests
            .submitted
            .iter()
            .find(|request| request.request_id == request_id)
            .expect("request should exist after payment preparation");
        assert_eq!(
            row.winning_provider_pubkey.as_deref(),
            Some(provider_pubkey.as_str())
        );
        assert_eq!(row.pending_bolt11.as_deref(), Some("lnbc20n1budgetok"));
        assert_eq!(row.status, NetworkRequestStatus::PaymentRequired);
        assert!(row.payment_notice.is_none());
    }

    #[test]
    fn network_requests_auto_payment_refuses_over_budget_invoice() {
        let mut requests = NetworkRequestsState::default();
        let request_id =
            queue_buy_mode_request_for_tests(&mut requests, "req-pay-budget-block-001", 26);
        let provider_pubkey = "56".repeat(32);

        requests.apply_nip90_buyer_feedback_event(
            request_id.as_str(),
            provider_pubkey.as_str(),
            "feedback-pay-budget-block-001",
            Some("payment-required"),
            Some("invoice required"),
            Some(25_000),
            Some("lnbc250n1budgetblock"),
        );
        requests.apply_nip90_buyer_result_event(
            request_id.as_str(),
            provider_pubkey.as_str(),
            "result-pay-budget-block-001",
            Some("success"),
        );

        assert!(
            requests
                .prepare_auto_payment_attempt_for_provider(
                    request_id.as_str(),
                    provider_pubkey.as_str(),
                    1_762_700_041,
                )
                .is_none(),
            "over-budget invoice must not queue Spark payment"
        );

        let refusal = requests
            .auto_payment_budget_refusal_for_provider(request_id.as_str(), provider_pubkey.as_str())
            .expect("over-budget provider should surface refusal");
        assert_eq!(refusal.invoice_amount_sats, 25);
        assert_eq!(
            refusal.approved_budget_sats,
            crate::app_state::MISSION_CONTROL_BUY_MODE_BUDGET_SATS
        );
        assert!(!refusal.amount_mismatch);

        let row = requests
            .submitted
            .iter()
            .find(|request| request.request_id == request_id)
            .expect("request should remain present");
        assert_eq!(row.winning_provider_pubkey, None);
        assert_eq!(row.pending_bolt11, None);
        assert_eq!(row.status, NetworkRequestStatus::ResultReceived);
        assert!(
            row.payment_notice
                .as_deref()
                .is_some_and(|notice| notice.contains("requested 25 sats above approved budget 2"))
        );

        let snapshot = crate::nip90_compute_flow::build_buyer_request_flow_snapshot(
            row,
            &SparkPaneState::default(),
        );
        assert!(
            snapshot
                .payment_blocker_codes
                .iter()
                .any(|code| code == "invoice_over_budget")
        );
        assert!(
            snapshot.payment_blocker_summary.as_deref().is_some_and(
                |summary| summary.contains("requested 25 sats above approved budget 2")
            )
        );
    }

    #[test]
    fn network_requests_auto_payment_refuses_mismatched_invoice_amounts() {
        let mut requests = NetworkRequestsState::default();
        let request_id =
            queue_buy_mode_request_for_tests(&mut requests, "req-pay-budget-mismatch-001", 27);
        let provider_pubkey = "57".repeat(32);

        requests.apply_nip90_buyer_feedback_event(
            request_id.as_str(),
            provider_pubkey.as_str(),
            "feedback-pay-budget-mismatch-001",
            Some("payment-required"),
            Some("invoice required"),
            Some(1_000),
            Some("lnbc250n1budgetmismatch"),
        );
        requests.apply_nip90_buyer_result_event(
            request_id.as_str(),
            provider_pubkey.as_str(),
            "result-pay-budget-mismatch-001",
            Some("success"),
        );

        assert!(
            requests
                .prepare_auto_payment_attempt_for_provider(
                    request_id.as_str(),
                    provider_pubkey.as_str(),
                    1_762_700_042,
                )
                .is_none(),
            "mismatched invoice metadata must not queue Spark payment"
        );

        let refusal = requests
            .auto_payment_budget_refusal_for_provider(request_id.as_str(), provider_pubkey.as_str())
            .expect("mismatched provider should surface refusal");
        assert_eq!(refusal.invoice_amount_sats, 25);
        assert!(refusal.amount_mismatch);

        let row = requests
            .submitted
            .iter()
            .find(|request| request.request_id == request_id)
            .expect("request should remain present");
        assert!(
            row.payment_notice
                .as_deref()
                .is_some_and(|notice| notice.contains("metadata mismatched the BOLT11 amount"))
        );
    }

    #[test]
    fn network_requests_payment_failure_stays_terminal_when_late_result_arrives() {
        let mut requests = NetworkRequestsState::default();
        let request_id = queue_buy_mode_request_for_tests(&mut requests, "req-pay-fail-001", 18);
        let provider_pubkey = "44".repeat(32);

        requests.apply_nip90_buyer_feedback_event(
            request_id.as_str(),
            provider_pubkey.as_str(),
            "feedback-pay-fail-001",
            Some("payment-required"),
            Some("pay invoice"),
            Some(crate::app_state::MISSION_CONTROL_BUY_MODE_BUDGET_SATS * 1000),
            Some("lnbc1payfail"),
        );
        requests
            .prepare_auto_payment_attempt(
                request_id.as_str(),
                "lnbc1payfail",
                Some(crate::app_state::MISSION_CONTROL_BUY_MODE_BUDGET_SATS * 1000),
                1_762_700_050,
            )
            .expect("payment-required invoice should prepare");
        requests.mark_auto_payment_failed(request_id.as_str(), "spark send failed", 1_762_700_051);

        let observed_before = requests
            .submitted
            .iter()
            .find(|request| request.request_id == request_id)
            .expect("request should exist before late result")
            .observed_buyer_event_ids
            .len();

        assert_eq!(
            requests.apply_nip90_buyer_result_event(
                request_id.as_str(),
                provider_pubkey.as_str(),
                "result-pay-fail-001",
                Some("success"),
            ),
            None
        );

        let row = requests
            .submitted
            .iter()
            .find(|request| request.request_id == request_id)
            .expect("request should exist after late result");
        assert_eq!(row.status, NetworkRequestStatus::Failed);
        assert_eq!(row.last_result_event_id, None);
        assert_eq!(row.winning_provider_pubkey, None);
        assert_eq!(row.payment_error.as_deref(), Some("spark send failed"));
        assert_eq!(row.observed_buyer_event_ids.len(), observed_before);
        assert_eq!(requests.pending_auto_payment_request_id, None);
    }

    #[test]
    fn network_requests_race_mode_flags_late_result_as_unpaid_duplicate() {
        let mut requests = NetworkRequestsState::default();
        let request_id = requests
            .queue_request_submission(NetworkRequestSubmission {
                request_id: Some("req-race-001".to_string()),
                request_type: "summarize.text".to_string(),
                payload: "{\"prompt\":\"hello\"}".to_string(),
                resolution_mode: BuyerResolutionMode::Race,
                target_provider_pubkeys: Vec::new(),
                skill_scope_id: None,
                credit_envelope_ref: None,
                budget_sats: 10,
                timeout_seconds: 60,
                authority_command_seq: 14,
            })
            .expect("request should queue");

        let winner = "11".repeat(32);
        let loser = "22".repeat(32);
        assert_eq!(
            requests.apply_nip90_buyer_feedback_event(
                request_id.as_str(),
                winner.as_str(),
                "feedback-winner-001",
                Some("payment-required"),
                Some("invoice ready"),
                Some(10_000),
                Some("lnbc1winner001"),
            ),
            None
        );
        assert_eq!(
            requests.apply_nip90_buyer_result_event(
                request_id.as_str(),
                winner.as_str(),
                "result-winner-001",
                Some("success"),
            ),
            None
        );
        let action = requests.apply_nip90_buyer_result_event(
            request_id.as_str(),
            loser.as_str(),
            "result-loser-001",
            Some("success"),
        );
        assert_eq!(
            action,
            Some(crate::state::operations::BuyerResolutionAction {
                request_id: request_id.clone(),
                provider_pubkey: loser.clone(),
                reason: BuyerResolutionReason::LateResultUnpaid,
            })
        );

        let request = requests
            .submitted
            .iter()
            .find(|request| request.request_id == request_id)
            .expect("request should exist");
        assert_eq!(
            request.winning_provider_pubkey.as_deref(),
            Some(winner.as_str())
        );
        assert_eq!(request.duplicate_outcomes.len(), 1);
        assert_eq!(
            request.duplicate_outcomes[0].reason_code,
            BuyerResolutionReason::LateResultUnpaid.code()
        );
    }

    #[test]
    fn network_requests_race_mode_flags_late_feedback_as_lost_race() {
        let mut requests = NetworkRequestsState::default();
        let request_id = requests
            .queue_request_submission(NetworkRequestSubmission {
                request_id: Some("req-race-002".to_string()),
                request_type: "summarize.text".to_string(),
                payload: "{\"prompt\":\"hello\"}".to_string(),
                resolution_mode: BuyerResolutionMode::Race,
                target_provider_pubkeys: Vec::new(),
                skill_scope_id: None,
                credit_envelope_ref: None,
                budget_sats: 10,
                timeout_seconds: 60,
                authority_command_seq: 15,
            })
            .expect("request should queue");

        let winner = "11".repeat(32);
        assert_eq!(
            requests.apply_nip90_buyer_feedback_event(
                request_id.as_str(),
                winner.as_str(),
                "feedback-winner-002",
                Some("payment-required"),
                Some("invoice ready"),
                Some(10_000),
                Some("lnbc1winner002"),
            ),
            None
        );
        assert_eq!(
            requests.apply_nip90_buyer_result_event(
                request_id.as_str(),
                winner.as_str(),
                "result-winner-002",
                Some("success"),
            ),
            None
        );
        let loser = "33".repeat(32);
        let action = requests.apply_nip90_buyer_feedback_event(
            request_id.as_str(),
            loser.as_str(),
            "feedback-loser-002",
            Some("processing"),
            Some("still working"),
            None,
            None,
        );
        assert_eq!(
            action,
            Some(crate::state::operations::BuyerResolutionAction {
                request_id: request_id.clone(),
                provider_pubkey: loser.clone(),
                reason: BuyerResolutionReason::LostRace,
            })
        );

        let request = requests
            .submitted
            .iter()
            .find(|request| request.request_id == request_id)
            .expect("request should exist");
        assert_eq!(request.duplicate_outcomes.len(), 1);
        assert_eq!(
            request.duplicate_outcomes[0].reason_code,
            BuyerResolutionReason::LostRace.code()
        );
    }

    #[test]
    fn network_requests_race_mode_requires_matching_invoice_and_result_before_payment() {
        let mut requests = NetworkRequestsState::default();
        let request_id =
            queue_buy_mode_request_for_tests(&mut requests, "req-buy-race-match-001", 28);
        let result_only_provider = "66".repeat(32);
        let payable_provider = "77".repeat(32);

        assert_eq!(
            requests.apply_nip90_buyer_result_event(
                request_id.as_str(),
                result_only_provider.as_str(),
                "result-race-match-001",
                Some("success"),
            ),
            None
        );
        assert_eq!(
            requests.apply_nip90_buyer_feedback_event(
                request_id.as_str(),
                payable_provider.as_str(),
                "feedback-race-match-001",
                Some("payment-required"),
                Some("invoice ready"),
                Some(2_000),
                Some("lnbc1racematch001"),
            ),
            None
        );
        assert!(
            requests
                .prepare_auto_payment_attempt_for_provider(
                    request_id.as_str(),
                    payable_provider.as_str(),
                    1_762_700_160,
                )
                .is_none(),
            "buyer must not pay a provider until that same provider has delivered a result"
        );

        let after_invoice_only = requests
            .submitted
            .iter()
            .find(|request| request.request_id == request_id)
            .expect("request should exist after invoice-only feedback");
        assert_eq!(after_invoice_only.winning_provider_pubkey, None);

        assert_eq!(
            requests.apply_nip90_buyer_result_event(
                request_id.as_str(),
                payable_provider.as_str(),
                "result-race-match-002",
                Some("success"),
            ),
            None
        );
        let prepared = requests
            .prepare_auto_payment_attempt_for_provider(
                request_id.as_str(),
                payable_provider.as_str(),
                1_762_700_161,
            )
            .expect("matching provider invoice should become payable once the result arrives");
        assert_eq!(prepared.0, "lnbc1racematch001");
        assert_eq!(prepared.1, Some(2));

        let request = requests
            .submitted
            .iter()
            .find(|request| request.request_id == request_id)
            .expect("request should exist");
        assert_eq!(
            request.winning_provider_pubkey.as_deref(),
            Some(payable_provider.as_str())
        );
        assert_eq!(
            request.winning_result_event_id.as_deref(),
            Some("result-race-match-002")
        );
        assert_eq!(
            requests.pending_auto_payment_request_id.as_deref(),
            Some(request_id.as_str())
        );
    }

    #[test]
    fn network_requests_race_mode_ignores_nonwinner_error_when_payable_provider_exists() {
        let mut requests = NetworkRequestsState::default();
        let request_id =
            queue_buy_mode_request_for_tests(&mut requests, "req-buy-race-safe-001", 29);
        let noisy_provider = "88".repeat(32);
        let payable_provider = "99".repeat(32);

        assert_eq!(
            requests.apply_nip90_buyer_result_event(
                request_id.as_str(),
                noisy_provider.as_str(),
                "result-race-safe-001",
                Some("success"),
            ),
            None
        );
        assert_eq!(
            requests.apply_nip90_buyer_feedback_event(
                request_id.as_str(),
                payable_provider.as_str(),
                "feedback-race-safe-001",
                Some("payment-required"),
                Some("invoice ready"),
                Some(2_000),
                Some("lnbc1racesafe001"),
            ),
            None
        );
        assert_eq!(
            requests.apply_nip90_buyer_result_event(
                request_id.as_str(),
                payable_provider.as_str(),
                "result-race-safe-002",
                Some("success"),
            ),
            None
        );
        let prepared = requests
            .prepare_auto_payment_attempt_for_provider(
                request_id.as_str(),
                payable_provider.as_str(),
                1_762_700_171,
            )
            .expect("matching provider invoice should queue Spark payment");
        assert_eq!(prepared.1, Some(2));

        assert_eq!(
            requests.apply_nip90_buyer_feedback_event(
                request_id.as_str(),
                noisy_provider.as_str(),
                "feedback-race-safe-err-001",
                Some("error"),
                Some("job aborted"),
                None,
                None,
            ),
            None
        );

        let request = requests
            .submitted
            .iter()
            .find(|request| request.request_id == request_id)
            .expect("request should exist");
        assert_eq!(request.status, NetworkRequestStatus::ResultReceived);
        assert_eq!(request.payment_error, None);
        assert_eq!(
            request.winning_provider_pubkey.as_deref(),
            Some(payable_provider.as_str())
        );
        assert_eq!(
            requests.pending_auto_payment_request_id.as_deref(),
            Some(request_id.as_str())
        );
    }

    #[test]
    fn network_requests_ignore_duplicate_buyer_event_ids() {
        let mut requests = NetworkRequestsState::default();
        let request_id = requests
            .queue_request_submission(NetworkRequestSubmission {
                request_id: Some("req-race-003".to_string()),
                request_type: "summarize.text".to_string(),
                payload: "{\"prompt\":\"hello\"}".to_string(),
                resolution_mode: BuyerResolutionMode::Race,
                target_provider_pubkeys: Vec::new(),
                skill_scope_id: None,
                credit_envelope_ref: None,
                budget_sats: 10,
                timeout_seconds: 60,
                authority_command_seq: 16,
            })
            .expect("request should queue");

        assert_eq!(
            requests.apply_nip90_buyer_result_event(
                request_id.as_str(),
                "11".repeat(32).as_str(),
                "result-dup-003",
                Some("success"),
            ),
            None
        );
        assert_eq!(
            requests.apply_nip90_buyer_result_event(
                request_id.as_str(),
                "11".repeat(32).as_str(),
                "result-dup-003",
                Some("success"),
            ),
            None
        );

        let request = requests
            .submitted
            .iter()
            .find(|request| request.request_id == request_id)
            .expect("request should exist");
        assert_eq!(request.observed_buyer_event_ids.len(), 1);
        assert!(request.duplicate_outcomes.is_empty());
    }

    #[test]
    fn stablesats_real_mode_initializes_single_wallet_topology() {
        let mut state = StableSatsSimulationPaneState::default();
        state.set_mode(crate::app_state::StableSatsSimulationMode::RealBlink);
        assert_eq!(state.agents.len(), 1);
        assert_eq!(
            state.agents[0].owner_kind,
            crate::app_state::StableSatsWalletOwnerKind::Operator
        );
        assert!(
            state.agents[0]
                .credential_key_name
                .starts_with("BLINK_API_KEY")
        );
    }

    #[test]
    fn stablesats_live_snapshot_updates_operator_wallet_and_ledger() {
        let mut state = StableSatsSimulationPaneState::default();
        state.set_mode(crate::app_state::StableSatsSimulationMode::RealBlink);
        state.apply_live_snapshot(1_761_921_200, 2_000, 250, 8_500_000, "btc:op usd:op");
        assert_eq!(state.agents.len(), 1);
        assert_eq!(state.agents[0].agent_name, "autopilot-user");
        assert_eq!(state.agents[0].btc_balance_sats, 2_000);
        assert_eq!(state.agents[0].usd_balance_cents, 250);
        assert!(!state.transfer_ledger.is_empty());
        assert!(
            state
                .transfer_ledger
                .iter()
                .all(|entry| entry.transfer_ref.starts_with("blink:live:transfer:"))
        );
    }

    #[test]
    fn stablesats_live_wallet_snapshots_apply_partial_refresh_without_global_failure() {
        let mut state = StableSatsSimulationPaneState::default();
        state.set_mode(crate::app_state::StableSatsSimulationMode::RealBlink);
        state.apply_live_wallet_snapshots(
            1_761_921_260,
            8_600_000,
            &[(
                "operator:autopilot".to_string(),
                1_500,
                80,
                "btc:op usd:op".to_string(),
            )],
            &[(
                "operator:autopilot".to_string(),
                "missing secure credential".to_string(),
            )],
        );

        assert_eq!(state.load_state, crate::app_state::PaneLoadState::Ready);
        assert_eq!(state.agents[0].btc_balance_sats, 1_500);
        assert_eq!(state.agents[0].usd_balance_cents, 80);
        assert_eq!(
            state.agents[0].last_switch_summary,
            "refresh failed: missing secure credential"
        );
        assert!(
            state
                .last_error
                .as_deref()
                .is_some_and(|value| value.contains("1 wallet error"))
        );
        assert!(!state.transfer_ledger.is_empty());
    }

    #[test]
    fn stablesats_treasury_operation_log_tracks_refresh_lifecycle() {
        let mut state = StableSatsSimulationPaneState::default();
        state.set_mode(crate::app_state::StableSatsSimulationMode::RealBlink);
        let request_id = state
            .begin_live_refresh()
            .expect("real mode refresh should start");
        let running_entry = state
            .treasury_operations
            .iter()
            .find(|entry| entry.request_id == request_id)
            .expect("refresh operation should be recorded");
        assert_eq!(
            running_entry.kind,
            crate::app_state::StableSatsTreasuryOperationKind::Refresh
        );
        assert_eq!(
            running_entry.status,
            crate::app_state::StableSatsTreasuryOperationStatus::Running
        );

        assert!(state.fail_live_refresh(request_id, "timeout".to_string()));
        let finished_entry = state
            .treasury_operations
            .iter()
            .find(|entry| entry.request_id == request_id)
            .expect("refresh operation should remain recorded");
        assert_eq!(
            finished_entry.status,
            crate::app_state::StableSatsTreasuryOperationStatus::Failed
        );
    }

    #[test]
    fn starter_demand_dispatch_respects_budget_interval_and_rollbacks() {
        let mut starter_jobs = StarterJobsState::default();
        starter_jobs.apply_dispatch_controls(260, 1, 4);
        let now = std::time::Instant::now();

        let first = starter_jobs
            .dispatch_next_if_due(now)
            .expect("dispatch should not error")
            .expect("first starter quest should dispatch");
        assert_eq!(first.status, StarterJobStatus::Queued);
        assert_eq!(starter_jobs.budget_allocated_sats, first.payout_sats);
        assert!(starter_jobs.budget_allocated_sats <= starter_jobs.budget_cap_sats);

        let blocked_by_interval = starter_jobs
            .dispatch_next_if_due(now + std::time::Duration::from_millis(500))
            .expect("interval check should not error");
        assert!(blocked_by_interval.is_none());

        let blocked_by_cap = starter_jobs
            .dispatch_next_if_due(now + std::time::Duration::from_secs(1))
            .expect("second dispatch check should not error");
        assert!(blocked_by_cap.is_none());
        assert_eq!(starter_jobs.inflight_jobs(), 1);
        assert!(
            starter_jobs
                .last_action
                .as_deref()
                .is_some_and(|value| value.contains("max=1"))
        );

        assert!(starter_jobs.rollback_dispatched_job(&first.job_id));
        assert_eq!(starter_jobs.inflight_jobs(), 0);

        let second = starter_jobs
            .dispatch_next_if_due(now + std::time::Duration::from_secs(2))
            .expect("dispatch after rollback should not error")
            .expect("second starter quest should dispatch after inflight slot opens");
        assert_eq!(starter_jobs.inflight_jobs(), 1);

        assert!(starter_jobs.rollback_dispatched_job(&second.job_id));
        assert_eq!(starter_jobs.inflight_jobs(), 0);
        assert_eq!(starter_jobs.budget_allocated_sats, 0);
    }

    #[test]
    fn starter_demand_kill_switch_blocks_dispatch() {
        let mut starter_jobs = StarterJobsState::default();
        starter_jobs.apply_dispatch_controls(500, 1, 2);
        let now = std::time::Instant::now();

        assert!(starter_jobs.toggle_kill_switch());
        let blocked = starter_jobs
            .dispatch_next_if_due(now)
            .expect("kill switch check should not error");
        assert!(blocked.is_none());

        assert!(!starter_jobs.toggle_kill_switch());
        let resumed = starter_jobs
            .dispatch_next_if_due(now + std::time::Duration::from_secs(1))
            .expect("dispatch after kill switch disable should not error");
        assert!(resumed.is_some());
    }

    #[test]
    fn starter_jobs_complete_selected_requires_wallet_pointer() {
        let mut starter_jobs = StarterJobsState::default();
        starter_jobs.jobs.push(fixture_starter_job(
            "job-starter-001",
            1200,
            true,
            StarterJobStatus::Queued,
        ));
        starter_jobs.select_by_index(0);
        let (job_id, _payout) = starter_jobs
            .start_selected_execution()
            .expect("eligible starter job should start");
        let (_job_id, _payout, pointer) = starter_jobs
            .complete_selected_with_payment("wallet:payment:starter-001")
            .expect("wallet-confirmed payout should complete");
        let job = starter_jobs
            .jobs
            .iter()
            .find(|job| job.job_id == job_id)
            .expect("job should remain present");
        assert_eq!(job.status, StarterJobStatus::Completed);
        assert_eq!(job.payout_pointer.as_deref(), Some(pointer.as_str()));
    }

    #[test]
    fn reciprocal_loop_start_direction_is_deterministic_per_identity_pair() {
        let mut loop_a = ReciprocalLoopState::default();
        loop_a.set_local_pubkey(Some("11".repeat(32).as_str()));
        loop_a.set_peer_pubkey(Some("22".repeat(32).as_str()));
        loop_a.start().expect("loop A should start");
        assert_eq!(loop_a.next_direction, ReciprocalLoopDirection::LocalToPeer);

        let mut loop_b = ReciprocalLoopState::default();
        loop_b.set_local_pubkey(Some("22".repeat(32).as_str()));
        loop_b.set_peer_pubkey(Some("11".repeat(32).as_str()));
        loop_b.start().expect("loop B should start");
        assert_eq!(loop_b.next_direction, ReciprocalLoopDirection::PeerToLocal);
    }

    #[test]
    fn reciprocal_loop_reconciles_outbound_and_inbound_paid_events_once() {
        let local_pubkey = "11".repeat(32);
        let peer_pubkey = "22".repeat(32);
        let mut reciprocal_loop = ReciprocalLoopState::default();
        reciprocal_loop.set_local_pubkey(Some(local_pubkey.as_str()));
        reciprocal_loop.set_peer_pubkey(Some(peer_pubkey.as_str()));
        reciprocal_loop.start().expect("loop should start");

        reciprocal_loop.register_outbound_dispatch("loop-req-001", 1_762_800_000);
        assert_eq!(reciprocal_loop.local_to_peer_dispatched, 1);
        assert_eq!(
            reciprocal_loop.in_flight_request_id.as_deref(),
            Some("loop-req-001")
        );

        let outbound = fixture_loop_submitted_request(
            "loop-req-001",
            NetworkRequestStatus::Paid,
            peer_pubkey.as_str(),
            reciprocal_loop.skill_scope_id.as_str(),
        );
        assert!(
            reciprocal_loop.reconcile_outbound_terminal_statuses(std::slice::from_ref(&outbound))
        );
        assert_eq!(reciprocal_loop.local_to_peer_paid, 1);
        assert_eq!(reciprocal_loop.sats_sent, 10);
        assert!(reciprocal_loop.in_flight_request_id.is_none());
        assert_eq!(
            reciprocal_loop.next_direction,
            ReciprocalLoopDirection::PeerToLocal
        );
        assert_eq!(
            reciprocal_loop.last_payment_pointer.as_deref(),
            Some("wallet:loop-req-001")
        );
        assert!(
            !reciprocal_loop.reconcile_outbound_terminal_statuses(std::slice::from_ref(&outbound))
        );
        assert_eq!(
            reciprocal_loop.local_to_peer_paid, 1,
            "outbound terminal event should be counted exactly once"
        );

        let mut inbound = fixture_history_row(
            "loop-job-001",
            JobHistoryStatus::Succeeded,
            1_762_800_010,
            10,
        );
        inbound.demand_source = JobDemandSource::OpenNetwork;
        inbound.skill_scope_id = Some(reciprocal_loop.skill_scope_id.clone());
        inbound.payment_pointer = "wallet:loop-inbound-001".to_string();

        assert!(reciprocal_loop.reconcile_inbound_history(std::slice::from_ref(&inbound)));
        assert_eq!(reciprocal_loop.peer_to_local_paid, 1);
        assert_eq!(reciprocal_loop.sats_received, 10);
        assert_eq!(
            reciprocal_loop.next_direction,
            ReciprocalLoopDirection::LocalToPeer
        );
        assert_eq!(
            reciprocal_loop.last_payment_pointer.as_deref(),
            Some("wallet:loop-inbound-001")
        );
        assert!(!reciprocal_loop.reconcile_inbound_history(&[inbound]));
        assert_eq!(
            reciprocal_loop.peer_to_local_paid, 1,
            "inbound terminal event should be counted exactly once"
        );
    }

    #[test]
    fn reciprocal_loop_stop_engages_kill_switch_and_blocks_dispatch() {
        let mut reciprocal_loop = ReciprocalLoopState::default();
        reciprocal_loop.set_local_pubkey(Some("11".repeat(32).as_str()));
        reciprocal_loop.set_peer_pubkey(Some("22".repeat(32).as_str()));
        reciprocal_loop.start().expect("loop should start");
        assert!(reciprocal_loop.ready_to_dispatch());

        reciprocal_loop.stop("operator stop");
        assert!(!reciprocal_loop.running);
        assert!(reciprocal_loop.kill_switch_active);
        assert!(!reciprocal_loop.ready_to_dispatch());
    }

    #[test]
    fn reciprocal_loop_retry_backoff_is_bounded_and_escalates_to_terminal() {
        let mut reciprocal_loop = ReciprocalLoopState::default();
        reciprocal_loop.set_local_pubkey(Some("11".repeat(32).as_str()));
        reciprocal_loop.set_peer_pubkey(Some("22".repeat(32).as_str()));
        reciprocal_loop.start().expect("loop should start");
        reciprocal_loop.max_retry_attempts = 2;
        reciprocal_loop.retry_backoff_seconds = 1;
        reciprocal_loop.retry_backoff_max_seconds = 4;

        assert!(!reciprocal_loop.record_recoverable_failure(
            ReciprocalLoopFailureClass::Dispatch,
            "relay timeout",
            100
        ));
        assert_eq!(reciprocal_loop.retry_attempts, 1);
        assert_eq!(
            reciprocal_loop.last_failure_disposition,
            Some(ReciprocalLoopFailureDisposition::Recoverable)
        );
        assert_eq!(reciprocal_loop.retry_backoff_until_epoch_seconds, Some(101));
        assert!(reciprocal_loop.in_backoff_window(100));
        assert!(reciprocal_loop.clear_retry_backoff_if_elapsed(101));
        assert!(!reciprocal_loop.in_backoff_window(101));

        assert!(!reciprocal_loop.record_recoverable_failure(
            ReciprocalLoopFailureClass::Dispatch,
            "relay timeout",
            200
        ));
        assert_eq!(reciprocal_loop.retry_attempts, 2);
        assert_eq!(reciprocal_loop.retry_backoff_until_epoch_seconds, Some(202));

        assert!(reciprocal_loop.record_recoverable_failure(
            ReciprocalLoopFailureClass::Dispatch,
            "relay timeout",
            300
        ));
        assert!(!reciprocal_loop.running);
        assert!(reciprocal_loop.kill_switch_active);
        assert_eq!(
            reciprocal_loop.last_failure_disposition,
            Some(ReciprocalLoopFailureDisposition::Terminal)
        );
    }

    #[test]
    fn reciprocal_loop_outbound_stale_timeout_marks_request_terminal_once() {
        let local_pubkey = "11".repeat(32);
        let peer_pubkey = "22".repeat(32);
        let mut reciprocal_loop = ReciprocalLoopState::default();
        reciprocal_loop.set_local_pubkey(Some(local_pubkey.as_str()));
        reciprocal_loop.set_peer_pubkey(Some(peer_pubkey.as_str()));
        reciprocal_loop.start().expect("loop should start");
        reciprocal_loop.register_outbound_dispatch("loop-req-stale-001", 1_762_800_000);

        assert!(!reciprocal_loop.outbound_stale_timed_out(1_762_800_060));
        assert!(reciprocal_loop.outbound_stale_timed_out(1_762_800_150));
        let timed_out = reciprocal_loop
            .mark_outbound_stale_timeout()
            .expect("stale request should be marked");
        assert_eq!(timed_out, "loop-req-stale-001");
        assert_eq!(reciprocal_loop.local_to_peer_failed, 1);
        assert!(reciprocal_loop.in_flight_request_id.is_none());

        let late_paid = fixture_loop_submitted_request(
            "loop-req-stale-001",
            NetworkRequestStatus::Paid,
            peer_pubkey.as_str(),
            reciprocal_loop.skill_scope_id.as_str(),
        );
        assert!(
            !reciprocal_loop.reconcile_outbound_terminal_statuses(std::slice::from_ref(&late_paid))
        );
        assert_eq!(reciprocal_loop.local_to_peer_paid, 0);
    }

    #[test]
    fn reciprocal_loop_peer_wait_timeout_recovers_to_local_dispatch_turn() {
        let mut reciprocal_loop = ReciprocalLoopState::default();
        reciprocal_loop.set_local_pubkey(Some("22".repeat(32).as_str()));
        reciprocal_loop.set_peer_pubkey(Some("11".repeat(32).as_str()));
        reciprocal_loop.start().expect("loop should start");
        assert_eq!(
            reciprocal_loop.next_direction,
            ReciprocalLoopDirection::PeerToLocal
        );
        reciprocal_loop.mark_peer_wait_started(1_762_800_000);
        assert!(!reciprocal_loop.inbound_wait_timed_out(1_762_800_060));
        assert!(reciprocal_loop.inbound_wait_timed_out(1_762_800_150));
        reciprocal_loop.mark_inbound_stale_timeout();
        assert_eq!(reciprocal_loop.peer_to_local_failed, 1);
        assert_eq!(
            reciprocal_loop.next_direction,
            ReciprocalLoopDirection::LocalToPeer
        );
    }

    #[test]
    fn activity_feed_upsert_deduplicates_stable_event_ids() {
        let mut feed = activity_feed_state_for_tests("dedupe");
        feed.upsert_event(fixture_activity_event(
            "wallet:payment:latest",
            ActivityEventDomain::Wallet,
            1_761_920_180,
        ));
        let baseline_count = feed.rows.len();
        feed.upsert_event(fixture_activity_event(
            "wallet:payment:latest",
            ActivityEventDomain::Wallet,
            1_761_920_200,
        ));
        assert_eq!(feed.rows.len(), baseline_count);

        feed.set_filter(ActivityFeedFilter::Wallet);
        assert!(
            feed.visible_rows()
                .into_iter()
                .all(|row| row.domain == ActivityEventDomain::Wallet)
        );

        feed.upsert_event(fixture_activity_event(
            "cad:event:1",
            ActivityEventDomain::Cad,
            1_761_920_260,
        ));
        feed.set_filter(ActivityFeedFilter::Cad);
        assert!(
            feed.visible_rows()
                .into_iter()
                .all(|row| row.domain == ActivityEventDomain::Cad)
        );
    }

    #[test]
    fn activity_feed_nip90_filter_limits_to_latest_fifty_events() {
        let mut feed = activity_feed_state_for_tests("nip90-limit");
        for index in 0..60_u64 {
            let mut row = fixture_activity_event(
                format!("nip90:req:{index}").as_str(),
                ActivityEventDomain::Network,
                1_761_920_700 + index,
            );
            row.source_tag = if index % 2 == 0 {
                "nip90.relay".to_string()
            } else {
                "nip90.publish".to_string()
            };
            feed.upsert_event(row);
        }
        for index in 0..5_u64 {
            let mut row = fixture_activity_event(
                format!("network:other:{index}").as_str(),
                ActivityEventDomain::Network,
                1_761_921_000 + index,
            );
            row.source_tag = "network.manual".to_string();
            feed.upsert_event(row);
        }

        feed.set_filter(ActivityFeedFilter::Nip90);
        let visible = feed.visible_rows();
        assert_eq!(feed.filtered_row_count(), 50);
        assert_eq!(feed.total_pages(), 7);
        assert_eq!(visible.len(), 8);
        assert!(
            visible
                .iter()
                .all(|row| row.source_tag.starts_with("nip90.")),
            "nip90 filter should exclude non-nip90 network rows"
        );
        assert_eq!(
            visible.first().map(|row| row.event_id.as_str()),
            Some("nip90:req:59")
        );
        assert_eq!(
            visible.last().map(|row| row.event_id.as_str()),
            Some("nip90:req:52")
        );
    }

    #[test]
    fn activity_feed_pagination_moves_through_filtered_rows() {
        let mut feed = activity_feed_state_for_tests("paging");
        for index in 0..18_u64 {
            feed.upsert_event(fixture_activity_event(
                format!("job:event:{index}").as_str(),
                ActivityEventDomain::Job,
                1_761_922_000 + index,
            ));
        }
        feed.set_filter(ActivityFeedFilter::Job);
        assert_eq!(feed.page, 0);
        assert_eq!(feed.total_pages(), 3);
        assert_eq!(
            feed.visible_rows().first().map(|row| row.event_id.as_str()),
            Some("job:event:17")
        );

        feed.next_page();
        assert_eq!(feed.page, 1);
        assert_eq!(
            feed.visible_rows().first().map(|row| row.event_id.as_str()),
            Some("job:event:9")
        );
        assert_eq!(feed.selected_event_id.as_deref(), Some("job:event:9"));

        feed.next_page();
        assert_eq!(feed.page, 2);
        assert_eq!(
            feed.visible_rows().first().map(|row| row.event_id.as_str()),
            Some("job:event:1")
        );
        assert_eq!(feed.selected_event_id.as_deref(), Some("job:event:1"));

        feed.next_page();
        assert_eq!(feed.page, 2);

        feed.previous_page();
        assert_eq!(feed.page, 1);
        assert_eq!(
            feed.visible_rows().first().map(|row| row.event_id.as_str()),
            Some("job:event:9")
        );
    }

    #[test]
    fn activity_feed_detail_scroll_clamps_and_resets_on_navigation() {
        let mut feed = activity_feed_state_for_tests("detail-scroll");
        for index in 0..12_u64 {
            let mut row = fixture_activity_event(
                format!("network:event:{index}").as_str(),
                ActivityEventDomain::Network,
                1_761_922_300 + index,
            );
            row.detail = (0..18)
                .map(|line| format!("line-{line} {}", row.event_id))
                .collect::<Vec<_>>()
                .join("\n");
            feed.upsert_event(row);
        }
        feed.set_filter(ActivityFeedFilter::Network);
        assert!(feed.select_visible_row(0));
        assert_eq!(feed.detail_scroll_line_offset, 0);

        let total_lines = 18;
        let visible_lines = 5;
        assert!(feed.scroll_detail_lines_by(9_999.0, total_lines, visible_lines));
        assert_eq!(
            feed.detail_scroll_offset_for(total_lines, visible_lines),
            13
        );
        assert!(feed.scroll_detail_lines_by(-9_999.0, total_lines, visible_lines));
        assert_eq!(feed.detail_scroll_line_offset, 0);

        assert!(feed.scroll_detail_lines_by(240.0, total_lines, visible_lines));
        assert!(feed.detail_scroll_line_offset > 0);
        assert!(feed.select_visible_row(1));
        assert_eq!(feed.detail_scroll_line_offset, 0);

        assert!(feed.scroll_detail_lines_by(240.0, total_lines, visible_lines));
        feed.next_page();
        assert_eq!(feed.detail_scroll_line_offset, 0);
        assert!(feed.scroll_detail_lines_by(240.0, total_lines, visible_lines));
        feed.previous_page();
        assert_eq!(feed.detail_scroll_line_offset, 0);
    }

    #[test]
    fn activity_feed_projection_rows_survive_restart() {
        let path = activity_feed_projection_test_path("restart");
        let _ = std::fs::remove_file(path.as_path());
        let mut feed = ActivityFeedState::from_projection_path_for_tests(path.clone());
        feed.upsert_event(fixture_activity_event(
            "chat:turn:42",
            ActivityEventDomain::Chat,
            1_761_920_301,
        ));
        feed.upsert_event(fixture_activity_event(
            "job:receipt:42",
            ActivityEventDomain::Job,
            1_761_920_311,
        ));
        let expected_rows = feed.rows.clone();

        let reloaded = ActivityFeedState::from_projection_path_for_tests(path.clone());
        assert_eq!(reloaded.rows, expected_rows);
        assert_eq!(
            reloaded.selected_event_id.as_deref(),
            reloaded.rows.first().map(|row| row.event_id.as_str())
        );
        let _ = std::fs::remove_file(path.as_path());
    }

    #[test]
    fn activity_feed_reload_projection_refreshes_from_projection_stream() {
        let path = activity_feed_projection_test_path("reload");
        let _ = std::fs::remove_file(path.as_path());
        let mut primary = ActivityFeedState::from_projection_path_for_tests(path.clone());
        primary.upsert_event(fixture_activity_event(
            "network:request:1",
            ActivityEventDomain::Network,
            1_761_920_340,
        ));

        let mut secondary = ActivityFeedState::from_projection_path_for_tests(path.clone());
        secondary.upsert_event(fixture_activity_event(
            "sync:checkpoint:2",
            ActivityEventDomain::Sync,
            1_761_920_360,
        ));

        assert!(
            primary
                .rows
                .iter()
                .all(|row| row.event_id != "sync:checkpoint:2")
        );
        primary
            .reload_projection()
            .expect("projection reload should reconcile rows");
        assert!(
            primary
                .rows
                .iter()
                .any(|row| row.event_id == "sync:checkpoint:2")
        );
        let _ = std::fs::remove_file(path.as_path());
    }

    #[test]
    fn earn_projection_replays_and_dedupes_job_lifecycle_rows() {
        let path = earn_projection_test_path("replay-dedupe");
        let _ = std::fs::remove_file(path.as_path());
        let mut projection =
            EarnJobLifecycleProjectionState::from_projection_path_for_tests(path.clone());
        let request = fixture_inbox_request(
            "req-projection",
            "summarize.text",
            700,
            120,
            JobInboxValidation::Valid,
        );
        projection.record_ingress_request(&request, 1_761_920_400, "nip90.relay.ingress");

        let mut inbox = seed_job_inbox(vec![request]);
        assert!(inbox.select_by_index(0));
        let selected = inbox
            .selected_request()
            .expect("selected request should exist")
            .clone();
        let mut active = ActiveJobState::default();
        active.start_from_request(&selected);
        let job = active.job.as_ref().expect("active job should exist");
        projection.record_active_job_stage(
            job,
            JobLifecycleStage::Accepted,
            1_761_920_420,
            "job.inbox.accept",
        );
        projection.record_active_job_stage(
            job,
            JobLifecycleStage::Accepted,
            1_761_920_421,
            "job.inbox.accept.duplicate",
        );

        let accepted_rows = projection
            .rows
            .iter()
            .filter(|row| row.stage == JobLifecycleStage::Accepted)
            .count();
        assert_eq!(accepted_rows, 1);

        let reloaded =
            EarnJobLifecycleProjectionState::from_projection_path_for_tests(path.clone());
        assert_eq!(reloaded.rows, projection.rows);
        assert_eq!(
            reloaded.authority,
            super::EARN_JOB_LIFECYCLE_PROJECTION_AUTHORITY
        );
        let _ = std::fs::remove_file(path.as_path());
    }

    #[test]
    fn earn_projection_marks_settlement_authority_without_changing_wallet_truth() {
        let path = earn_projection_test_path("authority");
        let _ = std::fs::remove_file(path.as_path());
        let mut projection =
            EarnJobLifecycleProjectionState::from_projection_path_for_tests(path.clone());

        let mut non_authoritative = fixture_history_row(
            "job-non-authoritative",
            JobHistoryStatus::Succeeded,
            1_761_920_450,
            50,
        );
        non_authoritative.payment_pointer = "pending:req-non-authoritative".to_string();
        projection.record_history_receipt(
            &non_authoritative,
            non_authoritative.completed_at_epoch_seconds,
            "earn.history.non_authoritative",
        );

        let mut authoritative = fixture_history_row(
            "job-authoritative",
            JobHistoryStatus::Succeeded,
            1_761_920_470,
            75,
        );
        authoritative.payment_pointer = "wallet-payment-777".to_string();
        projection.record_history_receipt(
            &authoritative,
            authoritative.completed_at_epoch_seconds,
            "earn.history.wallet_authoritative",
        );

        let non_authoritative_row = projection
            .rows
            .iter()
            .find(|row| row.job_id == "job-non-authoritative")
            .expect("non-authoritative projection row should exist");
        assert!(!non_authoritative_row.settlement_authoritative);
        assert_eq!(
            non_authoritative_row.settlement_authority,
            "projection.non_authoritative"
        );

        let authoritative_row = projection
            .rows
            .iter()
            .find(|row| row.job_id == "job-authoritative")
            .expect("authoritative projection row should exist");
        assert!(authoritative_row.settlement_authoritative);
        assert_eq!(
            authoritative_row.settlement_authority,
            "wallet.reconciliation"
        );
        let _ = std::fs::remove_file(path.as_path());
    }

    #[test]
    fn alerts_recovery_lifecycle_transitions_are_deterministic() {
        let mut alerts = AlertsRecoveryState::default();
        alerts.alerts.push(fixture_alert(
            "alert:identity:missing",
            AlertDomain::Identity,
            super::AlertSeverity::Critical,
            AlertLifecycle::Active,
        ));
        alerts.select_by_index(0);
        let alert_id = alerts
            .acknowledge_selected()
            .expect("active alert should acknowledge");
        let alert = alerts
            .alerts
            .iter()
            .find(|alert| alert.alert_id == alert_id)
            .expect("alert should exist after ack");
        assert_eq!(alert.lifecycle, AlertLifecycle::Acknowledged);
        assert_eq!(alerts.selected_domain(), Some(AlertDomain::Identity));

        let resolved_id = alerts
            .resolve_selected()
            .expect("acknowledged alert should resolve");
        let resolved = alerts
            .alerts
            .iter()
            .find(|alert| alert.alert_id == resolved_id)
            .expect("alert should exist after resolve");
        assert_eq!(resolved.lifecycle, AlertLifecycle::Resolved);
    }

    #[test]
    fn settings_updates_validate_ranges_and_reconnect_notice() {
        let mut settings = SettingsState::default();
        settings
            .apply_updates_internal("wss://relay.primal.net", "2500", "1", false)
            .expect("valid settings update should apply");
        assert_eq!(
            settings.document.primary_relay_url,
            "wss://relay.primal.net"
        );
        assert_eq!(
            settings
                .document
                .backup_relay_urls
                .first()
                .map(String::as_str),
            Some(super::DEFAULT_NEXUS_PRIMARY_RELAY_URL)
        );
        assert_eq!(settings.document.wallet_default_send_sats, 2500);
        assert_eq!(settings.document.provider_max_queue_depth, 1);
        assert!(settings.document.reconnect_required);

        let invalid = settings.apply_updates_internal("https://bad-relay", "0", "0", false);
        assert!(invalid.is_err());
        assert_eq!(settings.load_state, super::PaneLoadState::Error);
    }

    #[test]
    fn settings_document_default_uses_identity_authority_path() {
        let document = super::SettingsDocumentV1::default();
        assert!(document.identity_path.contains("identity.mnemonic"));
        assert_eq!(
            document.primary_relay_url,
            super::DEFAULT_NEXUS_PRIMARY_RELAY_URL
        );
        assert_eq!(document.backup_relay_urls.len(), 2);
    }

    #[test]
    fn parse_settings_document_overrides_stale_identity_path() {
        let raw = "schema_version=1\nrelay_url=wss://relay.example\nidentity_path=~/.openagents/nostr/identity.json\nwallet_default_send_sats=1000\nprovider_max_queue_depth=4\nreconnect_required=false\n";
        let document = super::parse_settings_document(raw).expect("settings parse should succeed");
        assert_ne!(document.identity_path, "~/.openagents/nostr/identity.json");
        assert!(document.identity_path.contains("identity.mnemonic"));
        assert_eq!(document.provider_max_queue_depth, 1);
        assert_eq!(document.primary_relay_url, "wss://relay.example");
        assert!(document.backup_relay_urls.is_empty());
    }

    #[test]
    fn parse_settings_document_preserves_loopback_ws_relay_for_packaged_verification() {
        let raw = "schema_version=2\nprimary_relay_url=ws://127.0.0.1:18490\nbackup_relay_urls=\nidentity_path=~/.openagents/pylon/identity.mnemonic\nwallet_default_send_sats=1000\nprovider_max_queue_depth=1\nreconnect_required=false\n";
        let document = super::parse_settings_document(raw).expect("settings parse should succeed");
        assert_eq!(document.primary_relay_url, "ws://127.0.0.1:18490");
        assert!(document.backup_relay_urls.is_empty());
        assert_eq!(
            document.configured_relay_urls(),
            vec!["ws://127.0.0.1:18490".to_string()]
        );
    }

    #[test]
    fn settings_document_configured_relay_urls_keep_primary_first() {
        let document = super::SettingsDocumentV1 {
            primary_relay_url: "wss://nexus.openagents.com/".to_string(),
            backup_relay_urls: vec![
                "wss://relay.primal.net".to_string(),
                "wss://nexus.openagents.com/".to_string(),
                "wss://relay.damus.io".to_string(),
            ],
            ..super::SettingsDocumentV1::default()
        };
        assert_eq!(
            document.configured_relay_urls(),
            vec![
                "wss://nexus.openagents.com/".to_string(),
                "wss://relay.primal.net".to_string(),
                "wss://relay.damus.io".to_string()
            ]
        );
    }

    #[test]
    fn earnings_scoreboard_refreshes_from_wallet_and_history() {
        let mut score = EarningsScoreboardState::default();
        let provider = ProviderRuntimeState::default();
        let mut row = fixture_history_row(
            "job-earned-001",
            JobHistoryStatus::Succeeded,
            1_761_919_970,
            2100,
        );
        row.payment_pointer = "wallet-payment-001".to_string();
        let history = seed_job_history(vec![row]);
        let mut spark = SparkPaneState::default();
        spark.balance = Some(openagents_spark::Balance {
            spark_sats: 1000,
            lightning_sats: 2000,
            onchain_sats: 3000,
        });
        spark
            .recent_payments
            .push(openagents_spark::PaymentSummary {
                id: "wallet-payment-001".to_string(),
                direction: "receive".to_string(),
                status: "succeeded".to_string(),
                amount_sats: 2100,
                timestamp: history.reference_epoch_seconds,
                ..Default::default()
            });

        let now = std::time::Instant::now();
        score.refresh_from_sources(now, &provider, &history, &spark);

        assert_eq!(score.load_state, super::PaneLoadState::Ready);
        assert_eq!(score.lifetime_sats, 2100);
        assert_eq!(score.jobs_today, 1);
        assert_eq!(score.sats_today, 2100);
        assert_eq!(score.sats_this_month, 2100);
        assert!(!score.is_stale(now));
    }

    #[test]
    fn earnings_scoreboard_ignores_unreconciled_history_rows() {
        let mut score = EarningsScoreboardState::default();
        let provider = ProviderRuntimeState::default();
        let mut row = fixture_history_row(
            "job-unreconciled-001",
            JobHistoryStatus::Succeeded,
            1_761_919_970,
            4200,
        );
        row.payment_pointer = "wallet-payment-missing".to_string();
        let history = seed_job_history(vec![row]);
        let mut spark = SparkPaneState::default();
        spark.balance = Some(openagents_spark::Balance {
            spark_sats: 10_000,
            lightning_sats: 0,
            onchain_sats: 0,
        });

        score.refresh_from_sources(std::time::Instant::now(), &provider, &history, &spark);

        assert_eq!(score.load_state, super::PaneLoadState::Ready);
        assert_eq!(score.lifetime_sats, 0);
        assert_eq!(score.jobs_today, 0);
        assert_eq!(score.sats_today, 0);
        assert_eq!(score.sats_this_month, 0);
    }

    #[test]
    fn earnings_scoreboard_tracks_monthly_reconciled_payouts() {
        let mut score = EarningsScoreboardState::default();
        let provider = ProviderRuntimeState::default();
        let current_month = chrono::Utc
            .with_ymd_and_hms(2026, 3, 10, 12, 0, 0)
            .single()
            .expect("valid current month timestamp")
            .timestamp() as u64;
        let previous_month = chrono::Utc
            .with_ymd_and_hms(2026, 2, 27, 12, 0, 0)
            .single()
            .expect("valid previous month timestamp")
            .timestamp() as u64;
        let reference = chrono::Utc
            .with_ymd_and_hms(2026, 3, 15, 8, 0, 0)
            .single()
            .expect("valid reference timestamp")
            .timestamp() as u64;

        let mut current_row = fixture_history_row(
            "job-month-001",
            JobHistoryStatus::Succeeded,
            current_month,
            1500,
        );
        current_row.payment_pointer = "wallet-month-001".to_string();
        let mut previous_row = fixture_history_row(
            "job-month-002",
            JobHistoryStatus::Succeeded,
            previous_month,
            900,
        );
        previous_row.payment_pointer = "wallet-month-002".to_string();
        let mut history = seed_job_history(vec![current_row, previous_row]);
        history.reference_epoch_seconds = reference;

        let mut spark = SparkPaneState::default();
        spark.balance = Some(openagents_spark::Balance {
            spark_sats: 10_000,
            lightning_sats: 0,
            onchain_sats: 0,
        });
        spark
            .recent_payments
            .push(openagents_spark::PaymentSummary {
                id: "wallet-month-001".to_string(),
                direction: "receive".to_string(),
                status: "settled".to_string(),
                amount_sats: 1500,
                timestamp: current_month,
                ..Default::default()
            });
        spark
            .recent_payments
            .push(openagents_spark::PaymentSummary {
                id: "wallet-month-002".to_string(),
                direction: "receive".to_string(),
                status: "settled".to_string(),
                amount_sats: 900,
                timestamp: previous_month,
                ..Default::default()
            });

        score.refresh_from_sources(std::time::Instant::now(), &provider, &history, &spark);

        assert_eq!(score.lifetime_sats, 2400);
        assert_eq!(score.sats_this_month, 1500);
    }

    #[test]
    fn earnings_scoreboard_surfaces_wallet_errors() {
        let mut score = EarningsScoreboardState::default();
        let provider = ProviderRuntimeState::default();
        let history = JobHistoryState::default();
        let mut spark = SparkPaneState::default();
        spark.last_error = Some("wallet backend unavailable".to_string());

        score.refresh_from_sources(std::time::Instant::now(), &provider, &history, &spark);

        assert_eq!(score.load_state, super::PaneLoadState::Error);
        assert!(
            score
                .last_error
                .as_deref()
                .is_some_and(|error| error.contains("wallet backend unavailable"))
        );
    }

    #[test]
    fn earnings_scoreboard_tracks_loop_integrity_slo_metrics() {
        let mut score = EarningsScoreboardState::default();
        let now = std::time::Instant::now();
        let mut provider = ProviderRuntimeState::default();
        provider.online_since = Some(now - std::time::Duration::from_secs(120));
        provider.last_completed_job_at = Some(now - std::time::Duration::from_secs(90));

        let mut succeeded_row = fixture_history_row(
            "job-loop-metric-001",
            JobHistoryStatus::Succeeded,
            1_761_919_970,
            2_100,
        );
        succeeded_row.payment_pointer = "wallet-loop-metric-001".to_string();
        let failed_row = fixture_history_row(
            "job-loop-metric-002",
            JobHistoryStatus::Failed,
            1_761_919_980,
            0,
        );
        let history = seed_job_history(vec![succeeded_row, failed_row]);

        let mut spark = SparkPaneState::default();
        spark.balance = Some(openagents_spark::Balance {
            spark_sats: 10_000,
            lightning_sats: 0,
            onchain_sats: 0,
        });
        spark
            .recent_payments
            .push(openagents_spark::PaymentSummary {
                id: "wallet-loop-metric-001".to_string(),
                direction: "receive".to_string(),
                status: "settled".to_string(),
                amount_sats: 2_100,
                timestamp: 1_761_920_000,
                ..Default::default()
            });

        score.refresh_from_sources(now, &provider, &history, &spark);

        assert_eq!(score.first_job_latency_seconds, Some(30));
        assert_eq!(score.completion_ratio_bps, Some(5_000));
        assert_eq!(score.payout_success_ratio_bps, Some(10_000));
        assert_eq!(score.avg_wallet_confirmation_latency_seconds, Some(30));
    }

    #[test]
    fn earnings_scoreboard_tracks_pending_first_job_latency_before_completion() {
        let mut score = EarningsScoreboardState::default();
        let now = std::time::Instant::now();
        let mut provider = ProviderRuntimeState::default();
        provider.online_since = Some(now - std::time::Duration::from_secs(70));

        let history = JobHistoryState::default();
        let mut spark = SparkPaneState::default();
        spark.balance = Some(openagents_spark::Balance {
            spark_sats: 100,
            lightning_sats: 0,
            onchain_sats: 0,
        });

        score.refresh_from_sources(now, &provider, &history, &spark);

        assert_eq!(score.first_job_latency_seconds, Some(70));
        assert_eq!(score.completion_ratio_bps, None);
        assert_eq!(score.payout_success_ratio_bps, None);
        assert_eq!(score.avg_wallet_confirmation_latency_seconds, None);
    }

    #[test]
    fn mission_control_log_lines_use_grouped_integer_projection_amounts() {
        let now_epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(1_762_000_000);
        let provider = ProviderRuntimeState::default();
        let projection = EarnJobLifecycleProjectionState {
            load_state: super::PaneLoadState::Ready,
            last_error: None,
            last_action: None,
            stream_id: "stream.earn_job_lifecycle_projection.v1".to_string(),
            authority: "non-authoritative".to_string(),
            rows: vec![EarnJobLifecycleProjectionRow {
                stream_seq: 1,
                event_id: "earn.lifecycle:job-123:paid:wallet".to_string(),
                job_id: "job-123".to_string(),
                request_id: "123".to_string(),
                stage: JobLifecycleStage::Paid,
                source_tag: "starter-demand".to_string(),
                occurred_at_epoch_seconds: now_epoch.saturating_sub(60),
                quoted_price_sats: 1_000,
                payment_pointer: Some("wallet:123".to_string()),
                settlement_authority: "wallet.reconciliation".to_string(),
                settlement_authoritative: true,
            }],
            projection_file_path: earn_projection_test_path("mission-control-log"),
        };

        let (lines, _) = super::build_mission_control_log_lines(
            Some("Mission Control ready"),
            None,
            crate::desktop_shell::DesktopShellMode::Production,
            &provider,
            &super::LocalInferenceExecutionSnapshot::default(),
            &[],
            &projection,
            &SparkPaneState::default(),
            &NetworkRequestsState::default(),
            &JobInboxState::default(),
            &ActiveJobState::default(),
        );

        assert!(
            lines
                .iter()
                .any(|line| line.stream == TerminalStream::Stdout
                    && line.text.contains("\u{20BF} 1 000"))
        );
    }

    #[test]
    fn mission_control_supported_production_lane_stays_apple_fm_only() {
        let local = super::LocalInferenceExecutionSnapshot {
            reachable: true,
            ready_model: Some("gpt-oss-20b".to_string()),
            backend_label: "cuda".to_string(),
            ..super::LocalInferenceExecutionSnapshot::default()
        };

        assert_eq!(
            super::mission_control_local_runtime_lane_for_platform(
                true,
                crate::desktop_shell::DesktopShellMode::Production,
                &local
            ),
            Some(super::MissionControlLocalRuntimeLane::AppleFoundationModels)
        );
    }

    #[test]
    fn mission_control_unsupported_production_lane_hides_local_model_controls() {
        let local = super::LocalInferenceExecutionSnapshot {
            reachable: true,
            ready_model: Some("gpt-oss-20b".to_string()),
            backend_label: "cuda".to_string(),
            ..super::LocalInferenceExecutionSnapshot::default()
        };

        assert_eq!(
            super::mission_control_local_runtime_lane_for_platform(
                false,
                crate::desktop_shell::DesktopShellMode::Production,
                &local
            ),
            None
        );
    }

    #[test]
    fn unsupported_sell_platform_log_lines_hide_runtime_noise() {
        if super::mission_control_uses_apple_fm() {
            return;
        }

        let mut provider = ProviderRuntimeState::default();
        provider.last_result =
            Some("apple foundation models capability pending: Apple FM unavailable".to_string());

        let local = super::LocalInferenceExecutionSnapshot {
            reachable: true,
            ready_model: Some("gpt-oss-20b".to_string()),
            backend_label: "cuda".to_string(),
            ..super::LocalInferenceExecutionSnapshot::default()
        };

        let (lines, _) = super::build_mission_control_log_lines(
            None,
            None,
            crate::desktop_shell::DesktopShellMode::Production,
            &provider,
            &local,
            &[ProviderBlocker::AppleFoundationModelsUnavailable],
            &EarnJobLifecycleProjectionState::default(),
            &SparkPaneState::default(),
            &NetworkRequestsState::default(),
            &JobInboxState::default(),
            &ActiveJobState::default(),
        );

        assert!(lines.iter().any(|line| {
            line.text == "Provider offline. Platform not supported for selling compute."
        }));
        assert!(
            !lines
                .iter()
                .any(|line| line.text.contains("Start Apple FM"))
        );
        assert!(
            !lines
                .iter()
                .any(|line| line.text.contains("capability pending"))
        );
        assert!(
            !lines
                .iter()
                .any(|line| line.text.contains("Preflight blocker"))
        );
    }

    #[test]
    fn mission_control_production_log_lines_do_not_fall_back_to_gpt_oss() {
        let mut provider = ProviderRuntimeState::default();
        provider.apple_fm.last_error =
            Some("Apple Foundation Models requires macOS 26+ on Apple Silicon".to_string());

        let local = super::LocalInferenceExecutionSnapshot {
            reachable: true,
            ready_model: Some("gpt-oss-20b".to_string()),
            backend_label: "cuda".to_string(),
            ..super::LocalInferenceExecutionSnapshot::default()
        };

        let (lines, _) = super::build_mission_control_log_lines(
            None,
            None,
            crate::desktop_shell::DesktopShellMode::Production,
            &provider,
            &local,
            &[],
            &EarnJobLifecycleProjectionState::default(),
            &SparkPaneState::default(),
            &NetworkRequestsState::default(),
            &JobInboxState::default(),
            &ActiveJobState::default(),
        );

        if super::mission_control_uses_apple_fm() {
            assert!(
                lines
                    .iter()
                    .any(|line| line.text.contains("Apple Foundation Models unavailable"))
            );
        } else {
            assert!(lines.iter().any(|line| {
                line.text == "Provider offline. Platform not supported for selling compute."
            }));
            assert!(
                !lines
                    .iter()
                    .any(|line| { line.text.contains("Apple Foundation Models unavailable") })
            );
        }
        assert!(!lines.iter().any(|line| line.text.contains("GPT-OSS")));
    }

    #[test]
    fn mission_control_log_lines_ignore_positive_apple_fm_health_message() {
        let mut provider = ProviderRuntimeState::default();
        provider.apple_fm.reachable = true;
        provider.apple_fm.model_available = true;
        provider.apple_fm.availability_message = Some("Foundation Models is available".to_string());

        let local = super::LocalInferenceExecutionSnapshot {
            reachable: true,
            ready_model: Some("gpt-oss-20b".to_string()),
            backend_label: "cuda".to_string(),
            ..super::LocalInferenceExecutionSnapshot::default()
        };

        let (lines, _) = super::build_mission_control_log_lines(
            None,
            None,
            crate::desktop_shell::DesktopShellMode::Production,
            &provider,
            &local,
            &[],
            &EarnJobLifecycleProjectionState::default(),
            &SparkPaneState::default(),
            &NetworkRequestsState::default(),
            &JobInboxState::default(),
            &ActiveJobState::default(),
        );

        assert!(
            lines
                .iter()
                .any(|line| line.text.contains("bridge reachable but not ready yet"))
        );
        assert!(
            !lines
                .iter()
                .any(|line| line.text.contains("Apple Foundation Models unavailable"))
        );
    }

    #[test]
    fn mission_control_log_lines_include_recent_buyer_request_status() {
        let mut requests = NetworkRequestsState::default();
        let request_id = requests
            .queue_request_submission(NetworkRequestSubmission {
                request_id: Some("req-buy-mode-1234567890".to_string()),
                request_type: crate::app_state::MISSION_CONTROL_BUY_MODE_REQUEST_TYPE.to_string(),
                payload: "Buy Mode test payload".to_string(),
                resolution_mode: BuyerResolutionMode::Race,
                target_provider_pubkeys: Vec::new(),
                skill_scope_id: None,
                credit_envelope_ref: None,
                budget_sats: crate::app_state::MISSION_CONTROL_BUY_MODE_BUDGET_SATS,
                timeout_seconds: crate::app_state::MISSION_CONTROL_BUY_MODE_TIMEOUT_SECONDS,
                authority_command_seq: 7,
            })
            .expect("queue buyer request");
        requests.apply_nip90_request_publish_outcome(
            request_id.as_str(),
            "event-buy-mode-abcdef123456",
            1,
            0,
            None,
        );

        let (lines, _) = super::build_mission_control_log_lines(
            None,
            None,
            crate::desktop_shell::DesktopShellMode::Production,
            &ProviderRuntimeState::default(),
            &super::LocalInferenceExecutionSnapshot::default(),
            &[],
            &EarnJobLifecycleProjectionState::default(),
            &SparkPaneState::default(),
            &requests,
            &JobInboxState::default(),
            &ActiveJobState::default(),
        );

        assert!(lines.iter().any(|line| {
            line.text.contains("Buyer req-buy-mode")
                && line.text.contains("buy_mode")
                && line.text.contains("work=awaiting-provider")
                && line.text.contains("payment=idle")
        }));
    }

    #[test]
    fn mission_control_log_lines_surface_buyer_payment_blockers() {
        let mut requests = NetworkRequestsState::default();
        let request_id = requests
            .queue_request_submission(NetworkRequestSubmission {
                request_id: Some("req-buy-blocker-1234567890".to_string()),
                request_type: crate::app_state::MISSION_CONTROL_BUY_MODE_REQUEST_TYPE.to_string(),
                payload: "Buy Mode blocker payload".to_string(),
                resolution_mode: BuyerResolutionMode::Race,
                target_provider_pubkeys: Vec::new(),
                skill_scope_id: None,
                credit_envelope_ref: None,
                budget_sats: crate::app_state::MISSION_CONTROL_BUY_MODE_BUDGET_SATS,
                timeout_seconds: crate::app_state::MISSION_CONTROL_BUY_MODE_TIMEOUT_SECONDS,
                authority_command_seq: 9,
            })
            .expect("queue buyer request");
        requests.apply_nip90_request_publish_outcome(
            request_id.as_str(),
            "event-buy-blocker-abcdef123456",
            1,
            0,
            None,
        );
        requests.apply_nip90_buyer_result_event(
            request_id.as_str(),
            "77".repeat(32).as_str(),
            "result-buy-blocker-001",
            Some("success"),
        );

        let (lines, _) = super::build_mission_control_log_lines(
            None,
            None,
            crate::desktop_shell::DesktopShellMode::Production,
            &ProviderRuntimeState::default(),
            &super::LocalInferenceExecutionSnapshot::default(),
            &[],
            &EarnJobLifecycleProjectionState::default(),
            &SparkPaneState::default(),
            &requests,
            &JobInboxState::default(),
            &ActiveJobState::default(),
        );

        assert!(lines.iter().any(|line| {
            line.text.contains("blocker_codes=result_without_invoice")
                && line
                    .text
                    .contains("blocker=result provider 777777..7777 has no valid invoice")
        }));
    }

    #[test]
    fn mission_control_log_lines_include_buyer_payment_fee_details() {
        let mut requests = NetworkRequestsState::default();
        let request_id = requests
            .queue_request_submission(NetworkRequestSubmission {
                request_id: Some("req-buy-fee-1234567890".to_string()),
                request_type: crate::app_state::MISSION_CONTROL_BUY_MODE_REQUEST_TYPE.to_string(),
                payload: "Buy Mode fee test payload".to_string(),
                resolution_mode: BuyerResolutionMode::Race,
                target_provider_pubkeys: Vec::new(),
                skill_scope_id: None,
                credit_envelope_ref: None,
                budget_sats: crate::app_state::MISSION_CONTROL_BUY_MODE_BUDGET_SATS,
                timeout_seconds: crate::app_state::MISSION_CONTROL_BUY_MODE_TIMEOUT_SECONDS,
                authority_command_seq: 8,
            })
            .expect("queue buyer request");
        requests.apply_nip90_request_publish_outcome(
            request_id.as_str(),
            "event-buy-fee-abcdef123456",
            1,
            0,
            None,
        );
        requests.apply_nip90_buyer_feedback_event(
            request_id.as_str(),
            "55".repeat(32).as_str(),
            "feedback-buy-fee-001",
            Some("payment-required"),
            Some("invoice ready"),
            Some(2_000),
            Some("lnbc1buyfeeinvoice"),
        );
        requests
            .prepare_auto_payment_attempt(
                request_id.as_str(),
                "lnbc1buyfeeinvoice",
                Some(2_000),
                1_762_700_222,
            )
            .expect("payment-required invoice should prepare");
        requests.record_auto_payment_pointer(request_id.as_str(), "wallet-buy-fee-001");
        requests.mark_auto_payment_sent(request_id.as_str(), "wallet-buy-fee-001", 1_762_700_223);

        let mut wallet = SparkPaneState::default();
        wallet
            .recent_payments
            .push(openagents_spark::PaymentSummary {
                id: "wallet-buy-fee-001".to_string(),
                direction: "send".to_string(),
                status: "succeeded".to_string(),
                amount_sats: 2,
                fees_sats: 3,
                timestamp: 1_762_700_223,
                ..Default::default()
            });

        let (lines, _) = super::build_mission_control_log_lines(
            None,
            None,
            crate::desktop_shell::DesktopShellMode::Production,
            &ProviderRuntimeState::default(),
            &super::LocalInferenceExecutionSnapshot::default(),
            &[],
            &EarnJobLifecycleProjectionState::default(),
            &wallet,
            &requests,
            &JobInboxState::default(),
            &ActiveJobState::default(),
        );

        assert!(lines.iter().any(|line| {
            line.text.contains("Buyer req-buy-fee")
                && line.text.contains("payment=sent")
                && line.text.contains("invoice_sats=2")
                && line.text.contains("fee_sats=3")
                && line.text.contains("wallet_debit_sats=5")
                && line.text.contains("wallet_delta_sats=-5")
        }));
    }

    #[test]
    fn mission_control_log_lines_include_provider_settlement_fee_details() {
        let request = crate::state::job_inbox::JobInboxRequest {
            request_id: "req-active-log-fee".to_string(),
            requester: "npub1requester".to_string(),
            demand_source: JobDemandSource::OpenNetwork,
            request_kind: 5050,
            capability: "text.generation".to_string(),
            execution_input: None,
            execution_prompt: Some("BUY MODE OK".to_string()),
            execution_params: Vec::new(),
            requested_model: None,
            requested_output_mime: None,
            skill_scope_id: None,
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_request_event_id: None,
            sa_tick_result_event_id: None,
            ac_envelope_event_id: None,
            price_sats: 2,
            ttl_seconds: 75,
            validation: JobInboxValidation::Valid,
            arrival_seq: 1,
            decision: JobInboxDecision::Pending,
        };
        let mut active_job = ActiveJobState::default();
        active_job.start_from_request(&request);
        let job = active_job.job.as_mut().expect("active job exists");
        job.stage = JobLifecycleStage::Paid;
        job.payment_id = Some("wallet-provider-log-fee-001".to_string());

        let mut wallet = SparkPaneState::default();
        wallet
            .recent_payments
            .push(openagents_spark::PaymentSummary {
                id: "wallet-provider-log-fee-001".to_string(),
                direction: "receive".to_string(),
                status: "succeeded".to_string(),
                amount_sats: 2,
                fees_sats: 1,
                method: "lightning".to_string(),
                timestamp: 1_762_700_779,
                ..Default::default()
            });

        let (lines, _) = super::build_mission_control_log_lines(
            None,
            None,
            crate::desktop_shell::DesktopShellMode::Production,
            &ProviderRuntimeState::default(),
            &super::LocalInferenceExecutionSnapshot::default(),
            &[],
            &EarnJobLifecycleProjectionState::default(),
            &wallet,
            &NetworkRequestsState::default(),
            &JobInboxState::default(),
            &active_job,
        );

        assert!(lines.iter().any(|line| {
            line.text.contains("Active job-req-active-log-fee")
                && line.text.contains("settlement_sats=2")
                && line.text.contains("settlement_fee_sats=1")
                && line.text.contains("wallet_delta_sats=2")
        }));
    }

    #[test]
    fn mission_control_sell_compute_supported_requires_supported_platform_or_dev_cuda_lane() {
        let cuda_local = super::LocalInferenceExecutionSnapshot {
            reachable: true,
            ready_model: Some("gpt-oss-20b".to_string()),
            backend_label: "cuda".to_string(),
            ..super::LocalInferenceExecutionSnapshot::default()
        };
        let metal_local = super::LocalInferenceExecutionSnapshot {
            reachable: true,
            ready_model: Some("gpt-oss-20b".to_string()),
            backend_label: "metal".to_string(),
            ..super::LocalInferenceExecutionSnapshot::default()
        };

        assert!(super::mission_control_sell_compute_supported_for_platform(
            true,
            crate::desktop_shell::DesktopShellMode::Production,
            &metal_local,
        ));
        assert!(!super::mission_control_sell_compute_supported_for_platform(
            false,
            crate::desktop_shell::DesktopShellMode::Production,
            &cuda_local,
        ));
        assert!(super::mission_control_sell_compute_supported_for_platform(
            false,
            crate::desktop_shell::DesktopShellMode::Dev,
            &cuda_local,
        ));
        assert!(!super::mission_control_sell_compute_supported_for_platform(
            false,
            crate::desktop_shell::DesktopShellMode::Dev,
            &metal_local,
        ));
    }

    #[test]
    fn mission_control_buy_mode_loop_toggle_and_schedule_are_deterministic() {
        let mut mission_control = super::MissionControlPaneState::default();
        let now = std::time::Instant::now();

        assert!(!mission_control.buy_mode_loop_enabled);
        assert!(!mission_control.buy_mode_dispatch_due(now));

        assert!(mission_control.toggle_buy_mode_loop(now));
        assert!(mission_control.buy_mode_dispatch_due(now));
        assert_eq!(
            mission_control.buy_mode_next_dispatch_countdown_seconds(now),
            Some(0)
        );

        mission_control.schedule_next_buy_mode_dispatch(now);
        assert_eq!(
            mission_control.buy_mode_next_dispatch_countdown_millis(now),
            Some(super::MISSION_CONTROL_BUY_MODE_INTERVAL_MILLIS)
        );
        assert!(
            !mission_control.buy_mode_dispatch_due(
                now + super::MISSION_CONTROL_BUY_MODE_INTERVAL
                    .saturating_sub(std::time::Duration::from_millis(1))
            )
        );

        assert!(!mission_control.toggle_buy_mode_loop(now));
        assert!(!mission_control.buy_mode_loop_enabled);
        assert_eq!(mission_control.buy_mode_next_dispatch_at, None);
    }

    #[test]
    fn mission_control_buy_mode_start_block_reason_requires_wallet_balance() {
        let wallet = SparkPaneState::default();
        assert_eq!(
            super::mission_control_buy_mode_start_block_reason(&wallet),
            Some(format!(
                "Buy Mode requires at least {} sats in Spark wallet (balance unavailable)",
                super::MISSION_CONTROL_BUY_MODE_BUDGET_SATS
            ))
        );

        let mut empty_balance = SparkPaneState::default();
        empty_balance.balance = Some(openagents_spark::Balance {
            spark_sats: 0,
            lightning_sats: 0,
            onchain_sats: 0,
        });
        assert_eq!(
            super::mission_control_buy_mode_start_block_reason(&empty_balance),
            Some(format!(
                "Buy Mode requires at least {} sats in Spark wallet (balance: 0 sats)",
                super::MISSION_CONTROL_BUY_MODE_BUDGET_SATS
            ))
        );

        let mut funded = SparkPaneState::default();
        funded.balance = Some(openagents_spark::Balance {
            spark_sats: super::MISSION_CONTROL_BUY_MODE_BUDGET_SATS,
            lightning_sats: 0,
            onchain_sats: 0,
        });
        assert_eq!(
            super::mission_control_buy_mode_start_block_reason(&funded),
            None
        );
    }

    #[test]
    fn network_aggregate_counters_refreshes_from_wallet_reconciled_payouts() {
        let mut counters = NetworkAggregateCountersState::default();
        let presence = fixture_presence_snapshot(7, "online", None, None);
        let mut row = fixture_history_row(
            "job-network-aggregate-001",
            JobHistoryStatus::Succeeded,
            1_761_919_970,
            2100,
        );
        row.payment_pointer = "wallet-payment-aggregate-001".to_string();
        let history = seed_job_history(vec![row]);
        let mut spark = SparkPaneState::default();
        spark.balance = Some(openagents_spark::Balance {
            spark_sats: 1000,
            lightning_sats: 2000,
            onchain_sats: 3000,
        });
        spark
            .recent_payments
            .push(openagents_spark::PaymentSummary {
                id: "wallet-payment-aggregate-001".to_string(),
                direction: "receive".to_string(),
                status: "settled".to_string(),
                amount_sats: 2100,
                timestamp: history.reference_epoch_seconds,
                ..Default::default()
            });

        let now = std::time::Instant::now();
        counters.refresh_from_sources(now, &presence, &history, &spark);

        assert_eq!(counters.load_state, super::PaneLoadState::Ready);
        assert_eq!(
            counters.source_tag,
            "aggregate.wallet-reconciled.spacetime-presence"
        );
        assert_eq!(counters.providers_online, 7);
        assert_eq!(
            counters.providers_online_source_tag,
            "spacetime.presence.identity"
        );
        assert_eq!(counters.jobs_completed, 1);
        assert_eq!(counters.sats_paid, 2100);
        assert_eq!(counters.global_earnings_today_sats, 2100);
        assert!(!counters.is_stale(now));
    }

    #[test]
    fn network_aggregate_counters_ignore_unreconciled_history_rows() {
        let mut counters = NetworkAggregateCountersState::default();
        let presence = fixture_presence_snapshot(3, "online", None, None);
        let mut row = fixture_history_row(
            "job-network-aggregate-002",
            JobHistoryStatus::Succeeded,
            1_761_919_970,
            4200,
        );
        row.payment_pointer = "wallet-payment-missing".to_string();
        let history = seed_job_history(vec![row]);
        let mut spark = SparkPaneState::default();
        spark.balance = Some(openagents_spark::Balance {
            spark_sats: 4_200,
            lightning_sats: 0,
            onchain_sats: 0,
        });

        counters.refresh_from_sources(std::time::Instant::now(), &presence, &history, &spark);

        assert_eq!(counters.load_state, super::PaneLoadState::Ready);
        assert_eq!(counters.providers_online, 3);
        assert_eq!(counters.jobs_completed, 0);
        assert_eq!(counters.sats_paid, 0);
        assert_eq!(counters.global_earnings_today_sats, 0);
    }

    #[test]
    fn network_aggregate_counters_surface_wallet_errors() {
        let mut counters = NetworkAggregateCountersState::default();
        let presence = fixture_presence_snapshot(2, "online", None, None);
        let history = JobHistoryState::default();
        let mut spark = SparkPaneState::default();
        spark.last_error = Some("wallet service unavailable".to_string());

        counters.refresh_from_sources(std::time::Instant::now(), &presence, &history, &spark);

        assert_eq!(counters.load_state, super::PaneLoadState::Error);
        assert_eq!(counters.source_tag, "aggregate.degraded.wallet");
        assert!(
            counters
                .last_error
                .as_deref()
                .is_some_and(|error| error.contains("wallet service unavailable"))
        );
    }

    #[test]
    fn network_aggregate_counters_surface_spacetime_presence_degraded_state() {
        let mut counters = NetworkAggregateCountersState::default();
        let presence =
            fixture_presence_snapshot(0, "unregistered", Some("presence query timeout"), None);
        let history = JobHistoryState::default();
        let mut spark = SparkPaneState::default();
        spark.balance = Some(openagents_spark::Balance {
            spark_sats: 0,
            lightning_sats: 0,
            onchain_sats: 0,
        });

        counters.refresh_from_sources(std::time::Instant::now(), &presence, &history, &spark);

        assert_eq!(counters.load_state, super::PaneLoadState::Error);
        assert_eq!(counters.source_tag, "aggregate.degraded.spacetime-presence");
        assert_eq!(
            counters.providers_online_source_tag,
            "spacetime.presence.degraded"
        );
        assert!(
            counters
                .last_error
                .as_deref()
                .is_some_and(|error| error.contains("presence query timeout"))
        );
    }

    #[test]
    fn network_aggregate_counters_surface_spacetime_presence_stale_state() {
        let mut counters = NetworkAggregateCountersState::default();
        let presence = fixture_presence_snapshot(0, "offline", None, Some("ttl_expired"));
        let history = JobHistoryState::default();
        let mut spark = SparkPaneState::default();
        spark.balance = Some(openagents_spark::Balance {
            spark_sats: 0,
            lightning_sats: 0,
            onchain_sats: 0,
        });

        counters.refresh_from_sources(std::time::Instant::now(), &presence, &history, &spark);

        assert_eq!(counters.load_state, super::PaneLoadState::Ready);
        assert_eq!(counters.source_tag, "aggregate.stale.spacetime-presence");
        assert_eq!(
            counters.providers_online_source_tag,
            "spacetime.presence.stale"
        );
        assert!(
            counters
                .last_action
                .as_deref()
                .is_some_and(|action| action.contains("stale"))
        );
    }

    #[test]
    fn cad_demo_state_defaults_are_deterministic() {
        let state = CadDemoPaneState::default();
        assert_eq!(state.load_state, super::PaneLoadState::Ready);
        assert_eq!(state.session_id, "cad.session.local");
        assert!(state.active_chat_session_id.is_none());
        assert!(state.chat_thread_session_bindings.is_empty());
        assert!(state.dispatch_sessions.is_empty());
        assert!(state.last_chat_intent_name.is_none());
        assert_eq!(state.build_session.phase, CadBuildSessionPhase::Idle);
        assert!(state.build_session.failure_class.is_none());
        assert_eq!(state.build_session.retry_attempts, 0);
        assert_eq!(state.build_session.retry_limit, 0);
        assert!(state.build_session.events.is_empty());
        assert!(state.last_build_session.is_none());
        assert_eq!(state.build_failure_metrics.tool_transport_failures, 0);
        assert_eq!(state.build_failure_metrics.intent_parse_failures, 0);
        assert_eq!(state.build_failure_metrics.dispatch_rebuild_failures, 0);
        assert_eq!(state.build_failure_metrics.tool_transport_retries, 0);
        assert_eq!(state.build_failure_metrics.intent_parse_retries, 0);
        assert_eq!(state.build_failure_metrics.dispatch_rebuild_retries, 0);
        assert_eq!(state.build_failure_metrics.terminal_failures, 0);
        assert_eq!(state.document_id, "cad.doc.demo-rack");
        assert_eq!(state.document_revision, 0);
        assert_eq!(state.active_variant_id, "variant.baseline");
        assert_eq!(state.variant_ids.len(), 4);
        assert_eq!(state.variant_ids[0], "variant.baseline");
        assert_eq!(state.active_variant_tile_index, 0);
        assert_eq!(state.variant_viewports.len(), 4);
        assert_eq!(state.variant_viewports[0].variant_id, "variant.baseline");
        assert!(state.last_rebuild_receipt.is_none());
        assert!(state.rebuild_receipts.is_empty());
        assert_eq!(state.eval_cache.len(), 0);
        assert!(state.rebuild_worker.is_none());
        assert_eq!(state.next_rebuild_request_id, 1);
        assert!(state.pending_rebuild_request_id.is_none());
        assert!(state.last_good_mesh_id.is_none());
        assert!(state.last_good_mesh_payload.is_none());
        assert_eq!(
            state.analysis_snapshot.material_id.as_deref(),
            Some("al-6061-t6")
        );
        assert_eq!(state.variant_analysis_snapshots.len(), 4);
        assert!(state.warnings.is_empty());
        assert_eq!(state.variant_warning_sets.len(), 4);
        assert_eq!(state.warning_filter_severity, "all");
        assert_eq!(state.warning_filter_code, "all");
        assert!(state.warning_hover_index.is_none());
        assert!(state.focused_warning_index.is_none());
        assert!(state.focused_geometry_ref.is_none());
        assert_eq!(state.hidden_line_mode, CadHiddenLineMode::Shaded);
        assert!(state.section_axis.is_none());
        assert_eq!(state.section_offset_normalized, 0.0);
        assert_eq!(state.section_summary(), "off");
        assert!(state.snap_toggles.grid);
        assert!(state.snap_toggles.origin);
        assert!(!state.snap_toggles.endpoint);
        assert!(!state.snap_toggles.midpoint);
        assert_eq!(state.projection_mode, CadProjectionMode::Orthographic);
        assert_eq!(state.drawing_view_mode, CadDrawingViewMode::ThreeD);
        assert_eq!(state.drawing_view_direction, CadDrawingViewDirection::Front);
        assert!(state.drawing_show_hidden_lines);
        assert!(state.drawing_show_dimensions);
        assert_eq!(state.drawing_zoom, 1.0);
        assert_eq!(state.drawing_pan_x, 0.0);
        assert_eq!(state.drawing_pan_y, 0.0);
        assert!(state.drawing_detail_views.is_empty());
        assert_eq!(state.drawing_next_detail_id, 1);
        assert_eq!(state.hotkey_profile, "default");
        assert_eq!(state.hotkeys.snap_top, "t");
        assert_eq!(state.hotkeys.toggle_projection, "p");
        assert_eq!(state.three_d_mouse_mode, CadThreeDMouseMode::Translate);
        assert_eq!(state.three_d_mouse_profile, CadThreeDMouseProfile::Balanced);
        assert_eq!(state.three_d_mouse_status(), "absent");
        assert_eq!(state.camera_zoom, 1.0);
        assert_eq!(state.camera_pan_x, 0.0);
        assert_eq!(state.camera_pan_y, 0.0);
        assert_eq!(state.camera_orbit_yaw_deg, 26.0);
        assert_eq!(state.camera_orbit_pitch_deg, 18.0);
        assert_eq!(state.history_stack.session_id, "cad.session.local");
        assert_eq!(state.history_stack.len_undo(), 0);
        assert!(state.timeline_rows.is_empty());
        assert!(state.timeline_selected_index.is_none());
        assert_eq!(state.timeline_scroll_offset, 0);
        assert!(state.selected_feature_params.is_empty());
        assert!(!state.context_menu.is_open);
        assert!(state.context_menu.items.is_empty());
    }

    #[test]
    fn cad_build_session_valid_transitions_archive_terminal_state() {
        let mut state = CadDemoPaneState::default();
        state
            .begin_agent_build_session("thread-1", "turn-1")
            .expect("build session should start");
        assert_eq!(state.build_session.phase, CadBuildSessionPhase::Planning);
        state
            .transition_agent_build_phase(
                CadBuildSessionPhase::Applying,
                "cad.build.applying.start",
                "tool executing".to_string(),
            )
            .expect("planning -> applying should be valid");
        state.record_agent_build_tool_result("OA-CAD-INTENT-OK", true, "intent applied");
        state
            .transition_agent_build_phase(
                CadBuildSessionPhase::Rebuilding,
                "cad.build.rebuilding.wait",
                "waiting for background rebuild".to_string(),
            )
            .expect("applying -> rebuilding should be valid");
        state.record_agent_build_rebuild_result("ai-intent:setmaterial", "ok");
        state
            .transition_agent_build_phase(
                CadBuildSessionPhase::Summarizing,
                "cad.build.summarizing.start",
                "rebuild committed".to_string(),
            )
            .expect("rebuilding -> summarizing should be valid");
        state
            .complete_agent_build_session("build complete".to_string())
            .expect("summarizing -> done should be valid");

        assert_eq!(state.build_session.phase, CadBuildSessionPhase::Idle);
        let archived = state
            .last_build_session
            .as_ref()
            .expect("completed session should be archived");
        assert_eq!(archived.thread_id, "thread-1");
        assert_eq!(archived.turn_id, "turn-1");
        assert_eq!(archived.terminal_phase, CadBuildSessionPhase::Done);
        assert!(
            archived
                .latest_tool_result
                .as_deref()
                .is_some_and(|value| value.contains("OA-CAD-INTENT-OK"))
        );
        assert!(
            archived
                .latest_rebuild_result
                .as_deref()
                .is_some_and(|value| value.contains("ai-intent:setmaterial"))
        );
        assert!(!archived.events.is_empty());
    }

    #[test]
    fn cad_build_session_rejects_invalid_phase_transition() {
        let mut state = CadDemoPaneState::default();
        let error = state
            .transition_agent_build_phase(
                CadBuildSessionPhase::Applying,
                "cad.build.applying.invalid",
                "invalid jump".to_string(),
            )
            .expect_err("idle -> applying must be rejected");
        assert!(error.contains("invalid CAD build phase transition"));
        assert_eq!(state.build_session.phase, CadBuildSessionPhase::Idle);
        assert!(state.build_session.events.is_empty());
    }

    #[test]
    fn cad_build_session_failure_archives_reason_and_remediation() {
        let mut state = CadDemoPaneState::default();
        state
            .begin_agent_build_session("thread-2", "turn-2")
            .expect("build session should start");
        state
            .transition_agent_build_phase(
                CadBuildSessionPhase::Applying,
                "cad.build.applying.start",
                "tool executing".to_string(),
            )
            .expect("planning -> applying should be valid");
        state.set_agent_build_failure_context(CadBuildFailureClass::IntentParseValidation, 1, 1);
        state
            .fail_agent_build_session(
                "cad.build.tool.failed",
                "tool returned parse error".to_string(),
                Some("retry with explicit rack dimensions".to_string()),
            )
            .expect("applying -> failed should be valid");

        assert_eq!(state.build_session.phase, CadBuildSessionPhase::Idle);
        let archived = state
            .last_build_session
            .as_ref()
            .expect("failed session should be archived");
        assert_eq!(archived.terminal_phase, CadBuildSessionPhase::Failed);
        assert_eq!(
            archived.failure_class,
            Some(CadBuildFailureClass::IntentParseValidation)
        );
        assert_eq!(archived.retry_attempts, 1);
        assert_eq!(archived.retry_limit, 1);
        assert_eq!(
            archived.failure_reason.as_deref(),
            Some("tool returned parse error")
        );
        assert_eq!(
            archived.remediation_hint.as_deref(),
            Some("retry with explicit rack dimensions")
        );
        assert_eq!(state.build_failure_metrics.terminal_failures, 1);
    }

    #[test]
    fn cad_camera_methods_are_deterministic_and_clamped() {
        let mut first = CadDemoPaneState::default();
        first.orbit_camera_by_drag(480.0, -1200.0);
        first.pan_camera_by_drag(1200.0, -1600.0);
        first.zoom_camera_by_scroll(-10_000.0);

        let mut second = CadDemoPaneState::default();
        second.orbit_camera_by_drag(480.0, -1200.0);
        second.pan_camera_by_drag(1200.0, -1600.0);
        second.zoom_camera_by_scroll(-10_000.0);

        assert_eq!(first.camera_orbit_yaw_deg, second.camera_orbit_yaw_deg);
        assert_eq!(first.camera_orbit_pitch_deg, second.camera_orbit_pitch_deg);
        assert_eq!(first.camera_pan_x, second.camera_pan_x);
        assert_eq!(first.camera_pan_y, second.camera_pan_y);
        assert_eq!(first.camera_zoom, second.camera_zoom);
        assert!(first.camera_orbit_pitch_deg < 180.0);
        assert!(first.camera_orbit_pitch_deg >= -180.0);
        assert!((first.camera_orbit_pitch_deg + 6.0).abs() <= 0.001);
        assert!(first.camera_pan_x <= 800.0);
        assert!(first.camera_pan_y >= -800.0);
        assert!(first.camera_zoom <= 1.0);
        assert!(first.camera_zoom >= 0.35);
    }

    #[test]
    fn cad_camera_view_snaps_are_deterministic() {
        let mut first = CadDemoPaneState::default();
        first.snap_camera_to_view(CadCameraViewSnap::Top);
        first.snap_camera_to_view(CadCameraViewSnap::Right);
        first.snap_camera_to_view(CadCameraViewSnap::Isometric);

        let mut second = CadDemoPaneState::default();
        second.snap_camera_to_view(CadCameraViewSnap::Top);
        second.snap_camera_to_view(CadCameraViewSnap::Right);
        second.snap_camera_to_view(CadCameraViewSnap::Isometric);

        assert_eq!(first.camera_orbit_yaw_deg, second.camera_orbit_yaw_deg);
        assert_eq!(first.camera_orbit_pitch_deg, second.camera_orbit_pitch_deg);
        assert_eq!(first.camera_pan_x, second.camera_pan_x);
        assert_eq!(first.camera_pan_y, second.camera_pan_y);
        assert_eq!(first.active_view_snap(), Some(CadCameraViewSnap::Isometric));
    }

    #[test]
    fn cad_projection_mode_cycles_deterministically() {
        let mut first = CadDemoPaneState::default();
        first.cycle_projection_mode();
        first.cycle_projection_mode();

        let mut second = CadDemoPaneState::default();
        second.cycle_projection_mode();
        second.cycle_projection_mode();

        assert_eq!(first.projection_mode, second.projection_mode);
        assert_eq!(first.projection_mode, CadProjectionMode::Orthographic);
        second.cycle_projection_mode();
        assert_eq!(second.projection_mode, CadProjectionMode::Perspective);
    }

    #[test]
    fn cad_drawing_mode_state_transitions_are_deterministic() {
        let mut first = CadDemoPaneState::default();
        let mut second = CadDemoPaneState::default();

        for state in [&mut first, &mut second] {
            assert_eq!(state.toggle_drawing_view_mode(), CadDrawingViewMode::TwoD);
            assert_eq!(
                state.cycle_drawing_view_direction(),
                CadDrawingViewDirection::Back
            );
            assert!(!state.toggle_drawing_hidden_lines());
            assert!(!state.toggle_drawing_dimensions());
            state.pan_drawing_view_by_drag(22.0, -14.0);
            state.zoom_drawing_view_by_scroll(-280.0);
            let detail = state.add_drawing_detail_view();
            assert_eq!(detail.detail_id, "detail-1");
            assert_eq!(detail.label, "A");
            assert_eq!(state.drawing_detail_views.len(), 1);
            let cleared = state.clear_drawing_detail_views();
            assert_eq!(cleared, 1);
            state.reset_drawing_view();
        }

        assert_eq!(first.drawing_view_mode, second.drawing_view_mode);
        assert_eq!(first.drawing_view_direction, second.drawing_view_direction);
        assert_eq!(
            first.drawing_show_hidden_lines,
            second.drawing_show_hidden_lines
        );
        assert_eq!(
            first.drawing_show_dimensions,
            second.drawing_show_dimensions
        );
        assert_eq!(first.drawing_zoom, second.drawing_zoom);
        assert_eq!(first.drawing_pan_x, second.drawing_pan_x);
        assert_eq!(first.drawing_pan_y, second.drawing_pan_y);
        assert!(first.drawing_detail_views.is_empty());
        assert_eq!(first.drawing_next_detail_id, 2);
    }

    #[test]
    fn cad_section_mode_and_offset_cycle_deterministically() {
        let mut state = CadDemoPaneState::default();
        assert!(state.section_axis.is_none());

        assert_eq!(state.cycle_section_axis(), Some(CadSectionAxis::X));
        assert_eq!(state.section_summary(), "x/0");
        assert_eq!(state.step_section_offset(), 0.2);
        assert_eq!(state.step_section_offset(), 0.4);
        assert_eq!(state.step_section_offset(), -0.4);

        assert_eq!(state.cycle_section_axis(), Some(CadSectionAxis::Y));
        assert_eq!(state.cycle_section_axis(), Some(CadSectionAxis::Z));
        assert_eq!(state.cycle_section_axis(), None);
        assert_eq!(state.section_offset_normalized, 0.0);
        assert!(state.section_plane().is_none());
    }

    #[test]
    fn cad_material_cycle_is_deterministic() {
        let mut state = CadDemoPaneState::default();
        assert_eq!(
            state.analysis_snapshot.material_id.as_deref(),
            Some("al-6061-t6")
        );
        assert_eq!(state.cycle_material_preset(), "al-5052-h32");
        assert_eq!(
            state.analysis_snapshot.material_id.as_deref(),
            Some("al-5052-h32")
        );
        assert_eq!(state.cycle_material_preset(), "steel-1018");
        assert_eq!(state.cycle_material_preset(), "ti-6al-4v");
        assert_eq!(state.cycle_material_preset(), "al-6061-t6");
    }

    #[test]
    fn cad_dimension_editing_is_typed_and_bounded() {
        let mut state = CadDemoPaneState::default();
        assert!(state.begin_dimension_edit(0));
        assert!(state.dimension_edit.is_some());

        // Replace default draft with a typed value.
        for _ in 0..16 {
            let _ = state.backspace_dimension_edit();
        }
        assert!(state.append_dimension_edit_char('4'));
        assert!(state.append_dimension_edit_char('2'));
        assert!(state.append_dimension_edit_char('0'));
        assert!(state.append_dimension_edit_char('.'));
        assert!(state.append_dimension_edit_char('5'));

        let (dimension_id, before, after) = state
            .commit_dimension_edit()
            .expect("dimension commit should succeed");
        assert_eq!(dimension_id, "width_mm");
        assert_eq!(before, 390.0);
        assert_eq!(after, 420.5);
        assert!(state.dimension_edit.is_none());
        assert_eq!(state.dimension_value_mm("width_mm"), Some(420.5));

        assert!(state.begin_dimension_edit(3));
        for _ in 0..16 {
            let _ = state.backspace_dimension_edit();
        }
        assert!(state.append_dimension_edit_char('9'));
        assert!(state.append_dimension_edit_char('9'));
        let error = state
            .commit_dimension_edit()
            .expect_err("out-of-range wall edit must fail");
        assert!(error.contains("Wall"));
        assert_eq!(state.dimension_value_mm("wall_mm"), Some(6.0));
    }

    #[test]
    fn cad_assembly_selection_and_editing_are_deterministic() {
        let mut first = CadDemoPaneState::default();
        let mut second = CadDemoPaneState::default();

        for state in [&mut first, &mut second] {
            state
                .select_assembly_instance("arm-1")
                .expect("instance selection should succeed");
            state
                .rename_selected_assembly_instance("Arm Segment".to_string())
                .expect("instance rename should succeed");
            state
                .select_assembly_joint("joint.hinge")
                .expect("joint selection should succeed");
            let semantics = state
                .set_selected_assembly_joint_state(120.0)
                .expect("joint state edit should succeed");
            assert!(semantics.was_clamped);
            assert_eq!(semantics.effective_state, 90.0);
        }

        let first_name = first
            .assembly_schema
            .instances
            .iter()
            .find(|instance| instance.id == "arm-1")
            .and_then(|instance| instance.name.clone());
        let second_name = second
            .assembly_schema
            .instances
            .iter()
            .find(|instance| instance.id == "arm-1")
            .and_then(|instance| instance.name.clone());
        assert_eq!(first_name.as_deref(), Some("Arm Segment"));
        assert_eq!(first_name, second_name);
        assert_eq!(first.assembly_ui_state, second.assembly_ui_state);
    }

    #[test]
    fn cad_snap_modes_and_point_snapping_are_deterministic() {
        let mut first = CadDemoPaneState::default();
        let mut second = CadDemoPaneState::default();
        let viewport = wgpui::Bounds::new(40.0, 30.0, 240.0, 160.0);
        let point = wgpui::Point::new(149.2, 113.7);

        first.toggle_snap_mode(CadSnapMode::Endpoint);
        first.toggle_snap_mode(CadSnapMode::Midpoint);
        second.toggle_snap_mode(CadSnapMode::Endpoint);
        second.toggle_snap_mode(CadSnapMode::Midpoint);

        let snapped_first = first.apply_snap_to_viewport_point(point, viewport);
        let snapped_second = second.apply_snap_to_viewport_point(point, viewport);
        assert_eq!(snapped_first, snapped_second);
        assert_eq!(first.snap_summary(), second.snap_summary());
    }

    #[test]
    fn cad_measurement_points_produce_deterministic_distance_and_angle() {
        let mut state = CadDemoPaneState::default();
        assert!(state.record_measurement_snap_point(0, wgpui::Point::new(100.0, 80.0)));
        assert!(state.record_measurement_snap_point(0, wgpui::Point::new(160.0, 80.0)));

        assert_eq!(state.measurement_tile_index, Some(0));
        assert_eq!(state.measurement_points.len(), 2);
        assert_eq!(state.measurement_distance_px, Some(60.0));
        assert_eq!(state.measurement_angle_deg, Some(0.0));

        assert!(state.record_measurement_snap_point(1, wgpui::Point::new(20.0, 20.0)));
        assert_eq!(state.measurement_tile_index, Some(1));
        assert_eq!(state.measurement_points.len(), 1);
        assert!(state.measurement_distance_px.is_none());
        assert!(state.measurement_angle_deg.is_none());
    }

    #[test]
    fn cad_hotkey_profiles_and_conflict_checks_are_deterministic() {
        let mut state = CadDemoPaneState::default();
        assert_eq!(state.hotkey_profile, "default");
        assert!(state.cycle_hotkey_profile().is_ok());
        assert_eq!(state.hotkey_profile, "compact");
        assert_eq!(state.hotkeys.snap_top, "7");
        assert!(state.hotkey_matches(CadHotkeyAction::SnapTop, "7"));

        let conflict = state.remap_hotkey(CadHotkeyAction::SnapFront, "7");
        assert!(conflict.is_err());

        assert!(state.remap_hotkey(CadHotkeyAction::SnapFront, "2").is_ok());
        assert!(state.hotkey_matches(CadHotkeyAction::SnapFront, "2"));
    }

    #[test]
    fn cad_three_d_mouse_mapping_modes_profiles_and_locks_are_deterministic() {
        let mut state = CadDemoPaneState::default();
        assert_eq!(state.three_d_mouse_status(), "absent");
        assert!(state.apply_three_d_mouse_motion(0, 0.25));
        assert!(state.camera_pan_x > 0.0);

        state.toggle_three_d_mouse_mode();
        assert_eq!(state.three_d_mouse_mode, CadThreeDMouseMode::Rotate);
        let yaw_before = state.camera_orbit_yaw_deg;
        assert!(state.apply_three_d_mouse_motion(3, 0.2));
        assert!(state.camera_orbit_yaw_deg > yaw_before);

        state.cycle_three_d_mouse_profile();
        assert_eq!(state.three_d_mouse_profile, CadThreeDMouseProfile::Fast);
        state.camera_zoom = 0.7;
        let zoom_before = state.camera_zoom;
        assert!(state.apply_three_d_mouse_motion(2, -0.35));
        assert!(state.camera_zoom > zoom_before);

        assert!(state.toggle_three_d_mouse_axis_lock(CadThreeDMouseAxis::Rx));
        let yaw_locked = state.camera_orbit_yaw_deg;
        assert!(!state.apply_three_d_mouse_motion(3, 0.4));
        assert_eq!(state.camera_orbit_yaw_deg, yaw_locked);
    }

    #[test]
    fn cad_context_menu_targeting_and_actions_are_deterministic() {
        let mut state = CadDemoPaneState::default();
        let viewport = wgpui::Bounds::new(40.0, 40.0, 360.0, 220.0);
        let left = wgpui::Point::new(80.0, 120.0);
        let middle = wgpui::Point::new(210.0, 120.0);
        let right = wgpui::Point::new(360.0, 120.0);

        let (kind_left, _) = state.infer_context_menu_target_for_viewport_point(left, viewport);
        let (kind_mid, _) = state.infer_context_menu_target_for_viewport_point(middle, viewport);
        let (kind_right, edge_ref) =
            state.infer_context_menu_target_for_viewport_point(right, viewport);
        assert_eq!(kind_left, CadContextMenuTargetKind::Body);
        assert_eq!(kind_mid, CadContextMenuTargetKind::Face);
        assert_eq!(kind_right, CadContextMenuTargetKind::Edge);

        state.open_context_menu(right, kind_right, edge_ref.clone());
        assert!(state.context_menu.is_open);
        assert_eq!(
            state.context_menu.target_kind,
            CadContextMenuTargetKind::Edge
        );
        assert_eq!(state.context_menu.items.len(), 3);

        let action = state
            .run_context_menu_item(0)
            .expect("edge context menu should include at least one action");
        assert!(action.contains("edge.inspect"));
        assert!(action.contains(edge_ref.as_str()));

        state.close_context_menu();
        assert!(!state.context_menu.is_open);
        assert!(state.context_menu.items.is_empty());
    }

    #[test]
    fn cad_variant_tiles_keep_independent_camera_and_selection_state() {
        let mut state = CadDemoPaneState::default();
        assert!(state.set_active_variant_tile(0));
        state.orbit_camera_by_drag(12.0, -4.0);
        state.set_focused_geometry_for_active_variant(Some("face.front".to_string()));
        state.set_hovered_geometry_for_active_variant(Some("face.front.hover".to_string()));

        assert!(state.set_active_variant_tile(1));
        let tile1_before = state.camera_orbit_yaw_deg;
        state.orbit_camera_by_drag(-18.0, 6.0);
        state.set_focused_geometry_for_active_variant(Some("edge.rim".to_string()));
        state.set_hovered_geometry_for_active_variant(Some("edge.rim.hover".to_string()));
        assert_ne!(state.camera_orbit_yaw_deg, tile1_before);

        assert!(state.set_active_variant_tile(0));
        assert_eq!(
            state.focused_geometry_ref.as_deref(),
            Some("face.front"),
            "tile 0 selection should persist"
        );
        assert_eq!(
            state.hovered_geometry_ref.as_deref(),
            Some("face.front.hover"),
            "tile 0 hover should persist"
        );
        let yaw_tile0 = state.camera_orbit_yaw_deg;

        assert!(state.set_active_variant_tile(1));
        assert_eq!(
            state.focused_geometry_ref.as_deref(),
            Some("edge.rim"),
            "tile 1 selection should persist"
        );
        assert_eq!(
            state.hovered_geometry_ref.as_deref(),
            Some("edge.rim.hover"),
            "tile 1 hover should persist"
        );
        assert_ne!(state.camera_orbit_yaw_deg, yaw_tile0);
    }

    #[test]
    fn cad_variant_tiles_keep_independent_analysis_and_warning_state() {
        let mut state = CadDemoPaneState::default();

        assert!(state.set_active_variant_tile(0));
        let mut tile0_analysis = state.analysis_snapshot.clone();
        tile0_analysis.variant_id = "variant.baseline".to_string();
        tile0_analysis.mass_kg = Some(2.7);
        state.set_variant_analysis_snapshot("variant.baseline", tile0_analysis.clone());
        state.set_variant_warning_set(
            "variant.baseline",
            vec![CadDemoWarningState {
                warning_id: "w0".to_string(),
                code: "CAD-WARN-SLIVER-FACE".to_string(),
                severity: "warning".to_string(),
                message: "sliver".to_string(),
                remediation_hint: "adjust vent spacing".to_string(),
                semantic_refs: vec!["vent_face_set".to_string()],
                deep_link: Some("cad://feature/feature.rack.vent_face_set".to_string()),
                feature_id: "feature.rack.vent_face_set".to_string(),
                entity_id: "face.0".to_string(),
            }],
        );

        assert!(state.set_active_variant_tile(1));
        let mut tile1_analysis = state.analysis_snapshot.clone();
        tile1_analysis.variant_id = "variant.lightweight".to_string();
        tile1_analysis.mass_kg = Some(2.2);
        state.set_variant_analysis_snapshot("variant.lightweight", tile1_analysis.clone());
        state.set_variant_warning_set("variant.lightweight", Vec::new());

        assert_eq!(state.analysis_snapshot.variant_id, "variant.lightweight");
        assert_eq!(state.analysis_snapshot.mass_kg, Some(2.2));
        assert!(state.warnings.is_empty());

        assert!(state.set_active_variant_tile(0));
        assert_eq!(state.analysis_snapshot.variant_id, "variant.baseline");
        assert_eq!(state.analysis_snapshot.mass_kg, Some(2.7));
        assert_eq!(state.warnings.len(), 1);
        assert_eq!(state.warnings[0].warning_id, "w0");
    }

    #[test]
    fn cad_chat_session_binding_is_deterministic_per_thread() {
        let mut state = CadDemoPaneState::default();
        let first = state.ensure_chat_session_for_thread("thread-alpha");
        let second = state.ensure_chat_session_for_thread("thread-alpha");
        let third = state.ensure_chat_session_for_thread("thread-beta");

        assert_eq!(first, second);
        assert_ne!(first, third);
        assert_eq!(
            state.active_chat_session_id.as_deref(),
            Some(third.as_str())
        );
        assert_eq!(
            state
                .chat_thread_session_bindings
                .get("thread-alpha")
                .map(String::as_str),
            Some(first.as_str())
        );
    }

    #[test]
    fn cad_chat_followup_intents_reuse_session_dispatch_state() {
        let mut state = CadDemoPaneState::default();
        let thread = "thread-followup";
        let intent_a = openagents_cad::intent::CadIntent::SetMaterial(
            openagents_cad::intent::SetMaterialIntent {
                material_id: "al-6061-t6".to_string(),
            },
        );
        let intent_b = openagents_cad::intent::CadIntent::SetObjective(
            openagents_cad::intent::SetObjectiveIntent {
                objective: "stiffness".to_string(),
            },
        );

        let first = state
            .apply_chat_intent_for_thread(thread, &intent_a)
            .expect("first intent should apply");
        let session = state
            .active_chat_session_id
            .clone()
            .expect("session should exist");
        let second = state
            .apply_chat_intent_for_thread(thread, &intent_b)
            .expect("second intent should apply");

        assert_eq!(first.state_revision, 1);
        assert_eq!(second.state_revision, 2);
        assert_eq!(state.session_id, session);
        assert_eq!(
            state
                .dispatch_sessions
                .get(&session)
                .map(|dispatch| dispatch.revision),
            Some(2)
        );
        assert_eq!(state.last_chat_intent_name.as_deref(), Some("SetObjective"));
    }
}
