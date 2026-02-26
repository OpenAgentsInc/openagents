use wgpui::components::hud::{PaneFrame, ResizeEdge};
use wgpui::{Bounds, Component, InputEvent, Modifiers, MouseButton, Point, Size};
use winit::window::CursorIcon;

use crate::app_state::{ActivityFeedFilter, DesktopPane, PaneDragMode, PaneKind, RenderState};
use crate::hotbar::{HOTBAR_FLOAT_GAP, HOTBAR_HEIGHT};
use crate::render::logical_size;
use crate::spark_pane::{
    self, CREATE_INVOICE_PANE_HEIGHT, CREATE_INVOICE_PANE_WIDTH, CreateInvoicePaneAction,
    PAY_INVOICE_PANE_HEIGHT, PAY_INVOICE_PANE_WIDTH, PayInvoicePaneAction, SPARK_PANE_HEIGHT,
    SPARK_PANE_WIDTH, SparkPaneAction,
};

const PANE_DEFAULT_WIDTH: f32 = 420.0;
const PANE_DEFAULT_HEIGHT: f32 = 280.0;
const CHAT_PANE_WIDTH: f32 = 940.0;
const CHAT_PANE_HEIGHT: f32 = 540.0;
const GO_ONLINE_PANE_WIDTH: f32 = 560.0;
const GO_ONLINE_PANE_HEIGHT: f32 = 300.0;
const PROVIDER_STATUS_PANE_WIDTH: f32 = 700.0;
const PROVIDER_STATUS_PANE_HEIGHT: f32 = 360.0;
const EARNINGS_SCOREBOARD_PANE_WIDTH: f32 = 640.0;
const EARNINGS_SCOREBOARD_PANE_HEIGHT: f32 = 320.0;
const RELAY_CONNECTIONS_PANE_WIDTH: f32 = 900.0;
const RELAY_CONNECTIONS_PANE_HEIGHT: f32 = 420.0;
const SYNC_HEALTH_PANE_WIDTH: f32 = 760.0;
const SYNC_HEALTH_PANE_HEIGHT: f32 = 360.0;
const NETWORK_REQUESTS_PANE_WIDTH: f32 = 900.0;
const NETWORK_REQUESTS_PANE_HEIGHT: f32 = 420.0;
const STARTER_JOBS_PANE_WIDTH: f32 = 860.0;
const STARTER_JOBS_PANE_HEIGHT: f32 = 420.0;
const ACTIVITY_FEED_PANE_WIDTH: f32 = 940.0;
const ACTIVITY_FEED_PANE_HEIGHT: f32 = 460.0;
const ALERTS_RECOVERY_PANE_WIDTH: f32 = 900.0;
const ALERTS_RECOVERY_PANE_HEIGHT: f32 = 460.0;
const JOB_INBOX_PANE_WIDTH: f32 = 860.0;
const JOB_INBOX_PANE_HEIGHT: f32 = 420.0;
const ACTIVE_JOB_PANE_WIDTH: f32 = 860.0;
const ACTIVE_JOB_PANE_HEIGHT: f32 = 440.0;
const JOB_HISTORY_PANE_WIDTH: f32 = 900.0;
const JOB_HISTORY_PANE_HEIGHT: f32 = 460.0;
const NOSTR_PANE_WIDTH: f32 = 760.0;
const NOSTR_PANE_HEIGHT: f32 = 380.0;
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

#[derive(Clone, Copy)]
pub struct PaneDescriptor {
    pub kind: PaneKind,
    pub width: f32,
    pub height: f32,
    pub singleton: bool,
}

impl PaneDescriptor {
    pub const fn empty() -> Self {
        Self {
            kind: PaneKind::Empty,
            width: PANE_DEFAULT_WIDTH,
            height: PANE_DEFAULT_HEIGHT,
            singleton: false,
        }
    }

    pub const fn autopilot_chat() -> Self {
        Self {
            kind: PaneKind::AutopilotChat,
            width: CHAT_PANE_WIDTH,
            height: CHAT_PANE_HEIGHT,
            singleton: true,
        }
    }

    pub const fn go_online() -> Self {
        Self {
            kind: PaneKind::GoOnline,
            width: GO_ONLINE_PANE_WIDTH,
            height: GO_ONLINE_PANE_HEIGHT,
            singleton: true,
        }
    }

    pub const fn provider_status() -> Self {
        Self {
            kind: PaneKind::ProviderStatus,
            width: PROVIDER_STATUS_PANE_WIDTH,
            height: PROVIDER_STATUS_PANE_HEIGHT,
            singleton: true,
        }
    }

    pub const fn earnings_scoreboard() -> Self {
        Self {
            kind: PaneKind::EarningsScoreboard,
            width: EARNINGS_SCOREBOARD_PANE_WIDTH,
            height: EARNINGS_SCOREBOARD_PANE_HEIGHT,
            singleton: true,
        }
    }

