use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use autopilot_app::{
    AppEvent, DesktopRouteState, DesktopSurfaceRoute, InboxAuditEntry, InboxRoutePane,
    InboxSnapshot, InboxThreadSummary, RuntimeAuthStateView, SessionId, ThreadSnapshot,
    ThreadSummary, UserAction, WorkspaceId,
};
use bip39::Mnemonic;
use editor::{Editor, EditorElement, SyntaxLanguage};
use nostr::{
    derive_keypair, get_public_key, nsec_to_private_key, private_key_to_nsec, public_key_to_npub,
};
use openagents_spark::SparkSigner;
use openagents_ui_core::tokens::palette as ui_palette;
use rand::RngCore;
use serde_json::Value;
use taffy::prelude::{AlignItems, JustifyContent};
use wgpui::components::EventContext;
use wgpui::components::atoms::{ToolStatus, ToolType};
use wgpui::components::hud::{
    DotShape, DotsGrid, Hotbar, HotbarSlot, PaneFrame, ResizablePane, ResizeEdge,
};
use wgpui::components::organisms::{
    AssistantMessage, CodexReasoningCard, DiffLine, DiffLineKind, DiffToolCall, SearchMatch,
    SearchToolCall, TerminalToolCall, ThreadEntry, ThreadEntryType, ToolCallCard, UserMessage,
};
use wgpui::components::sections::{MessageEditor, ThreadView};
use wgpui::components::{Text, TextInput};
use wgpui::input::{InputEvent, Key, Modifiers};
use wgpui::scroll::{ScrollContainer, ScrollDirection, ScrollRegion};
use wgpui::{
    Bounds, Button, ButtonVariant, Component, Cursor, Dropdown, DropdownOption, EventResult, Hsla,
    LayoutEngine, LayoutStyle, MarkdownConfig, MarkdownDocument, MarkdownView, MouseButton,
    PaintContext, Point, Quad, ScrollView, Size, StreamingMarkdown, copy_to_clipboard, length, px,
    text::FontStyle, theme,
};

pub mod shortcuts;
pub use shortcuts::{
    ShortcutBinding, ShortcutChord, ShortcutCommand, ShortcutContext, ShortcutRegistry,
    ShortcutScope,
};

mod constants;
mod layout;
mod render;
mod state;
use constants::*;
use layout::*;
use render::*;
use state::*;

#[derive(Default, Clone)]
pub struct AppViewModel {
    workspace_id: Option<WorkspaceId>,
    workspace_path: Option<PathBuf>,
    session_id: Option<SessionId>,
    session_label: Option<String>,
    last_event: Option<String>,
    event_count: usize,
    sessions: Vec<SessionSummary>,
    event_log: Vec<String>,
}

impl AppViewModel {
    pub fn apply_event(&mut self, event: &AppEvent) {
        self.event_count += 1;
        let formatted = format_event(event);
        self.last_event = Some(formatted.clone());
        self.event_log.push(formatted);
        if self.event_log.len() > 8 {
            let keep = self.event_log.len().saturating_sub(8);
            let _ = self.event_log.drain(0..keep);
        }

        match event {
            AppEvent::WorkspaceOpened { workspace_id, path } => {
                self.workspace_id = Some(*workspace_id);
                self.workspace_path = Some(path.clone());
            }
            AppEvent::SessionStarted {
                session_id, label, ..
            } => {
                self.session_id = Some(*session_id);
                self.session_label = label.clone();
                self.sessions.push(SessionSummary {
                    session_id: *session_id,
                    label: label.clone(),
                });
            }
            AppEvent::UserActionDispatched { .. } => {}
            AppEvent::AppServerEvent { .. } => {}
            AppEvent::PylonStatus { .. } => {}
            AppEvent::WalletStatus { .. } => {}
            AppEvent::LiquidityProviderStatus { .. } => {}
            AppEvent::DvmProviderStatus { .. } => {}
            AppEvent::DvmHistory { .. } => {}
            AppEvent::RuntimeAuthState { .. } => {}
            AppEvent::Nip90Log { .. } => {}
            AppEvent::ThreadsUpdated { .. } => {}
            AppEvent::ThreadLoaded { .. } => {}
            AppEvent::InboxUpdated { .. } => {}
            AppEvent::FileOpened { .. } => {}
            AppEvent::FileOpenFailed { .. } => {}
            AppEvent::FileSaved { .. } => {}
            AppEvent::FileSaveFailed { .. } => {}
        }
    }

    pub fn workspace_path(&self) -> Option<&PathBuf> {
        self.workspace_path.as_ref()
    }

    pub fn session_id(&self) -> Option<SessionId> {
        self.session_id
    }

    pub fn event_count(&self) -> usize {
        self.event_count
    }
}

enum UiAction {
    SendMessage(String),
}

#[derive(Clone, Debug)]
enum ToolEntry {
    Terminal {
        entry_index: usize,
        command: String,
        output: String,
        status: ToolStatus,
        exit_code: Option<i32>,
    },
    Edit {
        entry_index: usize,
        tool_name: String,
        input: String,
        output: String,
        status: ToolStatus,
    },
    Search {
        entry_index: usize,
        query: String,
        matches: Vec<SearchMatch>,
        status: ToolStatus,
    },
    Generic {
        entry_index: usize,
        tool_type: ToolType,
        tool_name: String,
        input: Option<String>,
        output: Option<String>,
        status: ToolStatus,
    },
}

impl ToolEntry {
    fn entry_index(&self) -> usize {
        match self {
            ToolEntry::Terminal { entry_index, .. } => *entry_index,
            ToolEntry::Edit { entry_index, .. } => *entry_index,
            ToolEntry::Search { entry_index, .. } => *entry_index,
            ToolEntry::Generic { entry_index, .. } => *entry_index,
        }
    }
}

fn new_markdown_stream() -> StreamingMarkdown {
    let mut stream = StreamingMarkdown::new();
    let mut markdown_config = MarkdownConfig::default();
    markdown_config.base_font_size = theme::font_size::XS;
    markdown_config.header_sizes = [1.0; 6];
    stream.set_markdown_config(markdown_config);
    stream
}

fn message_markdown_view(document: MarkdownDocument) -> MarkdownView {
    MarkdownView::new(document)
        .show_copy_button(false)
        .copy_button_on_hover(false)
}

fn generate_nip06_keypair() -> Result<(String, String, String, String), String> {
    let mut entropy = [0u8; 16];
    rand::rng().fill_bytes(&mut entropy);
    let mnemonic = Mnemonic::from_entropy(&entropy)
        .map_err(|e| format!("mnemonic error: {e}"))?
        .to_string();
    let keypair =
        derive_keypair(&mnemonic).map_err(|e| format!("keypair derivation error: {e}"))?;
    let npub = keypair
        .npub()
        .map_err(|e| format!("npub encoding error: {e}"))?;
    let nsec = keypair
        .nsec()
        .map_err(|e| format!("nsec encoding error: {e}"))?;
    let spark_signer =
        SparkSigner::from_mnemonic(&mnemonic, "").map_err(|e| format!("spark error: {e}"))?;
    let spark_pubkey = spark_signer.public_key_hex();
    Ok((npub, nsec, spark_pubkey, mnemonic))
}

fn parse_private_key_hex(input: &str) -> Result<[u8; 32], String> {
    let trimmed = input.trim();
    let normalized = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);

    if normalized.len() != 64 {
        return Err("private key hex must be exactly 64 characters".to_string());
    }

    let mut bytes = [0u8; 32];
    let raw = normalized.as_bytes();
    for (index, pair_bytes) in raw.chunks_exact(2).enumerate() {
        let pair = std::str::from_utf8(pair_bytes)
            .map_err(|_| "private key hex contains invalid characters".to_string())?;
        bytes[index] = u8::from_str_radix(pair, 16)
            .map_err(|_| "private key hex contains invalid characters".to_string())?;
    }
    Ok(bytes)
}

fn import_nostr_private_key(input: &str) -> Result<(String, String), String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("enter an nsec or 64-char hex private key".to_string());
    }

    let private_key = if trimmed.starts_with("nsec1") {
        nsec_to_private_key(trimmed).map_err(|e| format!("invalid nsec: {e}"))?
    } else {
        parse_private_key_hex(trimmed)?
    };

    let public_key =
        get_public_key(&private_key).map_err(|e| format!("public key derivation error: {e}"))?;
    let npub = public_key_to_npub(&public_key).map_err(|e| format!("npub encoding error: {e}"))?;
    let nsec =
        private_key_to_nsec(&private_key).map_err(|e| format!("nsec encoding error: {e}"))?;
    Ok((npub, nsec))
}

