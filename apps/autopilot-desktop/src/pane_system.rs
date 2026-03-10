use std::sync::atomic::{AtomicU64, Ordering};

use wgpui::components::hud::{PaneFrame, ResizablePane, ResizeEdge};
use wgpui::{Bounds, Component, InputEvent, Modifiers, MouseButton, Point, Size, theme};
use winit::window::CursorIcon;

use crate::app_state::{ActivityFeedFilter, DesktopPane, PaneDragMode, PaneKind, RenderState};
use crate::hotbar::{HOTBAR_FLOAT_GAP, HOTBAR_HEIGHT};
use crate::pane_registry::pane_spec;
use crate::panes::{
    calculator as calculator_pane, chat as chat_pane, local_inference as local_inference_pane,
    relay_connections as relay_connections_pane, wallet as wallet_pane,
};
use crate::render::{
    logical_size, sidebar_go_online_button_bounds, sidebar_handle_bounds,
    wallet_balance_sats_label_bounds,
};
use crate::spark_pane::{self, CreateInvoicePaneAction, PayInvoicePaneAction, SparkPaneAction};

pub const PANE_TITLE_HEIGHT: f32 = 36.0;
pub const PANE_MIN_WIDTH: f32 = 220.0;
pub const PANE_MIN_HEIGHT: f32 = 140.0;
/// Default target width for the global sidebar when open.
pub const SIDEBAR_DEFAULT_WIDTH: f32 = 300.0;
pub const RIGHT_SIDEBAR_ENABLED: bool = false;
const PANE_FRAME_HORIZONTAL_CHROME: f32 = 2.0;
const PANE_MARGIN: f32 = 18.0;
const PANE_CASCADE_X: f32 = 26.0;
const PANE_CASCADE_Y: f32 = 22.0;
const PANE_HOTBAR_CLEARANCE: f32 = 16.0;
const PANE_BOTTOM_RESERVED: f32 =
    HOTBAR_HEIGHT + HOTBAR_FLOAT_GAP + PANE_MARGIN + PANE_HOTBAR_CLEARANCE;
const CHAT_PAD: f32 = 12.0;
const CHAT_WORKSPACE_RAIL_WIDTH: f32 = 68.0;
const CHAT_WORKSPACE_SLOT_HEIGHT: f32 = 52.0;
const CHAT_THREAD_RAIL_WIDTH: f32 = 208.0;
const CHAT_COLUMN_GAP: f32 = 10.0;
const CHAT_TRANSCRIPT_HEADER_HEIGHT: f32 = 68.0;
const CHAT_COMPOSER_MIN_HEIGHT: f32 = 30.0;
const CHAT_COMPOSER_MAX_HEIGHT: f32 = 120.0;
const CHAT_SEND_WIDTH: f32 = 30.0;
const CHAT_HEADER_BUTTON_HEIGHT: f32 = 26.0;
const CHAT_HEADER_BUTTON_WIDTH: f32 = 110.0;
/// Compact + button next to "Threads" to start a new thread
const CHAT_NEW_THREAD_BUTTON_SIZE: f32 = 26.0;
const CHAT_THREAD_FILTER_BUTTON_HEIGHT: f32 = 22.0;
const CHAT_THREAD_FILTER_BUTTON_WIDTH: f32 = 72.0;
const CHAT_THREAD_ACTION_BUTTON_HEIGHT: f32 = 22.0;
const CHAT_THREAD_ACTION_BUTTON_WIDTH: f32 = 72.0;
const CHAT_THREAD_ACTION_BUTTON_GAP: f32 = 4.0;
const CHAT_SHELL_ROW_HEIGHT: f32 = 30.0;
const CHAT_SHELL_ROW_GAP: f32 = 6.0;
const CHAT_MAX_THREAD_ROWS: usize = 10;
pub const CHAT_AUTOPILOT_THREAD_PREVIEW_LIMIT: usize = 6;
const CALCULATOR_INPUT_HEIGHT: f32 = 30.0;
const CAST_BUTTON_HEIGHT: f32 = 28.0;
const SKILL_REGISTRY_ROW_HEIGHT: f32 = 28.0;
const SKILL_REGISTRY_ROW_GAP: f32 = 6.0;
const SKILL_REGISTRY_MAX_ROWS: usize = 8;
const CODEX_MCP_ROW_HEIGHT: f32 = 30.0;
const CODEX_MCP_ROW_GAP: f32 = 6.0;
const CODEX_MCP_MAX_ROWS: usize = 8;
const CODEX_APPS_ROW_HEIGHT: f32 = 30.0;
const CODEX_APPS_ROW_GAP: f32 = 6.0;
const CODEX_APPS_MAX_ROWS: usize = 8;
const JOB_INBOX_BUTTON_HEIGHT: f32 = 30.0;
const JOB_INBOX_BUTTON_GAP: f32 = 10.0;
const JOB_INBOX_ROW_GAP: f32 = 6.0;
const JOB_INBOX_ROW_HEIGHT: f32 = 30.0;
const JOB_INBOX_MAX_ROWS: usize = 8;
const RELAY_CONNECTIONS_ROW_HEIGHT: f32 = 30.0;
const RELAY_CONNECTIONS_ROW_GAP: f32 = 6.0;
const RELAY_CONNECTIONS_MAX_ROWS: usize = 8;
const ACTIVITY_FEED_FILTER_BUTTON_HEIGHT: f32 = 28.0;
const ACTIVITY_FEED_FILTER_GAP: f32 = 8.0;
const ACTIVITY_FEED_ROW_HEIGHT: f32 = 30.0;
const ACTIVITY_FEED_ROW_GAP: f32 = 6.0;
const ACTIVITY_FEED_MAX_ROWS: usize = 8;
const ACTIVITY_FEED_DETAILS_TOP_GAP: f32 = 10.0;
const ACTIVITY_FEED_DETAILS_LINE_HEIGHT: f32 = 16.0;
const ACTIVITY_FEED_DETAILS_HEADER_LINES: usize = 2;
const ACTIVITY_FEED_DETAILS_LABEL_INSET_X: f32 = 12.0;
const ACTIVITY_FEED_DETAILS_VALUE_OFFSET_X: f32 = 122.0;
const ACTIVITY_FEED_DETAILS_RIGHT_PADDING: f32 = 8.0;
const ACTIVITY_FEED_DETAILS_WRAP_CHARS: usize = 72;
const ALERTS_RECOVERY_ROW_HEIGHT: f32 = 30.0;
const ALERTS_RECOVERY_ROW_GAP: f32 = 6.0;
const ALERTS_RECOVERY_MAX_ROWS: usize = 8;
const CREDENTIALS_BUTTON_HEIGHT: f32 = 28.0;
const CREDENTIALS_BUTTON_WIDTH: f32 = 116.0;
const CREDENTIALS_BUTTON_GAP: f32 = 8.0;
const CREDENTIALS_ROW_HEIGHT: f32 = 28.0;
const CREDENTIALS_ROW_GAP: f32 = 6.0;
const CREDENTIALS_MAX_ROWS: usize = 10;
const CAD_CONTEXT_MENU_ROW_HEIGHT: f32 = 24.0;
static PANE_Z_SORT_INVOCATIONS: AtomicU64 = AtomicU64::new(0);

mod helpers;
use helpers::*;

pub struct PaneController;

pub struct PaneInput;

pub fn sidebar_reserved_width(state: &RenderState) -> f32 {
    if !RIGHT_SIDEBAR_ENABLED {
        return 0.0;
    }

    let logical = logical_size(&state.config, state.scale_factor);
    if state.sidebar.is_open {
        state.sidebar.width.min(logical.width.max(0.0))
    } else {
        0.0
    }
}

fn focus_chat_composer_for_pane_open(state: &mut RenderState) {
    state.spark_inputs.invoice_amount.blur();
    state.spark_inputs.send_request.blur();
    state.spark_inputs.send_amount.blur();
    state.pay_invoice_inputs.payment_request.blur();
    state.pay_invoice_inputs.amount_sats.blur();
    state.create_invoice_inputs.amount_sats.blur();
    state.create_invoice_inputs.description.blur();
    state.create_invoice_inputs.expiry_seconds.blur();
    state.relay_connections_inputs.relay_url.blur();
    state.network_requests_inputs.compute_family.blur();
    state.network_requests_inputs.preferred_backend.blur();
    state.network_requests_inputs.capability_constraints.blur();
    state.network_requests_inputs.quantity.blur();
    state.network_requests_inputs.delivery_start_minutes.blur();
    state.network_requests_inputs.window_minutes.blur();
    state.network_requests_inputs.max_price_sats.blur();
    state.local_inference_inputs.prompt.blur();
    state.local_inference_inputs.requested_model.blur();
    state.local_inference_inputs.max_tokens.blur();
    state.local_inference_inputs.temperature.blur();
    state.local_inference_inputs.top_k.blur();
    state.local_inference_inputs.top_p.blur();
    state.settings_inputs.relay_url.blur();
    state.settings_inputs.wallet_default_send_sats.blur();
    state.settings_inputs.provider_max_queue_depth.blur();
    state.credentials_inputs.variable_name.blur();
    state.credentials_inputs.variable_value.blur();
    state.job_history_inputs.search_job_id.blur();
    state.chat_inputs.composer.focus();
}

