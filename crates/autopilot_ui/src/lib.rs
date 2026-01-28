use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use autopilot_app::{AppEvent, SessionId, UserAction, WorkspaceId};
use wgpui::components::atoms::{ToolStatus, ToolType};
use wgpui::components::EventContext;
use wgpui::components::organisms::{
    AssistantMessage, DiffLine, DiffLineKind, DiffToolCall, SearchMatch, SearchToolCall,
    TerminalToolCall, ThreadEntry, ThreadEntryType, ToolCallCard, UserMessage,
};
use wgpui::components::sections::{MessageEditor, ThreadView};
use wgpui::components::Text;
use wgpui::input::InputEvent;
use wgpui::{
    Bounds, Button, Component, EventResult, Hsla, LayoutEngine, LayoutStyle, PaintContext, Point,
    Quad, ScrollView, Size, text::FontStyle, theme, length, px,
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
            AppEvent::SessionStarted { session_id, label, .. } => {
                self.session_id = Some(*session_id);
                self.session_label = label.clone();
                self.sessions.push(SessionSummary {
                    session_id: *session_id,
                    label: label.clone(),
                });
            }
            AppEvent::UserActionDispatched { .. } => {}
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

#[derive(Clone, Debug)]
enum UiAction {
    SendMessage(String),
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
    title: &'static str,
    subtitle: &'static str,
    button_label: &'static str,
}

impl MinimalRoot {
    pub fn new() -> Self {
        Self {
            title: "Autopilot Desktop",
            subtitle: "WGPUI minimal shell is live.",
            button_label: "Continue",
        }
    }

    pub fn apply_event(&mut self, _event: AppEvent) {}

    pub fn set_send_handler<F>(&mut self, _handler: F)
    where
        F: FnMut(UserAction) + 'static,
    {
    }

    pub fn handle_input(&mut self, _event: &InputEvent, _bounds: Bounds) -> bool {
        false
    }
}

impl Default for MinimalRoot {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for MinimalRoot {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let app_bg = Hsla::from_hex(0x0A0A0A);
        let card_bg = Hsla::from_hex(0x1A1A1A);
        let border = Hsla::from_hex(0xFFFFFF).with_alpha(0.1);
        let text_primary = Hsla::from_hex(0xCCCCCC);
        let text_muted = Hsla::from_hex(0x888888);
        let button_bg = Hsla::from_hex(0xCCCCCC);
        let button_text = Hsla::from_hex(0x0A0A0A);

        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(app_bg)
                .with_border(border, 1.0),
        );

        let padding = 24.0;
        let max_width = (bounds.size.width - padding * 2.0).max(160.0);
        let max_height = (bounds.size.height - padding * 2.0).max(120.0);
        let card_width = max_width.min(520.0);
        let card_height = max_height.min(220.0);

        let card_x = bounds.origin.x + (bounds.size.width - card_width) / 2.0;
        let card_y = bounds.origin.y + (bounds.size.height - card_height) / 2.0;
        let card_bounds = Bounds::new(card_x, card_y, card_width, card_height);

        cx.scene.draw_quad(
            Quad::new(card_bounds)
                .with_background(card_bg)
                .with_border(border, 1.0)
                .with_corner_radius(12.0),
        );

        let title_bounds = Bounds::new(
            card_bounds.origin.x + 20.0,
            card_bounds.origin.y + 18.0,
            card_bounds.size.width - 40.0,
            30.0,
        );
        Text::new(self.title)
            .font_size(theme::font_size::XL)
            .bold()
            .color(text_primary)
            .paint(title_bounds, cx);

        let subtitle_bounds = Bounds::new(
            card_bounds.origin.x + 20.0,
            card_bounds.origin.y + 52.0,
            card_bounds.size.width - 40.0,
            24.0,
        );
        Text::new(self.subtitle)
            .font_size(theme::font_size::BASE)
            .color(text_muted)
            .paint(subtitle_bounds, cx);

        let button_font = theme::font_size::SM;
        let label_width =
            cx.text
                .measure_styled_mono(self.button_label, button_font, FontStyle::default());
        let button_padding_x = 16.0;
        let button_width = (label_width + button_padding_x * 2.0).max(96.0);
        let button_height = 36.0;
        let button_bounds = Bounds::new(
            card_bounds.origin.x + 20.0,
            card_bounds.origin.y + card_bounds.size.height - button_height - 18.0,
            button_width,
            button_height,
        );

        let mut button = Button::new(self.button_label)
            .font_size(button_font)
            .padding(16.0, 8.0)
            .corner_radius(8.0)
            .background(button_bg)
            .text_color(button_text);
        button.paint(button_bounds, cx);
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
                let label = label.clone().unwrap_or_else(|| "Session started".to_string());
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
            }
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
            pending_send
                .borrow_mut()
                .push(UiAction::SendMessage(value));
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

        let id_bounds = Bounds::new(row_bounds.origin.x + 6.0, row_bounds.origin.y, id_column_width, row_bounds.size.height);
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
        Quad::new(Bounds::new(
            bounds.origin.x,
            y,
            bounds.size.width,
            1.0,
        ))
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
        rows.push(StatusRow::Header { title: section.title });
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
            UserAction::Message { text, .. } => format!("Message ({})", text),
            UserAction::Command { name, .. } => format!("Command ({})", name),
        },
    }
}

fn format_session_id(session_id: SessionId) -> String {
    let raw = format!("{:?}", session_id);
    let trimmed = raw
        .trim_start_matches("SessionId(")
        .trim_end_matches(')');
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

        let center_header = engine.request_leaf(&LayoutStyle::new().height(px(PANEL_HEADER_HEIGHT)));
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

        let content_row_style = LayoutStyle::new()
            .flex_row()
            .gap(panel_gap)
            .flex_grow(1.0);
        let content_row = engine.request_layout(
            &content_row_style,
            &[left_panel, center_panel, right_panel],
        );

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
        let style = LayoutStyle::new()
            .height(px(*height))
            .flex_shrink(0.0);
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