    pub const fn relay_connections() -> Self {
        Self {
            kind: PaneKind::RelayConnections,
            width: RELAY_CONNECTIONS_PANE_WIDTH,
            height: RELAY_CONNECTIONS_PANE_HEIGHT,
            singleton: true,
        }
    }

    pub const fn sync_health() -> Self {
        Self {
            kind: PaneKind::SyncHealth,
            width: SYNC_HEALTH_PANE_WIDTH,
            height: SYNC_HEALTH_PANE_HEIGHT,
            singleton: true,
        }
    }

    pub const fn network_requests() -> Self {
        Self {
            kind: PaneKind::NetworkRequests,
            width: NETWORK_REQUESTS_PANE_WIDTH,
            height: NETWORK_REQUESTS_PANE_HEIGHT,
            singleton: true,
        }
    }

    pub const fn starter_jobs() -> Self {
        Self {
            kind: PaneKind::StarterJobs,
            width: STARTER_JOBS_PANE_WIDTH,
            height: STARTER_JOBS_PANE_HEIGHT,
            singleton: true,
        }
    }

    pub const fn activity_feed() -> Self {
        Self {
            kind: PaneKind::ActivityFeed,
            width: ACTIVITY_FEED_PANE_WIDTH,
            height: ACTIVITY_FEED_PANE_HEIGHT,
            singleton: true,
        }
    }

    pub const fn alerts_recovery() -> Self {
        Self {
            kind: PaneKind::AlertsRecovery,
            width: ALERTS_RECOVERY_PANE_WIDTH,
            height: ALERTS_RECOVERY_PANE_HEIGHT,
            singleton: true,
        }
    }

    pub const fn job_inbox() -> Self {
        Self {
            kind: PaneKind::JobInbox,
            width: JOB_INBOX_PANE_WIDTH,
            height: JOB_INBOX_PANE_HEIGHT,
            singleton: true,
        }
    }

    pub const fn active_job() -> Self {
        Self {
            kind: PaneKind::ActiveJob,
            width: ACTIVE_JOB_PANE_WIDTH,
            height: ACTIVE_JOB_PANE_HEIGHT,
            singleton: true,
        }
    }

    pub const fn job_history() -> Self {
        Self {
            kind: PaneKind::JobHistory,
            width: JOB_HISTORY_PANE_WIDTH,
            height: JOB_HISTORY_PANE_HEIGHT,
            singleton: true,
        }
    }

    pub const fn nostr_identity() -> Self {
        Self {
            kind: PaneKind::NostrIdentity,
            width: NOSTR_PANE_WIDTH,
            height: NOSTR_PANE_HEIGHT,
            singleton: true,
        }
    }

    pub const fn spark_wallet() -> Self {
        Self {
            kind: PaneKind::SparkWallet,
            width: SPARK_PANE_WIDTH,
            height: SPARK_PANE_HEIGHT,
            singleton: true,
        }
    }

    pub const fn create_invoice() -> Self {
        Self {
            kind: PaneKind::SparkCreateInvoice,
            width: CREATE_INVOICE_PANE_WIDTH,
            height: CREATE_INVOICE_PANE_HEIGHT,
            singleton: true,
        }
    }