fn focus_local_inference_prompt_for_pane_open(state: &mut RenderState) {
    state.spark_inputs.invoice_amount.blur();
    state.spark_inputs.send_request.blur();
    state.spark_inputs.send_amount.blur();
    state.pay_invoice_inputs.payment_request.blur();
    state.pay_invoice_inputs.amount_sats.blur();
    state.create_invoice_inputs.amount_sats.blur();
    state.create_invoice_inputs.description.blur();
    state.create_invoice_inputs.expiry_seconds.blur();
    state.relay_connections_inputs.relay_url.blur();
    state.network_requests_inputs.compute_family.blur();
    state.network_requests_inputs.preferred_backend.blur();
    state.network_requests_inputs.capability_constraints.blur();
    state.network_requests_inputs.quantity.blur();
    state.network_requests_inputs.delivery_start_minutes.blur();
    state.network_requests_inputs.window_minutes.blur();
    state.network_requests_inputs.max_price_sats.blur();
    state.settings_inputs.relay_url.blur();
    state.settings_inputs.wallet_default_send_sats.blur();
    state.settings_inputs.provider_max_queue_depth.blur();
    state.credentials_inputs.variable_name.blur();
    state.credentials_inputs.variable_value.blur();
    state.job_history_inputs.search_job_id.blur();
    state.chat_inputs.composer.blur();
    state.local_inference_inputs.prompt.focus();
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobInboxPaneAction {
    AcceptSelected,
    RejectSelected,
    SelectRow(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActiveJobPaneAction {
    AdvanceStage,
    AbortJob,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobHistoryPaneAction {
    CycleStatusFilter,
    CycleTimeRange,
    PreviousPage,
    NextPage,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexAccountPaneAction {
    Refresh,
    LoginChatgpt,
    CancelLogin,
    Logout,
    RateLimits,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexModelsPaneAction {
    Refresh,
    ToggleHidden,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexConfigPaneAction {
    Read,
    Requirements,
    WriteSample,
    BatchWriteSample,
    DetectExternal,
    ImportExternal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexMcpPaneAction {
    Refresh,
    LoginSelected,
    Reload,
    SelectRow(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexAppsPaneAction {
    Refresh,
    SelectRow(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexLabsPaneAction {
    ReviewInline,
    ReviewDetached,
    CommandExec,
    CollaborationModes,
    ExperimentalFeatures,
    ToggleExperimental,
    RealtimeStart,
    RealtimeAppendText,
    RealtimeStop,
    WindowsSandboxSetup,
    FuzzyStart,
    FuzzyUpdate,
    FuzzyStop,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexDiagnosticsPaneAction {
    EnableWireLog,
    DisableWireLog,
    ClearEvents,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EarningsScoreboardPaneAction {
    Refresh,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MissionControlPaneAction {
    ToggleAmountDisplay,
    WarmLocalModel,
    OpenDocumentation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelayConnectionsPaneAction {
    AddRelay,
    RemoveSelected,
    RetrySelected,
    SelectRow(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyncHealthPaneAction {
    Rebootstrap,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderStatusPaneAction {
    ToggleInventory(crate::app_state::ProviderInventoryProductToggleTarget),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalInferencePaneAction {
    RefreshRuntime,
    WarmModel,
    UnloadModel,
    RunPrompt,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkRequestsPaneAction {
    RequestQuotes,
    AcceptSelectedQuote,
    SelectQuote(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StarterJobsPaneAction {
    CompleteSelected,
    ToggleKillSwitch,
    SelectRow(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReciprocalLoopPaneAction {
    Start,
    Stop,
    Reset,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivityFeedPaneAction {
    Refresh,
    PreviousPage,
    NextPage,
    SetFilter(ActivityFeedFilter),
    SelectRow(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AlertsRecoveryPaneAction {
    RecoverSelected,
    AcknowledgeSelected,
    ResolveSelected,
    SelectRow(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingsPaneAction {
    Save,
    ResetDefaults,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialsPaneAction {
    AddCustom,
    SaveValue,
    DeleteOrClear,
    ToggleEnabled,
    ToggleScopeCodex,
    ToggleScopeSpark,
    ToggleScopeSkills,
    ToggleScopeGlobal,
    ImportFromEnv,
    Reload,
    SelectRow(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentProfileStatePaneAction {
    PublishProfile,
    PublishState,
    UpdateGoals,
    CreateGoal,
    StartGoal,
    AbortGoal,
    InspectGoalReceipt,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentScheduleTickPaneAction {
    ApplySchedule,
    PublishManualTick,
    InspectLastResult,
    ToggleOsSchedulerAdapter,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrajectoryAuditPaneAction {
    OpenSession,
    CycleStepFilter,
    VerifyTrajectoryHash,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CastControlPaneAction {
    RefreshStatus,
    RunCheck,
    RunProve,
    RunSignBroadcast,
    RunInspect,
    RunLoopOnce,
    ToggleAutoLoop,
    ToggleBroadcastArmed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SkillRegistryPaneAction {
    DiscoverSkills,
    InspectManifest,
    InstallSelectedSkill,
    SelectRow(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SkillTrustRevocationPaneAction {
    RefreshTrust,
    InspectAttestations,
    ToggleKillSwitch,
    RevokeSkill,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreditDeskPaneAction {
    PublishIntent,
    PublishOffer,
    PublishEnvelope,
    AuthorizeSpend,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreditSettlementLedgerPaneAction {
    VerifySettlement,
    EmitDefaultNotice,
    EmitReputationLabel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CadDemoPaneAction {
    Noop,
    CycleVariant,
    ToggleGripperJawAnimation,
    ToggleViewportLayout,
    ResetSession,
    BootstrapDemo,
    ResetCamera,
    ToggleDrawingViewMode,
    CycleDrawingViewDirection,
    ToggleDrawingHiddenLines,
    ToggleDrawingDimensions,
    ResetDrawingView,
    AddDrawingDetailView,
    ClearDrawingDetailViews,
    ToggleProjectionMode,
    CycleSectionPlane,
    StepSectionPlaneOffset,
    CycleMaterialPreset,
    ToggleSnapGrid,
    ToggleSnapOrigin,
    ToggleSnapEndpoint,
    ToggleSnapMidpoint,
    CycleHotkeyProfile,
    ToggleThreeDMouseMode,
    CycleThreeDMouseProfile,
    ToggleThreeDMouseLockX,
    ToggleThreeDMouseLockY,
    ToggleThreeDMouseLockZ,
    ToggleThreeDMouseLockRx,
    ToggleThreeDMouseLockRy,
    ToggleThreeDMouseLockRz,
    SnapViewTop,
    SnapViewFront,
    SnapViewRight,
    SnapViewIsometric,
    CycleHiddenLineMode,
    CycleSensorVisualizationMode,
    CycleWarningSeverityFilter,
    CycleWarningCodeFilter,
    SelectWarning(usize),
    SelectWarningMarker(usize),
    SelectTimelineRow(usize),
    TimelineSelectPrev,
    TimelineSelectNext,
    StartDimensionEdit(usize),
    DimensionInputChar(char),
    DimensionInputBackspace,
    DimensionInputCommit,
    DimensionInputCancel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CadPaletteCommandSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub keybinding: Option<&'static str>,
    pub action: CadDemoPaneAction,
}

const CAD_PALETTE_COMMAND_SPECS: [CadPaletteCommandSpec; 32] = [
    CadPaletteCommandSpec {
        id: "cad.demo.bootstrap",
        label: "CAD: Bootstrap Demo",
        description: "Reset CAD demo session to deterministic baseline and queue rebuild",
        keybinding: Some("B"),
        action: CadDemoPaneAction::BootstrapDemo,
    },
    CadPaletteCommandSpec {
        id: "cad.gripper.toggle_jaw",
        label: "CAD: Toggle Gripper Jaw",
        description: "Toggle gripper jaw open/close animation step and queue rebuild",
        keybinding: Some("J"),
        action: CadDemoPaneAction::ToggleGripperJawAnimation,
    },
    CadPaletteCommandSpec {
        id: "cad.view.snap_top",
        label: "CAD: Snap View Top",
        description: "Snap CAD camera to top view",
        keybinding: Some("T"),
        action: CadDemoPaneAction::SnapViewTop,
    },
    CadPaletteCommandSpec {
        id: "cad.view.snap_front",
        label: "CAD: Snap View Front",
        description: "Snap CAD camera to front view",
        keybinding: Some("F"),
        action: CadDemoPaneAction::SnapViewFront,
    },
    CadPaletteCommandSpec {
        id: "cad.view.snap_right",
        label: "CAD: Snap View Right",
        description: "Snap CAD camera to right view",
        keybinding: Some("R"),
        action: CadDemoPaneAction::SnapViewRight,
    },
    CadPaletteCommandSpec {
        id: "cad.view.snap_isometric",
        label: "CAD: Snap View Isometric",
        description: "Snap CAD camera to isometric view",
        keybinding: Some("I"),
        action: CadDemoPaneAction::SnapViewIsometric,
    },
    CadPaletteCommandSpec {
        id: "cad.view.toggle_projection",
        label: "CAD: Toggle Projection",
        description: "Toggle CAD projection between orthographic and perspective",
        keybinding: Some("P"),
        action: CadDemoPaneAction::ToggleProjectionMode,
    },
    CadPaletteCommandSpec {
        id: "cad.view.toggle_layout",
        label: "CAD: Toggle Layout",
        description: "Toggle CAD viewport layout between single and quad",
        keybinding: Some("4"),
        action: CadDemoPaneAction::ToggleViewportLayout,
    },
    CadPaletteCommandSpec {
        id: "cad.drawing.toggle_mode",
        label: "CAD: Toggle Drawing Mode",
        description: "Toggle CAD viewport between 3D and 2D drawing mode",
        keybinding: Some("2"),
        action: CadDemoPaneAction::ToggleDrawingViewMode,
    },
    CadPaletteCommandSpec {
        id: "cad.drawing.cycle_direction",
        label: "CAD: Cycle Drawing Direction",
        description: "Cycle drawing direction (front/back/top/bottom/left/right/isometric)",
        keybinding: None,
        action: CadDemoPaneAction::CycleDrawingViewDirection,
    },
    CadPaletteCommandSpec {
        id: "cad.drawing.toggle_hidden_lines",
        label: "CAD: Toggle Drawing Hidden Lines",
        description: "Toggle hidden line visibility in 2D drawing mode",
        keybinding: None,
        action: CadDemoPaneAction::ToggleDrawingHiddenLines,
    },
    CadPaletteCommandSpec {
        id: "cad.drawing.toggle_dimensions",
        label: "CAD: Toggle Drawing Dimensions",
        description: "Toggle drawing dimensions overlay in 2D mode",
        keybinding: None,
        action: CadDemoPaneAction::ToggleDrawingDimensions,
    },
    CadPaletteCommandSpec {
        id: "cad.drawing.reset_view",
        label: "CAD: Reset Drawing View",
        description: "Reset 2D drawing zoom and pan",
        keybinding: None,
        action: CadDemoPaneAction::ResetDrawingView,
    },
    CadPaletteCommandSpec {
        id: "cad.drawing.add_detail",
        label: "CAD: Add Drawing Detail View",
        description: "Add a default detail view in 2D drawing mode",
        keybinding: None,
        action: CadDemoPaneAction::AddDrawingDetailView,
    },
    CadPaletteCommandSpec {
        id: "cad.drawing.clear_details",
        label: "CAD: Clear Drawing Detail Views",
        description: "Clear all 2D drawing detail views",
        keybinding: None,
        action: CadDemoPaneAction::ClearDrawingDetailViews,
    },
    CadPaletteCommandSpec {
        id: "cad.render.cycle_mode",
        label: "CAD: Cycle Render Mode",
        description: "Cycle CAD render mode (shaded, edges, wireframe)",
        keybinding: Some("V"),
        action: CadDemoPaneAction::CycleHiddenLineMode,
    },
    CadPaletteCommandSpec {
        id: "cad.render.cycle_sensor_mode",
        label: "CAD: Cycle Sensor Overlay",
        description: "Cycle sensor visualization mode (off, pressure, proximity, combined)",
        keybinding: None,
        action: CadDemoPaneAction::CycleSensorVisualizationMode,
    },
    CadPaletteCommandSpec {
        id: "cad.section.cycle_plane",
        label: "CAD: Cycle Section Plane",
        description: "Cycle section clipping plane axis (off/x/y/z)",
        keybinding: None,
        action: CadDemoPaneAction::CycleSectionPlane,
    },
    CadPaletteCommandSpec {
        id: "cad.section.step_offset",
        label: "CAD: Step Section Offset",
        description: "Step section clipping plane offset",
        keybinding: None,
        action: CadDemoPaneAction::StepSectionPlaneOffset,
    },
    CadPaletteCommandSpec {
        id: "cad.material.cycle",
        label: "CAD: Cycle Material",
        description: "Cycle material preset used by mass/cost analysis",
        keybinding: None,
        action: CadDemoPaneAction::CycleMaterialPreset,
    },
    CadPaletteCommandSpec {
        id: "cad.snap.toggle_grid",
        label: "CAD: Toggle Grid Snap",
        description: "Toggle CAD grid snap preview",
        keybinding: Some("G"),
        action: CadDemoPaneAction::ToggleSnapGrid,
    },
    CadPaletteCommandSpec {
        id: "cad.snap.toggle_origin",
        label: "CAD: Toggle Origin Snap",
        description: "Toggle CAD origin snap preview",
        keybinding: Some("O"),
        action: CadDemoPaneAction::ToggleSnapOrigin,
    },
    CadPaletteCommandSpec {
        id: "cad.snap.toggle_endpoint",
        label: "CAD: Toggle Endpoint Snap",
        description: "Toggle CAD endpoint snap preview",
        keybinding: Some("E"),
        action: CadDemoPaneAction::ToggleSnapEndpoint,
    },
    CadPaletteCommandSpec {
        id: "cad.snap.toggle_midpoint",
        label: "CAD: Toggle Midpoint Snap",
        description: "Toggle CAD midpoint snap preview",
        keybinding: Some("M"),
        action: CadDemoPaneAction::ToggleSnapMidpoint,
    },
    CadPaletteCommandSpec {
        id: "cad.3dmouse.toggle_mode",
        label: "CAD: 3D Mouse Toggle Mode",
        description: "Toggle 3D mouse translate/rotate mapping mode",
        keybinding: None,
        action: CadDemoPaneAction::ToggleThreeDMouseMode,
    },
    CadPaletteCommandSpec {
        id: "cad.3dmouse.cycle_profile",
        label: "CAD: 3D Mouse Cycle Profile",
        description: "Cycle 3D mouse sensitivity profile (precision, balanced, fast)",
        keybinding: None,
        action: CadDemoPaneAction::CycleThreeDMouseProfile,
    },
    CadPaletteCommandSpec {
        id: "cad.3dmouse.lock_x",
        label: "CAD: 3D Mouse Toggle X Lock",
        description: "Toggle X-axis lock for 3D mouse mapping",
        keybinding: None,
        action: CadDemoPaneAction::ToggleThreeDMouseLockX,
    },
    CadPaletteCommandSpec {
        id: "cad.3dmouse.lock_y",
        label: "CAD: 3D Mouse Toggle Y Lock",
        description: "Toggle Y-axis lock for 3D mouse mapping",
        keybinding: None,
        action: CadDemoPaneAction::ToggleThreeDMouseLockY,
    },
    CadPaletteCommandSpec {
        id: "cad.3dmouse.lock_z",
        label: "CAD: 3D Mouse Toggle Z Lock",
        description: "Toggle Z-axis lock for 3D mouse mapping",
        keybinding: None,
        action: CadDemoPaneAction::ToggleThreeDMouseLockZ,
    },
    CadPaletteCommandSpec {
        id: "cad.3dmouse.lock_rx",
        label: "CAD: 3D Mouse Toggle Rx Lock",
        description: "Toggle Rx-axis lock for 3D mouse mapping",
        keybinding: None,
        action: CadDemoPaneAction::ToggleThreeDMouseLockRx,
    },
    CadPaletteCommandSpec {
        id: "cad.3dmouse.lock_ry",
        label: "CAD: 3D Mouse Toggle Ry Lock",
        description: "Toggle Ry-axis lock for 3D mouse mapping",
        keybinding: None,
        action: CadDemoPaneAction::ToggleThreeDMouseLockRy,
    },
    CadPaletteCommandSpec {
        id: "cad.3dmouse.lock_rz",
        label: "CAD: 3D Mouse Toggle Rz Lock",
        description: "Toggle Rz-axis lock for 3D mouse mapping",
        keybinding: None,
        action: CadDemoPaneAction::ToggleThreeDMouseLockRz,
    },
];

pub fn cad_palette_command_specs() -> &'static [CadPaletteCommandSpec] {
    &CAD_PALETTE_COMMAND_SPECS
}

pub fn cad_palette_action_for_command_id(command_id: &str) -> Option<CadDemoPaneAction> {
    cad_palette_command_specs()
        .iter()
        .find(|spec| spec.id == command_id)
        .map(|spec| spec.action)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaneHitAction {
    NostrRegenerate,
    NostrReveal,
    NostrCopySecret,
    ChatSend,
    ChatRefreshThreads,
    ChatNewThread,
    ChatCycleModel,
    ChatInterruptTurn,
    ChatToggleArchivedFilter,
    ChatCycleSortFilter,
    ChatCycleSourceFilter,
    ChatCycleProviderFilter,
    ChatForkThread,
    ChatArchiveThread,
    ChatUnarchiveThread,
    ChatRenameThread,
    ChatRollbackThread,
    ChatCompactThread,
    ChatUnsubscribeThread,
    ChatRespondApprovalAccept,
    ChatRespondApprovalAcceptSession,
    ChatRespondApprovalDecline,
    ChatRespondApprovalCancel,
    ChatRespondToolCall,
    ChatRespondToolUserInput,
    ChatRespondAuthRefresh,
    ChatSelectWorkspace(usize),
    ChatToggleCategory(usize),
    ChatSelectThread(usize),
    GoOnlineToggle,
    MissionControl(MissionControlPaneAction),
    CodexAccount(CodexAccountPaneAction),
    CodexModels(CodexModelsPaneAction),
    CodexConfig(CodexConfigPaneAction),
    CodexMcp(CodexMcpPaneAction),
    CodexApps(CodexAppsPaneAction),
    CodexLabs(CodexLabsPaneAction),
    CodexDiagnostics(CodexDiagnosticsPaneAction),
    EarningsScoreboard(EarningsScoreboardPaneAction),
    RelayConnections(RelayConnectionsPaneAction),
    SyncHealth(SyncHealthPaneAction),
    ProviderStatus(ProviderStatusPaneAction),
    LocalInference(LocalInferencePaneAction),
    NetworkRequests(NetworkRequestsPaneAction),
    StarterJobs(StarterJobsPaneAction),
    ReciprocalLoop(ReciprocalLoopPaneAction),
    ActivityFeed(ActivityFeedPaneAction),
    AlertsRecovery(AlertsRecoveryPaneAction),
    Settings(SettingsPaneAction),
    Credentials(CredentialsPaneAction),
    JobInbox(JobInboxPaneAction),
    ActiveJob(ActiveJobPaneAction),
    JobHistory(JobHistoryPaneAction),
    AgentProfileState(AgentProfileStatePaneAction),
    AgentScheduleTick(AgentScheduleTickPaneAction),
    TrajectoryAudit(TrajectoryAuditPaneAction),
    CastControl(CastControlPaneAction),
    SkillRegistry(SkillRegistryPaneAction),
    SkillTrustRevocation(SkillTrustRevocationPaneAction),
    CreditDesk(CreditDeskPaneAction),
    CreditSettlementLedger(CreditSettlementLedgerPaneAction),
    CadDemo(CadDemoPaneAction),
    Spark(SparkPaneAction),
    SparkCreateInvoice(CreateInvoicePaneAction),
    SparkPayInvoice(PayInvoicePaneAction),
}

#[derive(Clone, Copy)]
pub struct PaneDescriptor {
    pub kind: PaneKind,
    pub width: f32,
    pub height: f32,
    pub singleton: bool,
}

impl PaneDescriptor {
    pub fn for_kind(kind: PaneKind) -> Self {
        let spec = pane_spec(kind);
        Self {
            kind,
            width: spec.default_width,
            height: spec.default_height,
            singleton: spec.singleton,
        }
    }
}

fn initial_pane_size(
    pane_size_memory: &crate::app_state::PaneSizeMemory,
    descriptor: PaneDescriptor,
) -> Size {
    pane_size_memory
        .size_for(descriptor.kind)
        .unwrap_or_else(|| Size::new(descriptor.width, descriptor.height))
}

fn pane_minimum_size(kind: PaneKind) -> Size {
    let spec = pane_spec(kind);
    let pane_size_for_content = |content_width: f32, content_height: f32| {
        Size::new(
            content_width + PANE_FRAME_HORIZONTAL_CHROME,
            content_height + PANE_TITLE_HEIGHT + 1.0,
        )
    };

    match kind {
        PaneKind::AutopilotChat => pane_size_for_content(900.0, 500.0),
        PaneKind::CodexAccount
        | PaneKind::CodexModels
        | PaneKind::CodexConfig
        | PaneKind::CodexMcp
        | PaneKind::CodexApps
        | PaneKind::CodexLabs
        | PaneKind::CodexDiagnostics => pane_size_for_content(920.0, 420.0),
        PaneKind::GoOnline => pane_size_for_content(560.0, 300.0),
        PaneKind::LocalInference => pane_size_for_content(940.0, 520.0),
        PaneKind::EarningsScoreboard => pane_size_for_content(640.0, 320.0),
        PaneKind::RelayConnections | PaneKind::NetworkRequests => {
            pane_size_for_content(900.0, 420.0)
        }
        PaneKind::SyncHealth => pane_size_for_content(760.0, 360.0),
        PaneKind::StarterJobs | PaneKind::JobInbox | PaneKind::ActiveJob => {
            pane_size_for_content(860.0, 420.0)
        }
        PaneKind::ReciprocalLoop | PaneKind::AgentProfileState | PaneKind::AgentScheduleTick => {
            pane_size_for_content(860.0, 440.0)
        }
        PaneKind::ActivityFeed | PaneKind::AlertsRecovery | PaneKind::JobHistory => {
            pane_size_for_content(900.0, 460.0)
        }
        PaneKind::NostrIdentity => pane_size_for_content(480.0, 220.0),
        PaneKind::TrajectoryAudit
        | PaneKind::CastControl
        | PaneKind::SkillRegistry
        | PaneKind::SkillTrustRevocation
        | PaneKind::CreditDesk
        | PaneKind::CreditSettlementLedger => pane_size_for_content(960.0, 480.0),
        PaneKind::CadDemo => pane_size_for_content(860.0, 420.0),
        _ => Size::new(spec.default_width, spec.default_height),
    }
}

pub fn create_pane(state: &mut RenderState, descriptor: PaneDescriptor) -> u64 {
    if descriptor.singleton
        && let Some(existing_id) = state
            .panes
            .iter()
            .find(|pane| pane.kind == descriptor.kind)
            .map(|pane| pane.id)
    {
        bring_pane_to_front(state, existing_id);
        return existing_id;
    }

    let id = state.next_pane_id;
    state.next_pane_id = state.next_pane_id.saturating_add(1);

    let logical = logical_size(&state.config, state.scale_factor);
    let sidebar_width = sidebar_reserved_width(state);
    let tier = (id as usize - 1) % 10;
    let x = PANE_MARGIN + tier as f32 * PANE_CASCADE_X;
    let y = PANE_MARGIN + tier as f32 * PANE_CASCADE_Y;
    let min_size = pane_minimum_size(descriptor.kind);
    let initial_size = initial_pane_size(&state.pane_size_memory, descriptor);
    let bounds = clamp_bounds_to_window(
        Bounds::new(x, y, initial_size.width, initial_size.height),
        logical,
        sidebar_width,
        min_size,
    );

    let title = pane_title(descriptor.kind, id);
    let pane = DesktopPane {
        id,
        title: title.clone(),
        kind: descriptor.kind,
        bounds,
        z_index: state.next_z_index,
        frame: PaneFrame::new()
            .title(title)
            .active(true)
            .dismissable(true)
            .title_height(PANE_TITLE_HEIGHT),
    };

    state.next_z_index = state.next_z_index.saturating_add(1);
    state.panes.push(pane);
    id
}

impl PaneController {
    pub fn create(state: &mut RenderState, descriptor: PaneDescriptor) -> u64 {
        create_pane(state, descriptor)
    }

    pub fn create_for_kind(state: &mut RenderState, kind: PaneKind) -> u64 {
        let id = Self::create(state, PaneDescriptor::for_kind(kind));
        if kind == PaneKind::AutopilotChat {
            focus_chat_composer_for_pane_open(state);
        } else if kind == PaneKind::LocalInference {
            focus_local_inference_prompt_for_pane_open(state);
        }
        id
    }

    pub fn close(state: &mut RenderState, pane_id: u64) {
        close_pane(state, pane_id);
    }

    pub fn active(state: &RenderState) -> Option<u64> {
        active_pane_id(state)
    }

    pub fn bring_to_front(state: &mut RenderState, pane_id: u64) {
        bring_pane_to_front_by_id(state, pane_id);
    }

    pub fn update_drag(state: &mut RenderState, current_mouse: Point) -> bool {
        self::update_drag(state, current_mouse)
    }
}

impl PaneInput {
    pub fn handle_mouse_down(state: &mut RenderState, point: Point, button: MouseButton) -> bool {
        handle_pane_mouse_down(state, point, button)
    }

    pub fn handle_mouse_up(state: &mut RenderState, event: &InputEvent) -> bool {
        handle_pane_mouse_up(state, event)
    }

    pub fn dispatch_frame_event(state: &mut RenderState, event: &InputEvent) -> bool {
        dispatch_pane_frame_event(state, event)
    }

    pub fn cursor_icon(state: &RenderState, point: Point) -> CursorIcon {
        cursor_icon_for_pointer(state, point)
    }
}

pub fn handle_pane_mouse_down(state: &mut RenderState, point: Point, button: MouseButton) -> bool {
    if button != MouseButton::Left {
        return false;
    }

    for pane_idx in pane_indices_by_z_desc(state) {
        let pane_id = state.panes[pane_idx].id;
        let bounds = state.panes[pane_idx].bounds;

        let down_event = InputEvent::MouseDown {
            button,
            x: point.x,
            y: point.y,
            modifiers: Modifiers::default(),
        };
        if state.panes[pane_idx]
            .frame
            .event(&down_event, bounds, &mut state.event_context)
            .is_handled()
        {
            bring_pane_to_front(state, pane_id);
            return true;
        }

        let resize_edge = state.pane_resizer.edge_at(bounds, point);
        let title_bounds = pane_title_bounds(bounds);

        if resize_edge != ResizeEdge::None || bounds.contains(point) {
            bring_pane_to_front(state, pane_id);

            if resize_edge != ResizeEdge::None {
                state.pane_drag_mode = Some(PaneDragMode::Resizing {
                    pane_id,
                    edge: resize_edge,
                    start_mouse: point,
                    start_bounds: bounds,
                });
                return true;
            }

            if title_bounds.contains(point) {
                state.pane_drag_mode = Some(PaneDragMode::Moving {
                    pane_id,
                    start_mouse: point,
                    start_bounds: bounds,
                });
            }

            return true;
        }
    }

    false
}

pub fn handle_pane_mouse_up(state: &mut RenderState, event: &InputEvent) -> bool {
    let mut handled = false;
    let mut close_target: Option<u64> = None;

    for pane_idx in pane_indices_by_z_desc(state) {
        let bounds = state.panes[pane_idx].bounds;
        if state.panes[pane_idx]
            .frame
            .event(event, bounds, &mut state.event_context)
            .is_handled()
        {
            handled = true;
        }

        if state.panes[pane_idx].frame.take_close_clicked() {
            close_target = Some(state.panes[pane_idx].id);
            break;
        }
    }

    if let Some(pane_id) = close_target {
        close_pane(state, pane_id);
        handled = true;
    }

    if state.pane_drag_mode.take().is_some() {
        let _ = state.pane_size_memory.persist_if_dirty();
        handled = true;
    }

    handled
}

pub fn dispatch_pane_frame_event(state: &mut RenderState, event: &InputEvent) -> bool {
    let mut handled = false;
    for pane_idx in pane_indices_by_z_desc(state) {
        let bounds = state.panes[pane_idx].bounds;
        if state.panes[pane_idx]
            .frame
            .event(event, bounds, &mut state.event_context)
            .is_handled()
        {
            handled = true;
        }
    }
    handled
}

pub fn update_drag(state: &mut RenderState, current_mouse: Point) -> bool {
    let Some(mode) = state.pane_drag_mode else {
        return false;
    };

    let logical = logical_size(&state.config, state.scale_factor);
    let sidebar_width = sidebar_reserved_width(state);

    match mode {
        PaneDragMode::Moving {
            pane_id,
            start_mouse,
            start_bounds,
        } => {
            let dx = current_mouse.x - start_mouse.x;
            let dy = current_mouse.y - start_mouse.y;

            if let Some(pane) = state.panes.iter_mut().find(|pane| pane.id == pane_id) {
                let min_size = pane_minimum_size(pane.kind);
                let next = Bounds::new(
                    start_bounds.origin.x + dx,
                    start_bounds.origin.y + dy,
                    start_bounds.size.width,
                    start_bounds.size.height,
                );
                pane.bounds = clamp_bounds_to_window(next, logical, sidebar_width, min_size);
                return true;
            }
        }
        PaneDragMode::Resizing {
            pane_id,
            edge,
            start_mouse,
            start_bounds,
        } => {
            if let Some(pane) = state.panes.iter_mut().find(|pane| pane.id == pane_id) {
                let min_size = pane_minimum_size(pane.kind);
                let next = ResizablePane::new()
                    .min_size(min_size.width, min_size.height)
                    .resize_bounds(edge, start_bounds, start_mouse, current_mouse);
                pane.bounds = clamp_bounds_to_window(next, logical, sidebar_width, min_size);
                state.pane_size_memory.remember(pane.kind, pane.bounds.size);
                return true;
            }
        }
    }

    false
}

pub fn close_pane(state: &mut RenderState, pane_id: u64) {
    state.panes.retain(|pane| pane.id != pane_id);
}

pub fn active_pane_id(state: &RenderState) -> Option<u64> {
    state
        .panes
        .iter()
        .max_by_key(|pane| pane.z_index)
        .map(|pane| pane.id)
}

fn cad_action_uses_dense_row_hot_zone(action: CadDemoPaneAction) -> bool {
    matches!(
        action,
        CadDemoPaneAction::StartDimensionEdit(_)
            | CadDemoPaneAction::SelectTimelineRow(_)
            | CadDemoPaneAction::SelectWarning(_)
    )
}

pub fn cursor_icon_for_pointer(state: &RenderState, point: Point) -> CursorIcon {
    if let Some(mode) = state.pane_drag_mode {
        return match mode {
            PaneDragMode::Moving { .. } => CursorIcon::Move,
            PaneDragMode::Resizing { edge, .. } => cursor_icon_for_resize_edge(edge),
        };
    }

    let handle_bounds = sidebar_handle_bounds(state);
    if handle_bounds.contains(point) {
        return if state.sidebar.is_pressed && state.sidebar.is_dragging {
            CursorIcon::Grabbing
        } else {
            CursorIcon::Grab
        };
    }

    let go_online_bounds = sidebar_go_online_button_bounds(state);
    if go_online_bounds.size.width > 0.0 && go_online_bounds.contains(point) {
        return CursorIcon::Pointer;
    }

    let wallet_label_bounds = wallet_balance_sats_label_bounds(state);
    if wallet_label_bounds.size.width > 0.0 && wallet_label_bounds.contains(point) {
        return CursorIcon::Pointer;
    }

    if state.hotbar_bounds.contains(point) {
        return CursorIcon::Pointer;
    }

    let pane_order = pane_indices_by_z_desc(state);
    for pane_idx in pane_order.iter().copied() {
        let bounds = state.panes[pane_idx].bounds;
        if !bounds.contains(point) {
            continue;
        }

        let edge = state.pane_resizer.edge_at(bounds, point);
        if edge != ResizeEdge::None {
            return cursor_icon_for_resize_edge(edge);
        }

        if pane_title_bounds(bounds).contains(point) {
            return CursorIcon::Move;
        }

        let pane = &state.panes[pane_idx];
        let content_bounds = pane_content_bounds(bounds);

        match pane.kind {
            PaneKind::AutopilotChat => {
                let composer_height = chat_composer_height_for_value(
                    content_bounds,
                    state.chat_inputs.composer.get_value(),
                );
                if chat_composer_input_bounds_with_height(content_bounds, composer_height)
                    .contains(point)
                {
                    return CursorIcon::Text;
                }
            }
            PaneKind::Calculator => {
                if calculator_expression_input_bounds(content_bounds).contains(point) {
                    return CursorIcon::Text;
                }
            }
            PaneKind::RelayConnections => {
                if relay_connections_url_input_bounds(content_bounds).contains(point) {
                    return CursorIcon::Text;
                }
            }
            PaneKind::NetworkRequests => {
                if network_requests_type_input_bounds(content_bounds).contains(point)
                    || network_requests_payload_input_bounds(content_bounds).contains(point)
                    || network_requests_skill_scope_input_bounds(content_bounds).contains(point)
                    || network_requests_credit_envelope_input_bounds(content_bounds).contains(point)
                    || network_requests_budget_input_bounds(content_bounds).contains(point)
                    || network_requests_timeout_input_bounds(content_bounds).contains(point)
                    || network_requests_max_price_input_bounds(content_bounds).contains(point)
                {
                    return CursorIcon::Text;
                }
            }
            PaneKind::LocalInference => {
                if local_inference_prompt_input_bounds(content_bounds).contains(point)
                    || local_inference_requested_model_input_bounds(content_bounds).contains(point)
                    || local_inference_max_tokens_input_bounds(content_bounds).contains(point)
                    || local_inference_temperature_input_bounds(content_bounds).contains(point)
                    || local_inference_top_k_input_bounds(content_bounds).contains(point)
                    || local_inference_top_p_input_bounds(content_bounds).contains(point)
                {
                    return CursorIcon::Text;
                }
            }
            PaneKind::Settings => {
                if settings_relay_input_bounds(content_bounds).contains(point)
                    || settings_wallet_default_input_bounds(content_bounds).contains(point)
                    || settings_provider_queue_input_bounds(content_bounds).contains(point)
                {
                    return CursorIcon::Text;
                }
            }
            PaneKind::Credentials => {
                if credentials_name_input_bounds(content_bounds).contains(point)
                    || credentials_value_input_bounds(content_bounds).contains(point)
                {
                    return CursorIcon::Text;
                }
            }
            PaneKind::JobHistory => {
                if job_history_search_input_bounds(content_bounds).contains(point) {
                    return CursorIcon::Text;
                }
            }
            PaneKind::SparkWallet => {
                let layout = spark_pane::layout(content_bounds);
                if spark_pane::hits_input(layout, point) {
                    return CursorIcon::Text;
                }
            }
            PaneKind::SparkCreateInvoice => {
                let layout = spark_pane::create_invoice_layout(content_bounds);
                if spark_pane::hits_create_invoice_input(layout, point) {
                    return CursorIcon::Text;
                }
            }
            PaneKind::SparkPayInvoice => {
                let layout = spark_pane::pay_invoice_layout(content_bounds);
                if spark_pane::hits_pay_invoice_input(layout, point) {
                    return CursorIcon::Text;
                }
            }
            PaneKind::Empty
            | PaneKind::ProjectOps
            | PaneKind::CodexAccount
            | PaneKind::CodexModels
            | PaneKind::CodexConfig
            | PaneKind::CodexMcp
            | PaneKind::CodexApps
            | PaneKind::CodexLabs
            | PaneKind::CodexDiagnostics
            | PaneKind::GoOnline
            | PaneKind::ProviderStatus
            | PaneKind::EarningsScoreboard
            | PaneKind::SyncHealth
            | PaneKind::StarterJobs
            | PaneKind::ReciprocalLoop
            | PaneKind::ActivityFeed
            | PaneKind::AlertsRecovery
            | PaneKind::NostrIdentity
            | PaneKind::JobInbox
            | PaneKind::ActiveJob
            | PaneKind::AgentProfileState
            | PaneKind::AgentScheduleTick
            | PaneKind::TrajectoryAudit
            | PaneKind::CastControl
            | PaneKind::SkillRegistry
            | PaneKind::SkillTrustRevocation
            | PaneKind::CreditDesk
            | PaneKind::CreditSettlementLedger
            | PaneKind::CadDemo => {}
        }

        if let Some(action) = pane_hit_action_for_pane(state, pane, point) {
            if let PaneHitAction::CadDemo(cad_action) = action
                && cad_action_uses_dense_row_hot_zone(cad_action)
            {
                return CursorIcon::Default;
            }
            return CursorIcon::Pointer;
        }

        return CursorIcon::Default;
    }

    CursorIcon::Default
}

pub fn pane_content_bounds(bounds: Bounds) -> Bounds {
    let border = 1.0;
    Bounds::new(
        bounds.origin.x + border,
        bounds.origin.y + PANE_TITLE_HEIGHT,
        (bounds.size.width - border * 2.0).max(0.0),
        (bounds.size.height - PANE_TITLE_HEIGHT - border).max(0.0),
    )
}

pub fn chat_workspace_rail_bounds(content_bounds: Bounds) -> Bounds {
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        content_bounds.origin.y + CHAT_PAD,
        CHAT_WORKSPACE_RAIL_WIDTH,
        (content_bounds.size.height - CHAT_PAD * 2.0).max(120.0),
    )
}

pub fn chat_workspace_row_bounds(content_bounds: Bounds, index: usize) -> Bounds {
    let workspace = chat_workspace_rail_bounds(content_bounds);
    Bounds::new(
        workspace.origin.x + 4.0,
        workspace.origin.y + 28.0 + index as f32 * CHAT_WORKSPACE_SLOT_HEIGHT,
        (workspace.size.width - 8.0).max(32.0),
        CHAT_WORKSPACE_SLOT_HEIGHT,
    )
}

pub fn chat_visible_workspace_row_count(content_bounds: Bounds, total_workspaces: usize) -> usize {
    if total_workspaces == 0 {
        return 0;
    }

    let first_row = chat_workspace_row_bounds(content_bounds, 0);
    let rail = chat_workspace_rail_bounds(content_bounds);
    let available_height = (rail.max_y() - first_row.origin.y).max(0.0);
    if available_height < CHAT_WORKSPACE_SLOT_HEIGHT {
        return 0;
    }

    let max_fit = (available_height / CHAT_WORKSPACE_SLOT_HEIGHT).floor() as usize;
    total_workspaces.min(max_fit.max(1))
}

pub fn chat_thread_rail_bounds(content_bounds: Bounds) -> Bounds {
    let workspace = chat_workspace_rail_bounds(content_bounds);
    Bounds::new(
        workspace.max_x() + CHAT_COLUMN_GAP,
        content_bounds.origin.y + CHAT_PAD,
        CHAT_THREAD_RAIL_WIDTH,
        (content_bounds.size.height - CHAT_PAD * 2.0).max(120.0),
    )
}

fn chat_right_column_bounds(content_bounds: Bounds) -> Bounds {
    let rail = chat_thread_rail_bounds(content_bounds);
    let left = rail.max_x() + CHAT_COLUMN_GAP;
    Bounds::new(
        left,
        content_bounds.origin.y + CHAT_PAD,
        (content_bounds.max_x() - left - CHAT_PAD).max(240.0),
        (content_bounds.size.height - CHAT_PAD * 2.0).max(120.0),
    )
}

pub fn chat_refresh_threads_button_bounds(content_bounds: Bounds) -> Bounds {
    let rail = chat_thread_rail_bounds(content_bounds);
    Bounds::new(
        rail.origin.x + 10.0,
        rail.origin.y + 28.0,
        CHAT_HEADER_BUTTON_WIDTH,
        CHAT_HEADER_BUTTON_HEIGHT,
    )
}

pub fn chat_new_thread_button_bounds(content_bounds: Bounds) -> Bounds {
    let rail = chat_thread_rail_bounds(content_bounds);
    Bounds::new(
        rail.origin.x + rail.size.width - 10.0 - CHAT_NEW_THREAD_BUTTON_SIZE,
        rail.origin.y + 28.0,
        CHAT_NEW_THREAD_BUTTON_SIZE,
        CHAT_NEW_THREAD_BUTTON_SIZE,
    )
}

pub fn chat_cycle_model_button_bounds(content_bounds: Bounds) -> Bounds {
    let transcript = chat_transcript_bounds(content_bounds);
    Bounds::new(
        transcript.origin.x + 10.0,
        transcript.origin.y + 28.0,
        CHAT_HEADER_BUTTON_WIDTH,
        CHAT_HEADER_BUTTON_HEIGHT,
    )
}

pub fn chat_interrupt_button_bounds(content_bounds: Bounds) -> Bounds {
    let cycle = chat_cycle_model_button_bounds(content_bounds);
    Bounds::new(
        cycle.max_x() + 8.0,
        cycle.origin.y,
        CHAT_HEADER_BUTTON_WIDTH,
        CHAT_HEADER_BUTTON_HEIGHT,
    )
}

pub fn chat_thread_row_bounds(content_bounds: Bounds, index: usize) -> Bounds {
    let rail = chat_thread_rail_bounds(content_bounds);
    let y = rail.origin.y + 54.0 + index as f32 * (CHAT_SHELL_ROW_HEIGHT + CHAT_SHELL_ROW_GAP);
    Bounds::new(
        rail.origin.x + 8.0,
        y,
        (rail.size.width - 16.0).max(80.0),
        CHAT_SHELL_ROW_HEIGHT,
    )
}

pub fn chat_visible_thread_row_count(content_bounds: Bounds, total_threads: usize) -> usize {
    if total_threads == 0 {
        return 0;
    }

    let first_row = chat_thread_row_bounds(content_bounds, 0);
    let rail = chat_thread_rail_bounds(content_bounds);
    let available_height = (rail.max_y() - first_row.origin.y).max(0.0);
    if available_height < CHAT_SHELL_ROW_HEIGHT {
        return 0;
    }

    let row_span = CHAT_SHELL_ROW_HEIGHT + CHAT_SHELL_ROW_GAP;
    let max_fit = ((available_height + CHAT_SHELL_ROW_GAP) / row_span).floor() as usize;
    total_threads.min(CHAT_MAX_THREAD_ROWS).min(max_fit.max(1))
}

pub fn chat_thread_filter_archived_button_bounds(content_bounds: Bounds) -> Bounds {
    let new_thread = chat_new_thread_button_bounds(content_bounds);
    Bounds::new(
        new_thread.origin.x,
        new_thread.max_y() + 8.0,
        CHAT_THREAD_FILTER_BUTTON_WIDTH,
        CHAT_THREAD_FILTER_BUTTON_HEIGHT,
    )
}

pub fn chat_thread_filter_sort_button_bounds(content_bounds: Bounds) -> Bounds {
    let archived = chat_thread_filter_archived_button_bounds(content_bounds);
    Bounds::new(
        archived.max_x() + CHAT_THREAD_ACTION_BUTTON_GAP,
        archived.origin.y,
        CHAT_THREAD_FILTER_BUTTON_WIDTH,
        CHAT_THREAD_FILTER_BUTTON_HEIGHT,
    )
}

pub fn chat_thread_filter_source_button_bounds(content_bounds: Bounds) -> Bounds {
    let archived = chat_thread_filter_archived_button_bounds(content_bounds);
    Bounds::new(
        archived.origin.x,
        archived.max_y() + CHAT_THREAD_ACTION_BUTTON_GAP,
        CHAT_THREAD_FILTER_BUTTON_WIDTH,
        CHAT_THREAD_FILTER_BUTTON_HEIGHT,
    )
}

pub fn chat_thread_filter_provider_button_bounds(content_bounds: Bounds) -> Bounds {
    let source = chat_thread_filter_source_button_bounds(content_bounds);
    Bounds::new(
        source.max_x() + CHAT_THREAD_ACTION_BUTTON_GAP,
        source.origin.y,
        CHAT_THREAD_FILTER_BUTTON_WIDTH,
        CHAT_THREAD_FILTER_BUTTON_HEIGHT,
    )
}

pub fn chat_thread_action_fork_button_bounds(content_bounds: Bounds) -> Bounds {
    chat_thread_action_grid_bounds(content_bounds, 0)
}

pub fn chat_thread_action_archive_button_bounds(content_bounds: Bounds) -> Bounds {
    chat_thread_action_grid_bounds(content_bounds, 1)
}

pub fn chat_thread_action_unarchive_button_bounds(content_bounds: Bounds) -> Bounds {
    chat_thread_action_grid_bounds(content_bounds, 2)
}

pub fn chat_thread_action_rename_button_bounds(content_bounds: Bounds) -> Bounds {
    chat_thread_action_grid_bounds(content_bounds, 3)
}

pub fn chat_thread_action_rollback_button_bounds(content_bounds: Bounds) -> Bounds {
    chat_thread_action_grid_bounds(content_bounds, 4)
}

pub fn chat_thread_action_compact_button_bounds(content_bounds: Bounds) -> Bounds {
    chat_thread_action_grid_bounds(content_bounds, 5)
}

pub fn chat_thread_action_unsubscribe_button_bounds(content_bounds: Bounds) -> Bounds {
    chat_thread_action_grid_bounds(content_bounds, 6)
}

pub fn chat_send_button_bounds(content_bounds: Bounds) -> Bounds {
    let right_column = chat_right_column_bounds(content_bounds);
    Bounds::new(
        right_column.max_x() - CHAT_SEND_WIDTH,
        right_column.max_y() - CHAT_SEND_WIDTH,
        CHAT_SEND_WIDTH,
        CHAT_SEND_WIDTH,
    )
}

pub fn chat_server_request_accept_button_bounds(content_bounds: Bounds) -> Bounds {
    let transcript = chat_transcript_bounds(content_bounds);
    Bounds::new(
        transcript.origin.x + 240.0,
        transcript.origin.y + 28.0,
        96.0,
        24.0,
    )
}

pub fn chat_server_request_session_button_bounds(content_bounds: Bounds) -> Bounds {
    let accept = chat_server_request_accept_button_bounds(content_bounds);
    Bounds::new(accept.max_x() + 6.0, accept.origin.y, 96.0, 24.0)
}

pub fn chat_server_request_decline_button_bounds(content_bounds: Bounds) -> Bounds {
    let session = chat_server_request_session_button_bounds(content_bounds);
    Bounds::new(session.max_x() + 6.0, session.origin.y, 96.0, 24.0)
}

pub fn chat_server_request_cancel_button_bounds(content_bounds: Bounds) -> Bounds {
    let decline = chat_server_request_decline_button_bounds(content_bounds);
    Bounds::new(decline.max_x() + 6.0, decline.origin.y, 96.0, 24.0)
}

pub fn chat_server_tool_call_button_bounds(content_bounds: Bounds) -> Bounds {
    let accept = chat_server_request_accept_button_bounds(content_bounds);
    Bounds::new(accept.origin.x, accept.max_y() + 6.0, 120.0, 24.0)
}

pub fn chat_server_user_input_button_bounds(content_bounds: Bounds) -> Bounds {
    let tool = chat_server_tool_call_button_bounds(content_bounds);
    Bounds::new(tool.max_x() + 6.0, tool.origin.y, 140.0, 24.0)
}

pub fn chat_server_auth_refresh_button_bounds(content_bounds: Bounds) -> Bounds {
    let user_input = chat_server_user_input_button_bounds(content_bounds);
    Bounds::new(user_input.max_x() + 6.0, user_input.origin.y, 140.0, 24.0)
}

fn chat_composer_visual_line_count(value: &str, composer_width: f32) -> usize {
    let char_width = theme::font_size::SM * 0.6;
    let text_width = (composer_width - theme::spacing::SM * 2.0).max(char_width);
    let chars_per_line = (text_width / char_width).floor().max(1.0) as usize;
    let normalized = if value.is_empty() { " " } else { value };
    normalized
        .split('\n')
        .map(|line| {
            let chars = line.chars().count();
            chars.max(1).div_ceil(chars_per_line)
        })
        .sum::<usize>()
        .max(1)
}

pub fn chat_composer_height_for_value(content_bounds: Bounds, value: &str) -> f32 {
    let send_bounds = chat_send_button_bounds(content_bounds);
    let left = chat_right_column_bounds(content_bounds).origin.x;
    let composer_width = (send_bounds.origin.x - left - CHAT_PAD).max(120.0);
    let line_height = theme::font_size::SM * 1.4;
    let line_count = chat_composer_visual_line_count(value, composer_width);
    (line_height * line_count as f32 + theme::spacing::XS * 2.0)
        .clamp(CHAT_COMPOSER_MIN_HEIGHT, CHAT_COMPOSER_MAX_HEIGHT)
}

pub fn chat_composer_input_bounds_with_height(
    content_bounds: Bounds,
    composer_height: f32,
) -> Bounds {
    let send_bounds = chat_send_button_bounds(content_bounds);
    let right_column = chat_right_column_bounds(content_bounds);
    let left = right_column.origin.x;
    Bounds::new(
        left,
        send_bounds.max_y() - composer_height,
        (send_bounds.origin.x - left - CHAT_PAD).max(120.0),
        composer_height,
    )
}

pub fn chat_composer_input_bounds(content_bounds: Bounds) -> Bounds {
    chat_composer_input_bounds_with_height(content_bounds, CHAT_COMPOSER_MIN_HEIGHT)
}

pub fn chat_transcript_bounds_with_height(content_bounds: Bounds, composer_height: f32) -> Bounds {
    let composer_bounds = chat_composer_input_bounds_with_height(content_bounds, composer_height);
    let right_column = chat_right_column_bounds(content_bounds);
    Bounds::new(
        right_column.origin.x,
        right_column.origin.y,
        right_column.size.width,
        (composer_bounds.origin.y - right_column.origin.y - CHAT_PAD).max(120.0),
    )
}

pub fn calculator_expression_input_bounds(content_bounds: Bounds) -> Bounds {
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        content_bounds.origin.y + CHAT_PAD,
        (content_bounds.size.width - CHAT_PAD * 2.0).max(120.0),
        CALCULATOR_INPUT_HEIGHT,
    )
}

pub fn chat_transcript_bounds(content_bounds: Bounds) -> Bounds {
    chat_transcript_bounds_with_height(content_bounds, CHAT_COMPOSER_MIN_HEIGHT)
}

pub fn chat_transcript_body_bounds_with_height(
    content_bounds: Bounds,
    composer_height: f32,
) -> Bounds {
    let transcript = chat_transcript_bounds_with_height(content_bounds, composer_height);
    Bounds::new(
        transcript.origin.x + 8.0,
        transcript.origin.y + CHAT_TRANSCRIPT_HEADER_HEIGHT,
        (transcript.size.width - 16.0).max(160.0),
        (transcript.size.height - CHAT_TRANSCRIPT_HEADER_HEIGHT - 10.0).max(80.0),
    )
}

#[derive(Clone, Copy, Debug)]
pub struct MissionControlPaneLayout {
    pub status_row: Bounds,
    pub left_column: Bounds,
    pub right_column: Bounds,
    pub sell_panel: Bounds,
    pub earnings_panel: Bounds,
    pub wallet_panel: Bounds,
    pub actions_panel: Bounds,
    pub active_jobs_panel: Bounds,
    pub log_stream: Bounds,
}

pub fn mission_control_layout(content_bounds: Bounds) -> MissionControlPaneLayout {
    let outer_pad = CHAT_PAD;
    let column_gap = 14.0;
    let panel_gap = 12.0;
    let status_row = Bounds::new(
        content_bounds.origin.x + outer_pad,
        content_bounds.origin.y + outer_pad,
        (content_bounds.size.width - outer_pad * 2.0).max(0.0),
        22.0,
    );
    let body_top = status_row.max_y() + 12.0;
    let body_height = (content_bounds.max_y() - body_top - outer_pad).max(0.0);
    let available_width = (content_bounds.size.width - outer_pad * 2.0).max(0.0);
    let tentative_left = (available_width * 0.34).clamp(220.0, 340.0);
    let max_left = (available_width - column_gap - 180.0).max(140.0);
    let left_width = tentative_left.min(max_left).max(140.0);
    let right_width = (available_width - left_width - column_gap).max(140.0);
    let left_column = Bounds::new(
        content_bounds.origin.x + outer_pad,
        body_top,
        left_width,
        body_height,
    );
    let right_column = Bounds::new(
        left_column.max_x() + column_gap,
        body_top,
        right_width,
        body_height,
    );

    let preferred_sell = 126.0;
    let preferred_earnings = 172.0;
    let preferred_wallet = 126.0;
    let preferred_actions = 88.0;
    let preferred_total = preferred_sell
        + preferred_earnings
        + preferred_wallet
        + preferred_actions
        + panel_gap * 3.0;
    let scale = if preferred_total > 0.0 {
        (body_height / preferred_total).min(1.0)
    } else {
        1.0
    };
    let sell_height = preferred_sell * scale;
    let earnings_height = preferred_earnings * scale;
    let wallet_height = preferred_wallet * scale;
    let actions_height =
        (body_height - sell_height - earnings_height - wallet_height - panel_gap * 3.0).max(0.0);

    let sell_panel = Bounds::new(
        left_column.origin.x,
        left_column.origin.y,
        left_column.size.width,
        sell_height,
    );
    let earnings_panel = Bounds::new(
        left_column.origin.x,
        sell_panel.max_y() + panel_gap,
        left_column.size.width,
        earnings_height,
    );
    let wallet_panel = Bounds::new(
        left_column.origin.x,
        earnings_panel.max_y() + panel_gap,
        left_column.size.width,
        wallet_height,
    );
    let actions_panel = Bounds::new(
        left_column.origin.x,
        wallet_panel.max_y() + panel_gap,
        left_column.size.width,
        actions_height,
    );

    let active_jobs_height = (88.0 * scale).max(60.0_f32.min(body_height));
    let active_jobs_panel = Bounds::new(
        right_column.origin.x,
        right_column.origin.y,
        right_column.size.width,
        active_jobs_height,
    );
    let log_stream = Bounds::new(
        right_column.origin.x,
        active_jobs_panel.max_y() + panel_gap,
        right_column.size.width,
        (right_column.size.height - active_jobs_height - panel_gap).max(0.0),
    );

    MissionControlPaneLayout {
        status_row,
        left_column,
        right_column,
        sell_panel,
        earnings_panel,
        wallet_panel,
        actions_panel,
        active_jobs_panel,
        log_stream,
    }
}

pub fn go_online_toggle_button_bounds(content_bounds: Bounds) -> Bounds {
    let panel = mission_control_layout(content_bounds).sell_panel;
    Bounds::new(
        panel.origin.x + 14.0,
        panel.origin.y + 44.0,
        (panel.size.width - 28.0).max(0.0),
        68.0,
    )
}

pub fn mission_control_amount_toggle_button_bounds(content_bounds: Bounds) -> Bounds {
    let panel = mission_control_layout(content_bounds).earnings_panel;
    let width = (panel.size.width * 0.42)
        .clamp(116.0, 156.0)
        .min((panel.size.width - 28.0).max(0.0));
    Bounds::new(
        panel.max_x() - width - 14.0,
        panel.origin.y + 14.0,
        width,
        24.0,
    )
}

pub fn mission_control_download_model_button_bounds(content_bounds: Bounds) -> Bounds {
    let panel = mission_control_layout(content_bounds).actions_panel;
    Bounds::new(
        panel.origin.x + 14.0,
        panel.origin.y + 30.0,
        (panel.size.width - 28.0).max(0.0),
        30.0,
    )
}

pub fn mission_control_documentation_button_bounds(content_bounds: Bounds) -> Bounds {
    let download = mission_control_download_model_button_bounds(content_bounds);
    Bounds::new(
        download.origin.x,
        download.max_y() + 10.0,
        download.size.width,
        download.size.height,
    )
}

pub fn mission_control_log_stream_bounds(content_bounds: Bounds) -> Bounds {
    mission_control_layout(content_bounds).log_stream
}

pub fn provider_inventory_toggle_button_bounds(content_bounds: Bounds, row_index: usize) -> Bounds {
    let safe_index = row_index.min(2);
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        content_bounds.origin.y + CHAT_PAD + safe_index as f32 * 40.0,
        (content_bounds.size.width * 0.34).clamp(180.0, 260.0),
        JOB_INBOX_BUTTON_HEIGHT,
    )
}

pub fn codex_account_refresh_button_bounds(content_bounds: Bounds) -> Bounds {
    codex_action_button_bounds(content_bounds, 0, 0, 3)
}

pub fn codex_account_login_button_bounds(content_bounds: Bounds) -> Bounds {
    codex_action_button_bounds(content_bounds, 0, 1, 3)
}

pub fn codex_account_cancel_login_button_bounds(content_bounds: Bounds) -> Bounds {
    codex_action_button_bounds(content_bounds, 0, 2, 3)
}

pub fn codex_account_logout_button_bounds(content_bounds: Bounds) -> Bounds {
    codex_action_button_bounds(content_bounds, 1, 0, 3)
}

pub fn codex_account_rate_limits_button_bounds(content_bounds: Bounds) -> Bounds {
    codex_action_button_bounds(content_bounds, 1, 1, 3)
}

pub fn codex_models_refresh_button_bounds(content_bounds: Bounds) -> Bounds {
    codex_action_button_bounds(content_bounds, 0, 0, 2)
}

pub fn codex_models_toggle_hidden_button_bounds(content_bounds: Bounds) -> Bounds {
    codex_action_button_bounds(content_bounds, 0, 1, 2)
}

pub fn codex_config_read_button_bounds(content_bounds: Bounds) -> Bounds {
    codex_action_button_bounds(content_bounds, 0, 0, 3)
}

pub fn codex_config_requirements_button_bounds(content_bounds: Bounds) -> Bounds {
    codex_action_button_bounds(content_bounds, 0, 1, 3)
}

pub fn codex_config_write_button_bounds(content_bounds: Bounds) -> Bounds {
    codex_action_button_bounds(content_bounds, 0, 2, 3)
}

pub fn codex_config_batch_write_button_bounds(content_bounds: Bounds) -> Bounds {
    codex_action_button_bounds(content_bounds, 1, 0, 3)
}

pub fn codex_config_detect_external_button_bounds(content_bounds: Bounds) -> Bounds {
    codex_action_button_bounds(content_bounds, 1, 1, 3)
}

pub fn codex_config_import_external_button_bounds(content_bounds: Bounds) -> Bounds {
    codex_action_button_bounds(content_bounds, 1, 2, 3)
}

pub fn codex_mcp_refresh_button_bounds(content_bounds: Bounds) -> Bounds {
    codex_action_button_bounds(content_bounds, 0, 0, 3)
}

pub fn codex_mcp_login_button_bounds(content_bounds: Bounds) -> Bounds {
    codex_action_button_bounds(content_bounds, 0, 1, 3)
}

pub fn codex_mcp_reload_button_bounds(content_bounds: Bounds) -> Bounds {
    codex_action_button_bounds(content_bounds, 0, 2, 3)
}

pub fn codex_mcp_row_bounds(content_bounds: Bounds, row_index: usize) -> Bounds {
    let safe_index = row_index.min(CODEX_MCP_MAX_ROWS.saturating_sub(1));
    let top = codex_mcp_refresh_button_bounds(content_bounds).max_y() + 12.0;
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        top + safe_index as f32 * (CODEX_MCP_ROW_HEIGHT + CODEX_MCP_ROW_GAP),
        (content_bounds.size.width - CHAT_PAD * 2.0).max(220.0),
        CODEX_MCP_ROW_HEIGHT,
    )
}

pub fn codex_mcp_visible_row_count(row_count: usize) -> usize {
    row_count.min(CODEX_MCP_MAX_ROWS)
}

pub fn codex_apps_refresh_button_bounds(content_bounds: Bounds) -> Bounds {
    codex_action_button_bounds(content_bounds, 0, 0, 2)
}

pub fn codex_apps_row_bounds(content_bounds: Bounds, row_index: usize) -> Bounds {
    let safe_index = row_index.min(CODEX_APPS_MAX_ROWS.saturating_sub(1));
    let top = codex_apps_refresh_button_bounds(content_bounds).max_y() + 12.0;
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        top + safe_index as f32 * (CODEX_APPS_ROW_HEIGHT + CODEX_APPS_ROW_GAP),
        (content_bounds.size.width - CHAT_PAD * 2.0).max(220.0),
        CODEX_APPS_ROW_HEIGHT,
    )
}

pub fn codex_apps_visible_row_count(row_count: usize) -> usize {
    row_count.min(CODEX_APPS_MAX_ROWS)
}

pub fn codex_labs_review_inline_button_bounds(content_bounds: Bounds) -> Bounds {
    codex_labs_button_bounds(content_bounds, 0, 0)
}

pub fn codex_labs_review_detached_button_bounds(content_bounds: Bounds) -> Bounds {
    codex_labs_button_bounds(content_bounds, 0, 1)
}

pub fn codex_labs_command_exec_button_bounds(content_bounds: Bounds) -> Bounds {
    codex_labs_button_bounds(content_bounds, 0, 2)
}

pub fn codex_labs_collaboration_modes_button_bounds(content_bounds: Bounds) -> Bounds {
    codex_labs_button_bounds(content_bounds, 1, 0)
}

pub fn codex_labs_experimental_features_button_bounds(content_bounds: Bounds) -> Bounds {
    codex_labs_button_bounds(content_bounds, 1, 1)
}

pub fn codex_labs_toggle_experimental_button_bounds(content_bounds: Bounds) -> Bounds {
    codex_labs_button_bounds(content_bounds, 1, 2)
}

pub fn codex_labs_realtime_start_button_bounds(content_bounds: Bounds) -> Bounds {
    codex_labs_button_bounds(content_bounds, 2, 0)
}

pub fn codex_labs_realtime_append_text_button_bounds(content_bounds: Bounds) -> Bounds {
    codex_labs_button_bounds(content_bounds, 2, 1)
}

pub fn codex_labs_realtime_stop_button_bounds(content_bounds: Bounds) -> Bounds {
    codex_labs_button_bounds(content_bounds, 2, 2)
}

pub fn codex_labs_windows_sandbox_setup_button_bounds(content_bounds: Bounds) -> Bounds {
    codex_labs_button_bounds(content_bounds, 3, 0)
}

pub fn codex_labs_fuzzy_start_button_bounds(content_bounds: Bounds) -> Bounds {
    codex_labs_button_bounds(content_bounds, 3, 1)
}

pub fn codex_labs_fuzzy_update_button_bounds(content_bounds: Bounds) -> Bounds {
    codex_labs_button_bounds(content_bounds, 3, 2)
}

pub fn codex_labs_fuzzy_stop_button_bounds(content_bounds: Bounds) -> Bounds {
    codex_labs_button_bounds(content_bounds, 4, 0)
}

pub fn codex_diagnostics_enable_wire_log_button_bounds(content_bounds: Bounds) -> Bounds {
    codex_diagnostics_button_bounds(content_bounds, 0)
}

pub fn codex_diagnostics_disable_wire_log_button_bounds(content_bounds: Bounds) -> Bounds {
    codex_diagnostics_button_bounds(content_bounds, 1)
}

pub fn codex_diagnostics_clear_events_button_bounds(content_bounds: Bounds) -> Bounds {
    codex_diagnostics_button_bounds(content_bounds, 2)
}

pub fn earnings_scoreboard_refresh_button_bounds(content_bounds: Bounds) -> Bounds {
    let width = content_bounds.size.width.clamp(160.0, 220.0);
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        content_bounds.origin.y + CHAT_PAD,
        width,
        34.0,
    )
}

pub fn relay_connections_url_input_bounds(content_bounds: Bounds) -> Bounds {
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        content_bounds.origin.y + CHAT_PAD,
        (content_bounds.size.width * 0.48).clamp(220.0, 420.0),
        JOB_INBOX_BUTTON_HEIGHT,
    )
}

pub fn relay_connections_add_button_bounds(content_bounds: Bounds) -> Bounds {
    let input = relay_connections_url_input_bounds(content_bounds);
    Bounds::new(
        input.max_x() + JOB_INBOX_BUTTON_GAP,
        input.origin.y,
        118.0,
        input.size.height,
    )
}

pub fn relay_connections_remove_button_bounds(content_bounds: Bounds) -> Bounds {
    let add = relay_connections_add_button_bounds(content_bounds);
    Bounds::new(
        add.max_x() + JOB_INBOX_BUTTON_GAP,
        add.origin.y,
        154.0,
        add.size.height,
    )
}

pub fn relay_connections_retry_button_bounds(content_bounds: Bounds) -> Bounds {
    let remove = relay_connections_remove_button_bounds(content_bounds);
    Bounds::new(
        remove.max_x() + JOB_INBOX_BUTTON_GAP,
        remove.origin.y,
        126.0,
        remove.size.height,
    )
}

pub fn relay_connections_row_bounds(content_bounds: Bounds, row_index: usize) -> Bounds {
    let safe_index = row_index.min(RELAY_CONNECTIONS_MAX_ROWS.saturating_sub(1));
    let top = content_bounds.origin.y + CHAT_PAD + JOB_INBOX_BUTTON_HEIGHT + 12.0;
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        top + safe_index as f32 * (RELAY_CONNECTIONS_ROW_HEIGHT + RELAY_CONNECTIONS_ROW_GAP),
        (content_bounds.size.width - CHAT_PAD * 2.0).max(240.0),
        RELAY_CONNECTIONS_ROW_HEIGHT,
    )
}

pub fn relay_connections_visible_row_count(row_count: usize) -> usize {
    row_count.min(RELAY_CONNECTIONS_MAX_ROWS)
}

pub fn sync_health_rebootstrap_button_bounds(content_bounds: Bounds) -> Bounds {
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        content_bounds.origin.y + CHAT_PAD,
        (content_bounds.size.width * 0.28).clamp(160.0, 240.0),
        JOB_INBOX_BUTTON_HEIGHT,
    )
}

pub fn local_inference_refresh_button_bounds(content_bounds: Bounds) -> Bounds {
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        content_bounds.origin.y + CHAT_PAD,
        126.0,
        JOB_INBOX_BUTTON_HEIGHT,
    )
}

pub fn local_inference_warm_button_bounds(content_bounds: Bounds) -> Bounds {
    let refresh = local_inference_refresh_button_bounds(content_bounds);
    Bounds::new(
        refresh.max_x() + JOB_INBOX_BUTTON_GAP,
        refresh.origin.y,
        116.0,
        refresh.size.height,
    )
}

pub fn local_inference_unload_button_bounds(content_bounds: Bounds) -> Bounds {
    let warm = local_inference_warm_button_bounds(content_bounds);
    Bounds::new(
        warm.max_x() + JOB_INBOX_BUTTON_GAP,
        warm.origin.y,
        116.0,
        warm.size.height,
    )
}

pub fn local_inference_run_button_bounds(content_bounds: Bounds) -> Bounds {
    let unload = local_inference_unload_button_bounds(content_bounds);
    Bounds::new(
        unload.max_x() + JOB_INBOX_BUTTON_GAP,
        unload.origin.y,
        148.0,
        unload.size.height,
    )
}

pub fn local_inference_prompt_input_bounds(content_bounds: Bounds) -> Bounds {
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        content_bounds.origin.y + CHAT_PAD + JOB_INBOX_BUTTON_HEIGHT + 62.0,
        (content_bounds.size.width - CHAT_PAD * 2.0).max(320.0),
        JOB_INBOX_BUTTON_HEIGHT,
    )
}

pub fn local_inference_requested_model_input_bounds(content_bounds: Bounds) -> Bounds {
    let prompt = local_inference_prompt_input_bounds(content_bounds);
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        prompt.max_y() + 36.0,
        (content_bounds.size.width * 0.28).clamp(180.0, 250.0),
        JOB_INBOX_BUTTON_HEIGHT,
    )
}

pub fn local_inference_max_tokens_input_bounds(content_bounds: Bounds) -> Bounds {
    let requested_model = local_inference_requested_model_input_bounds(content_bounds);
    Bounds::new(
        requested_model.max_x() + JOB_INBOX_BUTTON_GAP,
        requested_model.origin.y,
        98.0,
        requested_model.size.height,
    )
}

pub fn local_inference_temperature_input_bounds(content_bounds: Bounds) -> Bounds {
    let max_tokens = local_inference_max_tokens_input_bounds(content_bounds);
    Bounds::new(
        max_tokens.max_x() + JOB_INBOX_BUTTON_GAP,
        max_tokens.origin.y,
        108.0,
        max_tokens.size.height,
    )
}

pub fn local_inference_top_k_input_bounds(content_bounds: Bounds) -> Bounds {
    let temperature = local_inference_temperature_input_bounds(content_bounds);
    Bounds::new(
        temperature.max_x() + JOB_INBOX_BUTTON_GAP,
        temperature.origin.y,
        92.0,
        temperature.size.height,
    )
}

pub fn local_inference_top_p_input_bounds(content_bounds: Bounds) -> Bounds {
    let top_k = local_inference_top_k_input_bounds(content_bounds);
    Bounds::new(
        top_k.max_x() + JOB_INBOX_BUTTON_GAP,
        top_k.origin.y,
        92.0,
        top_k.size.height,
    )
}

pub fn network_requests_type_input_bounds(content_bounds: Bounds) -> Bounds {
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        content_bounds.origin.y + CHAT_PAD,
        (content_bounds.size.width * 0.45).clamp(220.0, 420.0),
        JOB_INBOX_BUTTON_HEIGHT,
    )
}

pub fn network_requests_payload_input_bounds(content_bounds: Bounds) -> Bounds {
    let type_input = network_requests_type_input_bounds(content_bounds);
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        type_input.max_y() + 10.0,
        (content_bounds.size.width - CHAT_PAD * 2.0).max(260.0),
        JOB_INBOX_BUTTON_HEIGHT,
    )
}

pub fn network_requests_skill_scope_input_bounds(content_bounds: Bounds) -> Bounds {
    let payload = network_requests_payload_input_bounds(content_bounds);
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        payload.max_y() + 10.0,
        (content_bounds.size.width - CHAT_PAD * 2.0).max(260.0),
        JOB_INBOX_BUTTON_HEIGHT,
    )
}

pub fn network_requests_credit_envelope_input_bounds(content_bounds: Bounds) -> Bounds {
    let scope = network_requests_skill_scope_input_bounds(content_bounds);
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        scope.max_y() + 10.0,
        (content_bounds.size.width - CHAT_PAD * 2.0).max(260.0),
        JOB_INBOX_BUTTON_HEIGHT,
    )
}

pub fn network_requests_budget_input_bounds(content_bounds: Bounds) -> Bounds {
    let envelope = network_requests_credit_envelope_input_bounds(content_bounds);
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        envelope.max_y() + 10.0,
        (content_bounds.size.width * 0.2).clamp(120.0, 180.0),
        JOB_INBOX_BUTTON_HEIGHT,
    )
}

pub fn network_requests_timeout_input_bounds(content_bounds: Bounds) -> Bounds {
    let budget = network_requests_budget_input_bounds(content_bounds);
    Bounds::new(
        budget.max_x() + JOB_INBOX_BUTTON_GAP,
        budget.origin.y,
        budget.size.width,
        budget.size.height,
    )
}

pub fn network_requests_max_price_input_bounds(content_bounds: Bounds) -> Bounds {
    let timeout = network_requests_timeout_input_bounds(content_bounds);
    Bounds::new(
        timeout.max_x() + JOB_INBOX_BUTTON_GAP,
        timeout.origin.y,
        timeout.size.width,
        timeout.size.height,
    )
}

pub fn network_requests_submit_button_bounds(content_bounds: Bounds) -> Bounds {
    let max_price = network_requests_max_price_input_bounds(content_bounds);
    Bounds::new(
        max_price.max_x() + JOB_INBOX_BUTTON_GAP,
        max_price.origin.y,
        (content_bounds.max_x() - max_price.max_x() - CHAT_PAD - JOB_INBOX_BUTTON_GAP).max(140.0),
        max_price.size.height,
    )
}

pub fn network_requests_accept_button_bounds(content_bounds: Bounds) -> Bounds {
    let submit = network_requests_submit_button_bounds(content_bounds);
    Bounds::new(
        submit.origin.x,
        submit.max_y() + 10.0,
        submit.size.width,
        submit.size.height,
    )
}

pub fn network_requests_quote_row_bounds(content_bounds: Bounds, row_index: usize) -> Bounds {
    let safe_index = row_index.min(4);
    let accept = network_requests_accept_button_bounds(content_bounds);
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        accept.max_y() + 56.0 + safe_index as f32 * (JOB_INBOX_ROW_HEIGHT + JOB_INBOX_ROW_GAP),
        (content_bounds.size.width - CHAT_PAD * 2.0).max(240.0),
        JOB_INBOX_ROW_HEIGHT,
    )
}

pub fn network_requests_visible_quote_count(row_count: usize) -> usize {
    row_count.min(5)
}

pub fn starter_jobs_complete_button_bounds(content_bounds: Bounds) -> Bounds {
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        content_bounds.origin.y + CHAT_PAD,
        (content_bounds.size.width * 0.28).clamp(160.0, 240.0),
        JOB_INBOX_BUTTON_HEIGHT,
    )
}

pub fn starter_jobs_kill_switch_button_bounds(content_bounds: Bounds) -> Bounds {
    let complete = starter_jobs_complete_button_bounds(content_bounds);
    Bounds::new(
        complete.max_x() + JOB_INBOX_BUTTON_GAP,
        complete.origin.y,
        (content_bounds.size.width * 0.28).clamp(160.0, 240.0),
        complete.size.height,
    )
}

pub fn reciprocal_loop_start_button_bounds(content_bounds: Bounds) -> Bounds {
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        content_bounds.origin.y + CHAT_PAD,
        (content_bounds.size.width * 0.24).clamp(140.0, 210.0),
        JOB_INBOX_BUTTON_HEIGHT,
    )
}

pub fn reciprocal_loop_stop_button_bounds(content_bounds: Bounds) -> Bounds {
    let start = reciprocal_loop_start_button_bounds(content_bounds);
    Bounds::new(
        start.max_x() + JOB_INBOX_BUTTON_GAP,
        start.origin.y,
        start.size.width,
        start.size.height,
    )
}

pub fn reciprocal_loop_reset_button_bounds(content_bounds: Bounds) -> Bounds {
    let stop = reciprocal_loop_stop_button_bounds(content_bounds);
    Bounds::new(
        stop.max_x() + JOB_INBOX_BUTTON_GAP,
        stop.origin.y,
        stop.size.width,
        stop.size.height,
    )
}

pub fn starter_jobs_row_bounds(content_bounds: Bounds, row_index: usize) -> Bounds {
    let safe_index = row_index.min(JOB_INBOX_MAX_ROWS.saturating_sub(1));
    let top = content_bounds.origin.y + CHAT_PAD + JOB_INBOX_BUTTON_HEIGHT + 12.0;
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        top + safe_index as f32 * (JOB_INBOX_ROW_HEIGHT + JOB_INBOX_ROW_GAP),
        (content_bounds.size.width - CHAT_PAD * 2.0).max(220.0),
        JOB_INBOX_ROW_HEIGHT,
    )
}

pub fn starter_jobs_visible_row_count(row_count: usize) -> usize {
    row_count.min(JOB_INBOX_MAX_ROWS)
}

pub fn activity_feed_refresh_button_bounds(content_bounds: Bounds) -> Bounds {
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        content_bounds.origin.y + CHAT_PAD,
        (content_bounds.size.width * 0.24).clamp(148.0, 220.0),
        JOB_INBOX_BUTTON_HEIGHT,
    )
}

pub fn activity_feed_prev_page_button_bounds(content_bounds: Bounds) -> Bounds {
    let refresh = activity_feed_refresh_button_bounds(content_bounds);
    Bounds::new(
        refresh.max_x() + JOB_INBOX_BUTTON_GAP,
        refresh.origin.y,
        (content_bounds.size.width * 0.12).clamp(84.0, 120.0),
        refresh.size.height,
    )
}

pub fn activity_feed_next_page_button_bounds(content_bounds: Bounds) -> Bounds {
    let prev = activity_feed_prev_page_button_bounds(content_bounds);
    Bounds::new(
        prev.max_x() + JOB_INBOX_BUTTON_GAP,
        prev.origin.y,
        prev.size.width,
        prev.size.height,
    )
}

pub fn activity_feed_filter_button_bounds(content_bounds: Bounds, filter_index: usize) -> Bounds {
    let filters = ActivityFeedFilter::all();
    let max_index = filters.len().saturating_sub(1);
    let safe_index = filter_index.min(max_index);
    let top = activity_feed_refresh_button_bounds(content_bounds).max_y() + 10.0;
    let count = filters.len() as f32;
    let gap_total = ACTIVITY_FEED_FILTER_GAP * (count - 1.0);
    let usable_width = (content_bounds.size.width - CHAT_PAD * 2.0 - gap_total).max(300.0);
    let button_width = (usable_width / count).clamp(72.0, 142.0);
    Bounds::new(
        content_bounds.origin.x
            + CHAT_PAD
            + safe_index as f32 * (button_width + ACTIVITY_FEED_FILTER_GAP),
        top,
        button_width,
        ACTIVITY_FEED_FILTER_BUTTON_HEIGHT,
    )
}

pub fn activity_feed_row_bounds(content_bounds: Bounds, row_index: usize) -> Bounds {
    let safe_index = row_index.min(ACTIVITY_FEED_MAX_ROWS.saturating_sub(1));
    let top = activity_feed_filter_button_bounds(content_bounds, 0).max_y() + 12.0;
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        top + safe_index as f32 * (ACTIVITY_FEED_ROW_HEIGHT + ACTIVITY_FEED_ROW_GAP),
        (content_bounds.size.width - CHAT_PAD * 2.0).max(220.0),
        ACTIVITY_FEED_ROW_HEIGHT,
    )
}

pub fn activity_feed_visible_row_count(row_count: usize) -> usize {
    row_count.min(ACTIVITY_FEED_MAX_ROWS)
}

pub fn activity_feed_details_bounds(content_bounds: Bounds, visible_rows: usize) -> Option<Bounds> {
    if visible_rows == 0 {
        return None;
    }
    let details_top = activity_feed_row_bounds(content_bounds, visible_rows.saturating_sub(1))
        .max_y()
        + ACTIVITY_FEED_DETAILS_TOP_GAP;
    let details_height = content_bounds.max_y() - CHAT_PAD - details_top;
    if details_height <= 0.0 {
        return None;
    }
    Some(Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        details_top,
        (content_bounds.size.width - CHAT_PAD * 2.0).max(220.0),
        details_height,
    ))
}

pub fn activity_feed_detail_viewport_bounds(
    content_bounds: Bounds,
    visible_rows: usize,
) -> Option<Bounds> {
    let details_bounds = activity_feed_details_bounds(content_bounds, visible_rows)?;
    let value_x = details_bounds.origin.x
        + ACTIVITY_FEED_DETAILS_LABEL_INSET_X
        + ACTIVITY_FEED_DETAILS_VALUE_OFFSET_X;
    let value_y = details_bounds.origin.y
        + ACTIVITY_FEED_DETAILS_LINE_HEIGHT * ACTIVITY_FEED_DETAILS_HEADER_LINES as f32;
    let value_width =
        (details_bounds.max_x() - value_x - ACTIVITY_FEED_DETAILS_RIGHT_PADDING).max(40.0);
    let value_height = details_bounds.max_y() - value_y;
    if value_height <= 0.0 {
        return None;
    }
    Some(Bounds::new(value_x, value_y, value_width, value_height))
}

pub fn activity_feed_detail_visible_line_capacity(
    content_bounds: Bounds,
    visible_rows: usize,
) -> usize {
    let Some(viewport) = activity_feed_detail_viewport_bounds(content_bounds, visible_rows) else {
        return 0;
    };
    ((viewport.size.height / ACTIVITY_FEED_DETAILS_LINE_HEIGHT).floor() as usize).max(1)
}

pub fn activity_feed_detail_wrapped_line_count(detail: &str) -> usize {
    wrapped_line_count_for_display(detail, ACTIVITY_FEED_DETAILS_WRAP_CHARS)
}

fn wrapped_line_count_for_display(text: &str, chunk_len: usize) -> usize {
    if text.trim().is_empty() {
        return 1;
    }
    let chunk_len = chunk_len.max(1);
    let mut lines = 0usize;
    for line in text.lines() {
        let line_chars = line.chars().count();
        lines += if line_chars == 0 {
            1
        } else {
            line_chars.div_ceil(chunk_len)
        };
    }
    if text.ends_with('\n') {
        lines += 1;
    }
    lines.max(1)
}

pub fn alerts_recovery_recover_button_bounds(content_bounds: Bounds) -> Bounds {
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        content_bounds.origin.y + CHAT_PAD,
        (content_bounds.size.width * 0.2).clamp(132.0, 200.0),
        JOB_INBOX_BUTTON_HEIGHT,
    )
}

pub fn alerts_recovery_ack_button_bounds(content_bounds: Bounds) -> Bounds {
    let recover = alerts_recovery_recover_button_bounds(content_bounds);
    Bounds::new(
        recover.max_x() + JOB_INBOX_BUTTON_GAP,
        recover.origin.y,
        recover.size.width,
        recover.size.height,
    )
}

pub fn alerts_recovery_resolve_button_bounds(content_bounds: Bounds) -> Bounds {
    let ack = alerts_recovery_ack_button_bounds(content_bounds);
    Bounds::new(
        ack.max_x() + JOB_INBOX_BUTTON_GAP,
        ack.origin.y,
        ack.size.width,
        ack.size.height,
    )
}

pub fn alerts_recovery_row_bounds(content_bounds: Bounds, row_index: usize) -> Bounds {
    let safe_index = row_index.min(ALERTS_RECOVERY_MAX_ROWS.saturating_sub(1));
    let top = content_bounds.origin.y + CHAT_PAD + JOB_INBOX_BUTTON_HEIGHT + 12.0;
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        top + safe_index as f32 * (ALERTS_RECOVERY_ROW_HEIGHT + ALERTS_RECOVERY_ROW_GAP),
        (content_bounds.size.width - CHAT_PAD * 2.0).max(220.0),
        ALERTS_RECOVERY_ROW_HEIGHT,
    )
}

pub fn alerts_recovery_visible_row_count(row_count: usize) -> usize {
    row_count.min(ALERTS_RECOVERY_MAX_ROWS)
}

pub fn settings_relay_input_bounds(content_bounds: Bounds) -> Bounds {
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        content_bounds.origin.y + CHAT_PAD,
        (content_bounds.size.width * 0.6).clamp(260.0, 560.0),
        JOB_INBOX_BUTTON_HEIGHT,
    )
}

pub fn settings_wallet_default_input_bounds(content_bounds: Bounds) -> Bounds {
    let relay = settings_relay_input_bounds(content_bounds);
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        relay.max_y() + 10.0,
        (content_bounds.size.width * 0.24).clamp(150.0, 220.0),
        JOB_INBOX_BUTTON_HEIGHT,
    )
}

pub fn settings_provider_queue_input_bounds(content_bounds: Bounds) -> Bounds {
    let wallet = settings_wallet_default_input_bounds(content_bounds);
    Bounds::new(
        wallet.max_x() + JOB_INBOX_BUTTON_GAP,
        wallet.origin.y,
        wallet.size.width,
        wallet.size.height,
    )
}

pub fn settings_save_button_bounds(content_bounds: Bounds) -> Bounds {
    let provider = settings_provider_queue_input_bounds(content_bounds);
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        provider.max_y() + 10.0,
        140.0,
        JOB_INBOX_BUTTON_HEIGHT,
    )
}

pub fn settings_reset_button_bounds(content_bounds: Bounds) -> Bounds {
    let save = settings_save_button_bounds(content_bounds);
    Bounds::new(
        save.max_x() + JOB_INBOX_BUTTON_GAP,
        save.origin.y,
        168.0,
        JOB_INBOX_BUTTON_HEIGHT,
    )
}

pub fn credentials_name_input_bounds(content_bounds: Bounds) -> Bounds {
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        content_bounds.origin.y + CHAT_PAD,
        (content_bounds.size.width * 0.34).clamp(220.0, 340.0),
        CREDENTIALS_BUTTON_HEIGHT,
    )
}

pub fn credentials_value_input_bounds(content_bounds: Bounds) -> Bounds {
    let name = credentials_name_input_bounds(content_bounds);
    Bounds::new(
        name.max_x() + CREDENTIALS_BUTTON_GAP,
        name.origin.y,
        (content_bounds.size.width - CHAT_PAD * 2.0 - name.size.width - CREDENTIALS_BUTTON_GAP)
            .max(240.0),
        CREDENTIALS_BUTTON_HEIGHT,
    )
}

pub fn credentials_add_custom_button_bounds(content_bounds: Bounds) -> Bounds {
    credentials_button_bounds(content_bounds, 0, 0)
}

pub fn credentials_save_value_button_bounds(content_bounds: Bounds) -> Bounds {
    credentials_button_bounds(content_bounds, 0, 1)
}

pub fn credentials_delete_button_bounds(content_bounds: Bounds) -> Bounds {
    credentials_button_bounds(content_bounds, 0, 2)
}

pub fn credentials_toggle_enabled_button_bounds(content_bounds: Bounds) -> Bounds {
    credentials_button_bounds(content_bounds, 0, 3)
}

pub fn credentials_import_button_bounds(content_bounds: Bounds) -> Bounds {
    credentials_button_bounds(content_bounds, 1, 0)
}

pub fn credentials_reload_button_bounds(content_bounds: Bounds) -> Bounds {
    credentials_button_bounds(content_bounds, 1, 1)
}

pub fn credentials_scope_codex_button_bounds(content_bounds: Bounds) -> Bounds {
    credentials_button_bounds(content_bounds, 1, 2)
}

pub fn credentials_scope_spark_button_bounds(content_bounds: Bounds) -> Bounds {
    credentials_button_bounds(content_bounds, 1, 3)
}

pub fn credentials_scope_skills_button_bounds(content_bounds: Bounds) -> Bounds {
    credentials_button_bounds(content_bounds, 1, 4)
}

pub fn credentials_scope_global_button_bounds(content_bounds: Bounds) -> Bounds {
    credentials_button_bounds(content_bounds, 1, 5)
}

pub fn credentials_row_bounds(content_bounds: Bounds, row_index: usize) -> Bounds {
    let safe_index = row_index.min(CREDENTIALS_MAX_ROWS.saturating_sub(1));
    let top = credentials_scope_global_button_bounds(content_bounds).max_y() + 12.0;
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        top + safe_index as f32 * (CREDENTIALS_ROW_HEIGHT + CREDENTIALS_ROW_GAP),
        (content_bounds.size.width - CHAT_PAD * 2.0).max(240.0),
        CREDENTIALS_ROW_HEIGHT,
    )
}

pub fn credentials_visible_row_count(row_count: usize) -> usize {
    row_count.min(CREDENTIALS_MAX_ROWS)
}

pub fn job_inbox_accept_button_bounds(content_bounds: Bounds) -> Bounds {
    let button_width = content_bounds.size.width.clamp(144.0, 196.0);
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        content_bounds.origin.y + CHAT_PAD,
        button_width,
        JOB_INBOX_BUTTON_HEIGHT,
    )
}

pub fn job_inbox_reject_button_bounds(content_bounds: Bounds) -> Bounds {
    let accept = job_inbox_accept_button_bounds(content_bounds);
    Bounds::new(
        accept.max_x() + JOB_INBOX_BUTTON_GAP,
        accept.origin.y,
        accept.size.width,
        accept.size.height,
    )
}

pub fn job_inbox_row_bounds(content_bounds: Bounds, row_index: usize) -> Bounds {
    let safe_index = row_index.min(JOB_INBOX_MAX_ROWS.saturating_sub(1));
    let top = content_bounds.origin.y + CHAT_PAD + JOB_INBOX_BUTTON_HEIGHT + 12.0;
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        top + safe_index as f32 * (JOB_INBOX_ROW_HEIGHT + JOB_INBOX_ROW_GAP),
        (content_bounds.size.width - CHAT_PAD * 2.0).max(220.0),
        JOB_INBOX_ROW_HEIGHT,
    )
}

pub fn job_inbox_visible_row_count(request_count: usize) -> usize {
    request_count.min(JOB_INBOX_MAX_ROWS)
}

pub fn active_job_advance_button_bounds(content_bounds: Bounds) -> Bounds {
    let width = content_bounds.size.width.clamp(144.0, 196.0);
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        content_bounds.origin.y + CHAT_PAD,
        width,
        JOB_INBOX_BUTTON_HEIGHT,
    )
}

pub fn active_job_abort_button_bounds(content_bounds: Bounds) -> Bounds {
    let advance = active_job_advance_button_bounds(content_bounds);
    Bounds::new(
        advance.max_x() + JOB_INBOX_BUTTON_GAP,
        advance.origin.y,
        advance.size.width,
        advance.size.height,
    )
}

pub fn job_history_search_input_bounds(content_bounds: Bounds) -> Bounds {
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        content_bounds.origin.y + CHAT_PAD,
        (content_bounds.size.width * 0.42).clamp(220.0, 360.0),
        JOB_INBOX_BUTTON_HEIGHT,
    )
}

pub fn job_history_status_button_bounds(content_bounds: Bounds) -> Bounds {
    let search = job_history_search_input_bounds(content_bounds);
    Bounds::new(
        search.max_x() + JOB_INBOX_BUTTON_GAP,
        search.origin.y,
        132.0,
        JOB_INBOX_BUTTON_HEIGHT,
    )
}

pub fn job_history_time_button_bounds(content_bounds: Bounds) -> Bounds {
    let status = job_history_status_button_bounds(content_bounds);
    Bounds::new(
        status.max_x() + JOB_INBOX_BUTTON_GAP,
        status.origin.y,
        116.0,
        JOB_INBOX_BUTTON_HEIGHT,
    )
}

pub fn job_history_prev_page_button_bounds(content_bounds: Bounds) -> Bounds {
    let y = content_bounds.max_y() - CHAT_PAD - JOB_INBOX_BUTTON_HEIGHT;
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        y,
        64.0,
        JOB_INBOX_BUTTON_HEIGHT,
    )
}

pub fn job_history_next_page_button_bounds(content_bounds: Bounds) -> Bounds {
    let prev = job_history_prev_page_button_bounds(content_bounds);
    Bounds::new(
        prev.max_x() + JOB_INBOX_BUTTON_GAP,
        prev.origin.y,
        64.0,
        prev.size.height,
    )
}

pub fn agent_profile_publish_profile_button_bounds(content_bounds: Bounds) -> Bounds {
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        content_bounds.origin.y + CHAT_PAD,
        (content_bounds.size.width * 0.22).clamp(140.0, 220.0),
        JOB_INBOX_BUTTON_HEIGHT,
    )
}

pub fn agent_profile_publish_state_button_bounds(content_bounds: Bounds) -> Bounds {
    let publish = agent_profile_publish_profile_button_bounds(content_bounds);
    Bounds::new(
        publish.max_x() + JOB_INBOX_BUTTON_GAP,
        publish.origin.y,
        publish.size.width,
        publish.size.height,
    )
}

pub fn agent_profile_update_goals_button_bounds(content_bounds: Bounds) -> Bounds {
    let state = agent_profile_publish_state_button_bounds(content_bounds);
    Bounds::new(
        state.max_x() + JOB_INBOX_BUTTON_GAP,
        state.origin.y,
        state.size.width,
        state.size.height,
    )
}

pub fn agent_profile_create_goal_button_bounds(content_bounds: Bounds) -> Bounds {
    let publish = agent_profile_publish_profile_button_bounds(content_bounds);
    Bounds::new(
        publish.origin.x,
        publish.max_y() + JOB_INBOX_BUTTON_GAP,
        publish.size.width,
        publish.size.height,
    )
}

pub fn agent_profile_start_goal_button_bounds(content_bounds: Bounds) -> Bounds {
    let create = agent_profile_create_goal_button_bounds(content_bounds);
    Bounds::new(
        create.max_x() + JOB_INBOX_BUTTON_GAP,
        create.origin.y,
        create.size.width,
        create.size.height,
    )
}

pub fn agent_profile_abort_goal_button_bounds(content_bounds: Bounds) -> Bounds {
    let start = agent_profile_start_goal_button_bounds(content_bounds);
    Bounds::new(
        start.max_x() + JOB_INBOX_BUTTON_GAP,
        start.origin.y,
        start.size.width,
        start.size.height,
    )
}

pub fn agent_profile_receipt_button_bounds(content_bounds: Bounds) -> Bounds {
    let abort = agent_profile_abort_goal_button_bounds(content_bounds);
    Bounds::new(
        abort.max_x() + JOB_INBOX_BUTTON_GAP,
        abort.origin.y,
        abort.size.width,
        abort.size.height,
    )
}

pub fn agent_schedule_apply_button_bounds(content_bounds: Bounds) -> Bounds {
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        content_bounds.origin.y + CHAT_PAD,
        (content_bounds.size.width * 0.2).clamp(132.0, 210.0),
        JOB_INBOX_BUTTON_HEIGHT,
    )
}

pub fn agent_schedule_manual_tick_button_bounds(content_bounds: Bounds) -> Bounds {
    let apply = agent_schedule_apply_button_bounds(content_bounds);
    Bounds::new(
        apply.max_x() + JOB_INBOX_BUTTON_GAP,
        apply.origin.y,
        apply.size.width,
        apply.size.height,
    )
}

pub fn agent_schedule_inspect_button_bounds(content_bounds: Bounds) -> Bounds {
    let tick = agent_schedule_manual_tick_button_bounds(content_bounds);
    Bounds::new(
        tick.max_x() + JOB_INBOX_BUTTON_GAP,
        tick.origin.y,
        tick.size.width,
        tick.size.height,
    )
}

pub fn agent_schedule_toggle_os_scheduler_button_bounds(content_bounds: Bounds) -> Bounds {
    let inspect = agent_schedule_inspect_button_bounds(content_bounds);
    Bounds::new(
        inspect.max_x() + JOB_INBOX_BUTTON_GAP,
        inspect.origin.y,
        inspect.size.width,
        inspect.size.height,
    )
}

pub fn trajectory_open_session_button_bounds(content_bounds: Bounds) -> Bounds {
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        content_bounds.origin.y + CHAT_PAD,
        (content_bounds.size.width * 0.24).clamp(146.0, 240.0),
        JOB_INBOX_BUTTON_HEIGHT,
    )
}

pub fn trajectory_filter_button_bounds(content_bounds: Bounds) -> Bounds {
    let open = trajectory_open_session_button_bounds(content_bounds);
    Bounds::new(
        open.max_x() + JOB_INBOX_BUTTON_GAP,
        open.origin.y,
        open.size.width,
        open.size.height,
    )
}

pub fn trajectory_verify_button_bounds(content_bounds: Bounds) -> Bounds {
    let filter = trajectory_filter_button_bounds(content_bounds);
    Bounds::new(
        filter.max_x() + JOB_INBOX_BUTTON_GAP,
        filter.origin.y,
        filter.size.width,
        filter.size.height,
    )
}

pub fn cast_refresh_button_bounds(content_bounds: Bounds) -> Bounds {
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        content_bounds.origin.y + CHAT_PAD,
        (content_bounds.size.width * 0.2).clamp(120.0, 180.0),
        CAST_BUTTON_HEIGHT,
    )
}

pub fn cast_check_button_bounds(content_bounds: Bounds) -> Bounds {
    let refresh = cast_refresh_button_bounds(content_bounds);
    Bounds::new(
        refresh.max_x() + JOB_INBOX_BUTTON_GAP,
        refresh.origin.y,
        refresh.size.width,
        refresh.size.height,
    )
}

pub fn cast_prove_button_bounds(content_bounds: Bounds) -> Bounds {
    let check = cast_check_button_bounds(content_bounds);
    Bounds::new(
        check.max_x() + JOB_INBOX_BUTTON_GAP,
        check.origin.y,
        check.size.width,
        check.size.height,
    )
}

pub fn cast_sign_button_bounds(content_bounds: Bounds) -> Bounds {
    let refresh = cast_refresh_button_bounds(content_bounds);
    Bounds::new(
        refresh.origin.x,
        refresh.max_y() + JOB_INBOX_BUTTON_GAP,
        refresh.size.width,
        refresh.size.height,
    )
}

pub fn cast_inspect_button_bounds(content_bounds: Bounds) -> Bounds {
    let sign = cast_sign_button_bounds(content_bounds);
    Bounds::new(
        sign.max_x() + JOB_INBOX_BUTTON_GAP,
        sign.origin.y,
        sign.size.width,
        sign.size.height,
    )
}

pub fn cast_toggle_broadcast_button_bounds(content_bounds: Bounds) -> Bounds {
    let inspect = cast_inspect_button_bounds(content_bounds);
    Bounds::new(
        inspect.max_x() + JOB_INBOX_BUTTON_GAP,
        inspect.origin.y,
        inspect.size.width,
        inspect.size.height,
    )
}

pub fn cast_loop_once_button_bounds(content_bounds: Bounds) -> Bounds {
    let sign = cast_sign_button_bounds(content_bounds);
    Bounds::new(
        sign.origin.x,
        sign.max_y() + JOB_INBOX_BUTTON_GAP,
        sign.size.width,
        sign.size.height,
    )
}

pub fn cast_toggle_loop_button_bounds(content_bounds: Bounds) -> Bounds {
    let loop_once = cast_loop_once_button_bounds(content_bounds);
    Bounds::new(
        loop_once.max_x() + JOB_INBOX_BUTTON_GAP,
        loop_once.origin.y,
        loop_once.size.width,
        loop_once.size.height,
    )
}

pub fn skill_registry_discover_button_bounds(content_bounds: Bounds) -> Bounds {
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        content_bounds.origin.y + CHAT_PAD,
        (content_bounds.size.width * 0.22).clamp(140.0, 210.0),
        JOB_INBOX_BUTTON_HEIGHT,
    )
}

pub fn skill_registry_inspect_button_bounds(content_bounds: Bounds) -> Bounds {
    let discover = skill_registry_discover_button_bounds(content_bounds);
    Bounds::new(
        discover.max_x() + JOB_INBOX_BUTTON_GAP,
        discover.origin.y,
        discover.size.width,
        discover.size.height,
    )
}

pub fn skill_registry_install_button_bounds(content_bounds: Bounds) -> Bounds {
    let inspect = skill_registry_inspect_button_bounds(content_bounds);
    Bounds::new(
        inspect.max_x() + JOB_INBOX_BUTTON_GAP,
        inspect.origin.y,
        inspect.size.width,
        inspect.size.height,
    )
}

pub fn skill_registry_row_bounds(content_bounds: Bounds, index: usize) -> Bounds {
    let discover = skill_registry_discover_button_bounds(content_bounds);
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        discover.max_y()
            + CHAT_PAD
            + index as f32 * (SKILL_REGISTRY_ROW_HEIGHT + SKILL_REGISTRY_ROW_GAP),
        (content_bounds.size.width - CHAT_PAD * 2.0).max(180.0),
        SKILL_REGISTRY_ROW_HEIGHT,
    )
}

