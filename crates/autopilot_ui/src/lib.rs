use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use autopilot_app::{AppEvent, SessionId, ThreadSnapshot, ThreadSummary, UserAction, WorkspaceId};
use bip39::Mnemonic;
use nostr::derive_keypair;
use openagents_spark::SparkSigner;
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
use wgpui::input::{InputEvent, Key, NamedKey};
use wgpui::{
    Bounds, Button, ButtonVariant, Component, Cursor, Dropdown, DropdownOption, EventResult, Hsla,
    LayoutEngine, LayoutStyle, MarkdownConfig, MarkdownDocument, MarkdownView, MouseButton,
    PaintContext, Point, Quad, ScrollView, Size, StreamingMarkdown, copy_to_clipboard, length, px,
    text::FontStyle, theme,
};

const PANEL_PADDING: f32 = 12.0;
const PANEL_GAP: f32 = 12.0;
const LEFT_PANEL_WIDTH: f32 = 250.0;
const RIGHT_PANEL_WIDTH: f32 = 260.0;
const PANEL_HEADER_HEIGHT: f32 = 26.0;
const SESSION_ROW_HEIGHT: f32 = 30.0;
const COMMAND_BAR_HEIGHT: f32 = 42.0;
const COMPOSER_HEIGHT: f32 = 56.0;
const STATUS_LINE_HEIGHT: f32 = 22.0;
const STATUS_SECTION_GAP: f32 = 10.0;
const ACCENT_BAR_WIDTH: f32 = 3.0;
const DEFAULT_THREAD_MODEL: &str = "gpt-5.1-codex-mini";
const BOTTOM_BAR_MIN_HEIGHT: f32 = 64.0;
const INPUT_MIN_LINES: usize = 1;
const INPUT_MAX_LINES: Option<usize> = None;
const MODEL_DROPDOWN_WIDTH: f32 = 320.0;
const REASONING_DROPDOWN_MIN_WIDTH: f32 = 140.0;
const REASONING_DROPDOWN_MAX_WIDTH: f32 = 200.0;
const DEFAULT_MODEL_INDEX: usize = 3;
const DEFAULT_REASONING_EFFORT: &str = "medium";
const SHOW_MODEL_DROPDOWN: bool = false;
const SHOW_MODEL_SELECTOR: bool = true;
const PANE_MARGIN: f32 = 24.0;
const PANE_OFFSET: f32 = 28.0;
const PANE_MIN_WIDTH: f32 = 200.0;
const PANE_MIN_HEIGHT: f32 = 100.0;
const PANE_TITLE_HEIGHT: f32 = 28.0;
const PANE_RESIZE_HANDLE: f32 = 10.0;
const CHAT_PANE_WIDTH: f32 = 820.0;
const CHAT_PANE_HEIGHT: f32 = 620.0;
const EVENTS_PANE_WIDTH: f32 = 480.0;
const EVENTS_PANE_HEIGHT: f32 = 520.0;
const THREADS_PANE_WIDTH: f32 = 520.0;
const THREADS_PANE_HEIGHT: f32 = 520.0;
const IDENTITY_PANE_WIDTH: f32 = 520.0;
const IDENTITY_PANE_HEIGHT: f32 = 520.0;
const PYLON_PANE_WIDTH: f32 = 520.0;
const PYLON_PANE_HEIGHT: f32 = 420.0;
const WALLET_PANE_WIDTH: f32 = 520.0;
const WALLET_PANE_HEIGHT: f32 = 420.0;
const SELL_COMPUTE_PANE_WIDTH: f32 = 560.0;
const SELL_COMPUTE_PANE_HEIGHT: f32 = 460.0;
const HISTORY_PANE_WIDTH: f32 = 640.0;
const HISTORY_PANE_HEIGHT: f32 = 500.0;
const NIP90_PANE_WIDTH: f32 = 640.0;
const NIP90_PANE_HEIGHT: f32 = 520.0;
const HOTBAR_HEIGHT: f32 = 52.0;
const HOTBAR_FLOAT_GAP: f32 = 18.0;
const HOTBAR_ITEM_SIZE: f32 = 36.0;
const HOTBAR_ITEM_GAP: f32 = 6.0;
const HOTBAR_PADDING: f32 = 6.0;
const HOTBAR_SLOT_EVENTS: u8 = 0;
const HOTBAR_SLOT_NEW_CHAT: u8 = 1;
const HOTBAR_SLOT_IDENTITY: u8 = 2;
const HOTBAR_SLOT_PYLON: u8 = 5;
const HOTBAR_SLOT_WALLET: u8 = 3;
const HOTBAR_SLOT_SELL_COMPUTE: u8 = 6;
const HOTBAR_SLOT_THREADS: u8 = 4;
const HOTBAR_CHAT_SLOT_START: u8 = 7;
const HOTBAR_SLOT_MAX: u8 = 9;
const MODEL_OPTIONS: [(&str, &str); 4] = [
    ("gpt-5.2-codex", "Latest frontier agentic coding model."),
    (
        "gpt-5.2",
        "Latest frontier model with improvements across knowledge, reasoning and coding.",
    ),
    (
        "gpt-5.1-codex-max",
        "Codex-optimized flagship for deep and fast reasoning.",
    ),
    (
        "gpt-5.1-codex-mini",
        "Optimized for Codex. Cheaper, faster, but less capable.",
    ),
];
const REASONING_OPTIONS_FULL: [&str; 4] = ["low", "medium", "high", "xhigh"];
const REASONING_OPTIONS_MINI: [&str; 2] = ["medium", "high"];

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
            AppEvent::DvmProviderStatus { .. } => {}
            AppEvent::DvmHistory { .. } => {}
            AppEvent::Nip90Log { .. } => {}
            AppEvent::ThreadsUpdated { .. } => {}
            AppEvent::ThreadLoaded { .. } => {}
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

fn build_model_options() -> Vec<DropdownOption> {
    MODEL_OPTIONS
        .iter()
        .map(|(id, _)| DropdownOption::new(*id, *id))
        .collect()
}

fn reasoning_options_for_model(model: &str) -> &'static [&'static str] {
    match model {
        "gpt-5.1-codex-mini" => &REASONING_OPTIONS_MINI,
        _ => &REASONING_OPTIONS_FULL,
    }
}

fn build_reasoning_options(model: &str) -> Vec<DropdownOption> {
    reasoning_options_for_model(model)
        .iter()
        .map(|value| DropdownOption::new(*value, *value))
        .collect()
}

fn reasoning_index(model: &str, effort: &str) -> Option<usize> {
    reasoning_options_for_model(model)
        .iter()
        .position(|value| value.eq_ignore_ascii_case(effort))
}

fn default_reasoning_for_model(_model: &str) -> &'static str {
    DEFAULT_REASONING_EFFORT
}

fn model_index(model: &str) -> Option<usize> {
    MODEL_OPTIONS.iter().position(|(id, _)| *id == model)
}

fn extract_message_text(item: &Value) -> Option<String> {
    if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
        return Some(text.to_string());
    }

    if let Some(content) = item.get("content").and_then(|c| c.as_array()) {
        for entry in content {
            if let Some(text) = entry.get("text").and_then(|t| t.as_str()) {
                return Some(text.to_string());
            }
        }
    }

    None
}

fn item_id(item: &Value) -> Option<String> {
    item.get("id")
        .and_then(|id| id.as_str())
        .map(|s| s.to_string())
}

fn item_string(item: &Value, key: &str) -> Option<String> {
    item.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn value_to_command_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => {
            if text.trim().is_empty() {
                None
            } else {
                Some(text.to_string())
            }
        }
        Value::Array(parts) => {
            let items: Vec<&str> = parts.iter().filter_map(|val| val.as_str()).collect();
            if items.is_empty() {
                None
            } else {
                Some(items.join(" "))
            }
        }
        _ => None,
    }
}

fn command_string_from_item(item: &Value) -> Option<String> {
    item.get("command").and_then(value_to_command_string)
}

fn extract_file_changes(item: &Value) -> (Vec<String>, Option<String>, Option<String>) {
    let mut paths = Vec::new();
    let mut first_path = None;
    let mut first_diff = None;
    if let Some(changes) = item.get("changes").and_then(Value::as_array) {
        for change in changes {
            if let Some(path) = change.get("path").and_then(Value::as_str) {
                if first_path.is_none() {
                    first_path = Some(path.to_string());
                }
                paths.push(path.to_string());
            }
            if first_diff.is_none() {
                if let Some(diff) = change.get("diff").and_then(Value::as_str) {
                    first_diff = Some(diff.to_string());
                }
            }
        }
    }
    (paths, first_path, first_diff)
}

#[derive(Clone, Debug)]
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
    Identity,
    Pylon,
    Wallet,
    SellCompute,
    DvmHistory,
    Nip90,
}

