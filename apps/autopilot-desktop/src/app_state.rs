use std::sync::Arc;
use std::time::{Duration, Instant};
use std::{cell::RefCell, rc::Rc};

use nostr::NostrIdentity;
use wgpui::components::TextInput;
use wgpui::components::hud::{CommandPalette, Hotbar, PaneFrame, ResizablePane, ResizeEdge};
use wgpui::renderer::Renderer;
use wgpui::{Bounds, EventContext, Modifiers, Point, TextSystem};
use winit::window::Window;

use crate::spark_wallet::{SparkPaneState, SparkWalletWorker};

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaneKind {
    Empty,
    AutopilotChat,
    GoOnline,
    ProviderStatus,
    NostrIdentity,
    SparkWallet,
    SparkPayInvoice,
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

pub struct DesktopPane {
    pub id: u64,
    pub title: String,
    pub kind: PaneKind,
    pub bounds: Bounds,
    pub z_index: i32,
    pub frame: PaneFrame,
}

pub struct SparkPaneInputs {
    pub invoice_amount: TextInput,
    pub send_request: TextInput,
    pub send_amount: TextInput,
}

impl Default for SparkPaneInputs {
    fn default() -> Self {
        Self {
            invoice_amount: TextInput::new().value("1000").placeholder("Invoice sats"),
            send_request: TextInput::new()
                .placeholder("Spark payment request or invoice")
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

pub struct ChatPaneInputs {
    pub composer: TextInput,
}

impl Default for ChatPaneInputs {
    fn default() -> Self {
        Self {
            composer: TextInput::new().placeholder("Ask Autopilot to do work..."),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AutopilotRole {
    User,
    Autopilot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AutopilotMessageStatus {
    Queued,
    Running,
    Done,
    Error,
}

pub struct AutopilotMessage {
    pub id: u64,
    pub role: AutopilotRole,
    pub status: AutopilotMessageStatus,
    pub content: String,
    pub status_due_at: Option<Instant>,
}

pub struct AutopilotChatState {
    pub threads: Vec<String>,
    pub active_thread: usize,
    pub messages: Vec<AutopilotMessage>,
    pub next_message_id: u64,
    pub last_error: Option<String>,
}

impl Default for AutopilotChatState {
    fn default() -> Self {
        Self {
            threads: vec!["Main".to_string()],
            active_thread: 0,
            messages: vec![AutopilotMessage {
                id: 1,
                role: AutopilotRole::Autopilot,
                status: AutopilotMessageStatus::Done,
                content: "Autopilot ready. Ask for a task to start.".to_string(),
                status_due_at: None,
            }],
            next_message_id: 2,
            last_error: None,
        }
    }
}

impl AutopilotChatState {
    pub fn submit_prompt(&mut self, now: Instant, prompt: String) {
        self.last_error = None;
        let trimmed = prompt.trim();
        if trimmed.is_empty() {
            self.last_error = Some("Prompt cannot be empty".to_string());
            self.messages.push(AutopilotMessage {
                id: self.next_message_id,
                role: AutopilotRole::Autopilot,
                status: AutopilotMessageStatus::Error,
                content: "Cannot run empty prompt".to_string(),
                status_due_at: None,
            });
            self.next_message_id = self.next_message_id.saturating_add(1);
            return;
        }

        self.messages.push(AutopilotMessage {
            id: self.next_message_id,
            role: AutopilotRole::User,
            status: AutopilotMessageStatus::Done,
            content: trimmed.to_string(),
            status_due_at: None,
        });
        self.next_message_id = self.next_message_id.saturating_add(1);

        self.messages.push(AutopilotMessage {
            id: self.next_message_id,
            role: AutopilotRole::Autopilot,
            status: AutopilotMessageStatus::Queued,
            content: format!("Queued local execution for: {trimmed}"),
            status_due_at: Some(now + Duration::from_millis(280)),
        });
        self.next_message_id = self.next_message_id.saturating_add(1);
    }

    pub fn tick(&mut self, now: Instant) -> bool {
        let mut changed = false;
        for message in &mut self.messages {
            match message.status {
                AutopilotMessageStatus::Queued => {
                    if message.status_due_at.is_some_and(|due| now >= due) {
                        message.status = AutopilotMessageStatus::Running;
                        message.status_due_at = Some(now + Duration::from_millis(620));
                        message.content = message.content.replacen(
                            "Queued local execution",
                            "Running local execution",
                            1,
                        );
                        changed = true;
                    }
                }
                AutopilotMessageStatus::Running => {
                    if message.status_due_at.is_some_and(|due| now >= due) {
                        message.status = AutopilotMessageStatus::Done;
                        message.status_due_at = None;
                        message.content = message.content.replacen(
                            "Running local execution",
                            "Completed local execution",
                            1,
                        );
                        changed = true;
                    }
                }
                AutopilotMessageStatus::Done | AutopilotMessageStatus::Error => {}
            }
        }
        changed
    }

    pub fn has_pending_messages(&self) -> bool {
        self.messages.iter().any(|message| {
            matches!(
                message.status,
                AutopilotMessageStatus::Queued | AutopilotMessageStatus::Running
            )
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderMode {
    Offline,
    Connecting,
    Online,
    Degraded,
}

impl ProviderMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Offline => "offline",
            Self::Connecting => "connecting",
            Self::Online => "online",
            Self::Degraded => "degraded",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderBlocker {
    IdentityMissing,
    WalletError,
}

impl ProviderBlocker {
    pub const fn code(self) -> &'static str {
        match self {
            Self::IdentityMissing => "IDENTITY_MISSING",
            Self::WalletError => "WALLET_ERROR",
        }
    }

    pub const fn detail(self) -> &'static str {
        match self {
            Self::IdentityMissing => "Nostr identity is not ready",
            Self::WalletError => "Spark wallet reports an error",
        }
    }
}

pub struct ProviderRuntimeState {
    pub mode: ProviderMode,
    pub mode_changed_at: Instant,
    pub connecting_until: Option<Instant>,
    pub online_since: Option<Instant>,
    pub last_heartbeat_at: Option<Instant>,
    pub heartbeat_interval: Duration,
    pub queue_depth: u32,
    pub last_completed_job_at: Option<Instant>,
    pub last_result: Option<String>,
    pub degraded_reason_code: Option<String>,
    pub last_error_detail: Option<String>,
}

impl Default for ProviderRuntimeState {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            mode: ProviderMode::Offline,
            mode_changed_at: now,
            connecting_until: None,
            online_since: None,
            last_heartbeat_at: None,
            heartbeat_interval: Duration::from_secs(1),
            queue_depth: 0,
            last_completed_job_at: None,
            last_result: None,
            degraded_reason_code: None,
            last_error_detail: None,
        }
    }
}

impl ProviderRuntimeState {
    pub fn toggle_online(&mut self, now: Instant, blockers: &[ProviderBlocker]) {
        if self.mode == ProviderMode::Offline {
            self.start_online(now, blockers);
        } else {
            self.go_offline(now);
        }
    }

    pub fn tick(&mut self, now: Instant, blockers: &[ProviderBlocker]) -> bool {
        let mut changed = false;

        if self.mode == ProviderMode::Connecting
            && self.connecting_until.is_some_and(|until| now >= until)
        {
            if blockers.is_empty() {
                self.mode = ProviderMode::Online;
                self.mode_changed_at = now;
                self.connecting_until = None;
                self.online_since = Some(now);
                self.last_heartbeat_at = Some(now);
                self.degraded_reason_code = None;
                self.last_error_detail = None;
            } else {
                self.move_degraded(now, blockers);
            }
            changed = true;
        }

        if self.mode == ProviderMode::Online {
            let should_heartbeat = self
                .last_heartbeat_at
                .is_none_or(|last| now.duration_since(last) >= self.heartbeat_interval);
            if should_heartbeat {
                self.last_heartbeat_at = Some(now);
                changed = true;
            }
        }

        changed
    }

    pub fn uptime_seconds(&self, now: Instant) -> u64 {
        self.online_since
            .and_then(|started| now.checked_duration_since(started))
            .map_or(0, |duration| duration.as_secs())
    }

    pub fn heartbeat_age_seconds(&self, now: Instant) -> Option<u64> {
        self.last_heartbeat_at
            .and_then(|last| now.checked_duration_since(last))
            .map(|duration| duration.as_secs())
    }

    fn start_online(&mut self, now: Instant, blockers: &[ProviderBlocker]) {
        if blockers.is_empty() {
            self.mode = ProviderMode::Connecting;
            self.mode_changed_at = now;
            self.connecting_until = Some(now + Duration::from_millis(900));
            self.degraded_reason_code = None;
            self.last_error_detail = None;
        } else {
            self.move_degraded(now, blockers);
        }
    }

    fn go_offline(&mut self, now: Instant) {
        self.mode = ProviderMode::Offline;
        self.mode_changed_at = now;
        self.connecting_until = None;
        self.online_since = None;
        self.last_heartbeat_at = None;
        self.queue_depth = 0;
        self.degraded_reason_code = None;
        self.last_error_detail = None;
    }

    fn move_degraded(&mut self, now: Instant, blockers: &[ProviderBlocker]) {
        self.mode = ProviderMode::Degraded;
        self.mode_changed_at = now;
        self.connecting_until = None;
        self.online_since = None;
        self.last_heartbeat_at = None;
        self.degraded_reason_code = blockers.first().map(|blocker| blocker.code().to_string());
        self.last_error_detail = Some(
            blockers
                .iter()
                .map(|blocker| blocker.detail())
                .collect::<Vec<_>>()
                .join("; "),
        );
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

pub struct RenderState {
    pub window: Arc<Window>,
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub renderer: Renderer,
    pub text_system: TextSystem,
    pub scale_factor: f32,
    pub hotbar: Hotbar,
    pub hotbar_bounds: Bounds,
    pub event_context: EventContext,
    pub input_modifiers: Modifiers,
    pub panes: Vec<DesktopPane>,
    pub nostr_identity: Option<NostrIdentity>,
    pub nostr_identity_error: Option<String>,
    pub nostr_secret_state: NostrSecretState,
    pub spark_wallet: SparkPaneState,
    pub spark_worker: SparkWalletWorker,
    pub spark_inputs: SparkPaneInputs,
    pub pay_invoice_inputs: PayInvoicePaneInputs,
    pub chat_inputs: ChatPaneInputs,
    pub autopilot_chat: AutopilotChatState,
    pub provider_runtime: ProviderRuntimeState,
    pub next_pane_id: u64,
    pub next_z_index: i32,
    pub pane_drag_mode: Option<PaneDragMode>,
    pub pane_resizer: ResizablePane,
    pub hotbar_flash_was_active: bool,
    pub command_palette: CommandPalette,
    pub command_palette_actions: Rc<RefCell<Vec<String>>>,
}

impl RenderState {
    pub fn provider_blockers(&self) -> Vec<ProviderBlocker> {
        let mut blockers = Vec::new();
        if self.nostr_identity.is_none() {
            blockers.push(ProviderBlocker::IdentityMissing);
        }
        if self.spark_wallet.last_error.is_some() {
            blockers.push(ProviderBlocker::WalletError);
        }
        blockers
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AutopilotChatState, AutopilotMessageStatus, NostrSecretState, ProviderBlocker,
        ProviderMode, ProviderRuntimeState,
    };

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
    fn provider_state_connects_then_becomes_online() {
        let mut provider = ProviderRuntimeState::default();
        let now = std::time::Instant::now();
        provider.toggle_online(now, &[]);
        assert_eq!(provider.mode, ProviderMode::Connecting);

        let advanced = now + std::time::Duration::from_secs(1);
        assert!(provider.tick(advanced, &[]));
        assert_eq!(provider.mode, ProviderMode::Online);
        assert!(provider.online_since.is_some());
        assert!(provider.last_heartbeat_at.is_some());
    }

    #[test]
    fn provider_state_enters_degraded_when_blocked() {
        let mut provider = ProviderRuntimeState::default();
        let now = std::time::Instant::now();
        provider.toggle_online(now, &[ProviderBlocker::IdentityMissing]);
        assert_eq!(provider.mode, ProviderMode::Degraded);
        assert_eq!(
            provider.degraded_reason_code.as_deref(),
            Some(ProviderBlocker::IdentityMissing.code())
        );
    }

    #[test]
    fn chat_state_progresses_queued_to_done() {
        let mut chat = AutopilotChatState::default();
        let now = std::time::Instant::now();
        chat.submit_prompt(now, "ping".to_string());
        assert!(
            chat.messages
                .iter()
                .any(|message| message.status == AutopilotMessageStatus::Queued)
        );

        assert!(chat.tick(now + std::time::Duration::from_millis(300)));
        assert!(
            chat.messages
                .iter()
                .any(|message| message.status == AutopilotMessageStatus::Running)
        );

        assert!(chat.tick(now + std::time::Duration::from_secs(2)));
        assert!(!chat.has_pending_messages());
    }
}