pub fn skill_registry_visible_row_count(total_rows: usize) -> usize {
    total_rows.min(SKILL_REGISTRY_MAX_ROWS)
}

pub fn skill_trust_refresh_button_bounds(content_bounds: Bounds) -> Bounds {
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        content_bounds.origin.y + CHAT_PAD,
        (content_bounds.size.width * 0.18).clamp(120.0, 180.0),
        JOB_INBOX_BUTTON_HEIGHT,
    )
}

pub fn skill_trust_attestations_button_bounds(content_bounds: Bounds) -> Bounds {
    let refresh = skill_trust_refresh_button_bounds(content_bounds);
    Bounds::new(
        refresh.max_x() + JOB_INBOX_BUTTON_GAP,
        refresh.origin.y,
        refresh.size.width,
        refresh.size.height,
    )
}

pub fn skill_trust_kill_switch_button_bounds(content_bounds: Bounds) -> Bounds {
    let attest = skill_trust_attestations_button_bounds(content_bounds);
    Bounds::new(
        attest.max_x() + JOB_INBOX_BUTTON_GAP,
        attest.origin.y,
        attest.size.width,
        attest.size.height,
    )
}

pub fn skill_trust_revoke_button_bounds(content_bounds: Bounds) -> Bounds {
    let kill_switch = skill_trust_kill_switch_button_bounds(content_bounds);
    Bounds::new(
        kill_switch.max_x() + JOB_INBOX_BUTTON_GAP,
        kill_switch.origin.y,
        kill_switch.size.width,
        kill_switch.size.height,
    )
}