    pub const fn pay_invoice() -> Self {
        Self {
            kind: PaneKind::SparkPayInvoice,
            width: PAY_INVOICE_PANE_WIDTH,
            height: PAY_INVOICE_PANE_HEIGHT,
            singleton: true,
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

    pub fn create_empty(state: &mut RenderState) {
        let _ = Self::create(state, PaneDescriptor::empty());
    }

    pub fn create_autopilot_chat(state: &mut RenderState) {
        let _ = Self::create(state, PaneDescriptor::autopilot_chat());
    }

    pub fn create_go_online(state: &mut RenderState) {
        let _ = Self::create(state, PaneDescriptor::go_online());
    }

    pub fn create_provider_status(state: &mut RenderState) {
        let _ = Self::create(state, PaneDescriptor::provider_status());
    }

    pub fn create_earnings_scoreboard(state: &mut RenderState) {
        let _ = Self::create(state, PaneDescriptor::earnings_scoreboard());
    }

    pub fn create_relay_connections(state: &mut RenderState) {
        let _ = Self::create(state, PaneDescriptor::relay_connections());
    }

    pub fn create_sync_health(state: &mut RenderState) {
        let _ = Self::create(state, PaneDescriptor::sync_health());
    }

    pub fn create_network_requests(state: &mut RenderState) {
        let _ = Self::create(state, PaneDescriptor::network_requests());
    }

    pub fn create_starter_jobs(state: &mut RenderState) {
        let _ = Self::create(state, PaneDescriptor::starter_jobs());
    }

    pub fn create_activity_feed(state: &mut RenderState) {
        let _ = Self::create(state, PaneDescriptor::activity_feed());
    }

    pub fn create_alerts_recovery(state: &mut RenderState) {
        let _ = Self::create(state, PaneDescriptor::alerts_recovery());
    }

    pub fn create_job_inbox(state: &mut RenderState) {
        let _ = Self::create(state, PaneDescriptor::job_inbox());
    }

    pub fn create_active_job(state: &mut RenderState) {
        let _ = Self::create(state, PaneDescriptor::active_job());
    }

    pub fn create_job_history(state: &mut RenderState) {
        let _ = Self::create(state, PaneDescriptor::job_history());
    }

    pub fn create_nostr_identity(state: &mut RenderState) {
        let _ = Self::create(state, PaneDescriptor::nostr_identity());
    }

    pub fn create_spark_wallet(state: &mut RenderState) {
        let _ = Self::create(state, PaneDescriptor::spark_wallet());
    }

    pub fn create_create_invoice(state: &mut RenderState) {
        let _ = Self::create(state, PaneDescriptor::create_invoice());
    }

    pub fn create_pay_invoice(state: &mut RenderState) {
        let _ = Self::create(state, PaneDescriptor::pay_invoice());
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

    for pane_idx in pane_indices_by_z_desc(state) {
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

        if state.panes[pane_idx].kind == PaneKind::AutopilotChat {
            let content_bounds = pane_content_bounds(bounds);
            let send_bounds = chat_send_button_bounds(content_bounds);
            let composer_bounds = chat_composer_input_bounds(content_bounds);
            if send_bounds.contains(point) {
                return CursorIcon::Pointer;
            }
            if composer_bounds.contains(point) {
                return CursorIcon::Text;
            }
        }

        if state.panes[pane_idx].kind == PaneKind::GoOnline {
            let content_bounds = pane_content_bounds(bounds);
            let toggle_bounds = go_online_toggle_button_bounds(content_bounds);
            if toggle_bounds.contains(point) {
                return CursorIcon::Pointer;
            }
        }

        if state.panes[pane_idx].kind == PaneKind::EarningsScoreboard {
            if topmost_earnings_scoreboard_action_hit(state, point).is_some() {
                return CursorIcon::Pointer;
            }
        }

        if state.panes[pane_idx].kind == PaneKind::RelayConnections {
            let content_bounds = pane_content_bounds(bounds);
            if topmost_relay_connections_action_hit(state, point).is_some() {
                return CursorIcon::Pointer;
            }
            if relay_connections_url_input_bounds(content_bounds).contains(point) {
                return CursorIcon::Text;
            }
        }

        if state.panes[pane_idx].kind == PaneKind::SyncHealth {
            if topmost_sync_health_action_hit(state, point).is_some() {
                return CursorIcon::Pointer;
            }
        }

        if state.panes[pane_idx].kind == PaneKind::NetworkRequests {
            let content_bounds = pane_content_bounds(bounds);
            if topmost_network_requests_action_hit(state, point).is_some() {
                return CursorIcon::Pointer;
            }
            if network_requests_type_input_bounds(content_bounds).contains(point)
                || network_requests_payload_input_bounds(content_bounds).contains(point)
                || network_requests_budget_input_bounds(content_bounds).contains(point)
                || network_requests_timeout_input_bounds(content_bounds).contains(point)
            {
                return CursorIcon::Text;
            }
        }

        if state.panes[pane_idx].kind == PaneKind::StarterJobs
            && topmost_starter_jobs_action_hit(state, point).is_some()
        {
            return CursorIcon::Pointer;
        }

        if state.panes[pane_idx].kind == PaneKind::ActivityFeed
            && topmost_activity_feed_action_hit(state, point).is_some()
        {
            return CursorIcon::Pointer;
        }

        if state.panes[pane_idx].kind == PaneKind::AlertsRecovery
            && topmost_alerts_recovery_action_hit(state, point).is_some()
        {
            return CursorIcon::Pointer;
        }

        if state.panes[pane_idx].kind == PaneKind::JobInbox {
            if topmost_job_inbox_action_hit(state, point).is_some() {
                return CursorIcon::Pointer;
            }
        }

        if state.panes[pane_idx].kind == PaneKind::ActiveJob {
            if topmost_active_job_action_hit(state, point).is_some() {
                return CursorIcon::Pointer;
            }
        }

        if state.panes[pane_idx].kind == PaneKind::JobHistory {
            let content_bounds = pane_content_bounds(bounds);
            if topmost_job_history_action_hit(state, point).is_some() {
                return CursorIcon::Pointer;
            }
            if job_history_search_input_bounds(content_bounds).contains(point) {
                return CursorIcon::Text;
            }
        }

        if state.panes[pane_idx].kind == PaneKind::NostrIdentity {
            let content_bounds = pane_content_bounds(bounds);
            let regenerate_bounds = nostr_regenerate_button_bounds(content_bounds);
            let reveal_bounds = nostr_reveal_button_bounds(content_bounds);
            let copy_bounds = nostr_copy_secret_button_bounds(content_bounds);
            if regenerate_bounds.contains(point)
                || reveal_bounds.contains(point)
                || copy_bounds.contains(point)
            {
                return CursorIcon::Pointer;
            }
        }

        if state.panes[pane_idx].kind == PaneKind::SparkWallet {
            let content_bounds = pane_content_bounds(bounds);
            let layout = spark_pane::layout(content_bounds);
            if spark_pane::hit_action(layout, point).is_some() {
                return CursorIcon::Pointer;
            }
            if spark_pane::hits_input(layout, point) {
                return CursorIcon::Text;
            }
        }

        if state.panes[pane_idx].kind == PaneKind::SparkCreateInvoice {
            let content_bounds = pane_content_bounds(bounds);
            let layout = spark_pane::create_invoice_layout(content_bounds);
            if spark_pane::hit_create_invoice_action(layout, point).is_some() {
                return CursorIcon::Pointer;
            }
            if spark_pane::hits_create_invoice_input(layout, point) {
                return CursorIcon::Text;
            }
        }

        if state.panes[pane_idx].kind == PaneKind::SparkPayInvoice {
            let content_bounds = pane_content_bounds(bounds);
            let layout = spark_pane::pay_invoice_layout(content_bounds);
            if spark_pane::hit_pay_invoice_action(layout, point).is_some() {
                return CursorIcon::Pointer;
            }
            if spark_pane::hits_pay_invoice_input(layout, point) {
                return CursorIcon::Text;
            }
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

pub fn chat_send_button_bounds(content_bounds: Bounds) -> Bounds {
    Bounds::new(
        content_bounds.max_x() - CHAT_PAD - CHAT_SEND_WIDTH,
        content_bounds.max_y() - CHAT_PAD - CHAT_COMPOSER_HEIGHT,
        CHAT_SEND_WIDTH,
        CHAT_COMPOSER_HEIGHT,
    )
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
    let width = content_bounds.size.width.min(220.0).max(160.0);
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        content_bounds.origin.y + CHAT_PAD,
        width,
        34.0,
    )
}

pub fn earnings_scoreboard_refresh_button_bounds(content_bounds: Bounds) -> Bounds {
    let width = content_bounds.size.width.min(220.0).max(160.0);
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

pub fn network_requests_budget_input_bounds(content_bounds: Bounds) -> Bounds {
    let payload = network_requests_payload_input_bounds(content_bounds);
    Bounds::new(
        content_bounds.origin.x + CHAT_PAD,
        payload.max_y() + 10.0,
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

pub fn job_inbox_accept_button_bounds(content_bounds: Bounds) -> Bounds {
    let button_width = content_bounds.size.width.min(196.0).max(144.0);
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
    let width = content_bounds.size.width.min(196.0).max(144.0);
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

pub fn topmost_nostr_regenerate_hit(state: &RenderState, point: Point) -> Option<u64> {
    for pane_idx in pane_indices_by_z_desc(state) {
        let pane = &state.panes[pane_idx];
        if pane.kind != PaneKind::NostrIdentity {
            continue;
        }

        let content_bounds = pane_content_bounds(pane.bounds);
        let regenerate_bounds = nostr_regenerate_button_bounds(content_bounds);
        if regenerate_bounds.contains(point) {
            return Some(pane.id);
        }
    }

    None
}

pub fn topmost_nostr_reveal_hit(state: &RenderState, point: Point) -> Option<u64> {
    for pane_idx in pane_indices_by_z_desc(state) {
        let pane = &state.panes[pane_idx];
        if pane.kind != PaneKind::NostrIdentity {
            continue;
        }

        let content_bounds = pane_content_bounds(pane.bounds);
        let reveal_bounds = nostr_reveal_button_bounds(content_bounds);
        if reveal_bounds.contains(point) {
            return Some(pane.id);
        }
    }

    None
}

pub fn topmost_nostr_copy_secret_hit(state: &RenderState, point: Point) -> Option<u64> {
    for pane_idx in pane_indices_by_z_desc(state) {
        let pane = &state.panes[pane_idx];
        if pane.kind != PaneKind::NostrIdentity {
            continue;
        }

        let content_bounds = pane_content_bounds(pane.bounds);
        let copy_bounds = nostr_copy_secret_button_bounds(content_bounds);
        if copy_bounds.contains(point) {
            return Some(pane.id);
        }
    }

    None
}

pub fn topmost_chat_send_hit(state: &RenderState, point: Point) -> Option<u64> {
    for pane_idx in pane_indices_by_z_desc(state) {
        let pane = &state.panes[pane_idx];
        if pane.kind != PaneKind::AutopilotChat {
            continue;
        }

        let content_bounds = pane_content_bounds(pane.bounds);
        if chat_send_button_bounds(content_bounds).contains(point) {
            return Some(pane.id);
        }
    }

    None
}

pub fn topmost_go_online_toggle_hit(state: &RenderState, point: Point) -> Option<u64> {
    for pane_idx in pane_indices_by_z_desc(state) {
        let pane = &state.panes[pane_idx];
        if pane.kind != PaneKind::GoOnline {
            continue;
        }

        let content_bounds = pane_content_bounds(pane.bounds);
        if go_online_toggle_button_bounds(content_bounds).contains(point) {
            return Some(pane.id);
        }
    }

    None
}

pub fn topmost_earnings_scoreboard_action_hit(
    state: &RenderState,
    point: Point,
) -> Option<(u64, EarningsScoreboardPaneAction)> {
    for pane_idx in pane_indices_by_z_desc(state) {
        let pane = &state.panes[pane_idx];
        if pane.kind != PaneKind::EarningsScoreboard {
            continue;
        }

        let content_bounds = pane_content_bounds(pane.bounds);
        if earnings_scoreboard_refresh_button_bounds(content_bounds).contains(point) {
            return Some((pane.id, EarningsScoreboardPaneAction::Refresh));
        }
    }

    None
}

pub fn topmost_relay_connections_action_hit(
    state: &RenderState,
    point: Point,
) -> Option<(u64, RelayConnectionsPaneAction)> {
    for pane_idx in pane_indices_by_z_desc(state) {
        let pane = &state.panes[pane_idx];
        if pane.kind != PaneKind::RelayConnections {
            continue;
        }

        let content_bounds = pane_content_bounds(pane.bounds);
        if relay_connections_add_button_bounds(content_bounds).contains(point) {
            return Some((pane.id, RelayConnectionsPaneAction::AddRelay));
        }
        if relay_connections_remove_button_bounds(content_bounds).contains(point) {
            return Some((pane.id, RelayConnectionsPaneAction::RemoveSelected));
        }
        if relay_connections_retry_button_bounds(content_bounds).contains(point) {
            return Some((pane.id, RelayConnectionsPaneAction::RetrySelected));
        }

        let visible_rows =
            relay_connections_visible_row_count(state.relay_connections.relays.len());
        for row_index in 0..visible_rows {
            if relay_connections_row_bounds(content_bounds, row_index).contains(point) {
                return Some((pane.id, RelayConnectionsPaneAction::SelectRow(row_index)));
            }
        }
    }

    None
}

pub fn topmost_sync_health_action_hit(
    state: &RenderState,
    point: Point,
) -> Option<(u64, SyncHealthPaneAction)> {
    for pane_idx in pane_indices_by_z_desc(state) {
        let pane = &state.panes[pane_idx];
        if pane.kind != PaneKind::SyncHealth {
            continue;
        }

        let content_bounds = pane_content_bounds(pane.bounds);
        if sync_health_rebootstrap_button_bounds(content_bounds).contains(point) {
            return Some((pane.id, SyncHealthPaneAction::Rebootstrap));
        }
    }

    None
}

pub fn topmost_network_requests_action_hit(
    state: &RenderState,
    point: Point,
) -> Option<(u64, NetworkRequestsPaneAction)> {
    for pane_idx in pane_indices_by_z_desc(state) {
        let pane = &state.panes[pane_idx];
        if pane.kind != PaneKind::NetworkRequests {
            continue;
        }

        let content_bounds = pane_content_bounds(pane.bounds);
        if network_requests_submit_button_bounds(content_bounds).contains(point) {
            return Some((pane.id, NetworkRequestsPaneAction::SubmitRequest));
        }
    }

    None
}

pub fn topmost_starter_jobs_action_hit(
    state: &RenderState,
    point: Point,
) -> Option<(u64, StarterJobsPaneAction)> {
    for pane_idx in pane_indices_by_z_desc(state) {
        let pane = &state.panes[pane_idx];
        if pane.kind != PaneKind::StarterJobs {
            continue;
        }

        let content_bounds = pane_content_bounds(pane.bounds);
        if starter_jobs_complete_button_bounds(content_bounds).contains(point) {
            return Some((pane.id, StarterJobsPaneAction::CompleteSelected));
        }

        let visible_rows = starter_jobs_visible_row_count(state.starter_jobs.jobs.len());
        for row_index in 0..visible_rows {
            if starter_jobs_row_bounds(content_bounds, row_index).contains(point) {
                return Some((pane.id, StarterJobsPaneAction::SelectRow(row_index)));
            }
        }
    }

    None
}

pub fn topmost_activity_feed_action_hit(
    state: &RenderState,
    point: Point,
) -> Option<(u64, ActivityFeedPaneAction)> {
    for pane_idx in pane_indices_by_z_desc(state) {
        let pane = &state.panes[pane_idx];
        if pane.kind != PaneKind::ActivityFeed {
            continue;
        }

        let content_bounds = pane_content_bounds(pane.bounds);
        if activity_feed_refresh_button_bounds(content_bounds).contains(point) {
            return Some((pane.id, ActivityFeedPaneAction::Refresh));
        }

        let filters = ActivityFeedFilter::all();
        for (filter_index, filter) in filters.into_iter().enumerate() {
            if activity_feed_filter_button_bounds(content_bounds, filter_index).contains(point) {
                return Some((pane.id, ActivityFeedPaneAction::SetFilter(filter)));
            }
        }

        let visible_rows =
            activity_feed_visible_row_count(state.activity_feed.visible_rows().len());
        for row_index in 0..visible_rows {
            if activity_feed_row_bounds(content_bounds, row_index).contains(point) {
                return Some((pane.id, ActivityFeedPaneAction::SelectRow(row_index)));
            }
        }
    }

    None
}

pub fn topmost_alerts_recovery_action_hit(
    state: &RenderState,
    point: Point,
) -> Option<(u64, AlertsRecoveryPaneAction)> {
    for pane_idx in pane_indices_by_z_desc(state) {
        let pane = &state.panes[pane_idx];
        if pane.kind != PaneKind::AlertsRecovery {
            continue;
        }

        let content_bounds = pane_content_bounds(pane.bounds);
        if alerts_recovery_recover_button_bounds(content_bounds).contains(point) {
            return Some((pane.id, AlertsRecoveryPaneAction::RecoverSelected));
        }
        if alerts_recovery_ack_button_bounds(content_bounds).contains(point) {
            return Some((pane.id, AlertsRecoveryPaneAction::AcknowledgeSelected));
        }
        if alerts_recovery_resolve_button_bounds(content_bounds).contains(point) {
            return Some((pane.id, AlertsRecoveryPaneAction::ResolveSelected));
        }

        let visible_rows = alerts_recovery_visible_row_count(state.alerts_recovery.alerts.len());
        for row_index in 0..visible_rows {
            if alerts_recovery_row_bounds(content_bounds, row_index).contains(point) {
                return Some((pane.id, AlertsRecoveryPaneAction::SelectRow(row_index)));
            }
        }
    }

    None
}

pub fn topmost_job_inbox_action_hit(
    state: &RenderState,
    point: Point,
) -> Option<(u64, JobInboxPaneAction)> {
    for pane_idx in pane_indices_by_z_desc(state) {
        let pane = &state.panes[pane_idx];
        if pane.kind != PaneKind::JobInbox {
            continue;
        }

        let content_bounds = pane_content_bounds(pane.bounds);
        if job_inbox_accept_button_bounds(content_bounds).contains(point) {
            return Some((pane.id, JobInboxPaneAction::AcceptSelected));
        }
        if job_inbox_reject_button_bounds(content_bounds).contains(point) {
            return Some((pane.id, JobInboxPaneAction::RejectSelected));
        }

        let visible_rows = job_inbox_visible_row_count(state.job_inbox.requests.len());
        for row_index in 0..visible_rows {
            if job_inbox_row_bounds(content_bounds, row_index).contains(point) {
                return Some((pane.id, JobInboxPaneAction::SelectRow(row_index)));
            }
        }
    }

    None
}

pub fn topmost_active_job_action_hit(
    state: &RenderState,
    point: Point,
) -> Option<(u64, ActiveJobPaneAction)> {
    for pane_idx in pane_indices_by_z_desc(state) {
        let pane = &state.panes[pane_idx];
        if pane.kind != PaneKind::ActiveJob {
            continue;
        }

        let content_bounds = pane_content_bounds(pane.bounds);
        if active_job_advance_button_bounds(content_bounds).contains(point) {
            return Some((pane.id, ActiveJobPaneAction::AdvanceStage));
        }
        if state.active_job.runtime_supports_abort
            && active_job_abort_button_bounds(content_bounds).contains(point)
        {
            return Some((pane.id, ActiveJobPaneAction::AbortJob));
        }
    }

    None
}

pub fn topmost_job_history_action_hit(
    state: &RenderState,
    point: Point,
) -> Option<(u64, JobHistoryPaneAction)> {
    for pane_idx in pane_indices_by_z_desc(state) {
        let pane = &state.panes[pane_idx];
        if pane.kind != PaneKind::JobHistory {
            continue;
        }

        let content_bounds = pane_content_bounds(pane.bounds);
        if job_history_status_button_bounds(content_bounds).contains(point) {
            return Some((pane.id, JobHistoryPaneAction::CycleStatusFilter));
        }
        if job_history_time_button_bounds(content_bounds).contains(point) {
            return Some((pane.id, JobHistoryPaneAction::CycleTimeRange));
        }
        if job_history_prev_page_button_bounds(content_bounds).contains(point) {
            return Some((pane.id, JobHistoryPaneAction::PreviousPage));
        }
        if job_history_next_page_button_bounds(content_bounds).contains(point) {
            return Some((pane.id, JobHistoryPaneAction::NextPage));
        }
    }

    None
}

pub fn topmost_spark_action_hit(
    state: &RenderState,
    point: Point,
) -> Option<(u64, SparkPaneAction)> {
    for pane_idx in pane_indices_by_z_desc(state) {
        let pane = &state.panes[pane_idx];
        if pane.kind != PaneKind::SparkWallet {
            continue;
        }

        let content_bounds = pane_content_bounds(pane.bounds);
        let layout = spark_pane::layout(content_bounds);
        if let Some(action) = spark_pane::hit_action(layout, point) {
            return Some((pane.id, action));
        }
    }

    None
}

pub fn topmost_create_invoice_action_hit(
    state: &RenderState,
    point: Point,
) -> Option<(u64, CreateInvoicePaneAction)> {
    for pane_idx in pane_indices_by_z_desc(state) {
        let pane = &state.panes[pane_idx];
        if pane.kind != PaneKind::SparkCreateInvoice {
            continue;
        }

        let content_bounds = pane_content_bounds(pane.bounds);
        let layout = spark_pane::create_invoice_layout(content_bounds);
        if let Some(action) = spark_pane::hit_create_invoice_action(layout, point) {
            return Some((pane.id, action));
        }
    }

    None
}

pub fn topmost_pay_invoice_action_hit(
    state: &RenderState,
    point: Point,
) -> Option<(u64, PayInvoicePaneAction)> {
    for pane_idx in pane_indices_by_z_desc(state) {
        let pane = &state.panes[pane_idx];
        if pane.kind != PaneKind::SparkPayInvoice {
            continue;
        }

        let content_bounds = pane_content_bounds(pane.bounds);
        let layout = spark_pane::pay_invoice_layout(content_bounds);
        if let Some(action) = spark_pane::hit_pay_invoice_action(layout, point) {
            return Some((pane.id, action));
        }
    }

    None
}

pub fn dispatch_spark_input_event(state: &mut RenderState, event: &InputEvent) -> bool {
    let top_spark = state
        .panes
        .iter()
        .filter(|pane| pane.kind == PaneKind::SparkWallet)
        .max_by_key(|pane| pane.z_index)
        .map(|pane| pane.bounds);
    let Some(bounds) = top_spark else {
        return false;
    };

    let content_bounds = pane_content_bounds(bounds);
    let layout = spark_pane::layout(content_bounds);
    let mut handled = false;

    handled |= state
        .spark_inputs
        .invoice_amount
        .event(event, layout.invoice_amount_input, &mut state.event_context)
        .is_handled();
    handled |= state
        .spark_inputs
        .send_request
        .event(event, layout.send_request_input, &mut state.event_context)
        .is_handled();
    handled |= state
        .spark_inputs
        .send_amount
        .event(event, layout.send_amount_input, &mut state.event_context)
        .is_handled();

    handled
}

pub fn dispatch_create_invoice_input_event(state: &mut RenderState, event: &InputEvent) -> bool {
    let top_create_invoice = state
        .panes
        .iter()
        .filter(|pane| pane.kind == PaneKind::SparkCreateInvoice)
        .max_by_key(|pane| pane.z_index)
        .map(|pane| pane.bounds);
    let Some(bounds) = top_create_invoice else {
        return false;
    };

    let content_bounds = pane_content_bounds(bounds);
    let layout = spark_pane::create_invoice_layout(content_bounds);
    let mut handled = false;

    handled |= state
        .create_invoice_inputs
        .amount_sats
        .event(event, layout.amount_input, &mut state.event_context)
        .is_handled();
    handled |= state
        .create_invoice_inputs
        .description
        .event(event, layout.description_input, &mut state.event_context)
        .is_handled();
    handled |= state
        .create_invoice_inputs
        .expiry_seconds
        .event(event, layout.expiry_input, &mut state.event_context)
        .is_handled();

    handled
}

pub fn dispatch_pay_invoice_input_event(state: &mut RenderState, event: &InputEvent) -> bool {
    let top_pay_invoice = state
        .panes
        .iter()
        .filter(|pane| pane.kind == PaneKind::SparkPayInvoice)
        .max_by_key(|pane| pane.z_index)
        .map(|pane| pane.bounds);
    let Some(bounds) = top_pay_invoice else {
        return false;
    };

    let content_bounds = pane_content_bounds(bounds);
    let layout = spark_pane::pay_invoice_layout(content_bounds);
    let mut handled = false;

    handled |= state
        .pay_invoice_inputs
        .payment_request
        .event(
            event,
            layout.payment_request_input,
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .pay_invoice_inputs
        .amount_sats
        .event(event, layout.amount_input, &mut state.event_context)
        .is_handled();

    handled
}

pub fn dispatch_chat_input_event(state: &mut RenderState, event: &InputEvent) -> bool {
    let top_chat = state
        .panes
        .iter()
        .filter(|pane| pane.kind == PaneKind::AutopilotChat)
        .max_by_key(|pane| pane.z_index)
        .map(|pane| pane.bounds);
    let Some(bounds) = top_chat else {
        return false;
    };

    let composer_bounds = chat_composer_input_bounds(pane_content_bounds(bounds));
    state
        .chat_inputs
        .composer
        .event(event, composer_bounds, &mut state.event_context)
        .is_handled()
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
    let top_relay = state
        .panes
        .iter()
        .filter(|pane| pane.kind == PaneKind::RelayConnections)
        .max_by_key(|pane| pane.z_index)
        .map(|pane| pane.bounds);
    let Some(bounds) = top_relay else {
        return false;
    };

    let input_bounds = relay_connections_url_input_bounds(pane_content_bounds(bounds));
    state
        .relay_connections_inputs
        .relay_url
        .event(event, input_bounds, &mut state.event_context)
        .is_handled()
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

pub fn bring_pane_to_front_by_id(state: &mut RenderState, pane_id: u64) {
    bring_pane_to_front(state, pane_id);
}

fn pane_indices_by_z_desc(state: &RenderState) -> Vec<usize> {
    let mut ordered: Vec<usize> = (0..state.panes.len()).collect();
    ordered.sort_by(|lhs, rhs| state.panes[*rhs].z_index.cmp(&state.panes[*lhs].z_index));
    ordered
}

fn bring_pane_to_front(state: &mut RenderState, pane_id: u64) {
    if let Some(pane) = state.panes.iter_mut().find(|pane| pane.id == pane_id) {
        pane.z_index = state.next_z_index;
        state.next_z_index = state.next_z_index.saturating_add(1);
    }
}

fn pane_title(kind: PaneKind, pane_id: u64) -> String {
    match kind {
        PaneKind::Empty => format!("Pane {pane_id}"),
        PaneKind::AutopilotChat => "Autopilot Chat".to_string(),
        PaneKind::GoOnline => "Go Online".to_string(),
        PaneKind::ProviderStatus => "Provider Status".to_string(),
        PaneKind::EarningsScoreboard => "Earnings Scoreboard".to_string(),
        PaneKind::RelayConnections => "Relay Connections".to_string(),
        PaneKind::SyncHealth => "Sync Health".to_string(),
        PaneKind::NetworkRequests => "Network Requests".to_string(),
        PaneKind::StarterJobs => "Starter Jobs".to_string(),
        PaneKind::ActivityFeed => "Activity Feed".to_string(),
        PaneKind::AlertsRecovery => "Alerts and Recovery".to_string(),
        PaneKind::JobInbox => "Job Inbox".to_string(),
        PaneKind::ActiveJob => "Active Job".to_string(),
        PaneKind::JobHistory => "Job History".to_string(),
        PaneKind::NostrIdentity => "Nostr Keys (NIP-06)".to_string(),
        PaneKind::SparkWallet => "Spark Lightning Wallet".to_string(),
        PaneKind::SparkCreateInvoice => "Create Lightning Invoice".to_string(),
        PaneKind::SparkPayInvoice => "Pay Lightning Invoice".to_string(),
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
        active_job_abort_button_bounds, active_job_advance_button_bounds,
        activity_feed_filter_button_bounds, activity_feed_refresh_button_bounds,
        activity_feed_row_bounds, alerts_recovery_ack_button_bounds,
        alerts_recovery_recover_button_bounds, alerts_recovery_resolve_button_bounds,
        alerts_recovery_row_bounds, chat_composer_input_bounds, chat_send_button_bounds,
        chat_thread_rail_bounds, chat_transcript_bounds, earnings_scoreboard_refresh_button_bounds,
        go_online_toggle_button_bounds, job_history_next_page_button_bounds,
        job_history_prev_page_button_bounds, job_history_search_input_bounds,
        job_history_status_button_bounds, job_history_time_button_bounds,
        job_inbox_accept_button_bounds, job_inbox_reject_button_bounds, job_inbox_row_bounds,
        network_requests_budget_input_bounds, network_requests_payload_input_bounds,
        network_requests_submit_button_bounds, network_requests_timeout_input_bounds,
        network_requests_type_input_bounds, nostr_copy_secret_button_bounds,
        nostr_regenerate_button_bounds, nostr_reveal_button_bounds, pane_content_bounds,
        relay_connections_add_button_bounds, relay_connections_remove_button_bounds,
        relay_connections_retry_button_bounds, relay_connections_row_bounds,
        relay_connections_url_input_bounds, starter_jobs_complete_button_bounds,
        starter_jobs_row_bounds, sync_health_rebootstrap_button_bounds,
    };
    use wgpui::Bounds;

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
        let budget = network_requests_budget_input_bounds(content);
        let timeout = network_requests_timeout_input_bounds(content);
        let submit = network_requests_submit_button_bounds(content);

        assert!(request_type.max_y() < payload.min_y());
        assert!(payload.max_y() < budget.min_y());
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
    fn earnings_refresh_button_is_inside_content() {
        let content = Bounds::new(0.0, 0.0, 640.0, 320.0);
        let button = earnings_scoreboard_refresh_button_bounds(content);
        assert!(content.contains(button.origin));
        assert!(button.max_x() <= content.max_x());
        assert!(button.max_y() <= content.max_y());
    }
}
