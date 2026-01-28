use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use autopilot_app::{AppEvent, SessionId, UserAction, WorkspaceId};
use bip39::Mnemonic;
use nostr::derive_keypair;
use openagents_spark::SparkSigner;
use rand::RngCore;
use serde_json::Value;
use taffy::prelude::{AlignItems, JustifyContent};
use wgpui::components::EventContext;
use wgpui::components::atoms::{ToolStatus, ToolType};
use wgpui::components::hud::{Hotbar, HotbarSlot, PaneFrame};
use wgpui::components::organisms::{
    AssistantMessage, CodexReasoningCard, DiffLine, DiffLineKind, DiffToolCall, SearchMatch,
    SearchToolCall, TerminalToolCall, ThreadEntry, ThreadEntryType, ToolCallCard, UserMessage,
};
use wgpui::components::sections::{MessageEditor, ThreadView};
use wgpui::components::{Text, TextInput};
use wgpui::input::{InputEvent, Key, NamedKey};
use wgpui::{
    Bounds, Button, ButtonVariant, Component, Cursor, Dropdown, DropdownOption, EventResult,
    LayoutEngine, LayoutStyle, MarkdownConfig, MarkdownDocument, MarkdownView, PaintContext, Point,
    Quad, ScrollView, Size, StreamingMarkdown, copy_to_clipboard, length, px, text::FontStyle,
    theme,
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
const DEFAULT_MODEL_INDEX: usize = 3;
const SHOW_MODEL_DROPDOWN: bool = false;
const PANE_MARGIN: f32 = 24.0;
const PANE_OFFSET: f32 = 28.0;
const PANE_MIN_WIDTH: f32 = 240.0;
const PANE_MIN_HEIGHT: f32 = 140.0;
const PANE_TITLE_HEIGHT: f32 = 28.0;
const CHAT_PANE_WIDTH: f32 = 820.0;
const CHAT_PANE_HEIGHT: f32 = 620.0;
const EVENTS_PANE_WIDTH: f32 = 480.0;
const EVENTS_PANE_HEIGHT: f32 = 520.0;
const IDENTITY_PANE_WIDTH: f32 = 520.0;
const IDENTITY_PANE_HEIGHT: f32 = 520.0;
const HOTBAR_HEIGHT: f32 = 52.0;
const HOTBAR_FLOAT_GAP: f32 = 18.0;
const HOTBAR_ITEM_SIZE: f32 = 36.0;
const HOTBAR_ITEM_GAP: f32 = 6.0;
const HOTBAR_PADDING: f32 = 6.0;
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

#[derive(Clone, Debug, PartialEq, Eq)]
enum PaneKind {
    Chat,
    Events,
    Identity,
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

    fn toggle_pane<F>(&mut self, id: &str, screen: Size, mut create: F)
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
        pane.rect = ensure_pane_visible(pane.rect, screen);
        self.add_pane(pane);
    }
}