pub fn credit_desk_intent_button_bounds(content_bounds: Bounds) -> Bounds {
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        content_bounds.origin.y + CHAT_PAD,
        (content_bounds.size.width * 0.18).clamp(120.0, 180.0),
        JOB_INBOX_BUTTON_HEIGHT,
    )
}

pub fn credit_desk_offer_button_bounds(content_bounds: Bounds) -> Bounds {
    let intent = credit_desk_intent_button_bounds(content_bounds);
    Bounds::new(
        intent.max_x() + JOB_INBOX_BUTTON_GAP,
        intent.origin.y,
        intent.size.width,
        intent.size.height,
    )
}

pub fn credit_desk_envelope_button_bounds(content_bounds: Bounds) -> Bounds {
    let offer = credit_desk_offer_button_bounds(content_bounds);
    Bounds::new(
        offer.max_x() + JOB_INBOX_BUTTON_GAP,
        offer.origin.y,
        offer.size.width,
        offer.size.height,
    )
}

pub fn credit_desk_spend_button_bounds(content_bounds: Bounds) -> Bounds {
    let envelope = credit_desk_envelope_button_bounds(content_bounds);
    Bounds::new(
        envelope.max_x() + JOB_INBOX_BUTTON_GAP,
        envelope.origin.y,
        envelope.size.width,
        envelope.size.height,
    )
}

