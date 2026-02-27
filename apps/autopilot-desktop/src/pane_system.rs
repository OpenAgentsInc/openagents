use std::sync::atomic::{AtomicU64, Ordering};

use wgpui::components::hud::{PaneFrame, ResizeEdge};
use wgpui::{Bounds, Component, InputEvent, Modifiers, MouseButton, Point, Size};
use winit::window::CursorIcon;

use crate::app_state::{ActivityFeedFilter, DesktopPane, PaneDragMode, PaneKind, RenderState};
use crate::hotbar::{HOTBAR_FLOAT_GAP, HOTBAR_HEIGHT};
use crate::pane_registry::pane_spec;
use crate::panes::{
    chat as chat_pane, relay_connections as relay_connections_pane, wallet as wallet_pane,
};
use crate::render::logical_size;
use crate::spark_pane::{self, CreateInvoicePaneAction, PayInvoicePaneAction, SparkPaneAction};

pub const PANE_TITLE_HEIGHT: f32 = 28.0;
pub const PANE_MIN_WIDTH: f32 = 220.0;
pub const PANE_MIN_HEIGHT: f32 = 140.0;
const PANE_MARGIN: f32 = 18.0;
const PANE_CASCADE_X: f32 = 26.0;
const PANE_CASCADE_Y: f32 = 22.0;
const PANE_BOTTOM_RESERVED: f32 = HOTBAR_HEIGHT + HOTBAR_FLOAT_GAP + PANE_MARGIN;
const CHAT_PAD: f32 = 12.0;
const CHAT_THREAD_RAIL_WIDTH: f32 = 170.0;
const CHAT_COMPOSER_HEIGHT: f32 = 30.0;
const CHAT_SEND_WIDTH: f32 = 92.0;
const CHAT_HEADER_BUTTON_HEIGHT: f32 = 26.0;
const CHAT_HEADER_BUTTON_WIDTH: f32 = 110.0;
const CHAT_THREAD_FILTER_BUTTON_HEIGHT: f32 = 22.0;
const CHAT_THREAD_FILTER_BUTTON_WIDTH: f32 = 72.0;
const CHAT_THREAD_ACTION_BUTTON_HEIGHT: f32 = 22.0;
const CHAT_THREAD_ACTION_BUTTON_WIDTH: f32 = 72.0;
const CHAT_THREAD_ACTION_BUTTON_GAP: f32 = 4.0;
const CHAT_THREAD_ROW_HEIGHT: f32 = 24.0;
const CHAT_THREAD_ROW_GAP: f32 = 4.0;
const CHAT_MAX_THREAD_ROWS: usize = 10;
const SKILL_REGISTRY_ROW_HEIGHT: f32 = 28.0;
const SKILL_REGISTRY_ROW_GAP: f32 = 6.0;
const SKILL_REGISTRY_MAX_ROWS: usize = 8;
const CODEX_MCP_ROW_HEIGHT: f32 = 30.0;
const CODEX_MCP_ROW_GAP: f32 = 6.0;
const CODEX_MCP_MAX_ROWS: usize = 8;
const CODEX_APPS_ROW_HEIGHT: f32 = 30.0;
const CODEX_APPS_ROW_GAP: f32 = 6.0;
const CODEX_APPS_MAX_ROWS: usize = 8;
const CODEX_REMOTE_SKILLS_ROW_HEIGHT: f32 = 30.0;
const CODEX_REMOTE_SKILLS_ROW_GAP: f32 = 6.0;
const CODEX_REMOTE_SKILLS_MAX_ROWS: usize = 8;
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
const ALERTS_RECOVERY_ROW_HEIGHT: f32 = 30.0;
const ALERTS_RECOVERY_ROW_GAP: f32 = 6.0;
const ALERTS_RECOVERY_MAX_ROWS: usize = 8;
static PANE_Z_SORT_INVOCATIONS: AtomicU64 = AtomicU64::new(0);

pub struct PaneController;