#[derive(Clone, Debug)]
enum HotbarAction {
    FocusPane(String),
    ToggleEvents,
    ToggleThreads,
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
                self.closed_positions.insert(
                    pane.id.clone(),
                    PaneSnapshot {
                        rect: pane.rect,
                    },
                );
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

fn normalize_pane_rect(rect: PaneRect) -> PaneRect {
    let mut width = rect.width.max(PANE_MIN_WIDTH);
    let mut height = rect.height.max(PANE_MIN_HEIGHT);
    if width.is_nan() || width <= 0.0 {
        width = PANE_MIN_WIDTH;
    }
    if height.is_nan() || height <= 0.0 {
        height = PANE_MIN_HEIGHT;
    }
    PaneRect {
        x: rect.x,
        y: rect.y,
        width,
        height,
    }
}

fn calculate_new_pane_position(
    last: Option<PaneRect>,
    screen: Size,
    width: f32,
    height: f32,
) -> PaneRect {
    if let Some(last) = last {
        let mut x = last.x + PANE_OFFSET;
        let mut y = last.y + PANE_OFFSET;
        if x + width > screen.width - PANE_MARGIN {
            x = PANE_MARGIN;
        }
        if y + height > screen.height - PANE_MARGIN {
            y = PANE_MARGIN;
        }
        PaneRect { x, y, width, height }
    } else {
        PaneRect {
            x: (screen.width - width) * 0.5,
            y: (screen.height - height) * 0.3,
            width,
            height,
        }
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
    thread_entries: Vec<ThreadEntryView>,
    pending_thread_open: Rc<RefCell<Option<String>>>,
    keygen_button: Button,
    keygen_bounds: Bounds,
    pending_keygen: Rc<RefCell<bool>>,
    pylon_status: PylonStatusView,
    pylon_toggle_button: Button,
    pylon_toggle_bounds: Bounds,
    pylon_init_button: Button,
    pylon_start_button: Button,
    pylon_stop_button: Button,
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
    pending_wallet_refresh: Rc<RefCell<bool>>,
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
    queued_messages: Vec<QueuedMessage>,
    queued_in_flight: bool,
    stop_button: Button,
    stop_bounds: Bounds,
    pending_stop: Rc<RefCell<bool>>,
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
            .background(theme::bg::APP)
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
        self.input.set_value("");
        self.input_needs_focus = true;
        self.submit_button
            .set_disabled(self.input.get_value().trim().is_empty());
    }

    fn load_thread_snapshot(&mut self, thread: &ThreadSnapshot) {
        self.formatted_thread.clear();
        self.formatted_message_streams.clear();
        self.formatted_message_entries.clear();
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
        self.queued_in_flight = true;
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
                            "agentMessage" => {
                                let text = extract_message_text(item);
                                if let Some(item_id) = item_id(item) {
                                    self.finish_agent_message(&item_id, text.as_deref());
                                } else if let Some(text) = text {
                                    self.append_agent_text(&text);
                                }
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
                                        self.append_reasoning_summary_delta(&item_id, &summary_text);
                                    }
                                    if !content_text.is_empty() && !has_content {
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
                        self.append_agent_delta(item_id, delta);
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

        let pending_thread_open = Rc::new(RefCell::new(None));

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
            thread_entries: Vec::new(),
            pending_thread_open,
            keygen_button,
            keygen_bounds: Bounds::ZERO,
            pending_keygen,
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
            pending_wallet_refresh,
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
        root.open_chat_pane(screen, true, true, DEFAULT_THREAD_MODEL);
        root
    }

    pub fn set_zoom_factor(&mut self, zoom: f32) {
        self.zoom_factor = zoom.max(0.1);
    }

    pub fn apply_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::SessionStarted { session_id, .. } => {
                let pane_id = self
                    .pending_session_panes
                    .pop_front()
                    .unwrap_or_else(|| {
                        self.open_chat_pane(self.screen_size(), true, false, DEFAULT_THREAD_MODEL)
                    });
                if let Some(chat) = self.chat_panes.get_mut(&pane_id) {
                    chat.set_session_id(session_id);
                }
                self.session_to_pane.insert(session_id, pane_id);
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
            AppEvent::DvmProviderStatus { status } => {
                self.sell_compute_status = SellComputeStatusView {
                    running: status.running,
                    provider_active: status.provider_active,
                    host_active: status.host_active,
                    min_price_msats: status.min_price_msats,
                    require_payment: status.require_payment,
                    default_model: status.default_model,
                    backend_preference: status.backend_preference,
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
            AppEvent::Nip90Log { message } => {
                self.nip90_log.push(message);
                if self.nip90_log.len() > 200 {
                    let drain = self.nip90_log.len() - 200;
                    self.nip90_log.drain(0..drain);
                }
            }
            AppEvent::ThreadsUpdated { threads } => {
                self.set_thread_entries(threads);
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
            AppEvent::AppServerEvent { message } => {
                if let Ok(value) = serde_json::from_str::<Value>(&message) {
                    let method = value.get("method").and_then(|m| m.as_str()).unwrap_or("");
                    let params = value.get("params");
                    let session_hint = params
                        .and_then(|p| p.get("sessionId").or_else(|| p.get("session_id")))
                        .and_then(|id| id.as_str());
                    let thread_hint = params
                        .and_then(|p| p.get("threadId").or_else(|| p.get("thread_id")))
                        .and_then(|id| id.as_str())
                        .or_else(|| {
                            params
                                .and_then(|p| p.get("thread"))
                                .and_then(|thread| thread.get("id"))
                                .and_then(|id| id.as_str())
                        });

                    let pane_id = session_hint
                        .and_then(|session_id| self.pane_for_session_id(session_id))
                        .or_else(|| {
                            thread_hint
                                .and_then(|thread_id| self.thread_to_pane.get(thread_id).cloned())
                        })
                        .or_else(|| self.first_chat_without_thread());

                    if let Some(pane_id) = pane_id {
                        if let Some(chat) = self.chat_panes.get_mut(&pane_id) {
                            if method == "thread/started" {
                                if let Some(thread_id) = thread_hint {
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
                                }
                            }

                            if method == "fullauto/decision" {
                                if let Some(action) = params
                                    .and_then(|p| p.get("action"))
                                    .and_then(|value| value.as_str())
                                {
                                    if action != "continue" {
                                        chat.full_auto_enabled = false;
                                    }
                                }
                            }

                            chat.apply_formatted_event(&value);
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

    fn set_thread_entries(&mut self, threads: Vec<ThreadSummary>) {
        self.thread_entries = threads
            .into_iter()
            .map(|summary| {
                let pending = self.pending_thread_open.clone();
                let thread_id = summary.id.clone();
                let label = short_thread_id(&thread_id);
                let button = Button::new(label)
                    .variant(ButtonVariant::Ghost)
                    .font_size(theme::font_size::XS)
                    .padding(6.0, 2.0)
                    .corner_radius(4.0)
                    .on_click(move || {
                        *pending.borrow_mut() = Some(thread_id.clone());
                    });
                ThreadEntryView {
                    summary,
                    open_button: button,
                    open_bounds: Bounds::ZERO,
                }
            })
            .collect();
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

    fn toggle_sell_compute_pane(&mut self, screen: Size) {
        let last_position = self.pane_store.last_pane_position;
        self.pane_store.toggle_pane("sell_compute", screen, |snapshot| {
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
            self.chat_slot_assignments
                .insert(pane_id.to_string(), slot);
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
                let new_pane =
                    self.open_chat_pane(screen, true, true, active_model.as_str());
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
        let start_index = start_id
            .and_then(|id| chat_ids.iter().position(|pane_id| pane_id == &id));

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
        if let InputEvent::KeyDown { key, modifiers } = event {
            if matches!(key, Key::Named(NamedKey::Escape)) && !modifiers.meta && !modifiers.ctrl {
                if let Some(active_id) = self.pane_store.active_pane_id.clone() {
                    let dismissable = self
                        .pane_store
                        .pane(&active_id)
                        .map(|pane| pane.dismissable)
                        .unwrap_or(true);
                    if dismissable {
                        self.close_pane(&active_id);
                        return true;
                    }
                }
            }

            if matches!(key, Key::Named(NamedKey::Tab))
                && !modifiers.shift
                && !modifiers.ctrl
                && !modifiers.alt
                && !modifiers.meta
            {
                return self.cycle_chat_focus();
            }

            if matches!(key, Key::Named(NamedKey::Tab))
                && modifiers.shift
                && !modifiers.ctrl
                && !modifiers.alt
                && !modifiers.meta
            {
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
            }

            if modifiers.meta || modifiers.ctrl {
                if let Key::Character(value) = key {
                    if let Ok(slot) = value.parse::<u8>() {
                        if slot <= HOTBAR_SLOT_MAX {
                            self.hotbar.flash_slot(slot);
                        }
                        if slot <= HOTBAR_SLOT_MAX && self.handle_hotbar_slot(slot) {
                            return true;
                        }
                    }
                }
            }
        }

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
                    if matches!(event, InputEvent::MouseDown { button: MouseButton::Left, .. })
                        && frame.title_bounds().contains(self.cursor_position)
                        && !frame.close_bounds().contains(self.cursor_position)
                    {
                        if let Some(pane) = self.pane_store.pane(&pane_id) {
                            self.pane_drag = Some(PaneDragState {
                                pane_id: pane_id.clone(),
                                origin: self.cursor_position,
                                start_rect: pane.rect.clone(),
                            });
                            self.pane_store.bring_to_front(&pane_id);
                            return true;
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
                                    chat.input
                                        .event(event, chat.input_bounds, &mut self.event_context),
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
                            handled |= refresh_handled || entries_handled;
                        }
                        PaneKind::Identity => {
                            let keygen_handled = matches!(
                                self.keygen_button.event(
                                    event,
                                    self.keygen_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );
                            handled |= keygen_handled;
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
                            handled |= refresh_handled;
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
                let provider = self
                    .nip90_provider_input
                    .get_value()
                    .trim()
                    .to_string();
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
                self.copy_feedback_until =
                    Some(Instant::now() + self.copy_feedback_duration);
            }
        }

        let should_threads_refresh = {
            let mut pending = self.pending_threads_refresh.borrow_mut();
            let value = *pending;
            *pending = false;
            value
        };
        if should_threads_refresh {
            if let Some(handler) = self.send_handler.as_mut() {
                handler(UserAction::ThreadsRefresh);
            }
        }

        let thread_open = {
            let mut pending = self.pending_thread_open.borrow_mut();
            pending.take()
        };
        if let Some(thread_id) = thread_open {
            self.open_thread_from_list(thread_id);
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
        } else if self
            .pane_store
            .panes()
            .iter()
            .rev()
            .any(|pane| {
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
        } else if self.keygen_button.is_hovered() {
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
        } else if self.threads_refresh_button.is_hovered() {
            Cursor::Pointer
        } else if self
            .thread_entries
            .iter()
            .any(|entry| entry.open_button.is_hovered())
        {
            Cursor::Pointer
        } else if SHOW_MODEL_SELECTOR
            && self
                .chat_panes
                .values()
                .any(|chat| {
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
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(Hsla::black()),
        );

        let mut dots_grid = DotsGrid::new()
            .shape(DotShape::Circle)
            .color(theme::text::MUTED)
            .opacity(0.12)
            .distance(32.0)
            .size(1.5);
        dots_grid.paint(bounds, cx);

        self.set_screen_size(Size::new(bounds.size.width, bounds.size.height));

        self.copy_bounds = Bounds::ZERO;
        self.threads_refresh_bounds = Bounds::ZERO;
        self.event_scroll_bounds = Bounds::ZERO;
        self.keygen_bounds = Bounds::ZERO;

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
            cx.scene
                .set_layer(pane_layer_start + index as u32);
            let pane_bounds = Bounds::new(
                bounds.origin.x + pane.rect.x,
                bounds.origin.y + pane.rect.y,
                pane.rect.width,
                pane.rect.height,
            );
            self.pane_bounds.insert(pane.id.clone(), pane_bounds);

            cx.scene.push_clip(pane_bounds);
            let frame = self
                .pane_frames
                .entry(pane.id.clone())
                .or_insert_with(PaneFrame::new);
            frame.set_title(pane.title.clone());
            frame.set_active(self.pane_store.is_active(&pane.id));
            frame.set_dismissable(pane.dismissable);
            frame.set_title_height(PANE_TITLE_HEIGHT);
            frame.paint(pane_bounds, cx);

            let content_bounds = frame.content_bounds();
            match pane.kind {
                PaneKind::Chat => {
                    if let Some(chat) = self.chat_panes.get_mut(&pane.id) {
                        paint_chat_pane(chat, content_bounds, cx);
                    }
                }
                PaneKind::Events => paint_events_pane(self, content_bounds, cx),
                PaneKind::Threads => paint_threads_pane(self, content_bounds, cx),
                PaneKind::Identity => paint_identity_pane(self, content_bounds, cx),
                PaneKind::Pylon => paint_pylon_pane(self, content_bounds, cx),
                PaneKind::Wallet => paint_wallet_pane(self, content_bounds, cx),
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
        // Pylon + Sell Compute panes hidden from hotbar for now.
        // items.push(
        //     HotbarSlot::new(HOTBAR_SLOT_PYLON, "PY", "Pylon")
        //         .active(self.pane_store.is_active("pylon")),
        // );
        // self.hotbar_bindings
        //     .insert(HOTBAR_SLOT_PYLON, HotbarAction::TogglePylon);
        //
        // items.push(
        //     HotbarSlot::new(HOTBAR_SLOT_SELL_COMPUTE, "SC", "Sell")
        //         .active(self.pane_store.is_active("sell_compute")),
        // );
        // self.hotbar_bindings
        //     .insert(HOTBAR_SLOT_SELL_COMPUTE, HotbarAction::ToggleSellCompute);

        items.push(
            HotbarSlot::new(HOTBAR_SLOT_THREADS, "TH", "Threads")
                .active(self.pane_store.is_active("threads")),
        );
        self.hotbar_bindings
            .insert(HOTBAR_SLOT_THREADS, HotbarAction::ToggleThreads);
        // Reserve slots 5 and 6 (currently unused) to keep hotbar numbering contiguous.
        items.push(HotbarSlot::new(HOTBAR_SLOT_PYLON, "", "Slot 5").ghost(true));
        items.push(HotbarSlot::new(HOTBAR_SLOT_SELL_COMPUTE, "", "Slot 6").ghost(true));

        // DVM History + NIP-90 panes disabled for now (keep code around).
        // items.push(
        //     HotbarSlot::new(HOTBAR_SLOT_HISTORY, "HI", "History")
        //         .active(self.pane_store.is_active("dvm_history")),
        // );
        // self.hotbar_bindings
        //     .insert(HOTBAR_SLOT_HISTORY, HotbarAction::ToggleDvmHistory);
        //
        // items.push(
        //     HotbarSlot::new(HOTBAR_SLOT_NIP90, "N9", "NIP-90")
        //         .active(self.pane_store.is_active("nip90")),
        // );
        // self.hotbar_bindings
        //     .insert(HOTBAR_SLOT_NIP90, HotbarAction::ToggleNip90);

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
                    HotbarSlot::new(slot, "CH", title)
                        .active(self.pane_store.is_active(pane_id)),
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

fn paint_chat_pane(chat: &mut ChatPaneState, bounds: Bounds, cx: &mut PaintContext) {
    let padding_x = 24.0;
    let padding_top = 4.0;
    let padding_bottom = 16.0;
    let header_height = 0.0;
    let content_width = bounds.size.width - padding_x * 2.0;
    let header_bounds = Bounds::new(
        bounds.origin.x + padding_x,
        bounds.origin.y + padding_top,
        content_width,
        header_height,
    );

    let mut description_top = header_bounds.origin.y + 4.0;
    let mut dropdown_bounds = Bounds::ZERO;

    if SHOW_MODEL_DROPDOWN {
        let selector_height = 30.0;
        let selector_bounds = Bounds::new(
            header_bounds.origin.x,
            description_top + 2.0,
            content_width,
            selector_height,
        );

        let label_text = "Model";
        let label_width =
            cx.text
                .measure_styled_mono(label_text, theme::font_size::SM, FontStyle::default());
        let available_width = (content_width - label_width - 8.0).max(140.0);
        let dropdown_width = available_width.min(MODEL_DROPDOWN_WIDTH);

        let mut engine = LayoutEngine::new();
        let label_node = engine.request_leaf(
            &LayoutStyle::new()
                .width(px(label_width))
                .height(px(selector_height)),
        );
        let dropdown_node = engine.request_leaf(
            &LayoutStyle::new()
                .width(px(dropdown_width))
                .height(px(selector_height)),
        );
        let row = engine.request_layout(
            &LayoutStyle::new()
                .flex_row()
                .align_items(AlignItems::Center)
                .justify_content(JustifyContent::FlexStart)
                .gap(length(8.0)),
            &[label_node, dropdown_node],
        );
        engine.compute_layout(row, Size::new(content_width, selector_height));

        let label_bounds = offset_bounds(engine.layout(label_node), selector_bounds.origin);
        dropdown_bounds = offset_bounds(engine.layout(dropdown_node), selector_bounds.origin);

        Text::new(label_text)
            .font_size(theme::font_size::SM)
            .color(theme::text::MUTED)
            .paint(label_bounds, cx);

        chat.model_bounds = dropdown_bounds;
        description_top = selector_bounds.origin.y + selector_height + 6.0;
    } else {
        chat.model_bounds = Bounds::ZERO;
    }

    if !chat.current_queue().is_empty() {
        let mut queue_text = String::from("Queued");
        for item in chat.current_queue() {
            if item.text.trim().is_empty() {
                continue;
            }
            queue_text.push('\n');
            queue_text.push_str("- ");
            queue_text.push_str(item.text.trim());
        }
        let mut queue_block = Text::new(queue_text)
            .font_size(theme::font_size::XS)
            .color(theme::text::SECONDARY);
        let (_, queue_height) = queue_block.size_hint_with_width(content_width);
        let queue_height = queue_height.unwrap_or(0.0);
        let queue_bounds = Bounds::new(
            header_bounds.origin.x,
            description_top + 10.0,
            content_width,
            queue_height,
        );
        queue_block.paint(queue_bounds, cx);
        description_top = queue_bounds.origin.y + queue_bounds.size.height + 12.0;
    }

    let input_height = input_bar_height(chat, bounds.size.width, cx);
    let input_bounds = Bounds::new(
        bounds.origin.x,
        bounds.origin.y + bounds.size.height - input_height,
        bounds.size.width,
        input_height,
    );

    let feed_top = description_top + 14.0;
    let feed_bottom = input_bounds.origin.y - padding_bottom;
    let feed_bounds = Bounds::new(
        header_bounds.origin.x,
        feed_top,
        header_bounds.size.width,
        (feed_bottom - feed_top).max(0.0),
    );
    chat.formatted_thread_bounds = feed_bounds;

    if chat.formatted_thread.entry_count() != 0 {
        chat.formatted_thread.paint(feed_bounds, cx);
    }

    // Paint dropdown last so the menu overlays the rest of the feed.
    if SHOW_MODEL_DROPDOWN {
        chat.model_dropdown.paint(dropdown_bounds, cx);
    }

    paint_input_bar(chat, input_bounds, cx);
}

fn metrics_buttons_width(
    queue_width: f32,
    full_auto_width: f32,
    send_width: f32,
    stop_width: f32,
    show_stop: bool,
    gap: f32,
) -> f32 {
    let mut width = queue_width + full_auto_width + send_width + gap * 2.0;
    if show_stop {
        width += stop_width + gap;
    }
    width
}

struct InputBarMetrics {
    total_width: f32,
    input_height: f32,
    row_height: f32,
    total_height: f32,
    model_width: f32,
    reasoning_width: f32,
    queue_width: f32,
    full_auto_width: f32,
    send_width: f32,
    stop_width: f32,
    show_stop: bool,
}

fn input_bar_metrics(
    chat: &mut ChatPaneState,
    available_width: f32,
    cx: &mut PaintContext,
) -> InputBarMetrics {
    let gap = 8.0;
    let padding_x = 24.0;
    let padding_y = theme::spacing::MD;
    let button_font = theme::font_size::SM;
    let send_label_width =
        cx.text
            .measure_styled_mono("Send", button_font, FontStyle::default());
    let send_width = (send_label_width + 28.0).max(72.0);
    let stop_label_width =
        cx.text
            .measure_styled_mono("Stop", button_font, FontStyle::default());
    let stop_width = (stop_label_width + 28.0).max(72.0);
    let full_auto_label = if chat.full_auto_enabled {
        "Full Auto On"
    } else {
        "Full Auto Off"
    };
    let full_auto_width =
        cx.text
            .measure_styled_mono(full_auto_label, button_font, FontStyle::default())
            + 28.0;
    let full_auto_width = full_auto_width.max(110.0);
    let queue_label_width = cx
        .text
        .measure_styled_mono("Queue", button_font, FontStyle::default())
        .max(cx.text.measure_styled_mono(
            "Instant",
            button_font,
            FontStyle::default(),
        ));
    let queue_width = (queue_label_width + 28.0).max(90.0);
    let show_stop = chat.is_processing();

    let mut total_width = (available_width - padding_x * 2.0).max(240.0);
    total_width = total_width.min(available_width - padding_x * 2.0);
    let buttons_width =
        metrics_buttons_width(queue_width, full_auto_width, send_width, stop_width, show_stop, gap);
    let min_left = REASONING_DROPDOWN_MIN_WIDTH + 180.0 + gap;
    let left_available = (total_width - buttons_width - gap).max(min_left);
    let mut model_width = (left_available * 0.6).min(MODEL_DROPDOWN_WIDTH).max(180.0);
    let mut reasoning_width = (left_available - model_width - gap)
        .clamp(REASONING_DROPDOWN_MIN_WIDTH, REASONING_DROPDOWN_MAX_WIDTH);
    if model_width + reasoning_width + gap > left_available {
        let overflow = model_width + reasoning_width + gap - left_available;
        if reasoning_width - overflow >= REASONING_DROPDOWN_MIN_WIDTH {
            reasoning_width -= overflow;
        } else if model_width - overflow >= 180.0 {
            model_width -= overflow;
        }
    }

    chat.input.set_max_width(total_width);
    let line_height = button_font * 1.4;
    let input_padding_y = theme::spacing::XS;
    let min_height = line_height * INPUT_MIN_LINES as f32 + input_padding_y * 2.0;
    let mut input_height = chat.input.current_height().max(min_height);
    if let Some(max_lines) = INPUT_MAX_LINES {
        let max_height = line_height * max_lines as f32 + input_padding_y * 2.0;
        input_height = input_height.min(max_height);
    }

    let row_height = 28.0;
    let total_height = input_height + row_height + gap + padding_y * 2.0;

    InputBarMetrics {
        total_width,
        input_height,
        row_height,
        total_height,
        model_width,
        reasoning_width,
        queue_width,
        full_auto_width,
        send_width,
        stop_width,
        show_stop,
    }
}

fn input_bar_height(chat: &mut ChatPaneState, available_width: f32, cx: &mut PaintContext) -> f32 {
    let metrics = input_bar_metrics(chat, available_width, cx);
    metrics.total_height.max(BOTTOM_BAR_MIN_HEIGHT)
}

fn paint_input_bar(chat: &mut ChatPaneState, bounds: Bounds, cx: &mut PaintContext) {
    let gap = 8.0;
    let padding_y = theme::spacing::MD;
    let padding_x = 24.0;
    let metrics = input_bar_metrics(chat, bounds.size.width, cx);

    let bar_x = bounds.origin.x + padding_x;
    let bar_width = metrics.total_width;
    let bar_bottom = bounds.origin.y + bounds.size.height - padding_y;
    let row_y = bar_bottom - metrics.row_height;
    let input_y = row_y - gap - metrics.input_height;
    let input_bounds = Bounds::new(bar_x, input_y, bar_width, metrics.input_height);

    let model_bounds = Bounds::new(bar_x, row_y, metrics.model_width, metrics.row_height);
    let reasoning_bounds = Bounds::new(
        model_bounds.origin.x + model_bounds.size.width + gap,
        row_y,
        metrics.reasoning_width,
        metrics.row_height,
    );

    let mut right_x = bar_x + bar_width;
    let submit_bounds = Bounds::new(
        right_x - metrics.send_width,
        row_y,
        metrics.send_width,
        metrics.row_height,
    );
    right_x = submit_bounds.origin.x - gap;
    let stop_bounds = if metrics.show_stop {
        let bounds = Bounds::new(
            right_x - metrics.stop_width,
            row_y,
            metrics.stop_width,
            metrics.row_height,
        );
        right_x = bounds.origin.x - gap;
        bounds
    } else {
        Bounds::ZERO
    };
    let full_auto_bounds = Bounds::new(
        right_x - metrics.full_auto_width,
        row_y,
        metrics.full_auto_width,
        metrics.row_height,
    );
    right_x = full_auto_bounds.origin.x - gap;
    let queue_bounds = Bounds::new(
        right_x - metrics.queue_width,
        row_y,
        metrics.queue_width,
        metrics.row_height,
    );

    chat.input_bounds = input_bounds;
    chat.submit_bounds = submit_bounds;
    chat.stop_bounds = stop_bounds;
    chat.full_auto_bounds = full_auto_bounds;
    chat.queue_toggle_bounds = queue_bounds;
    chat.model_bounds = model_bounds;
    chat.reasoning_bounds = reasoning_bounds;
    chat.input.set_max_width(input_bounds.size.width);
    chat.submit_button
        .set_disabled(chat.input.get_value().trim().is_empty());
    chat.stop_button.set_disabled(!metrics.show_stop);
    let full_auto_label = if chat.full_auto_enabled {
        "Full Auto On"
    } else {
        "Full Auto Off"
    };
    chat.full_auto_button.set_label(full_auto_label);
    chat.full_auto_button.set_variant(if chat.full_auto_enabled {
        ButtonVariant::Primary
    } else {
        ButtonVariant::Secondary
    });
    let queue_label = if chat.queue_mode { "Queue" } else { "Instant" };
    chat.queue_toggle_button.set_label(queue_label);
    chat.queue_toggle_button.set_variant(if chat.queue_mode {
        ButtonVariant::Primary
    } else {
        ButtonVariant::Secondary
    });
    chat.full_auto_button
        .set_disabled(chat.thread_id.is_none());
    if chat.input_needs_focus {
        chat.input.focus();
        chat.input_needs_focus = false;
    }

    chat.input.paint(input_bounds, cx);
    chat.model_dropdown.paint(model_bounds, cx);
    chat.reasoning_dropdown.paint(reasoning_bounds, cx);
    chat.queue_toggle_button.paint(queue_bounds, cx);
    chat.full_auto_button.paint(full_auto_bounds, cx);
    if metrics.show_stop {
        chat.stop_button.paint(stop_bounds, cx);
    }
    chat.submit_button.paint(submit_bounds, cx);
}

fn paint_identity_pane(root: &mut MinimalRoot, bounds: Bounds, cx: &mut PaintContext) {
    let padding = 16.0;
    let button_height = 28.0;
    let nostr_font = theme::font_size::XS + 4.0;
    let label_height = 16.0;
    let label_value_gap = 4.0;
    let value_spacing = 12.0;

    let mut content_width = (bounds.size.width * 0.6).min(720.0).max(320.0);
    content_width = content_width.min(bounds.size.width - padding * 2.0);
    let content_x = bounds.origin.x + (bounds.size.width - content_width) / 2.0;

    let mut content_height = button_height + 12.0;
    if let Some(npub) = &root.nostr_npub {
        let seed_display = format_seed_phrase(root.seed_phrase.as_deref().unwrap_or(""));
        let mut npub_text = Text::new(npub).font_size(nostr_font);
        let (_, npub_height) = npub_text.size_hint_with_width(content_width);
        let npub_height = npub_height.unwrap_or(label_height);
        content_height += label_height + label_value_gap + npub_height + value_spacing;

        let mut nsec_text =
            Text::new(root.nostr_nsec.as_deref().unwrap_or("")).font_size(nostr_font);
        let (_, nsec_height) = nsec_text.size_hint_with_width(content_width);
        let nsec_height = nsec_height.unwrap_or(label_height);
        content_height += label_height + label_value_gap + nsec_height + value_spacing;

        let mut spark_text =
            Text::new(root.spark_pubkey_hex.as_deref().unwrap_or("")).font_size(nostr_font);
        let (_, spark_height) = spark_text.size_hint_with_width(content_width);
        let spark_height = spark_height.unwrap_or(label_height);
        content_height += label_height + label_value_gap + spark_height + value_spacing;

        let mut seed_text = Text::new(seed_display).font_size(nostr_font);
        let (_, seed_height) = seed_text.size_hint_with_width(content_width);
        let seed_height = seed_height.unwrap_or(label_height);
        content_height += label_height + label_value_gap + seed_height + value_spacing;
    } else if let Some(err) = &root.nostr_error {
        let mut err_text = Text::new(err).font_size(nostr_font);
        let (_, err_height) = err_text.size_hint_with_width(content_width);
        let err_height = err_height.unwrap_or(label_height);
        content_height += err_height + value_spacing;
    } else {
        content_height += label_height + value_spacing;
    }

    let centered_y = bounds.origin.y + (bounds.size.height - content_height).max(0.0) * 0.5;
    let mut y = centered_y.max(bounds.origin.y + padding);

    let button_width = (cx
        .text
        .measure_styled_mono("Generate keys", nostr_font, FontStyle::default())
        + 32.0)
        .max(160.0)
        .min(content_width);
    let keygen_bounds = Bounds::new(
        content_x + (content_width - button_width) / 2.0,
        y,
        button_width,
        button_height,
    );
    root.keygen_bounds = keygen_bounds;
    root.keygen_button.paint(keygen_bounds, cx);
    y += button_height + 12.0;

    if let Some(npub) = &root.nostr_npub {
        let seed_display = format_seed_phrase(root.seed_phrase.as_deref().unwrap_or(""));
        Text::new("nostr public key")
            .font_size(nostr_font)
            .color(theme::text::MUTED)
            .paint(
                Bounds::new(content_x, y, content_width, label_height),
                cx,
            );
        y += label_height + label_value_gap;
        let mut npub_text = Text::new(npub)
            .font_size(nostr_font)
            .color(theme::text::PRIMARY);
        let (_, npub_height) = npub_text.size_hint_with_width(content_width);
        let npub_height = npub_height.unwrap_or(label_height);
        npub_text.paint(
            Bounds::new(content_x, y, content_width, npub_height),
            cx,
        );
        y += npub_height + value_spacing;

        Text::new("nostr secret key")
            .font_size(nostr_font)
            .color(theme::text::MUTED)
            .paint(
                Bounds::new(content_x, y, content_width, label_height),
                cx,
            );
        y += label_height + label_value_gap;
        let mut nsec_text = Text::new(root.nostr_nsec.as_deref().unwrap_or(""))
            .font_size(nostr_font)
            .color(theme::text::PRIMARY);
        let (_, nsec_height) = nsec_text.size_hint_with_width(content_width);
        let nsec_height = nsec_height.unwrap_or(label_height);
        nsec_text.paint(
            Bounds::new(content_x, y, content_width, nsec_height),
            cx,
        );
        y += nsec_height + value_spacing;

        Text::new("spark public key")
            .font_size(nostr_font)
            .color(theme::text::MUTED)
            .paint(
                Bounds::new(content_x, y, content_width, label_height),
                cx,
            );
        y += label_height + label_value_gap;
        let mut spark_text = Text::new(root.spark_pubkey_hex.as_deref().unwrap_or(""))
            .font_size(nostr_font)
            .color(theme::text::PRIMARY);
        let (_, spark_height) = spark_text.size_hint_with_width(content_width);
        let spark_height = spark_height.unwrap_or(label_height);
        spark_text.paint(
            Bounds::new(content_x, y, content_width, spark_height),
            cx,
        );
        y += spark_height + value_spacing;

        Text::new("seed phrase")
            .font_size(nostr_font)
            .color(theme::text::MUTED)
            .paint(
                Bounds::new(content_x, y, content_width, label_height),
                cx,
            );
        y += label_height + label_value_gap;
        let mut seed_text = Text::new(seed_display)
            .font_size(nostr_font)
            .color(theme::text::PRIMARY);
        let (_, seed_height) = seed_text.size_hint_with_width(content_width);
        let seed_height = seed_height.unwrap_or(label_height);
        seed_text.paint(
            Bounds::new(content_x, y, content_width, seed_height),
            cx,
        );
    } else if let Some(err) = &root.nostr_error {
        let mut err_text = Text::new(err)
            .font_size(nostr_font)
            .color(theme::status::ERROR);
        let (_, err_height) = err_text.size_hint_with_width(content_width);
        let err_height = err_height.unwrap_or(label_height);
        err_text.paint(
            Bounds::new(content_x, y, content_width, err_height),
            cx,
        );
    } else {
        Text::new("No keypair generated yet.")
            .font_size(nostr_font)
            .color(theme::text::MUTED)
            .paint(
                Bounds::new(content_x, y, content_width, label_height),
                cx,
            );
    }

    root.copy_bounds = Bounds::ZERO;
    root.event_scroll_bounds = Bounds::ZERO;
}

fn paint_pylon_pane(root: &mut MinimalRoot, bounds: Bounds, cx: &mut PaintContext) {
    let padding = 16.0;
    let button_height = 28.0;
    let label_height = 16.0;
    let value_spacing = 10.0;
    let text_size = theme::font_size::XS;

    let mut content_width = (bounds.size.width * 0.8).min(560.0).max(280.0);
    content_width = content_width.min(bounds.size.width - padding * 2.0);
    let content_x = bounds.origin.x + (bounds.size.width - content_width) / 2.0;

    let mut y = bounds.origin.y + padding;
    let button_row_bounds = Bounds::new(content_x, y, content_width, button_height);
    let toggle_width = 120.0;
    let toggle_bounds = Bounds::new(
        button_row_bounds.origin.x,
        button_row_bounds.origin.y,
        toggle_width,
        button_height,
    );

    root.pylon_toggle_bounds = toggle_bounds;
    root.pylon_init_bounds = Bounds::ZERO;
    root.pylon_start_bounds = Bounds::ZERO;
    root.pylon_stop_bounds = Bounds::ZERO;
    root.pylon_refresh_bounds = Bounds::ZERO;

    let toggle_label = if root.pylon_status.running {
        "Turn Off"
    } else {
        "Turn On"
    };
    root.pylon_toggle_button.set_label(toggle_label);
    root.pylon_toggle_button.set_variant(if root.pylon_status.running {
        ButtonVariant::Secondary
    } else {
        ButtonVariant::Primary
    });
    root.pylon_toggle_button
        .set_disabled(root.pylon_status.last_error.is_some() && !root.pylon_status.running);

    root.pylon_toggle_button.paint(toggle_bounds, cx);

    y += button_height + 14.0;

    let status_line = if root.pylon_status.running {
        "Provider: ON"
    } else {
        "Provider: OFF"
    };
    Text::new(status_line)
        .font_size(text_size)
        .color(theme::text::PRIMARY)
        .paint(Bounds::new(content_x, y, content_width, label_height), cx);
    y += label_height + value_spacing;

    let identity_line = if root.pylon_status.identity_exists {
        "Identity: present"
    } else {
        "Identity: missing (auto-generate on first start)"
    };
    Text::new(identity_line)
        .font_size(text_size)
        .color(theme::text::MUTED)
        .paint(Bounds::new(content_x, y, content_width, label_height), cx);
    y += label_height + value_spacing;

    if let Some(uptime) = root.pylon_status.uptime_secs {
        Text::new(&format!("Uptime: {}s", uptime))
            .font_size(text_size)
            .color(theme::text::MUTED)
            .paint(Bounds::new(content_x, y, content_width, label_height), cx);
        y += label_height + value_spacing;
    }

    if let Some(provider_active) = root.pylon_status.provider_active {
        Text::new(&format!(
            "Provider: {}",
            if provider_active { "active" } else { "inactive" }
        ))
        .font_size(text_size)
        .color(theme::text::MUTED)
        .paint(Bounds::new(content_x, y, content_width, label_height), cx);
        y += label_height + value_spacing;
    }

    if let Some(host_active) = root.pylon_status.host_active {
        Text::new(&format!(
            "Host: {}",
            if host_active { "active" } else { "inactive" }
        ))
        .font_size(text_size)
        .color(theme::text::MUTED)
        .paint(Bounds::new(content_x, y, content_width, label_height), cx);
        y += label_height + value_spacing;
    }

    Text::new(&format!("Jobs completed: {}", root.pylon_status.jobs_completed))
        .font_size(text_size)
        .color(theme::text::MUTED)
        .paint(Bounds::new(content_x, y, content_width, label_height), cx);
    y += label_height + value_spacing;

    Text::new(&format!("Earnings: {} msats", root.pylon_status.earnings_msats))
        .font_size(text_size)
        .color(theme::text::MUTED)
        .paint(Bounds::new(content_x, y, content_width, label_height), cx);
    y += label_height + value_spacing;

    if let Some(err) = root.pylon_status.last_error.as_deref() {
        Text::new(err)
            .font_size(text_size)
            .color(theme::accent::RED)
            .paint(Bounds::new(content_x, y, content_width, label_height), cx);
    }
}

fn paint_wallet_pane(root: &mut MinimalRoot, bounds: Bounds, cx: &mut PaintContext) {
    let padding = 16.0;
    let button_height = 28.0;
    let label_height = 16.0;
    let value_spacing = 10.0;
    let text_size = theme::font_size::XS;

    let mut content_width = (bounds.size.width * 0.8).min(560.0).max(280.0);
    content_width = content_width.min(bounds.size.width - padding * 2.0);
    let content_x = bounds.origin.x + (bounds.size.width - content_width) / 2.0;
    let mut y = bounds.origin.y + padding;

    let refresh_bounds = Bounds::new(content_x, y, 90.0, button_height);
    root.wallet_refresh_bounds = refresh_bounds;
    root.wallet_refresh_button.paint(refresh_bounds, cx);
    y += button_height + 14.0;

    let identity_line = if root.wallet_status.identity_exists {
        "Identity: present"
    } else {
        "Identity: missing"
    };
    Text::new(identity_line)
        .font_size(text_size)
        .color(theme::text::MUTED)
        .paint(Bounds::new(content_x, y, content_width, label_height), cx);
    y += label_height + value_spacing;

    if let Some(network) = root.wallet_status.network.as_deref() {
        Text::new(&format!("Network: {}", network))
            .font_size(text_size)
            .color(theme::text::MUTED)
            .paint(Bounds::new(content_x, y, content_width, label_height), cx);
        y += label_height + value_spacing;
    }

    Text::new(&format!("Total: {} sats", root.wallet_status.total_sats))
        .font_size(text_size)
        .color(theme::text::PRIMARY)
        .paint(Bounds::new(content_x, y, content_width, label_height), cx);
    y += label_height + value_spacing;

    Text::new(&format!("Spark: {} sats", root.wallet_status.spark_sats))
        .font_size(text_size)
        .color(theme::text::MUTED)
        .paint(Bounds::new(content_x, y, content_width, label_height), cx);
    y += label_height + value_spacing;

    Text::new(&format!(
        "Lightning: {} sats",
        root.wallet_status.lightning_sats
    ))
    .font_size(text_size)
    .color(theme::text::MUTED)
    .paint(Bounds::new(content_x, y, content_width, label_height), cx);
    y += label_height + value_spacing;

    Text::new(&format!("On-chain: {} sats", root.wallet_status.onchain_sats))
        .font_size(text_size)
        .color(theme::text::MUTED)
        .paint(Bounds::new(content_x, y, content_width, label_height), cx);
    y += label_height + value_spacing;

    if let Some(address) = root.wallet_status.spark_address.as_deref() {
        Text::new("Spark address")
            .font_size(text_size)
            .color(theme::text::MUTED)
            .paint(Bounds::new(content_x, y, content_width, label_height), cx);
        y += label_height + 4.0;
        let mut text = Text::new(address)
            .font_size(text_size)
            .color(theme::text::PRIMARY);
        let (_, height) = text.size_hint_with_width(content_width);
        let height = height.unwrap_or(label_height);
        text.paint(Bounds::new(content_x, y, content_width, height), cx);
        y += height + value_spacing;
    }

    if let Some(address) = root.wallet_status.bitcoin_address.as_deref() {
        Text::new("Bitcoin address")
            .font_size(text_size)
            .color(theme::text::MUTED)
            .paint(Bounds::new(content_x, y, content_width, label_height), cx);
        y += label_height + 4.0;
        let mut text = Text::new(address)
            .font_size(text_size)
            .color(theme::text::PRIMARY);
        let (_, height) = text.size_hint_with_width(content_width);
        let height = height.unwrap_or(label_height);
        text.paint(Bounds::new(content_x, y, content_width, height), cx);
        y += height + value_spacing;
    }

    if let Some(err) = root.wallet_status.last_error.as_deref() {
        Text::new(err)
            .font_size(text_size)
            .color(theme::accent::RED)
            .paint(Bounds::new(content_x, y, content_width, label_height), cx);
    }
}

fn paint_sell_compute_pane(root: &mut MinimalRoot, bounds: Bounds, cx: &mut PaintContext) {
    let padding = 16.0;
    let button_height = 28.0;
    let label_height = 16.0;
    let value_spacing = 10.0;
    let text_size = theme::font_size::XS;

    let mut content_width = (bounds.size.width * 0.85).min(600.0).max(320.0);
    content_width = content_width.min(bounds.size.width - padding * 2.0);
    let content_x = bounds.origin.x + (bounds.size.width - content_width) / 2.0;
    let mut y = bounds.origin.y + padding;

    let provider_active = root.sell_compute_status.provider_active.unwrap_or(false);
    let running = root.sell_compute_status.running;
    root.sell_compute_online_button
        .set_disabled(running && provider_active);
    root.sell_compute_offline_button
        .set_disabled(!running);

    let online_width = 92.0;
    let offline_width = 96.0;
    let refresh_width = 86.0;
    let gap = 8.0;
    let row_width = online_width + offline_width + refresh_width + gap * 2.0;
    let row_x = content_x + (content_width - row_width).max(0.0) / 2.0;
    let online_bounds = Bounds::new(row_x, y, online_width, button_height);
    let offline_bounds = Bounds::new(row_x + online_width + gap, y, offline_width, button_height);
    let refresh_bounds = Bounds::new(
        row_x + online_width + offline_width + gap * 2.0,
        y,
        refresh_width,
        button_height,
    );
    root.sell_compute_online_bounds = online_bounds;
    root.sell_compute_offline_bounds = offline_bounds;
    root.sell_compute_refresh_bounds = refresh_bounds;
    root.sell_compute_online_button.paint(online_bounds, cx);
    root.sell_compute_offline_button.paint(offline_bounds, cx);
    root.sell_compute_refresh_button.paint(refresh_bounds, cx);
    y += button_height + 14.0;

    let status_line = if running {
        "Daemon: running"
    } else {
        "Daemon: stopped"
    };
    Text::new(status_line)
        .font_size(text_size)
        .color(theme::text::PRIMARY)
        .paint(Bounds::new(content_x, y, content_width, label_height), cx);
    y += label_height + value_spacing;

    if let Some(provider_active) = root.sell_compute_status.provider_active {
        Text::new(&format!(
            "Provider: {}",
            if provider_active { "online" } else { "offline" }
        ))
        .font_size(text_size)
        .color(theme::text::MUTED)
        .paint(Bounds::new(content_x, y, content_width, label_height), cx);
        y += label_height + value_spacing;
    }

    if let Some(host_active) = root.sell_compute_status.host_active {
        Text::new(&format!(
            "Host: {}",
            if host_active { "active" } else { "inactive" }
        ))
        .font_size(text_size)
        .color(theme::text::MUTED)
        .paint(Bounds::new(content_x, y, content_width, label_height), cx);
        y += label_height + value_spacing;
    }

    Text::new(&format!(
        "Min price: {} msats",
        root.sell_compute_status.min_price_msats
    ))
    .font_size(text_size)
    .color(theme::text::MUTED)
    .paint(Bounds::new(content_x, y, content_width, label_height), cx);
    y += label_height + value_spacing;

    Text::new(&format!(
        "Require payment: {}",
        if root.sell_compute_status.require_payment {
            "yes"
        } else {
            "no"
        }
    ))
    .font_size(text_size)
    .color(theme::text::MUTED)
    .paint(Bounds::new(content_x, y, content_width, label_height), cx);
    y += label_height + value_spacing;

    Text::new(&format!(
        "Payments enabled: {}",
        if root.sell_compute_status.enable_payments {
            "yes"
        } else {
            "no"
        }
    ))
    .font_size(text_size)
    .color(theme::text::MUTED)
    .paint(Bounds::new(content_x, y, content_width, label_height), cx);
    y += label_height + value_spacing;

    Text::new(&format!(
        "Network: {}",
        root.sell_compute_status.network
    ))
    .font_size(text_size)
    .color(theme::text::MUTED)
    .paint(Bounds::new(content_x, y, content_width, label_height), cx);
    y += label_height + value_spacing;

    if !root.sell_compute_status.default_model.is_empty() {
        Text::new(&format!(
            "Default model: {}",
            root.sell_compute_status.default_model
        ))
        .font_size(text_size)
        .color(theme::text::MUTED)
        .paint(Bounds::new(content_x, y, content_width, label_height), cx);
        y += label_height + value_spacing;
    }

    if !root.sell_compute_status.backend_preference.is_empty() {
        Text::new("Backends")
            .font_size(text_size)
            .color(theme::text::MUTED)
            .paint(Bounds::new(content_x, y, content_width, label_height), cx);
        y += label_height + 4.0;
        let list = root.sell_compute_status.backend_preference.join(", ");
        let mut text = Text::new(list.as_str())
            .font_size(text_size)
            .color(theme::text::PRIMARY);
        let (_, height) = text.size_hint_with_width(content_width);
        let height = height.unwrap_or(label_height);
        text.paint(Bounds::new(content_x, y, content_width, height), cx);
        y += height + value_spacing;
    }

    if let Some(err) = root.sell_compute_status.last_error.as_deref() {
        Text::new(err)
            .font_size(text_size)
            .color(theme::accent::RED)
            .paint(Bounds::new(content_x, y, content_width, label_height), cx);
    }
}

fn paint_dvm_history_pane(root: &mut MinimalRoot, bounds: Bounds, cx: &mut PaintContext) {
    let padding = 16.0;
    let button_height = 28.0;
    let label_height = 16.0;
    let value_spacing = 8.0;
    let text_size = theme::font_size::XS;

    let mut content_width = (bounds.size.width * 0.9).min(700.0).max(320.0);
    content_width = content_width.min(bounds.size.width - padding * 2.0);
    let content_x = bounds.origin.x + (bounds.size.width - content_width) / 2.0;
    let mut y = bounds.origin.y + padding;

    let refresh_bounds = Bounds::new(
        content_x + content_width - 90.0,
        y,
        90.0,
        button_height,
    );
    root.dvm_history_refresh_bounds = refresh_bounds;
    root.dvm_history_refresh_button.paint(refresh_bounds, cx);

    Text::new("Earnings summary")
        .font_size(text_size)
        .color(theme::text::PRIMARY)
        .paint(Bounds::new(content_x, y + 6.0, content_width, label_height), cx);
    y += button_height + 10.0;

    Text::new(&format!(
        "Total: {} sats ({} msats)",
        root.dvm_history.summary_total_sats, root.dvm_history.summary_total_msats
    ))
    .font_size(text_size)
    .color(theme::text::MUTED)
    .paint(Bounds::new(content_x, y, content_width, label_height), cx);
    y += label_height + value_spacing;

    Text::new(&format!(
        "Jobs completed: {}",
        root.dvm_history.summary_job_count
    ))
    .font_size(text_size)
    .color(theme::text::MUTED)
    .paint(Bounds::new(content_x, y, content_width, label_height), cx);
    y += label_height + value_spacing;

    if !root.dvm_history.summary_by_source.is_empty() {
        let mut sources = root.dvm_history.summary_by_source.clone();
        sources.sort_by(|a, b| a.0.cmp(&b.0));
        let joined = sources
            .into_iter()
            .map(|(source, amount)| format!("{source}: {amount} msats"))
            .collect::<Vec<_>>()
            .join(" | ");
        let mut text = Text::new(joined.as_str())
            .font_size(text_size)
            .color(theme::text::MUTED);
        let (_, height) = text.size_hint_with_width(content_width);
        let height = height.unwrap_or(label_height);
        text.paint(Bounds::new(content_x, y, content_width, height), cx);
        y += height + value_spacing;
    }

    if !root.dvm_history.status_counts.is_empty() {
        let mut counts = root.dvm_history.status_counts.clone();
        counts.sort_by(|a, b| a.0.cmp(&b.0));
        let joined = counts
            .into_iter()
            .map(|(status, count)| format!("{status}: {count}"))
            .collect::<Vec<_>>()
            .join(" | ");
        let mut text = Text::new(joined.as_str())
            .font_size(text_size)
            .color(theme::text::MUTED);
        let (_, height) = text.size_hint_with_width(content_width);
        let height = height.unwrap_or(label_height);
        text.paint(Bounds::new(content_x, y, content_width, height), cx);
        y += height + value_spacing;
    }

    Text::new("Recent jobs")
        .font_size(text_size)
        .color(theme::text::PRIMARY)
        .paint(Bounds::new(content_x, y, content_width, label_height), cx);
    y += label_height + value_spacing;

    if root.dvm_history.jobs.is_empty() {
        Text::new("No jobs recorded yet.")
            .font_size(text_size)
            .color(theme::text::MUTED)
            .paint(Bounds::new(content_x, y, content_width, label_height), cx);
        y += label_height + value_spacing;
    } else {
        for job in &root.dvm_history.jobs {
            let id = if job.id.len() > 8 {
                &job.id[..8]
            } else {
                &job.id
            };
            let line = format!(
                "{id} | {} | kind {} | {} msats | {}",
                job.status, job.kind, job.price_msats, job.created_at
            );
            Text::new(&line)
                .font_size(text_size)
                .color(theme::text::MUTED)
                .paint(Bounds::new(content_x, y, content_width, label_height), cx);
            y += label_height + 4.0;
        }
    }

    if let Some(err) = root.dvm_history.last_error.as_deref() {
        Text::new(err)
            .font_size(text_size)
            .color(theme::accent::RED)
            .paint(Bounds::new(content_x, y, content_width, label_height), cx);
    }
}

fn paint_nip90_pane(root: &mut MinimalRoot, bounds: Bounds, cx: &mut PaintContext) {
    let padding = 16.0;
    let label_height = 16.0;
    let input_height = 28.0;
    let gap = 8.0;
    let text_size = theme::font_size::XS;

    let mut content_width = (bounds.size.width * 0.9).min(720.0).max(320.0);
    content_width = content_width.min(bounds.size.width - padding * 2.0);
    let content_x = bounds.origin.x + (bounds.size.width - content_width) / 2.0;
    let mut y = bounds.origin.y + padding;

    Text::new("Relays")
        .font_size(text_size)
        .color(theme::text::MUTED)
        .paint(Bounds::new(content_x, y, content_width, label_height), cx);
    y += label_height + 4.0;
    let relay_bounds = Bounds::new(content_x, y, content_width, input_height);
    root.nip90_relay_bounds = relay_bounds;
    root.nip90_relay_input.paint(relay_bounds, cx);
    y += input_height + gap;

    Text::new("Job kind")
        .font_size(text_size)
        .color(theme::text::MUTED)
        .paint(Bounds::new(content_x, y, content_width, label_height), cx);
    y += label_height + 4.0;
    let kind_bounds = Bounds::new(content_x, y, 140.0, input_height);
    root.nip90_kind_bounds = kind_bounds;
    root.nip90_kind_input.paint(kind_bounds, cx);
    y += input_height + gap;

    Text::new("Provider (optional)")
        .font_size(text_size)
        .color(theme::text::MUTED)
        .paint(Bounds::new(content_x, y, content_width, label_height), cx);
    y += label_height + 4.0;
    let provider_bounds = Bounds::new(content_x, y, content_width, input_height);
    root.nip90_provider_bounds = provider_bounds;
    root.nip90_provider_input.paint(provider_bounds, cx);
    y += input_height + gap;

    Text::new("Prompt")
        .font_size(text_size)
        .color(theme::text::MUTED)
        .paint(Bounds::new(content_x, y, content_width, label_height), cx);
    y += label_height + 4.0;
    let prompt_height = 64.0;
    let prompt_bounds = Bounds::new(content_x, y, content_width, prompt_height);
    root.nip90_prompt_bounds = prompt_bounds;
    root.nip90_prompt_input.paint(prompt_bounds, cx);
    y += prompt_height + gap;

    let submit_bounds = Bounds::new(content_x, y, 120.0, 32.0);
    root.nip90_submit_bounds = submit_bounds;
    root.nip90_submit_button.paint(submit_bounds, cx);
    y += 32.0 + gap;

    Text::new("Activity")
        .font_size(text_size)
        .color(theme::text::PRIMARY)
        .paint(Bounds::new(content_x, y, content_width, label_height), cx);
    y += label_height + 6.0;

    if root.nip90_log.is_empty() {
        Text::new("No NIP-90 activity yet.")
            .font_size(text_size)
            .color(theme::text::MUTED)
            .paint(Bounds::new(content_x, y, content_width, label_height), cx);
        return;
    }

    let mut lines = String::new();
    for line in root.nip90_log.iter().rev().take(12).rev() {
        lines.push_str(line);
        lines.push('\n');
    }
    let mut log_text = Text::new(lines.as_str())
        .font_size(text_size)
        .color(theme::text::MUTED);
    let (_, height) = log_text.size_hint_with_width(content_width);
    let height = height.unwrap_or(label_height);
    log_text.paint(Bounds::new(content_x, y, content_width, height), cx);
}

fn paint_events_pane(root: &mut MinimalRoot, bounds: Bounds, cx: &mut PaintContext) {
    let padding = 16.0;
    let header_height = 20.0;
    let content_width = bounds.size.width - padding * 2.0;
    let content_x = bounds.origin.x + padding;
    let header_bounds = Bounds::new(content_x, bounds.origin.y + padding, content_width, header_height);
    let copy_button_width = 68.0;
    let copy_bounds = Bounds::new(
        header_bounds.origin.x + header_bounds.size.width - copy_button_width,
        header_bounds.origin.y - 4.0,
        copy_button_width,
        24.0,
    );
    let title_bounds = Bounds::new(
        header_bounds.origin.x,
        header_bounds.origin.y,
        header_bounds.size.width - copy_button_width - 8.0,
        header_height,
    );

    Text::new("CODEX EVENTS")
        .font_size(theme::font_size::SM)
        .bold()
        .color(theme::text::PRIMARY)
        .paint(title_bounds, cx);

    root.copy_bounds = copy_bounds;
    let copy_label = if let Some(until) = root.copy_feedback_until {
        if Instant::now() < until {
            "Copied"
        } else {
            root.copy_feedback_until = None;
            "Copy"
        }
    } else {
        "Copy"
    };
    root.copy_button.set_label(copy_label);
    root.copy_button.set_disabled(root.event_log.is_empty());
    root.copy_button.paint(copy_bounds, cx);

    let feed_top = header_bounds.origin.y + header_height + 8.0;
    let feed_bottom = bounds.origin.y + bounds.size.height - padding;
    let feed_height = (feed_bottom - feed_top).max(0.0);
    let feed_bounds = Bounds::new(content_x, feed_top, content_width, feed_height);

    let font_size = theme::font_size::XS;
    root.event_scroll_bounds = feed_bounds;

    if root.event_log.is_empty() {
        Text::new("No events yet.")
            .font_size(font_size)
            .color(theme::text::MUTED)
            .paint(feed_bounds, cx);
        return;
    }

    let mut block = String::new();
    for line in &root.event_log {
        block.push_str(line);
        block.push('\n');
    }

    let mut feed_text = Text::new(block.as_str())
        .font_size(font_size)
        .color(theme::text::MUTED);
    let (_, height_opt) = feed_text.size_hint_with_width(feed_bounds.size.width);
    let content_height = height_opt
        .unwrap_or(feed_bounds.size.height)
        .max(feed_bounds.size.height);
    root.event_scroll
        .set_content_size(Size::new(feed_bounds.size.width, content_height));
    root.event_scroll.set_content(feed_text);

    if root.event_log_dirty {
        let max_scroll = (content_height - feed_bounds.size.height).max(0.0);
        root.event_scroll.scroll_to(Point::new(0.0, max_scroll));
        root.event_log_dirty = false;
    }

    root.event_scroll.paint(feed_bounds, cx);
}

fn paint_threads_pane(root: &mut MinimalRoot, bounds: Bounds, cx: &mut PaintContext) {
    let padding = 16.0;
    let header_height = 20.0;
    let content_width = bounds.size.width - padding * 2.0;
    let content_x = bounds.origin.x + padding;
    let header_bounds =
        Bounds::new(content_x, bounds.origin.y + padding, content_width, header_height);
    let refresh_button_width = 72.0;
    let refresh_bounds = Bounds::new(
        header_bounds.origin.x + header_bounds.size.width - refresh_button_width,
        header_bounds.origin.y - 4.0,
        refresh_button_width,
        24.0,
    );
    let title_bounds = Bounds::new(
        header_bounds.origin.x,
        header_bounds.origin.y,
        header_bounds.size.width - refresh_button_width - 8.0,
        header_height,
    );

    Text::new("RECENT THREADS")
        .font_size(theme::font_size::SM)
        .bold()
        .color(theme::text::PRIMARY)
        .paint(title_bounds, cx);

    root.threads_refresh_bounds = refresh_bounds;
    root.threads_refresh_button.paint(refresh_bounds, cx);

    let mut y = header_bounds.origin.y + header_height + 8.0;
    let row_height = 18.0;
    let id_width = 86.0;
    let preview_width = (content_width - id_width - 8.0).max(0.0);

    if root.thread_entries.is_empty() {
        Text::new("No recent threads.")
            .font_size(theme::font_size::XS)
            .color(theme::text::MUTED)
            .paint(Bounds::new(content_x, y, content_width, row_height), cx);
        return;
    }

    for entry in &mut root.thread_entries {
        let id_bounds = Bounds::new(content_x, y, id_width, row_height);
        entry.open_bounds = id_bounds;
        entry.open_button.paint(id_bounds, cx);

        let preview_text = if entry.summary.preview.trim().is_empty() {
            "No preview"
        } else {
            entry.summary.preview.trim()
        };
        Text::new(preview_text)
            .font_size(theme::font_size::XS)
            .color(theme::text::MUTED)
            .paint(
                Bounds::new(content_x + id_width + 8.0, y, preview_width, row_height),
                cx,
            );
        y += row_height + 6.0;
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
    let mut x = header_bounds.origin.x + header_bounds.size.width - 6.0;

    let count_label = session_count.to_string();
    let count_width = badge_width(cx, &count_label);
    x -= count_width;
    paint_badge(
        cx,
        &count_label,
        Bounds::new(
            x,
            header_bounds.origin.y + (header_bounds.size.height - badge_height) / 2.0,
            count_width,
            badge_height,
        ),
        true,
    );

    let new_width = badge_width(cx, "NEW");
    x -= badge_gap + new_width;
    paint_badge(
        cx,
        "NEW",
        Bounds::new(
            x,
            header_bounds.origin.y + (header_bounds.size.height - badge_height) / 2.0,
            new_width,
            badge_height,
        ),
        false,
    );
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
            cx.scene.draw_quad(
                Quad::new(Bounds::new(
                    row_bounds.origin.x,
                    row_bounds.origin.y,
                    ACCENT_BAR_WIDTH,
                    row_bounds.size.height,
                ))
                .with_background(theme::accent::PRIMARY),
            );
        }

        let id_bounds = Bounds::new(
            row_bounds.origin.x + 6.0,
            row_bounds.origin.y,
            id_column_width,
            row_bounds.size.height,
        );
        let detail_bounds = Bounds::new(
            row_bounds.origin.x + id_column_width + 10.0,
            row_bounds.origin.y,
            (row_bounds.size.width - id_column_width - 12.0).max(0.0),
            row_bounds.size.height,
        );

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
    let y = bounds.origin.y + bounds.size.height + 4.0;
    cx.scene.draw_quad(
        Quad::new(Bounds::new(bounds.origin.x, y, bounds.size.width, 1.0))
            .with_background(theme::border::SUBTLE),
    );
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
    let mut x = header_bounds.origin.x + header_bounds.size.width - 6.0;

    for label in labels.iter() {
        let width = badge_width(cx, label);
        x -= width;
        let bounds = Bounds::new(
            x,
            header_bounds.origin.y + (header_bounds.size.height - pill_height) / 2.0,
            width,
            pill_height,
        );
        if *label == "CONNECTED" {
            paint_badge(cx, label, bounds, true);
        } else {
            paint_badge(cx, label, bounds, false);
        }
        x -= gap;
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
                let label_bounds = Bounds::new(
                    row_bounds.origin.x,
                    row_bounds.origin.y,
                    label_width,
                    row_bounds.size.height,
                );
                let value_bounds = Bounds::new(
                    row_bounds.origin.x + label_width + 6.0,
                    row_bounds.origin.y,
                    row_bounds.size.width - label_width - 6.0,
                    row_bounds.size.height,
                );

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
                let mut x = row_bounds.origin.x;
                for action in actions {
                    let width = badge_width(cx, action.label);
                    paint_badge(
                        cx,
                        action.label,
                        Bounds::new(x, row_bounds.origin.y, width, 16.0),
                        action.active,
                    );
                    x += width + 6.0;
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

    let mut x = bounds.origin.x + 8.0;
    let y = bounds.origin.y + (bounds.size.height - 18.0) / 2.0;
    for (key, tag, label, shortcut) in hints {
        let text = format!("{key} {tag} {label} {shortcut}");
        let text_width = cx.text.measure(&text, theme::font_size::SM);
        let width = text_width + 18.0;
        let box_bounds = Bounds::new(x, y, width, 18.0);
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
        x += width + 6.0;
    }
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
struct SellComputeStatusView {
    running: bool,
    provider_active: Option<bool>,
    host_active: Option<bool>,
    min_price_msats: u64,
    require_payment: bool,
    default_model: String,
    backend_preference: Vec<String>,
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
            UserAction::PylonInit => "PylonInit".to_string(),
            UserAction::PylonStart => "PylonStart".to_string(),
            UserAction::PylonStop => "PylonStop".to_string(),
            UserAction::PylonRefresh => "PylonRefresh".to_string(),
            UserAction::WalletRefresh => "WalletRefresh".to_string(),
            UserAction::DvmProviderStart => "DvmProviderStart".to_string(),
            UserAction::DvmProviderStop => "DvmProviderStop".to_string(),
            UserAction::DvmProviderRefresh => "DvmProviderRefresh".to_string(),
            UserAction::DvmHistoryRefresh => "DvmHistoryRefresh".to_string(),
            UserAction::Nip90Submit { kind, .. } => format!("Nip90Submit (kind {kind})"),
            UserAction::ThreadsRefresh => "ThreadsRefresh".to_string(),
            UserAction::ThreadOpen { thread_id } => format!("ThreadOpen ({thread_id})"),
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
        AppEvent::DvmProviderStatus { status } => {
            if status.running {
                "DvmProviderStatus (running)".to_string()
            } else {
                "DvmProviderStatus (stopped)".to_string()
            }
        }
        AppEvent::DvmHistory { snapshot } => format!(
            "DvmHistory ({} jobs)",
            snapshot.summary.job_count
        ),
        AppEvent::Nip90Log { message } => format!("Nip90Log ({message})"),
        AppEvent::ThreadsUpdated { threads } => format!("ThreadsUpdated ({})", threads.len()),
        AppEvent::ThreadLoaded { thread, .. } => format!("ThreadLoaded ({})", thread.id),
    }
}

fn format_session_id(session_id: SessionId) -> String {
    let raw = format!("{:?}", session_id);
    let trimmed = raw.trim_start_matches("SessionId(").trim_end_matches(')');
    trimmed.chars().take(6).collect()
}

fn short_thread_id(id: &str) -> String {
    id.chars().take(8).collect()
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
        let panel_gap = length(PANEL_GAP);
        let inner_gap = length(6.0);
        let padding = length(PANEL_PADDING);

        let left_header = engine.request_leaf(&LayoutStyle::new().height(px(PANEL_HEADER_HEIGHT)));
        let left_list = engine.request_layout(&LayoutStyle::new().flex_grow(1.0), &[]);
        let left_panel_style = LayoutStyle::new()
            .flex_col()
            .width(px(LEFT_PANEL_WIDTH))
            .flex_shrink(0.0)
            .gap(inner_gap)
            .padding(padding);
        let left_panel = engine.request_layout(&left_panel_style, &[left_header, left_list]);

        let center_header =
            engine.request_leaf(&LayoutStyle::new().height(px(PANEL_HEADER_HEIGHT)));
        let thread_body = engine.request_layout(&LayoutStyle::new().flex_grow(1.0), &[]);
        let composer = engine.request_leaf(&LayoutStyle::new().height(px(COMPOSER_HEIGHT)));
        let center_panel_style = LayoutStyle::new()
            .flex_col()
            .flex_grow(1.0)
            .gap(inner_gap)
            .padding(padding);
        let center_panel =
            engine.request_layout(&center_panel_style, &[center_header, thread_body, composer]);

        let right_header = engine.request_leaf(&LayoutStyle::new().height(px(PANEL_HEADER_HEIGHT)));
        let right_body = engine.request_layout(&LayoutStyle::new().flex_grow(1.0), &[]);
        let right_panel_style = LayoutStyle::new()
            .flex_col()
            .width(px(RIGHT_PANEL_WIDTH))
            .flex_shrink(0.0)
            .gap(inner_gap)
            .padding(padding);
        let right_panel = engine.request_layout(&right_panel_style, &[right_header, right_body]);

        let content_row_style = LayoutStyle::new().flex_row().gap(panel_gap).flex_grow(1.0);
        let content_row =
            engine.request_layout(&content_row_style, &[left_panel, center_panel, right_panel]);

        let command_bar = engine.request_leaf(&LayoutStyle::new().height(px(COMMAND_BAR_HEIGHT)));

        let root_style = LayoutStyle::new()
            .flex_col()
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
mod tests {
    use super::*;

    fn approx_eq(a: f32, b: f32) {
        assert!(
            (a - b).abs() < 0.5,
            "expected {a} ~= {b} (diff {})",
            (a - b).abs()
        );
    }

    #[test]
    fn layout_panels_are_consistent() {
        let bounds = Bounds::new(0.0, 0.0, 1200.0, 800.0);
        let layout = Layout::new(bounds);

        approx_eq(layout.left_panel_bounds.size.width, LEFT_PANEL_WIDTH);
        approx_eq(layout.right_panel_bounds.size.width, RIGHT_PANEL_WIDTH);

        let expected_center_width =
            bounds.size.width - LEFT_PANEL_WIDTH - RIGHT_PANEL_WIDTH - PANEL_GAP * 2.0;
        approx_eq(layout.center_panel_bounds.size.width, expected_center_width);

        approx_eq(layout.command_bar_bounds.size.height, COMMAND_BAR_HEIGHT);
        approx_eq(
            layout.command_bar_bounds.origin.y,
            bounds.size.height - COMMAND_BAR_HEIGHT,
        );

        assert!(
            layout.right_body_bounds.origin.y
                >= layout.right_header_bounds.origin.y + layout.right_header_bounds.size.height
        );
        assert!(
            layout.left_header_bounds.origin.x
                >= layout.left_panel_bounds.origin.x + PANEL_PADDING - 0.5
        );
    }
}