pub fn credit_settlement_verify_button_bounds(content_bounds: Bounds) -> Bounds {
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        content_bounds.origin.y + CHAT_PAD,
        (content_bounds.size.width * 0.22).clamp(140.0, 220.0),
        JOB_INBOX_BUTTON_HEIGHT,
    )
}

pub fn credit_settlement_default_button_bounds(content_bounds: Bounds) -> Bounds {
    let verify = credit_settlement_verify_button_bounds(content_bounds);
    Bounds::new(
        verify.max_x() + JOB_INBOX_BUTTON_GAP,
        verify.origin.y,
        verify.size.width,
        verify.size.height,
    )
}

pub fn credit_settlement_reputation_button_bounds(content_bounds: Bounds) -> Bounds {
    let default_notice = credit_settlement_default_button_bounds(content_bounds);
    Bounds::new(
        default_notice.max_x() + JOB_INBOX_BUTTON_GAP,
        default_notice.origin.y,
        default_notice.size.width,
        default_notice.size.height,
    )
}

struct CadDemoTopRowLayout {
    cycle: Bounds,
    jaw: Bounds,
    reset: Bounds,
    hidden_line: Bounds,
    reset_camera: Bounds,
    projection: Bounds,
    viewport_layout: Bounds,
}

fn cad_demo_top_row_layout(content_bounds: Bounds) -> CadDemoTopRowLayout {
    let min_x = content_bounds.origin.x + CHAT_PAD;
    let max_x = content_bounds.max_x() - CHAT_PAD;
    let top = content_bounds.origin.y + CHAT_PAD;
    let gap = JOB_INBOX_BUTTON_GAP;
    let button_height = JOB_INBOX_BUTTON_HEIGHT;

    let desired = [
        (content_bounds.size.width * 0.24).clamp(150.0, 220.0),
        (content_bounds.size.width * 0.16).clamp(102.0, 170.0),
        (content_bounds.size.width * 0.18).clamp(120.0, 190.0),
        (content_bounds.size.width * 0.22).clamp(140.0, 220.0),
        (content_bounds.size.width * 0.18).clamp(120.0, 180.0),
        (content_bounds.size.width * 0.2).clamp(130.0, 210.0),
        (content_bounds.size.width * 0.16).clamp(98.0, 170.0),
    ];
    let min_widths = [120.0, 90.0, 100.0, 100.0, 90.0, 90.0, 80.0];

    let gaps_total = gap * (desired.len().saturating_sub(1) as f32);
    let available = (max_x - min_x - gaps_total).max(0.0);
    let min_sum: f32 = min_widths.iter().sum();
    let mut widths = [0.0f32; 7];

    if available > 0.0 {
        if available < min_sum {
            let scale = available / min_sum;
            for (slot, min_width) in widths.iter_mut().zip(min_widths.into_iter()) {
                *slot = min_width * scale;
            }
        } else {
            widths = desired;
            let mut overflow = widths.iter().sum::<f32>() - available;
            if overflow > 0.0 {
                for index in (0..widths.len()).rev() {
                    let reducible = (widths[index] - min_widths[index]).max(0.0);
                    let cut = reducible.min(overflow);
                    widths[index] -= cut;
                    overflow -= cut;
                    if overflow <= 0.0 {
                        break;
                    }
                }
            }
        }
    }

    let mut cursor_x = min_x;
    let mut place = |width: f32| {
        let origin_x = cursor_x.min(max_x);
        let clamped_width = width.min((max_x - origin_x).max(0.0));
        let bounds = Bounds::new(origin_x, top, clamped_width, button_height);
        cursor_x = origin_x + clamped_width + gap;
        bounds
    };

    let cycle = place(widths[0]);
    let jaw = place(widths[1]);
    let reset = place(widths[2]);
    let hidden_line = place(widths[3]);
    let reset_camera = place(widths[4]);
    let projection = place(widths[5]);
    let viewport_layout = place(widths[6]);

    CadDemoTopRowLayout {
        cycle,
        jaw,
        reset,
        hidden_line,
        reset_camera,
        projection,
        viewport_layout,
    }
}

pub fn cad_demo_cycle_variant_button_bounds(content_bounds: Bounds) -> Bounds {
    cad_demo_top_row_layout(content_bounds).cycle
}

pub fn cad_demo_gripper_jaw_button_bounds(content_bounds: Bounds) -> Bounds {
    cad_demo_top_row_layout(content_bounds).jaw
}

pub fn cad_demo_reset_button_bounds(content_bounds: Bounds) -> Bounds {
    cad_demo_top_row_layout(content_bounds).reset
}

pub fn cad_demo_hidden_line_mode_button_bounds(content_bounds: Bounds) -> Bounds {
    cad_demo_top_row_layout(content_bounds).hidden_line
}

pub fn cad_demo_reset_camera_button_bounds(content_bounds: Bounds) -> Bounds {
    cad_demo_top_row_layout(content_bounds).reset_camera
}

pub fn cad_demo_projection_mode_button_bounds(content_bounds: Bounds) -> Bounds {
    cad_demo_top_row_layout(content_bounds).projection
}

pub fn cad_demo_viewport_layout_button_bounds(content_bounds: Bounds) -> Bounds {
    cad_demo_top_row_layout(content_bounds).viewport_layout
}

fn cad_demo_drawing_toolbar_top(content_bounds: Bounds) -> f32 {
    cad_demo_projection_mode_button_bounds(content_bounds)
        .max_y()
        .max(cad_demo_viewport_layout_button_bounds(content_bounds).max_y())
        .max(cad_demo_reset_camera_button_bounds(content_bounds).max_y())
        + 6.0
}

pub fn cad_demo_drawing_mode_button_bounds(content_bounds: Bounds) -> Bounds {
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        cad_demo_drawing_toolbar_top(content_bounds),
        (content_bounds.size.width * 0.14).clamp(90.0, 150.0),
        20.0,
    )
}

pub fn cad_demo_drawing_direction_button_bounds(content_bounds: Bounds) -> Bounds {
    let mode = cad_demo_drawing_mode_button_bounds(content_bounds);
    let min_x = content_bounds.origin.x + CHAT_PAD;
    let max_x = content_bounds.max_x() - CHAT_PAD;
    let origin_x = (mode.max_x() + JOB_INBOX_BUTTON_GAP).max(min_x);
    let desired_width = (content_bounds.size.width * 0.18).clamp(110.0, 190.0);
    let width = desired_width.min((max_x - origin_x).max(50.0));
    Bounds::new(origin_x, mode.origin.y, width, mode.size.height)
}

pub fn cad_demo_drawing_hidden_lines_button_bounds(content_bounds: Bounds) -> Bounds {
    let direction = cad_demo_drawing_direction_button_bounds(content_bounds);
    let min_x = content_bounds.origin.x + CHAT_PAD;
    let max_x = content_bounds.max_x() - CHAT_PAD;
    let origin_x = (direction.max_x() + JOB_INBOX_BUTTON_GAP).max(min_x);
    let desired_width = (content_bounds.size.width * 0.14).clamp(96.0, 160.0);
    let width = desired_width.min((max_x - origin_x).max(50.0));
    Bounds::new(origin_x, direction.origin.y, width, direction.size.height)
}

pub fn cad_demo_drawing_dimensions_button_bounds(content_bounds: Bounds) -> Bounds {
    let hidden = cad_demo_drawing_hidden_lines_button_bounds(content_bounds);
    let min_x = content_bounds.origin.x + CHAT_PAD;
    let max_x = content_bounds.max_x() - CHAT_PAD;
    let origin_x = (hidden.max_x() + JOB_INBOX_BUTTON_GAP).max(min_x);
    let desired_width = (content_bounds.size.width * 0.14).clamp(96.0, 160.0);
    let width = desired_width.min((max_x - origin_x).max(50.0));
    Bounds::new(origin_x, hidden.origin.y, width, hidden.size.height)
}

pub fn cad_demo_drawing_reset_view_button_bounds(content_bounds: Bounds) -> Bounds {
    let dimensions = cad_demo_drawing_dimensions_button_bounds(content_bounds);
    let min_x = content_bounds.origin.x + CHAT_PAD;
    let max_x = content_bounds.max_x() - CHAT_PAD;
    let origin_x = (dimensions.max_x() + JOB_INBOX_BUTTON_GAP).max(min_x);
    let desired_width = (content_bounds.size.width * 0.14).clamp(96.0, 160.0);
    let width = desired_width.min((max_x - origin_x).max(50.0));
    Bounds::new(origin_x, dimensions.origin.y, width, dimensions.size.height)
}

pub fn cad_demo_drawing_add_detail_button_bounds(content_bounds: Bounds) -> Bounds {
    let reset_view = cad_demo_drawing_reset_view_button_bounds(content_bounds);
    let min_x = content_bounds.origin.x + CHAT_PAD;
    let max_x = content_bounds.max_x() - CHAT_PAD;
    let origin_x = (reset_view.max_x() + JOB_INBOX_BUTTON_GAP).max(min_x);
    let desired_width = (content_bounds.size.width * 0.14).clamp(96.0, 160.0);
    let width = desired_width.min((max_x - origin_x).max(50.0));
    Bounds::new(origin_x, reset_view.origin.y, width, reset_view.size.height)
}

pub fn cad_demo_drawing_clear_details_button_bounds(content_bounds: Bounds) -> Bounds {
    let add_detail = cad_demo_drawing_add_detail_button_bounds(content_bounds);
    let min_x = content_bounds.origin.x + CHAT_PAD;
    let max_x = content_bounds.max_x() - CHAT_PAD;
    let origin_x = (add_detail.max_x() + JOB_INBOX_BUTTON_GAP).max(min_x);
    let desired_width = (content_bounds.size.width * 0.14).clamp(96.0, 160.0);
    let width = desired_width.min((max_x - origin_x).max(50.0));
    Bounds::new(origin_x, add_detail.origin.y, width, add_detail.size.height)
}

fn cad_demo_drawing_controls_bottom(content_bounds: Bounds) -> f32 {
    cad_demo_drawing_mode_button_bounds(content_bounds)
        .max_y()
        .max(cad_demo_drawing_direction_button_bounds(content_bounds).max_y())
        .max(cad_demo_drawing_hidden_lines_button_bounds(content_bounds).max_y())
        .max(cad_demo_drawing_dimensions_button_bounds(content_bounds).max_y())
        .max(cad_demo_drawing_reset_view_button_bounds(content_bounds).max_y())
        .max(cad_demo_drawing_add_detail_button_bounds(content_bounds).max_y())
        .max(cad_demo_drawing_clear_details_button_bounds(content_bounds).max_y())
}

pub fn cad_demo_snap_grid_button_bounds(content_bounds: Bounds) -> Bounds {
    let top_row_bottom = cad_demo_drawing_controls_bottom(content_bounds);
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        top_row_bottom + 6.0,
        (content_bounds.size.width * 0.16).clamp(92.0, 160.0),
        20.0,
    )
}

pub fn cad_demo_snap_origin_button_bounds(content_bounds: Bounds) -> Bounds {
    let grid = cad_demo_snap_grid_button_bounds(content_bounds);
    Bounds::new(
        grid.max_x() + JOB_INBOX_BUTTON_GAP,
        grid.origin.y,
        grid.size.width,
        grid.size.height,
    )
}

pub fn cad_demo_snap_endpoint_button_bounds(content_bounds: Bounds) -> Bounds {
    let origin = cad_demo_snap_origin_button_bounds(content_bounds);
    Bounds::new(
        origin.max_x() + JOB_INBOX_BUTTON_GAP,
        origin.origin.y,
        grid_like_snap_width(content_bounds),
        origin.size.height,
    )
}

pub fn cad_demo_snap_midpoint_button_bounds(content_bounds: Bounds) -> Bounds {
    let endpoint = cad_demo_snap_endpoint_button_bounds(content_bounds);
    let min_x = content_bounds.origin.x + CHAT_PAD;
    let max_x = content_bounds.max_x() - CHAT_PAD;
    let origin_x = (endpoint.max_x() + JOB_INBOX_BUTTON_GAP).max(min_x);
    let width = grid_like_snap_width(content_bounds).min((max_x - origin_x).max(44.0));
    Bounds::new(origin_x, endpoint.origin.y, width, endpoint.size.height)
}

pub fn cad_demo_hotkey_profile_button_bounds(content_bounds: Bounds) -> Bounds {
    let midpoint = cad_demo_snap_midpoint_button_bounds(content_bounds);
    let min_x = content_bounds.origin.x + CHAT_PAD;
    let max_x = content_bounds.max_x() - CHAT_PAD;
    let origin_x = (midpoint.max_x() + JOB_INBOX_BUTTON_GAP).max(min_x);
    let desired_width = (content_bounds.size.width * 0.20).clamp(120.0, 210.0);
    let width = desired_width.min((max_x - origin_x).max(44.0));
    Bounds::new(origin_x, midpoint.origin.y, width, midpoint.size.height)
}

pub fn cad_demo_section_plane_button_bounds(content_bounds: Bounds) -> Bounds {
    let top = cad_demo_snap_grid_button_bounds(content_bounds).max_y() + 6.0;
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        top,
        (content_bounds.size.width * 0.22).clamp(130.0, 220.0),
        20.0,
    )
}

pub fn cad_demo_section_offset_button_bounds(content_bounds: Bounds) -> Bounds {
    let section_plane = cad_demo_section_plane_button_bounds(content_bounds);
    let min_x = content_bounds.origin.x + CHAT_PAD;
    let max_x = content_bounds.max_x() - CHAT_PAD;
    let origin_x = (section_plane.max_x() + JOB_INBOX_BUTTON_GAP).max(min_x);
    let desired_width = (content_bounds.size.width * 0.2).clamp(120.0, 210.0);
    let width = desired_width.min((max_x - origin_x).max(44.0));
    Bounds::new(
        origin_x,
        section_plane.origin.y,
        width,
        section_plane.size.height,
    )
}

pub fn cad_demo_material_button_bounds(content_bounds: Bounds) -> Bounds {
    let section_offset = cad_demo_section_offset_button_bounds(content_bounds);
    let min_x = content_bounds.origin.x + CHAT_PAD;
    let max_x = content_bounds.max_x() - CHAT_PAD;
    let origin_x = (section_offset.max_x() + JOB_INBOX_BUTTON_GAP).max(min_x);
    let desired_width = (content_bounds.size.width * 0.24).clamp(140.0, 240.0);
    let width = desired_width.min((max_x - origin_x).max(44.0));
    Bounds::new(
        origin_x,
        section_offset.origin.y,
        width,
        section_offset.size.height,
    )
}

pub fn cad_demo_sensor_mode_button_bounds(content_bounds: Bounds) -> Bounds {
    let material = cad_demo_material_button_bounds(content_bounds);
    let min_x = content_bounds.origin.x + CHAT_PAD;
    let max_x = content_bounds.max_x() - CHAT_PAD;
    let origin_x = (material.max_x() + JOB_INBOX_BUTTON_GAP).max(min_x);
    let desired_width = (content_bounds.size.width * 0.16).clamp(110.0, 180.0);
    let width = desired_width.min((max_x - origin_x).max(44.0));
    Bounds::new(origin_x, material.origin.y, width, material.size.height)
}

fn grid_like_snap_width(content_bounds: Bounds) -> f32 {
    (content_bounds.size.width * 0.16).clamp(92.0, 160.0)
}

fn cad_demo_controls_bottom(content_bounds: Bounds) -> f32 {
    cad_demo_cycle_variant_button_bounds(content_bounds)
        .max_y()
        .max(cad_demo_gripper_jaw_button_bounds(content_bounds).max_y())
        .max(cad_demo_reset_button_bounds(content_bounds).max_y())
        .max(cad_demo_hidden_line_mode_button_bounds(content_bounds).max_y())
        .max(cad_demo_reset_camera_button_bounds(content_bounds).max_y())
        .max(cad_demo_projection_mode_button_bounds(content_bounds).max_y())
        .max(cad_demo_viewport_layout_button_bounds(content_bounds).max_y())
        .max(cad_demo_drawing_mode_button_bounds(content_bounds).max_y())
        .max(cad_demo_drawing_direction_button_bounds(content_bounds).max_y())
        .max(cad_demo_drawing_hidden_lines_button_bounds(content_bounds).max_y())
        .max(cad_demo_drawing_dimensions_button_bounds(content_bounds).max_y())
        .max(cad_demo_drawing_reset_view_button_bounds(content_bounds).max_y())
        .max(cad_demo_drawing_add_detail_button_bounds(content_bounds).max_y())
        .max(cad_demo_drawing_clear_details_button_bounds(content_bounds).max_y())
        .max(cad_demo_snap_grid_button_bounds(content_bounds).max_y())
        .max(cad_demo_snap_origin_button_bounds(content_bounds).max_y())
        .max(cad_demo_snap_endpoint_button_bounds(content_bounds).max_y())
        .max(cad_demo_snap_midpoint_button_bounds(content_bounds).max_y())
        .max(cad_demo_hotkey_profile_button_bounds(content_bounds).max_y())
        .max(cad_demo_section_plane_button_bounds(content_bounds).max_y())
        .max(cad_demo_section_offset_button_bounds(content_bounds).max_y())
        .max(cad_demo_material_button_bounds(content_bounds).max_y())
        .max(cad_demo_sensor_mode_button_bounds(content_bounds).max_y())
}

pub fn cad_demo_view_cube_bounds(content_bounds: Bounds) -> Bounds {
    let buttons_bottom = cad_demo_controls_bottom(content_bounds);
    let warning_top = cad_demo_warning_panel_bounds(content_bounds).origin.y;
    let min_x = content_bounds.origin.x + CHAT_PAD;
    let min_y = content_bounds.origin.y + CHAT_PAD;
    let size = 88.0f32.min((content_bounds.size.width - CHAT_PAD * 2.0).max(40.0));
    let max_x = (content_bounds.max_x() - CHAT_PAD - size).max(min_x);
    let max_y = (warning_top - 8.0 - size)
        .min(content_bounds.max_y() - CHAT_PAD - size)
        .max(min_y);
    let origin_x = max_x.max(min_x);
    let origin_y = (buttons_bottom + 10.0).min(max_y).max(min_y);
    Bounds::new(origin_x, origin_y, size, size)
}

pub fn cad_demo_view_snap_top_button_bounds(content_bounds: Bounds) -> Bounds {
    cad_demo_view_cube_cell_bounds(content_bounds, 0, 0)
}

pub fn cad_demo_view_snap_front_button_bounds(content_bounds: Bounds) -> Bounds {
    cad_demo_view_cube_cell_bounds(content_bounds, 1, 0)
}

pub fn cad_demo_view_snap_right_button_bounds(content_bounds: Bounds) -> Bounds {
    cad_demo_view_cube_cell_bounds(content_bounds, 0, 1)
}

pub fn cad_demo_view_snap_iso_button_bounds(content_bounds: Bounds) -> Bounds {
    cad_demo_view_cube_cell_bounds(content_bounds, 1, 1)
}

fn cad_demo_view_cube_cell_bounds(content_bounds: Bounds, col: usize, row: usize) -> Bounds {
    let cube = cad_demo_view_cube_bounds(content_bounds);
    let inner_pad = 6.0;
    let gap = 4.0;
    let cell_width = ((cube.size.width - inner_pad * 2.0 - gap) * 0.5).max(12.0);
    let cell_height = ((cube.size.height - inner_pad * 2.0 - gap) * 0.5).max(10.0);
    Bounds::new(
        cube.origin.x + inner_pad + col as f32 * (cell_width + gap),
        cube.origin.y + inner_pad + row as f32 * (cell_height + gap),
        cell_width,
        cell_height,
    )
}

pub fn cad_demo_context_menu_bounds(
    content_bounds: Bounds,
    anchor: Point,
    item_count: usize,
) -> Bounds {
    let width = (content_bounds.size.width * 0.24).clamp(150.0, 220.0);
    let row_count = item_count.max(1);
    let height = row_count as f32 * CAD_CONTEXT_MENU_ROW_HEIGHT + 22.0;
    let min_x = content_bounds.origin.x + CHAT_PAD;
    let min_y = content_bounds.origin.y + CHAT_PAD;
    let max_x = (content_bounds.max_x() - CHAT_PAD - width).max(min_x);
    let max_y = (content_bounds.max_y() - CHAT_PAD - height).max(min_y);
    let x = anchor.x.clamp(min_x, max_x);
    let y = anchor.y.clamp(min_y, max_y);
    Bounds::new(x, y, width, height)
}

pub fn cad_demo_context_menu_row_bounds(menu_bounds: Bounds, index: usize) -> Bounds {
    Bounds::new(
        menu_bounds.origin.x + 6.0,
        menu_bounds.origin.y + 18.0 + index as f32 * CAD_CONTEXT_MENU_ROW_HEIGHT,
        menu_bounds.size.width - 12.0,
        (CAD_CONTEXT_MENU_ROW_HEIGHT - 2.0).max(12.0),
    )
}

pub fn cad_demo_warning_panel_bounds(content_bounds: Bounds) -> Bounds {
    let width = (content_bounds.size.width * 0.42).clamp(200.0, 300.0);
    let height = (content_bounds.size.height * 0.36).clamp(120.0, 220.0);
    let origin_x =
        (content_bounds.max_x() - width - CHAT_PAD).max(content_bounds.origin.x + CHAT_PAD);
    let origin_y =
        (content_bounds.max_y() - height - CHAT_PAD).max(content_bounds.origin.y + CHAT_PAD);
    Bounds::new(origin_x, origin_y, width, height)
}

pub fn cad_demo_warning_filter_severity_button_bounds(content_bounds: Bounds) -> Bounds {
    let panel = cad_demo_warning_panel_bounds(content_bounds);
    let inner_width = (panel.size.width - CHAT_PAD * 1.5).max(40.0);
    Bounds::new(
        panel.origin.x + CHAT_PAD * 0.5,
        panel.origin.y + CHAT_PAD * 0.5,
        inner_width * 0.5 - CHAT_PAD * 0.25,
        20.0,
    )
}

pub fn cad_demo_warning_filter_code_button_bounds(content_bounds: Bounds) -> Bounds {
    let severity = cad_demo_warning_filter_severity_button_bounds(content_bounds);
    Bounds::new(
        severity.max_x() + CHAT_PAD * 0.5,
        severity.origin.y,
        severity.size.width,
        severity.size.height,
    )
}

pub fn cad_demo_warning_row_bounds(content_bounds: Bounds, index: usize) -> Bounds {
    let panel = cad_demo_warning_panel_bounds(content_bounds);
    let top = cad_demo_warning_filter_code_button_bounds(content_bounds).max_y() + 8.0;
    let max_origin_y = (panel.max_y() - 16.0).max(top);
    let origin_y = (top + index as f32 * 18.0).min(max_origin_y);
    Bounds::new(
        panel.origin.x + CHAT_PAD * 0.5,
        origin_y,
        (panel.size.width - CHAT_PAD).max(30.0),
        16.0,
    )
}

pub fn cad_demo_warning_marker_bounds(content_bounds: Bounds, index: usize) -> Bounds {
    let panel = cad_demo_warning_panel_bounds(content_bounds);
    let buttons_bottom = cad_demo_controls_bottom(content_bounds);
    let viewport_top = (buttons_bottom + 12.0).min(content_bounds.max_y());
    let viewport_bottom = (panel.origin.y - 8.0).max(viewport_top);
    let viewport_height = (viewport_bottom - viewport_top).max(1.0);
    let row = index / 6;
    let col = index % 6;
    let marker_size = 8.0;
    let marker_x = (content_bounds.origin.x + CHAT_PAD + 14.0 + col as f32 * 20.0)
        .min(content_bounds.max_x() - marker_size - CHAT_PAD);
    let marker_y =
        (viewport_top + 14.0 + row as f32 * 20.0).min(viewport_top + viewport_height - marker_size);
    Bounds::new(marker_x, marker_y, marker_size, marker_size)
}