fn ensure_pane_visible(rect: PaneRect, screen: Size) -> PaneRect {
    let mut width = rect.width.max(PANE_MIN_WIDTH).min(screen.width - PANE_MARGIN * 2.0);
    let mut height = rect.height.max(PANE_MIN_HEIGHT).min(screen.height - PANE_MARGIN * 2.0);
    if width.is_nan() || width <= 0.0 {
        width = PANE_MIN_WIDTH;
    }
    if height.is_nan() || height <= 0.0 {
        height = PANE_MIN_HEIGHT;
    }
    let mut x = rect.x;
    let mut y = rect.y;
    x = x.max(PANE_MARGIN).min(screen.width - width - PANE_MARGIN);
    y = y.max(PANE_MARGIN).min(screen.height - height - PANE_MARGIN);
    PaneRect { x, y, width, height }
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
    pane_store: PaneStore,
    pane_frames: HashMap<String, PaneFrame>,
    pane_bounds: HashMap<String, Bounds>,
    chat_pane_id: Option<String>,
    next_chat_index: u64,
    hotbar: Hotbar,
    hotbar_bounds: Bounds,
    model_dropdown: Dropdown,
    model_bounds: Bounds,
    model_hovered: bool,
    pending_model_changes: Rc<RefCell<Vec<String>>>,
    selected_model: String,
    copy_button: Button,
    copy_bounds: Bounds,
    pending_copy: Rc<RefCell<bool>>,
    new_chat_button: Button,
    new_chat_bounds: Bounds,
    pending_new_chat: Rc<RefCell<bool>>,
    keygen_button: Button,
    keygen_bounds: Bounds,
    pending_keygen: Rc<RefCell<bool>>,
    nostr_npub: Option<String>,
    nostr_nsec: Option<String>,
    spark_pubkey_hex: Option<String>,
    seed_phrase: Option<String>,
    nostr_error: Option<String>,
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
    queued_by_thread: HashMap<String, Vec<QueuedMessage>>,
    queued_in_flight: Option<String>,
    send_handler: Option<Box<dyn FnMut(UserAction)>>,
    stop_button: Button,
    stop_bounds: Bounds,
    pending_stop: Rc<RefCell<bool>>,
    session_id: Option<SessionId>,
    event_log: Vec<String>,
    event_log_dirty: bool,
    event_scroll: ScrollView,
    event_scroll_bounds: Bounds,
    thread_model: Option<String>,
    thread_id: Option<String>,
    active_turn_id: Option<String>,
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

impl MinimalRoot {
    pub fn new() -> Self {
        let pending_model_changes = Rc::new(RefCell::new(Vec::new()));
        let pending_models = pending_model_changes.clone();
        let model_dropdown = Dropdown::new(build_model_options())
            .selected(DEFAULT_MODEL_INDEX)
            .font_size(theme::font_size::SM)
            .padding(12.0, 6.0)
            .on_change(move |_, value| {
                pending_models.borrow_mut().push(value.to_string());
            });

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

        let pending_new_chat = Rc::new(RefCell::new(false));
        let pending_new_chat_click = pending_new_chat.clone();
        let new_chat_button = Button::new("New chat")
            .variant(ButtonVariant::Secondary)
            .font_size(theme::font_size::XS)
            .padding(12.0, 6.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_new_chat_click.borrow_mut() = true;
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

        let event_scroll = ScrollView::new().show_scrollbar(true).scrollbar_width(6.0);
        let hotbar = Hotbar::new()
            .item_size(HOTBAR_ITEM_SIZE)
            .padding(HOTBAR_PADDING)
            .gap(HOTBAR_ITEM_GAP)
            .corner_radius(8.0);
        let mut root = Self {
            event_context: EventContext::new(),
            cursor_position: Point::ZERO,
            screen_size: Size::new(1280.0, 720.0),
            pane_store: PaneStore::default(),
            pane_frames: HashMap::new(),
            pane_bounds: HashMap::new(),
            chat_pane_id: None,
            next_chat_index: 1,
            hotbar,
            hotbar_bounds: Bounds::ZERO,
            model_dropdown,
            model_bounds: Bounds::ZERO,
            model_hovered: false,
            pending_model_changes,
            selected_model: DEFAULT_THREAD_MODEL.to_string(),
            copy_button,
            copy_bounds: Bounds::ZERO,
            pending_copy,
            new_chat_button,
            new_chat_bounds: Bounds::ZERO,
            pending_new_chat,
            keygen_button,
            keygen_bounds: Bounds::ZERO,
            pending_keygen,
            nostr_npub: None,
            nostr_nsec: None,
            spark_pubkey_hex: None,
            seed_phrase: None,
            nostr_error: None,
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
            queued_by_thread: HashMap::new(),
            queued_in_flight: None,
            send_handler: None,
            stop_button,
            stop_bounds: Bounds::ZERO,
            pending_stop,
            session_id: None,
            event_log: Vec::new(),
            event_log_dirty: false,
            event_scroll,
            event_scroll_bounds: Bounds::ZERO,
            thread_model: Some(DEFAULT_THREAD_MODEL.to_string()),
            thread_id: None,
            active_turn_id: None,
        };

        let screen = Size::new(1280.0, 720.0);
        root.open_chat_pane(screen, true);
        root
    }

    pub fn apply_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::SessionStarted { session_id, .. } => {
                self.session_id = Some(session_id);
            }
            AppEvent::AppServerEvent { message } => {
                if let Ok(value) = serde_json::from_str::<Value>(&message) {
                    if let Some(method) = value.get("method").and_then(|m| m.as_str())
                        && method == "thread/started"
                    {
                        if let Some(thread_id) = value
                            .get("params")
                            .and_then(|params| params.get("threadId"))
                            .and_then(|id| id.as_str())
                            .or_else(|| {
                                value
                                    .get("params")
                                    .and_then(|params| params.get("thread"))
                                    .and_then(|thread| thread.get("id"))
                                    .and_then(|id| id.as_str())
                            })
                        {
                            self.thread_id = Some(thread_id.to_string());
                        }
                        if let Some(model) = value
                            .get("params")
                            .and_then(|params| params.get("model"))
                            .and_then(|m| m.as_str())
                            .or_else(|| {
                                value
                                    .get("params")
                                    .and_then(|params| params.get("thread"))
                                    .and_then(|thread| thread.get("model"))
                                    .and_then(|m| m.as_str())
                            })
                        {
                            self.thread_model = Some(model.to_string());
                            self.selected_model = model.to_string();
                            if let Some(index) = model_index(model) {
                                self.model_dropdown.set_selected(Some(index));
                            }
                        }
                    }

                    if let Some(method) = value.get("method").and_then(|m| m.as_str())
                        && method == "turn/started"
                    {
                        if let Some(turn_id) = value
                            .get("params")
                            .and_then(|params| params.get("turnId"))
                            .and_then(|id| id.as_str())
                            .or_else(|| {
                                value
                                    .get("params")
                                    .and_then(|params| params.get("turn"))
                                    .and_then(|turn| turn.get("id"))
                                    .and_then(|id| id.as_str())
                            })
                        {
                            self.active_turn_id = Some(turn_id.to_string());
                            self.queued_in_flight = None;
                            self.show_working_indicator();
                        }
                    }

                    if let Some(method) = value.get("method").and_then(|m| m.as_str())
                        && (method == "turn/completed"
                            || method == "turn/failed"
                            || method == "turn/aborted"
                            || method == "turn/interrupted")
                    {
                        let completed_turn = value
                            .get("params")
                            .and_then(|params| params.get("turnId"))
                            .and_then(|id| id.as_str())
                            .or_else(|| {
                                value
                                    .get("params")
                                    .and_then(|params| params.get("turn"))
                                    .and_then(|turn| turn.get("id"))
                                    .and_then(|id| id.as_str())
                            });
                        if completed_turn
                            .map(|id| self.active_turn_id.as_deref() == Some(id))
                            .unwrap_or(true)
                        {
                            self.active_turn_id = None;
                            self.flush_queue_if_idle();
                            self.clear_working_indicator();
                        }
                    }

                    if let Some(method) = value.get("method").and_then(|m| m.as_str())
                        && method == "fullauto/status"
                    {
                        if let Some(enabled) = value
                            .get("params")
                            .and_then(|params| params.get("enabled"))
                            .and_then(|value| value.as_bool())
                        {
                            self.full_auto_enabled = enabled;
                        }
                    }

                    if let Some(method) = value.get("method").and_then(|m| m.as_str())
                        && method == "fullauto/decision"
                    {
                        if let Some(action) = value
                            .get("params")
                            .and_then(|params| params.get("action"))
                            .and_then(|value| value.as_str())
                        {
                            if action != "continue" {
                                self.full_auto_enabled = false;
                            }
                        }
                    }

                    self.apply_formatted_event(&value);
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

    fn open_chat_pane(&mut self, screen: Size, reset_chat: bool) -> String {
        if let Some(existing) = self.chat_pane_id.take() {
            self.pane_store.remove_pane(&existing, true);
        }

        let id = format!("chat-{}", self.next_chat_index);
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
            title: "Chat".to_string(),
            rect: ensure_pane_visible(rect, screen),
            dismissable: true,
        };
        self.pane_store.add_pane(pane);
        self.chat_pane_id = Some(id.clone());

        if reset_chat {
            self.reset_chat_state();
        }

        id
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

    fn close_pane(&mut self, id: &str) {
        self.pane_store.remove_pane(id, true);
        self.pane_frames.remove(id);
        self.pane_bounds.remove(id);
        if self.chat_pane_id.as_deref() == Some(id) {
            self.chat_pane_id = None;
        }
    }

    fn handle_hotbar_slot(&mut self, slot: u8) -> bool {
        let screen = self.screen_size();
        match slot {
            1 => {
                if let Some(chat_id) = self.chat_pane_id.clone() {
                    if self.pane_store.is_active(&chat_id) {
                        self.close_pane(&chat_id);
                    } else {
                        self.pane_store.bring_to_front(&chat_id);
                    }
                } else {
                    self.open_chat_pane(screen, false);
                }
                true
            }
            2 => {
                self.toggle_events_pane(screen);
                true
            }
            3 => {
                self.toggle_identity_pane(screen);
                true
            }
            _ => false,
        }
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

    fn is_processing(&self) -> bool {
        self.active_turn_id.is_some() || self.queued_in_flight.is_some()
    }

    fn current_queue(&self) -> &[QueuedMessage] {
        let Some(thread_id) = self.thread_id.as_deref() else {
            return &[];
        };
        self.queued_by_thread
            .get(thread_id)
            .map(|queue| queue.as_slice())
            .unwrap_or(&[])
    }

    fn enqueue_message(&mut self, text: String) -> bool {
        let Some(thread_id) = self.thread_id.clone() else {
            return false;
        };
        let entry = QueuedMessage { text };
        self.queued_by_thread
            .entry(thread_id)
            .or_default()
            .push(entry);
        true
    }

    fn dispatch_message(&mut self, text: String) {
        let Some(session_id) = self.session_id else {
            return;
        };
        let Some(handler) = self.send_handler.as_mut() else {
            return;
        };
        handler(UserAction::Message {
            session_id,
            text,
            model: Some(self.selected_model.clone()),
        });
        if let Some(thread_id) = self.thread_id.clone() {
            self.queued_in_flight = Some(thread_id);
        }
    }

    fn flush_queue_if_idle(&mut self) {
        if self.is_processing() {
            return;
        }
        let Some(thread_id) = self.thread_id.clone() else {
            return;
        };
        let Some(queue) = self.queued_by_thread.get_mut(&thread_id) else {
            return;
        };
        if queue.is_empty() {
            return;
        }
        let next = queue.remove(0);
        self.dispatch_message(next.text);
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

        let mut stream = new_markdown_stream();
        stream.append(&format!("> {text}"));
        stream.complete();
        let view = message_markdown_view(stream.document().clone());
        self.formatted_thread
            .push_entry(ThreadEntry::new(ThreadEntryType::User, view));
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
        self.formatted_thread
            .push_entry(ThreadEntry::new(ThreadEntryType::Assistant, view));
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
        self.formatted_thread
            .push_entry(ThreadEntry::new(ThreadEntryType::Assistant, view));
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

    fn reset_chat_state(&mut self) {
        self.formatted_thread.clear();
        self.formatted_message_streams.clear();
        self.formatted_message_entries.clear();
        self.reasoning_entries.clear();
        self.tool_entries.clear();
        self.last_user_message = None;
        self.working_entry_index = None;
        self.event_log.clear();
        self.event_log_dirty = true;
        self.queued_by_thread.clear();
        self.queued_in_flight = None;
        self.active_turn_id = None;
        self.thread_id = None;
        self.thread_model = Some(self.selected_model.clone());
        self.full_auto_enabled = false;
        self.input.set_value("");
        self.input_needs_focus = true;
        self.submit_button
            .set_disabled(self.input.get_value().trim().is_empty());
    }

    fn ensure_reasoning_entry(&mut self, item_id: &str) -> usize {
        if let Some(entry) = self.reasoning_entries.get(item_id) {
            return entry.entry_index;
        }

        self.clear_working_indicator();
        let card = CodexReasoningCard::new(None, None);
        self.formatted_thread
            .push_entry(ThreadEntry::new(ThreadEntryType::Assistant, card));
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

    pub fn handle_input(&mut self, event: &InputEvent, _bounds: Bounds) -> bool {
        if let InputEvent::KeyDown { key, modifiers } = event {
            if matches!(key, Key::Named(NamedKey::Tab))
                && !modifiers.shift
                && !modifiers.ctrl
                && !modifiers.alt
                && !modifiers.meta
                && self.input.is_focused()
                && self.is_processing()
            {
                let value = self.input.get_value().trim().to_string();
                if !value.is_empty() && self.enqueue_message(value) {
                    self.input.set_value("");
                    self.submit_button.set_disabled(true);
                    return true;
                }
            }

            if modifiers.meta || modifiers.ctrl {
                if let Key::Character(value) = key {
                    if let Ok(slot) = value.parse::<u8>() {
                        if slot >= 1 && slot <= 9 && self.handle_hotbar_slot(slot) {
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
            self.input_hovered = self.input_bounds.contains(self.cursor_position);
            self.model_hovered = if SHOW_MODEL_DROPDOWN {
                self.model_bounds.contains(self.cursor_position)
            } else {
                false
            };
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
                            let dropdown_handled = if SHOW_MODEL_DROPDOWN {
                                matches!(
                                    self.model_dropdown.event(
                                        event,
                                        self.model_bounds,
                                        &mut self.event_context
                                    ),
                                    EventResult::Handled
                                )
                            } else {
                                false
                            };

                            let new_chat_handled = matches!(
                                self.new_chat_button.event(
                                    event,
                                    self.new_chat_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );

                            let full_auto_handled = matches!(
                                self.full_auto_button.event(
                                    event,
                                    self.full_auto_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );

                            let formatted_handled = if self
                                .formatted_thread_bounds
                                .contains(self.cursor_position)
                            {
                                matches!(
                                    self.formatted_thread.event(
                                        event,
                                        self.formatted_thread_bounds,
                                        &mut self.event_context
                                    ),
                                    EventResult::Handled
                                )
                            } else {
                                false
                            };

                            let input_handled = matches!(
                                self.input
                                    .event(event, self.input_bounds, &mut self.event_context),
                                EventResult::Handled
                            );

                            let submit_handled = matches!(
                                self.submit_button.event(
                                    event,
                                    self.submit_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );

                            let stop_handled = matches!(
                                self.stop_button.event(
                                    event,
                                    self.stop_bounds,
                                    &mut self.event_context
                                ),
                                EventResult::Handled
                            );

                            handled |= dropdown_handled
                                || new_chat_handled
                                || full_auto_handled
                                || formatted_handled
                                || input_handled
                                || submit_handled
                                || stop_handled;
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
                    }
                }
            }
        }

        let pending_models = {
            let mut pending = self.pending_model_changes.borrow_mut();
            let models = pending.clone();
            pending.clear();
            models
        };

        if let Some(model) = pending_models.last() {
            self.selected_model = model.clone();
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
            let _ = copy_to_clipboard(&block);
        }

        let should_new_chat = {
            let mut pending = self.pending_new_chat.borrow_mut();
            let value = *pending;
            *pending = false;
            value
        };

        if should_new_chat {
            let screen = self.screen_size();
            self.open_chat_pane(screen, true);
            if let (Some(session_id), Some(handler)) = (self.session_id, self.send_handler.as_mut())
            {
                handler(UserAction::NewChat {
                    session_id,
                    model: Some(self.selected_model.clone()),
                });
            }
            return true;
        }

        let should_stop = {
            let mut pending = self.pending_stop.borrow_mut();
            let value = *pending;
            *pending = false;
            value
        };

        if should_stop {
            if let (Some(session_id), Some(handler)) = (self.session_id, self.send_handler.as_mut())
            {
                handler(UserAction::Interrupt {
                    session_id,
                    thread_id: self.thread_id.clone(),
                    turn_id: self.active_turn_id.clone(),
                });
            }
            return true;
        }

        let should_toggle_full_auto = {
            let mut pending = self.pending_full_auto.borrow_mut();
            let value = *pending;
            *pending = false;
            value
        };

        if should_toggle_full_auto {
            let enabled = !self.full_auto_enabled;
            self.full_auto_enabled = enabled;
            if let (Some(session_id), Some(handler)) = (self.session_id, self.send_handler.as_mut())
            {
                handler(UserAction::FullAutoToggle {
                    session_id,
                    enabled,
                    thread_id: self.thread_id.clone(),
                    continue_prompt: None,
                });
            }
            return true;
        }

        let pending_messages = {
            let mut pending = self.pending_sends.borrow_mut();
            let messages = pending.clone();
            pending.clear();
            messages
        };

        if !pending_messages.is_empty() {
            let mut messages = Vec::new();
            for message in pending_messages {
                if message.trim().is_empty() {
                    let value = self.input.get_value().trim().to_string();
                    if !value.is_empty() {
                        messages.push(value);
                    }
                } else {
                    messages.push(message);
                }
            }

            if !messages.is_empty() {
                for message in messages {
                    self.dispatch_message(message);
                }
                self.input.set_value("");
                self.input.focus();
                self.submit_button
                    .set_disabled(self.input.get_value().trim().is_empty());
                return true;
            }
        }

        self.submit_button
            .set_disabled(self.input.get_value().trim().is_empty());
        self.stop_button
            .set_disabled(self.thread_id.is_none());

        handled
    }

    pub fn cursor(&self) -> Cursor {
        if self.hotbar.is_hovered() {
            Cursor::Pointer
        } else if self
            .pane_frames
            .values()
            .any(|frame| frame.is_close_hovered())
        {
            Cursor::Pointer
        } else if self.submit_button.is_hovered() && !self.submit_button.is_disabled() {
            Cursor::Pointer
        } else if self.stop_button.is_hovered() && !self.stop_button.is_disabled() {
            Cursor::Pointer
        } else if self.new_chat_button.is_hovered() && !self.new_chat_button.is_disabled() {
            Cursor::Pointer
        } else if self.keygen_button.is_hovered() {
            Cursor::Pointer
        } else if self.full_auto_button.is_hovered() && !self.full_auto_button.is_disabled() {
            Cursor::Pointer
        } else if self.copy_button.is_hovered() && !self.copy_button.is_disabled() {
            Cursor::Pointer
        } else if SHOW_MODEL_DROPDOWN && (self.model_hovered || self.model_dropdown.is_open()) {
            Cursor::Pointer
        } else if self.input_hovered || self.input.is_focused() {
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
                .with_background(theme::bg::APP)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        self.set_screen_size(Size::new(bounds.size.width, bounds.size.height));

        self.copy_bounds = Bounds::ZERO;
        self.event_scroll_bounds = Bounds::ZERO;
        self.keygen_bounds = Bounds::ZERO;
        self.new_chat_bounds = Bounds::ZERO;
        self.full_auto_bounds = Bounds::ZERO;
        self.input_bounds = Bounds::ZERO;
        self.submit_bounds = Bounds::ZERO;
        self.stop_bounds = Bounds::ZERO;

        self.pane_bounds.clear();

        let panes = self.pane_store.panes().to_vec();
        for pane in panes.iter() {
            let pane_bounds = Bounds::new(
                bounds.origin.x + pane.rect.x,
                bounds.origin.y + pane.rect.y,
                pane.rect.width,
                pane.rect.height,
            );
            self.pane_bounds.insert(pane.id.clone(), pane_bounds);

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
                PaneKind::Chat => paint_chat_pane(self, content_bounds, cx),
                PaneKind::Events => paint_events_pane(self, content_bounds, cx),
                PaneKind::Identity => paint_identity_pane(self, content_bounds, cx),
            }
        }

        let slot_count: usize = 9;
        let bar_width = HOTBAR_PADDING * 2.0
            + HOTBAR_ITEM_SIZE * slot_count as f32
            + HOTBAR_ITEM_GAP * (slot_count.saturating_sub(1) as f32);
        let bar_x = bounds.origin.x + (bounds.size.width - bar_width) * 0.5;
        let bar_y = bounds.origin.y + bounds.size.height - HOTBAR_FLOAT_GAP - HOTBAR_HEIGHT;
        let bar_bounds = Bounds::new(bar_x, bar_y, bar_width, HOTBAR_HEIGHT);
        self.hotbar_bounds = bar_bounds;

        let chat_active = self
            .chat_pane_id
            .as_ref()
            .map(|id| self.pane_store.is_active(id))
            .unwrap_or(false);

        let items = vec![
            HotbarSlot::new(1, "CH", "Chat").active(chat_active),
            HotbarSlot::new(2, "EV", "Events").active(self.pane_store.is_active("events")),
            HotbarSlot::new(3, "ID", "Identity").active(self.pane_store.is_active("identity")),
            HotbarSlot::new(4, "", "Slot 4").ghost(true),
            HotbarSlot::new(5, "", "Slot 5").ghost(true),
            HotbarSlot::new(6, "", "Slot 6").ghost(true),
            HotbarSlot::new(7, "", "Slot 7").ghost(true),
            HotbarSlot::new(8, "", "Slot 8").ghost(true),
            HotbarSlot::new(9, "", "Slot 9").ghost(true),
        ];
        self.hotbar.set_items(items);
        self.hotbar.paint(bar_bounds, cx);
    }
}

fn paint_chat_pane(root: &mut MinimalRoot, bounds: Bounds, cx: &mut PaintContext) {
    let padding_x = 24.0;
    let padding_top = 12.0;
    let padding_bottom = 16.0;
    let header_height = 28.0;
    let content_width = bounds.size.width - padding_x * 2.0;
    let header_bounds = Bounds::new(
        bounds.origin.x + padding_x,
        bounds.origin.y + padding_top,
        content_width,
        header_height,
    );

    let full_auto_label = if root.full_auto_enabled {
        "Full Auto On"
    } else {
        "Full Auto Off"
    };
    root.full_auto_button.set_label(full_auto_label);
    root.full_auto_button.set_variant(if root.full_auto_enabled {
        ButtonVariant::Primary
    } else {
        ButtonVariant::Secondary
    });
    root.full_auto_button
        .set_disabled(root.thread_id.is_none());
    let button_font = theme::font_size::XS;
    let full_auto_label_width =
        cx.text
            .measure_styled_mono(full_auto_label, button_font, FontStyle::default());
    let button_width = (full_auto_label_width + 28.0).max(110.0);
    let button_height = 24.0;
    let button_bounds = Bounds::new(
        header_bounds.origin.x + content_width - button_width,
        header_bounds.origin.y + (header_height - button_height) / 2.0,
        button_width,
        button_height,
    );
    root.full_auto_bounds = button_bounds;
    root.full_auto_button.paint(button_bounds, cx);

    root.new_chat_button
        .set_disabled(root.session_id.is_none());
    let new_chat_label = "New chat";
    let new_chat_label_width =
        cx.text
            .measure_styled_mono(new_chat_label, button_font, FontStyle::default());
    let new_chat_width = (new_chat_label_width + 28.0).max(96.0);
    let new_chat_bounds = Bounds::new(
        header_bounds.origin.x,
        header_bounds.origin.y + (header_height - button_height) / 2.0,
        new_chat_width,
        button_height,
    );
    root.new_chat_bounds = new_chat_bounds;
    root.new_chat_button.paint(new_chat_bounds, cx);

    let model = root.thread_model.as_deref().unwrap_or(DEFAULT_THREAD_MODEL);
    let thread_id = root.thread_id.as_deref().unwrap_or("unknown-thread");
    let thread_line_bounds = Bounds::new(
        header_bounds.origin.x,
        header_bounds.origin.y + header_height + 6.0,
        header_bounds.size.width,
        18.0,
    );
    Text::new(&format!("Initialized thread {thread_id}"))
        .font_size(theme::font_size::XS)
        .italic()
        .color(theme::text::MUTED)
        .paint(thread_line_bounds, cx);

    let model_line_bounds = Bounds::new(
        header_bounds.origin.x,
        thread_line_bounds.origin.y + 18.0,
        header_bounds.size.width,
        18.0,
    );
    Text::new(&format!("Model: {model}"))
        .font_size(theme::font_size::XS)
        .color(theme::text::MUTED)
        .paint(model_line_bounds, cx);

    let mut description_top = model_line_bounds.origin.y + 18.0;
    let mut dropdown_bounds = Bounds::ZERO;

    if SHOW_MODEL_DROPDOWN {
        let selector_height = 30.0;
        let selector_bounds = Bounds::new(
            header_bounds.origin.x,
            model_line_bounds.origin.y + 26.0,
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

        root.model_bounds = dropdown_bounds;
        description_top = selector_bounds.origin.y + selector_height + 6.0;
    } else {
        root.model_bounds = Bounds::ZERO;
    }

    if !root.current_queue().is_empty() {
        let mut queue_text = String::from("Queued");
        for item in root.current_queue() {
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

    let input_height = input_bar_height(root, bounds.size.width, cx);
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
    root.formatted_thread_bounds = feed_bounds;

    if root.formatted_thread.entry_count() != 0 {
        root.formatted_thread.paint(feed_bounds, cx);
    }

    // Paint dropdown last so the menu overlays the rest of the feed.
    if SHOW_MODEL_DROPDOWN {
        root.model_dropdown.paint(dropdown_bounds, cx);
    }

    paint_input_bar(root, input_bounds, cx);
}

struct InputBarMetrics {
    total_width: f32,
    input_width: f32,
    input_height: f32,
    send_width: f32,
    stop_width: f32,
}

fn input_bar_metrics(
    root: &mut MinimalRoot,
    available_width: f32,
    cx: &mut PaintContext,
) -> InputBarMetrics {
    let gap = 8.0;
    let padding_x = 24.0;
    let button_font = theme::font_size::SM;
    let send_label_width =
        cx.text
            .measure_styled_mono("Send", button_font, FontStyle::default());
    let send_width = (send_label_width + 28.0).max(72.0);
    let stop_label_width =
        cx.text
            .measure_styled_mono("Stop", button_font, FontStyle::default());
    let stop_width = (stop_label_width + 28.0).max(72.0);

    let mut total_width = (available_width * 0.6).min(720.0).max(320.0);
    total_width = total_width.min(available_width - padding_x * 2.0);
    let input_width = (total_width - send_width - stop_width - gap * 2.0).max(120.0);

    root.input.set_max_width(input_width);
    let line_height = button_font * 1.4;
    let padding_y = theme::spacing::XS;
    let min_height = line_height * INPUT_MIN_LINES as f32 + padding_y * 2.0;
    let mut input_height = root.input.current_height().max(min_height);
    if let Some(max_lines) = INPUT_MAX_LINES {
        let max_height = line_height * max_lines as f32 + padding_y * 2.0;
        input_height = input_height.min(max_height);
    }

    InputBarMetrics {
        total_width,
        input_width,
        input_height,
        send_width,
        stop_width,
    }
}

fn input_bar_height(root: &mut MinimalRoot, available_width: f32, cx: &mut PaintContext) -> f32 {
    let metrics = input_bar_metrics(root, available_width, cx);
    let padding_y = theme::spacing::MD;
    (metrics.input_height + padding_y * 2.0).max(BOTTOM_BAR_MIN_HEIGHT)
}

fn paint_input_bar(root: &mut MinimalRoot, bounds: Bounds, cx: &mut PaintContext) {
    let gap = 8.0;
    let padding_y = theme::spacing::MD;
    let metrics = input_bar_metrics(root, bounds.size.width, cx);

    let bar_x = bounds.origin.x + (bounds.size.width - metrics.total_width) / 2.0;
    let bar_y = bounds.origin.y + bounds.size.height - metrics.input_height - padding_y;
    let input_bounds = Bounds::new(bar_x, bar_y, metrics.input_width, metrics.input_height);
    let stop_bounds = Bounds::new(
        input_bounds.origin.x + input_bounds.size.width + gap,
        bar_y,
        metrics.stop_width,
        metrics.input_height,
    );
    let submit_bounds = Bounds::new(
        stop_bounds.origin.x + stop_bounds.size.width + gap,
        bar_y,
        metrics.send_width,
        metrics.input_height,
    );

    root.input_bounds = input_bounds;
    root.submit_bounds = submit_bounds;
    root.stop_bounds = stop_bounds;
    root.input.set_max_width(input_bounds.size.width);
    root.submit_button
        .set_disabled(root.input.get_value().trim().is_empty());
    root.stop_button
        .set_disabled(root.thread_id.is_none());
    if root.input_needs_focus {
        root.input.focus();
        root.input_needs_focus = false;
    }

    root.input.paint(input_bounds, cx);
    root.stop_button.paint(stop_bounds, cx);
    root.submit_button.paint(submit_bounds, cx);
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

        let mut seed_text =
            Text::new(root.seed_phrase.as_deref().unwrap_or("")).font_size(nostr_font);
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
        let mut seed_text = Text::new(root.seed_phrase.as_deref().unwrap_or(""))
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
                        AssistantMessage::new("Queued message for processing."),
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

        let terminal = TerminalToolCall::new("cargo build -p autopilot-desktop-wgpu")
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

fn format_event(event: &AppEvent) -> String {
    match event {
        AppEvent::WorkspaceOpened { path, .. } => {
            format!("WorkspaceOpened ({})", path.display())
        }
        AppEvent::SessionStarted { session_id, .. } => {
            format!("SessionStarted ({:?})", session_id)
        }
        AppEvent::UserActionDispatched { action, .. } => match action {
            UserAction::Message { text, model, .. } => {
                if let Some(model) = model {
                    format!("Message ({text}) [{model}]")
                } else {
                    format!("Message ({text})")
                }
            }
            UserAction::Command { name, .. } => format!("Command ({})", name),
            UserAction::NewChat { model, .. } => {
                if let Some(model) = model {
                    format!("NewChat [{model}]")
                } else {
                    "NewChat".to_string()
                }
            }
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
    }
}

fn format_session_id(session_id: SessionId) -> String {
    let raw = format!("{:?}", session_id);
    let trimmed = raw.trim_start_matches("SessionId(").trim_end_matches(')');
    trimmed.chars().take(6).collect()
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