pub struct PaneInput;

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
pub enum CodexRemoteSkillsPaneAction {
    Refresh,
    ExportSelected,
    SelectRow(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EarningsScoreboardPaneAction {
    Refresh,
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
pub enum NetworkRequestsPaneAction {
    SubmitRequest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StarterJobsPaneAction {
    CompleteSelected,
    SelectRow(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivityFeedPaneAction {
    Refresh,
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
pub enum AgentProfileStatePaneAction {
    PublishProfile,
    PublishState,
    UpdateGoals,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentScheduleTickPaneAction {
    ApplySchedule,
    PublishManualTick,
    InspectLastResult,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrajectoryAuditPaneAction {
    OpenSession,
    CycleStepFilter,
    VerifyTrajectoryHash,
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
pub enum AgentNetworkSimulationPaneAction {
    RunRound,
    Reset,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TreasuryExchangeSimulationPaneAction {
    RunRound,
    Reset,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelaySecuritySimulationPaneAction {
    RunRound,
    Reset,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaneHitAction {
    NostrRegenerate,
    NostrReveal,
    NostrCopySecret,
    ChatSend,
    ChatRefreshThreads,
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
    ChatSelectThread(usize),
    GoOnlineToggle,
    CodexAccount(CodexAccountPaneAction),
    CodexModels(CodexModelsPaneAction),
    CodexConfig(CodexConfigPaneAction),
    CodexMcp(CodexMcpPaneAction),
    CodexApps(CodexAppsPaneAction),
    CodexRemoteSkills(CodexRemoteSkillsPaneAction),
    EarningsScoreboard(EarningsScoreboardPaneAction),
    RelayConnections(RelayConnectionsPaneAction),
    SyncHealth(SyncHealthPaneAction),
    NetworkRequests(NetworkRequestsPaneAction),
    StarterJobs(StarterJobsPaneAction),
    ActivityFeed(ActivityFeedPaneAction),
    AlertsRecovery(AlertsRecoveryPaneAction),
    Settings(SettingsPaneAction),
    JobInbox(JobInboxPaneAction),
    ActiveJob(ActiveJobPaneAction),
    JobHistory(JobHistoryPaneAction),
    AgentProfileState(AgentProfileStatePaneAction),
    AgentScheduleTick(AgentScheduleTickPaneAction),
    TrajectoryAudit(TrajectoryAuditPaneAction),
    SkillRegistry(SkillRegistryPaneAction),
    SkillTrustRevocation(SkillTrustRevocationPaneAction),
    CreditDesk(CreditDeskPaneAction),
    CreditSettlementLedger(CreditSettlementLedgerPaneAction),
    AgentNetworkSimulation(AgentNetworkSimulationPaneAction),
    TreasuryExchangeSimulation(TreasuryExchangeSimulationPaneAction),
    RelaySecuritySimulation(RelaySecuritySimulationPaneAction),
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
    let tier = (id as usize - 1) % 10;
    let x = PANE_MARGIN + tier as f32 * PANE_CASCADE_X;
    let y = PANE_MARGIN + tier as f32 * PANE_CASCADE_Y;
    let bounds = clamp_bounds_to_window(
        Bounds::new(x, y, descriptor.width, descriptor.height),
        logical,
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
        Self::create(state, PaneDescriptor::for_kind(kind))
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

    match mode {
        PaneDragMode::Moving {
            pane_id,
            start_mouse,
            start_bounds,
        } => {
            let dx = current_mouse.x - start_mouse.x;
            let dy = current_mouse.y - start_mouse.y;

            if let Some(pane) = state.panes.iter_mut().find(|pane| pane.id == pane_id) {
                let next = Bounds::new(
                    start_bounds.origin.x + dx,
                    start_bounds.origin.y + dy,
                    start_bounds.size.width,
                    start_bounds.size.height,
                );
                pane.bounds = clamp_bounds_to_window(next, logical);
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
                let next = state.pane_resizer.resize_bounds(
                    edge,
                    start_bounds,
                    start_mouse,
                    current_mouse,
                );
                pane.bounds = clamp_bounds_to_window(next, logical);
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

pub fn cursor_icon_for_pointer(state: &RenderState, point: Point) -> CursorIcon {
    if let Some(mode) = state.pane_drag_mode {
        return match mode {
            PaneDragMode::Moving { .. } => CursorIcon::Move,
            PaneDragMode::Resizing { edge, .. } => cursor_icon_for_resize_edge(edge),
        };
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
                if chat_composer_input_bounds(content_bounds).contains(point) {
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
            | PaneKind::CodexAccount
            | PaneKind::CodexModels
            | PaneKind::CodexConfig
            | PaneKind::CodexMcp
            | PaneKind::CodexApps
            | PaneKind::CodexRemoteSkills
            | PaneKind::GoOnline
            | PaneKind::ProviderStatus
            | PaneKind::EarningsScoreboard
            | PaneKind::SyncHealth
            | PaneKind::StarterJobs
            | PaneKind::ActivityFeed
            | PaneKind::AlertsRecovery
            | PaneKind::NostrIdentity
            | PaneKind::JobInbox
            | PaneKind::ActiveJob
            | PaneKind::AgentProfileState
            | PaneKind::AgentScheduleTick
            | PaneKind::TrajectoryAudit
            | PaneKind::SkillRegistry
            | PaneKind::SkillTrustRevocation
            | PaneKind::CreditDesk
            | PaneKind::CreditSettlementLedger
            | PaneKind::AgentNetworkSimulation
            | PaneKind::TreasuryExchangeSimulation
            | PaneKind::RelaySecuritySimulation => {}
        }

        if pane_hit_action_for_pane(state, pane, point).is_some() {
            return CursorIcon::Pointer;
        }

        return CursorIcon::Default;
    }

    CursorIcon::Default
}

pub fn pane_content_bounds(bounds: Bounds) -> Bounds {
    Bounds::new(
        bounds.origin.x,
        bounds.origin.y + PANE_TITLE_HEIGHT,
        bounds.size.width,
        (bounds.size.height - PANE_TITLE_HEIGHT).max(0.0),
    )
}

pub fn chat_thread_rail_bounds(content_bounds: Bounds) -> Bounds {
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        content_bounds.origin.y + CHAT_PAD,
        CHAT_THREAD_RAIL_WIDTH,
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
    let actions_bottom = chat_thread_action_unsubscribe_button_bounds(content_bounds).max_y();
    let y = actions_bottom + 10.0 + index as f32 * (CHAT_THREAD_ROW_HEIGHT + CHAT_THREAD_ROW_GAP);
    Bounds::new(
        rail.origin.x + 10.0,
        y,
        (rail.size.width - 20.0).max(80.0),
        CHAT_THREAD_ROW_HEIGHT,
    )
}

pub fn chat_visible_thread_row_count(total_threads: usize) -> usize {
    total_threads.min(CHAT_MAX_THREAD_ROWS)
}

pub fn chat_thread_filter_archived_button_bounds(content_bounds: Bounds) -> Bounds {
    let refresh = chat_refresh_threads_button_bounds(content_bounds);
    Bounds::new(
        refresh.origin.x,
        refresh.max_y() + 8.0,
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

fn chat_thread_action_grid_bounds(content_bounds: Bounds, index: usize) -> Bounds {
    let source = chat_thread_filter_source_button_bounds(content_bounds);
    let row = index / 2;
    let col = index % 2;
    Bounds::new(
        source.origin.x
            + col as f32 * (CHAT_THREAD_ACTION_BUTTON_WIDTH + CHAT_THREAD_ACTION_BUTTON_GAP),
        source.max_y()
            + 8.0
            + row as f32 * (CHAT_THREAD_ACTION_BUTTON_HEIGHT + CHAT_THREAD_ACTION_BUTTON_GAP),
        CHAT_THREAD_ACTION_BUTTON_WIDTH,
        CHAT_THREAD_ACTION_BUTTON_HEIGHT,
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
    Bounds::new(
        content_bounds.max_x() - CHAT_PAD - CHAT_SEND_WIDTH,
        content_bounds.max_y() - CHAT_PAD - CHAT_COMPOSER_HEIGHT,
        CHAT_SEND_WIDTH,
        CHAT_COMPOSER_HEIGHT,
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

pub fn chat_composer_input_bounds(content_bounds: Bounds) -> Bounds {
    let rail_bounds = chat_thread_rail_bounds(content_bounds);
    let send_bounds = chat_send_button_bounds(content_bounds);
    Bounds::new(
        rail_bounds.max_x() + CHAT_PAD,
        send_bounds.origin.y,
        (send_bounds.origin.x - (rail_bounds.max_x() + CHAT_PAD) - CHAT_PAD).max(120.0),
        CHAT_COMPOSER_HEIGHT,
    )
}

pub fn chat_transcript_bounds(content_bounds: Bounds) -> Bounds {
    let rail_bounds = chat_thread_rail_bounds(content_bounds);
    let composer_bounds = chat_composer_input_bounds(content_bounds);
    Bounds::new(
        rail_bounds.max_x() + CHAT_PAD,
        content_bounds.origin.y + CHAT_PAD,
        (content_bounds.max_x() - (rail_bounds.max_x() + CHAT_PAD) - CHAT_PAD).max(220.0),
        (composer_bounds.origin.y - (content_bounds.origin.y + CHAT_PAD) - CHAT_PAD).max(120.0),
    )
}

pub fn go_online_toggle_button_bounds(content_bounds: Bounds) -> Bounds {
    let width = content_bounds.size.width.clamp(160.0, 220.0);
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        content_bounds.origin.y + CHAT_PAD,
        width,
        34.0,
    )
}

fn codex_action_button_bounds(
    content_bounds: Bounds,
    row: usize,
    col: usize,
    columns: usize,
) -> Bounds {
    let columns = columns.max(1);
    let gap = JOB_INBOX_BUTTON_GAP;
    let usable_width =
        (content_bounds.size.width - CHAT_PAD * 2.0 - gap * (columns as f32 - 1.0)).max(220.0);
    let width = (usable_width / columns as f32).clamp(120.0, 220.0);
    let x = content_bounds.origin.x + CHAT_PAD + col as f32 * (width + gap);
    let y = content_bounds.origin.y + CHAT_PAD + row as f32 * (JOB_INBOX_BUTTON_HEIGHT + gap);
    Bounds::new(x, y, width, JOB_INBOX_BUTTON_HEIGHT)
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

pub fn codex_remote_skills_refresh_button_bounds(content_bounds: Bounds) -> Bounds {
    codex_action_button_bounds(content_bounds, 0, 0, 2)
}

pub fn codex_remote_skills_export_button_bounds(content_bounds: Bounds) -> Bounds {
    codex_action_button_bounds(content_bounds, 0, 1, 2)
}

pub fn codex_remote_skills_row_bounds(content_bounds: Bounds, row_index: usize) -> Bounds {
    let safe_index = row_index.min(CODEX_REMOTE_SKILLS_MAX_ROWS.saturating_sub(1));
    let top = codex_remote_skills_refresh_button_bounds(content_bounds).max_y() + 12.0;
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        top + safe_index as f32 * (CODEX_REMOTE_SKILLS_ROW_HEIGHT + CODEX_REMOTE_SKILLS_ROW_GAP),
        (content_bounds.size.width - CHAT_PAD * 2.0).max(220.0),
        CODEX_REMOTE_SKILLS_ROW_HEIGHT,
    )
}

pub fn codex_remote_skills_visible_row_count(row_count: usize) -> usize {
    row_count.min(CODEX_REMOTE_SKILLS_MAX_ROWS)
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

pub fn network_requests_submit_button_bounds(content_bounds: Bounds) -> Bounds {
    let timeout = network_requests_timeout_input_bounds(content_bounds);
    Bounds::new(
        timeout.max_x() + JOB_INBOX_BUTTON_GAP,
        timeout.origin.y,
        (content_bounds.max_x() - timeout.max_x() - CHAT_PAD - JOB_INBOX_BUTTON_GAP).max(140.0),
        timeout.size.height,
    )
}

pub fn starter_jobs_complete_button_bounds(content_bounds: Bounds) -> Bounds {
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        content_bounds.origin.y + CHAT_PAD,
        (content_bounds.size.width * 0.28).clamp(160.0, 240.0),
        JOB_INBOX_BUTTON_HEIGHT,
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

pub fn agent_network_simulation_run_button_bounds(content_bounds: Bounds) -> Bounds {
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        content_bounds.origin.y + CHAT_PAD,
        (content_bounds.size.width * 0.28).clamp(180.0, 280.0),
        JOB_INBOX_BUTTON_HEIGHT,
    )
}

pub fn agent_network_simulation_reset_button_bounds(content_bounds: Bounds) -> Bounds {
    let run = agent_network_simulation_run_button_bounds(content_bounds);
    Bounds::new(
        run.max_x() + JOB_INBOX_BUTTON_GAP,
        run.origin.y,
        (content_bounds.size.width * 0.22).clamp(140.0, 220.0),
        run.size.height,
    )
}

pub fn treasury_exchange_simulation_run_button_bounds(content_bounds: Bounds) -> Bounds {
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        content_bounds.origin.y + CHAT_PAD,
        (content_bounds.size.width * 0.26).clamp(180.0, 280.0),
        JOB_INBOX_BUTTON_HEIGHT,
    )
}

pub fn treasury_exchange_simulation_reset_button_bounds(content_bounds: Bounds) -> Bounds {
    let run = treasury_exchange_simulation_run_button_bounds(content_bounds);
    Bounds::new(
        run.max_x() + JOB_INBOX_BUTTON_GAP,
        run.origin.y,
        (content_bounds.size.width * 0.22).clamp(140.0, 220.0),
        run.size.height,
    )
}

pub fn relay_security_simulation_run_button_bounds(content_bounds: Bounds) -> Bounds {
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        content_bounds.origin.y + CHAT_PAD,
        (content_bounds.size.width * 0.26).clamp(180.0, 280.0),
        JOB_INBOX_BUTTON_HEIGHT,
    )
}

pub fn relay_security_simulation_reset_button_bounds(content_bounds: Bounds) -> Bounds {
    let run = relay_security_simulation_run_button_bounds(content_bounds);
    Bounds::new(
        run.max_x() + JOB_INBOX_BUTTON_GAP,
        run.origin.y,
        (content_bounds.size.width * 0.22).clamp(140.0, 220.0),
        run.size.height,
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

fn nostr_button_bounds(content_bounds: Bounds) -> (Bounds, Bounds, Bounds) {
    let gap = 8.0;
    let button_width = ((content_bounds.size.width - 24.0 - gap * 2.0) / 3.0).clamp(92.0, 156.0);
    let start_x = content_bounds.origin.x + 12.0;
    let y = content_bounds.origin.y + 12.0;

    let regenerate_bounds = Bounds::new(start_x, y, button_width, 30.0);
    let reveal_bounds = Bounds::new(
        regenerate_bounds.origin.x + button_width + gap,
        y,
        button_width,
        30.0,
    );
    let copy_bounds = Bounds::new(
        reveal_bounds.origin.x + button_width + gap,
        y,
        button_width,
        30.0,
    );

    (regenerate_bounds, reveal_bounds, copy_bounds)
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
        PaneKind::AutopilotChat => {
            if chat_send_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::ChatSend);
            }
            if chat_refresh_threads_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::ChatRefreshThreads);
            }
            if chat_cycle_model_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::ChatCycleModel);
            }
            if chat_interrupt_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::ChatInterruptTurn);
            }
            if chat_thread_filter_archived_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::ChatToggleArchivedFilter);
            }
            if chat_thread_filter_sort_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::ChatCycleSortFilter);
            }
            if chat_thread_filter_source_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::ChatCycleSourceFilter);
            }
            if chat_thread_filter_provider_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::ChatCycleProviderFilter);
            }
            if chat_thread_action_fork_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::ChatForkThread);
            }
            if chat_thread_action_archive_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::ChatArchiveThread);
            }
            if chat_thread_action_unarchive_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::ChatUnarchiveThread);
            }
            if chat_thread_action_rename_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::ChatRenameThread);
            }
            if chat_thread_action_rollback_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::ChatRollbackThread);
            }
            if chat_thread_action_compact_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::ChatCompactThread);
            }
            if chat_thread_action_unsubscribe_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::ChatUnsubscribeThread);
            }
            if chat_server_request_accept_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::ChatRespondApprovalAccept);
            }
            if chat_server_request_session_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::ChatRespondApprovalAcceptSession);
            }
            if chat_server_request_decline_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::ChatRespondApprovalDecline);
            }
            if chat_server_request_cancel_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::ChatRespondApprovalCancel);
            }
            if chat_server_tool_call_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::ChatRespondToolCall);
            }
            if chat_server_user_input_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::ChatRespondToolUserInput);
            }
            if chat_server_auth_refresh_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::ChatRespondAuthRefresh);
            }
            let visible_threads = chat_visible_thread_row_count(state.autopilot_chat.threads.len());
            for row_index in 0..visible_threads {
                if chat_thread_row_bounds(content_bounds, row_index).contains(point) {
                    return Some(PaneHitAction::ChatSelectThread(row_index));
                }
            }
            None
        }
        PaneKind::GoOnline => {
            if go_online_toggle_button_bounds(content_bounds).contains(point) {
                Some(PaneHitAction::GoOnlineToggle)
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
        PaneKind::CodexRemoteSkills => {
            if codex_remote_skills_refresh_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CodexRemoteSkills(
                    CodexRemoteSkillsPaneAction::Refresh,
                ));
            }
            if codex_remote_skills_export_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::CodexRemoteSkills(
                    CodexRemoteSkillsPaneAction::ExportSelected,
                ));
            }
            let visible_rows =
                codex_remote_skills_visible_row_count(state.codex_remote_skills.skills.len());
            for row_index in 0..visible_rows {
                if codex_remote_skills_row_bounds(content_bounds, row_index).contains(point) {
                    return Some(PaneHitAction::CodexRemoteSkills(
                        CodexRemoteSkillsPaneAction::SelectRow(row_index),
                    ));
                }
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
        PaneKind::NetworkRequests => {
            if network_requests_submit_button_bounds(content_bounds).contains(point) {
                Some(PaneHitAction::NetworkRequests(
                    NetworkRequestsPaneAction::SubmitRequest,
                ))
            } else {
                None
            }
        }
        PaneKind::StarterJobs => {
            if starter_jobs_complete_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::StarterJobs(
                    StarterJobsPaneAction::CompleteSelected,
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
        PaneKind::ActivityFeed => {
            if activity_feed_refresh_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::ActivityFeed(ActivityFeedPaneAction::Refresh));
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
            if active_job_advance_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::ActiveJob(ActiveJobPaneAction::AdvanceStage));
            }
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
        PaneKind::AgentNetworkSimulation => {
            if agent_network_simulation_run_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::AgentNetworkSimulation(
                    AgentNetworkSimulationPaneAction::RunRound,
                ));
            }
            if agent_network_simulation_reset_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::AgentNetworkSimulation(
                    AgentNetworkSimulationPaneAction::Reset,
                ));
            }
            None
        }
        PaneKind::TreasuryExchangeSimulation => {
            if treasury_exchange_simulation_run_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::TreasuryExchangeSimulation(
                    TreasuryExchangeSimulationPaneAction::RunRound,
                ));
            }
            if treasury_exchange_simulation_reset_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::TreasuryExchangeSimulation(
                    TreasuryExchangeSimulationPaneAction::Reset,
                ));
            }
            None
        }
        PaneKind::RelaySecuritySimulation => {
            if relay_security_simulation_run_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::RelaySecuritySimulation(
                    RelaySecuritySimulationPaneAction::RunRound,
                ));
            }
            if relay_security_simulation_reset_button_bounds(content_bounds).contains(point) {
                return Some(PaneHitAction::RelaySecuritySimulation(
                    RelaySecuritySimulationPaneAction::Reset,
                ));
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
        PaneKind::Empty | PaneKind::ProviderStatus => None,
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
        .request_type
        .event(
            event,
            network_requests_type_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .network_requests_inputs
        .payload
        .event(
            event,
            network_requests_payload_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .network_requests_inputs
        .skill_scope_id
        .event(
            event,
            network_requests_skill_scope_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .network_requests_inputs
        .credit_envelope_ref
        .event(
            event,
            network_requests_credit_envelope_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .network_requests_inputs
        .budget_sats
        .event(
            event,
            network_requests_budget_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .network_requests_inputs
        .timeout_seconds
        .event(
            event,
            network_requests_timeout_input_bounds(content_bounds),
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

fn pane_title_bounds(bounds: Bounds) -> Bounds {
    Bounds::new(
        bounds.origin.x,
        bounds.origin.y,
        bounds.size.width,
        PANE_TITLE_HEIGHT,
    )
}

fn cursor_icon_for_resize_edge(edge: ResizeEdge) -> CursorIcon {
    match edge {
        ResizeEdge::Top | ResizeEdge::Bottom => CursorIcon::NsResize,
        ResizeEdge::Left | ResizeEdge::Right => CursorIcon::EwResize,
        ResizeEdge::TopLeft | ResizeEdge::BottomRight => CursorIcon::NwseResize,
        ResizeEdge::TopRight | ResizeEdge::BottomLeft => CursorIcon::NeswResize,
        ResizeEdge::None => CursorIcon::Default,
    }
}

fn clamp_bounds_to_window(bounds: Bounds, window_size: Size) -> Bounds {
    let max_width = (window_size.width - PANE_MARGIN * 2.0).max(PANE_MIN_WIDTH);
    let width = bounds.size.width.clamp(PANE_MIN_WIDTH, max_width);

    let max_height = (window_size.height - PANE_MARGIN - PANE_BOTTOM_RESERVED).max(PANE_MIN_HEIGHT);
    let height = bounds.size.height.clamp(PANE_MIN_HEIGHT, max_height);

    let max_x = (window_size.width - width - PANE_MARGIN).max(PANE_MARGIN);
    let max_y = (window_size.height - height - PANE_BOTTOM_RESERVED).max(PANE_MARGIN);

    let x = bounds.origin.x.clamp(PANE_MARGIN, max_x);
    let y = bounds.origin.y.clamp(PANE_MARGIN, max_y);

    Bounds::new(x, y, width, height)
}

#[cfg(test)]
mod tests {
    use super::{
        PaneDescriptor, active_job_abort_button_bounds, active_job_advance_button_bounds,
        activity_feed_filter_button_bounds, activity_feed_refresh_button_bounds,
        activity_feed_row_bounds, agent_profile_publish_profile_button_bounds,
        agent_profile_publish_state_button_bounds, agent_profile_update_goals_button_bounds,
        agent_schedule_apply_button_bounds, agent_schedule_inspect_button_bounds,
        agent_schedule_manual_tick_button_bounds, alerts_recovery_ack_button_bounds,
        alerts_recovery_recover_button_bounds, alerts_recovery_resolve_button_bounds,
        alerts_recovery_row_bounds, chat_composer_input_bounds, chat_send_button_bounds,
        chat_thread_rail_bounds, chat_transcript_bounds, codex_account_cancel_login_button_bounds,
        codex_account_login_button_bounds, codex_account_logout_button_bounds,
        codex_account_rate_limits_button_bounds, codex_account_refresh_button_bounds,
        codex_apps_refresh_button_bounds, codex_config_batch_write_button_bounds,
        codex_config_detect_external_button_bounds, codex_config_import_external_button_bounds,
        codex_config_read_button_bounds, codex_config_requirements_button_bounds,
        codex_config_write_button_bounds, codex_mcp_login_button_bounds,
        codex_mcp_refresh_button_bounds, codex_mcp_reload_button_bounds,
        codex_models_refresh_button_bounds, codex_models_toggle_hidden_button_bounds,
        codex_remote_skills_export_button_bounds, codex_remote_skills_refresh_button_bounds,
        credit_desk_envelope_button_bounds, credit_desk_intent_button_bounds,
        credit_desk_offer_button_bounds, credit_desk_spend_button_bounds,
        credit_settlement_default_button_bounds, credit_settlement_reputation_button_bounds,
        credit_settlement_verify_button_bounds, earnings_scoreboard_refresh_button_bounds,
        go_online_toggle_button_bounds, job_history_next_page_button_bounds,
        job_history_prev_page_button_bounds, job_history_search_input_bounds,
        job_history_status_button_bounds, job_history_time_button_bounds,
        job_inbox_accept_button_bounds, job_inbox_reject_button_bounds, job_inbox_row_bounds,
        network_requests_budget_input_bounds, network_requests_credit_envelope_input_bounds,
        network_requests_payload_input_bounds, network_requests_skill_scope_input_bounds,
        network_requests_submit_button_bounds, network_requests_timeout_input_bounds,
        network_requests_type_input_bounds, nostr_copy_secret_button_bounds,
        nostr_regenerate_button_bounds, nostr_reveal_button_bounds, pane_content_bounds,
        relay_connections_add_button_bounds, relay_connections_remove_button_bounds,
        relay_connections_retry_button_bounds, relay_connections_row_bounds,
        relay_connections_url_input_bounds, settings_provider_queue_input_bounds,
        settings_relay_input_bounds, settings_reset_button_bounds, settings_save_button_bounds,
        settings_wallet_default_input_bounds, skill_registry_discover_button_bounds,
        skill_registry_inspect_button_bounds, skill_registry_install_button_bounds,
        skill_trust_attestations_button_bounds, skill_trust_kill_switch_button_bounds,
        skill_trust_refresh_button_bounds, skill_trust_revoke_button_bounds,
        starter_jobs_complete_button_bounds, starter_jobs_row_bounds,
        sync_health_rebootstrap_button_bounds, trajectory_filter_button_bounds,
        trajectory_open_session_button_bounds, trajectory_verify_button_bounds,
    };
    use crate::pane_registry::pane_specs;
    use wgpui::Bounds;

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
    fn pane_content_bounds_reserve_title_space() {
        let pane = Bounds::new(10.0, 20.0, 400.0, 300.0);
        let content = pane_content_bounds(pane);

        assert!((content.origin.x - pane.origin.x).abs() <= f32::EPSILON);
        assert!(content.origin.y > pane.origin.y);
        assert!((content.size.width - pane.size.width).abs() <= f32::EPSILON);
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
        let rail = chat_thread_rail_bounds(content);
        let transcript = chat_transcript_bounds(content);
        let composer = chat_composer_input_bounds(content);
        let send = chat_send_button_bounds(content);

        assert!(rail.max_x() < transcript.min_x());
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

        let remote_refresh = codex_remote_skills_refresh_button_bounds(content);
        let remote_export = codex_remote_skills_export_button_bounds(content);
        assert!(remote_refresh.max_x() < remote_export.min_x());
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
        let submit = network_requests_submit_button_bounds(content);

        assert!(request_type.max_y() < payload.min_y());
        assert!(payload.max_y() < scope.min_y());
        assert!(scope.max_y() < envelope.min_y());
        assert!(envelope.max_y() < budget.min_y());
        assert!(budget.max_x() < timeout.min_x());
        assert!(timeout.max_x() < submit.min_x());
    }

    #[test]
    fn starter_jobs_controls_and_rows_are_ordered() {
        let content = Bounds::new(0.0, 0.0, 860.0, 420.0);
        let complete = starter_jobs_complete_button_bounds(content);
        let row0 = starter_jobs_row_bounds(content, 0);
        let row1 = starter_jobs_row_bounds(content, 1);

        assert!(complete.max_y() < row0.min_y());
        assert!(row0.max_y() < row1.min_y());
    }

    #[test]
    fn activity_feed_controls_and_rows_are_ordered() {
        let content = Bounds::new(0.0, 0.0, 940.0, 460.0);
        let refresh = activity_feed_refresh_button_bounds(content);
        let filter0 = activity_feed_filter_button_bounds(content, 0);
        let filter1 = activity_feed_filter_button_bounds(content, 1);
        let row0 = activity_feed_row_bounds(content, 0);

        assert!(refresh.max_y() < filter0.min_y());
        assert!(filter0.max_x() < filter1.max_x());
        assert!(filter0.max_y() < row0.min_y());
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
        assert!(profile.max_x() < state.min_x());
        assert!(state.max_x() < goals.min_x());

        let apply = agent_schedule_apply_button_bounds(content);
        let tick = agent_schedule_manual_tick_button_bounds(content);
        let inspect = agent_schedule_inspect_button_bounds(content);
        assert!(apply.max_x() < tick.min_x());
        assert!(tick.max_x() < inspect.min_x());
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
    fn earnings_refresh_button_is_inside_content() {
        let content = Bounds::new(0.0, 0.0, 640.0, 320.0);
        let button = earnings_scoreboard_refresh_button_bounds(content);
        assert!(content.contains(button.origin));
        assert!(button.max_x() <= content.max_x());
        assert!(button.max_y() <= content.max_y());
    }
}