pub fn cad_demo_dimension_panel_bounds(content_bounds: Bounds) -> Bounds {
    let warning_panel = cad_demo_warning_panel_bounds(content_bounds);
    let width = (content_bounds.size.width * 0.24).clamp(150.0, 210.0);
    let height = 98.0;
    let origin_x = content_bounds.origin.x + CHAT_PAD;
    let max_origin_y = (warning_panel.origin.y - height - 10.0).max(content_bounds.origin.y + 10.0);
    let base_top =
        (cad_demo_controls_bottom(content_bounds) + 10.0).max(content_bounds.origin.y + 10.0);
    let origin_y = base_top.min(max_origin_y);
    Bounds::new(
        origin_x,
        origin_y,
        width.min((warning_panel.origin.x - origin_x - 8.0).max(120.0)),
        height.max(72.0),
    )
}

pub fn cad_demo_dimension_row_bounds(content_bounds: Bounds, index: usize) -> Bounds {
    let panel = cad_demo_dimension_panel_bounds(content_bounds);
    let safe_index = index.min(7);
    let top = panel.origin.y + 22.0;
    let max_origin_y = (panel.max_y() - 16.0).max(top);
    let origin_y = (top + safe_index as f32 * 18.0).min(max_origin_y);
    Bounds::new(
        panel.origin.x + 6.0,
        origin_y,
        (panel.size.width - 12.0).max(24.0),
        16.0,
    )
}

pub fn cad_demo_timeline_panel_bounds(content_bounds: Bounds) -> Bounds {
    let warning_panel = cad_demo_warning_panel_bounds(content_bounds);
    let width = ((warning_panel.origin.x - content_bounds.origin.x) - CHAT_PAD * 2.0).max(180.0);
    let height = warning_panel.size.height;
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        warning_panel.origin.y,
        width,
        height,
    )
}

pub fn cad_demo_timeline_row_bounds(content_bounds: Bounds, index: usize) -> Bounds {
    let panel = cad_demo_timeline_panel_bounds(content_bounds);
    let top = panel.origin.y + CHAT_PAD * 0.5;
    let max_origin_y = (panel.max_y() - 16.0).max(top);
    let origin_y = (top + index as f32 * 18.0).min(max_origin_y);
    Bounds::new(
        panel.origin.x + CHAT_PAD * 0.5,
        origin_y,
        (panel.size.width - CHAT_PAD).max(24.0),
        16.0,
    )
}

pub fn nostr_regenerate_button_bounds(content_bounds: Bounds) -> Bounds {
    let (regenerate_bounds, _, _) = nostr_button_bounds(content_bounds);
    regenerate_bounds
}

pub fn nostr_reveal_button_bounds(content_bounds: Bounds) -> Bounds {
    let (_, reveal_bounds, _) = nostr_button_bounds(content_bounds);
    reveal_bounds
}

pub fn nostr_copy_secret_button_bounds(content_bounds: Bounds) -> Bounds {
    let (_, _, copy_bounds) = nostr_button_bounds(content_bounds);
    copy_bounds
}

pub(crate) fn topmost_pane_hit_action_in_order(
    state: &RenderState,
    point: Point,
    pane_order: &[usize],
) -> Option<(u64, PaneHitAction)> {
    for pane_idx in pane_order {
        let pane_idx = *pane_idx;
        let pane = &state.panes[pane_idx];
        if let Some(action) = pane_hit_action_for_pane(state, pane, point) {
            return Some((pane.id, action));
        }
    }

    None
}

fn pane_hit_action_for_pane(
    state: &RenderState,
    pane: &DesktopPane,
    point: Point,
) -> Option<PaneHitAction> {
    if !pane.bounds.contains(point) {
        return None;
    }

    let content_bounds = pane_content_bounds(pane.bounds);
    match pane.kind {
        PaneKind::NostrIdentity => {
            if nostr_regenerate_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::NostrRegenerate);
            }
            if nostr_reveal_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::NostrReveal);
            }
            if nostr_copy_secret_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::NostrCopySecret);
            }
            None
        }
        PaneKind::ProjectOps => None,
        PaneKind::AutopilotChat => {
            let browse_mode = state.autopilot_chat.chat_browse_mode();
            if browse_mode == crate::app_state::ChatBrowseMode::Autopilot {
                if chat_new_thread_button_bounds(content_bounds).contains(point) {
                    return Some(PaneHitAction::ChatNewThread);
                }
                if chat_refresh_threads_button_bounds(content_bounds).contains(point) {
                    return Some(PaneHitAction::ChatRefreshThreads);
                }
            }
            if state.autopilot_chat.chat_has_browseable_content() {
                let workspace_count = chat_visible_workspace_row_count(
                    content_bounds,
                    state.autopilot_chat.chat_workspace_entries().len(),
                );
                for index in 0..workspace_count {
                    if chat_workspace_row_bounds(content_bounds, index).contains(point) {
                        return Some(PaneHitAction::ChatSelectWorkspace(index));
                    }
                }
            }
            let managed_channel_rows = (browse_mode == crate::app_state::ChatBrowseMode::Managed)
                .then(|| state.autopilot_chat.active_managed_chat_channel_rail_rows());
            let direct_room_count =
                if browse_mode == crate::app_state::ChatBrowseMode::DirectMessages {
                    state.autopilot_chat.active_direct_message_rooms().len()
                } else {
                    0
                };
            let channel_count = if let Some(rows) = managed_channel_rows.as_ref() {
                rows.len()
            } else if browse_mode == crate::app_state::ChatBrowseMode::DirectMessages {
                direct_room_count
            } else {
                1 + state
                    .autopilot_chat
                    .threads
                    .len()
                    .min(CHAT_AUTOPILOT_THREAD_PREVIEW_LIMIT)
            };
            let visible_rows = chat_visible_thread_row_count(content_bounds, channel_count);
            for index in 0..visible_rows {
                if chat_thread_row_bounds(content_bounds, index).contains(point) {
                    if let Some(rows) = managed_channel_rows.as_ref() {
                        return match rows.get(index) {
                            Some(crate::app_state::ManagedChatChannelRailRow::Category {
                                ..
                            }) => Some(PaneHitAction::ChatToggleCategory(index)),
                            Some(crate::app_state::ManagedChatChannelRailRow::Channel {
                                ..
                            }) => Some(PaneHitAction::ChatSelectThread(index)),
                            None => None,
                        };
                    }
                    return Some(PaneHitAction::ChatSelectThread(index));
                }
            }
            let can_send = match browse_mode {
                crate::app_state::ChatBrowseMode::Managed => state
                    .autopilot_chat
                    .managed_chat_can_send(state.chat_inputs.composer.get_value()),
                crate::app_state::ChatBrowseMode::DirectMessages => state
                    .autopilot_chat
                    .direct_message_can_send(state.chat_inputs.composer.get_value()),
                crate::app_state::ChatBrowseMode::Autopilot => {
                    !state.chat_inputs.composer.get_value().trim().is_empty()
                }
            };
            if can_send && chat_send_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::ChatSend);
            }
            None
        }
        PaneKind::Calculator => None,
        PaneKind::GoOnline => {
            if go_online_toggle_button_bounds(content_bounds).contains(point) {
                if matches!(
                    state.provider_runtime.mode,
                    crate::app_state::ProviderMode::Offline
                        | crate::app_state::ProviderMode::Degraded
                ) && !state.mission_control_go_online_enabled()
                {
                    return None;
                }
                Some(PaneHitAction::GoOnlineToggle)
            } else if mission_control_amount_toggle_button_bounds(content_bounds).contains(point) {
                Some(PaneHitAction::MissionControl(
                    MissionControlPaneAction::ToggleAmountDisplay,
                ))
            } else if mission_control_download_model_button_bounds(content_bounds).contains(point) {
                Some(PaneHitAction::MissionControl(
                    MissionControlPaneAction::WarmLocalModel,
                ))
            } else if mission_control_documentation_button_bounds(content_bounds).contains(point) {
                Some(PaneHitAction::MissionControl(
                    MissionControlPaneAction::OpenDocumentation,
                ))
            } else {
                None
            }
        }
        PaneKind::CodexAccount => {
            if codex_account_refresh_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CodexAccount(CodexAccountPaneAction::Refresh));
            }
            if codex_account_login_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CodexAccount(
                    CodexAccountPaneAction::LoginChatgpt,
                ));
            }
            if codex_account_cancel_login_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CodexAccount(
                    CodexAccountPaneAction::CancelLogin,
                ));
            }
            if codex_account_logout_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CodexAccount(CodexAccountPaneAction::Logout));
            }
            if codex_account_rate_limits_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CodexAccount(
                    CodexAccountPaneAction::RateLimits,
                ));
            }
            None
        }
        PaneKind::CodexModels => {
            if codex_models_refresh_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CodexModels(CodexModelsPaneAction::Refresh));
            }
            if codex_models_toggle_hidden_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CodexModels(
                    CodexModelsPaneAction::ToggleHidden,
                ));
            }
            None
        }
        PaneKind::CodexConfig => {
            if codex_config_read_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CodexConfig(CodexConfigPaneAction::Read));
            }
            if codex_config_requirements_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CodexConfig(
                    CodexConfigPaneAction::Requirements,
                ));
            }
            if codex_config_write_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CodexConfig(
                    CodexConfigPaneAction::WriteSample,
                ));
            }
            if codex_config_batch_write_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CodexConfig(
                    CodexConfigPaneAction::BatchWriteSample,
                ));
            }
            if codex_config_detect_external_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CodexConfig(
                    CodexConfigPaneAction::DetectExternal,
                ));
            }
            if codex_config_import_external_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CodexConfig(
                    CodexConfigPaneAction::ImportExternal,
                ));
            }
            None
        }
        PaneKind::CodexMcp => {
            if codex_mcp_refresh_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CodexMcp(CodexMcpPaneAction::Refresh));
            }
            if codex_mcp_login_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CodexMcp(CodexMcpPaneAction::LoginSelected));
            }
            if codex_mcp_reload_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CodexMcp(CodexMcpPaneAction::Reload));
            }
            let visible_rows = codex_mcp_visible_row_count(state.codex_mcp.servers.len());
            for row_index in 0..visible_rows {
                if codex_mcp_row_bounds(content_bounds, row_index).contains(point) {
                    return Some(PaneHitAction::CodexMcp(CodexMcpPaneAction::SelectRow(
                        row_index,
                    )));
                }
            }
            None
        }
        PaneKind::CodexApps => {
            if codex_apps_refresh_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CodexApps(CodexAppsPaneAction::Refresh));
            }
            let visible_rows = codex_apps_visible_row_count(state.codex_apps.apps.len());
            for row_index in 0..visible_rows {
                if codex_apps_row_bounds(content_bounds, row_index).contains(point) {
                    return Some(PaneHitAction::CodexApps(CodexAppsPaneAction::SelectRow(
                        row_index,
                    )));
                }
            }
            None
        }
        PaneKind::CodexLabs => {
            if codex_labs_review_inline_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CodexLabs(CodexLabsPaneAction::ReviewInline));
            }
            if codex_labs_review_detached_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CodexLabs(
                    CodexLabsPaneAction::ReviewDetached,
                ));
            }
            if codex_labs_command_exec_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CodexLabs(CodexLabsPaneAction::CommandExec));
            }
            if codex_labs_collaboration_modes_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CodexLabs(
                    CodexLabsPaneAction::CollaborationModes,
                ));
            }
            if codex_labs_experimental_features_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CodexLabs(
                    CodexLabsPaneAction::ExperimentalFeatures,
                ));
            }
            if codex_labs_toggle_experimental_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CodexLabs(
                    CodexLabsPaneAction::ToggleExperimental,
                ));
            }
            if codex_labs_realtime_start_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CodexLabs(CodexLabsPaneAction::RealtimeStart));
            }
            if codex_labs_realtime_append_text_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CodexLabs(
                    CodexLabsPaneAction::RealtimeAppendText,
                ));
            }
            if codex_labs_realtime_stop_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CodexLabs(CodexLabsPaneAction::RealtimeStop));
            }
            if codex_labs_windows_sandbox_setup_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CodexLabs(
                    CodexLabsPaneAction::WindowsSandboxSetup,
                ));
            }
            if codex_labs_fuzzy_start_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CodexLabs(CodexLabsPaneAction::FuzzyStart));
            }
            if codex_labs_fuzzy_update_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CodexLabs(CodexLabsPaneAction::FuzzyUpdate));
            }
            if codex_labs_fuzzy_stop_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CodexLabs(CodexLabsPaneAction::FuzzyStop));
            }
            None
        }
        PaneKind::CodexDiagnostics => {
            if codex_diagnostics_enable_wire_log_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CodexDiagnostics(
                    CodexDiagnosticsPaneAction::EnableWireLog,
                ));
            }
            if codex_diagnostics_disable_wire_log_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CodexDiagnostics(
                    CodexDiagnosticsPaneAction::DisableWireLog,
                ));
            }
            if codex_diagnostics_clear_events_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CodexDiagnostics(
                    CodexDiagnosticsPaneAction::ClearEvents,
                ));
            }
            None
        }
        PaneKind::EarningsScoreboard => {
            if earnings_scoreboard_refresh_button_bounds(content_bounds).contains(point) {
                Some(PaneHitAction::EarningsScoreboard(
                    EarningsScoreboardPaneAction::Refresh,
                ))
            } else {
                None
            }
        }
        PaneKind::RelayConnections => {
            if relay_connections_add_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::RelayConnections(
                    RelayConnectionsPaneAction::AddRelay,
                ));
            }
            if relay_connections_remove_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::RelayConnections(
                    RelayConnectionsPaneAction::RemoveSelected,
                ));
            }
            if relay_connections_retry_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::RelayConnections(
                    RelayConnectionsPaneAction::RetrySelected,
                ));
            }

            let visible_rows =
                relay_connections_visible_row_count(state.relay_connections.relays.len());
            for row_index in 0..visible_rows {
                if relay_connections_row_bounds(content_bounds, row_index).contains(point) {
                    return Some(PaneHitAction::RelayConnections(
                        RelayConnectionsPaneAction::SelectRow(row_index),
                    ));
                }
            }
            None
        }
        PaneKind::SyncHealth => {
            if sync_health_rebootstrap_button_bounds(content_bounds).contains(point) {
                Some(PaneHitAction::SyncHealth(SyncHealthPaneAction::Rebootstrap))
            } else {
                None
            }
        }
        PaneKind::ProviderStatus => {
            for (row_index, target) in crate::app_state::ProviderInventoryProductToggleTarget::all()
                .iter()
                .enumerate()
            {
                if provider_inventory_toggle_button_bounds(content_bounds, row_index)
                    .contains(point)
                {
                    return Some(PaneHitAction::ProviderStatus(
                        ProviderStatusPaneAction::ToggleInventory(*target),
                    ));
                }
            }
            None
        }
        PaneKind::LocalInference => {
            if local_inference_refresh_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::LocalInference(
                    LocalInferencePaneAction::RefreshRuntime,
                ));
            }
            if local_inference_warm_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::LocalInference(
                    LocalInferencePaneAction::WarmModel,
                ));
            }
            if local_inference_unload_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::LocalInference(
                    LocalInferencePaneAction::UnloadModel,
                ));
            }
            if local_inference_run_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::LocalInference(
                    LocalInferencePaneAction::RunPrompt,
                ));
            }
            None
        }
        PaneKind::NetworkRequests => {
            if network_requests_submit_button_bounds(content_bounds).contains(point) {
                Some(PaneHitAction::NetworkRequests(
                    NetworkRequestsPaneAction::RequestQuotes,
                ))
            } else if network_requests_accept_button_bounds(content_bounds).contains(point) {
                Some(PaneHitAction::NetworkRequests(
                    NetworkRequestsPaneAction::AcceptSelectedQuote,
                ))
            } else {
                let visible_rows = network_requests_visible_quote_count(
                    state.network_requests.active_quote_count(),
                );
                for row_index in 0..visible_rows {
                    if network_requests_quote_row_bounds(content_bounds, row_index).contains(point)
                    {
                        return Some(PaneHitAction::NetworkRequests(
                            NetworkRequestsPaneAction::SelectQuote(row_index),
                        ));
                    }
                }
                None
            }
        }
        PaneKind::StarterJobs => {
            if starter_jobs_complete_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::StarterJobs(
                    StarterJobsPaneAction::CompleteSelected,
                ));
            }
            if starter_jobs_kill_switch_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::StarterJobs(
                    StarterJobsPaneAction::ToggleKillSwitch,
                ));
            }

            let visible_rows = starter_jobs_visible_row_count(state.starter_jobs.jobs.len());
            for row_index in 0..visible_rows {
                if starter_jobs_row_bounds(content_bounds, row_index).contains(point) {
                    return Some(PaneHitAction::StarterJobs(
                        StarterJobsPaneAction::SelectRow(row_index),
                    ));
                }
            }
            None
        }
        PaneKind::ReciprocalLoop => {
            if reciprocal_loop_start_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::ReciprocalLoop(
                    ReciprocalLoopPaneAction::Start,
                ));
            }
            if reciprocal_loop_stop_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::ReciprocalLoop(
                    ReciprocalLoopPaneAction::Stop,
                ));
            }
            if reciprocal_loop_reset_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::ReciprocalLoop(
                    ReciprocalLoopPaneAction::Reset,
                ));
            }
            None
        }
        PaneKind::ActivityFeed => {
            if activity_feed_refresh_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::ActivityFeed(ActivityFeedPaneAction::Refresh));
            }
            if activity_feed_prev_page_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::ActivityFeed(
                    ActivityFeedPaneAction::PreviousPage,
                ));
            }
            if activity_feed_next_page_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::ActivityFeed(
                    ActivityFeedPaneAction::NextPage,
                ));
            }

            let filters = ActivityFeedFilter::all();
            for (filter_index, filter) in filters.into_iter().enumerate() {
                if activity_feed_filter_button_bounds(content_bounds, filter_index).contains(point)
                {
                    return Some(PaneHitAction::ActivityFeed(
                        ActivityFeedPaneAction::SetFilter(filter),
                    ));
                }
            }

            let visible_rows =
                activity_feed_visible_row_count(state.activity_feed.visible_rows().len());
            for row_index in 0..visible_rows {
                if activity_feed_row_bounds(content_bounds, row_index).contains(point) {
                    return Some(PaneHitAction::ActivityFeed(
                        ActivityFeedPaneAction::SelectRow(row_index),
                    ));
                }
            }
            None
        }
        PaneKind::AlertsRecovery => {
            if alerts_recovery_recover_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::AlertsRecovery(
                    AlertsRecoveryPaneAction::RecoverSelected,
                ));
            }
            if alerts_recovery_ack_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::AlertsRecovery(
                    AlertsRecoveryPaneAction::AcknowledgeSelected,
                ));
            }
            if alerts_recovery_resolve_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::AlertsRecovery(
                    AlertsRecoveryPaneAction::ResolveSelected,
                ));
            }

            let visible_rows =
                alerts_recovery_visible_row_count(state.alerts_recovery.alerts.len());
            for row_index in 0..visible_rows {
                if alerts_recovery_row_bounds(content_bounds, row_index).contains(point) {
                    return Some(PaneHitAction::AlertsRecovery(
                        AlertsRecoveryPaneAction::SelectRow(row_index),
                    ));
                }
            }
            None
        }
        PaneKind::Settings => {
            if settings_save_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::Settings(SettingsPaneAction::Save));
            }
            if settings_reset_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::Settings(SettingsPaneAction::ResetDefaults));
            }
            None
        }
        PaneKind::Credentials => {
            if credentials_add_custom_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::Credentials(CredentialsPaneAction::AddCustom));
            }
            if credentials_save_value_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::Credentials(CredentialsPaneAction::SaveValue));
            }
            if credentials_delete_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::Credentials(
                    CredentialsPaneAction::DeleteOrClear,
                ));
            }
            if credentials_toggle_enabled_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::Credentials(
                    CredentialsPaneAction::ToggleEnabled,
                ));
            }
            if credentials_import_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::Credentials(
                    CredentialsPaneAction::ImportFromEnv,
                ));
            }
            if credentials_reload_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::Credentials(CredentialsPaneAction::Reload));
            }
            if credentials_scope_codex_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::Credentials(
                    CredentialsPaneAction::ToggleScopeCodex,
                ));
            }
            if credentials_scope_spark_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::Credentials(
                    CredentialsPaneAction::ToggleScopeSpark,
                ));
            }
            if credentials_scope_skills_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::Credentials(
                    CredentialsPaneAction::ToggleScopeSkills,
                ));
            }
            if credentials_scope_global_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::Credentials(
                    CredentialsPaneAction::ToggleScopeGlobal,
                ));
            }

            let visible_rows = credentials_visible_row_count(state.credentials.entries.len());
            for row_index in 0..visible_rows {
                if credentials_row_bounds(content_bounds, row_index).contains(point) {
                    return Some(PaneHitAction::Credentials(
                        CredentialsPaneAction::SelectRow(row_index),
                    ));
                }
            }
            None
        }
        PaneKind::JobInbox => {
            if job_inbox_accept_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::JobInbox(JobInboxPaneAction::AcceptSelected));
            }
            if job_inbox_reject_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::JobInbox(JobInboxPaneAction::RejectSelected));
            }

            let visible_rows = job_inbox_visible_row_count(state.job_inbox.requests.len());
            for row_index in 0..visible_rows {
                if job_inbox_row_bounds(content_bounds, row_index).contains(point) {
                    return Some(PaneHitAction::JobInbox(JobInboxPaneAction::SelectRow(
                        row_index,
                    )));
                }
            }
            None
        }
        PaneKind::ActiveJob => {
            if state.active_job.runtime_supports_abort
                && active_job_abort_button_bounds(content_bounds).contains(point)
            {
                return Some(PaneHitAction::ActiveJob(ActiveJobPaneAction::AbortJob));
            }
            None
        }
        PaneKind::JobHistory => {
            if job_history_status_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::JobHistory(
                    JobHistoryPaneAction::CycleStatusFilter,
                ));
            }
            if job_history_time_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::JobHistory(
                    JobHistoryPaneAction::CycleTimeRange,
                ));
            }
            if job_history_prev_page_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::JobHistory(
                    JobHistoryPaneAction::PreviousPage,
                ));
            }
            if job_history_next_page_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::JobHistory(JobHistoryPaneAction::NextPage));
            }
            None
        }
        PaneKind::AgentProfileState => {
            if agent_profile_publish_profile_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::AgentProfileState(
                    AgentProfileStatePaneAction::PublishProfile,
                ));
            }
            if agent_profile_publish_state_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::AgentProfileState(
                    AgentProfileStatePaneAction::PublishState,
                ));
            }
            if agent_profile_update_goals_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::AgentProfileState(
                    AgentProfileStatePaneAction::UpdateGoals,
                ));
            }
            if agent_profile_create_goal_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::AgentProfileState(
                    AgentProfileStatePaneAction::CreateGoal,
                ));
            }
            if agent_profile_start_goal_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::AgentProfileState(
                    AgentProfileStatePaneAction::StartGoal,
                ));
            }
            if agent_profile_abort_goal_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::AgentProfileState(
                    AgentProfileStatePaneAction::AbortGoal,
                ));
            }
            if agent_profile_receipt_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::AgentProfileState(
                    AgentProfileStatePaneAction::InspectGoalReceipt,
                ));
            }
            None
        }
        PaneKind::AgentScheduleTick => {
            if agent_schedule_apply_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::AgentScheduleTick(
                    AgentScheduleTickPaneAction::ApplySchedule,
                ));
            }
            if agent_schedule_manual_tick_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::AgentScheduleTick(
                    AgentScheduleTickPaneAction::PublishManualTick,
                ));
            }
            if agent_schedule_inspect_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::AgentScheduleTick(
                    AgentScheduleTickPaneAction::InspectLastResult,
                ));
            }
            if agent_schedule_toggle_os_scheduler_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::AgentScheduleTick(
                    AgentScheduleTickPaneAction::ToggleOsSchedulerAdapter,
                ));
            }
            None
        }
        PaneKind::TrajectoryAudit => {
            if trajectory_open_session_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::TrajectoryAudit(
                    TrajectoryAuditPaneAction::OpenSession,
                ));
            }
            if trajectory_filter_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::TrajectoryAudit(
                    TrajectoryAuditPaneAction::CycleStepFilter,
                ));
            }
            if trajectory_verify_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::TrajectoryAudit(
                    TrajectoryAuditPaneAction::VerifyTrajectoryHash,
                ));
            }
            None
        }
        PaneKind::CastControl => {
            if cast_refresh_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CastControl(
                    CastControlPaneAction::RefreshStatus,
                ));
            }
            if cast_check_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CastControl(CastControlPaneAction::RunCheck));
            }
            if cast_prove_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CastControl(CastControlPaneAction::RunProve));
            }
            if cast_sign_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CastControl(
                    CastControlPaneAction::RunSignBroadcast,
                ));
            }
            if cast_inspect_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CastControl(
                    CastControlPaneAction::RunInspect,
                ));
            }
            if cast_toggle_broadcast_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CastControl(
                    CastControlPaneAction::ToggleBroadcastArmed,
                ));
            }
            if cast_loop_once_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CastControl(
                    CastControlPaneAction::RunLoopOnce,
                ));
            }
            if cast_toggle_loop_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CastControl(
                    CastControlPaneAction::ToggleAutoLoop,
                ));
            }
            None
        }
        PaneKind::SkillRegistry => {
            if skill_registry_discover_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::SkillRegistry(
                    SkillRegistryPaneAction::DiscoverSkills,
                ));
            }
            if skill_registry_inspect_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::SkillRegistry(
                    SkillRegistryPaneAction::InspectManifest,
                ));
            }
            if skill_registry_install_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::SkillRegistry(
                    SkillRegistryPaneAction::InstallSelectedSkill,
                ));
            }
            let visible_rows =
                skill_registry_visible_row_count(state.skill_registry.discovered_skills.len());
            for row_index in 0..visible_rows {
                if skill_registry_row_bounds(content_bounds, row_index).contains(point) {
                    return Some(PaneHitAction::SkillRegistry(
                        SkillRegistryPaneAction::SelectRow(row_index),
                    ));
                }
            }
            None
        }
        PaneKind::SkillTrustRevocation => {
            if skill_trust_refresh_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::SkillTrustRevocation(
                    SkillTrustRevocationPaneAction::RefreshTrust,
                ));
            }
            if skill_trust_attestations_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::SkillTrustRevocation(
                    SkillTrustRevocationPaneAction::InspectAttestations,
                ));
            }
            if skill_trust_kill_switch_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::SkillTrustRevocation(
                    SkillTrustRevocationPaneAction::ToggleKillSwitch,
                ));
            }
            if skill_trust_revoke_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::SkillTrustRevocation(
                    SkillTrustRevocationPaneAction::RevokeSkill,
                ));
            }
            None
        }
        PaneKind::CreditDesk => {
            if credit_desk_intent_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CreditDesk(
                    CreditDeskPaneAction::PublishIntent,
                ));
            }
            if credit_desk_offer_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CreditDesk(
                    CreditDeskPaneAction::PublishOffer,
                ));
            }
            if credit_desk_envelope_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CreditDesk(
                    CreditDeskPaneAction::PublishEnvelope,
                ));
            }
            if credit_desk_spend_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CreditDesk(
                    CreditDeskPaneAction::AuthorizeSpend,
                ));
            }
            None
        }
        PaneKind::CreditSettlementLedger => {
            if credit_settlement_verify_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CreditSettlementLedger(
                    CreditSettlementLedgerPaneAction::VerifySettlement,
                ));
            }
            if credit_settlement_default_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CreditSettlementLedger(
                    CreditSettlementLedgerPaneAction::EmitDefaultNotice,
                ));
            }
            if credit_settlement_reputation_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CreditSettlementLedger(
                    CreditSettlementLedgerPaneAction::EmitReputationLabel,
                ));
            }
            None
        }
        PaneKind::CadDemo => {
            if cad_demo_cycle_variant_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CadDemo(CadDemoPaneAction::CycleVariant));
            }
            if cad_demo_gripper_jaw_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CadDemo(
                    CadDemoPaneAction::ToggleGripperJawAnimation,
                ));
            }
            if cad_demo_reset_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CadDemo(CadDemoPaneAction::BootstrapDemo));
            }
            if cad_demo_reset_camera_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CadDemo(CadDemoPaneAction::ResetCamera));
            }
            if cad_demo_projection_mode_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CadDemo(
                    CadDemoPaneAction::ToggleProjectionMode,
                ));
            }
            if cad_demo_viewport_layout_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CadDemo(
                    CadDemoPaneAction::ToggleViewportLayout,
                ));
            }
            if cad_demo_drawing_mode_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CadDemo(
                    CadDemoPaneAction::ToggleDrawingViewMode,
                ));
            }
            if cad_demo_drawing_direction_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CadDemo(
                    CadDemoPaneAction::CycleDrawingViewDirection,
                ));
            }
            if cad_demo_drawing_hidden_lines_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CadDemo(
                    CadDemoPaneAction::ToggleDrawingHiddenLines,
                ));
            }
            if cad_demo_drawing_dimensions_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CadDemo(
                    CadDemoPaneAction::ToggleDrawingDimensions,
                ));
            }
            if cad_demo_drawing_reset_view_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CadDemo(CadDemoPaneAction::ResetDrawingView));
            }
            if cad_demo_drawing_add_detail_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CadDemo(
                    CadDemoPaneAction::AddDrawingDetailView,
                ));
            }
            if cad_demo_drawing_clear_details_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CadDemo(
                    CadDemoPaneAction::ClearDrawingDetailViews,
                ));
            }
            if cad_demo_section_plane_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CadDemo(CadDemoPaneAction::CycleSectionPlane));
            }
            if cad_demo_section_offset_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CadDemo(
                    CadDemoPaneAction::StepSectionPlaneOffset,
                ));
            }
            if cad_demo_material_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CadDemo(
                    CadDemoPaneAction::CycleMaterialPreset,
                ));
            }
            if cad_demo_sensor_mode_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CadDemo(
                    CadDemoPaneAction::CycleSensorVisualizationMode,
                ));
            }
            if cad_demo_snap_grid_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CadDemo(CadDemoPaneAction::ToggleSnapGrid));
            }
            if cad_demo_snap_origin_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CadDemo(CadDemoPaneAction::ToggleSnapOrigin));
            }
            if cad_demo_snap_endpoint_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CadDemo(
                    CadDemoPaneAction::ToggleSnapEndpoint,
                ));
            }
            if cad_demo_snap_midpoint_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CadDemo(
                    CadDemoPaneAction::ToggleSnapMidpoint,
                ));
            }
            if cad_demo_hotkey_profile_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CadDemo(
                    CadDemoPaneAction::CycleHotkeyProfile,
                ));
            }
            if cad_demo_view_snap_top_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CadDemo(CadDemoPaneAction::SnapViewTop));
            }
            if cad_demo_view_snap_front_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CadDemo(CadDemoPaneAction::SnapViewFront));
            }
            if cad_demo_view_snap_right_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CadDemo(CadDemoPaneAction::SnapViewRight));
            }
            if cad_demo_view_snap_iso_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CadDemo(CadDemoPaneAction::SnapViewIsometric));
            }
            if cad_demo_hidden_line_mode_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CadDemo(
                    CadDemoPaneAction::CycleHiddenLineMode,
                ));
            }
            if cad_demo_warning_filter_severity_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CadDemo(
                    CadDemoPaneAction::CycleWarningSeverityFilter,
                ));
            }
            if cad_demo_warning_filter_code_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CadDemo(
                    CadDemoPaneAction::CycleWarningCodeFilter,
                ));
            }
            let dimension_rows = state.cad_demo.visible_dimension_indices().len().min(4);
            for index in 0..dimension_rows {
                if cad_demo_dimension_row_bounds(content_bounds, index).contains(point) {
                    return Some(PaneHitAction::CadDemo(
                        CadDemoPaneAction::StartDimensionEdit(index),
                    ));
                }
            }
            let visible_timeline_rows = state
                .cad_demo
                .timeline_rows
                .len()
                .saturating_sub(state.cad_demo.timeline_scroll_offset)
                .min(10);
            for index in 0..visible_timeline_rows {
                if cad_demo_timeline_row_bounds(content_bounds, index).contains(point) {
                    return Some(PaneHitAction::CadDemo(
                        CadDemoPaneAction::SelectTimelineRow(index),
                    ));
                }
            }
            let warning_filter_severity = state.cad_demo.warning_filter_severity.as_str();
            let warning_filter_code = state.cad_demo.warning_filter_code.as_str();
            let visible_warnings = state
                .cad_demo
                .warnings
                .iter()
                .filter(|warning| {
                    let severity_ok = warning_filter_severity == "all"
                        || warning
                            .severity
                            .eq_ignore_ascii_case(warning_filter_severity);
                    let code_ok = warning_filter_code == "all"
                        || warning.code.eq_ignore_ascii_case(warning_filter_code);
                    severity_ok && code_ok
                })
                .count()
                .min(8);
            for index in 0..visible_warnings {
                if cad_demo_warning_row_bounds(content_bounds, index).contains(point) {
                    return Some(PaneHitAction::CadDemo(CadDemoPaneAction::SelectWarning(
                        index,
                    )));
                }
                if cad_demo_warning_marker_bounds(content_bounds, index).contains(point) {
                    return Some(PaneHitAction::CadDemo(
                        CadDemoPaneAction::SelectWarningMarker(index),
                    ));
                }
            }
            None
        }
        PaneKind::SparkWallet => {
            let layout = spark_pane::layout(content_bounds);
            spark_pane::hit_action(layout, point).map(PaneHitAction::Spark)
        }
        PaneKind::SparkCreateInvoice => {
            let layout = spark_pane::create_invoice_layout(content_bounds);
            spark_pane::hit_create_invoice_action(layout, point)
                .map(PaneHitAction::SparkCreateInvoice)
        }
        PaneKind::SparkPayInvoice => {
            let layout = spark_pane::pay_invoice_layout(content_bounds);
            spark_pane::hit_pay_invoice_action(layout, point).map(PaneHitAction::SparkPayInvoice)
        }
        PaneKind::Empty => None,
    }
}