fn format_seed_phrase(seed: &str) -> String {
    let words: Vec<&str> = seed.split_whitespace().collect();
    if words.is_empty() {
        return String::new();
    }
    words
        .chunks(6)
        .map(|chunk| chunk.join(" "))
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PaneKind {
    Chat,
    Events,
    Threads,
    InboxList,
    InboxThread,
    InboxApprovals,
    InboxAudit,
    FileEditor,
    Auth,
    Identity,
    Pylon,
    Wallet,
    Liquidity,
    SellCompute,
    DvmHistory,
    Nip90,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
enum HotbarAction {
    FocusPane(String),
    ToggleEvents,
    ToggleThreads,
    ToggleInboxList,
    ToggleFileEditor,
    ToggleAuth,
    ToggleIdentity,
    TogglePylon,
    ToggleWallet,
    ToggleSellCompute,
    ToggleDvmHistory,
    ToggleNip90,
    NewChat,
}

#[derive(Clone, Copy, Debug)]
struct PaneRect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

#[derive(Clone, Debug)]
struct Pane {
    id: String,
    kind: PaneKind,
    title: String,
    rect: PaneRect,
    dismissable: bool,
}

#[derive(Clone, Debug)]
struct PaneSnapshot {
    rect: PaneRect,
}

#[derive(Clone, Debug)]
struct PaneDragState {
    pane_id: String,
    origin: Point,
    start_rect: PaneRect,
}

#[derive(Clone, Debug)]
struct CanvasPanState {
    last: Point,
}

#[derive(Clone, Debug)]
struct PaneResizeState {
    pane_id: String,
    edge: ResizeEdge,
    origin: Point,
    start_rect: PaneRect,
}

#[derive(Default)]
struct PaneStore {
    panes: Vec<Pane>,
    active_pane_id: Option<String>,
    last_pane_position: Option<PaneRect>,
    closed_positions: HashMap<String, PaneSnapshot>,
}

impl PaneStore {
    fn is_active(&self, id: &str) -> bool {
        self.active_pane_id.as_deref() == Some(id)
    }

    fn pane(&self, id: &str) -> Option<&Pane> {
        self.panes.iter().find(|pane| pane.id == id)
    }

    fn pane_index(&self, id: &str) -> Option<usize> {
        self.panes.iter().position(|pane| pane.id == id)
    }

    fn panes(&self) -> &[Pane] {
        &self.panes
    }

    fn add_pane(&mut self, pane: Pane) {
        if let Some(index) = self.pane_index(&pane.id) {
            self.active_pane_id = Some(pane.id.clone());
            let pane = self.panes.remove(index);
            self.panes.push(pane);
            return;
        }

        self.active_pane_id = Some(pane.id.clone());
        self.last_pane_position = Some(pane.rect);
        self.panes.push(pane);
    }

    fn remove_pane(&mut self, id: &str, store_position: bool) {
        if let Some(index) = self.pane_index(id) {
            let pane = self.panes.remove(index);
            if store_position {
                self.closed_positions
                    .insert(pane.id.clone(), PaneSnapshot { rect: pane.rect });
            }
            if self.active_pane_id.as_deref() == Some(id) {
                self.active_pane_id = self.panes.last().map(|pane| pane.id.clone());
            }
        }
    }

    fn bring_to_front(&mut self, id: &str) {
        if let Some(index) = self.pane_index(id) {
            let pane = self.panes.remove(index);
            self.active_pane_id = Some(pane.id.clone());
            self.panes.push(pane);
        }
    }

    fn update_rect(&mut self, id: &str, rect: PaneRect) {
        if let Some(index) = self.pane_index(id) {
            self.panes[index].rect = rect;
        }
    }

    fn set_title(&mut self, id: &str, title: impl Into<String>) {
        if let Some(index) = self.pane_index(id) {
            self.panes[index].title = title.into();
        }
    }

    fn offset_all(&mut self, dx: f32, dy: f32) {
        if dx == 0.0 && dy == 0.0 {
            return;
        }
        for pane in &mut self.panes {
            pane.rect.x += dx;
            pane.rect.y += dy;
        }
        if let Some(last) = self.last_pane_position.as_mut() {
            last.x += dx;
            last.y += dy;
        }
    }

    fn set_last_position(&mut self, rect: PaneRect) {
        self.last_pane_position = Some(rect);
    }

    fn toggle_pane<F>(&mut self, id: &str, _screen: Size, mut create: F)
    where
        F: FnMut(Option<PaneSnapshot>) -> Pane,
    {
        if let Some(index) = self.pane_index(id) {
            let is_active = self.active_pane_id.as_deref() == Some(id);
            if is_active {
                self.remove_pane(id, true);
            } else {
                let pane = self.panes.remove(index);
                self.active_pane_id = Some(pane.id.clone());
                self.panes.push(pane);
            }
            return;
        }

        let snapshot = self.closed_positions.get(id).cloned();
        let mut pane = create(snapshot);
        pane.rect = normalize_pane_rect(pane.rect);
        self.add_pane(pane);
    }
}

pub struct DesktopRoot {
    view_model: AppViewModel,
    event_context: EventContext,
    left_header: Text,
    center_header: Text,
    right_header: Text,
    session_rows: Vec<SessionRow>,
    session_scroll: ScrollView,
    thread_view: ThreadView,
    message_editor: MessageEditor,
    status_scroll: ScrollView,
    pending_actions: Rc<RefCell<Vec<UiAction>>>,
    send_handler: Option<Box<dyn FnMut(UserAction)>>,
    cursor_position: Point,
}

pub struct MinimalRoot {
    event_context: EventContext,
    cursor_position: Point,
    screen_size: Size,
    zoom_factor: f32,
    pane_store: PaneStore,
    pane_frames: HashMap<String, PaneFrame>,
    pane_bounds: HashMap<String, Bounds>,
    pane_drag: Option<PaneDragState>,
    pane_resize: Option<PaneResizeState>,
    canvas_pan: Option<CanvasPanState>,
    background_offset: Point,
    pane_resizer: ResizablePane,
    chat_panes: HashMap<String, ChatPaneState>,
    chat_slot_assignments: HashMap<String, u8>,
    chat_slot_labels: HashMap<String, String>,
    pending_session_panes: VecDeque<String>,
    session_to_pane: HashMap<SessionId, String>,
    thread_to_pane: HashMap<String, String>,
    next_chat_index: u64,
    hotbar: Hotbar,
    hotbar_bounds: Bounds,
    copy_button: Button,
    copy_bounds: Bounds,
    pending_copy: Rc<RefCell<bool>>,
    copy_feedback_until: Option<Instant>,
    copy_feedback_duration: Duration,
    threads_refresh_button: Button,
    threads_refresh_bounds: Bounds,
    pending_threads_refresh: Rc<RefCell<bool>>,
    threads_load_more_button: Button,
    threads_load_more_bounds: Bounds,
    pending_threads_load_more: Rc<RefCell<bool>>,
    threads_next_cursor: Option<String>,
    thread_entries: Vec<ThreadEntryView>,
    pending_thread_open: Rc<RefCell<Option<String>>>,
    inbox: InboxPaneState,
    route_state: DesktopRouteState,
    inbox_refresh_button: Button,
    inbox_refresh_bounds: Bounds,
    inbox_open_thread_button: Button,
    inbox_open_thread_bounds: Bounds,
    inbox_open_approvals_button: Button,
    inbox_open_approvals_bounds: Bounds,
    inbox_open_audit_button: Button,
    inbox_open_audit_bounds: Bounds,
    inbox_approve_button: Button,
    inbox_approve_bounds: Bounds,
    inbox_reject_button: Button,
    inbox_reject_bounds: Bounds,
    pending_inbox_refresh: Rc<RefCell<bool>>,
    pending_inbox_select_thread: Rc<RefCell<Option<String>>>,
    pending_inbox_open_thread_pane: Rc<RefCell<bool>>,
    pending_inbox_open_approvals_pane: Rc<RefCell<bool>>,
    pending_inbox_open_audit_pane: Rc<RefCell<bool>>,
    pending_inbox_approve: Rc<RefCell<bool>>,
    pending_inbox_reject: Rc<RefCell<bool>>,
    file_editor: FileEditorPaneState,
    runtime_auth: RuntimeAuthStateView,
    runtime_auth_email_input: TextInput,
    runtime_auth_email_bounds: Bounds,
    runtime_auth_code_input: TextInput,
    runtime_auth_code_bounds: Bounds,
    runtime_auth_send_button: Button,
    runtime_auth_send_bounds: Bounds,
    runtime_auth_verify_button: Button,
    runtime_auth_verify_bounds: Bounds,
    runtime_auth_status_button: Button,
    runtime_auth_status_bounds: Bounds,
    runtime_auth_logout_button: Button,
    runtime_auth_logout_bounds: Bounds,
    pending_runtime_auth_send: Rc<RefCell<bool>>,
    pending_runtime_auth_verify: Rc<RefCell<bool>>,
    pending_runtime_auth_status: Rc<RefCell<bool>>,
    pending_runtime_auth_logout: Rc<RefCell<bool>>,
    identity_private_key_input: TextInput,
    identity_private_key_bounds: Bounds,
    identity_load_button: Button,
    identity_load_bounds: Bounds,
    pending_identity_load: Rc<RefCell<bool>>,
    keygen_button: Button,
    keygen_bounds: Bounds,
    pending_keygen: Rc<RefCell<bool>>,
    identity_copy_npub_button: Button,
    identity_copy_npub_bounds: Bounds,
    pending_identity_copy_npub: Rc<RefCell<bool>>,
    identity_copy_nsec_button: Button,
    identity_copy_nsec_bounds: Bounds,
    pending_identity_copy_nsec: Rc<RefCell<bool>>,
    pylon_status: PylonStatusView,
    pylon_toggle_button: Button,
    pylon_toggle_bounds: Bounds,
    #[allow(dead_code)]
    pylon_init_button: Button,
    #[allow(dead_code)]
    pylon_start_button: Button,
    #[allow(dead_code)]
    pylon_stop_button: Button,
    #[allow(dead_code)]
    pylon_refresh_button: Button,
    pylon_init_bounds: Bounds,
    pylon_start_bounds: Bounds,
    pylon_stop_bounds: Bounds,
    pylon_refresh_bounds: Bounds,
    pending_pylon_toggle: Rc<RefCell<bool>>,
    pending_pylon_init: Rc<RefCell<bool>>,
    pending_pylon_start: Rc<RefCell<bool>>,
    pending_pylon_stop: Rc<RefCell<bool>>,
    pending_pylon_refresh: Rc<RefCell<bool>>,
    wallet_status: WalletStatusView,
    wallet_refresh_button: Button,
    wallet_refresh_bounds: Bounds,
    wallet_liquidity_button: Button,
    wallet_liquidity_bounds: Bounds,
    pending_wallet_refresh: Rc<RefCell<bool>>,
    pending_wallet_liquidity: Rc<RefCell<bool>>,
    liquidity_status: LiquidityProviderStatusView,
    liquidity_online_button: Button,
    liquidity_offline_button: Button,
    liquidity_refresh_button: Button,
    liquidity_online_bounds: Bounds,
    liquidity_offline_bounds: Bounds,
    liquidity_refresh_bounds: Bounds,
    pending_liquidity_online: Rc<RefCell<bool>>,
    pending_liquidity_offline: Rc<RefCell<bool>>,
    pending_liquidity_refresh: Rc<RefCell<bool>>,
    liquidity_invoice_amount_input: TextInput,
    liquidity_invoice_amount_bounds: Bounds,
    liquidity_invoice_create_button: Button,
    liquidity_invoice_create_bounds: Bounds,
    pending_liquidity_invoice_create: Rc<RefCell<bool>>,
    liquidity_invoice_copy_button: Button,
    liquidity_invoice_copy_bounds: Bounds,
    pending_liquidity_invoice_copy: Rc<RefCell<bool>>,
    sell_compute_status: SellComputeStatusView,
    sell_compute_online_button: Button,
    sell_compute_offline_button: Button,
    sell_compute_refresh_button: Button,
    sell_compute_online_bounds: Bounds,
    sell_compute_offline_bounds: Bounds,
    sell_compute_refresh_bounds: Bounds,
    pending_sell_compute_online: Rc<RefCell<bool>>,
    pending_sell_compute_offline: Rc<RefCell<bool>>,
    pending_sell_compute_refresh: Rc<RefCell<bool>>,
    dvm_history: DvmHistoryView,
    dvm_history_refresh_button: Button,
    dvm_history_refresh_bounds: Bounds,
    pending_dvm_history_refresh: Rc<RefCell<bool>>,
    nip90_kind_input: TextInput,
    nip90_kind_bounds: Bounds,
    nip90_relay_input: TextInput,
    nip90_relay_bounds: Bounds,
    nip90_provider_input: TextInput,
    nip90_provider_bounds: Bounds,
    nip90_prompt_input: TextInput,
    nip90_prompt_bounds: Bounds,
    nip90_submit_button: Button,
    nip90_submit_bounds: Bounds,
    pending_nip90_submit: Rc<RefCell<bool>>,
    nip90_log: Vec<String>,
    nostr_npub: Option<String>,
    nostr_nsec: Option<String>,
    spark_pubkey_hex: Option<String>,
    seed_phrase: Option<String>,
    nostr_error: Option<String>,
    send_handler: Option<Box<dyn FnMut(UserAction)>>,
    event_log: Vec<String>,
    event_log_dirty: bool,
    event_scroll: ScrollView,
    event_scroll_bounds: Bounds,
    hotbar_bindings: HashMap<u8, HotbarAction>,
}

#[derive(Clone, Debug)]
struct QueuedMessage {
    text: String,
}

#[derive(Clone, Debug)]
struct ReasoningEntry {
    summary: String,
    content: String,
    entry_index: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AgentDeltaSource {
    Modern,
    Legacy,
}

struct ChatPaneState {
    session_id: Option<SessionId>,
    thread_id: Option<String>,
    thread_model: Option<String>,
    active_turn_id: Option<String>,
    model_dropdown: Dropdown,
    model_bounds: Bounds,
    model_hovered: bool,
    pending_model_changes: Rc<RefCell<Vec<String>>>,
    selected_model: String,
    reasoning_dropdown: Dropdown,
    reasoning_bounds: Bounds,
    reasoning_hovered: bool,
    pending_reasoning_changes: Rc<RefCell<Vec<String>>>,
    selected_reasoning: String,
    queue_toggle_button: Button,
    queue_toggle_bounds: Bounds,
    pending_queue_toggle: Rc<RefCell<bool>>,
    queue_mode: bool,
    formatted_thread: ThreadView,
    formatted_thread_bounds: Bounds,
    formatted_message_streams: HashMap<String, StreamingMarkdown>,
    formatted_message_entries: HashMap<String, usize>,
    agent_delta_aliases: HashMap<String, AgentDeltaSource>,
    reasoning_entries: HashMap<String, ReasoningEntry>,
    tool_entries: HashMap<String, ToolEntry>,
    last_user_message: Option<String>,
    working_entry_index: Option<usize>,
    input: TextInput,
    input_bounds: Bounds,
    input_hovered: bool,
    input_needs_focus: bool,
    submit_button: Button,
    submit_bounds: Bounds,
    pending_sends: Rc<RefCell<Vec<String>>>,
    full_auto_button: Button,
    full_auto_bounds: Bounds,
    pending_full_auto: Rc<RefCell<bool>>,
    full_auto_enabled: bool,
    guidance_mode_active: bool,
    queued_messages: Vec<QueuedMessage>,
    queued_in_flight: bool,
    stop_button: Button,
    stop_bounds: Bounds,
    pending_stop: Rc<RefCell<bool>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SplitDirection {
    None,
    Horizontal,
    Vertical,
}

struct EditorTab {
    path: PathBuf,
    title: String,
    editor: EditorElement,
    saved_revision: u64,
    loading: bool,
}

struct TabHit {
    tab_id: usize,
    bounds: Bounds,
}

struct EditorGroup {
    tabs: Vec<usize>,
    active_tab: Option<usize>,
    group_bounds: Bounds,
    tab_bar_bounds: Bounds,
    editor_bounds: Bounds,
    tab_hits: Vec<TabHit>,
}

struct FileNode {
    path: PathBuf,
    name: String,
    is_dir: bool,
    expanded: bool,
    children: Vec<FileNode>,
}

struct FileTreeRow {
    path: PathBuf,
    depth: usize,
    is_dir: bool,
    expanded: bool,
    bounds: Bounds,
}

struct PendingOpenRequest {
    path: PathBuf,
    target_group: usize,
}

struct FileEditorPaneState {
    workspace_root: Option<PathBuf>,
    tree_root: Option<FileNode>,
    tree_rows: Vec<FileTreeRow>,
    tree_scroll: ScrollContainer,
    tree_bounds: Bounds,
    tree_header_bounds: Bounds,
    tree_refresh_button: Button,
    tree_refresh_bounds: Bounds,
    pending_tree_refresh: Rc<RefCell<bool>>,
    selected_tree_path: Option<PathBuf>,
    path_input: TextInput,
    open_button: Button,
    reload_button: Button,
    save_button: Button,
    split_horizontal_button: Button,
    split_vertical_button: Button,
    path_bounds: Bounds,
    open_bounds: Bounds,
    reload_bounds: Bounds,
    save_bounds: Bounds,
    split_h_bounds: Bounds,
    split_v_bounds: Bounds,
    status: Option<String>,
    status_is_error: bool,
    tabs: HashMap<usize, EditorTab>,
    tab_by_path: HashMap<PathBuf, usize>,
    groups: Vec<EditorGroup>,
    active_group: usize,
    split_direction: SplitDirection,
    next_tab_id: usize,
    pending_open: Rc<RefCell<bool>>,
    pending_reload: Rc<RefCell<bool>>,
    pending_save: Rc<RefCell<bool>>,
    pending_split: Rc<RefCell<Option<SplitDirection>>>,
    pending_open_requests: Vec<PendingOpenRequest>,
    pending_open_dispatches: Vec<PathBuf>,
}

#[derive(Clone, Debug, Default)]
struct InboxPaneState {
    threads: Vec<InboxThreadSummary>,
    selected_thread_id: Option<String>,
    audit_log: Vec<InboxAuditEntry>,
    source: Option<String>,
    list_row_bounds: Vec<(String, Bounds)>,
}

impl InboxPaneState {
    fn apply_snapshot(&mut self, snapshot: InboxSnapshot, source: String) {
        self.threads = snapshot.threads;
        self.selected_thread_id = snapshot.selected_thread_id;
        self.audit_log = snapshot.audit_log;
        self.source = Some(source);
        self.list_row_bounds.clear();
    }

    fn selected_thread(&self) -> Option<&InboxThreadSummary> {
        let selected_id = self.selected_thread_id.as_deref()?;
        self.threads.iter().find(|thread| thread.id == selected_id)
    }

    fn pending_threads(&self) -> Vec<&InboxThreadSummary> {
        self.threads
            .iter()
            .filter(|thread| thread.pending_approval)
            .collect()
    }

    fn row_at(&self, point: Point) -> Option<String> {
        self.list_row_bounds
            .iter()
            .find(|(_, bounds)| bounds.contains(point))
            .map(|(thread_id, _)| thread_id.clone())
    }
}

impl ChatPaneState {
    fn new(default_model: &str) -> Self {
        let pending_model_changes = Rc::new(RefCell::new(Vec::new()));
        let pending_models = pending_model_changes.clone();
        let selected_index = model_index(default_model).unwrap_or(DEFAULT_MODEL_INDEX);
        let model_dropdown = Dropdown::new(build_model_options())
            .selected(selected_index)
            .font_size(theme::font_size::SM)
            .padding(12.0, 6.0)
            .open_up(true)
            .on_change(move |_, value| {
                pending_models.borrow_mut().push(value.to_string());
            });

        let pending_reasoning_changes = Rc::new(RefCell::new(Vec::new()));
        let pending_reasoning = pending_reasoning_changes.clone();
        let reasoning_options = build_reasoning_options(default_model);
        let reasoning_default = default_reasoning_for_model(default_model);
        let reasoning_index = reasoning_index(default_model, reasoning_default).unwrap_or(0);
        let reasoning_dropdown = Dropdown::new(reasoning_options)
            .selected(reasoning_index)
            .font_size(theme::font_size::SM)
            .padding(12.0, 6.0)
            .open_up(true)
            .on_change(move |_, value| {
                pending_reasoning.borrow_mut().push(value.to_string());
            });

        let pending_queue_toggle = Rc::new(RefCell::new(false));
        let pending_queue_toggle_click = pending_queue_toggle.clone();
        let queue_toggle_button = Button::new("Queue")
            .variant(ButtonVariant::Primary)
            .font_size(theme::font_size::SM)
            .padding(12.0, 6.0)
            .corner_radius(8.0)
            .on_click(move || {
                *pending_queue_toggle_click.borrow_mut() = true;
            });

        let formatted_thread = ThreadView::new().item_spacing(8.0);

        let pending_sends = Rc::new(RefCell::new(Vec::new()));
        let pending_submit = pending_sends.clone();
        let input = TextInput::new()
            .placeholder("Send message to Codex…")
            .background(ui_palette::canvas_bg())
            .border_color(theme::border::DEFAULT)
            .border_color_focused(theme::border::FOCUS)
            .text_color(theme::text::PRIMARY)
            .placeholder_color(theme::text::MUTED)
            .on_submit(move |value| {
                if !value.trim().is_empty() {
                    pending_submit.borrow_mut().push(value.to_string());
                }
            });

        let pending_send_clicks = pending_sends.clone();
        let submit_button = Button::new("Send")
            .font_size(theme::font_size::SM)
            .padding(14.0, 8.0)
            .corner_radius(8.0)
            .background(theme::accent::PRIMARY)
            .text_color(theme::bg::APP)
            .on_click(move || {
                pending_send_clicks.borrow_mut().push(String::new());
            });

        let pending_full_auto = Rc::new(RefCell::new(false));
        let pending_full_auto_click = pending_full_auto.clone();
        let full_auto_button = Button::new("Full Auto")
            .variant(ButtonVariant::Secondary)
            .font_size(theme::font_size::XS)
            .padding(12.0, 6.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_full_auto_click.borrow_mut() = true;
            });

        let pending_stop = Rc::new(RefCell::new(false));
        let pending_stop_click = pending_stop.clone();
        let stop_button = Button::new("Stop")
            .variant(ButtonVariant::Danger)
            .font_size(theme::font_size::SM)
            .padding(14.0, 8.0)
            .corner_radius(8.0)
            .on_click(move || {
                *pending_stop_click.borrow_mut() = true;
            });

        Self {
            session_id: None,
            thread_id: None,
            thread_model: Some(default_model.to_string()),
            active_turn_id: None,
            model_dropdown,
            model_bounds: Bounds::ZERO,
            model_hovered: false,
            pending_model_changes,
            selected_model: default_model.to_string(),
            reasoning_dropdown,
            reasoning_bounds: Bounds::ZERO,
            reasoning_hovered: false,
            pending_reasoning_changes,
            selected_reasoning: reasoning_default.to_string(),
            queue_toggle_button,
            queue_toggle_bounds: Bounds::ZERO,
            pending_queue_toggle,
            queue_mode: true,
            formatted_thread,
            formatted_thread_bounds: Bounds::ZERO,
            formatted_message_streams: HashMap::new(),
            formatted_message_entries: HashMap::new(),
            agent_delta_aliases: HashMap::new(),
            reasoning_entries: HashMap::new(),
            tool_entries: HashMap::new(),
            last_user_message: None,
            working_entry_index: None,
            input,
            input_bounds: Bounds::ZERO,
            input_hovered: false,
            input_needs_focus: true,
            submit_button,
            submit_bounds: Bounds::ZERO,
            pending_sends,
            full_auto_button,
            full_auto_bounds: Bounds::ZERO,
            pending_full_auto,
            full_auto_enabled: false,
            guidance_mode_active: false,
            queued_messages: Vec::new(),
            queued_in_flight: false,
            stop_button,
            stop_bounds: Bounds::ZERO,
            pending_stop,
        }
    }

    fn reset_chat_state(&mut self) {
        self.formatted_thread.clear();
        self.formatted_message_streams.clear();
        self.formatted_message_entries.clear();
        self.agent_delta_aliases.clear();
        self.reasoning_entries.clear();
        self.tool_entries.clear();
        self.last_user_message = None;
        self.working_entry_index = None;
        self.queued_messages.clear();
        self.queued_in_flight = false;
        self.active_turn_id = None;
        self.thread_id = None;
        self.thread_model = Some(self.selected_model.clone());
        self.full_auto_enabled = false;
        self.guidance_mode_active = false;
        self.input.set_value("");
        self.input_needs_focus = true;
        self.submit_button
            .set_disabled(self.input.get_value().trim().is_empty());
    }

    fn load_thread_snapshot(&mut self, thread: &ThreadSnapshot) {
        self.formatted_thread.clear();
        self.formatted_message_streams.clear();
        self.formatted_message_entries.clear();
        self.agent_delta_aliases.clear();
        self.reasoning_entries.clear();
        self.tool_entries.clear();
        self.last_user_message = None;
        self.working_entry_index = None;
        self.queued_messages.clear();
        self.queued_in_flight = false;
        self.active_turn_id = None;
        self.thread_id = Some(thread.id.clone());
        self.input.set_value("");
        self.input_needs_focus = true;
        self.submit_button
            .set_disabled(self.input.get_value().trim().is_empty());

        for turn in &thread.turns {
            for item in &turn.items {
                self.append_snapshot_item(item);
            }
        }
    }

    fn append_snapshot_item(&mut self, item: &Value) {
        let item_type = item
            .get("type")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        match item_type {
            "UserMessage" | "userMessage" => {
                if let Some(text) = extract_message_text(item) {
                    self.append_user_message(&text);
                }
            }
            "AgentMessage" | "agentMessage" | "assistantMessage" => {
                if let Some(text) = extract_message_text(item) {
                    self.append_agent_text(&text);
                }
            }
            "Reasoning" | "reasoning" => {
                if let Some(summary) = item
                    .get("summary_text")
                    .and_then(|value| value.as_array())
                    .and_then(|items| items.first())
                    .and_then(|value| value.as_str())
                {
                    let mut stream = new_markdown_stream();
                    stream.append(summary);
                    stream.complete();
                    let view = message_markdown_view(stream.document().clone());
                    let entry = ThreadEntry::new(ThreadEntryType::Assistant, view)
                        .copyable_text(summary.to_string());
                    self.formatted_thread.push_entry(entry);
                }
            }
            _ => {}
        }
    }

    fn take_pending_sends(&mut self) -> Vec<String> {
        let mut pending = self.pending_sends.borrow_mut();
        let items = pending.clone();
        pending.clear();
        items
    }

    fn take_pending_queue_toggle(&mut self) -> bool {
        let mut pending = self.pending_queue_toggle.borrow_mut();
        let value = *pending;
        *pending = false;
        value
    }

    fn take_pending_stop(&mut self) -> bool {
        let mut pending = self.pending_stop.borrow_mut();
        let value = *pending;
        *pending = false;
        value
    }

    fn take_pending_full_auto(&mut self) -> bool {
        let mut pending = self.pending_full_auto.borrow_mut();
        let value = *pending;
        *pending = false;
        value
    }

    fn take_pending_models(&mut self) -> Vec<String> {
        let mut pending = self.pending_model_changes.borrow_mut();
        let models = pending.clone();
        pending.clear();
        models
    }

    fn take_pending_reasoning(&mut self) -> Vec<String> {
        let mut pending = self.pending_reasoning_changes.borrow_mut();
        let efforts = pending.clone();
        pending.clear();
        efforts
    }

    fn set_session_id(&mut self, session_id: SessionId) {
        self.session_id = Some(session_id);
    }

    fn update_thread_model(&mut self, model: &str) {
        self.thread_model = Some(model.to_string());
        self.selected_model = model.to_string();
        if let Some(index) = model_index(model) {
            self.model_dropdown.set_selected(Some(index));
        }
        self.refresh_reasoning_options(model);
    }

    fn update_reasoning(&mut self, effort: &str) {
        self.selected_reasoning = effort.to_string();
        if let Some(index) = reasoning_index(&self.selected_model, effort) {
            self.reasoning_dropdown.set_selected(Some(index));
        }
    }

    fn refresh_reasoning_options(&mut self, model: &str) {
        let options = build_reasoning_options(model);
        self.reasoning_dropdown.set_options(options);
        let default_effort = default_reasoning_for_model(model);
        let next_effort = if reasoning_index(model, &self.selected_reasoning).is_some() {
            self.selected_reasoning.clone()
        } else {
            default_effort.to_string()
        };
        self.selected_reasoning = next_effort.clone();
        if let Some(index) = reasoning_index(model, &next_effort) {
            self.reasoning_dropdown.set_selected(Some(index));
        }
    }

    fn is_processing(&self) -> bool {
        self.active_turn_id.is_some() || self.queued_in_flight
    }

    fn current_queue(&self) -> &[QueuedMessage] {
        self.queued_messages.as_slice()
    }

    fn queue_message(&mut self, text: String) {
        self.queued_messages.push(QueuedMessage { text });
    }

    fn can_dispatch(&self, send_handler: &Option<Box<dyn FnMut(UserAction)>>) -> bool {
        self.session_id.is_some() && self.thread_id.is_some() && send_handler.is_some()
    }

    fn dispatch_message(
        &mut self,
        text: String,
        send_handler: &mut Option<Box<dyn FnMut(UserAction)>>,
    ) {
        let Some(session_id) = self.session_id else {
            return;
        };
        if self.thread_id.is_none() {
            return;
        }
        let Some(handler) = send_handler.as_mut() else {
            return;
        };
        handler(UserAction::Message {
            session_id,
            text,
            model: Some(self.selected_model.clone()),
            reasoning: Some(self.selected_reasoning.clone()),
        });
        if self.full_auto_enabled {
            self.activate_guidance_mode();
        }
        self.queued_in_flight = true;
    }

    fn activate_guidance_mode(&mut self) {
        if self.guidance_mode_active {
            return;
        }
        self.guidance_mode_active = true;
        self.append_agent_text("GUIDANCE MODE");
    }

    fn dispatch_or_queue_message(
        &mut self,
        text: String,
        send_handler: &mut Option<Box<dyn FnMut(UserAction)>>,
    ) {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return;
        }

        if !self.queued_messages.is_empty() {
            self.queue_message(trimmed.to_string());
            self.flush_queue_if_idle(send_handler);
            return;
        }

        let should_queue = self.queue_mode && self.is_processing();
        if should_queue || !self.can_dispatch(send_handler) {
            self.queue_message(trimmed.to_string());
            return;
        }

        self.dispatch_message(trimmed.to_string(), send_handler);
    }

    fn flush_queue_if_idle(&mut self, send_handler: &mut Option<Box<dyn FnMut(UserAction)>>) {
        if self.is_processing() {
            return;
        }
        if self.thread_id.is_none() {
            return;
        }
        if self.queued_messages.is_empty() {
            return;
        }
        if !self.can_dispatch(send_handler) {
            return;
        }
        let next = self.queued_messages.remove(0);
        self.dispatch_message(next.text, send_handler);
    }

    fn apply_formatted_event(&mut self, value: &Value) {
        let Some(method) = value.get("method").and_then(|m| m.as_str()) else {
            return;
        };

        match method {
            "item/started" => {
                if let Some(item) = value.get("params").and_then(|p| p.get("item")) {
                    if let Some(item_type) = item.get("type").and_then(|t| t.as_str()) {
                        match item_type {
                            "userMessage" => {
                                if let Some(text) = extract_message_text(item) {
                                    self.append_user_message(&text);
                                }
                            }
                            "reasoning" | "Reasoning" => {
                                if let Some(item_id) = item_id(item) {
                                    self.ensure_reasoning_entry(&item_id);
                                }
                            }
                            "commandExecution" | "fileChange" | "mcpToolCall" | "webSearch" => {
                                self.start_tool_entry(item_type, item);
                            }
                            _ => {}
                        }
                    }
                }
            }
            "item/completed" => {
                if let Some(item) = value.get("params").and_then(|p| p.get("item")) {
                    if let Some(item_type) = item.get("type").and_then(|t| t.as_str()) {
                        match item_type {
                            "AgentMessage" | "agentMessage" => {
                                let text = extract_message_text(item);
                                if let Some(item_id) = item_id(item) {
                                    self.finish_agent_message(&item_id, text.as_deref());
                                } else if let Some(text) = text {
                                    self.append_agent_text(&text);
                                }
                                self.queued_in_flight = false;
                            }
                            "reasoning" | "Reasoning" => {
                                if let Some(item_id) = item_id(item) {
                                    let (has_summary, has_content) = self
                                        .reasoning_entries
                                        .get(&item_id)
                                        .map(|entry| {
                                            (
                                                !entry.summary.trim().is_empty(),
                                                !entry.content.trim().is_empty(),
                                            )
                                        })
                                        .unwrap_or((false, false));
                                    let summary_text = item
                                        .get("summary")
                                        .or_else(|| item.get("summary_text"))
                                        .and_then(|value| value.as_array())
                                        .map(|parts| {
                                            parts
                                                .iter()
                                                .filter_map(|part| part.as_str())
                                                .collect::<Vec<_>>()
                                                .join("\n")
                                        })
                                        .unwrap_or_default();
                                    let content_text = item
                                        .get("content")
                                        .or_else(|| item.get("raw_content"))
                                        .and_then(|value| value.as_array())
                                        .map(|parts| {
                                            parts
                                                .iter()
                                                .filter_map(|part| part.as_str())
                                                .collect::<Vec<_>>()
                                                .join("\n")
                                        })
                                        .unwrap_or_default();
                                    if !summary_text.is_empty() && !has_summary {
                                        self.append_reasoning_summary_delta(
                                            &item_id,
                                            &summary_text,
                                        );
                                    }
                                    if !content_text.is_empty() && !has_content {
                                        self.append_reasoning_content_delta(
                                            &item_id,
                                            &content_text,
                                        );
                                    }
                                }
                            }
                            "commandExecution" | "fileChange" | "mcpToolCall" | "webSearch" => {
                                self.complete_tool_entry(item_type, item);
                            }
                            _ => {}
                        }
                    }
                }
            }
            "codex/event/item_started" => {
                if let Some(item) = value
                    .get("params")
                    .and_then(|params| params.get("msg"))
                    .and_then(|msg| msg.get("item"))
                    && let Some(item_type) = item.get("type").and_then(|t| t.as_str())
                {
                    match item_type {
                        "userMessage" | "UserMessage" => {
                            if let Some(text) = extract_message_text(item) {
                                self.append_user_message(&text);
                            }
                        }
                        "AgentMessage" | "agentMessage" => {
                            if let Some(item_id) = item_id(item) {
                                self.ensure_agent_entry(&item_id);
                            }
                        }
                        "reasoning" | "Reasoning" => {
                            if let Some(item_id) = item_id(item) {
                                self.ensure_reasoning_entry(&item_id);
                            }
                        }
                        "commandExecution" | "fileChange" | "mcpToolCall" | "webSearch" => {
                            self.start_tool_entry(item_type, item);
                        }
                        _ => {}
                    }
                }
            }
            "codex/event/item_completed" => {
                if let Some(item) = value
                    .get("params")
                    .and_then(|params| params.get("msg"))
                    .and_then(|msg| msg.get("item"))
                    && let Some(item_type) = item.get("type").and_then(|t| t.as_str())
                {
                    match item_type {
                        "AgentMessage" | "agentMessage" => {
                            let text = extract_message_text(item);
                            if let Some(item_id) = item_id(item) {
                                self.finish_agent_message(&item_id, text.as_deref());
                            } else if let Some(text) = text {
                                self.append_agent_text(&text);
                            }
                            self.queued_in_flight = false;
                        }
                        "reasoning" | "Reasoning" => {
                            if let Some(item_id) = item_id(item) {
                                let summary_text = item
                                    .get("summary")
                                    .or_else(|| item.get("summary_text"))
                                    .and_then(|value| value.as_array())
                                    .map(|parts| {
                                        parts
                                            .iter()
                                            .filter_map(|part| part.as_str())
                                            .collect::<Vec<_>>()
                                            .join("\n")
                                    })
                                    .unwrap_or_default();
                                let content_text = item
                                    .get("content")
                                    .or_else(|| item.get("raw_content"))
                                    .and_then(|value| value.as_array())
                                    .map(|parts| {
                                        parts
                                            .iter()
                                            .filter_map(|part| part.as_str())
                                            .collect::<Vec<_>>()
                                            .join("\n")
                                    })
                                    .unwrap_or_default();
                                if !summary_text.is_empty() {
                                    self.append_reasoning_summary_delta(&item_id, &summary_text);
                                }
                                if !content_text.is_empty() {
                                    self.append_reasoning_content_delta(&item_id, &content_text);
                                }
                            }
                        }
                        "commandExecution" | "fileChange" | "mcpToolCall" | "webSearch" => {
                            self.complete_tool_entry(item_type, item);
                        }
                        _ => {}
                    }
                }
            }
            "item/agentMessage/delta" => {
                if let Some(params) = value.get("params")
                    && let Some(delta) = params.get("delta").and_then(|d| d.as_str())
                {
                    let item_id = params
                        .get("itemId")
                        .or_else(|| params.get("item_id"))
                        .and_then(|id| id.as_str());
                    if let Some(item_id) = item_id {
                        self.append_agent_delta_from_source(
                            AgentDeltaSource::Modern,
                            item_id,
                            delta,
                        );
                    }
                }
            }
            "codex/event/agent_message_content_delta" | "codex/event/agent_message_delta" => {
                if let Some(msg) = value.get("params").and_then(|params| params.get("msg"))
                    && let Some(delta) = msg.get("delta").and_then(|d| d.as_str())
                {
                    let item_id = msg
                        .get("item_id")
                        .or_else(|| msg.get("itemId"))
                        .and_then(|id| id.as_str());
                    if let Some(item_id) = item_id {
                        self.append_agent_delta_from_source(
                            AgentDeltaSource::Legacy,
                            item_id,
                            delta,
                        );
                    }
                }
            }
            "item/reasoning/summaryTextDelta" => {
                if let Some(params) = value.get("params")
                    && let Some(delta) = params.get("delta").and_then(|d| d.as_str())
                    && let Some(item_id) = params
                        .get("itemId")
                        .or_else(|| params.get("item_id"))
                        .and_then(|id| id.as_str())
                {
                    self.append_reasoning_summary_delta(item_id, delta);
                }
            }
            "item/reasoning/contentDelta" => {
                if let Some(params) = value.get("params")
                    && let Some(delta) = params.get("delta").and_then(|d| d.as_str())
                    && let Some(item_id) = params
                        .get("itemId")
                        .or_else(|| params.get("item_id"))
                        .and_then(|id| id.as_str())
                {
                    self.append_reasoning_content_delta(item_id, delta);
                }
            }
            "item/commandExecution/outputDelta" | "item/fileChange/outputDelta" => {
                if let Some(params) = value.get("params")
                    && let Some(delta) = params.get("delta").and_then(|d| d.as_str())
                {
                    let item_id = params
                        .get("itemId")
                        .or_else(|| params.get("item_id"))
                        .and_then(|id| id.as_str());
                    if let Some(item_id) = item_id {
                        self.append_tool_output(item_id, delta);
                    }
                }
            }
            "codex/event/user_message" => {
                if let Some(text) = value
                    .get("params")
                    .and_then(|params| params.get("msg"))
                    .and_then(|msg| msg.get("message"))
                    .and_then(|m| m.as_str())
                {
                    self.append_user_message(text);
                }
            }
            "codex/event/agent_message" => {
                self.queued_in_flight = false;
                self.clear_working_indicator();
            }
            "guidance/step" => {
                if let Some(params) = value.get("params") {
                    let signature = params.get("signature").and_then(|value| value.as_str());
                    let model = params.get("model").and_then(|value| value.as_str());
                    let text = params.get("text").and_then(|value| value.as_str());
                    if let Some(text) = text {
                        let label = match (signature, model) {
                            (Some(signature), Some(model)) => {
                                format!("{signature}: {text} ({model})")
                            }
                            (Some(signature), None) => format!("{signature}: {text}"),
                            _ => text.to_string(),
                        };
                        self.append_agent_text(&label);
                    }
                }
            }
            "guidance/status" => {
                if let Some(params) = value.get("params") {
                    let signature = params.get("signature").and_then(|value| value.as_str());
                    let text = params.get("text").and_then(|value| value.as_str());
                    if let Some(text) = text {
                        let label = match signature {
                            Some(signature) if !signature.trim().is_empty() => {
                                format!("{signature}: {text}")
                            }
                            _ => text.to_string(),
                        };
                        self.append_agent_text(&label);
                    }
                }
            }
            "guidance/response" => {
                if let Some(params) = value.get("params") {
                    let model = params.get("model").and_then(|value| value.as_str());
                    let signatures = params
                        .get("signatures")
                        .and_then(|value| value.as_array())
                        .map(|items| {
                            items
                                .iter()
                                .filter_map(|item| item.as_str())
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    if !signatures.is_empty() {
                        let label = match model {
                            Some(model) => {
                                format!("Signatures: {} ({model})", signatures.join(" -> "))
                            }
                            None => format!("Signatures: {}", signatures.join(" -> ")),
                        };
                        self.append_agent_text(&label);
                    } else if let Some(signature) =
                        params.get("signature").and_then(|value| value.as_str())
                    {
                        let label = match model {
                            Some(model) => format!("Signature: {signature} ({model})"),
                            None => format!("Signature: {signature}"),
                        };
                        self.append_agent_text(&label);
                    }
                    if let Some(text) = params.get("text").and_then(|value| value.as_str()) {
                        self.queued_in_flight = false;
                        self.append_agent_text(text);
                    }
                }
            }
            "guidance/user_message" => {
                if let Some(text) = value
                    .get("params")
                    .and_then(|params| params.get("text"))
                    .and_then(|value| value.as_str())
                {
                    self.append_user_message(text);
                }
            }
            _ => {}
        }
    }

    fn append_user_message(&mut self, text: &str) {
        if text.trim().is_empty() {
            return;
        }
        if self.last_user_message.as_deref() == Some(text) {
            return;
        }
        self.last_user_message = Some(text.to_string());

        self.clear_working_indicator();
        let mut stream = new_markdown_stream();
        let markdown = format!("> {text}");
        stream.append(&markdown);
        stream.complete();
        let view = message_markdown_view(stream.document().clone());
        let entry = ThreadEntry::new(ThreadEntryType::User, view).copyable_text(markdown);
        self.formatted_thread.push_entry(entry);
    }

    fn append_agent_text(&mut self, text: &str) {
        if text.trim().is_empty() {
            return;
        }
        self.clear_working_indicator();
        let mut stream = new_markdown_stream();
        stream.append(text);
        stream.complete();
        let view = message_markdown_view(stream.document().clone());
        let entry =
            ThreadEntry::new(ThreadEntryType::Assistant, view).copyable_text(text.to_string());
        self.formatted_thread.push_entry(entry);
    }

    fn append_agent_delta(&mut self, item_id: &str, delta: &str) {
        if delta.is_empty() {
            return;
        }
        self.clear_working_indicator();

        let entry_index = self.ensure_agent_entry(item_id);
        let updated_doc = {
            if let Some(stream) = self.formatted_message_streams.get_mut(item_id) {
                stream.append(delta);
                stream.force_reparse();
                Some(stream.document().clone())
            } else {
                None
            }
        };

        if let Some(document) = updated_doc
            && let Some(entry) = self.formatted_thread.entry_mut(entry_index)
        {
            if let Some(stream) = self.formatted_message_streams.get(item_id) {
                entry.set_copyable_text(stream.source().to_string());
            }
            entry.set_content(message_markdown_view(document));
        }
    }

    fn append_agent_delta_from_source(
        &mut self,
        source: AgentDeltaSource,
        item_id: &str,
        delta: &str,
    ) {
        if delta.is_empty() {
            return;
        }
        let dedupe_key = format!("{item_id}\x1f{delta}");
        if let Some(previous) = self.agent_delta_aliases.get(&dedupe_key).copied() {
            if previous != source {
                return;
            }
        } else {
            if self.agent_delta_aliases.len() >= AGENT_DELTA_ALIAS_CACHE_LIMIT {
                self.agent_delta_aliases.clear();
            }
            self.agent_delta_aliases.insert(dedupe_key, source);
        }

        self.append_agent_delta(item_id, delta);
    }

    fn finish_agent_message(&mut self, item_id: &str, text: Option<&str>) {
        self.clear_working_indicator();
        let entry_index = self.ensure_agent_entry(item_id);

        let updated_doc = {
            if let Some(stream) = self.formatted_message_streams.get_mut(item_id) {
                if stream.source().trim().is_empty() {
                    if let Some(text) = text {
                        stream.append(text);
                    }
                }
                stream.complete();
                Some(stream.document().clone())
            } else {
                None
            }
        };

        if let Some(document) = updated_doc
            && let Some(entry) = self.formatted_thread.entry_mut(entry_index)
        {
            if let Some(stream) = self.formatted_message_streams.get(item_id) {
                entry.set_copyable_text(stream.source().to_string());
            }
            entry.set_content(message_markdown_view(document));
        }
    }

    fn ensure_agent_entry(&mut self, item_id: &str) -> usize {
        if let Some(entry_index) = self.formatted_message_entries.get(item_id) {
            return *entry_index;
        }

        self.clear_working_indicator();
        let stream = new_markdown_stream();
        let view = message_markdown_view(stream.document().clone());
        let entry = ThreadEntry::new(ThreadEntryType::Assistant, view).copyable_text(String::new());
        self.formatted_thread.push_entry(entry);
        let entry_index = self.formatted_thread.entry_count().saturating_sub(1);
        self.formatted_message_entries
            .insert(item_id.to_string(), entry_index);
        self.formatted_message_streams
            .insert(item_id.to_string(), stream);
        entry_index
    }

    fn start_tool_entry(&mut self, item_type: &str, item: &Value) {
        let Some(item_id) = item_id(item) else {
            return;
        };
        if self.tool_entries.contains_key(&item_id) {
            return;
        }

        self.clear_working_indicator();

        let entry_index = self.formatted_thread.entry_count();

        let entry = match item_type {
            "commandExecution" => {
                let command =
                    command_string_from_item(item).unwrap_or_else(|| "command".to_string());
                ToolEntry::Terminal {
                    entry_index,
                    command,
                    output: String::new(),
                    status: ToolStatus::Running,
                    exit_code: None,
                }
            }
            "fileChange" => {
                let (paths, first_path, _) = extract_file_changes(item);
                let input = if !paths.is_empty() {
                    paths.join(", ")
                } else {
                    "file change".to_string()
                };
                let tool_name = first_path.unwrap_or_else(|| "file_change".to_string());
                ToolEntry::Edit {
                    entry_index,
                    tool_name,
                    input,
                    output: String::new(),
                    status: ToolStatus::Running,
                }
            }
            "webSearch" => {
                let query = item_string(item, "query").unwrap_or_else(|| "search".to_string());
                ToolEntry::Search {
                    entry_index,
                    query,
                    matches: Vec::new(),
                    status: ToolStatus::Running,
                }
            }
            "mcpToolCall" => {
                let server = item_string(item, "server").unwrap_or_else(|| "mcp".to_string());
                let tool = item_string(item, "tool").unwrap_or_else(|| "tool".to_string());
                let tool_name = format!("mcp__{}__{}", server, tool);
                let input = item.get("arguments").map(|args| args.to_string());
                ToolEntry::Generic {
                    entry_index,
                    tool_type: ToolType::Task,
                    tool_name,
                    input,
                    output: None,
                    status: ToolStatus::Running,
                }
            }
            _ => {
                let tool_name = item_string(item, "tool").unwrap_or_else(|| "tool".to_string());
                ToolEntry::Generic {
                    entry_index,
                    tool_type: ToolType::Unknown,
                    tool_name,
                    input: None,
                    output: None,
                    status: ToolStatus::Running,
                }
            }
        };

        self.formatted_thread
            .push_entry(ThreadEntry::new(ThreadEntryType::Tool, Text::new("")));
        self.tool_entries.insert(item_id.clone(), entry);
        self.refresh_tool_entry(&item_id);
    }

    fn show_working_indicator(&mut self) {
        if self.working_entry_index.is_some() {
            return;
        }
        let mut stream = new_markdown_stream();
        stream.append("*Working...*");
        stream.complete();
        let view = message_markdown_view(stream.document().clone());
        self.formatted_thread
            .push_entry(ThreadEntry::new(ThreadEntryType::Assistant, view));
        let entry_index = self.formatted_thread.entry_count().saturating_sub(1);
        self.working_entry_index = Some(entry_index);
    }

    fn clear_working_indicator(&mut self) {
        let Some(index) = self.working_entry_index else {
            return;
        };
        if index + 1 == self.formatted_thread.entry_count() {
            let _ = self.formatted_thread.pop_entry();
        }
        self.working_entry_index = None;
    }

    fn ensure_reasoning_entry(&mut self, item_id: &str) -> usize {
        if let Some(entry) = self.reasoning_entries.get(item_id) {
            return entry.entry_index;
        }

        self.clear_working_indicator();
        let card = CodexReasoningCard::new(None, None);
        let entry = ThreadEntry::new(ThreadEntryType::Assistant, card).copyable_text(String::new());
        self.formatted_thread.push_entry(entry);
        let entry_index = self.formatted_thread.entry_count().saturating_sub(1);
        self.reasoning_entries.insert(
            item_id.to_string(),
            ReasoningEntry {
                summary: String::new(),
                content: String::new(),
                entry_index,
            },
        );
        entry_index
    }

    fn append_reasoning_summary_delta(&mut self, item_id: &str, delta: &str) {
        if delta.is_empty() {
            return;
        }
        let entry_index = self.ensure_reasoning_entry(item_id);
        if let Some(entry) = self.reasoning_entries.get_mut(item_id) {
            entry.summary.push_str(delta);
            if let Some(thread_entry) = self.formatted_thread.entry_mut(entry_index) {
                let show_summary = entry.content.trim().is_empty();
                let summary = if show_summary && !entry.summary.trim().is_empty() {
                    Some(entry.summary.clone())
                } else {
                    None
                };
                let content = if entry.content.trim().is_empty() {
                    None
                } else {
                    Some(entry.content.clone())
                };
                let card = CodexReasoningCard::new(summary, content);
                thread_entry.set_content(card);
                let copy_text = if !entry.content.trim().is_empty() {
                    entry.content.clone()
                } else {
                    entry.summary.clone()
                };
                if !copy_text.trim().is_empty() {
                    thread_entry.set_copyable_text(copy_text);
                }
            }
        }
    }

    fn append_reasoning_content_delta(&mut self, item_id: &str, delta: &str) {
        if delta.is_empty() {
            return;
        }
        let entry_index = self.ensure_reasoning_entry(item_id);
        if let Some(entry) = self.reasoning_entries.get_mut(item_id) {
            entry.content.push_str(delta);
            if let Some(thread_entry) = self.formatted_thread.entry_mut(entry_index) {
                let summary = None;
                let content = if entry.content.trim().is_empty() {
                    None
                } else {
                    Some(entry.content.clone())
                };
                let card = CodexReasoningCard::new(summary, content);
                thread_entry.set_content(card);
                if !entry.content.trim().is_empty() {
                    thread_entry.set_copyable_text(entry.content.clone());
                }
            }
        }
    }

    fn append_tool_output(&mut self, item_id: &str, delta: &str) {
        if delta.is_empty() {
            return;
        }

        if let Some(entry) = self.tool_entries.get_mut(item_id) {
            match entry {
                ToolEntry::Terminal { output, .. } => output.push_str(delta),
                ToolEntry::Edit { output, .. } => output.push_str(delta),
                ToolEntry::Generic { output, .. } => {
                    if let Some(existing) = output.as_mut() {
                        existing.push_str(delta);
                    } else {
                        *output = Some(delta.to_string());
                    }
                }
                ToolEntry::Search { .. } => {}
            }
            self.refresh_tool_entry(item_id);
        }
    }

    fn complete_tool_entry(&mut self, _item_type: &str, item: &Value) {
        let Some(item_id) = item_id(item) else {
            return;
        };

        if let Some(entry) = self.tool_entries.get_mut(&item_id) {
            match entry {
                ToolEntry::Terminal {
                    status, exit_code, ..
                } => {
                    let code = item
                        .get("exitCode")
                        .and_then(|v| v.as_i64())
                        .or_else(|| item.get("exit_code").and_then(|v| v.as_i64()))
                        .map(|v| v as i32);
                    if let Some(code) = code {
                        *exit_code = Some(code);
                        *status = if code == 0 {
                            ToolStatus::Success
                        } else {
                            ToolStatus::Error
                        };
                    } else {
                        *status = ToolStatus::Success;
                    }
                }
                ToolEntry::Edit { status, output, .. } => {
                    if output.is_empty() {
                        let (_, _, diff) = extract_file_changes(item);
                        if let Some(diff) = diff {
                            output.push_str(&diff);
                        }
                    }
                    *status = ToolStatus::Success;
                }
                ToolEntry::Search { status, .. } => {
                    *status = ToolStatus::Success;
                }
                ToolEntry::Generic { status, .. } => {
                    *status = ToolStatus::Success;
                }
            }
            self.refresh_tool_entry(&item_id);
        }
    }

    fn refresh_tool_entry(&mut self, item_id: &str) {
        let entry_snapshot = match self.tool_entries.get(item_id) {
            Some(entry) => entry.clone(),
            None => return,
        };

        let entry_index = entry_snapshot.entry_index();
        let Some(entry) = self.formatted_thread.entry_mut(entry_index) else {
            return;
        };

        match entry_snapshot {
            ToolEntry::Terminal {
                command,
                output,
                status,
                exit_code,
                ..
            } => {
                let mut tool = TerminalToolCall::new(command)
                    .status(status)
                    .output(output)
                    .expanded(false);
                if let Some(code) = exit_code {
                    tool = tool.exit_code(code);
                }
                entry.set_content(tool);
            }
            ToolEntry::Edit {
                tool_name,
                input,
                output,
                status,
                ..
            } => {
                let mut card = ToolCallCard::new(ToolType::Edit, tool_name)
                    .status(status)
                    .input(input)
                    .expanded(false);
                if !output.is_empty() {
                    card = card.output(output);
                }
                entry.set_content(card);
            }
            ToolEntry::Search {
                query,
                matches,
                status,
                ..
            } => {
                let mut tool = SearchToolCall::new(query).status(status).expanded(false);
                if !matches.is_empty() {
                    tool = tool.matches(matches);
                }
                entry.set_content(tool);
            }
            ToolEntry::Generic {
                tool_type,
                tool_name,
                input,
                output,
                status,
                ..
            } => {
                let mut card = ToolCallCard::new(tool_type, tool_name)
                    .status(status)
                    .expanded(false);
                if let Some(input) = input {
                    card = card.input(input);
                }
                if let Some(output) = output {
                    card = card.output(output);
                }
                entry.set_content(card);
            }
        }
    }
}

impl EditorGroup {
    fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active_tab: None,
            group_bounds: Bounds::ZERO,
            tab_bar_bounds: Bounds::ZERO,
            editor_bounds: Bounds::ZERO,
            tab_hits: Vec::new(),
        }
    }
}

impl EditorTab {
    fn new(path: PathBuf) -> Self {
        let title = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("untitled")
            .to_string();
        let mut editor = EditorElement::new(Editor::new(""));
        editor.set_wrap_lines(true);
        editor.set_read_only(false);
        Self {
            path,
            title,
            editor,
            saved_revision: 0,
            loading: true,
        }
    }

    fn is_dirty(&self) -> bool {
        self.editor.editor().revision() != self.saved_revision
    }
}

impl FileEditorPaneState {
    fn new() -> Self {
        let pending_open = Rc::new(RefCell::new(false));
        let pending_open_submit = pending_open.clone();
        let mut path_input = TextInput::new()
            .placeholder("Open path (relative to workspace)")
            .background(theme::bg::APP)
            .border_color(theme::border::DEFAULT)
            .border_color_focused(theme::border::FOCUS)
            .text_color(theme::text::PRIMARY)
            .placeholder_color(theme::text::MUTED)
            .on_submit(move |_value| {
                *pending_open_submit.borrow_mut() = true;
            });
        path_input.set_mono(true);

        let pending_open_click = pending_open.clone();
        let open_button = Button::new("Open")
            .variant(ButtonVariant::Primary)
            .font_size(theme::font_size::SM)
            .padding(12.0, 6.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_open_click.borrow_mut() = true;
            });

        let pending_reload = Rc::new(RefCell::new(false));
        let pending_reload_click = pending_reload.clone();
        let reload_button = Button::new("Reload")
            .variant(ButtonVariant::Secondary)
            .font_size(theme::font_size::SM)
            .padding(12.0, 6.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_reload_click.borrow_mut() = true;
            });

        let pending_save = Rc::new(RefCell::new(false));
        let pending_save_click = pending_save.clone();
        let save_button = Button::new("Save")
            .variant(ButtonVariant::Primary)
            .font_size(theme::font_size::SM)
            .padding(12.0, 6.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_save_click.borrow_mut() = true;
            });

        let pending_split = Rc::new(RefCell::new(None));
        let pending_split_h = pending_split.clone();
        let split_horizontal_button = Button::new("Split H")
            .variant(ButtonVariant::Secondary)
            .font_size(theme::font_size::SM)
            .padding(10.0, 6.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_split_h.borrow_mut() = Some(SplitDirection::Horizontal);
            });

        let pending_split_v = pending_split.clone();
        let split_vertical_button = Button::new("Split V")
            .variant(ButtonVariant::Secondary)
            .font_size(theme::font_size::SM)
            .padding(10.0, 6.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_split_v.borrow_mut() = Some(SplitDirection::Vertical);
            });

        let pending_tree_refresh = Rc::new(RefCell::new(false));
        let pending_tree_refresh_click = pending_tree_refresh.clone();
        let tree_refresh_button = Button::new("Refresh")
            .variant(ButtonVariant::Secondary)
            .font_size(theme::font_size::XS)
            .padding(10.0, 6.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_tree_refresh_click.borrow_mut() = true;
            });

        let groups = vec![EditorGroup::new()];

        Self {
            workspace_root: None,
            tree_root: None,
            tree_rows: Vec::new(),
            tree_scroll: ScrollContainer::new(Bounds::ZERO, ScrollDirection::Vertical),
            tree_bounds: Bounds::ZERO,
            tree_header_bounds: Bounds::ZERO,
            tree_refresh_button,
            tree_refresh_bounds: Bounds::ZERO,
            pending_tree_refresh,
            selected_tree_path: None,
            path_input,
            open_button,
            reload_button,
            save_button,
            split_horizontal_button,
            split_vertical_button,
            path_bounds: Bounds::ZERO,
            open_bounds: Bounds::ZERO,
            reload_bounds: Bounds::ZERO,
            save_bounds: Bounds::ZERO,
            split_h_bounds: Bounds::ZERO,
            split_v_bounds: Bounds::ZERO,
            status: Some("No workspace open".to_string()),
            status_is_error: false,
            tabs: HashMap::new(),
            tab_by_path: HashMap::new(),
            groups,
            active_group: 0,
            split_direction: SplitDirection::None,
            next_tab_id: 1,
            pending_open,
            pending_reload,
            pending_save,
            pending_split,
            pending_open_requests: Vec::new(),
            pending_open_dispatches: Vec::new(),
        }
    }

    fn take_pending_open(&mut self) -> bool {
        let mut pending = self.pending_open.borrow_mut();
        let value = *pending;
        *pending = false;
        value
    }

    fn take_pending_reload(&mut self) -> bool {
        let mut pending = self.pending_reload.borrow_mut();
        let value = *pending;
        *pending = false;
        value
    }

    fn take_pending_save(&mut self) -> bool {
        let mut pending = self.pending_save.borrow_mut();
        let value = *pending;
        *pending = false;
        value
    }

    fn take_pending_split(&mut self) -> Option<SplitDirection> {
        let mut pending = self.pending_split.borrow_mut();
        let value = *pending;
        *pending = None;
        value
    }

    fn take_pending_tree_refresh(&mut self) -> bool {
        let mut pending = self.pending_tree_refresh.borrow_mut();
        let value = *pending;
        *pending = false;
        value
    }

    fn request_save(&mut self) {
        *self.pending_save.borrow_mut() = true;
    }

    fn set_workspace_root(&mut self, path: PathBuf) {
        self.workspace_root = Some(path);
        self.refresh_tree();
    }

    fn refresh_tree(&mut self) {
        if let Some(root) = self.workspace_root.clone() {
            let mut remaining = FILE_TREE_MAX_ENTRIES;
            let mut node = build_file_node(&root, &mut remaining);
            node.expanded = true;
            self.tree_root = Some(node);
            self.tree_scroll.scroll_to(Point::ZERO);
            self.tree_rows.clear();
            self.selected_tree_path = None;
        }
    }

    fn toggle_tree_node(&mut self, path: &Path) {
        if let Some(root) = self.tree_root.as_mut() {
            toggle_tree_node(root, path);
        }
    }

    fn queue_open_path(&mut self, path: PathBuf, force_reload: bool) {
        if let Some(tab_id) = self.tab_by_path.get(&path).copied() {
            self.activate_tab_by_id(tab_id);
            if let Some(tab) = self.tabs.get(&tab_id) {
                if tab.is_dirty() && !force_reload {
                    self.status = Some(format!(
                        "Unsaved changes in {} (not reloaded).",
                        path.display()
                    ));
                    self.status_is_error = false;
                    return;
                }
            }
            if !force_reload {
                self.status = Some(format!("Focused {}", path.display()));
                self.status_is_error = false;
                self.path_input
                    .set_value(path.to_string_lossy().to_string());
                return;
            }
        }

        if self
            .pending_open_requests
            .iter()
            .any(|req| req.path == path)
        {
            return;
        }

        let target_group = self.active_group;
        self.pending_open_requests.push(PendingOpenRequest {
            path: path.clone(),
            target_group,
        });
        self.pending_open_dispatches.push(path.clone());
        let verb = if force_reload { "Reloading" } else { "Opening" };
        self.status = Some(format!("{verb} {}...", path.display()));
        self.status_is_error = false;
        self.path_input
            .set_value(path.to_string_lossy().to_string());
    }

    fn resolve_path(&self, raw: &str) -> Option<PathBuf> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }
        let mut path = if trimmed == "~" || trimmed.starts_with("~/") {
            let home = std::env::var("HOME").unwrap_or_default();
            if home.is_empty() {
                PathBuf::from(trimmed)
            } else if trimmed == "~" {
                PathBuf::from(home)
            } else {
                PathBuf::from(home).join(&trimmed[2..])
            }
        } else {
            PathBuf::from(trimmed)
        };

        if path.is_absolute() {
            return Some(path);
        }

        if let Some(root) = &self.workspace_root {
            path = root.join(path);
            return Some(path);
        }

        std::env::current_dir().ok().map(|cwd| cwd.join(path))
    }

    fn set_contents(&mut self, path: PathBuf, contents: String) {
        let target_group = self
            .pending_open_requests
            .iter()
            .position(|req| req.path == path)
            .map(|idx| self.pending_open_requests.remove(idx).target_group)
            .unwrap_or(self.active_group);

        let existing_tab = self.tab_by_path.get(&path).copied();
        let tab_id = existing_tab.unwrap_or_else(|| self.create_tab(path.clone(), target_group));

        if let Some(tab) = self.tabs.get_mut(&tab_id) {
            tab.loading = false;
            tab.editor.set_text(&contents);
            let language = SyntaxLanguage::from_path(path.to_string_lossy().as_ref());
            tab.editor.set_language(language);
            tab.editor.reset_scroll();
            tab.saved_revision = tab.editor.editor().revision();
        }

        if existing_tab.is_some() {
            self.activate_tab_by_id(tab_id);
        } else {
            self.activate_tab(tab_id, target_group);
        }
        self.path_input
            .set_value(path.to_string_lossy().to_string());

        if let Some(tab) = self.tabs.get(&tab_id) {
            let line_count = tab.editor.editor().buffer().line_count();
            let byte_count = tab.editor.editor().buffer().len_bytes();
            self.status = Some(format!(
                "{} | {} lines | {} bytes",
                path.display(),
                line_count,
                byte_count
            ));
            self.status_is_error = false;
        }
    }

    fn set_error(&mut self, path: PathBuf, error: String) {
        if let Some(tab_id) = self.tab_by_path.get(&path).copied() {
            if let Some(tab) = self.tabs.get_mut(&tab_id) {
                tab.loading = false;
            }
        }
        self.pending_open_requests.retain(|req| req.path != path);
        self.status = Some(format!("{}: {}", path.display(), error));
        self.status_is_error = true;
        self.path_input
            .set_value(path.to_string_lossy().to_string());
    }

    fn set_saved(&mut self, path: PathBuf) {
        if let Some(tab_id) = self.tab_by_path.get(&path).copied() {
            if let Some(tab) = self.tabs.get_mut(&tab_id) {
                tab.saved_revision = tab.editor.editor().revision();
                tab.loading = false;
            }
            self.status = Some(format!("Saved {}", path.display()));
            self.status_is_error = false;
        }
    }

    fn set_save_error(&mut self, path: PathBuf, error: String) {
        self.status = Some(format!("Save failed: {}: {}", path.display(), error));
        self.status_is_error = true;
    }

    fn active_tab_id(&self) -> Option<usize> {
        self.groups
            .get(self.active_group)
            .and_then(|group| group.active_tab)
    }

    fn active_tab(&self) -> Option<&EditorTab> {
        self.active_tab_id().and_then(|id| self.tabs.get(&id))
    }

    fn active_tab_mut(&mut self) -> Option<&mut EditorTab> {
        let id = self.active_tab_id()?;
        self.tabs.get_mut(&id)
    }

    fn active_tab_path(&self) -> Option<PathBuf> {
        self.active_tab().map(|tab| tab.path.clone())
    }

    fn tree_row_at(&self, point: Point) -> Option<&FileTreeRow> {
        self.tree_rows.iter().find(|row| row.bounds.contains(point))
    }

    fn tab_hit_at(&self, point: Point) -> Option<(usize, usize)> {
        for (group_index, group) in self.groups.iter().enumerate() {
            for hit in &group.tab_hits {
                if hit.bounds.contains(point) {
                    return Some((group_index, hit.tab_id));
                }
            }
        }
        None
    }

    fn group_at_point(&self, point: Point) -> Option<usize> {
        self.groups
            .iter()
            .enumerate()
            .find(|(_, group)| group.group_bounds.contains(point))
            .map(|(idx, _)| idx)
    }

    fn editor_bounds_for_group(&self, group_index: usize) -> Option<Bounds> {
        self.groups
            .get(group_index)
            .map(|group| group.editor_bounds)
    }

    fn editor_cursor_at(&self, point: Point) -> Option<Cursor> {
        for group in &self.groups {
            if group.editor_bounds.contains(point) {
                if let Some(tab_id) = group.active_tab {
                    if let Some(tab) = self.tabs.get(&tab_id) {
                        return Some(tab.editor.cursor());
                    }
                }
            }
        }
        if let Some(tab) = self.active_tab() {
            if tab.editor.is_focused() {
                return Some(tab.editor.cursor());
            }
        }
        None
    }

    fn blur_editors(&mut self) {
        for tab in self.tabs.values_mut() {
            tab.editor.blur();
        }
    }

    fn create_tab(&mut self, path: PathBuf, group_index: usize) -> usize {
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        let tab = EditorTab::new(path.clone());
        self.tabs.insert(id, tab);
        self.tab_by_path.insert(path, id);
        let target_group = if self.groups.is_empty() {
            self.groups.push(EditorGroup::new());
            0
        } else {
            group_index.min(self.groups.len().saturating_sub(1))
        };
        if let Some(group) = self.groups.get_mut(target_group) {
            group.tabs.push(id);
            group.active_tab = Some(id);
        }
        id
    }

    fn activate_tab_by_id(&mut self, tab_id: usize) {
        let mut group_index = None;
        for (idx, group) in self.groups.iter().enumerate() {
            if group.tabs.contains(&tab_id) {
                group_index = Some(idx);
                break;
            }
        }
        if let Some(index) = group_index {
            self.activate_tab(tab_id, index);
        }
    }

    fn activate_tab(&mut self, tab_id: usize, group_index: usize) {
        if let Some(group) = self.groups.get_mut(group_index) {
            group.active_tab = Some(tab_id);
        }
        self.active_group = group_index.min(self.groups.len().saturating_sub(1));
        if let Some(tab) = self.tabs.get(&tab_id) {
            self.path_input
                .set_value(tab.path.to_string_lossy().to_string());
            self.selected_tree_path = Some(tab.path.clone());
        }
    }

    fn save_active_tab(&mut self) -> Option<(PathBuf, String)> {
        let tab = self.active_tab_mut()?;
        let path = tab.path.clone();
        let contents = tab.editor.editor().text();
        self.status = Some(format!("Saving {}...", path.display()));
        self.status_is_error = false;
        Some((path, contents))
    }

    fn set_split_direction(&mut self, direction: SplitDirection) {
        if direction == self.split_direction {
            self.collapse_split();
            return;
        }

        self.split_direction = direction;
        if self.groups.len() < 2 {
            self.groups.push(EditorGroup::new());
        }
    }

    fn collapse_split(&mut self) {
        if self.groups.len() > 1 {
            let mut merged_tabs = Vec::new();
            for group in self.groups.iter().skip(1) {
                merged_tabs.extend(group.tabs.iter().copied());
            }
            if let Some(primary) = self.groups.first_mut() {
                for tab_id in merged_tabs {
                    if !primary.tabs.contains(&tab_id) {
                        primary.tabs.push(tab_id);
                    }
                }
                if primary.active_tab.is_none() {
                    primary.active_tab = primary.tabs.first().copied();
                }
            }
        }
        self.groups.truncate(1);
        self.split_direction = SplitDirection::None;
        self.active_group = 0;
    }
}