pub fn dispatch_spark_input_event(state: &mut RenderState, event: &InputEvent) -> bool {
    wallet_pane::dispatch_spark_input_event(state, event)
}

pub fn dispatch_create_invoice_input_event(state: &mut RenderState, event: &InputEvent) -> bool {
    wallet_pane::dispatch_create_invoice_input_event(state, event)
}

pub fn dispatch_pay_invoice_input_event(state: &mut RenderState, event: &InputEvent) -> bool {
    wallet_pane::dispatch_pay_invoice_input_event(state, event)
}

pub fn dispatch_chat_input_event(state: &mut RenderState, event: &InputEvent) -> bool {
    chat_pane::dispatch_input_event(state, event)
}

pub fn dispatch_calculator_input_event(state: &mut RenderState, event: &InputEvent) -> bool {
    calculator_pane::dispatch_input_event(state, event)
}

pub fn dispatch_chat_scroll_event(
    state: &mut RenderState,
    cursor_position: Point,
    scroll_dy: f32,
) -> bool {
    chat_pane::dispatch_transcript_scroll_event(state, cursor_position, scroll_dy)
}

pub fn dispatch_mission_control_log_scroll_event(
    state: &mut RenderState,
    cursor_position: Point,
    event: &InputEvent,
) -> bool {
    let Some(pane_idx) = pane_indices_by_z_desc(state)
        .into_iter()
        .find(|index| state.panes[*index].bounds.contains(cursor_position))
    else {
        return false;
    };
    let pane = &state.panes[pane_idx];
    if pane.kind != PaneKind::GoOnline {
        return false;
    }

    let content_bounds = pane_content_bounds(pane.bounds);
    let log_bounds = mission_control_log_stream_bounds(content_bounds);
    if !log_bounds.contains(cursor_position) {
        return false;
    }

    state
        .mission_control
        .log_stream
        .event(event, log_bounds, &mut state.event_context)
        .is_handled()
}

pub fn dispatch_activity_feed_detail_scroll_event(
    state: &mut RenderState,
    cursor_position: Point,
    scroll_dy: f32,
) -> bool {
    if scroll_dy.abs() <= f32::EPSILON {
        return false;
    }

    let Some(pane_idx) = pane_indices_by_z_desc(state)
        .into_iter()
        .find(|index| state.panes[*index].bounds.contains(cursor_position))
    else {
        return false;
    };
    let pane = &state.panes[pane_idx];
    if pane.kind != PaneKind::ActivityFeed {
        return false;
    }

    let content_bounds = pane_content_bounds(pane.bounds);
    if !content_bounds.contains(cursor_position) {
        return false;
    }
    let visible_rows = activity_feed_visible_row_count(state.activity_feed.visible_rows().len());
    let Some(detail_viewport) = activity_feed_detail_viewport_bounds(content_bounds, visible_rows)
    else {
        return false;
    };
    if !detail_viewport.contains(cursor_position) {
        return false;
    }

    let total_lines = {
        let feed = &state.activity_feed;
        let Some(selected) = feed.selected() else {
            return false;
        };
        if !feed.active_filter.matches_row(selected) {
            return false;
        }
        activity_feed_detail_wrapped_line_count(selected.detail.as_str())
    };
    let visible_lines = activity_feed_detail_visible_line_capacity(content_bounds, visible_rows);
    if visible_lines == 0 {
        return false;
    }
    state
        .activity_feed
        .scroll_detail_lines_by(scroll_dy, total_lines, visible_lines)
}

pub fn dispatch_job_history_input_event(state: &mut RenderState, event: &InputEvent) -> bool {
    let top_history = state
        .panes
        .iter()
        .filter(|pane| pane.kind == PaneKind::JobHistory)
        .max_by_key(|pane| pane.z_index)
        .map(|pane| pane.bounds);
    let Some(bounds) = top_history else {
        return false;
    };

    let search_bounds = job_history_search_input_bounds(pane_content_bounds(bounds));
    let handled = state
        .job_history_inputs
        .search_job_id
        .event(event, search_bounds, &mut state.event_context)
        .is_handled();
    state.job_history.set_search_job_id(
        state
            .job_history_inputs
            .search_job_id
            .get_value()
            .to_string(),
    );
    handled
}

pub fn dispatch_relay_connections_input_event(state: &mut RenderState, event: &InputEvent) -> bool {
    relay_connections_pane::dispatch_input_event(state, event)
}

pub fn dispatch_local_inference_input_event(state: &mut RenderState, event: &InputEvent) -> bool {
    local_inference_pane::dispatch_input_event(state, event)
}

pub fn dispatch_network_requests_input_event(state: &mut RenderState, event: &InputEvent) -> bool {
    let top_network = state
        .panes
        .iter()
        .filter(|pane| pane.kind == PaneKind::NetworkRequests)
        .max_by_key(|pane| pane.z_index)
        .map(|pane| pane.bounds);
    let Some(bounds) = top_network else {
        return false;
    };

    let content_bounds = pane_content_bounds(bounds);
    let mut handled = false;
    handled |= state
        .network_requests_inputs
        .compute_family
        .event(
            event,
            network_requests_type_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .network_requests_inputs
        .preferred_backend
        .event(
            event,
            network_requests_payload_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .network_requests_inputs
        .capability_constraints
        .event(
            event,
            network_requests_skill_scope_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .network_requests_inputs
        .quantity
        .event(
            event,
            network_requests_credit_envelope_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .network_requests_inputs
        .delivery_start_minutes
        .event(
            event,
            network_requests_budget_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .network_requests_inputs
        .window_minutes
        .event(
            event,
            network_requests_timeout_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .network_requests_inputs
        .max_price_sats
        .event(
            event,
            network_requests_max_price_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled
}

pub fn dispatch_settings_input_event(state: &mut RenderState, event: &InputEvent) -> bool {
    let top_settings = state
        .panes
        .iter()
        .filter(|pane| pane.kind == PaneKind::Settings)
        .max_by_key(|pane| pane.z_index)
        .map(|pane| pane.bounds);
    let Some(bounds) = top_settings else {
        return false;
    };

    let content_bounds = pane_content_bounds(bounds);
    let mut handled = false;
    handled |= state
        .settings_inputs
        .relay_url
        .event(
            event,
            settings_relay_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .settings_inputs
        .wallet_default_send_sats
        .event(
            event,
            settings_wallet_default_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .settings_inputs
        .provider_max_queue_depth
        .event(
            event,
            settings_provider_queue_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled
}

pub fn dispatch_credentials_input_event(state: &mut RenderState, event: &InputEvent) -> bool {
    let top_credentials = state
        .panes
        .iter()
        .filter(|pane| pane.kind == PaneKind::Credentials)
        .max_by_key(|pane| pane.z_index)
        .map(|pane| pane.bounds);
    let Some(bounds) = top_credentials else {
        return false;
    };

    let content_bounds = pane_content_bounds(bounds);
    let mut handled = false;
    handled |= state
        .credentials_inputs
        .variable_name
        .event(
            event,
            credentials_name_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .credentials_inputs
        .variable_value
        .event(
            event,
            credentials_value_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled
}

pub fn bring_pane_to_front_by_id(state: &mut RenderState, pane_id: u64) {
    bring_pane_to_front(state, pane_id);
}

pub(crate) fn pane_indices_by_z_desc(state: &RenderState) -> Vec<usize> {
    PANE_Z_SORT_INVOCATIONS.fetch_add(1, Ordering::Relaxed);
    let mut ordered: Vec<usize> = (0..state.panes.len()).collect();
    ordered.sort_by(|lhs, rhs| state.panes[*rhs].z_index.cmp(&state.panes[*lhs].z_index));
    ordered
}

pub(crate) fn pane_z_sort_invocation_count() -> u64 {
    PANE_Z_SORT_INVOCATIONS.load(Ordering::Relaxed)
}

fn bring_pane_to_front(state: &mut RenderState, pane_id: u64) {
    if let Some(pane) = state.panes.iter_mut().find(|pane| pane.id == pane_id) {
        pane.z_index = state.next_z_index;
        state.next_z_index = state.next_z_index.saturating_add(1);
    }
}

fn pane_title(kind: PaneKind, pane_id: u64) -> String {
    let title = pane_spec(kind).title;
    match kind {
        PaneKind::Empty => format!("{title} {pane_id}"),
        _ => title.to_string(),
    }
}

/// Re-clamp all pane bounds to the current window and sidebar so panes never overlap the sidebar.
/// Call this when the sidebar width or window size changes (e.g. during sidebar drag).
pub fn clamp_all_panes_to_window(state: &mut RenderState) {
    let logical = logical_size(&state.config, state.scale_factor);
    let sidebar_width = sidebar_reserved_width(state);
    for pane in state.panes.iter_mut() {
        pane.bounds = clamp_bounds_to_window(
            pane.bounds,
            logical,
            sidebar_width,
            pane_minimum_size(pane.kind),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PaneDescriptor, active_job_abort_button_bounds, active_job_advance_button_bounds,
        activity_feed_detail_viewport_bounds, activity_feed_details_bounds,
        activity_feed_filter_button_bounds, activity_feed_next_page_button_bounds,
        activity_feed_prev_page_button_bounds, activity_feed_refresh_button_bounds,
        activity_feed_row_bounds, agent_profile_abort_goal_button_bounds,
        agent_profile_create_goal_button_bounds, agent_profile_publish_profile_button_bounds,
        agent_profile_publish_state_button_bounds, agent_profile_receipt_button_bounds,
        agent_profile_start_goal_button_bounds, agent_profile_update_goals_button_bounds,
        agent_schedule_apply_button_bounds, agent_schedule_inspect_button_bounds,
        agent_schedule_manual_tick_button_bounds, agent_schedule_toggle_os_scheduler_button_bounds,
        alerts_recovery_ack_button_bounds, alerts_recovery_recover_button_bounds,
        alerts_recovery_resolve_button_bounds, alerts_recovery_row_bounds,
        cad_action_uses_dense_row_hot_zone, cad_demo_context_menu_bounds,
        cad_demo_context_menu_row_bounds, cad_demo_cycle_variant_button_bounds,
        cad_demo_dimension_panel_bounds, cad_demo_dimension_row_bounds,
        cad_demo_drawing_add_detail_button_bounds, cad_demo_drawing_clear_details_button_bounds,
        cad_demo_drawing_dimensions_button_bounds, cad_demo_drawing_direction_button_bounds,
        cad_demo_drawing_hidden_lines_button_bounds, cad_demo_drawing_mode_button_bounds,
        cad_demo_drawing_reset_view_button_bounds, cad_demo_gripper_jaw_button_bounds,
        cad_demo_hidden_line_mode_button_bounds, cad_demo_hotkey_profile_button_bounds,
        cad_demo_material_button_bounds, cad_demo_projection_mode_button_bounds,
        cad_demo_reset_button_bounds, cad_demo_reset_camera_button_bounds,
        cad_demo_section_offset_button_bounds, cad_demo_section_plane_button_bounds,
        cad_demo_sensor_mode_button_bounds, cad_demo_snap_endpoint_button_bounds,
        cad_demo_snap_grid_button_bounds, cad_demo_snap_midpoint_button_bounds,
        cad_demo_snap_origin_button_bounds, cad_demo_timeline_panel_bounds,
        cad_demo_timeline_row_bounds, cad_demo_view_cube_bounds,
        cad_demo_view_snap_front_button_bounds, cad_demo_view_snap_iso_button_bounds,
        cad_demo_view_snap_right_button_bounds, cad_demo_view_snap_top_button_bounds,
        cad_demo_viewport_layout_button_bounds, cad_demo_warning_filter_code_button_bounds,
        cad_demo_warning_filter_severity_button_bounds, cad_demo_warning_marker_bounds,
        cad_demo_warning_panel_bounds, cad_demo_warning_row_bounds,
        cad_palette_action_for_command_id, cad_palette_command_specs, chat_composer_input_bounds,
        chat_send_button_bounds, chat_thread_rail_bounds, chat_transcript_body_bounds_with_height,
        chat_transcript_bounds, chat_workspace_rail_bounds,
        codex_account_cancel_login_button_bounds, codex_account_login_button_bounds,
        codex_account_logout_button_bounds, codex_account_rate_limits_button_bounds,
        codex_account_refresh_button_bounds, codex_apps_refresh_button_bounds,
        codex_config_batch_write_button_bounds, codex_config_detect_external_button_bounds,
        codex_config_import_external_button_bounds, codex_config_read_button_bounds,
        codex_config_requirements_button_bounds, codex_config_write_button_bounds,
        codex_diagnostics_clear_events_button_bounds,
        codex_diagnostics_disable_wire_log_button_bounds,
        codex_diagnostics_enable_wire_log_button_bounds,
        codex_labs_collaboration_modes_button_bounds, codex_labs_command_exec_button_bounds,
        codex_labs_experimental_features_button_bounds, codex_labs_review_detached_button_bounds,
        codex_labs_review_inline_button_bounds, codex_labs_toggle_experimental_button_bounds,
        codex_mcp_login_button_bounds, codex_mcp_refresh_button_bounds,
        codex_mcp_reload_button_bounds, codex_models_refresh_button_bounds,
        codex_models_toggle_hidden_button_bounds, credit_desk_envelope_button_bounds,
        credit_desk_intent_button_bounds, credit_desk_offer_button_bounds,
        credit_desk_spend_button_bounds, credit_settlement_default_button_bounds,
        credit_settlement_reputation_button_bounds, credit_settlement_verify_button_bounds,
        earnings_scoreboard_refresh_button_bounds, go_online_toggle_button_bounds,
        job_history_next_page_button_bounds, job_history_prev_page_button_bounds,
        job_history_search_input_bounds, job_history_status_button_bounds,
        job_history_time_button_bounds, job_inbox_accept_button_bounds,
        job_inbox_reject_button_bounds, job_inbox_row_bounds,
        local_inference_max_tokens_input_bounds, local_inference_prompt_input_bounds,
        local_inference_refresh_button_bounds, local_inference_requested_model_input_bounds,
        local_inference_run_button_bounds, local_inference_temperature_input_bounds,
        local_inference_top_k_input_bounds, local_inference_top_p_input_bounds,
        local_inference_unload_button_bounds, local_inference_warm_button_bounds,
        network_requests_budget_input_bounds, network_requests_credit_envelope_input_bounds,
        network_requests_max_price_input_bounds, network_requests_payload_input_bounds,
        network_requests_skill_scope_input_bounds, network_requests_submit_button_bounds,
        network_requests_timeout_input_bounds, network_requests_type_input_bounds,
        nostr_copy_secret_button_bounds, nostr_regenerate_button_bounds,
        nostr_reveal_button_bounds, pane_content_bounds, reciprocal_loop_reset_button_bounds,
        reciprocal_loop_start_button_bounds, reciprocal_loop_stop_button_bounds,
        relay_connections_add_button_bounds, relay_connections_remove_button_bounds,
        relay_connections_retry_button_bounds, relay_connections_row_bounds,
        relay_connections_url_input_bounds, settings_provider_queue_input_bounds,
        settings_relay_input_bounds, settings_reset_button_bounds, settings_save_button_bounds,
        settings_wallet_default_input_bounds, skill_registry_discover_button_bounds,
        skill_registry_inspect_button_bounds, skill_registry_install_button_bounds,
        skill_trust_attestations_button_bounds, skill_trust_kill_switch_button_bounds,
        skill_trust_refresh_button_bounds, skill_trust_revoke_button_bounds,
        starter_jobs_complete_button_bounds, starter_jobs_kill_switch_button_bounds,
        starter_jobs_row_bounds, sync_health_rebootstrap_button_bounds,
        trajectory_filter_button_bounds, trajectory_open_session_button_bounds,
        trajectory_verify_button_bounds,
    };
    use crate::pane_registry::pane_specs;
    use wgpui::{Bounds, Point, Size};

    #[test]
    fn pane_descriptor_singleton_matches_registry_specs() {
        for spec in pane_specs() {
            let descriptor = PaneDescriptor::for_kind(spec.kind);
            assert_eq!(
                descriptor.singleton, spec.singleton,
                "singleton mismatch for {:?}",
                spec.kind
            );
        }
    }

    #[test]
    fn initial_pane_size_prefers_remembered_dimensions() {
        let descriptor = PaneDescriptor::for_kind(crate::app_state::PaneKind::GoOnline);
        let mut pane_size_memory = crate::app_state::PaneSizeMemory::default();
        pane_size_memory.remember(
            crate::app_state::PaneKind::GoOnline,
            Size::new(704.0, 436.0),
        );

        let size = super::initial_pane_size(&pane_size_memory, descriptor);

        assert_eq!(size.width, 704.0);
        assert_eq!(size.height, 436.0);
    }

    #[test]
    fn mission_control_bounds_clamp_to_content_safe_minimum() {
        let min_size = super::pane_minimum_size(crate::app_state::PaneKind::GoOnline);
        let clamped = super::clamp_bounds_to_window(
            Bounds::new(12.0, 16.0, 220.0, 140.0),
            Size::new(1600.0, 900.0),
            0.0,
            min_size,
        );

        assert_eq!(clamped.size.width, min_size.width);
        assert_eq!(clamped.size.height, min_size.height);
    }

    #[test]
    fn mission_control_minimum_size_is_smaller_than_default_but_above_global_floor() {
        let spec = crate::pane_registry::pane_spec(crate::app_state::PaneKind::GoOnline);
        let min_size = super::pane_minimum_size(crate::app_state::PaneKind::GoOnline);

        assert!(min_size.width > super::PANE_MIN_WIDTH);
        assert!(min_size.height > super::PANE_MIN_HEIGHT);
        assert!(min_size.width < spec.default_width);
        assert!(min_size.height < spec.default_height);
    }

    #[test]
    fn cad_palette_command_specs_are_unique_and_resolve_actions() {
        let specs = cad_palette_command_specs();
        assert!(
            specs.len() >= 10,
            "cad command palette parity must cover all hotkey actions"
        );

        let mut ids = std::collections::BTreeSet::new();
        let mut actions = std::collections::BTreeSet::<String>::new();
        for spec in specs {
            assert!(ids.insert(spec.id), "duplicate cad command id {}", spec.id);
            assert!(
                actions.insert(format!("{:?}", spec.action)),
                "duplicate cad command action {:?}",
                spec.action
            );
            assert_eq!(
                cad_palette_action_for_command_id(spec.id),
                Some(spec.action),
                "command id {} must resolve to an action",
                spec.id
            );
        }
        assert_eq!(cad_palette_action_for_command_id("cad.unknown"), None);
    }

    #[test]
    fn cad_dense_row_actions_are_not_pointer_hotspots() {
        assert!(cad_action_uses_dense_row_hot_zone(
            super::CadDemoPaneAction::StartDimensionEdit(0)
        ));
        assert!(cad_action_uses_dense_row_hot_zone(
            super::CadDemoPaneAction::SelectTimelineRow(0)
        ));
        assert!(cad_action_uses_dense_row_hot_zone(
            super::CadDemoPaneAction::SelectWarning(0)
        ));
        assert!(!cad_action_uses_dense_row_hot_zone(
            super::CadDemoPaneAction::SelectWarningMarker(0)
        ));
        assert!(!cad_action_uses_dense_row_hot_zone(
            super::CadDemoPaneAction::CycleVariant
        ));
    }

    #[test]
    fn pane_content_bounds_reserve_title_space() {
        let pane = Bounds::new(10.0, 20.0, 400.0, 300.0);
        let content = pane_content_bounds(pane);

        assert!((content.origin.x - (pane.origin.x + 1.0)).abs() <= f32::EPSILON);
        assert!(content.origin.y > pane.origin.y);
        assert!((content.size.width - (pane.size.width - 2.0)).abs() <= f32::EPSILON);
        assert!(content.size.height < pane.size.height);
    }

    #[test]
    fn nostr_buttons_are_non_overlapping_and_ordered() {
        let content = Bounds::new(0.0, 0.0, 480.0, 220.0);
        let regenerate = nostr_regenerate_button_bounds(content);
        let reveal = nostr_reveal_button_bounds(content);
        let copy = nostr_copy_secret_button_bounds(content);

        assert!(regenerate.max_x() < reveal.min_x());
        assert!(reveal.max_x() < copy.min_x());
        assert!(regenerate.size.height > 0.0);
        assert!(reveal.size.height > 0.0);
        assert!(copy.size.height > 0.0);
    }

    #[test]
    fn chat_layout_has_non_overlapping_regions() {
        let content = Bounds::new(0.0, 0.0, 900.0, 500.0);
        let workspace = chat_workspace_rail_bounds(content);
        let channel = chat_thread_rail_bounds(content);
        let transcript = chat_transcript_bounds(content);
        let transcript_body =
            chat_transcript_body_bounds_with_height(content, super::CHAT_COMPOSER_MIN_HEIGHT);
        let composer = chat_composer_input_bounds(content);
        let send = chat_send_button_bounds(content);

        assert!(content.contains(workspace.origin));
        assert!(content.contains(channel.origin));
        assert!(content.contains(transcript.origin));
        assert!(workspace.max_x() < channel.min_x());
        assert!(channel.max_x() < transcript.min_x());
        assert!(transcript.contains(transcript_body.origin));
        assert!(transcript_body.max_x() <= transcript.max_x());
        assert!(transcript_body.max_y() <= transcript.max_y());
        assert!(transcript.max_x() <= content.max_x());
        assert!(transcript.max_y() <= content.max_y());
        assert!(transcript.max_y() < composer.min_y());
        assert!(composer.max_x() < send.min_x());
    }

    #[test]
    fn go_online_toggle_bounds_are_inside_content() {
        let content = Bounds::new(10.0, 20.0, 560.0, 300.0);
        let toggle = go_online_toggle_button_bounds(content);
        assert!(content.contains(toggle.origin));
        assert!(toggle.max_x() <= content.max_x());
        assert!(toggle.max_y() <= content.max_y());
    }

    #[test]
    fn codex_controls_are_ordered() {
        let content = Bounds::new(0.0, 0.0, 920.0, 420.0);
        let account_refresh = codex_account_refresh_button_bounds(content);
        let account_login = codex_account_login_button_bounds(content);
        let account_cancel = codex_account_cancel_login_button_bounds(content);
        let account_logout = codex_account_logout_button_bounds(content);
        let account_limits = codex_account_rate_limits_button_bounds(content);
        assert!(account_refresh.max_x() < account_login.min_x());
        assert!(account_login.max_x() < account_cancel.min_x());
        assert!(account_refresh.max_y() < account_logout.min_y());
        assert!(account_logout.max_x() < account_limits.min_x());

        let models_refresh = codex_models_refresh_button_bounds(content);
        let models_toggle = codex_models_toggle_hidden_button_bounds(content);
        assert!(models_refresh.max_x() < models_toggle.min_x());

        let config_read = codex_config_read_button_bounds(content);
        let config_requirements = codex_config_requirements_button_bounds(content);
        let config_write = codex_config_write_button_bounds(content);
        let config_batch = codex_config_batch_write_button_bounds(content);
        let config_detect = codex_config_detect_external_button_bounds(content);
        let config_import = codex_config_import_external_button_bounds(content);
        assert!(config_read.max_x() < config_requirements.min_x());
        assert!(config_requirements.max_x() < config_write.min_x());
        assert!(config_read.max_y() < config_batch.min_y());
        assert!(config_batch.max_x() < config_detect.min_x());
        assert!(config_detect.max_x() < config_import.min_x());

        let mcp_refresh = codex_mcp_refresh_button_bounds(content);
        let mcp_login = codex_mcp_login_button_bounds(content);
        let mcp_reload = codex_mcp_reload_button_bounds(content);
        assert!(mcp_refresh.max_x() < mcp_login.min_x());
        assert!(mcp_login.max_x() < mcp_reload.min_x());

        let apps_refresh = codex_apps_refresh_button_bounds(content);
        assert!(apps_refresh.size.width > 0.0);

        let review_inline = codex_labs_review_inline_button_bounds(content);
        let review_detached = codex_labs_review_detached_button_bounds(content);
        let command_exec = codex_labs_command_exec_button_bounds(content);
        assert!(review_inline.max_x() < review_detached.min_x());
        assert!(review_detached.max_x() < command_exec.min_x());

        let collab = codex_labs_collaboration_modes_button_bounds(content);
        let features = codex_labs_experimental_features_button_bounds(content);
        let toggle = codex_labs_toggle_experimental_button_bounds(content);
        assert!(collab.max_x() < features.min_x());
        assert!(features.max_x() < toggle.min_x());

        let diagnostics_enable = codex_diagnostics_enable_wire_log_button_bounds(content);
        let diagnostics_disable = codex_diagnostics_disable_wire_log_button_bounds(content);
        let diagnostics_clear = codex_diagnostics_clear_events_button_bounds(content);
        assert!(diagnostics_enable.max_x() < diagnostics_disable.min_x());
        assert!(diagnostics_disable.max_x() < diagnostics_clear.min_x());
    }

    #[test]
    fn sync_health_rebootstrap_button_is_inside_content() {
        let content = Bounds::new(0.0, 0.0, 760.0, 360.0);
        let button = sync_health_rebootstrap_button_bounds(content);
        assert!(content.contains(button.origin));
        assert!(button.max_x() <= content.max_x());
        assert!(button.max_y() <= content.max_y());
    }

    #[test]
    fn relay_connections_controls_and_rows_are_ordered() {
        let content = Bounds::new(0.0, 0.0, 900.0, 420.0);
        let input = relay_connections_url_input_bounds(content);
        let add = relay_connections_add_button_bounds(content);
        let remove = relay_connections_remove_button_bounds(content);
        let retry = relay_connections_retry_button_bounds(content);
        let row0 = relay_connections_row_bounds(content, 0);

        assert!(input.max_x() < add.min_x());
        assert!(add.max_x() < remove.min_x());
        assert!(remove.max_x() < retry.min_x());
        assert!(retry.max_y() < row0.min_y());
    }

    #[test]
    fn network_requests_controls_are_ordered() {
        let content = Bounds::new(0.0, 0.0, 900.0, 420.0);
        let request_type = network_requests_type_input_bounds(content);
        let payload = network_requests_payload_input_bounds(content);
        let scope = network_requests_skill_scope_input_bounds(content);
        let envelope = network_requests_credit_envelope_input_bounds(content);
        let budget = network_requests_budget_input_bounds(content);
        let timeout = network_requests_timeout_input_bounds(content);
        let max_price = network_requests_max_price_input_bounds(content);
        let submit = network_requests_submit_button_bounds(content);

        assert!(request_type.max_y() < payload.min_y());
        assert!(payload.max_y() < scope.min_y());
        assert!(scope.max_y() < envelope.min_y());
        assert!(envelope.max_y() < budget.min_y());
        assert!(budget.max_x() < timeout.min_x());
        assert!(timeout.max_x() < max_price.min_x());
        assert!(max_price.max_x() < submit.min_x());
    }

    #[test]
    fn local_inference_controls_are_ordered() {
        let content = Bounds::new(0.0, 0.0, 940.0, 520.0);
        let refresh = local_inference_refresh_button_bounds(content);
        let warm = local_inference_warm_button_bounds(content);
        let unload = local_inference_unload_button_bounds(content);
        let run = local_inference_run_button_bounds(content);
        let prompt = local_inference_prompt_input_bounds(content);
        let model = local_inference_requested_model_input_bounds(content);
        let max_tokens = local_inference_max_tokens_input_bounds(content);
        let temperature = local_inference_temperature_input_bounds(content);
        let top_k = local_inference_top_k_input_bounds(content);
        let top_p = local_inference_top_p_input_bounds(content);

        assert!(refresh.max_x() < warm.min_x());
        assert!(warm.max_x() < unload.min_x());
        assert!(unload.max_x() < run.min_x());
        assert!(refresh.max_y() < prompt.min_y());
        assert!(prompt.max_y() < model.min_y());
        assert!(model.max_x() < max_tokens.min_x());
        assert!(max_tokens.max_x() < temperature.min_x());
        assert!(temperature.max_x() < top_k.min_x());
        assert!(top_k.max_x() < top_p.min_x());
    }

    #[test]
    fn starter_jobs_controls_and_rows_are_ordered() {
        let content = Bounds::new(0.0, 0.0, 860.0, 420.0);
        let complete = starter_jobs_complete_button_bounds(content);
        let kill_switch = starter_jobs_kill_switch_button_bounds(content);
        let row0 = starter_jobs_row_bounds(content, 0);
        let row1 = starter_jobs_row_bounds(content, 1);

        assert!(complete.max_x() < kill_switch.min_x());
        assert!(complete.max_y() < row0.min_y());
        assert!(row0.max_y() < row1.min_y());
    }

    #[test]
    fn reciprocal_loop_controls_are_ordered() {
        let content = Bounds::new(0.0, 0.0, 860.0, 440.0);
        let start = reciprocal_loop_start_button_bounds(content);
        let stop = reciprocal_loop_stop_button_bounds(content);
        let reset = reciprocal_loop_reset_button_bounds(content);

        assert!(start.max_x() < stop.min_x());
        assert!(stop.max_x() < reset.min_x());
        assert!(start.origin.y == stop.origin.y && stop.origin.y == reset.origin.y);
    }

    #[test]
    fn activity_feed_controls_and_rows_are_ordered() {
        let content = Bounds::new(0.0, 0.0, 940.0, 460.0);
        let refresh = activity_feed_refresh_button_bounds(content);
        let prev = activity_feed_prev_page_button_bounds(content);
        let next = activity_feed_next_page_button_bounds(content);
        let filter0 = activity_feed_filter_button_bounds(content, 0);
        let filter1 = activity_feed_filter_button_bounds(content, 1);
        let row0 = activity_feed_row_bounds(content, 0);
        let details =
            activity_feed_details_bounds(content, 8).expect("details bounds should exist");
        let detail_viewport =
            activity_feed_detail_viewport_bounds(content, 8).expect("detail viewport should exist");

        assert!(refresh.max_x() < prev.min_x());
        assert!(prev.max_x() < next.min_x());
        assert!(refresh.max_y() < filter0.min_y());
        assert!(filter0.max_x() < filter1.max_x());
        assert!(filter0.max_y() < row0.min_y());
        assert!(row0.max_y() < details.max_y());
        assert!(details.contains(detail_viewport.origin));
        assert!(detail_viewport.max_y() <= details.max_y());
    }

    #[test]
    fn alerts_recovery_controls_and_rows_are_ordered() {
        let content = Bounds::new(0.0, 0.0, 900.0, 460.0);
        let recover = alerts_recovery_recover_button_bounds(content);
        let ack = alerts_recovery_ack_button_bounds(content);
        let resolve = alerts_recovery_resolve_button_bounds(content);
        let row0 = alerts_recovery_row_bounds(content, 0);

        assert!(recover.max_x() < ack.max_x());
        assert!(ack.max_x() < resolve.max_x());
        assert!(recover.max_y() < row0.min_y());
    }

    #[test]
    fn settings_controls_are_ordered() {
        let content = Bounds::new(0.0, 0.0, 860.0, 420.0);
        let relay = settings_relay_input_bounds(content);
        let wallet = settings_wallet_default_input_bounds(content);
        let provider = settings_provider_queue_input_bounds(content);
        let save = settings_save_button_bounds(content);
        let reset = settings_reset_button_bounds(content);

        assert!(relay.max_y() < wallet.min_y());
        assert!(wallet.max_x() < provider.min_x());
        assert!(wallet.max_y() < save.min_y());
        assert!(save.max_x() < reset.min_x());
    }

    #[test]
    fn job_inbox_controls_and_rows_are_ordered() {
        let content = Bounds::new(0.0, 0.0, 860.0, 420.0);
        let accept = job_inbox_accept_button_bounds(content);
        let reject = job_inbox_reject_button_bounds(content);
        let row0 = job_inbox_row_bounds(content, 0);
        let row1 = job_inbox_row_bounds(content, 1);

        assert!(accept.max_x() < reject.min_x());
        assert!(accept.max_y() < row0.min_y());
        assert!(row0.max_y() < row1.min_y());
    }

    #[test]
    fn active_job_controls_are_non_overlapping() {
        let content = Bounds::new(0.0, 0.0, 860.0, 420.0);
        let advance = active_job_advance_button_bounds(content);
        let abort = active_job_abort_button_bounds(content);

        assert!(advance.max_x() < abort.min_x());
        assert!((advance.origin.y - abort.origin.y).abs() <= f32::EPSILON);
    }

    #[test]
    fn job_history_controls_have_stable_layout() {
        let content = Bounds::new(0.0, 0.0, 900.0, 460.0);
        let search = job_history_search_input_bounds(content);
        let status = job_history_status_button_bounds(content);
        let time = job_history_time_button_bounds(content);
        let prev = job_history_prev_page_button_bounds(content);
        let next = job_history_next_page_button_bounds(content);

        assert!(search.max_x() < status.min_x());
        assert!(status.max_x() < time.min_x());
        assert!(prev.max_x() < next.min_x());
        assert!(prev.origin.y > search.origin.y);
    }

    #[test]
    fn agent_profile_and_schedule_controls_are_ordered() {
        let content = Bounds::new(0.0, 0.0, 900.0, 440.0);
        let profile = agent_profile_publish_profile_button_bounds(content);
        let state = agent_profile_publish_state_button_bounds(content);
        let goals = agent_profile_update_goals_button_bounds(content);
        let create_goal = agent_profile_create_goal_button_bounds(content);
        let start_goal = agent_profile_start_goal_button_bounds(content);
        let abort_goal = agent_profile_abort_goal_button_bounds(content);
        let inspect_receipt = agent_profile_receipt_button_bounds(content);
        assert!(profile.max_x() < state.min_x());
        assert!(state.max_x() < goals.min_x());
        assert!(profile.max_y() < create_goal.min_y());
        assert!(create_goal.max_x() < start_goal.min_x());
        assert!(start_goal.max_x() < abort_goal.min_x());
        assert!(abort_goal.max_x() < inspect_receipt.min_x());

        let apply = agent_schedule_apply_button_bounds(content);
        let tick = agent_schedule_manual_tick_button_bounds(content);
        let inspect = agent_schedule_inspect_button_bounds(content);
        let toggle_os = agent_schedule_toggle_os_scheduler_button_bounds(content);
        assert!(apply.max_x() < tick.min_x());
        assert!(tick.max_x() < inspect.min_x());
        assert!(inspect.max_x() < toggle_os.min_x());
    }

    #[test]
    fn trajectory_skill_and_credit_controls_are_ordered() {
        let content = Bounds::new(0.0, 0.0, 960.0, 480.0);

        let open = trajectory_open_session_button_bounds(content);
        let filter = trajectory_filter_button_bounds(content);
        let verify = trajectory_verify_button_bounds(content);
        assert!(open.max_x() < filter.min_x());
        assert!(filter.max_x() < verify.min_x());

        let discover = skill_registry_discover_button_bounds(content);
        let inspect = skill_registry_inspect_button_bounds(content);
        let install = skill_registry_install_button_bounds(content);
        assert!(discover.max_x() < inspect.min_x());
        assert!(inspect.max_x() < install.min_x());

        let refresh = skill_trust_refresh_button_bounds(content);
        let attest = skill_trust_attestations_button_bounds(content);
        let kill = skill_trust_kill_switch_button_bounds(content);
        let revoke = skill_trust_revoke_button_bounds(content);
        assert!(refresh.max_x() < attest.min_x());
        assert!(attest.max_x() < kill.min_x());
        assert!(kill.max_x() < revoke.min_x());

        let intent = credit_desk_intent_button_bounds(content);
        let offer = credit_desk_offer_button_bounds(content);
        let envelope = credit_desk_envelope_button_bounds(content);
        let spend = credit_desk_spend_button_bounds(content);
        assert!(intent.max_x() < offer.min_x());
        assert!(offer.max_x() < envelope.min_x());
        assert!(envelope.max_x() < spend.min_x());

        let settle = credit_settlement_verify_button_bounds(content);
        let default_notice = credit_settlement_default_button_bounds(content);
        let reputation = credit_settlement_reputation_button_bounds(content);
        assert!(settle.max_x() < default_notice.min_x());
        assert!(default_notice.max_x() < reputation.min_x());
    }

    #[test]
    fn cad_demo_controls_are_ordered_and_inside_content() {
        let content = Bounds::new(0.0, 0.0, 820.0, 360.0);
        let cycle = cad_demo_cycle_variant_button_bounds(content);
        let jaw = cad_demo_gripper_jaw_button_bounds(content);
        let reset = cad_demo_reset_button_bounds(content);
        let hidden_line = cad_demo_hidden_line_mode_button_bounds(content);
        let reset_camera = cad_demo_reset_camera_button_bounds(content);
        let projection = cad_demo_projection_mode_button_bounds(content);
        let viewport_layout = cad_demo_viewport_layout_button_bounds(content);
        let drawing_mode = cad_demo_drawing_mode_button_bounds(content);
        let drawing_direction = cad_demo_drawing_direction_button_bounds(content);
        let drawing_hidden = cad_demo_drawing_hidden_lines_button_bounds(content);
        let drawing_dimensions = cad_demo_drawing_dimensions_button_bounds(content);
        let drawing_reset = cad_demo_drawing_reset_view_button_bounds(content);
        let drawing_add_detail = cad_demo_drawing_add_detail_button_bounds(content);
        let drawing_clear_details = cad_demo_drawing_clear_details_button_bounds(content);
        let snap_grid = cad_demo_snap_grid_button_bounds(content);
        let snap_origin = cad_demo_snap_origin_button_bounds(content);
        let snap_endpoint = cad_demo_snap_endpoint_button_bounds(content);
        let snap_midpoint = cad_demo_snap_midpoint_button_bounds(content);
        let hotkeys = cad_demo_hotkey_profile_button_bounds(content);
        let section_plane = cad_demo_section_plane_button_bounds(content);
        let section_offset = cad_demo_section_offset_button_bounds(content);
        let material = cad_demo_material_button_bounds(content);
        let sensor_mode = cad_demo_sensor_mode_button_bounds(content);
        assert!(content.contains(cycle.origin));
        assert!(content.contains(jaw.origin));
        assert!(content.contains(reset.origin));
        assert!(content.contains(hidden_line.origin));
        assert!(content.contains(reset_camera.origin));
        assert!(content.contains(projection.origin));
        assert!(content.contains(viewport_layout.origin));
        assert!(content.contains(drawing_mode.origin));
        assert!(content.contains(drawing_direction.origin));
        assert!(content.contains(drawing_hidden.origin));
        assert!(content.contains(drawing_dimensions.origin));
        assert!(content.contains(drawing_reset.origin));
        assert!(content.contains(drawing_add_detail.origin));
        assert!(content.contains(drawing_clear_details.origin));
        assert!(content.contains(snap_grid.origin));
        assert!(content.contains(snap_origin.origin));
        assert!(content.contains(snap_endpoint.origin));
        assert!(content.contains(snap_midpoint.origin));
        assert!(content.contains(hotkeys.origin));
        assert!(content.contains(section_plane.origin));
        assert!(content.contains(section_offset.origin));
        assert!(content.contains(material.origin));
        assert!(content.contains(sensor_mode.origin));
        assert!(cycle.max_y() <= content.max_y());
        assert!(jaw.max_y() <= content.max_y());
        assert!(reset.max_y() <= content.max_y());
        assert!(hidden_line.max_y() <= content.max_y());
        assert!(reset_camera.max_y() <= content.max_y());
        assert!(projection.max_y() <= content.max_y());
        assert!(viewport_layout.max_y() <= content.max_y());
        assert!(drawing_mode.max_y() <= content.max_y());
        assert!(drawing_direction.max_y() <= content.max_y());
        assert!(drawing_hidden.max_y() <= content.max_y());
        assert!(drawing_dimensions.max_y() <= content.max_y());
        assert!(drawing_reset.max_y() <= content.max_y());
        assert!(drawing_add_detail.max_y() <= content.max_y());
        assert!(drawing_clear_details.max_y() <= content.max_y());
        assert!(snap_grid.max_y() <= content.max_y());
        assert!(snap_origin.max_y() <= content.max_y());
        assert!(snap_endpoint.max_y() <= content.max_y());
        assert!(snap_midpoint.max_y() <= content.max_y());
        assert!(hotkeys.max_y() <= content.max_y());
        assert!(section_plane.max_y() <= content.max_y());
        assert!(section_offset.max_y() <= content.max_y());
        assert!(material.max_y() <= content.max_y());
        assert!(sensor_mode.max_y() <= content.max_y());
        assert!(cycle.max_x() < jaw.min_x());
        assert!(jaw.max_x() < reset.min_x());
        assert!(reset.max_x() <= hidden_line.min_x() + 0.001);
        assert!(hidden_line.max_x() <= reset_camera.min_x() + 0.001);
        assert!(reset_camera.max_x() <= projection.min_x() + 0.001);
        assert!(projection.max_x() <= viewport_layout.min_x() + 0.001);
        assert!(drawing_mode.origin.y >= projection.max_y() - 0.001);
        assert!(drawing_mode.max_x() <= drawing_direction.min_x() + 0.001);
        assert!(drawing_direction.max_x() <= drawing_hidden.min_x() + 0.001);
        assert!(drawing_hidden.max_x() <= drawing_dimensions.min_x() + 0.001);
        assert!(drawing_dimensions.max_x() <= drawing_reset.min_x() + 0.001);
        assert!(drawing_reset.max_x() <= drawing_add_detail.min_x() + 0.001);
        assert!(drawing_add_detail.max_x() <= drawing_clear_details.min_x() + 0.001);
        assert!(snap_grid.origin.y >= drawing_mode.max_y() - 0.001);
        assert!(snap_grid.max_y() <= snap_origin.max_y() + 0.001);
        assert!(snap_grid.max_x() <= snap_origin.min_x() + 0.001);
        assert!(snap_origin.max_x() <= snap_endpoint.min_x() + 0.001);
        assert!(snap_endpoint.max_x() <= snap_midpoint.min_x() + 0.001);
        assert!(snap_midpoint.max_x() <= hotkeys.min_x() + 0.001);
        assert!(section_plane.origin.y >= hotkeys.max_y() - 0.001);
        assert!(section_plane.max_x() <= section_offset.min_x() + 0.001);
        assert!(section_offset.max_x() <= material.min_x() + 0.001);
        assert!(material.max_x() <= sensor_mode.min_x() + 0.001);
        assert!(viewport_layout.max_x() <= content.max_x() + 0.001);
    }

    #[test]
    fn cad_view_cube_buttons_stay_inside_content_and_non_overlapping() {
        let content = Bounds::new(0.0, 0.0, 860.0, 420.0);
        let cube = cad_demo_view_cube_bounds(content);
        let top = cad_demo_view_snap_top_button_bounds(content);
        let front = cad_demo_view_snap_front_button_bounds(content);
        let right = cad_demo_view_snap_right_button_bounds(content);
        let iso = cad_demo_view_snap_iso_button_bounds(content);

        assert!(content.contains(cube.origin));
        assert!(cube.max_x() <= content.max_x() + 0.001);
        assert!(cube.max_y() <= content.max_y() + 0.001);
        assert!(cube.contains(top.origin));
        assert!(cube.contains(front.origin));
        assert!(cube.contains(right.origin));
        assert!(cube.contains(iso.origin));
        assert!(top.max_x() <= front.min_x() + 0.001);
        assert!(top.max_y() <= right.min_y() + 0.001);
        assert!(right.max_x() <= iso.min_x() + 0.001);
        assert!(front.max_y() <= iso.min_y() + 0.001);
    }

    #[test]
    fn cad_context_menu_bounds_and_rows_stay_within_content() {
        let content = Bounds::new(0.0, 0.0, 860.0, 420.0);
        let menu = cad_demo_context_menu_bounds(content, Point::new(850.0, 410.0), 3);
        assert!(content.contains(menu.origin));
        assert!(menu.max_x() <= content.max_x() + 0.001);
        assert!(menu.max_y() <= content.max_y() + 0.001);

        for index in 0..3 {
            let row = cad_demo_context_menu_row_bounds(menu, index);
            assert!(row.origin.x >= menu.origin.x);
            assert!(row.max_x() <= menu.max_x() + 0.001);
            assert!(row.origin.y >= menu.origin.y);
            assert!(row.max_y() <= menu.max_y() + 0.001);
        }
    }

    #[test]
    fn cad_warning_panel_and_markers_stay_within_content_no_overflow() {
        let content = Bounds::new(0.0, 0.0, 840.0, 420.0);
        let panel = cad_demo_warning_panel_bounds(content);
        let severity = cad_demo_warning_filter_severity_button_bounds(content);
        let code = cad_demo_warning_filter_code_button_bounds(content);
        assert!(content.contains(panel.origin));
        assert!(panel.max_x() <= content.max_x());
        assert!(panel.max_y() <= content.max_y());
        assert!(panel.contains(severity.origin));
        assert!(panel.contains(code.origin));
        assert!(severity.max_x() <= panel.max_x());
        assert!(code.max_x() <= panel.max_x());

        for index in 0..8 {
            let row = cad_demo_warning_row_bounds(content, index);
            let marker = cad_demo_warning_marker_bounds(content, index);
            assert!(row.origin.x >= panel.origin.x);
            assert!(row.max_x() <= panel.max_x() + 0.001);
            assert!(row.max_y() <= panel.max_y() + 0.001);
            assert!(marker.origin.x >= content.origin.x);
            assert!(marker.max_x() <= content.max_x() + 0.001);
            assert!(marker.origin.y >= content.origin.y);
            assert!(marker.max_y() <= content.max_y() + 0.001);
        }
    }

    #[test]
    fn cad_timeline_panel_rows_stay_within_content_in_small_panes() {
        let content = Bounds::new(0.0, 0.0, 360.0, 220.0);
        let panel = cad_demo_timeline_panel_bounds(content);
        assert!(content.contains(panel.origin));
        assert!(panel.max_x() <= content.max_x() + 0.001);
        assert!(panel.max_y() <= content.max_y() + 0.001);
        for index in 0..12 {
            let row = cad_demo_timeline_row_bounds(content, index);
            assert!(row.origin.x >= panel.origin.x);
            assert!(row.max_x() <= panel.max_x() + 0.001);
            assert!(row.max_y() <= panel.max_y() + 0.001);
        }
    }

    #[test]
    fn cad_dimension_panel_rows_stay_within_content() {
        let content = Bounds::new(0.0, 0.0, 840.0, 420.0);
        let panel = cad_demo_dimension_panel_bounds(content);
        assert!(content.contains(panel.origin));
        assert!(panel.max_x() <= content.max_x() + 0.001);
        assert!(panel.max_y() <= content.max_y() + 0.001);
        for index in 0..4 {
            let row = cad_demo_dimension_row_bounds(content, index);
            assert!(row.origin.x >= panel.origin.x);
            assert!(row.max_x() <= panel.max_x() + 0.001);
            assert!(row.origin.y >= panel.origin.y);
            assert!(row.max_y() <= panel.max_y() + 0.001);
        }
    }

    #[test]
    fn earnings_refresh_button_is_inside_content() {
        let content = Bounds::new(0.0, 0.0, 640.0, 320.0);
        let button = earnings_scoreboard_refresh_button_bounds(content);
        assert!(content.contains(button.origin));
        assert!(button.max_x() <= content.max_x());
        assert!(button.max_y() <= content.max_y());
    }
}