impl MinimalRoot {
    pub fn new() -> Self {
        let pending_copy = Rc::new(RefCell::new(false));
        let pending_copy_click = pending_copy.clone();
        let copy_button = Button::new("Copy")
            .variant(ButtonVariant::Secondary)
            .font_size(theme::font_size::XS)
            .padding(12.0, 6.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_copy_click.borrow_mut() = true;
            });

        let pending_threads_refresh = Rc::new(RefCell::new(false));
        let pending_threads_refresh_click = pending_threads_refresh.clone();
        let threads_refresh_button = Button::new("Refresh")
            .variant(ButtonVariant::Secondary)
            .font_size(theme::font_size::XS)
            .padding(10.0, 6.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_threads_refresh_click.borrow_mut() = true;
            });

        let pending_threads_load_more = Rc::new(RefCell::new(false));
        let pending_threads_load_more_click = pending_threads_load_more.clone();
        let threads_load_more_button = Button::new("Load 10 more")
            .variant(ButtonVariant::Secondary)
            .font_size(theme::font_size::XS)
            .padding(10.0, 6.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_threads_load_more_click.borrow_mut() = true;
            });

        let pending_thread_open = Rc::new(RefCell::new(None));
        let pending_inbox_refresh = Rc::new(RefCell::new(false));
        let pending_inbox_refresh_click = pending_inbox_refresh.clone();
        let inbox_refresh_button = Button::new("Refresh")
            .variant(ButtonVariant::Secondary)
            .font_size(theme::font_size::XS)
            .padding(10.0, 6.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_inbox_refresh_click.borrow_mut() = true;
            });

        let pending_inbox_open_thread_pane = Rc::new(RefCell::new(false));
        let pending_inbox_open_thread_click = pending_inbox_open_thread_pane.clone();
        let inbox_open_thread_button = Button::new("Thread")
            .variant(ButtonVariant::Secondary)
            .font_size(theme::font_size::XS)
            .padding(10.0, 6.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_inbox_open_thread_click.borrow_mut() = true;
            });

        let pending_inbox_open_approvals_pane = Rc::new(RefCell::new(false));
        let pending_inbox_open_approvals_click = pending_inbox_open_approvals_pane.clone();
        let inbox_open_approvals_button = Button::new("Approvals")
            .variant(ButtonVariant::Secondary)
            .font_size(theme::font_size::XS)
            .padding(10.0, 6.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_inbox_open_approvals_click.borrow_mut() = true;
            });

        let pending_inbox_open_audit_pane = Rc::new(RefCell::new(false));
        let pending_inbox_open_audit_click = pending_inbox_open_audit_pane.clone();
        let inbox_open_audit_button = Button::new("Audit")
            .variant(ButtonVariant::Secondary)
            .font_size(theme::font_size::XS)
            .padding(10.0, 6.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_inbox_open_audit_click.borrow_mut() = true;
            });

        let pending_inbox_approve = Rc::new(RefCell::new(false));
        let pending_inbox_approve_click = pending_inbox_approve.clone();
        let inbox_approve_button = Button::new("Approve")
            .variant(ButtonVariant::Primary)
            .font_size(theme::font_size::XS)
            .padding(10.0, 6.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_inbox_approve_click.borrow_mut() = true;
            });

        let pending_inbox_reject = Rc::new(RefCell::new(false));
        let pending_inbox_reject_click = pending_inbox_reject.clone();
        let inbox_reject_button = Button::new("Needs Human")
            .variant(ButtonVariant::Danger)
            .font_size(theme::font_size::XS)
            .padding(10.0, 6.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_inbox_reject_click.borrow_mut() = true;
            });

        let pending_inbox_select_thread = Rc::new(RefCell::new(None));
        let file_editor = FileEditorPaneState::new();

        let pending_runtime_auth_send = Rc::new(RefCell::new(false));
        let pending_runtime_auth_send_submit = pending_runtime_auth_send.clone();
        let runtime_auth_email_input = TextInput::new()
            .placeholder("you@domain.com")
            .background(theme::bg::APP)
            .border_color(theme::border::DEFAULT)
            .border_color_focused(theme::border::FOCUS)
            .text_color(theme::text::PRIMARY)
            .placeholder_color(theme::text::MUTED)
            .on_submit(move |_value| {
                *pending_runtime_auth_send_submit.borrow_mut() = true;
            });

        let pending_runtime_auth_verify = Rc::new(RefCell::new(false));
        let pending_runtime_auth_verify_submit = pending_runtime_auth_verify.clone();
        let runtime_auth_code_input = TextInput::new()
            .placeholder("6-digit code (from email)")
            .background(theme::bg::APP)
            .border_color(theme::border::DEFAULT)
            .border_color_focused(theme::border::FOCUS)
            .text_color(theme::text::PRIMARY)
            .placeholder_color(theme::text::MUTED)
            .on_submit(move |_value| {
                *pending_runtime_auth_verify_submit.borrow_mut() = true;
            });

        let pending_runtime_auth_send_click = pending_runtime_auth_send.clone();
        let runtime_auth_send_button = Button::new("Send code")
            .variant(ButtonVariant::Primary)
            .font_size(theme::font_size::SM)
            .padding(12.0, 8.0)
            .corner_radius(8.0)
            .on_click(move || {
                *pending_runtime_auth_send_click.borrow_mut() = true;
            });

        let pending_runtime_auth_verify_click = pending_runtime_auth_verify.clone();
        let runtime_auth_verify_button = Button::new("Verify")
            .variant(ButtonVariant::Secondary)
            .font_size(theme::font_size::SM)
            .padding(12.0, 8.0)
            .corner_radius(8.0)
            .on_click(move || {
                *pending_runtime_auth_verify_click.borrow_mut() = true;
            });

        let pending_runtime_auth_status = Rc::new(RefCell::new(false));
        let pending_runtime_auth_status_click = pending_runtime_auth_status.clone();
        let runtime_auth_status_button = Button::new("Refresh")
            .variant(ButtonVariant::Secondary)
            .font_size(theme::font_size::SM)
            .padding(12.0, 8.0)
            .corner_radius(8.0)
            .on_click(move || {
                *pending_runtime_auth_status_click.borrow_mut() = true;
            });

        let pending_runtime_auth_logout = Rc::new(RefCell::new(false));
        let pending_runtime_auth_logout_click = pending_runtime_auth_logout.clone();
        let runtime_auth_logout_button = Button::new("Logout")
            .variant(ButtonVariant::Secondary)
            .font_size(theme::font_size::SM)
            .padding(12.0, 8.0)
            .corner_radius(8.0)
            .on_click(move || {
                *pending_runtime_auth_logout_click.borrow_mut() = true;
            });

        let pending_identity_load = Rc::new(RefCell::new(false));
        let pending_identity_load_submit = pending_identity_load.clone();
        let identity_private_key_input = TextInput::new()
            .placeholder("nsec1... or 64-char hex private key")
            .background(theme::bg::APP)
            .border_color(theme::border::DEFAULT)
            .border_color_focused(theme::border::FOCUS)
            .text_color(theme::text::PRIMARY)
            .placeholder_color(theme::text::MUTED)
            .on_submit(move |_value| {
                *pending_identity_load_submit.borrow_mut() = true;
            });

        let pending_identity_load_click = pending_identity_load.clone();
        let identity_load_button = Button::new("Load key")
            .variant(ButtonVariant::Primary)
            .font_size(theme::font_size::XS + 4.0)
            .padding(12.0, 6.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_identity_load_click.borrow_mut() = true;
            });

        let pending_keygen = Rc::new(RefCell::new(false));
        let pending_keygen_click = pending_keygen.clone();
        let keygen_button = Button::new("Generate keys")
            .variant(ButtonVariant::Secondary)
            .font_size(theme::font_size::XS + 4.0)
            .padding(12.0, 6.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_keygen_click.borrow_mut() = true;
            });

        let pending_identity_copy_npub = Rc::new(RefCell::new(false));
        let pending_identity_copy_npub_click = pending_identity_copy_npub.clone();
        let identity_copy_npub_button = Button::new("Copy")
            .variant(ButtonVariant::Secondary)
            .font_size(theme::font_size::XS)
            .padding(10.0, 6.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_identity_copy_npub_click.borrow_mut() = true;
            });

        let pending_identity_copy_nsec = Rc::new(RefCell::new(false));
        let pending_identity_copy_nsec_click = pending_identity_copy_nsec.clone();
        let identity_copy_nsec_button = Button::new("Copy")
            .variant(ButtonVariant::Secondary)
            .font_size(theme::font_size::XS)
            .padding(10.0, 6.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_identity_copy_nsec_click.borrow_mut() = true;
            });

        let pending_pylon_toggle = Rc::new(RefCell::new(false));
        let pending_pylon_toggle_click = pending_pylon_toggle.clone();
        let pylon_toggle_button = Button::new("Turn On")
            .variant(ButtonVariant::Primary)
            .font_size(theme::font_size::XS)
            .padding(12.0, 6.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_pylon_toggle_click.borrow_mut() = true;
            });

        let pending_pylon_init = Rc::new(RefCell::new(false));
        let pending_pylon_init_click = pending_pylon_init.clone();
        let pylon_init_button = Button::new("Init identity")
            .variant(ButtonVariant::Secondary)
            .font_size(theme::font_size::XS)
            .padding(10.0, 6.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_pylon_init_click.borrow_mut() = true;
            });

        let pending_pylon_start = Rc::new(RefCell::new(false));
        let pending_pylon_start_click = pending_pylon_start.clone();
        let pylon_start_button = Button::new("Start")
            .variant(ButtonVariant::Secondary)
            .font_size(theme::font_size::XS)
            .padding(10.0, 6.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_pylon_start_click.borrow_mut() = true;
            });

        let pending_pylon_stop = Rc::new(RefCell::new(false));
        let pending_pylon_stop_click = pending_pylon_stop.clone();
        let pylon_stop_button = Button::new("Stop")
            .variant(ButtonVariant::Secondary)
            .font_size(theme::font_size::XS)
            .padding(10.0, 6.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_pylon_stop_click.borrow_mut() = true;
            });

        let pending_pylon_refresh = Rc::new(RefCell::new(false));
        let pending_pylon_refresh_click = pending_pylon_refresh.clone();
        let pylon_refresh_button = Button::new("Refresh")
            .variant(ButtonVariant::Secondary)
            .font_size(theme::font_size::XS)
            .padding(10.0, 6.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_pylon_refresh_click.borrow_mut() = true;
            });

        let pending_wallet_refresh = Rc::new(RefCell::new(false));
        let pending_wallet_refresh_click = pending_wallet_refresh.clone();
        let wallet_refresh_button = Button::new("Refresh")
            .variant(ButtonVariant::Secondary)
            .font_size(theme::font_size::XS)
            .padding(10.0, 6.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_wallet_refresh_click.borrow_mut() = true;
            });

        let pending_wallet_liquidity = Rc::new(RefCell::new(false));
        let pending_wallet_liquidity_click = pending_wallet_liquidity.clone();
        let wallet_liquidity_button = Button::new("Liquidity")
            .variant(ButtonVariant::Secondary)
            .font_size(theme::font_size::XS)
            .padding(10.0, 6.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_wallet_liquidity_click.borrow_mut() = true;
            });

        let pending_liquidity_online = Rc::new(RefCell::new(false));
        let pending_liquidity_online_click = pending_liquidity_online.clone();
        let liquidity_online_button = Button::new("Go online")
            .variant(ButtonVariant::Primary)
            .font_size(theme::font_size::XS)
            .padding(12.0, 6.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_liquidity_online_click.borrow_mut() = true;
            });

        let pending_liquidity_offline = Rc::new(RefCell::new(false));
        let pending_liquidity_offline_click = pending_liquidity_offline.clone();
        let liquidity_offline_button = Button::new("Go offline")
            .variant(ButtonVariant::Secondary)
            .font_size(theme::font_size::XS)
            .padding(12.0, 6.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_liquidity_offline_click.borrow_mut() = true;
            });

        let pending_liquidity_refresh = Rc::new(RefCell::new(false));
        let pending_liquidity_refresh_click = pending_liquidity_refresh.clone();
        let liquidity_refresh_button = Button::new("Refresh")
            .variant(ButtonVariant::Secondary)
            .font_size(theme::font_size::XS)
            .padding(10.0, 6.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_liquidity_refresh_click.borrow_mut() = true;
            });

        let pending_liquidity_invoice_create = Rc::new(RefCell::new(false));
        let pending_liquidity_invoice_create_click = pending_liquidity_invoice_create.clone();
        let mut liquidity_invoice_amount_input = TextInput::new()
            .placeholder("Invoice amount (sats)")
            .background(theme::bg::APP)
            .border_color(theme::border::DEFAULT)
            .border_color_focused(theme::border::FOCUS)
            .text_color(theme::text::PRIMARY)
            .placeholder_color(theme::text::MUTED)
            .on_submit(move |_value| {
                *pending_liquidity_invoice_create_click.borrow_mut() = true;
            });
        liquidity_invoice_amount_input.set_value("1000");

        let pending_liquidity_invoice_create_click = pending_liquidity_invoice_create.clone();
        let liquidity_invoice_create_button = Button::new("Create invoice")
            .variant(ButtonVariant::Primary)
            .font_size(theme::font_size::XS)
            .padding(12.0, 6.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_liquidity_invoice_create_click.borrow_mut() = true;
            });

        let pending_liquidity_invoice_copy = Rc::new(RefCell::new(false));
        let pending_liquidity_invoice_copy_click = pending_liquidity_invoice_copy.clone();
        let liquidity_invoice_copy_button = Button::new("Copy")
            .variant(ButtonVariant::Secondary)
            .font_size(theme::font_size::XS)
            .padding(10.0, 6.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_liquidity_invoice_copy_click.borrow_mut() = true;
            });

        let pending_sell_compute_online = Rc::new(RefCell::new(false));
        let pending_sell_compute_online_click = pending_sell_compute_online.clone();
        let sell_compute_online_button = Button::new("Go online")
            .variant(ButtonVariant::Primary)
            .font_size(theme::font_size::XS)
            .padding(12.0, 6.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_sell_compute_online_click.borrow_mut() = true;
            });

        let pending_sell_compute_offline = Rc::new(RefCell::new(false));
        let pending_sell_compute_offline_click = pending_sell_compute_offline.clone();
        let sell_compute_offline_button = Button::new("Go offline")
            .variant(ButtonVariant::Secondary)
            .font_size(theme::font_size::XS)
            .padding(12.0, 6.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_sell_compute_offline_click.borrow_mut() = true;
            });

        let pending_sell_compute_refresh = Rc::new(RefCell::new(false));
        let pending_sell_compute_refresh_click = pending_sell_compute_refresh.clone();
        let sell_compute_refresh_button = Button::new("Refresh")
            .variant(ButtonVariant::Secondary)
            .font_size(theme::font_size::XS)
            .padding(10.0, 6.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_sell_compute_refresh_click.borrow_mut() = true;
            });

        let pending_dvm_history_refresh = Rc::new(RefCell::new(false));
        let pending_dvm_history_refresh_click = pending_dvm_history_refresh.clone();
        let dvm_history_refresh_button = Button::new("Refresh")
            .variant(ButtonVariant::Secondary)
            .font_size(theme::font_size::XS)
            .padding(10.0, 6.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_dvm_history_refresh_click.borrow_mut() = true;
            });

        let pending_nip90_submit = Rc::new(RefCell::new(false));
        let pending_nip90_submit_click = pending_nip90_submit.clone();
        let mut nip90_kind_input = TextInput::new()
            .placeholder("Job kind (e.g. 5050)")
            .background(theme::bg::APP)
            .border_color(theme::border::DEFAULT)
            .border_color_focused(theme::border::FOCUS)
            .text_color(theme::text::PRIMARY)
            .placeholder_color(theme::text::MUTED);
        nip90_kind_input.set_value("5050");
        let nip90_relay_input = TextInput::new()
            .placeholder("Relay URLs (comma separated)")
            .background(theme::bg::APP)
            .border_color(theme::border::DEFAULT)
            .border_color_focused(theme::border::FOCUS)
            .text_color(theme::text::PRIMARY)
            .placeholder_color(theme::text::MUTED);
        let nip90_provider_input = TextInput::new()
            .placeholder("Provider pubkey (optional)")
            .background(theme::bg::APP)
            .border_color(theme::border::DEFAULT)
            .border_color_focused(theme::border::FOCUS)
            .text_color(theme::text::PRIMARY)
            .placeholder_color(theme::text::MUTED);
        let nip90_prompt_input = TextInput::new()
            .placeholder("Prompt / job input")
            .background(theme::bg::APP)
            .border_color(theme::border::DEFAULT)
            .border_color_focused(theme::border::FOCUS)
            .text_color(theme::text::PRIMARY)
            .placeholder_color(theme::text::MUTED)
            .on_submit(move |_value| {
                *pending_nip90_submit_click.borrow_mut() = true;
            });

        let pending_nip90_submit_click = pending_nip90_submit.clone();
        let nip90_submit_button = Button::new("Submit")
            .variant(ButtonVariant::Primary)
            .font_size(theme::font_size::SM)
            .padding(14.0, 8.0)
            .corner_radius(8.0)
            .background(theme::accent::PRIMARY)
            .text_color(theme::bg::APP)
            .on_click(move || {
                *pending_nip90_submit_click.borrow_mut() = true;
            });

        let event_scroll = ScrollView::new().show_scrollbar(true).scrollbar_width(6.0);
        let hotbar = Hotbar::new()
            .item_size(HOTBAR_ITEM_SIZE)
            .padding(HOTBAR_PADDING)
            .gap(HOTBAR_ITEM_GAP)
            .corner_radius(8.0);
        let pane_resizer = ResizablePane::new()
            .handle_size(PANE_RESIZE_HANDLE)
            .min_size(PANE_MIN_WIDTH, PANE_MIN_HEIGHT);

        let mut root = Self {
            event_context: EventContext::new(),
            cursor_position: Point::ZERO,
            screen_size: Size::new(1280.0, 720.0),
            zoom_factor: 1.0,
            pane_store: PaneStore::default(),
            pane_frames: HashMap::new(),
            pane_bounds: HashMap::new(),
            pane_drag: None,
            pane_resize: None,
            canvas_pan: None,
            background_offset: Point::ZERO,
            pane_resizer,
            chat_panes: HashMap::new(),
            chat_slot_assignments: HashMap::new(),
            chat_slot_labels: HashMap::new(),
            pending_session_panes: VecDeque::new(),
            session_to_pane: HashMap::new(),
            thread_to_pane: HashMap::new(),
            next_chat_index: 1,
            hotbar,
            hotbar_bounds: Bounds::ZERO,
            copy_button,
            copy_bounds: Bounds::ZERO,
            pending_copy,
            copy_feedback_until: None,
            copy_feedback_duration: Duration::from_secs(1),
            threads_refresh_button,
            threads_refresh_bounds: Bounds::ZERO,
            pending_threads_refresh,
            threads_load_more_button,
            threads_load_more_bounds: Bounds::ZERO,
            pending_threads_load_more,
            threads_next_cursor: None,
            thread_entries: Vec::new(),
            pending_thread_open,
            inbox: InboxPaneState::default(),
            route_state: DesktopRouteState::default(),
            inbox_refresh_button,
            inbox_refresh_bounds: Bounds::ZERO,
            inbox_open_thread_button,
            inbox_open_thread_bounds: Bounds::ZERO,
            inbox_open_approvals_button,
            inbox_open_approvals_bounds: Bounds::ZERO,
            inbox_open_audit_button,
            inbox_open_audit_bounds: Bounds::ZERO,
            inbox_approve_button,
            inbox_approve_bounds: Bounds::ZERO,
            inbox_reject_button,
            inbox_reject_bounds: Bounds::ZERO,
            pending_inbox_refresh,
            pending_inbox_select_thread,
            pending_inbox_open_thread_pane,
            pending_inbox_open_approvals_pane,
            pending_inbox_open_audit_pane,
            pending_inbox_approve,
            pending_inbox_reject,
            file_editor,
            runtime_auth: RuntimeAuthStateView::default(),
            runtime_auth_email_input,
            runtime_auth_email_bounds: Bounds::ZERO,
            runtime_auth_code_input,
            runtime_auth_code_bounds: Bounds::ZERO,
            runtime_auth_send_button,
            runtime_auth_send_bounds: Bounds::ZERO,
            runtime_auth_verify_button,
            runtime_auth_verify_bounds: Bounds::ZERO,
            runtime_auth_status_button,
            runtime_auth_status_bounds: Bounds::ZERO,
            runtime_auth_logout_button,
            runtime_auth_logout_bounds: Bounds::ZERO,
            pending_runtime_auth_send,
            pending_runtime_auth_verify,
            pending_runtime_auth_status,
            pending_runtime_auth_logout,
            identity_private_key_input,
            identity_private_key_bounds: Bounds::ZERO,
            identity_load_button,
            identity_load_bounds: Bounds::ZERO,
            pending_identity_load,
            keygen_button,
            keygen_bounds: Bounds::ZERO,
            pending_keygen,
            identity_copy_npub_button,
            identity_copy_npub_bounds: Bounds::ZERO,
            pending_identity_copy_npub,
            identity_copy_nsec_button,
            identity_copy_nsec_bounds: Bounds::ZERO,
            pending_identity_copy_nsec,
            pylon_status: PylonStatusView::default(),
            pylon_toggle_button,
            pylon_toggle_bounds: Bounds::ZERO,
            pylon_init_button,
            pylon_start_button,
            pylon_stop_button,
            pylon_refresh_button,
            pylon_init_bounds: Bounds::ZERO,
            pylon_start_bounds: Bounds::ZERO,
            pylon_stop_bounds: Bounds::ZERO,
            pylon_refresh_bounds: Bounds::ZERO,
            pending_pylon_toggle,
            pending_pylon_init,
            pending_pylon_start,
            pending_pylon_stop,
            pending_pylon_refresh,
            wallet_status: WalletStatusView::default(),
            wallet_refresh_button,
            wallet_refresh_bounds: Bounds::ZERO,
            wallet_liquidity_button,
            wallet_liquidity_bounds: Bounds::ZERO,
            pending_wallet_refresh,
            pending_wallet_liquidity,
            liquidity_status: LiquidityProviderStatusView::default(),
            liquidity_online_button,
            liquidity_offline_button,
            liquidity_refresh_button,
            liquidity_online_bounds: Bounds::ZERO,
            liquidity_offline_bounds: Bounds::ZERO,
            liquidity_refresh_bounds: Bounds::ZERO,
            pending_liquidity_online,
            pending_liquidity_offline,
            pending_liquidity_refresh,
            liquidity_invoice_amount_input,
            liquidity_invoice_amount_bounds: Bounds::ZERO,
            liquidity_invoice_create_button,
            liquidity_invoice_create_bounds: Bounds::ZERO,
            pending_liquidity_invoice_create,
            liquidity_invoice_copy_button,
            liquidity_invoice_copy_bounds: Bounds::ZERO,
            pending_liquidity_invoice_copy,
            sell_compute_status: SellComputeStatusView::default(),
            sell_compute_online_button,
            sell_compute_offline_button,
            sell_compute_refresh_button,
            sell_compute_online_bounds: Bounds::ZERO,
            sell_compute_offline_bounds: Bounds::ZERO,
            sell_compute_refresh_bounds: Bounds::ZERO,
            pending_sell_compute_online,
            pending_sell_compute_offline,
            pending_sell_compute_refresh,
            dvm_history: DvmHistoryView::default(),
            dvm_history_refresh_button,
            dvm_history_refresh_bounds: Bounds::ZERO,
            pending_dvm_history_refresh,
            nip90_kind_input,
            nip90_kind_bounds: Bounds::ZERO,
            nip90_relay_input,
            nip90_relay_bounds: Bounds::ZERO,
            nip90_provider_input,
            nip90_provider_bounds: Bounds::ZERO,
            nip90_prompt_input,
            nip90_prompt_bounds: Bounds::ZERO,
            nip90_submit_button,
            nip90_submit_bounds: Bounds::ZERO,
            pending_nip90_submit,
            nip90_log: Vec::new(),
            nostr_npub: None,
            nostr_nsec: None,
            spark_pubkey_hex: None,
            seed_phrase: None,
            nostr_error: None,
            send_handler: None,
            event_log: Vec::new(),
            event_log_dirty: false,
            event_scroll,
            event_scroll_bounds: Bounds::ZERO,
            hotbar_bindings: HashMap::new(),
        };

        let screen = Size::new(1280.0, 720.0);
        root.toggle_identity_pane(screen);
        root.sync_route_state();
        root
    }

    pub fn set_clipboard(
        &mut self,
        read: impl Fn() -> Option<String> + 'static,
        write: impl Fn(&str) + 'static,
    ) {
        self.event_context.set_clipboard(read, write);
    }

    pub fn set_zoom_factor(&mut self, zoom: f32) {
        self.zoom_factor = zoom.max(0.1);
    }

    pub fn shortcut_context(&self) -> ShortcutContext {
        let text_input_focused = self.chat_panes.values().any(|chat| chat.input.is_focused())
            || self.file_editor.path_input.is_focused()
            || self.runtime_auth_email_input.is_focused()
            || self.runtime_auth_code_input.is_focused()
            || self.identity_private_key_input.is_focused()
            || self.liquidity_invoice_amount_input.is_focused();
        ShortcutContext { text_input_focused }
    }

    pub fn apply_shortcut(&mut self, command: ShortcutCommand) -> bool {
        let handled = match command {
            ShortcutCommand::HotbarSlot(slot) => {
                self.hotbar.flash_slot(slot);
                self.handle_hotbar_slot(slot)
            }
            ShortcutCommand::CycleChatFocus => self.cycle_chat_focus(),
            ShortcutCommand::CycleChatModel => self.cycle_chat_model(),
            ShortcutCommand::CloseActivePane => self.close_active_pane(),
            _ => false,
        };
        if handled {
            self.sync_route_state();
        }
        handled
    }

    pub fn apply_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::WorkspaceOpened { path, .. } => {
                self.file_editor.set_workspace_root(path.clone());
            }
            AppEvent::SessionStarted { session_id, .. } => {
                if let Some(pane_id) = self.pending_session_panes.pop_front() {
                    if let Some(chat) = self.chat_panes.get_mut(&pane_id) {
                        chat.set_session_id(session_id);
                    }
                    self.session_to_pane.insert(session_id, pane_id);
                }
            }
            AppEvent::PylonStatus { status } => {
                self.pylon_status = PylonStatusView {
                    running: status.running,
                    pid: status.pid,
                    uptime_secs: status.uptime_secs,
                    provider_active: status.provider_active,
                    host_active: status.host_active,
                    jobs_completed: status.jobs_completed,
                    earnings_msats: status.earnings_msats,
                    identity_exists: status.identity_exists,
                    last_error: status.last_error,
                };
            }
            AppEvent::WalletStatus { status } => {
                self.wallet_status = WalletStatusView {
                    network: status.network,
                    spark_sats: status.spark_sats,
                    lightning_sats: status.lightning_sats,
                    onchain_sats: status.onchain_sats,
                    total_sats: status.total_sats,
                    spark_address: status.spark_address,
                    bitcoin_address: status.bitcoin_address,
                    identity_exists: status.identity_exists,
                    last_error: status.last_error,
                };
            }
            AppEvent::LiquidityProviderStatus { status } => {
                self.liquidity_status = LiquidityProviderStatusView {
                    running: status.running,
                    provider_active: status.provider_active,
                    worker_id: status.worker_id,
                    earned_sats: status.earned_sats,
                    max_invoice_sats: status.max_invoice_sats,
                    max_hourly_sats: status.max_hourly_sats,
                    max_daily_sats: status.max_daily_sats,
                    last_invoice: status.last_invoice,
                    last_error: status.last_error,
                };
            }
            AppEvent::DvmProviderStatus { status } => {
                self.sell_compute_status = SellComputeStatusView {
                    running: status.running,
                    provider_active: status.provider_active,
                    host_active: status.host_active,
                    min_price_msats: status.min_price_msats,
                    require_payment: status.require_payment,
                    default_model: status.default_model,
                    backend_preference: status.backend_preference,
                    agent_backends: status.agent_backends,
                    supported_bazaar_kinds: status.supported_bazaar_kinds,
                    network: status.network,
                    enable_payments: status.enable_payments,
                    last_error: status.last_error,
                };
            }
            AppEvent::DvmHistory { snapshot } => {
                self.dvm_history = DvmHistoryView {
                    summary_total_msats: snapshot.summary.total_msats,
                    summary_total_sats: snapshot.summary.total_sats,
                    summary_job_count: snapshot.summary.job_count,
                    summary_by_source: snapshot.summary.by_source,
                    status_counts: snapshot.status_counts,
                    jobs: snapshot
                        .jobs
                        .into_iter()
                        .map(|job| DvmJobView {
                            id: job.id,
                            status: job.status,
                            kind: job.kind,
                            price_msats: job.price_msats,
                            created_at: job.created_at,
                        })
                        .collect(),
                    last_error: snapshot.last_error,
                };
            }
            AppEvent::RuntimeAuthState { state } => {
                if !self.runtime_auth_email_input.is_focused() {
                    if let Some(email) = state.pending_email.as_deref().or(state.email.as_deref()) {
                        self.runtime_auth_email_input.set_value(email.to_string());
                    }
                }
                if state.token_present && state.pending_email.is_none() {
                    self.runtime_auth_code_input.set_value(String::new());
                }
                self.runtime_auth = state;
            }
            AppEvent::Nip90Log { message } => {
                self.nip90_log.push(message);
                if self.nip90_log.len() > 200 {
                    let drain = self.nip90_log.len() - 200;
                    self.nip90_log.drain(0..drain);
                }
            }
            AppEvent::ThreadsUpdated {
                threads,
                next_cursor,
                append,
            } => {
                self.set_thread_entries(threads, append);
                self.threads_next_cursor = next_cursor;
            }
            AppEvent::ThreadLoaded {
                session_id,
                thread,
                model,
            } => {
                let pane_id = self
                    .pane_for_session_id(&session_id.to_string())
                    .unwrap_or_else(|| {
                        self.open_chat_pane(self.screen_size(), true, false, DEFAULT_THREAD_MODEL)
                    });
                if let Some(chat) = self.chat_panes.get_mut(&pane_id) {
                    chat.load_thread_snapshot(&thread);
                    chat.update_thread_model(model.as_str());
                    chat.set_session_id(session_id);
                    chat.thread_id = Some(thread.id.clone());
                }
                self.pane_store
                    .set_title(&pane_id, format!("Thread {}", thread.id));
                self.session_to_pane.insert(session_id, pane_id.clone());
                self.thread_to_pane.insert(thread.id, pane_id);
            }
            AppEvent::InboxUpdated { snapshot, source } => {
                self.inbox.apply_snapshot(snapshot, source);
            }
            AppEvent::FileOpened { path, contents } => {
                self.file_editor.set_contents(path, contents);
            }
            AppEvent::FileOpenFailed { path, error } => {
                self.file_editor.set_error(path, error);
            }
            AppEvent::FileSaved { path } => {
                self.file_editor.set_saved(path);
            }
            AppEvent::FileSaveFailed { path, error } => {
                self.file_editor.set_save_error(path, error);
            }
            AppEvent::AppServerEvent { message } => {
                if let Ok(value) = serde_json::from_str::<Value>(&message) {
                    let method = value.get("method").and_then(|m| m.as_str()).unwrap_or("");
                    let params = value.get("params");
                    let session_hint = extract_session_hint(params);
                    let thread_hint = extract_thread_hint(params);

                    let pane_id = session_hint
                        .as_deref()
                        .and_then(|session_id| self.pane_for_session_id(session_id))
                        .or_else(|| {
                            thread_hint
                                .as_deref()
                                .and_then(|thread_id| self.thread_to_pane.get(thread_id).cloned())
                        })
                        .or_else(|| self.pane_with_active_turn())
                        .or_else(|| {
                            self.pane_store
                                .active_pane_id
                                .clone()
                                .filter(|pane_id| self.chat_panes.contains_key(pane_id))
                        })
                        .or_else(|| self.first_chat_without_thread());

                    if let Some(pane_id) = pane_id {
                        if let Some(chat) = self.chat_panes.get_mut(&pane_id) {
                            if method == "thread/started" {
                                if let Some(thread_id) = thread_hint.as_deref() {
                                    chat.thread_id = Some(thread_id.to_string());
                                    self.thread_to_pane
                                        .insert(thread_id.to_string(), pane_id.clone());
                                    self.pane_store
                                        .set_title(&pane_id, format!("Thread {}", thread_id));
                                }
                                if let Some(model) = params
                                    .and_then(|p| p.get("model"))
                                    .and_then(|m| m.as_str())
                                    .or_else(|| {
                                        params
                                            .and_then(|p| p.get("thread"))
                                            .and_then(|thread| thread.get("model"))
                                            .and_then(|m| m.as_str())
                                    })
                                {
                                    chat.update_thread_model(model);
                                }
                                chat.flush_queue_if_idle(&mut self.send_handler);
                            }

                            if method == "turn/started" {
                                if let Some(turn_id) = params
                                    .and_then(|p| p.get("turnId"))
                                    .and_then(|id| id.as_str())
                                    .or_else(|| {
                                        params
                                            .and_then(|p| p.get("turn"))
                                            .and_then(|turn| turn.get("id"))
                                            .and_then(|id| id.as_str())
                                    })
                                {
                                    chat.active_turn_id = Some(turn_id.to_string());
                                    chat.queued_in_flight = false;
                                    chat.show_working_indicator();
                                }
                            }

                            if matches!(
                                method,
                                "turn/completed"
                                    | "turn/failed"
                                    | "turn/aborted"
                                    | "turn/interrupted"
                            ) {
                                let completed_turn = params
                                    .and_then(|p| p.get("turnId"))
                                    .and_then(|id| id.as_str())
                                    .or_else(|| {
                                        params
                                            .and_then(|p| p.get("turn"))
                                            .and_then(|turn| turn.get("id"))
                                            .and_then(|id| id.as_str())
                                    });
                                if completed_turn
                                    .map(|id| chat.active_turn_id.as_deref() == Some(id))
                                    .unwrap_or(true)
                                {
                                    chat.active_turn_id = None;
                                    chat.clear_working_indicator();
                                    chat.flush_queue_if_idle(&mut self.send_handler);
                                }
                            }

                            if method == "fullauto/status" {
                                if let Some(enabled) = params
                                    .and_then(|p| p.get("enabled"))
                                    .and_then(|value| value.as_bool())
                                {
                                    chat.full_auto_enabled = enabled;
                                    if !enabled {
                                        chat.guidance_mode_active = false;
                                    }
                                }
                            }

                            if method == "fullauto/decision" {
                                if let Some(action) = params
                                    .and_then(|p| p.get("action"))
                                    .and_then(|value| value.as_str())
                                {
                                    if action != "continue" {
                                        chat.full_auto_enabled = false;
                                        chat.guidance_mode_active = false;
                                    }
                                }
                            }

                            chat.apply_formatted_event(&value);
                            if method == "guidance/response" {
                                chat.flush_queue_if_idle(&mut self.send_handler);
                            }
                        }
                    }
                }

                self.event_log.push(message);
                if self.event_log.len() > 200 {
                    let drain = self.event_log.len() - 200;
                    self.event_log.drain(0..drain);
                }
                self.event_log_dirty = true;
            }
            _ => {}
        }
        self.sync_route_state();
    }

    pub fn set_send_handler<F>(&mut self, handler: F)
    where
        F: FnMut(UserAction) + 'static,
    {
        self.send_handler = Some(Box::new(handler));
    }

    fn screen_size(&self) -> Size {
        self.screen_size
    }

    fn set_screen_size(&mut self, size: Size) {
        self.screen_size = size;
    }

    fn inbox_route_for_kind(kind: PaneKind) -> Option<InboxRoutePane> {
        match kind {
            PaneKind::InboxList => Some(InboxRoutePane::List),
            PaneKind::InboxThread => Some(InboxRoutePane::Thread),
            PaneKind::InboxApprovals => Some(InboxRoutePane::Approvals),
            PaneKind::InboxAudit => Some(InboxRoutePane::Audit),
            _ => None,
        }
    }

    fn is_inbox_kind(kind: PaneKind) -> bool {
        matches!(
            kind,
            PaneKind::InboxList
                | PaneKind::InboxThread
                | PaneKind::InboxApprovals
                | PaneKind::InboxAudit
        )
    }

    fn sync_route_state(&mut self) {
        let active_kind = self
            .pane_store
            .active_pane_id
            .as_ref()
            .and_then(|pane_id| self.pane_store.pane(pane_id))
            .map(|pane| pane.kind.clone());
        let has_chat_open = self
            .pane_store
            .panes()
            .iter()
            .any(|pane| pane.kind == PaneKind::Chat);
        let has_inbox_open = self
            .pane_store
            .panes()
            .iter()
            .any(|pane| Self::is_inbox_kind(pane.kind.clone()));
        let active_surface = match active_kind.clone() {
            Some(PaneKind::Chat) => DesktopSurfaceRoute::Codex,
            Some(kind) if Self::is_inbox_kind(kind.clone()) => DesktopSurfaceRoute::Inbox,
            _ if has_chat_open && has_inbox_open => DesktopSurfaceRoute::Mixed,
            _ if has_chat_open => DesktopSurfaceRoute::Codex,
            _ if has_inbox_open => DesktopSurfaceRoute::Inbox,
            _ => DesktopSurfaceRoute::Mixed,
        };
        let codex_thread_id = self
            .pane_store
            .active_pane_id
            .as_ref()
            .and_then(|pane_id| self.chat_panes.get(pane_id))
            .and_then(|chat| chat.thread_id.clone())
            .or_else(|| {
                self.chat_panes
                    .values()
                    .find_map(|chat| chat.thread_id.clone())
            });
        let inbox_pane = active_kind
            .and_then(Self::inbox_route_for_kind)
            .or_else(|| {
                self.pane_store
                    .panes()
                    .iter()
                    .rev()
                    .find_map(|pane| Self::inbox_route_for_kind(pane.kind.clone()))
            });

        self.route_state = DesktopRouteState {
            active_surface,
            codex_thread_id,
            inbox_thread_id: self.inbox.selected_thread_id.clone(),
            inbox_pane,
        };
    }

    fn open_chat_pane(
        &mut self,
        screen: Size,
        reset_chat: bool,
        request_session: bool,
        default_model: &str,
    ) -> String {
        let chat_index = self.next_chat_index;
        let id = format!("chat-{}", chat_index);
        self.next_chat_index += 1;
        let rect = calculate_new_pane_position(
            self.pane_store.last_pane_position,
            screen,
            CHAT_PANE_WIDTH,
            CHAT_PANE_HEIGHT,
        );
        let pane = Pane {
            id: id.clone(),
            kind: PaneKind::Chat,
            title: "Thread".to_string(),
            rect: normalize_pane_rect(rect),
            dismissable: true,
        };
        self.pane_store.add_pane(pane);

        let mut chat_state = ChatPaneState::new(default_model);
        if reset_chat {
            chat_state.reset_chat_state();
        }
        self.chat_panes.insert(id.clone(), chat_state);
        self.chat_slot_labels
            .insert(id.clone(), format!("Chat {}", chat_index));
        self.assign_chat_slot(&id);
        if request_session {
            self.pending_session_panes.push_back(id.clone());
        }

        id
    }

    fn set_thread_entries(&mut self, threads: Vec<ThreadSummary>, append: bool) {
        let mut existing_ids = HashSet::new();
        let mut entries = if append {
            for entry in &self.thread_entries {
                existing_ids.insert(entry.summary.id.clone());
            }
            std::mem::take(&mut self.thread_entries)
        } else {
            Vec::new()
        };

        for summary in threads {
            if append && existing_ids.contains(&summary.id) {
                continue;
            }
            let pending = self.pending_thread_open.clone();
            let thread_id = summary.id.clone();
            let button = Button::new("")
                .variant(ButtonVariant::Ghost)
                .font_size(theme::font_size::XS)
                .padding(0.0, 0.0)
                .corner_radius(0.0)
                .on_click(move || {
                    *pending.borrow_mut() = Some(thread_id.clone());
                });
            let branch = git_branch_for_cwd(&summary.cwd);
            entries.push(ThreadEntryView {
                summary,
                branch,
                open_button: button,
                open_bounds: Bounds::ZERO,
            });
        }

        self.thread_entries = entries;
    }

    fn open_thread_from_list(&mut self, thread_id: String) {
        let screen = self.screen_size();
        let pane_id = self.open_chat_pane(screen, true, true, DEFAULT_THREAD_MODEL);
        if let Some(handler) = self.send_handler.as_mut() {
            handler(UserAction::ThreadOpen { thread_id });
        }
        self.pane_store.bring_to_front(&pane_id);
    }

    fn toggle_events_pane(&mut self, screen: Size) {
        let last_position = self.pane_store.last_pane_position;
        self.pane_store.toggle_pane("events", screen, |snapshot| {
            let rect = snapshot
                .as_ref()
                .map(|snapshot| snapshot.rect)
                .unwrap_or_else(|| {
                    calculate_new_pane_position(
                        last_position,
                        screen,
                        EVENTS_PANE_WIDTH,
                        EVENTS_PANE_HEIGHT,
                    )
                });
            Pane {
                id: "events".to_string(),
                kind: PaneKind::Events,
                title: "Codex Events".to_string(),
                rect,
                dismissable: true,
            }
        });
    }

    fn toggle_threads_pane(&mut self, screen: Size) {
        let last_position = self.pane_store.last_pane_position;
        self.pane_store.toggle_pane("threads", screen, |snapshot| {
            let rect = snapshot
                .as_ref()
                .map(|snapshot| snapshot.rect)
                .unwrap_or_else(|| {
                    calculate_new_pane_position(
                        last_position,
                        screen,
                        THREADS_PANE_WIDTH,
                        THREADS_PANE_HEIGHT,
                    )
                });
            Pane {
                id: "threads".to_string(),
                kind: PaneKind::Threads,
                title: "Recent Threads".to_string(),
                rect,
                dismissable: true,
            }
        });
    }

    fn toggle_inbox_list_pane(&mut self, screen: Size) {
        let last_position = self.pane_store.last_pane_position;
        self.pane_store
            .toggle_pane("inbox-list", screen, |snapshot| {
                let rect = snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.rect)
                    .unwrap_or_else(|| {
                        calculate_new_pane_position(
                            last_position,
                            screen,
                            INBOX_LIST_PANE_WIDTH,
                            INBOX_LIST_PANE_HEIGHT,
                        )
                    });
                Pane {
                    id: "inbox-list".to_string(),
                    kind: PaneKind::InboxList,
                    title: "Inbox".to_string(),
                    rect,
                    dismissable: true,
                }
            });
    }

    fn toggle_inbox_thread_pane(&mut self, screen: Size) {
        let last_position = self.pane_store.last_pane_position;
        self.pane_store
            .toggle_pane("inbox-thread", screen, |snapshot| {
                let rect = snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.rect)
                    .unwrap_or_else(|| {
                        calculate_new_pane_position(
                            last_position,
                            screen,
                            INBOX_THREAD_PANE_WIDTH,
                            INBOX_THREAD_PANE_HEIGHT,
                        )
                    });
                Pane {
                    id: "inbox-thread".to_string(),
                    kind: PaneKind::InboxThread,
                    title: "Inbox Thread".to_string(),
                    rect,
                    dismissable: true,
                }
            });
    }

    fn toggle_inbox_approvals_pane(&mut self, screen: Size) {
        let last_position = self.pane_store.last_pane_position;
        self.pane_store
            .toggle_pane("inbox-approvals", screen, |snapshot| {
                let rect = snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.rect)
                    .unwrap_or_else(|| {
                        calculate_new_pane_position(
                            last_position,
                            screen,
                            INBOX_APPROVALS_PANE_WIDTH,
                            INBOX_APPROVALS_PANE_HEIGHT,
                        )
                    });
                Pane {
                    id: "inbox-approvals".to_string(),
                    kind: PaneKind::InboxApprovals,
                    title: "Inbox Approvals".to_string(),
                    rect,
                    dismissable: true,
                }
            });
    }

    fn toggle_inbox_audit_pane(&mut self, screen: Size) {
        let last_position = self.pane_store.last_pane_position;
        self.pane_store
            .toggle_pane("inbox-audit", screen, |snapshot| {
                let rect = snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.rect)
                    .unwrap_or_else(|| {
                        calculate_new_pane_position(
                            last_position,
                            screen,
                            INBOX_AUDIT_PANE_WIDTH,
                            INBOX_AUDIT_PANE_HEIGHT,
                        )
                    });
                Pane {
                    id: "inbox-audit".to_string(),
                    kind: PaneKind::InboxAudit,
                    title: "Inbox Audit".to_string(),
                    rect,
                    dismissable: true,
                }
            });
    }

    fn toggle_file_editor_pane(&mut self, screen: Size) {
        let last_position = self.pane_store.last_pane_position;
        self.pane_store
            .toggle_pane("file_editor", screen, |snapshot| {
                let rect = snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.rect)
                    .unwrap_or_else(|| {
                        calculate_new_pane_position(
                            last_position,
                            screen,
                            FILE_EDITOR_PANE_WIDTH,
                            FILE_EDITOR_PANE_HEIGHT,
                        )
                    });
                Pane {
                    id: "file_editor".to_string(),
                    kind: PaneKind::FileEditor,
                    title: "File Editor".to_string(),
                    rect,
                    dismissable: true,
                }
            });
    }

    fn toggle_auth_pane(&mut self, screen: Size) {
        let last_position = self.pane_store.last_pane_position;
        self.pane_store.toggle_pane("auth", screen, |snapshot| {
            let rect = snapshot
                .as_ref()
                .map(|snapshot| snapshot.rect)
                .unwrap_or_else(|| {
                    calculate_new_pane_position(
                        last_position,
                        screen,
                        AUTH_PANE_WIDTH,
                        AUTH_PANE_HEIGHT,
                    )
                });
            Pane {
                id: "auth".to_string(),
                kind: PaneKind::Auth,
                title: "Runtime Login".to_string(),
                rect,
                dismissable: true,
            }
        });

        if self.pane_store.is_active("auth") {
            if let Some(handler) = self.send_handler.as_mut() {
                handler(UserAction::RuntimeAuthStatus);
            }
        }
    }

    fn toggle_identity_pane(&mut self, screen: Size) {
        let last_position = self.pane_store.last_pane_position;
        self.pane_store.toggle_pane("identity", screen, |snapshot| {
            let rect = snapshot
                .as_ref()
                .map(|snapshot| snapshot.rect)
                .unwrap_or_else(|| {
                    calculate_new_pane_position(
                        last_position,
                        screen,
                        IDENTITY_PANE_WIDTH,
                        IDENTITY_PANE_HEIGHT,
                    )
                });
            Pane {
                id: "identity".to_string(),
                kind: PaneKind::Identity,
                title: "Identity".to_string(),
                rect,
                dismissable: true,
            }
        });
    }

    fn toggle_pylon_pane(&mut self, screen: Size) {
        let last_position = self.pane_store.last_pane_position;
        self.pane_store.toggle_pane("pylon", screen, |snapshot| {
            let rect = snapshot
                .as_ref()
                .map(|snapshot| snapshot.rect)
                .unwrap_or_else(|| {
                    calculate_new_pane_position(
                        last_position,
                        screen,
                        PYLON_PANE_WIDTH,
                        PYLON_PANE_HEIGHT,
                    )
                });
            Pane {
                id: "pylon".to_string(),
                kind: PaneKind::Pylon,
                title: "Pylon".to_string(),
                rect,
                dismissable: true,
            }
        });

        if self.pane_store.is_active("pylon") {
            if let Some(handler) = self.send_handler.as_mut() {
                handler(UserAction::PylonRefresh);
            }
        }
    }

    fn toggle_wallet_pane(&mut self, screen: Size) {
        let last_position = self.pane_store.last_pane_position;
        self.pane_store.toggle_pane("wallet", screen, |snapshot| {
            let rect = snapshot
                .as_ref()
                .map(|snapshot| snapshot.rect)
                .unwrap_or_else(|| {
                    calculate_new_pane_position(
                        last_position,
                        screen,
                        WALLET_PANE_WIDTH,
                        WALLET_PANE_HEIGHT,
                    )
                });
            Pane {
                id: "wallet".to_string(),
                kind: PaneKind::Wallet,
                title: "Wallet".to_string(),
                rect,
                dismissable: true,
            }
        });

        if self.pane_store.is_active("wallet") {
            if let Some(handler) = self.send_handler.as_mut() {
                handler(UserAction::WalletRefresh);
            }
        }
    }

    fn toggle_liquidity_pane(&mut self, screen: Size) {
        let last_position = self.pane_store.last_pane_position;
        self.pane_store
            .toggle_pane("liquidity", screen, |snapshot| {
                let rect = snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.rect)
                    .unwrap_or_else(|| {
                        calculate_new_pane_position(
                            last_position,
                            screen,
                            LIQUIDITY_PANE_WIDTH,
                            LIQUIDITY_PANE_HEIGHT,
                        )
                    });
                Pane {
                    id: "liquidity".to_string(),
                    kind: PaneKind::Liquidity,
                    title: "Liquidity".to_string(),
                    rect,
                    dismissable: true,
                }
            });

        if self.pane_store.is_active("liquidity") {
            if let Some(handler) = self.send_handler.as_mut() {
                handler(UserAction::WalletRefresh);
                handler(UserAction::LiquidityProviderRefresh);
            }
        }
    }

    fn toggle_sell_compute_pane(&mut self, screen: Size) {
        let last_position = self.pane_store.last_pane_position;
        self.pane_store
            .toggle_pane("sell_compute", screen, |snapshot| {
                let rect = snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.rect)
                    .unwrap_or_else(|| {
                        calculate_new_pane_position(
                            last_position,
                            screen,
                            SELL_COMPUTE_PANE_WIDTH,
                            SELL_COMPUTE_PANE_HEIGHT,
                        )
                    });
                Pane {
                    id: "sell_compute".to_string(),
                    kind: PaneKind::SellCompute,
                    title: "Sell Compute".to_string(),
                    rect,
                    dismissable: true,
                }
            });

        if self.pane_store.is_active("sell_compute") {
            if let Some(handler) = self.send_handler.as_mut() {
                handler(UserAction::DvmProviderRefresh);
            }
        }
    }

    fn toggle_dvm_history_pane(&mut self, screen: Size) {
        let last_position = self.pane_store.last_pane_position;
        self.pane_store
            .toggle_pane("dvm_history", screen, |snapshot| {
                let rect = snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.rect)
                    .unwrap_or_else(|| {
                        calculate_new_pane_position(
                            last_position,
                            screen,
                            HISTORY_PANE_WIDTH,
                            HISTORY_PANE_HEIGHT,
                        )
                    });
                Pane {
                    id: "dvm_history".to_string(),
                    kind: PaneKind::DvmHistory,
                    title: "DVM History".to_string(),
                    rect,
                    dismissable: true,
                }
            });

        if self.pane_store.is_active("dvm_history") {
            if let Some(handler) = self.send_handler.as_mut() {
                handler(UserAction::DvmHistoryRefresh);
            }
        }
    }

    fn toggle_nip90_pane(&mut self, screen: Size) {
        let last_position = self.pane_store.last_pane_position;
        self.pane_store.toggle_pane("nip90", screen, |snapshot| {
            let rect = snapshot
                .as_ref()
                .map(|snapshot| snapshot.rect)
                .unwrap_or_else(|| {
                    calculate_new_pane_position(
                        last_position,
                        screen,
                        NIP90_PANE_WIDTH,
                        NIP90_PANE_HEIGHT,
                    )
                });
            Pane {
                id: "nip90".to_string(),
                kind: PaneKind::Nip90,
                title: "NIP-90".to_string(),
                rect,
                dismissable: true,
            }
        });
    }

    fn close_pane(&mut self, id: &str) {
        self.pane_store.remove_pane(id, true);
        self.pane_frames.remove(id);
        self.pane_bounds.remove(id);
        if id == "file_editor" {
            self.file_editor.blur_editors();
        }
        if let Some(chat) = self.chat_panes.remove(id) {
            if let Some(session_id) = chat.session_id {
                self.session_to_pane.remove(&session_id);
            }
            if let Some(thread_id) = chat.thread_id {
                self.thread_to_pane.remove(&thread_id);
            }
        }
        self.chat_slot_assignments.remove(id);
        self.chat_slot_labels.remove(id);
    }

    fn close_active_pane(&mut self) -> bool {
        let Some(active_id) = self.pane_store.active_pane_id.clone() else {
            return false;
        };
        let dismissable = self
            .pane_store
            .pane(&active_id)
            .map(|pane| pane.dismissable)
            .unwrap_or(true);
        if dismissable {
            self.close_pane(&active_id);
            return true;
        }
        false
    }

    fn assign_chat_slot(&mut self, pane_id: &str) {
        if self.chat_slot_assignments.contains_key(pane_id) {
            return;
        }
        for slot in HOTBAR_CHAT_SLOT_START..=HOTBAR_SLOT_MAX {
            if self
                .chat_slot_assignments
                .values()
                .any(|assigned| *assigned == slot)
            {
                continue;
            }
            self.chat_slot_assignments.insert(pane_id.to_string(), slot);
            break;
        }
    }

    fn handle_hotbar_slot(&mut self, slot: u8) -> bool {
        let screen = self.screen_size();
        let Some(action) = self.hotbar_bindings.get(&slot).cloned() else {
            return false;
        };
        match action {
            HotbarAction::FocusPane(pane_id) => {
                if self.pane_store.is_active(&pane_id) {
                    self.close_pane(&pane_id);
                } else {
                    self.pane_store.bring_to_front(&pane_id);
                }
                true
            }
            HotbarAction::ToggleEvents => {
                self.toggle_events_pane(screen);
                true
            }
            HotbarAction::ToggleThreads => {
                self.toggle_threads_pane(screen);
                if self.pane_store.is_active("threads") {
                    if let Some(handler) = self.send_handler.as_mut() {
                        handler(UserAction::ThreadsRefresh);
                    }
                }
                true
            }
            HotbarAction::ToggleInboxList => {
                self.toggle_inbox_list_pane(screen);
                if self.pane_store.is_active("inbox-list") {
                    if let Some(handler) = self.send_handler.as_mut() {
                        handler(UserAction::InboxRefresh);
                    }
                }
                true
            }
            HotbarAction::ToggleFileEditor => {
                self.toggle_file_editor_pane(screen);
                true
            }
            HotbarAction::ToggleAuth => {
                self.toggle_auth_pane(screen);
                true
            }
            HotbarAction::ToggleIdentity => {
                self.toggle_identity_pane(screen);
                true
            }
            HotbarAction::TogglePylon => {
                self.toggle_pylon_pane(screen);
                true
            }
            HotbarAction::ToggleWallet => {
                self.toggle_wallet_pane(screen);
                true
            }
            HotbarAction::ToggleSellCompute => {
                self.toggle_sell_compute_pane(screen);
                true
            }
            HotbarAction::ToggleDvmHistory => {
                self.toggle_dvm_history_pane(screen);
                true
            }
            HotbarAction::ToggleNip90 => {
                self.toggle_nip90_pane(screen);
                true
            }
            HotbarAction::NewChat => {
                let active_model = self
                    .pane_store
                    .active_pane_id
                    .as_ref()
                    .and_then(|id| self.chat_panes.get(id))
                    .map(|chat| chat.selected_model.clone())
                    .unwrap_or_else(|| DEFAULT_THREAD_MODEL.to_string());
                let active_reasoning = self
                    .pane_store
                    .active_pane_id
                    .as_ref()
                    .and_then(|id| self.chat_panes.get(id))
                    .map(|chat| chat.selected_reasoning.clone())
                    .unwrap_or_else(|| DEFAULT_REASONING_EFFORT.to_string());
                let new_pane = self.open_chat_pane(screen, true, true, active_model.as_str());
                if let Some(chat) = self.chat_panes.get_mut(&new_pane) {
                    chat.update_reasoning(&active_reasoning);
                }
                let session_id = self
                    .pane_store
                    .active_pane_id
                    .as_ref()
                    .and_then(|id| self.chat_panes.get(id))
                    .and_then(|chat| chat.session_id)
                    .or_else(|| self.session_to_pane.keys().next().copied())
                    .unwrap_or_else(SessionId::new);
                if let Some(handler) = self.send_handler.as_mut() {
                    handler(UserAction::NewChat {
                        session_id,
                        model: Some(active_model.clone()),
                    });
                }
                self.pane_store.bring_to_front(&new_pane);
                true
            }
        }
    }

    fn cycle_chat_model(&mut self) -> bool {
        if let Some(active_id) = self.pane_store.active_pane_id.clone() {
            if let Some(chat) = self.chat_panes.get_mut(&active_id) {
                let options = build_model_options();
                if !options.is_empty() {
                    let current = model_index(&chat.selected_model).unwrap_or(0);
                    let next = (current + 1) % options.len();
                    let value = options[next].value.clone();
                    chat.pending_model_changes.borrow_mut().push(value);
                    return true;
                }
            }
        }
        false
    }

    fn pane_for_session_id(&self, session_id: &str) -> Option<String> {
        self.session_to_pane
            .iter()
            .find(|(id, _)| id.to_string() == session_id)
            .map(|(_, pane)| pane.clone())
    }

    fn first_chat_without_thread(&self) -> Option<String> {
        self.chat_panes
            .iter()
            .find(|(_, chat)| chat.thread_id.is_none())
            .map(|(id, _)| id.clone())
    }

    fn pane_with_active_turn(&self) -> Option<String> {
        for pane in self.pane_store.panes().iter().rev() {
            if self
                .chat_panes
                .get(&pane.id)
                .map(|chat| chat.active_turn_id.is_some() || chat.queued_in_flight)
                .unwrap_or(false)
            {
                return Some(pane.id.clone());
            }
        }
        None
    }

    fn pane_at(&self, point: Point) -> Option<String> {
        for pane in self.pane_store.panes().iter().rev() {
            if let Some(bounds) = self.pane_bounds.get(&pane.id) {
                if bounds.contains(point) {
                    return Some(pane.id.clone());
                }
            }
        }
        None
    }

    fn pane_resize_target(&self, point: Point) -> Option<(String, ResizeEdge)> {
        for pane in self.pane_store.panes().iter().rev() {
            let Some(bounds) = self.pane_bounds.get(&pane.id) else {
                continue;
            };
            if let Some(frame) = self.pane_frames.get(&pane.id) {
                if frame.close_bounds().contains(point) {
                    continue;
                }
            }
            let edge = self.pane_resizer.edge_at(*bounds, point);
            if edge != ResizeEdge::None {
                return Some((pane.id.clone(), edge));
            }
        }
        None
    }

    fn cycle_chat_focus(&mut self) -> bool {
        let mut chat_ids = Vec::new();
        for pane in self.pane_store.panes() {
            if self.chat_panes.contains_key(&pane.id) {
                chat_ids.push(pane.id.clone());
            }
        }
        if chat_ids.is_empty() {
            return false;
        }

        let focused_id = self
            .chat_panes
            .iter()
            .find(|(_, chat)| chat.input.is_focused())
            .map(|(id, _)| id.clone());
        let active_id = self
            .pane_store
            .active_pane_id
            .clone()
            .filter(|id| self.chat_panes.contains_key(id));

        let start_id = focused_id.or(active_id);
        let start_index =
            start_id.and_then(|id| chat_ids.iter().position(|pane_id| pane_id == &id));

        let next_index = match start_index {
            Some(index) => (index + 1) % chat_ids.len(),
            None => 0,
        };
        let next_id = chat_ids[next_index].clone();

        for chat in self.chat_panes.values_mut() {
            chat.input.blur();
        }

        if let Some(chat) = self.chat_panes.get_mut(&next_id) {
            chat.input.focus();
            chat.input_needs_focus = false;
        }
        self.pane_store.bring_to_front(&next_id);
        true
    }

    pub fn handle_input(&mut self, event: &InputEvent, _bounds: Bounds) -> bool {
        if let InputEvent::MouseMove { x, y }
        | InputEvent::MouseDown { x, y, .. }
        | InputEvent::MouseUp { x, y, .. } = event
        {
            self.cursor_position = Point::new(*x, *y);
            for chat in self.chat_panes.values_mut() {
                chat.input_hovered = chat.input_bounds.contains(self.cursor_position);
                chat.model_hovered = if SHOW_MODEL_SELECTOR {
                    chat.model_bounds.contains(self.cursor_position)
                } else {
                    false
                };
                chat.reasoning_hovered = if SHOW_MODEL_SELECTOR {
                    chat.reasoning_bounds.contains(self.cursor_position)
                } else {
                    false
                };
            }
        }

        if let Some(pan) = self.canvas_pan.clone() {
            match event {
                InputEvent::MouseMove { x, y } => {
                    let next = Point::new(*x, *y);
                    let dx = next.x - pan.last.x;
                    let dy = next.y - pan.last.y;
                    self.pane_store.offset_all(dx, dy);
                    self.background_offset = Point::new(
                        (self.background_offset.x + dx).rem_euclid(GRID_DOT_DISTANCE),
                        (self.background_offset.y + dy).rem_euclid(GRID_DOT_DISTANCE),
                    );
                    self.canvas_pan = Some(CanvasPanState { last: next });
                    return true;
                }
                InputEvent::MouseUp {
                    button: MouseButton::Left,
                    ..
                } => {
                    self.canvas_pan = None;
                    return true;
                }
                _ => {}
            }
        }

        if let InputEvent::MouseDown {
            button: MouseButton::Left,
            ..
        } = event
        {
            if self.pane_resize.is_none() && self.pane_drag.is_none() {
                let over_pane = self.pane_at(self.cursor_position).is_some();
                let over_hotbar = self.hotbar_bounds.contains(self.cursor_position);
                if !over_pane && !over_hotbar {
                    self.canvas_pan = Some(CanvasPanState {
                        last: self.cursor_position,
                    });
                    return true;
                }
            }

            if self.pane_resize.is_none() {
                if let Some((pane_id, edge)) = self.pane_resize_target(self.cursor_position) {
                    if let Some(pane) = self.pane_store.pane(&pane_id) {
                        self.pane_resize = Some(PaneResizeState {
                            pane_id: pane_id.clone(),
                            edge,
                            origin: self.cursor_position,
                            start_rect: pane.rect.clone(),
                        });
                        self.pane_store.bring_to_front(&pane_id);
                        return true;
                    }
                }
            }
        }

        if let Some(resize) = self.pane_resize.clone() {
            match event {
                InputEvent::MouseMove { x, y } => {
                    let start_bounds = Bounds::new(
                        resize.start_rect.x,
                        resize.start_rect.y,
                        resize.start_rect.width,
                        resize.start_rect.height,
                    );
                    let next_bounds = self.pane_resizer.resize_bounds(
                        resize.edge,
                        start_bounds,
                        resize.origin,
                        Point::new(*x, *y),
                    );
                    let mut rect = PaneRect {
                        x: next_bounds.origin.x,
                        y: next_bounds.origin.y,
                        width: next_bounds.size.width,
                        height: next_bounds.size.height,
                    };
                    rect = normalize_pane_rect(rect);
                    self.pane_store.update_rect(&resize.pane_id, rect);
                    return true;
                }
                InputEvent::MouseUp {
                    button: MouseButton::Left,
                    ..
                } => {
                    if let Some(rect) = self
                        .pane_store
                        .pane(&resize.pane_id)
                        .map(|pane| pane.rect.clone())
                    {
                        let rect = normalize_pane_rect(rect);
                        self.pane_store.update_rect(&resize.pane_id, rect);
                        self.pane_store.set_last_position(rect);
                    }
                    self.pane_resize = None;
                    return true;
                }
                _ => {}
            }
        }

        if let Some(drag) = self.pane_drag.clone() {
            match event {
                InputEvent::MouseMove { x, y } => {
                    let dx = x - drag.origin.x;
                    let dy = y - drag.origin.y;
                    let mut rect = drag.start_rect.clone();
                    rect.x += dx;
                    rect.y += dy;
                    rect = normalize_pane_rect(rect);
                    self.pane_store.update_rect(&drag.pane_id, rect);
                    return true;
                }
                InputEvent::MouseUp {
                    button: MouseButton::Left,
                    ..
                } => {
                    if let Some(rect) = self
                        .pane_store
                        .pane(&drag.pane_id)
                        .map(|pane| pane.rect.clone())
                    {
                        let rect = normalize_pane_rect(rect);
                        self.pane_store.update_rect(&drag.pane_id, rect);
                        self.pane_store.set_last_position(rect);
                    }
                    self.pane_drag = None;
                    return true;
                }
                _ => {}
            }
        }

        let mut handled = matches!(
            self.hotbar
                .event(event, self.hotbar_bounds, &mut self.event_context),
            EventResult::Handled
        );

        let hotbar_clicks = self.hotbar.take_clicked_slots();
        for slot in hotbar_clicks {
            handled |= self.handle_hotbar_slot(slot);
        }

        let target_pane = match event {
            InputEvent::MouseMove { .. }
            | InputEvent::MouseDown { .. }
            | InputEvent::MouseUp { .. }
            | InputEvent::Scroll { .. } => self.pane_at(self.cursor_position),
            _ => self.pane_store.active_pane_id.clone(),
        };

        if let Some(pane_id) = target_pane {
            if matches!(event, InputEvent::MouseDown { .. }) {
                self.pane_store.bring_to_front(&pane_id);
            }

            if let Some(pane_bounds) = self.pane_bounds.get(&pane_id).copied() {
                if let Some(frame) = self.pane_frames.get_mut(&pane_id) {
                    if matches!(
                        event,
                        InputEvent::MouseDown {
                            button: MouseButton::Left,
                            ..
                        }
                    ) && frame.title_bounds().contains(self.cursor_position)
                        && !frame.close_bounds().contains(self.cursor_position)
                    {
                        let allow_drag = true;
                        if let Some(pane) = self.pane_store.pane(&pane_id) {
                            if allow_drag {
                                self.pane_drag = Some(PaneDragState {
                                    pane_id: pane_id.clone(),
                                    origin: self.cursor_position,
                                    start_rect: pane.rect.clone(),
                                });
                                self.pane_store.bring_to_front(&pane_id);
                                return true;
                            }
                        }
                    }
                    handled |= frame
                        .event(event, pane_bounds, &mut self.event_context)
                        .is_handled();
                    if frame.take_close_clicked() {
                        self.close_pane(&pane_id);
                        return true;
                    }
                }

                if let Some(pane) = self.pane_store.pane(&pane_id) {
                    match pane.kind {
                        PaneKind::Chat => {
                            if let Some(chat) = self.chat_panes.get_mut(&pane_id) {
                                let dropdown_handled = if SHOW_MODEL_SELECTOR {
                                    let model_handled = matches!(
                                        chat.model_dropdown.event(
                                            event,
                                            chat.model_bounds,
                                            &mut self.event_context
                                        ),
                                        EventResult::Handled
                                    );
                                    let reasoning_handled = matches!(
                                        chat.reasoning_dropdown.event(
                                            event,
                                            chat.reasoning_bounds,
                                            &mut self.event_context
                                        ),
                                        EventResult::Handled
                                    );
                                    model_handled || reasoning_handled
                                } else {
                                    false
                                };

                                let full_auto_handled = matches!(
                                    chat.full_auto_button.event(
                                        event,
                                        chat.full_auto_bounds,
                                        &mut self.event_context
                                    ),
                                    EventResult::Handled
                                );

                                let queue_toggle_handled = matches!(
                                    chat.queue_toggle_button.event(
                                        event,
                                        chat.queue_toggle_bounds,
                                        &mut self.event_context
                                    ),
                                    EventResult::Handled
                                );

                                let formatted_handled = if chat
                                    .formatted_thread_bounds
                                    .contains(self.cursor_position)
                                {
                                    matches!(
                                        chat.formatted_thread.event(
                                            event,
                                            chat.formatted_thread_bounds,
                                            &mut self.event_context
                                        ),
                                        EventResult::Handled
                                    )
                                } else {
                                    false
                                };

                                let input_handled = matches!(
                                    chat.input.event(
                                        event,
                                        chat.input_bounds,
                                        &mut self.event_context
                                    ),
                                    EventResult::Handled
                                );

                                let submit_handled = matches!(
                                    chat.submit_button.event(
                                        event,
                                        chat.submit_bounds,
                                        &mut self.event_context
                                    ),
                                    EventResult::Handled
                                );

                                let stop_handled = matches!(
                                    chat.stop_button.event(
                                        event,
                                        chat.stop_bounds,
                                        &mut self.event_context
                                    ),
                                    EventResult::Handled
                                );

                                handled |= dropdown_handled
                                    || full_auto_handled
                                    || queue_toggle_handled
                                    || formatted_handled
                                    || input_handled
                                    || submit_handled
                                    || stop_handled;
                            }
                        }
                        PaneKind::Events => {
                            let copy_handled = matches!(
                                self.copy_button.event(
                                    event,
                                    self.copy_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            let scroll_handled =
                                self.event_scroll_bounds.contains(self.cursor_position)
                                    && matches!(
                                        self.event_scroll.event(
                                            event,
                                            self.event_scroll_bounds,
                                            &mut self.event_context
                                        ),
                                        EventResult::Handled
                                    );
                            handled |= copy_handled || scroll_handled;
                        }
                        PaneKind::Threads => {
                            let refresh_handled = matches!(
                                self.threads_refresh_button.event(
                                    event,
                                    self.threads_refresh_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            let load_more_handled = matches!(
                                self.threads_load_more_button.event(
                                    event,
                                    self.threads_load_more_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            let mut entries_handled = false;
                            for entry in &mut self.thread_entries {
                                if matches!(
                                    entry.open_button.event(
                                        event,
                                        entry.open_bounds,
                                        &mut self.event_context
                                    ),
                                    EventResult::Handled
                                ) {
                                    entries_handled = true;
                                }
                            }
                            handled |= refresh_handled || load_more_handled || entries_handled;
                        }
                        PaneKind::InboxList => {
                            let refresh_handled = matches!(
                                self.inbox_refresh_button.event(
                                    event,
                                    self.inbox_refresh_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            let open_thread_handled = matches!(
                                self.inbox_open_thread_button.event(
                                    event,
                                    self.inbox_open_thread_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            let open_approvals_handled = matches!(
                                self.inbox_open_approvals_button.event(
                                    event,
                                    self.inbox_open_approvals_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            let open_audit_handled = matches!(
                                self.inbox_open_audit_button.event(
                                    event,
                                    self.inbox_open_audit_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            let mut row_handled = false;
                            if let InputEvent::MouseDown {
                                button: MouseButton::Left,
                                ..
                            } = event
                            {
                                if let Some(thread_id) = self.inbox.row_at(self.cursor_position) {
                                    *self.pending_inbox_select_thread.borrow_mut() =
                                        Some(thread_id);
                                    row_handled = true;
                                }
                            }
                            handled |= refresh_handled
                                || open_thread_handled
                                || open_approvals_handled
                                || open_audit_handled
                                || row_handled;
                        }
                        PaneKind::InboxThread => {
                            let open_approvals_handled = matches!(
                                self.inbox_open_approvals_button.event(
                                    event,
                                    self.inbox_open_approvals_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            let open_audit_handled = matches!(
                                self.inbox_open_audit_button.event(
                                    event,
                                    self.inbox_open_audit_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            handled |= open_approvals_handled || open_audit_handled;
                        }
                        PaneKind::InboxApprovals => {
                            let approve_handled = matches!(
                                self.inbox_approve_button.event(
                                    event,
                                    self.inbox_approve_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            let reject_handled = matches!(
                                self.inbox_reject_button.event(
                                    event,
                                    self.inbox_reject_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            handled |= approve_handled || reject_handled;
                        }
                        PaneKind::InboxAudit => {}
                        PaneKind::FileEditor => {
                            let path_handled = matches!(
                                self.file_editor.path_input.event(
                                    event,
                                    self.file_editor.path_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            if self.file_editor.path_input.is_focused() {
                                if let Some(tab) = self.file_editor.active_tab_mut() {
                                    if tab.editor.is_focused() {
                                        tab.editor.blur();
                                    }
                                }
                            }
                            let open_handled = matches!(
                                self.file_editor.open_button.event(
                                    event,
                                    self.file_editor.open_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            let reload_handled = matches!(
                                self.file_editor.reload_button.event(
                                    event,
                                    self.file_editor.reload_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            let save_handled = matches!(
                                self.file_editor.save_button.event(
                                    event,
                                    self.file_editor.save_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            let split_h_handled = matches!(
                                self.file_editor.split_horizontal_button.event(
                                    event,
                                    self.file_editor.split_h_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            let split_v_handled = matches!(
                                self.file_editor.split_vertical_button.event(
                                    event,
                                    self.file_editor.split_v_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            let tree_refresh_handled = matches!(
                                self.file_editor.tree_refresh_button.event(
                                    event,
                                    self.file_editor.tree_refresh_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );

                            let mut tree_scroll_handled = false;
                            if let InputEvent::Scroll { dx, dy } = event {
                                if self.file_editor.tree_bounds.contains(self.cursor_position) {
                                    self.file_editor.tree_scroll.scroll_by(Point::new(*dx, *dy));
                                    tree_scroll_handled = true;
                                }
                            }

                            let mut click_handled = false;
                            if let InputEvent::MouseDown {
                                button: MouseButton::Left,
                                ..
                            } = event
                            {
                                if let Some((row_path, is_dir)) = self
                                    .file_editor
                                    .tree_row_at(self.cursor_position)
                                    .map(|row| (row.path.clone(), row.is_dir))
                                {
                                    self.file_editor.selected_tree_path = Some(row_path.clone());
                                    if is_dir {
                                        self.file_editor.toggle_tree_node(&row_path);
                                    } else {
                                        self.file_editor.queue_open_path(row_path, false);
                                    }
                                    click_handled = true;
                                }

                                if let Some((group_index, tab_id)) =
                                    self.file_editor.tab_hit_at(self.cursor_position)
                                {
                                    self.file_editor.activate_tab(tab_id, group_index);
                                    click_handled = true;
                                }

                                if let Some(group_index) =
                                    self.file_editor.group_at_point(self.cursor_position)
                                {
                                    self.file_editor.active_group = group_index;
                                }
                            }

                            let mut save_shortcut_handled = false;
                            if let InputEvent::KeyDown { key, modifiers } = event {
                                if is_save_chord(modifiers, key)
                                    && !self.file_editor.path_input.is_focused()
                                {
                                    self.file_editor.request_save();
                                    save_shortcut_handled = true;
                                }
                            }

                            let key_event = matches!(
                                event,
                                InputEvent::KeyDown { .. } | InputEvent::KeyUp { .. }
                            );
                            let mut editor_handled = false;
                            if !save_shortcut_handled && !self.file_editor.path_input.is_focused() {
                                let mut target_group = None;
                                if key_event {
                                    if let Some(tab) = self.file_editor.active_tab() {
                                        if tab.editor.is_focused() {
                                            target_group = Some(self.file_editor.active_group);
                                        }
                                    }
                                } else if let Some(group_index) =
                                    self.file_editor.group_at_point(self.cursor_position)
                                {
                                    if let Some(bounds) =
                                        self.file_editor.editor_bounds_for_group(group_index)
                                    {
                                        if bounds.contains(self.cursor_position) {
                                            target_group = Some(group_index);
                                        }
                                    }
                                }

                                if let Some(group_index) = target_group {
                                    let bounds =
                                        self.file_editor.editor_bounds_for_group(group_index);
                                    let tab_id = self
                                        .file_editor
                                        .groups
                                        .get(group_index)
                                        .and_then(|group| group.active_tab);
                                    if let (Some(bounds), Some(tab_id)) = (bounds, tab_id) {
                                        if let Some(tab) = self.file_editor.tabs.get_mut(&tab_id) {
                                            editor_handled = matches!(
                                                tab.editor.event(
                                                    event,
                                                    bounds,
                                                    &mut self.event_context
                                                ),
                                                EventResult::Handled
                                            );
                                        }
                                    }
                                }
                            }

                            handled |= path_handled
                                || open_handled
                                || reload_handled
                                || save_handled
                                || split_h_handled
                                || split_v_handled
                                || tree_refresh_handled
                                || tree_scroll_handled
                                || click_handled
                                || save_shortcut_handled
                                || editor_handled;
                        }
                        PaneKind::Auth => {
                            let email_handled = matches!(
                                self.runtime_auth_email_input.event(
                                    event,
                                    self.runtime_auth_email_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            let code_handled = matches!(
                                self.runtime_auth_code_input.event(
                                    event,
                                    self.runtime_auth_code_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            let send_handled = matches!(
                                self.runtime_auth_send_button.event(
                                    event,
                                    self.runtime_auth_send_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            let verify_handled = matches!(
                                self.runtime_auth_verify_button.event(
                                    event,
                                    self.runtime_auth_verify_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            let status_handled = matches!(
                                self.runtime_auth_status_button.event(
                                    event,
                                    self.runtime_auth_status_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            let logout_handled = matches!(
                                self.runtime_auth_logout_button.event(
                                    event,
                                    self.runtime_auth_logout_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            handled |= email_handled
                                || code_handled
                                || send_handled
                                || verify_handled
                                || status_handled
                                || logout_handled;
                        }
                        PaneKind::Identity => {
                            let key_input_handled = matches!(
                                self.identity_private_key_input.event(
                                    event,
                                    self.identity_private_key_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            let load_handled = matches!(
                                self.identity_load_button.event(
                                    event,
                                    self.identity_load_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            let keygen_handled = matches!(
                                self.keygen_button.event(
                                    event,
                                    self.keygen_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            let copy_npub_handled = matches!(
                                self.identity_copy_npub_button.event(
                                    event,
                                    self.identity_copy_npub_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            let copy_nsec_handled = matches!(
                                self.identity_copy_nsec_button.event(
                                    event,
                                    self.identity_copy_nsec_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            handled |= key_input_handled
                                || load_handled
                                || keygen_handled
                                || copy_npub_handled
                                || copy_nsec_handled;
                        }
                        PaneKind::Pylon => {
                            let toggle_handled = matches!(
                                self.pylon_toggle_button.event(
                                    event,
                                    self.pylon_toggle_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            handled |= toggle_handled;
                        }
                        PaneKind::Wallet => {
                            let refresh_handled = matches!(
                                self.wallet_refresh_button.event(
                                    event,
                                    self.wallet_refresh_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            let liquidity_handled = matches!(
                                self.wallet_liquidity_button.event(
                                    event,
                                    self.wallet_liquidity_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            handled |= refresh_handled || liquidity_handled;
                        }
                        PaneKind::Liquidity => {
                            let online_handled = matches!(
                                self.liquidity_online_button.event(
                                    event,
                                    self.liquidity_online_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            let offline_handled = matches!(
                                self.liquidity_offline_button.event(
                                    event,
                                    self.liquidity_offline_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            let refresh_handled = matches!(
                                self.liquidity_refresh_button.event(
                                    event,
                                    self.liquidity_refresh_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            let amount_handled = matches!(
                                self.liquidity_invoice_amount_input.event(
                                    event,
                                    self.liquidity_invoice_amount_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            let create_handled = matches!(
                                self.liquidity_invoice_create_button.event(
                                    event,
                                    self.liquidity_invoice_create_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            let copy_handled = matches!(
                                self.liquidity_invoice_copy_button.event(
                                    event,
                                    self.liquidity_invoice_copy_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            handled |= online_handled
                                || offline_handled
                                || refresh_handled
                                || amount_handled
                                || create_handled
                                || copy_handled;
                        }
                        PaneKind::SellCompute => {
                            let online_handled = matches!(
                                self.sell_compute_online_button.event(
                                    event,
                                    self.sell_compute_online_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            let offline_handled = matches!(
                                self.sell_compute_offline_button.event(
                                    event,
                                    self.sell_compute_offline_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            let refresh_handled = matches!(
                                self.sell_compute_refresh_button.event(
                                    event,
                                    self.sell_compute_refresh_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            handled |= online_handled || offline_handled || refresh_handled;
                        }
                        PaneKind::DvmHistory => {
                            let refresh_handled = matches!(
                                self.dvm_history_refresh_button.event(
                                    event,
                                    self.dvm_history_refresh_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            handled |= refresh_handled;
                        }
                        PaneKind::Nip90 => {
                            let kind_handled = matches!(
                                self.nip90_kind_input.event(
                                    event,
                                    self.nip90_kind_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            let relay_handled = matches!(
                                self.nip90_relay_input.event(
                                    event,
                                    self.nip90_relay_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            let provider_handled = matches!(
                                self.nip90_provider_input.event(
                                    event,
                                    self.nip90_provider_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            let prompt_handled = matches!(
                                self.nip90_prompt_input.event(
                                    event,
                                    self.nip90_prompt_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            let submit_handled = matches!(
                                self.nip90_submit_button.event(
                                    event,
                                    self.nip90_submit_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            handled |= kind_handled
                                || relay_handled
                                || provider_handled
                                || prompt_handled
                                || submit_handled;
                        }
                    }
                }
            }
        }

        let should_runtime_auth_send = {
            let mut pending = self.pending_runtime_auth_send.borrow_mut();
            let value = *pending;
            *pending = false;
            value
        };
        if should_runtime_auth_send {
            let email = self.runtime_auth_email_input.get_value().trim().to_string();
            if email.is_empty() {
                self.runtime_auth.last_error = Some("Enter an email address first.".to_string());
                self.runtime_auth.last_message = None;
            } else if let Some(handler) = self.send_handler.as_mut() {
                handler(UserAction::RuntimeAuthSendCode { email });
            }
        }

        let should_runtime_auth_verify = {
            let mut pending = self.pending_runtime_auth_verify.borrow_mut();
            let value = *pending;
            *pending = false;
            value
        };
        if should_runtime_auth_verify {
            let code = self.runtime_auth_code_input.get_value().trim().to_string();
            if code.is_empty() {
                self.runtime_auth.last_error =
                    Some("Enter the verification code first.".to_string());
                self.runtime_auth.last_message = None;
            } else if let Some(handler) = self.send_handler.as_mut() {
                handler(UserAction::RuntimeAuthVerifyCode { code });
            }
        }

        let should_runtime_auth_status = {
            let mut pending = self.pending_runtime_auth_status.borrow_mut();
            let value = *pending;
            *pending = false;
            value
        };
        if should_runtime_auth_status {
            if let Some(handler) = self.send_handler.as_mut() {
                handler(UserAction::RuntimeAuthStatus);
            }
        }

        let should_runtime_auth_logout = {
            let mut pending = self.pending_runtime_auth_logout.borrow_mut();
            let value = *pending;
            *pending = false;
            value
        };
        if should_runtime_auth_logout {
            if let Some(handler) = self.send_handler.as_mut() {
                handler(UserAction::RuntimeAuthLogout);
            }
        }

        let should_identity_load = {
            let mut pending = self.pending_identity_load.borrow_mut();
            let value = *pending;
            *pending = false;
            value
        };
        if should_identity_load {
            let input_value = self
                .identity_private_key_input
                .get_value()
                .trim()
                .to_string();
            match import_nostr_private_key(&input_value) {
                Ok((npub, nsec)) => {
                    self.nostr_npub = Some(npub);
                    self.nostr_nsec = Some(nsec);
                    self.spark_pubkey_hex = None;
                    self.seed_phrase = None;
                    self.nostr_error = None;
                }
                Err(err) => {
                    self.nostr_error = Some(err);
                }
            }
        }

        let should_generate = {
            let mut pending = self.pending_keygen.borrow_mut();
            let value = *pending;
            *pending = false;
            value
        };

        if should_generate {
            match generate_nip06_keypair() {
                Ok((npub, nsec, spark_pubkey, seed_phrase)) => {
                    self.nostr_npub = Some(npub);
                    self.nostr_nsec = Some(nsec);
                    self.spark_pubkey_hex = Some(spark_pubkey);
                    self.seed_phrase = Some(seed_phrase);
                    self.nostr_error = None;
                }
                Err(err) => {
                    self.nostr_error = Some(err);
                    self.nostr_npub = None;
                    self.nostr_nsec = None;
                    self.spark_pubkey_hex = None;
                    self.seed_phrase = None;
                }
            }
        }

        let should_copy_npub = {
            let mut pending = self.pending_identity_copy_npub.borrow_mut();
            let value = *pending;
            *pending = false;
            value
        };
        if should_copy_npub {
            if let Some(npub) = self.nostr_npub.as_deref() {
                if copy_to_clipboard(npub).is_err() {
                    self.nostr_error = Some("failed to copy npub to clipboard".to_string());
                }
            }
        }

        let should_copy_nsec = {
            let mut pending = self.pending_identity_copy_nsec.borrow_mut();
            let value = *pending;
            *pending = false;
            value
        };
        if should_copy_nsec {
            if let Some(nsec) = self.nostr_nsec.as_deref() {
                if copy_to_clipboard(nsec).is_err() {
                    self.nostr_error = Some("failed to copy nsec to clipboard".to_string());
                }
            }
        }

        let should_toggle_pylon = {
            let mut pending = self.pending_pylon_toggle.borrow_mut();
            let value = *pending;
            *pending = false;
            value
        };
        if should_toggle_pylon {
            if let Some(handler) = self.send_handler.as_mut() {
                if self.pylon_status.running {
                    handler(UserAction::PylonStop);
                } else {
                    handler(UserAction::PylonStart);
                }
            }
        }

        let should_init = {
            let mut pending = self.pending_pylon_init.borrow_mut();
            let value = *pending;
            *pending = false;
            value
        };
        if should_init {
            if let Some(handler) = self.send_handler.as_mut() {
                handler(UserAction::PylonInit);
            }
        }

        let should_start = {
            let mut pending = self.pending_pylon_start.borrow_mut();
            let value = *pending;
            *pending = false;
            value
        };
        if should_start {
            if let Some(handler) = self.send_handler.as_mut() {
                handler(UserAction::PylonStart);
            }
        }

        let should_stop = {
            let mut pending = self.pending_pylon_stop.borrow_mut();
            let value = *pending;
            *pending = false;
            value
        };
        if should_stop {
            if let Some(handler) = self.send_handler.as_mut() {
                handler(UserAction::PylonStop);
            }
        }

        let should_refresh = {
            let mut pending = self.pending_pylon_refresh.borrow_mut();
            let value = *pending;
            *pending = false;
            value
        };
        if should_refresh {
            if let Some(handler) = self.send_handler.as_mut() {
                handler(UserAction::PylonRefresh);
            }
        }

        let should_wallet_refresh = {
            let mut pending = self.pending_wallet_refresh.borrow_mut();
            let value = *pending;
            *pending = false;
            value
        };
        if should_wallet_refresh {
            if let Some(handler) = self.send_handler.as_mut() {
                handler(UserAction::WalletRefresh);
            }
        }

        let should_wallet_liquidity = {
            let mut pending = self.pending_wallet_liquidity.borrow_mut();
            let value = *pending;
            *pending = false;
            value
        };
        if should_wallet_liquidity {
            self.toggle_liquidity_pane(self.screen_size());
        }

        let should_liquidity_online = {
            let mut pending = self.pending_liquidity_online.borrow_mut();
            let value = *pending;
            *pending = false;
            value
        };
        if should_liquidity_online {
            if let Some(handler) = self.send_handler.as_mut() {
                handler(UserAction::LiquidityProviderOnline);
            }
        }

        let should_liquidity_offline = {
            let mut pending = self.pending_liquidity_offline.borrow_mut();
            let value = *pending;
            *pending = false;
            value
        };
        if should_liquidity_offline {
            if let Some(handler) = self.send_handler.as_mut() {
                handler(UserAction::LiquidityProviderOffline);
            }
        }

        let should_liquidity_refresh = {
            let mut pending = self.pending_liquidity_refresh.borrow_mut();
            let value = *pending;
            *pending = false;
            value
        };
        if should_liquidity_refresh {
            if let Some(handler) = self.send_handler.as_mut() {
                handler(UserAction::LiquidityProviderRefresh);
            }
        }

        let should_liquidity_create_invoice = {
            let mut pending = self.pending_liquidity_invoice_create.borrow_mut();
            let value = *pending;
            *pending = false;
            value
        };
        if should_liquidity_create_invoice {
            let amount = self
                .liquidity_invoice_amount_input
                .get_value()
                .trim()
                .parse::<u64>()
                .unwrap_or(0);
            if amount > 0 {
                if let Some(handler) = self.send_handler.as_mut() {
                    handler(UserAction::LiquidityProviderCreateInvoice {
                        amount_sats: amount,
                    });
                }
            }
        }

        let should_liquidity_copy_invoice = {
            let mut pending = self.pending_liquidity_invoice_copy.borrow_mut();
            let value = *pending;
            *pending = false;
            value
        };
        if should_liquidity_copy_invoice {
            if let Some(invoice) = self.liquidity_status.last_invoice.as_deref() {
                let _ = copy_to_clipboard(invoice);
            }
        }

        let should_sell_compute_online = {
            let mut pending = self.pending_sell_compute_online.borrow_mut();
            let value = *pending;
            *pending = false;
            value
        };
        if should_sell_compute_online {
            if let Some(handler) = self.send_handler.as_mut() {
                handler(UserAction::DvmProviderStart);
            }
        }

        let should_sell_compute_offline = {
            let mut pending = self.pending_sell_compute_offline.borrow_mut();
            let value = *pending;
            *pending = false;
            value
        };
        if should_sell_compute_offline {
            if let Some(handler) = self.send_handler.as_mut() {
                handler(UserAction::DvmProviderStop);
            }
        }

        let should_sell_compute_refresh = {
            let mut pending = self.pending_sell_compute_refresh.borrow_mut();
            let value = *pending;
            *pending = false;
            value
        };
        if should_sell_compute_refresh {
            if let Some(handler) = self.send_handler.as_mut() {
                handler(UserAction::DvmProviderRefresh);
            }
        }

        let should_history_refresh = {
            let mut pending = self.pending_dvm_history_refresh.borrow_mut();
            let value = *pending;
            *pending = false;
            value
        };
        if should_history_refresh {
            if let Some(handler) = self.send_handler.as_mut() {
                handler(UserAction::DvmHistoryRefresh);
            }
        }

        let should_nip90_submit = {
            let mut pending = self.pending_nip90_submit.borrow_mut();
            let value = *pending;
            *pending = false;
            value
        };
        if should_nip90_submit {
            let prompt = self.nip90_prompt_input.get_value().trim().to_string();
            if !prompt.is_empty() {
                let kind_value = self.nip90_kind_input.get_value();
                let kind = kind_value.trim().parse::<u16>().unwrap_or(5050);
                let relays_raw = self.nip90_relay_input.get_value();
                let relays = relays_raw
                    .split(|c: char| c == ',' || c.is_whitespace())
                    .filter(|part| !part.trim().is_empty())
                    .map(|part| part.trim().to_string())
                    .collect::<Vec<_>>();
                let provider = self.nip90_provider_input.get_value().trim().to_string();
                let provider = if provider.is_empty() {
                    None
                } else {
                    Some(provider)
                };
                if let Some(handler) = self.send_handler.as_mut() {
                    handler(UserAction::Nip90Submit {
                        kind,
                        prompt,
                        relays,
                        provider,
                    });
                }
                self.nip90_prompt_input.set_value("");
            }
        }

        let should_copy = {
            let mut pending = self.pending_copy.borrow_mut();
            let value = *pending;
            *pending = false;
            value
        };

        if should_copy && !self.event_log.is_empty() {
            let mut block = String::new();
            for line in &self.event_log {
                block.push_str(line);
                block.push('\n');
            }
            if copy_to_clipboard(&block).is_ok() {
                self.copy_feedback_until = Some(Instant::now() + self.copy_feedback_duration);
            }
        }

        let should_threads_refresh = {
            let mut pending = self.pending_threads_refresh.borrow_mut();
            let value = *pending;
            *pending = false;
            value
        };
        if should_threads_refresh {
            self.threads_next_cursor = None;
            if let Some(handler) = self.send_handler.as_mut() {
                handler(UserAction::ThreadsRefresh);
            }
        }

        let should_threads_load_more = {
            let mut pending = self.pending_threads_load_more.borrow_mut();
            let value = *pending;
            *pending = false;
            value
        };
        if should_threads_load_more {
            if let Some(handler) = self.send_handler.as_mut() {
                handler(UserAction::ThreadsLoadMore {
                    cursor: self.threads_next_cursor.clone(),
                });
            }
        }

        let thread_open = {
            let mut pending = self.pending_thread_open.borrow_mut();
            pending.take()
        };
        if let Some(thread_id) = thread_open {
            self.open_thread_from_list(thread_id);
        }

        let should_inbox_refresh = {
            let mut pending = self.pending_inbox_refresh.borrow_mut();
            let value = *pending;
            *pending = false;
            value
        };
        if should_inbox_refresh {
            if let Some(handler) = self.send_handler.as_mut() {
                handler(UserAction::InboxRefresh);
            }
        }

        let selected_thread = {
            let mut pending = self.pending_inbox_select_thread.borrow_mut();
            pending.take()
        };
        if let Some(thread_id) = selected_thread {
            self.inbox.selected_thread_id = Some(thread_id.clone());
            if let Some(handler) = self.send_handler.as_mut() {
                handler(UserAction::InboxSelectThread { thread_id });
            }
        }

        let should_open_thread_pane = {
            let mut pending = self.pending_inbox_open_thread_pane.borrow_mut();
            let value = *pending;
            *pending = false;
            value
        };
        if should_open_thread_pane {
            self.toggle_inbox_thread_pane(self.screen_size());
        }

        let should_open_approvals_pane = {
            let mut pending = self.pending_inbox_open_approvals_pane.borrow_mut();
            let value = *pending;
            *pending = false;
            value
        };
        if should_open_approvals_pane {
            self.toggle_inbox_approvals_pane(self.screen_size());
        }

        let should_open_audit_pane = {
            let mut pending = self.pending_inbox_open_audit_pane.borrow_mut();
            let value = *pending;
            *pending = false;
            value
        };
        if should_open_audit_pane {
            self.toggle_inbox_audit_pane(self.screen_size());
            if let (Some(thread_id), Some(handler)) = (
                self.inbox.selected_thread_id.clone(),
                self.send_handler.as_mut(),
            ) {
                handler(UserAction::InboxLoadAudit { thread_id });
            }
        }

        let should_inbox_approve = {
            let mut pending = self.pending_inbox_approve.borrow_mut();
            let value = *pending;
            *pending = false;
            value
        };
        if should_inbox_approve {
            if let (Some(thread_id), Some(handler)) = (
                self.inbox.selected_thread_id.clone(),
                self.send_handler.as_mut(),
            ) {
                handler(UserAction::InboxApproveDraft { thread_id });
            }
        }

        let should_inbox_reject = {
            let mut pending = self.pending_inbox_reject.borrow_mut();
            let value = *pending;
            *pending = false;
            value
        };
        if should_inbox_reject {
            if let (Some(thread_id), Some(handler)) = (
                self.inbox.selected_thread_id.clone(),
                self.send_handler.as_mut(),
            ) {
                handler(UserAction::InboxRejectDraft { thread_id });
            }
        }

        let should_tree_refresh = self.file_editor.take_pending_tree_refresh();
        if should_tree_refresh {
            if self.file_editor.workspace_root.is_some() {
                self.file_editor.refresh_tree();
            } else {
                self.file_editor.status = Some("No workspace open.".to_string());
                self.file_editor.status_is_error = true;
            }
        }

        if let Some(split) = self.file_editor.take_pending_split() {
            self.file_editor.set_split_direction(split);
        }

        let should_file_open = self.file_editor.take_pending_open();
        if should_file_open {
            let raw = self.file_editor.path_input.get_value();
            if let Some(path) = self.file_editor.resolve_path(&raw) {
                self.file_editor.queue_open_path(path, false);
            } else {
                self.file_editor.status = Some("Enter a file path to open.".to_string());
                self.file_editor.status_is_error = true;
            }
        }

        let should_file_reload = self.file_editor.take_pending_reload();
        if should_file_reload {
            let target = self.file_editor.active_tab_path().or_else(|| {
                let raw = self.file_editor.path_input.get_value();
                self.file_editor.resolve_path(&raw)
            });
            if let Some(path) = target {
                self.file_editor.queue_open_path(path, true);
            } else {
                self.file_editor.status = Some("No file to reload.".to_string());
                self.file_editor.status_is_error = true;
            }
        }

        let should_file_save = self.file_editor.take_pending_save();
        if should_file_save {
            if let Some((path, contents)) = self.file_editor.save_active_tab() {
                if let Some(handler) = self.send_handler.as_mut() {
                    handler(UserAction::SaveFile {
                        path: path.to_string_lossy().to_string(),
                        contents,
                    });
                }
            } else {
                self.file_editor.status = Some("No file to save.".to_string());
                self.file_editor.status_is_error = true;
            }
        }

        if !self.file_editor.pending_open_dispatches.is_empty() {
            if let Some(handler) = self.send_handler.as_mut() {
                for path in self.file_editor.pending_open_dispatches.drain(..) {
                    handler(UserAction::OpenFile {
                        path: path.to_string_lossy().to_string(),
                    });
                }
            } else {
                self.file_editor.pending_open_dispatches.clear();
            }
        }

        let chat_ids: Vec<String> = self.chat_panes.keys().cloned().collect();
        for chat_id in chat_ids {
            if let Some(chat) = self.chat_panes.get_mut(&chat_id) {
                let pending_models = chat.take_pending_models();
                if let Some(model) = pending_models.last() {
                    chat.update_thread_model(model);
                }

                let pending_reasoning = chat.take_pending_reasoning();
                if let Some(effort) = pending_reasoning.last() {
                    chat.update_reasoning(effort);
                }

                if chat.take_pending_queue_toggle() {
                    chat.queue_mode = !chat.queue_mode;
                }

                if chat.take_pending_stop() {
                    if let (Some(session_id), Some(handler)) =
                        (chat.session_id, self.send_handler.as_mut())
                    {
                        handler(UserAction::Interrupt {
                            session_id,
                            thread_id: chat.thread_id.clone(),
                            turn_id: chat.active_turn_id.clone(),
                        });
                    }
                }

                if chat.take_pending_full_auto() {
                    let enabled = !chat.full_auto_enabled;
                    chat.full_auto_enabled = enabled;
                    if let (Some(session_id), Some(handler)) =
                        (chat.session_id, self.send_handler.as_mut())
                    {
                        handler(UserAction::FullAutoToggle {
                            session_id,
                            enabled,
                            thread_id: chat.thread_id.clone(),
                            continue_prompt: None,
                        });
                    }
                }

                let pending_messages = chat.take_pending_sends();
                if !pending_messages.is_empty() {
                    let mut messages = Vec::new();
                    for message in pending_messages {
                        if message.trim().is_empty() {
                            let value = chat.input.get_value().trim().to_string();
                            if !value.is_empty() {
                                messages.push(value);
                            }
                        } else {
                            messages.push(message);
                        }
                    }

                    if !messages.is_empty() {
                        for message in messages {
                            chat.dispatch_or_queue_message(message, &mut self.send_handler);
                        }
                        chat.input.set_value("");
                        chat.input.focus();
                        chat.input_needs_focus = false;
                        chat.submit_button
                            .set_disabled(chat.input.get_value().trim().is_empty());
                    }
                }

                chat.submit_button
                    .set_disabled(chat.input.get_value().trim().is_empty());
            }
        }

        self.nip90_submit_button
            .set_disabled(self.nip90_prompt_input.get_value().trim().is_empty());

        self.sync_route_state();
        handled
    }

    pub fn needs_redraw(&mut self) -> bool {
        self.hotbar.is_flashing()
    }

    pub fn cursor(&self) -> Cursor {
        if let Some(resize) = &self.pane_resize {
            cursor_for_resize(resize.edge).unwrap_or(Cursor::Default)
        } else if let Some((_pane_id, edge)) = self.pane_resize_target(self.cursor_position) {
            cursor_for_resize(edge).unwrap_or(Cursor::Default)
        } else if self.canvas_pan.is_some() {
            Cursor::Grabbing
        } else if self.pane_drag.is_some() {
            Cursor::Grabbing
        } else if self.pane_store.panes().iter().rev().any(|pane| {
            if let Some(bounds) = self.pane_bounds.get(&pane.id) {
                if bounds.contains(self.cursor_position) {
                    if let Some(frame) = self.pane_frames.get(&pane.id) {
                        let title_bounds = frame.title_bounds();
                        let close_bounds = frame.close_bounds();
                        return title_bounds.contains(self.cursor_position)
                            && !close_bounds.contains(self.cursor_position);
                    }
                }
            }
            false
        }) {
            Cursor::Grab
        } else if self.hotbar.is_hovered() {
            Cursor::Pointer
        } else if self
            .pane_frames
            .values()
            .any(|frame| frame.is_close_hovered())
        {
            Cursor::Pointer
        } else if self
            .chat_panes
            .values()
            .any(|chat| chat.submit_button.is_hovered() && !chat.submit_button.is_disabled())
        {
            Cursor::Pointer
        } else if self
            .chat_panes
            .values()
            .any(|chat| chat.formatted_thread.is_action_hovered())
        {
            Cursor::Pointer
        } else if self
            .chat_panes
            .values()
            .any(|chat| chat.stop_button.is_hovered() && !chat.stop_button.is_disabled())
        {
            Cursor::Pointer
        } else if self.keygen_button.is_hovered()
            || self.identity_load_button.is_hovered()
            || self.identity_copy_npub_button.is_hovered()
            || self.identity_copy_nsec_button.is_hovered()
        {
            Cursor::Pointer
        } else if self.pylon_toggle_button.is_hovered() {
            Cursor::Pointer
        } else if self.dvm_history_refresh_button.is_hovered() {
            Cursor::Pointer
        } else if self.nip90_submit_button.is_hovered() {
            Cursor::Pointer
        } else if self
            .chat_panes
            .values()
            .any(|chat| chat.full_auto_button.is_hovered() && !chat.full_auto_button.is_disabled())
        {
            Cursor::Pointer
        } else if self.copy_button.is_hovered() && !self.copy_button.is_disabled() {
            Cursor::Pointer
        } else if self.threads_refresh_button.is_hovered()
            || self.threads_load_more_button.is_hovered()
        {
            Cursor::Pointer
        } else if self
            .thread_entries
            .iter()
            .any(|entry| entry.open_button.is_hovered())
        {
            Cursor::Pointer
        } else if self.inbox_refresh_button.is_hovered()
            || self.inbox_open_thread_button.is_hovered()
            || self.inbox_open_approvals_button.is_hovered()
            || self.inbox_open_audit_button.is_hovered()
            || (self.inbox_approve_button.is_hovered() && !self.inbox_approve_button.is_disabled())
            || (self.inbox_reject_button.is_hovered() && !self.inbox_reject_button.is_disabled())
            || self.inbox.row_at(self.cursor_position).is_some()
        {
            Cursor::Pointer
        } else if (self.runtime_auth_send_button.is_hovered()
            && !self.runtime_auth_send_button.is_disabled())
            || (self.runtime_auth_verify_button.is_hovered()
                && !self.runtime_auth_verify_button.is_disabled())
            || self.runtime_auth_status_button.is_hovered()
            || self.runtime_auth_logout_button.is_hovered()
        {
            Cursor::Pointer
        } else if (self.file_editor.open_button.is_hovered()
            && !self.file_editor.open_button.is_disabled())
            || (self.file_editor.reload_button.is_hovered()
                && !self.file_editor.reload_button.is_disabled())
            || (self.file_editor.save_button.is_hovered()
                && !self.file_editor.save_button.is_disabled())
            || self.file_editor.split_horizontal_button.is_hovered()
            || self.file_editor.split_vertical_button.is_hovered()
            || self.file_editor.tree_refresh_button.is_hovered()
            || self.file_editor.tree_row_at(self.cursor_position).is_some()
            || self.file_editor.tab_hit_at(self.cursor_position).is_some()
        {
            Cursor::Pointer
        } else if SHOW_MODEL_SELECTOR
            && self.chat_panes.values().any(|chat| {
                chat.model_hovered
                    || chat.model_dropdown.is_open()
                    || chat.reasoning_hovered
                    || chat.reasoning_dropdown.is_open()
            })
        {
            Cursor::Pointer
        } else if self
            .chat_panes
            .values()
            .any(|chat| chat.input_hovered || chat.input.is_focused())
        {
            Cursor::Text
        } else if let Some(cursor) = self.file_editor.editor_cursor_at(self.cursor_position) {
            cursor
        } else if self.file_editor.path_bounds.contains(self.cursor_position)
            || self.file_editor.path_input.is_focused()
        {
            Cursor::Text
        } else if self
            .runtime_auth_email_bounds
            .contains(self.cursor_position)
            || self.runtime_auth_code_bounds.contains(self.cursor_position)
            || self.runtime_auth_email_input.is_focused()
            || self.runtime_auth_code_input.is_focused()
            || self
                .identity_private_key_bounds
                .contains(self.cursor_position)
            || self.identity_private_key_input.is_focused()
        {
            Cursor::Text
        } else if self.nip90_kind_bounds.contains(self.cursor_position)
            || self.nip90_relay_bounds.contains(self.cursor_position)
            || self.nip90_provider_bounds.contains(self.cursor_position)
            || self.nip90_prompt_bounds.contains(self.cursor_position)
            || self.nip90_kind_input.is_focused()
            || self.nip90_relay_input.is_focused()
            || self.nip90_provider_input.is_focused()
            || self.nip90_prompt_input.is_focused()
        {
            Cursor::Text
        } else {
            Cursor::Default
        }
    }
}

impl Default for MinimalRoot {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for MinimalRoot {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        cx.scene
            .draw_quad(Quad::new(bounds).with_background(Hsla::black()));

        let mut dots_grid = DotsGrid::new()
            .shape(DotShape::Circle)
            .color(theme::text::MUTED)
            .opacity(0.12)
            .distance(GRID_DOT_DISTANCE)
            .size(1.5);
        let grid_offset = Point::new(
            self.background_offset.x.rem_euclid(GRID_DOT_DISTANCE),
            self.background_offset.y.rem_euclid(GRID_DOT_DISTANCE),
        );
        let grid_bounds = Bounds::new(
            bounds.origin.x + grid_offset.x,
            bounds.origin.y + grid_offset.y,
            bounds.size.width,
            bounds.size.height,
        );
        dots_grid.paint(grid_bounds, cx);

        self.set_screen_size(Size::new(bounds.size.width, bounds.size.height));

        self.copy_bounds = Bounds::ZERO;
        self.threads_refresh_bounds = Bounds::ZERO;
        self.threads_load_more_bounds = Bounds::ZERO;
        self.inbox_refresh_bounds = Bounds::ZERO;
        self.inbox_open_thread_bounds = Bounds::ZERO;
        self.inbox_open_approvals_bounds = Bounds::ZERO;
        self.inbox_open_audit_bounds = Bounds::ZERO;
        self.inbox_approve_bounds = Bounds::ZERO;
        self.inbox_reject_bounds = Bounds::ZERO;
        self.inbox.list_row_bounds.clear();
        self.event_scroll_bounds = Bounds::ZERO;
        self.keygen_bounds = Bounds::ZERO;
        self.identity_private_key_bounds = Bounds::ZERO;
        self.identity_load_bounds = Bounds::ZERO;
        self.identity_copy_npub_bounds = Bounds::ZERO;
        self.identity_copy_nsec_bounds = Bounds::ZERO;
        self.file_editor.path_bounds = Bounds::ZERO;
        self.file_editor.open_bounds = Bounds::ZERO;
        self.file_editor.reload_bounds = Bounds::ZERO;
        self.file_editor.save_bounds = Bounds::ZERO;
        self.file_editor.split_h_bounds = Bounds::ZERO;
        self.file_editor.split_v_bounds = Bounds::ZERO;
        self.file_editor.tree_bounds = Bounds::ZERO;
        self.file_editor.tree_header_bounds = Bounds::ZERO;
        self.file_editor.tree_refresh_bounds = Bounds::ZERO;
        self.runtime_auth_email_bounds = Bounds::ZERO;
        self.runtime_auth_code_bounds = Bounds::ZERO;
        self.runtime_auth_send_bounds = Bounds::ZERO;
        self.runtime_auth_verify_bounds = Bounds::ZERO;
        self.runtime_auth_status_bounds = Bounds::ZERO;
        self.runtime_auth_logout_bounds = Bounds::ZERO;

        self.pane_bounds.clear();

        let panes_snapshot = self.pane_store.panes().to_vec();
        let mut panes = Vec::with_capacity(panes_snapshot.len());
        if let Some(active_id) = self.pane_store.active_pane_id.as_ref() {
            for pane in panes_snapshot.iter() {
                if pane.id != *active_id {
                    panes.push(pane.clone());
                }
            }
            if let Some(active) = panes_snapshot.iter().find(|pane| pane.id == *active_id) {
                panes.push(active.clone());
            }
        } else {
            panes = panes_snapshot;
        }

        let base_layer = cx.scene.layer();
        let pane_layer_start = base_layer + 1;
        for (index, pane) in panes.iter().enumerate() {
            cx.scene.set_layer(pane_layer_start + index as u32);
            let pane_bounds = Bounds::new(
                bounds.origin.x + pane.rect.x,
                bounds.origin.y + pane.rect.y,
                pane.rect.width,
                pane.rect.height,
            );
            self.pane_bounds.insert(pane.id.clone(), pane_bounds);

            cx.scene.push_clip(pane_bounds);
            let (_title_bounds, _close_bounds, content_bounds) = {
                let frame = self
                    .pane_frames
                    .entry(pane.id.clone())
                    .or_insert_with(PaneFrame::new);
                frame.set_title(pane.title.clone());
                frame.set_active(self.pane_store.is_active(&pane.id));
                frame.set_dismissable(pane.dismissable);
                frame.set_title_height(PANE_TITLE_HEIGHT);
                frame.paint(pane_bounds, cx);
                (
                    frame.title_bounds(),
                    frame.close_bounds(),
                    frame.content_bounds(),
                )
            };

            match pane.kind {
                PaneKind::Chat => {
                    if let Some(chat) = self.chat_panes.get_mut(&pane.id) {
                        paint_chat_pane(chat, content_bounds, cx);
                    }
                }
                PaneKind::Events => paint_events_pane(self, content_bounds, cx),
                PaneKind::Threads => paint_threads_pane(self, content_bounds, cx),
                PaneKind::InboxList => paint_inbox_list_pane(self, content_bounds, cx),
                PaneKind::InboxThread => paint_inbox_thread_pane(self, content_bounds, cx),
                PaneKind::InboxApprovals => paint_inbox_approvals_pane(self, content_bounds, cx),
                PaneKind::InboxAudit => paint_inbox_audit_pane(self, content_bounds, cx),
                PaneKind::FileEditor => paint_file_editor_pane(self, content_bounds, cx),
                PaneKind::Auth => paint_auth_pane(self, content_bounds, cx),
                PaneKind::Identity => paint_identity_pane(self, content_bounds, cx),
                PaneKind::Pylon => paint_pylon_pane(self, content_bounds, cx),
                PaneKind::Wallet => paint_wallet_pane(self, content_bounds, cx),
                PaneKind::Liquidity => paint_liquidity_pane(self, content_bounds, cx),
                PaneKind::SellCompute => paint_sell_compute_pane(self, content_bounds, cx),
                PaneKind::DvmHistory => paint_dvm_history_pane(self, content_bounds, cx),
                PaneKind::Nip90 => paint_nip90_pane(self, content_bounds, cx),
            }
            cx.scene.pop_clip();
        }

        cx.scene.set_layer(pane_layer_start + panes.len() as u32);
        let hotbar_scale = 1.0 / self.zoom_factor.max(0.1);
        let slot_count: usize = (HOTBAR_SLOT_MAX + 1) as usize;
        let item_size = HOTBAR_ITEM_SIZE * hotbar_scale;
        let padding = HOTBAR_PADDING * hotbar_scale;
        let gap = HOTBAR_ITEM_GAP * hotbar_scale;
        let bar_height = HOTBAR_HEIGHT * hotbar_scale;
        let float_gap = HOTBAR_FLOAT_GAP * hotbar_scale;
        let bar_width = padding * 2.0
            + item_size * slot_count as f32
            + gap * (slot_count.saturating_sub(1) as f32);
        let bar_x = bounds.origin.x + (bounds.size.width - bar_width) * 0.5;
        let bar_y = bounds.origin.y + bounds.size.height - float_gap - bar_height;
        let bar_bounds = Bounds::new(bar_x, bar_y, bar_width, bar_height);
        self.hotbar_bounds = bar_bounds;

        self.hotbar.set_item_size(item_size);
        self.hotbar.set_padding(padding);
        self.hotbar.set_gap(gap);
        self.hotbar.set_corner_radius(8.0 * hotbar_scale);
        self.hotbar.set_font_scale(hotbar_scale);

        let mut items = Vec::new();
        self.hotbar_bindings.clear();

        items.push(HotbarSlot::new(HOTBAR_SLOT_NEW_CHAT, "+", "New chat"));
        self.hotbar_bindings
            .insert(HOTBAR_SLOT_NEW_CHAT, HotbarAction::NewChat);

        items.push(
            HotbarSlot::new(HOTBAR_SLOT_IDENTITY, "ID", "Identity")
                .active(self.pane_store.is_active("identity")),
        );
        self.hotbar_bindings
            .insert(HOTBAR_SLOT_IDENTITY, HotbarAction::ToggleIdentity);

        items.push(
            HotbarSlot::new(HOTBAR_SLOT_WALLET, "WL", "Wallet")
                .active(self.pane_store.is_active("wallet")),
        );
        self.hotbar_bindings
            .insert(HOTBAR_SLOT_WALLET, HotbarAction::ToggleWallet);

        items.push(
            HotbarSlot::new(HOTBAR_SLOT_THREADS, "TH", "Threads")
                .active(self.pane_store.is_active("threads")),
        );
        self.hotbar_bindings
            .insert(HOTBAR_SLOT_THREADS, HotbarAction::ToggleThreads);
        items.push(
            HotbarSlot::new(HOTBAR_SLOT_AUTH, "AU", "Auth")
                .active(self.pane_store.is_active("auth")),
        );
        self.hotbar_bindings
            .insert(HOTBAR_SLOT_AUTH, HotbarAction::ToggleAuth);

        items.push(
            HotbarSlot::new(HOTBAR_SLOT_INBOX, "IN", "Inbox")
                .active(self.pane_store.is_active("inbox-list")),
        );
        self.hotbar_bindings
            .insert(HOTBAR_SLOT_INBOX, HotbarAction::ToggleInboxList);

        let mut slot_to_pane: HashMap<u8, String> = HashMap::new();
        for (pane_id, slot) in self.chat_slot_assignments.iter() {
            if self.chat_panes.contains_key(pane_id) {
                slot_to_pane.insert(*slot, pane_id.clone());
            }
        }

        for slot in HOTBAR_CHAT_SLOT_START..=HOTBAR_SLOT_MAX {
            if let Some(pane_id) = slot_to_pane.get(&slot) {
                let title = self
                    .chat_slot_labels
                    .get(pane_id)
                    .cloned()
                    .unwrap_or_else(|| "Chat".to_string());
                items.push(
                    HotbarSlot::new(slot, "CH", title).active(self.pane_store.is_active(pane_id)),
                );
                self.hotbar_bindings
                    .insert(slot, HotbarAction::FocusPane(pane_id.clone()));
            } else {
                items.push(HotbarSlot::new(slot, "", format!("Slot {}", slot)).ghost(true));
            }
        }

        // Slot 0 should be rendered last (after 1-9).
        items.push(
            HotbarSlot::new(HOTBAR_SLOT_EVENTS, "EV", "Events")
                .active(self.pane_store.is_active("events")),
        );
        self.hotbar_bindings
            .insert(HOTBAR_SLOT_EVENTS, HotbarAction::ToggleEvents);

        self.hotbar.set_items(items);
        self.hotbar.paint(bar_bounds, cx);
        cx.scene.set_layer(base_layer);
    }
}

impl DesktopRoot {
    pub fn new() -> Self {
        let pending_actions: Rc<RefCell<Vec<UiAction>>> = Rc::new(RefCell::new(Vec::new()));
        let message_editor = build_message_editor(pending_actions.clone());

        let mut root = Self {
            view_model: AppViewModel::default(),
            event_context: EventContext::new(),
            left_header: Text::new("SESSIONS")
                .font_size(theme::font_size::BASE)
                .bold()
                .color(theme::accent::PRIMARY),
            center_header: Text::new("SESSION --")
                .font_size(theme::font_size::BASE)
                .bold()
                .color(theme::accent::PRIMARY),
            right_header: Text::new("STATUS")
                .font_size(theme::font_size::BASE)
                .bold()
                .color(theme::accent::PRIMARY),
            session_rows: Vec::new(),
            session_scroll: ScrollView::new().show_scrollbar(true).scrollbar_width(6.0),
            thread_view: ThreadView::new().auto_scroll(true).item_spacing(12.0),
            message_editor,
            status_scroll: ScrollView::new().show_scrollbar(true).scrollbar_width(6.0),
            pending_actions,
            send_handler: None,
            cursor_position: Point::ZERO,
        };
        root.refresh_text();
        root
    }

    pub fn apply_event(&mut self, event: AppEvent) {
        self.view_model.apply_event(&event);
        self.update_thread_view(&event);
        self.refresh_text();
    }

    pub fn view_model(&self) -> &AppViewModel {
        &self.view_model
    }

    pub fn set_send_handler<F>(&mut self, handler: F)
    where
        F: FnMut(UserAction) + 'static,
    {
        self.send_handler = Some(Box::new(handler));
    }

    pub fn handle_input(&mut self, event: &InputEvent, bounds: Bounds) -> bool {
        if let InputEvent::MouseMove { x, y } = event {
            self.cursor_position = Point::new(*x, *y);
        }

        let layout = Layout::new(bounds);
        let mut handled = false;

        if let InputEvent::Scroll { .. } = event {
            if layout.left_list_bounds.contains(self.cursor_position) {
                handled |= matches!(
                    self.session_scroll.event(
                        event,
                        layout.left_list_bounds,
                        &mut self.event_context
                    ),
                    EventResult::Handled
                );
            }
            if layout.right_body_bounds.contains(self.cursor_position) {
                handled |= matches!(
                    self.status_scroll.event(
                        event,
                        layout.right_body_bounds,
                        &mut self.event_context
                    ),
                    EventResult::Handled
                );
            }
            if layout.thread_bounds.contains(self.cursor_position) {
                handled |= matches!(
                    self.thread_view
                        .event(event, layout.thread_bounds, &mut self.event_context),
                    EventResult::Handled
                );
            }
            if layout.editor_bounds.contains(self.cursor_position) {
                handled |= matches!(
                    self.message_editor
                        .event(event, layout.editor_bounds, &mut self.event_context),
                    EventResult::Handled
                );
            }
            self.flush_ui_actions();
            return handled;
        }

        if layout.left_list_bounds.contains(self.cursor_position) {
            handled |= matches!(
                self.session_scroll
                    .event(event, layout.left_list_bounds, &mut self.event_context),
                EventResult::Handled
            );
        }
        if layout.right_body_bounds.contains(self.cursor_position) {
            handled |= matches!(
                self.status_scroll
                    .event(event, layout.right_body_bounds, &mut self.event_context),
                EventResult::Handled
            );
        }
        handled |= matches!(
            self.thread_view
                .event(event, layout.thread_bounds, &mut self.event_context),
            EventResult::Handled
        );

        handled |= matches!(
            self.message_editor
                .event(event, layout.editor_bounds, &mut self.event_context),
            EventResult::Handled
        );

        self.flush_ui_actions();
        handled
    }

    fn refresh_text(&mut self) {
        let session_label = self
            .view_model
            .session_id
            .map(format_session_id)
            .map(|id| format!("SESSION {id}"))
            .unwrap_or_else(|| "SESSION --".to_string());
        self.center_header.set_content(session_label);

        self.session_rows = self
            .view_model
            .sessions
            .iter()
            .map(|session| SessionRow {
                id: format_session_id(session.session_id),
                detail: session
                    .label
                    .clone()
                    .unwrap_or_else(|| "Session ready".to_string()),
                active: Some(session.session_id) == self.view_model.session_id,
            })
            .collect();
    }

    fn status_sections(&self) -> Vec<StatusSectionData> {
        let workspace_path = self
            .view_model
            .workspace_path()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "--".to_string());

        let workspace_id = self
            .view_model
            .workspace_id
            .map(|id| format!("{id:?}"))
            .unwrap_or_else(|| "--".to_string());

        let session_id = self
            .view_model
            .session_id
            .map(format_session_id)
            .unwrap_or_else(|| "--".to_string());

        let last_event = self
            .view_model
            .last_event
            .clone()
            .unwrap_or_else(|| "--".to_string());

        vec![
            StatusSectionData {
                title: "SYSTEM",
                lines: vec![
                    StatusLineData {
                        label: "CLI",
                        value: "OK codex-cli".to_string(),
                        value_color: theme::status::SUCCESS,
                    },
                    StatusLineData {
                        label: "APP-SERVER",
                        value: "READY".to_string(),
                        value_color: theme::status::SUCCESS,
                    },
                ],
                actions: vec![],
            },
            StatusSectionData {
                title: "WORKSPACE",
                lines: vec![
                    StatusLineData {
                        label: "WORKING DIR",
                        value: workspace_path,
                        value_color: theme::text::PRIMARY,
                    },
                    StatusLineData {
                        label: "WORKSPACE ID",
                        value: workspace_id,
                        value_color: theme::text::MUTED,
                    },
                ],
                actions: vec![
                    StatusActionData {
                        label: "CONNECT",
                        active: true,
                    },
                    StatusActionData {
                        label: "DISCONNECT",
                        active: false,
                    },
                ],
            },
            StatusSectionData {
                title: "CONNECTION",
                lines: vec![
                    StatusLineData {
                        label: "STATUS",
                        value: "CONNECTED".to_string(),
                        value_color: theme::status::SUCCESS,
                    },
                    StatusLineData {
                        label: "LAST EVENT",
                        value: last_event,
                        value_color: theme::text::MUTED,
                    },
                    StatusLineData {
                        label: "SESSION",
                        value: session_id,
                        value_color: theme::text::SECONDARY,
                    },
                ],
                actions: vec![],
            },
            StatusSectionData {
                title: "FULL AUTO",
                lines: vec![StatusLineData {
                    label: "STATE",
                    value: "OFF".to_string(),
                    value_color: theme::text::MUTED,
                }],
                actions: vec![
                    StatusActionData {
                        label: "ENABLE",
                        active: false,
                    },
                    StatusActionData {
                        label: "DISABLE",
                        active: true,
                    },
                ],
            },
        ]
    }

    fn update_thread_view(&mut self, event: &AppEvent) {
        match event {
            AppEvent::WorkspaceOpened { path, .. } => {
                let message = format!("Workspace ready: {}", path.display());
                self.thread_view.push_entry(ThreadEntry::new(
                    ThreadEntryType::Assistant,
                    AssistantMessage::new(message),
                ));
            }
            AppEvent::SessionStarted { label, .. } => {
                let label = label
                    .clone()
                    .unwrap_or_else(|| "Session started".to_string());
                self.thread_view.push_entry(ThreadEntry::new(
                    ThreadEntryType::Assistant,
                    AssistantMessage::new(label),
                ));
                self.bootstrap_tool_calls();
            }
            AppEvent::UserActionDispatched { action, .. } => {
                if let UserAction::Message { text, .. } = action {
                    self.thread_view.push_entry(ThreadEntry::new(
                        ThreadEntryType::User,
                        UserMessage::new(text.clone()),
                    ));
                    self.thread_view.push_entry(ThreadEntry::new(
                        ThreadEntryType::Assistant,
                        AssistantMessage::new("Message sent."),
                    ));
                }

                if let UserAction::Command { name, .. } = action {
                    let note = format!("Command received: {name}");
                    self.thread_view.push_entry(ThreadEntry::new(
                        ThreadEntryType::Assistant,
                        AssistantMessage::new(note),
                    ));
                }

                if let UserAction::Interrupt { .. } = action {
                    self.thread_view.push_entry(ThreadEntry::new(
                        ThreadEntryType::Assistant,
                        AssistantMessage::new("Interrupt requested."),
                    ));
                }
                if let UserAction::NewChat { .. } = action {
                    self.thread_view.push_entry(ThreadEntry::new(
                        ThreadEntryType::Assistant,
                        AssistantMessage::new("Starting new chat."),
                    ));
                }
            }
            AppEvent::AppServerEvent { .. } => {}
            _ => {}
        }
    }

    fn bootstrap_tool_calls(&mut self) {
        let read_card = ToolCallCard::new(ToolType::Read, "read_file")
            .status(ToolStatus::Success)
            .input("README.md")
            .output("Loaded project overview.");
        self.thread_view
            .push_entry(ThreadEntry::new(ThreadEntryType::Tool, read_card));

        let search = SearchToolCall::new("ThreadView")
            .status(ToolStatus::Success)
            .matches(vec![SearchMatch {
                file: "crates/wgpui/src/components/sections/thread_view.rs".to_string(),
                line: 6,
                content: "pub struct ThreadView { ... }".to_string(),
            }]);
        self.thread_view
            .push_entry(ThreadEntry::new(ThreadEntryType::Tool, search));

        let terminal = TerminalToolCall::new("cargo build -p autopilot-desktop")
            .status(ToolStatus::Success)
            .exit_code(0)
            .output("Finished dev [unoptimized + debuginfo] target(s) in 2.8s\n");
        self.thread_view
            .push_entry(ThreadEntry::new(ThreadEntryType::Tool, terminal));

        let diff = DiffToolCall::new("crates/autopilot_ui/src/lib.rs")
            .status(ToolStatus::Success)
            .lines(vec![
                DiffLine {
                    kind: DiffLineKind::Header,
                    content: "@@ -42,6 +42,16 @@".to_string(),
                    old_line: None,
                    new_line: None,
                },
                DiffLine {
                    kind: DiffLineKind::Context,
                    content: "impl DesktopRoot {".to_string(),
                    old_line: Some(42),
                    new_line: Some(42),
                },
                DiffLine {
                    kind: DiffLineKind::Addition,
                    content: "    fn bootstrap_tool_calls(&mut self) {".to_string(),
                    old_line: None,
                    new_line: Some(48),
                },
                DiffLine {
                    kind: DiffLineKind::Addition,
                    content: "        // seed tool cards".to_string(),
                    old_line: None,
                    new_line: Some(49),
                },
                DiffLine {
                    kind: DiffLineKind::Context,
                    content: "}".to_string(),
                    old_line: Some(60),
                    new_line: Some(60),
                },
            ]);
        self.thread_view
            .push_entry(ThreadEntry::new(ThreadEntryType::Tool, diff));

        let edit_card = ToolCallCard::new(ToolType::Edit, "apply_patch")
            .status(ToolStatus::Success)
            .input("crates/autopilot_ui/src/lib.rs")
            .output("Applied UI scaffold updates.");
        self.thread_view
            .push_entry(ThreadEntry::new(ThreadEntryType::Tool, edit_card));
    }

    fn flush_ui_actions(&mut self) {
        let actions = {
            let mut pending = self.pending_actions.borrow_mut();
            pending.drain(..).collect::<Vec<_>>()
        };

        for action in actions {
            match action {
                UiAction::SendMessage(text) => {
                    let trimmed = text.trim();
                    if trimmed.is_empty() {
                        continue;
                    }

                    if let (Some(session_id), Some(handler)) =
                        (self.view_model.session_id, self.send_handler.as_mut())
                    {
                        handler(UserAction::Message {
                            session_id,
                            text: trimmed.to_string(),
                            model: None,
                            reasoning: None,
                        });
                        self.message_editor.clear();
                    } else {
                        self.thread_view.push_entry(ThreadEntry::new(
                            ThreadEntryType::System,
                            AssistantMessage::new("No active session; message not sent."),
                        ));
                    }
                }
            }
        }
    }
}

impl Default for DesktopRoot {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for DesktopRoot {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let layout = Layout::new(bounds);

        paint_background(cx, bounds);
        paint_panel(cx, layout.left_panel_bounds);
        paint_panel(cx, layout.center_panel_bounds);
        paint_panel(cx, layout.right_panel_bounds);

        self.left_header.paint(layout.left_header_bounds, cx);
        paint_session_badges(cx, layout.left_header_bounds, self.session_rows.len());
        paint_divider(cx, layout.left_header_bounds);
        let session_height = session_list_height(&self.session_rows);
        self.session_scroll
            .set_content(SessionListView::new(self.session_rows.clone()));
        self.session_scroll.set_content_size(Size::new(
            layout.left_list_bounds.size.width,
            session_height.max(layout.left_list_bounds.size.height),
        ));
        self.session_scroll.paint(layout.left_list_bounds, cx);

        self.center_header.paint(layout.center_header_bounds, cx);
        paint_divider(cx, layout.center_header_bounds);
        paint_panel_inset(cx, layout.thread_bounds);
        self.thread_view.paint(layout.thread_bounds, cx);
        paint_composer_backdrop(cx, layout.editor_bounds);
        self.message_editor.paint(layout.editor_bounds, cx);

        self.right_header.paint(layout.right_header_bounds, cx);
        paint_status_pills(cx, layout.right_header_bounds);
        paint_divider(cx, layout.right_header_bounds);
        let status_sections = self.status_sections();
        let status_height = status_sections_height(&status_sections);
        self.status_scroll
            .set_content(StatusPanelView::new(status_sections));
        self.status_scroll.set_content_size(Size::new(
            layout.right_body_bounds.size.width,
            status_height.max(layout.right_body_bounds.size.height),
        ));
        self.status_scroll.paint(layout.right_body_bounds, cx);

        paint_command_bar(cx, layout.command_bar_bounds);
    }
}

fn build_message_editor(pending: Rc<RefCell<Vec<UiAction>>>) -> MessageEditor {
    let pending_send = pending.clone();
    MessageEditor::new()
        .show_mode_badge(false)
        .show_keybinding_hint(false)
        .on_send(move |value| {
            pending_send.borrow_mut().push(UiAction::SendMessage(value));
        })
}

fn paint_background(cx: &mut PaintContext, bounds: Bounds) {
    cx.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::APP)
            .with_border(theme::border::SUBTLE, 1.0),
    );
}

fn paint_session_badges(cx: &mut PaintContext, header_bounds: Bounds, session_count: usize) {
    let badge_gap = 6.0;
    let badge_height = 16.0;
    let count_label = session_count.to_string();
    let widths = [badge_width(cx, "NEW"), badge_width(cx, &count_label)];
    let inner_bounds = Bounds::new(
        header_bounds.origin.x + 6.0,
        header_bounds.origin.y,
        (header_bounds.size.width - 12.0).max(0.0),
        header_bounds.size.height,
    );
    let badge_bounds = right_aligned_row_bounds(inner_bounds, badge_height, &widths, badge_gap);
    if let Some(bounds) = badge_bounds.get(0) {
        paint_badge(cx, "NEW", *bounds, false);
    }
    if let Some(bounds) = badge_bounds.get(1) {
        paint_badge(cx, &count_label, *bounds, true);
    }
}

fn badge_width(cx: &mut PaintContext, label: &str) -> f32 {
    let padding = 10.0;
    let text_width = cx.text.measure(label, theme::font_size::SM);
    (text_width + padding * 2.0).max(28.0)
}

fn paint_badge(cx: &mut PaintContext, label: &str, bounds: Bounds, filled: bool) {
    let border = theme::accent::PRIMARY;
    let bg = if filled {
        theme::accent::PRIMARY.with_alpha(0.2)
    } else {
        theme::bg::ELEVATED
    };
    cx.scene.draw_quad(
        Quad::new(bounds)
            .with_background(bg)
            .with_border(border, 1.0),
    );

    let text_color = theme::accent::PRIMARY;
    let mut text = Text::new(label)
        .font_size(theme::font_size::SM)
        .bold()
        .color(text_color)
        .no_wrap();
    text.paint(bounds, cx);
}

fn paint_session_list(cx: &mut PaintContext, rows: &[SessionRow], bounds: Bounds) {
    let id_column_width = session_id_column_width(cx, rows);
    let heights: Vec<f32> = rows.iter().map(|_| SESSION_ROW_HEIGHT).collect();
    let row_bounds = stack_bounds(bounds, &heights, 0.0);

    for (row, row_bounds) in rows.iter().zip(row_bounds) {
        if row_bounds.origin.y > bounds.origin.y + bounds.size.height {
            break;
        }

        if row.active {
            cx.scene.draw_quad(
                Quad::new(row_bounds)
                    .with_background(theme::bg::ELEVATED)
                    .with_border(theme::border::DEFAULT, 1.0),
            );
        }

        let row_columns = wgpui::row_bounds(
            row_bounds,
            row_bounds.size.height,
            &[
                wgpui::RowItem::fixed(ACCENT_BAR_WIDTH),
                wgpui::RowItem::flex(1.0),
            ],
            3.0,
        );
        let accent_bounds = row_columns.get(0).copied().unwrap_or(row_bounds);
        let content_bounds = row_columns.get(1).copied().unwrap_or(row_bounds);

        if row.active {
            cx.scene
                .draw_quad(Quad::new(accent_bounds).with_background(theme::accent::PRIMARY));
        }

        let content_columns = wgpui::row_bounds(
            content_bounds,
            row_bounds.size.height,
            &[
                wgpui::RowItem::fixed(id_column_width),
                wgpui::RowItem::flex(1.0),
            ],
            4.0,
        );
        let id_bounds = content_columns.get(0).copied().unwrap_or(content_bounds);
        let detail_bounds = content_columns.get(1).copied().unwrap_or(content_bounds);

        let mut id_text = Text::new(&row.id)
            .font_size(theme::font_size::BASE)
            .bold()
            .color(if row.active {
                theme::accent::PRIMARY
            } else {
                theme::text::PRIMARY
            })
            .no_wrap();
        id_text.paint(id_bounds, cx);

        let mut detail_text = Text::new(&row.detail)
            .font_size(theme::font_size::BASE)
            .color(theme::text::SECONDARY)
            .no_wrap();
        detail_text.paint(detail_bounds, cx);
    }
}

fn session_id_column_width(cx: &mut PaintContext, rows: &[SessionRow]) -> f32 {
    let mut max_width: f32 = 0.0;
    for row in rows {
        max_width = max_width.max(cx.text.measure(&row.id, theme::font_size::BASE));
    }
    (max_width + 14.0).max(70.0)
}

fn session_list_height(rows: &[SessionRow]) -> f32 {
    rows.len() as f32 * SESSION_ROW_HEIGHT
}

struct SessionListView {
    rows: Vec<SessionRow>,
}

impl SessionListView {
    fn new(rows: Vec<SessionRow>) -> Self {
        Self { rows }
    }
}

impl Component for SessionListView {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        paint_session_list(cx, &self.rows, bounds);
    }
}

fn paint_divider(cx: &mut PaintContext, bounds: Bounds) {
    let divider_bounds = divider_bounds_below(bounds, 4.0, 1.0);
    cx.scene
        .draw_quad(Quad::new(divider_bounds).with_background(theme::border::SUBTLE));
}

fn paint_panel_inset(cx: &mut PaintContext, bounds: Bounds) {
    cx.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::CODE)
            .with_border(theme::border::DEFAULT, 1.0),
    );
}

fn paint_composer_backdrop(cx: &mut PaintContext, bounds: Bounds) {
    cx.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );
}

fn paint_status_pills(cx: &mut PaintContext, header_bounds: Bounds) {
    let pill_height = 16.0;
    let gap = 6.0;
    let labels = ["SHOW CANVAS", "CONNECTED"];
    let widths = labels
        .iter()
        .map(|label| badge_width(cx, label))
        .collect::<Vec<_>>();
    let inner_bounds = Bounds::new(
        header_bounds.origin.x + 6.0,
        header_bounds.origin.y,
        (header_bounds.size.width - 12.0).max(0.0),
        header_bounds.size.height,
    );
    let pill_bounds = right_aligned_row_bounds(inner_bounds, pill_height, &widths, gap);
    for (label, bounds) in labels.iter().zip(pill_bounds) {
        paint_badge(cx, label, bounds, *label == "CONNECTED");
    }
}

#[derive(Clone, Debug)]
struct StatusLineData {
    label: &'static str,
    value: String,
    value_color: wgpui::color::Hsla,
}

#[derive(Clone, Debug)]
struct StatusActionData {
    label: &'static str,
    active: bool,
}

#[derive(Clone, Debug)]
struct StatusSectionData {
    title: &'static str,
    lines: Vec<StatusLineData>,
    actions: Vec<StatusActionData>,
}

#[derive(Clone, Debug)]
enum StatusRow {
    Header { title: &'static str },
    Line { line: StatusLineData },
    Actions { actions: Vec<StatusActionData> },
    Spacer { height: f32 },
}

fn build_status_rows(sections: Vec<StatusSectionData>) -> Vec<StatusRow> {
    let mut rows = Vec::new();
    for section in sections {
        rows.push(StatusRow::Header {
            title: section.title,
        });
        rows.push(StatusRow::Spacer { height: 2.0 });

        for line in section.lines {
            rows.push(StatusRow::Line { line });
        }

        if !section.actions.is_empty() {
            rows.push(StatusRow::Spacer { height: 4.0 });
            rows.push(StatusRow::Actions {
                actions: section.actions,
            });
            rows.push(StatusRow::Spacer { height: 2.0 });
        }

        rows.push(StatusRow::Spacer {
            height: STATUS_SECTION_GAP,
        });
    }
    rows
}

fn paint_status_sections(cx: &mut PaintContext, bounds: Bounds, sections: &[StatusSectionData]) {
    let mut label_width: f32 = 0.0;
    for section in sections {
        for line in &section.lines {
            label_width = label_width.max(cx.text.measure(line.label, theme::font_size::BASE));
        }
    }
    if label_width < 90.0 {
        label_width = 90.0;
    }
    let rows = build_status_rows(sections.to_vec());
    let heights: Vec<f32> = rows
        .iter()
        .map(|row| match row {
            StatusRow::Header { .. } => STATUS_LINE_HEIGHT,
            StatusRow::Line { .. } => STATUS_LINE_HEIGHT,
            StatusRow::Actions { .. } => 18.0,
            StatusRow::Spacer { height } => *height,
        })
        .collect();
    let row_bounds = stack_bounds(bounds, &heights, 0.0);

    for (row, row_bounds) in rows.into_iter().zip(row_bounds) {
        if row_bounds.origin.y > bounds.origin.y + bounds.size.height {
            break;
        }

        match row {
            StatusRow::Header { title } => {
                let mut header = Text::new(title)
                    .font_size(theme::font_size::BASE)
                    .bold()
                    .color(theme::accent::PRIMARY)
                    .no_wrap();
                header.paint(row_bounds, cx);
            }
            StatusRow::Line { line } => {
                let columns = wgpui::row_bounds(
                    row_bounds,
                    row_bounds.size.height,
                    &[
                        wgpui::RowItem::fixed(label_width),
                        wgpui::RowItem::flex(1.0),
                    ],
                    6.0,
                );
                let label_bounds = columns.get(0).copied().unwrap_or(row_bounds);
                let value_bounds = columns.get(1).copied().unwrap_or(row_bounds);

                let mut label_text = Text::new(line.label)
                    .font_size(theme::font_size::BASE)
                    .color(theme::text::SECONDARY)
                    .no_wrap();
                label_text.paint(label_bounds, cx);

                let mut value_text = Text::new(line.value)
                    .font_size(theme::font_size::BASE)
                    .color(line.value_color)
                    .no_wrap();
                value_text.paint(value_bounds, cx);
            }
            StatusRow::Actions { actions } => {
                let badge_height = 16.0;
                let items: Vec<wgpui::RowItem> = actions
                    .iter()
                    .map(|action| wgpui::RowItem::fixed(badge_width(cx, action.label)))
                    .collect();
                let badge_bounds = aligned_row_bounds(
                    row_bounds,
                    badge_height,
                    &items,
                    6.0,
                    JustifyContent::FlexStart,
                    AlignItems::Center,
                );
                for (action, bounds) in actions.iter().zip(badge_bounds) {
                    paint_badge(cx, action.label, bounds, action.active);
                }
            }
            StatusRow::Spacer { .. } => {}
        }
    }
}

fn status_sections_height(sections: &[StatusSectionData]) -> f32 {
    let rows = build_status_rows(sections.to_vec());
    rows.iter()
        .map(|row| match row {
            StatusRow::Header { .. } => STATUS_LINE_HEIGHT,
            StatusRow::Line { .. } => STATUS_LINE_HEIGHT,
            StatusRow::Actions { .. } => 18.0,
            StatusRow::Spacer { height } => *height,
        })
        .sum()
}

struct StatusPanelView {
    sections: Vec<StatusSectionData>,
}

impl StatusPanelView {
    fn new(sections: Vec<StatusSectionData>) -> Self {
        Self { sections }
    }
}

impl Component for StatusPanelView {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        paint_status_sections(cx, bounds, &self.sections);
    }
}

fn paint_command_bar(cx: &mut PaintContext, bounds: Bounds) {
    cx.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::SUBTLE, 1.0),
    );

    let hints = [
        ("1", "SESS", "SESSIONS", "CMD+1"),
        ("2", "NEW", "NEW", "CMD+2"),
        ("3", "GRID", "GUIDANCE", "CMD+3"),
        ("4", "SB", "STORYBOOK", "CMD+4"),
        ("8", "SET", "SETTINGS", "CMD+8"),
        ("9", "HELP", "HELP", "CMD+9"),
    ];

    let row_height = 18.0;
    let row_bounds = Bounds::new(
        bounds.origin.x + 8.0,
        bounds.origin.y,
        (bounds.size.width - 16.0).max(0.0),
        bounds.size.height,
    );
    let hint_texts: Vec<String> = hints
        .iter()
        .map(|(key, tag, label, shortcut)| format!("{key} {tag} {label} {shortcut}"))
        .collect();
    let hint_items: Vec<wgpui::RowItem> = hint_texts
        .iter()
        .map(|text| {
            let text_width = cx.text.measure(text, theme::font_size::SM);
            wgpui::RowItem::fixed(text_width + 18.0)
        })
        .collect();
    let hint_bounds = aligned_row_bounds(
        row_bounds,
        row_height,
        &hint_items,
        6.0,
        JustifyContent::FlexStart,
        AlignItems::Center,
    );
    for (text, box_bounds) in hint_texts.iter().zip(hint_bounds) {
        cx.scene.draw_quad(
            Quad::new(box_bounds)
                .with_background(theme::bg::MUTED)
                .with_border(theme::border::DEFAULT, 1.0),
        );
        let mut hint = Text::new(text)
            .font_size(theme::font_size::SM)
            .color(theme::text::SECONDARY)
            .no_wrap();
        hint.paint(box_bounds, cx);
    }
}

fn right_aligned_row_bounds(
    bounds: Bounds,
    item_height: f32,
    item_widths: &[f32],
    gap: f32,
) -> Vec<Bounds> {
    let items = item_widths
        .iter()
        .map(|width| wgpui::RowItem::fixed(*width))
        .collect::<Vec<_>>();
    aligned_row_bounds(
        bounds,
        item_height,
        &items,
        gap,
        JustifyContent::FlexEnd,
        AlignItems::Center,
    )
}

fn h_flex() -> LayoutStyle {
    LayoutStyle::new().flex_row()
}

fn v_flex() -> LayoutStyle {
    LayoutStyle::new().flex_col()
}

fn flex_1(style: LayoutStyle) -> LayoutStyle {
    style.flex_grow(1.0)
}

fn min_w(mut style: LayoutStyle, value: f32) -> LayoutStyle {
    style.min_width = px(value);
    style
}

fn gap(style: LayoutStyle, value: f32) -> LayoutStyle {
    style.gap(length(value))
}

#[derive(Clone, Copy, Debug)]
enum ColumnItem {
    Fixed(f32),
    Flex(f32),
}

fn aligned_row_bounds(
    bounds: Bounds,
    item_height: f32,
    items: &[wgpui::RowItem],
    gap: f32,
    justify_content: JustifyContent,
    align_items: AlignItems,
) -> Vec<Bounds> {
    if items.is_empty() {
        return Vec::new();
    }

    let mut engine = LayoutEngine::new();
    let gap = length(gap);
    let mut nodes = Vec::with_capacity(items.len());

    for item in items {
        let style = match item {
            wgpui::RowItem::Fixed(width) => LayoutStyle::new()
                .width(px(*width))
                .height(px(item_height))
                .flex_shrink(0.0),
            wgpui::RowItem::Flex(grow) => {
                LayoutStyle::new().height(px(item_height)).flex_grow(*grow)
            }
        };
        nodes.push(engine.request_leaf(&style));
    }

    let row_style = h_flex()
        .gap(gap)
        .justify_content(justify_content)
        .align_items(align_items)
        .width(px(bounds.size.width))
        .height(px(bounds.size.height));
    let row = engine.request_layout(&row_style, &nodes);

    engine.compute_layout(row, Size::new(bounds.size.width, bounds.size.height));

    nodes
        .into_iter()
        .map(|node| offset_bounds(engine.layout(node), bounds.origin))
        .collect()
}

fn column_bounds(bounds: Bounds, items: &[ColumnItem], gap: f32) -> Vec<Bounds> {
    if items.is_empty() {
        return Vec::new();
    }

    let mut engine = LayoutEngine::new();
    let gap = length(gap);
    let mut nodes = Vec::with_capacity(items.len());

    for item in items {
        let style = match item {
            ColumnItem::Fixed(height) => LayoutStyle::new()
                .width(px(bounds.size.width))
                .height(px(*height))
                .flex_shrink(0.0),
            ColumnItem::Flex(grow) => LayoutStyle::new()
                .width(px(bounds.size.width))
                .flex_grow(*grow),
        };
        nodes.push(engine.request_leaf(&style));
    }

    let column_style = v_flex()
        .gap(gap)
        .width(px(bounds.size.width))
        .height(px(bounds.size.height));
    let column = engine.request_layout(&column_style, &nodes);

    engine.compute_layout(column, Size::new(bounds.size.width, bounds.size.height));

    nodes
        .into_iter()
        .map(|node| offset_bounds(engine.layout(node), bounds.origin))
        .collect()
}

#[allow(dead_code)]
fn centered_bounds(
    bounds: Bounds,
    content_width: f32,
    content_height: f32,
    padding: f32,
) -> Bounds {
    let available = Bounds::new(
        bounds.origin.x + padding,
        bounds.origin.y + padding,
        (bounds.size.width - padding * 2.0).max(0.0),
        (bounds.size.height - padding * 2.0).max(0.0),
    );

    let mut engine = LayoutEngine::new();
    let content = engine.request_leaf(
        &LayoutStyle::new()
            .width(px(content_width))
            .height(px(content_height))
            .flex_shrink(0.0),
    );
    let root = engine.request_layout(
        &v_flex()
            .justify_content(JustifyContent::Center)
            .align_items(AlignItems::Center)
            .width(px(available.size.width))
            .height(px(available.size.height)),
        &[content],
    );
    engine.compute_layout(root, Size::new(available.size.width, available.size.height));

    offset_bounds(engine.layout(content), available.origin)
}

fn centered_column_bounds(bounds: Bounds, content_width: f32, padding: f32) -> Bounds {
    let available = Bounds::new(
        bounds.origin.x + padding,
        bounds.origin.y + padding,
        (bounds.size.width - padding * 2.0).max(0.0),
        (bounds.size.height - padding * 2.0).max(0.0),
    );

    let mut engine = LayoutEngine::new();
    let content = engine.request_layout(
        &v_flex()
            .width(px(content_width))
            .height(px(available.size.height)),
        &[],
    );
    let row = engine.request_layout(
        &h_flex()
            .justify_content(JustifyContent::Center)
            .align_items(AlignItems::FlexStart)
            .width(px(available.size.width))
            .height(px(available.size.height)),
        &[content],
    );
    engine.compute_layout(row, Size::new(available.size.width, available.size.height));

    offset_bounds(engine.layout(content), available.origin)
}

fn divider_bounds_below(bounds: Bounds, gap: f32, height: f32) -> Bounds {
    let mut engine = LayoutEngine::new();
    let spacer = engine.request_leaf(
        &LayoutStyle::new()
            .height(px(bounds.size.height + gap))
            .flex_shrink(0.0),
    );
    let divider = engine.request_leaf(&LayoutStyle::new().height(px(height)).flex_shrink(0.0));
    let root_style = v_flex()
        .width(px(bounds.size.width))
        .height(px(bounds.size.height + gap + height));
    let root = engine.request_layout(&root_style, &[spacer, divider]);
    engine.compute_layout(
        root,
        Size::new(bounds.size.width, bounds.size.height + gap + height),
    );
    offset_bounds(engine.layout(divider), bounds.origin)
}

#[derive(Clone, Debug)]
struct SessionSummary {
    session_id: SessionId,
    label: Option<String>,
}

#[derive(Clone, Debug)]
struct SessionRow {
    id: String,
    detail: String,
    active: bool,
}

#[derive(Clone, Debug, Default)]
struct PylonStatusView {
    running: bool,
    #[allow(dead_code)]
    pid: Option<u32>,
    uptime_secs: Option<u64>,
    provider_active: Option<bool>,
    host_active: Option<bool>,
    jobs_completed: u64,
    earnings_msats: u64,
    identity_exists: bool,
    last_error: Option<String>,
}

#[derive(Clone, Debug, Default)]
struct WalletStatusView {
    network: Option<String>,
    spark_sats: u64,
    lightning_sats: u64,
    onchain_sats: u64,
    total_sats: u64,
    spark_address: Option<String>,
    bitcoin_address: Option<String>,
    identity_exists: bool,
    last_error: Option<String>,
}

#[derive(Clone, Debug, Default)]
struct LiquidityProviderStatusView {
    running: bool,
    provider_active: Option<bool>,
    worker_id: Option<String>,
    earned_sats: u64,
    max_invoice_sats: u64,
    max_hourly_sats: u64,
    max_daily_sats: u64,
    last_invoice: Option<String>,
    last_error: Option<String>,
}

#[derive(Clone, Debug, Default)]
struct SellComputeStatusView {
    running: bool,
    provider_active: Option<bool>,
    host_active: Option<bool>,
    min_price_msats: u64,
    require_payment: bool,
    default_model: String,
    backend_preference: Vec<String>,
    agent_backends: Vec<String>,
    supported_bazaar_kinds: Vec<u16>,
    network: String,
    enable_payments: bool,
    last_error: Option<String>,
}

#[derive(Clone, Debug, Default)]
struct DvmHistoryView {
    summary_total_msats: u64,
    summary_total_sats: u64,
    summary_job_count: u64,
    summary_by_source: Vec<(String, u64)>,
    status_counts: Vec<(String, u64)>,
    jobs: Vec<DvmJobView>,
    last_error: Option<String>,
}

#[derive(Clone, Debug)]
struct DvmJobView {
    id: String,
    status: String,
    kind: u16,
    price_msats: u64,
    created_at: u64,
}

struct ThreadEntryView {
    summary: ThreadSummary,
    branch: Option<String>,
    open_button: Button,
    open_bounds: Bounds,
}

fn format_event(event: &AppEvent) -> String {
    match event {
        AppEvent::WorkspaceOpened { path, .. } => {
            format!("WorkspaceOpened ({})", path.display())
        }
        AppEvent::SessionStarted { session_id, .. } => {
            format!("SessionStarted ({:?})", session_id)
        }
        AppEvent::UserActionDispatched { action, .. } => match action {
            UserAction::Message {
                text,
                model,
                reasoning,
                ..
            } => match (model, reasoning) {
                (Some(model), Some(reasoning)) => {
                    format!("Message ({text}) [{model}, {reasoning}]")
                }
                (Some(model), None) => format!("Message ({text}) [{model}]"),
                _ => format!("Message ({text})"),
            },
            UserAction::Command { name, .. } => format!("Command ({})", name),
            UserAction::NewChat { model, .. } => {
                if let Some(model) = model {
                    format!("NewChat [{model}]")
                } else {
                    "NewChat".to_string()
                }
            }
            UserAction::RuntimeAuthSendCode { email } => {
                format!("RuntimeAuthSendCode ({email})")
            }
            UserAction::RuntimeAuthVerifyCode { .. } => "RuntimeAuthVerifyCode".to_string(),
            UserAction::RuntimeAuthStatus => "RuntimeAuthStatus".to_string(),
            UserAction::RuntimeAuthLogout => "RuntimeAuthLogout".to_string(),
            UserAction::PylonInit => "PylonInit".to_string(),
            UserAction::PylonStart => "PylonStart".to_string(),
            UserAction::PylonStop => "PylonStop".to_string(),
            UserAction::PylonRefresh => "PylonRefresh".to_string(),
            UserAction::WalletRefresh => "WalletRefresh".to_string(),
            UserAction::LiquidityProviderOnline => "LiquidityProviderOnline".to_string(),
            UserAction::LiquidityProviderOffline => "LiquidityProviderOffline".to_string(),
            UserAction::LiquidityProviderRefresh => "LiquidityProviderRefresh".to_string(),
            UserAction::LiquidityProviderCreateInvoice { amount_sats } => {
                format!("LiquidityProviderCreateInvoice ({amount_sats} sats)")
            }
            UserAction::DvmProviderStart => "DvmProviderStart".to_string(),
            UserAction::DvmProviderStop => "DvmProviderStop".to_string(),
            UserAction::DvmProviderRefresh => "DvmProviderRefresh".to_string(),
            UserAction::DvmHistoryRefresh => "DvmHistoryRefresh".to_string(),
            UserAction::Nip90Submit { kind, .. } => format!("Nip90Submit (kind {kind})"),
            UserAction::ThreadsRefresh => "ThreadsRefresh".to_string(),
            UserAction::ThreadsLoadMore { .. } => "ThreadsLoadMore".to_string(),
            UserAction::ThreadOpen { thread_id } => format!("ThreadOpen ({thread_id})"),
            UserAction::InboxRefresh => "InboxRefresh".to_string(),
            UserAction::InboxSelectThread { thread_id } => {
                format!("InboxSelectThread ({thread_id})")
            }
            UserAction::InboxApproveDraft { thread_id } => {
                format!("InboxApproveDraft ({thread_id})")
            }
            UserAction::InboxRejectDraft { thread_id } => {
                format!("InboxRejectDraft ({thread_id})")
            }
            UserAction::InboxLoadAudit { thread_id } => format!("InboxLoadAudit ({thread_id})"),
            UserAction::OpenFile { path } => format!("OpenFile ({path})"),
            UserAction::SaveFile { path, .. } => format!("SaveFile ({path})"),
            UserAction::Interrupt { .. } => "Interrupt".to_string(),
            UserAction::FullAutoToggle { enabled, .. } => {
                if *enabled {
                    "Full Auto (enabled)".to_string()
                } else {
                    "Full Auto (disabled)".to_string()
                }
            }
        },
        AppEvent::AppServerEvent { message } => format!("AppServerEvent ({message})"),
        AppEvent::PylonStatus { status } => {
            if status.running {
                "PylonStatus (running)".to_string()
            } else {
                "PylonStatus (stopped)".to_string()
            }
        }
        AppEvent::WalletStatus { status } => {
            if status.identity_exists {
                "WalletStatus (identity)".to_string()
            } else {
                "WalletStatus (missing identity)".to_string()
            }
        }
        AppEvent::LiquidityProviderStatus { status } => {
            if status.running {
                "LiquidityProviderStatus (running)".to_string()
            } else {
                "LiquidityProviderStatus (stopped)".to_string()
            }
        }
        AppEvent::DvmProviderStatus { status } => {
            if status.running {
                "DvmProviderStatus (running)".to_string()
            } else {
                "DvmProviderStatus (stopped)".to_string()
            }
        }
        AppEvent::DvmHistory { snapshot } => {
            format!("DvmHistory ({} jobs)", snapshot.summary.job_count)
        }
        AppEvent::RuntimeAuthState { state } => {
            let token = if state.token_present {
                "present"
            } else {
                "missing"
            };
            let email = state.email.as_deref().unwrap_or("<none>");
            format!("RuntimeAuthState ({token}, {email})")
        }
        AppEvent::Nip90Log { message } => format!("Nip90Log ({message})"),
        AppEvent::ThreadsUpdated {
            threads, append, ..
        } => {
            if *append {
                format!("ThreadsUpdated (+{})", threads.len())
            } else {
                format!("ThreadsUpdated ({})", threads.len())
            }
        }
        AppEvent::ThreadLoaded { thread, .. } => format!("ThreadLoaded ({})", thread.id),
        AppEvent::InboxUpdated { snapshot, source } => {
            format!(
                "InboxUpdated ({} threads, source {})",
                snapshot.threads.len(),
                source
            )
        }
        AppEvent::FileOpened { path, .. } => format!("FileOpened ({})", path.display()),
        AppEvent::FileOpenFailed { path, error } => {
            format!("FileOpenFailed ({}: {})", path.display(), error)
        }
        AppEvent::FileSaved { path } => format!("FileSaved ({})", path.display()),
        AppEvent::FileSaveFailed { path, error } => {
            format!("FileSaveFailed ({}: {})", path.display(), error)
        }
    }
}

fn format_session_id(session_id: SessionId) -> String {
    let raw = format!("{:?}", session_id);
    let trimmed = raw.trim_start_matches("SessionId(").trim_end_matches(')');
    trimmed.chars().take(6).collect()
}

fn relative_time_label(timestamp: i64) -> String {
    if timestamp <= 0 {
        return "-".to_string();
    }

    let mut seconds = timestamp;
    if seconds > 1_000_000_000_000 {
        seconds /= 1000;
    }

    let now = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs() as i64,
        Err(_) => return "-".to_string(),
    };
    let delta = (now - seconds).max(0);

    if delta < 60 {
        let n = delta.max(1);
        if n == 1 {
            format!("{n} second ago")
        } else {
            format!("{n} seconds ago")
        }
    } else if delta < 3600 {
        let n = delta / 60;
        if n == 1 {
            format!("{n} minute ago")
        } else {
            format!("{n} minutes ago")
        }
    } else if delta < 86_400 {
        let n = delta / 3600;
        if n == 1 {
            format!("{n} hour ago")
        } else {
            format!("{n} hours ago")
        }
    } else {
        let n = delta / 86_400;
        if n == 1 {
            format!("{n} day ago")
        } else {
            format!("{n} days ago")
        }
    }
}

fn truncate_line(text: &str, max_chars: usize) -> String {
    let clean = text.replace('\n', " ").trim().to_string();
    if max_chars == 0 {
        return String::new();
    }
    if clean.chars().count() <= max_chars {
        return clean;
    }
    if max_chars == 1 {
        return "…".to_string();
    }
    let mut out = clean.chars().take(max_chars - 1).collect::<String>();
    out.push('…');
    out
}

fn right_elide(text: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    if max_chars == 1 {
        return "…".to_string();
    }
    let tail_len = max_chars - 1;
    let tail: String = text
        .chars()
        .rev()
        .take(tail_len)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!("…{tail}")
}

fn git_branch_for_cwd(cwd: &Option<String>) -> Option<String> {
    let cwd = cwd.as_ref()?;
    let git_path = Path::new(cwd).join(".git");
    let head_path = if git_path.is_dir() {
        git_path.join("HEAD")
    } else if git_path.is_file() {
        let gitdir_line = std::fs::read_to_string(&git_path).ok()?;
        let gitdir = gitdir_line.trim().strip_prefix("gitdir:")?.trim();
        let gitdir_path = Path::new(gitdir);
        if gitdir_path.is_absolute() {
            gitdir_path.join("HEAD")
        } else {
            Path::new(cwd).join(gitdir).join("HEAD")
        }
    } else {
        return None;
    };

    let head = std::fs::read_to_string(head_path).ok()?;
    let head = head.trim();
    if let Some(reference) = head.strip_prefix("ref: ") {
        reference
            .rsplit('/')
            .next()
            .map(|branch| branch.to_string())
    } else if head.is_empty() {
        None
    } else {
        Some(head.chars().take(7).collect())
    }
}

fn cursor_for_resize(edge: ResizeEdge) -> Option<Cursor> {
    match edge {
        ResizeEdge::Top | ResizeEdge::Bottom => Some(Cursor::ResizeNs),
        ResizeEdge::Left | ResizeEdge::Right => Some(Cursor::ResizeEw),
        ResizeEdge::TopLeft | ResizeEdge::BottomRight => Some(Cursor::ResizeNwse),
        ResizeEdge::TopRight | ResizeEdge::BottomLeft => Some(Cursor::ResizeNesw),
        ResizeEdge::None => None,
    }
}

struct Layout {
    left_panel_bounds: Bounds,
    center_panel_bounds: Bounds,
    right_panel_bounds: Bounds,
    command_bar_bounds: Bounds,
    left_header_bounds: Bounds,
    left_list_bounds: Bounds,
    center_header_bounds: Bounds,
    thread_bounds: Bounds,
    editor_bounds: Bounds,
    right_header_bounds: Bounds,
    right_body_bounds: Bounds,
}

impl Layout {
    fn new(bounds: Bounds) -> Self {
        let mut engine = LayoutEngine::new();
        let padding = length(PANEL_PADDING);

        let left_header = engine.request_leaf(&LayoutStyle::new().height(px(PANEL_HEADER_HEIGHT)));
        let left_list = engine.request_layout(&LayoutStyle::new().flex_grow(1.0), &[]);
        let left_panel_style = gap(
            v_flex()
                .width(px(LEFT_PANEL_WIDTH))
                .flex_shrink(0.0)
                .padding(padding),
            6.0,
        );
        let left_panel = engine.request_layout(&left_panel_style, &[left_header, left_list]);

        let center_header =
            engine.request_leaf(&LayoutStyle::new().height(px(PANEL_HEADER_HEIGHT)));
        let thread_body = engine.request_layout(&LayoutStyle::new().flex_grow(1.0), &[]);
        let composer = engine.request_leaf(&LayoutStyle::new().height(px(COMPOSER_HEIGHT)));
        let center_panel_style = min_w(gap(flex_1(v_flex().padding(padding)), 6.0), 0.0);
        let center_panel =
            engine.request_layout(&center_panel_style, &[center_header, thread_body, composer]);

        let right_header = engine.request_leaf(&LayoutStyle::new().height(px(PANEL_HEADER_HEIGHT)));
        let right_body = engine.request_layout(&LayoutStyle::new().flex_grow(1.0), &[]);
        let right_panel_style = gap(
            v_flex()
                .width(px(RIGHT_PANEL_WIDTH))
                .flex_shrink(0.0)
                .padding(padding),
            6.0,
        );
        let right_panel = engine.request_layout(&right_panel_style, &[right_header, right_body]);

        let content_row_style = flex_1(gap(h_flex(), PANEL_GAP));
        let content_row =
            engine.request_layout(&content_row_style, &[left_panel, center_panel, right_panel]);

        let command_bar = engine.request_leaf(&LayoutStyle::new().height(px(COMMAND_BAR_HEIGHT)));

        let root_style = v_flex()
            .width(px(bounds.size.width))
            .height(px(bounds.size.height));
        let root = engine.request_layout(&root_style, &[content_row, command_bar]);

        engine.compute_layout(root, Size::new(bounds.size.width, bounds.size.height));
        let origin = bounds.origin;

        let left_panel_bounds = offset_bounds(engine.layout(left_panel), origin);
        let center_panel_bounds = offset_bounds(engine.layout(center_panel), origin);
        let right_panel_bounds = offset_bounds(engine.layout(right_panel), origin);
        let command_bar_bounds = offset_bounds(engine.layout(command_bar), origin);

        let left_header_bounds = offset_bounds(engine.layout(left_header), origin);
        let left_list_bounds = offset_bounds(engine.layout(left_list), origin);
        let center_header_bounds = offset_bounds(engine.layout(center_header), origin);
        let thread_bounds = offset_bounds(engine.layout(thread_body), origin);
        let editor_bounds = offset_bounds(engine.layout(composer), origin);
        let right_header_bounds = offset_bounds(engine.layout(right_header), origin);
        let right_body_bounds = offset_bounds(engine.layout(right_body), origin);

        Self {
            left_panel_bounds,
            center_panel_bounds,
            right_panel_bounds,
            command_bar_bounds,
            left_header_bounds,
            left_list_bounds,
            center_header_bounds,
            thread_bounds,
            editor_bounds,
            right_header_bounds,
            right_body_bounds,
        }
    }
}

fn offset_bounds(bounds: Bounds, origin: Point) -> Bounds {
    Bounds::new(
        bounds.origin.x + origin.x,
        bounds.origin.y + origin.y,
        bounds.size.width,
        bounds.size.height,
    )
}

fn stack_bounds(bounds: Bounds, heights: &[f32], gap: f32) -> Vec<Bounds> {
    if heights.is_empty() {
        return Vec::new();
    }

    let mut engine = LayoutEngine::new();
    let gap = length(gap);
    let mut nodes = Vec::with_capacity(heights.len());

    for height in heights {
        let style = LayoutStyle::new().height(px(*height)).flex_shrink(0.0);
        nodes.push(engine.request_leaf(&style));
    }

    let stack_style = LayoutStyle::new()
        .flex_col()
        .gap(gap)
        .width(px(bounds.size.width))
        .height(px(bounds.size.height));
    let stack = engine.request_layout(&stack_style, &nodes);

    engine.compute_layout(stack, Size::new(bounds.size.width, bounds.size.height));

    nodes
        .into_iter()
        .map(|node| offset_bounds(engine.layout(node), bounds.origin))
        .collect()
}

fn paint_panel(cx: &mut PaintContext, bounds: Bounds) {
    let panel = Quad::new(bounds)
        .with_background(theme::bg::MUTED)
        .with_border(theme::border::DEFAULT, 1.0)
        .with_corner_radius(6.0);
    cx.scene.draw_quad(panel);
}

#[cfg(test)]
mod tests;
