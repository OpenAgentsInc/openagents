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
use wgpui::{Bounds, Component, EventResult, PaintContext, Point, Quad, theme};

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
    thread_view: ThreadView,
    message_editor: MessageEditor,
    pending_actions: Rc<RefCell<Vec<UiAction>>>,
    send_handler: Option<Box<dyn FnMut(UserAction)>>,
    cursor_position: Point,
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
            thread_view: ThreadView::new().auto_scroll(true).item_spacing(12.0),
            message_editor,
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
            if layout.thread_bounds.contains(self.cursor_position) {
                handled |= matches!(
                    self.thread_view
                        .event(event, layout.thread_bounds, &mut self.event_context),
                    EventResult::Handled
                );
            }
            self.flush_ui_actions();
            return handled;
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
        paint_session_list(cx, &self.session_rows, layout.left_list_bounds);

        self.center_header.paint(layout.center_header_bounds, cx);
        paint_divider(cx, layout.center_header_bounds);
        paint_panel_inset(cx, layout.thread_bounds);
        self.thread_view.paint(layout.thread_bounds, cx);
        paint_composer_backdrop(cx, layout.editor_bounds);
        self.message_editor.paint(layout.editor_bounds, cx);

        self.right_header.paint(layout.right_header_bounds, cx);
        paint_status_pills(cx, layout.right_header_bounds);
        paint_divider(cx, layout.right_header_bounds);
        paint_status_sections(cx, layout.right_body_bounds, self.status_sections());

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
    let count_width = badge_width(&count_label);
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

    x -= badge_gap + badge_width("NEW");
    paint_badge(
        cx,
        "NEW",
        Bounds::new(
            x,
            header_bounds.origin.y + (header_bounds.size.height - badge_height) / 2.0,
            badge_width("NEW"),
            badge_height,
        ),
        false,
    );
}

fn badge_width(label: &str) -> f32 {
    let padding = 10.0;
    let char_width = theme::font_size::SM * 0.6;
    (label.len() as f32 * char_width + padding * 2.0).max(28.0)
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
    let mut y = bounds.origin.y;
    let id_column_width = 78.0;

    for row in rows {
        let row_bounds = Bounds::new(bounds.origin.x, y, bounds.size.width, SESSION_ROW_HEIGHT);
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
            row_bounds.size.width - id_column_width - 12.0,
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

        y += SESSION_ROW_HEIGHT;
        if y > bounds.origin.y + bounds.size.height {
            break;
        }
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
        let width = badge_width(label);
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

fn paint_status_sections(cx: &mut PaintContext, bounds: Bounds, sections: Vec<StatusSectionData>) {
    let mut y = bounds.origin.y;
    let label_width = 110.0;

    for section in sections {
        let mut header = Text::new(section.title)
            .font_size(theme::font_size::BASE)
            .bold()
            .color(theme::accent::PRIMARY)
            .no_wrap();
        header.paint(
            Bounds::new(bounds.origin.x, y, bounds.size.width, STATUS_LINE_HEIGHT),
            cx,
        );
        y += STATUS_LINE_HEIGHT + 2.0;

        for line in section.lines {
            let label_bounds = Bounds::new(bounds.origin.x, y, label_width, STATUS_LINE_HEIGHT);
            let value_bounds = Bounds::new(
                bounds.origin.x + label_width + 6.0,
                y,
                bounds.size.width - label_width - 6.0,
                STATUS_LINE_HEIGHT,
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

            y += STATUS_LINE_HEIGHT;
        }

        if !section.actions.is_empty() {
            y += 4.0;
            let mut x = bounds.origin.x;
            for action in section.actions {
                let width = badge_width(action.label);
                paint_badge(
                    cx,
                    action.label,
                    Bounds::new(x, y, width, 16.0),
                    action.active,
                );
                x += width + 6.0;
            }
            y += 18.0;
        }

        y += STATUS_SECTION_GAP;
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
        let width = text.len() as f32 * theme::font_size::SM * 0.55 + 18.0;
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
        let command_bar_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + bounds.size.height - COMMAND_BAR_HEIGHT,
            bounds.size.width,
            COMMAND_BAR_HEIGHT,
        );

        let content_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            bounds.size.height - COMMAND_BAR_HEIGHT,
        );

        let left_panel_bounds = Bounds::new(
            content_bounds.origin.x,
            content_bounds.origin.y,
            LEFT_PANEL_WIDTH.min(content_bounds.size.width),
            content_bounds.size.height,
        );

        let right_panel_bounds = Bounds::new(
            content_bounds.origin.x + content_bounds.size.width - RIGHT_PANEL_WIDTH,
            content_bounds.origin.y,
            RIGHT_PANEL_WIDTH.min(content_bounds.size.width),
            content_bounds.size.height,
        );

        let center_panel_bounds = Bounds::new(
            left_panel_bounds.origin.x + left_panel_bounds.size.width + PANEL_GAP,
            content_bounds.origin.y,
            (content_bounds.size.width
                - left_panel_bounds.size.width
                - right_panel_bounds.size.width
                - PANEL_GAP * 2.0)
                .max(0.0),
            content_bounds.size.height,
        );

        let left_inner = inset(left_panel_bounds, PANEL_PADDING);
        let center_inner = inset(center_panel_bounds, PANEL_PADDING);
        let right_inner = inset(right_panel_bounds, PANEL_PADDING);

        let left_header_bounds = Bounds::new(
            left_inner.origin.x,
            left_inner.origin.y,
            left_inner.size.width,
            PANEL_HEADER_HEIGHT,
        );
        let left_list_bounds = Bounds::new(
            left_inner.origin.x,
            left_inner.origin.y + PANEL_HEADER_HEIGHT + 6.0,
            left_inner.size.width,
            (left_inner.size.height - PANEL_HEADER_HEIGHT - 6.0).max(0.0),
        );

        let center_header_bounds = Bounds::new(
            center_inner.origin.x,
            center_inner.origin.y,
            center_inner.size.width,
            PANEL_HEADER_HEIGHT,
        );

        let editor_bounds = Bounds::new(
            center_inner.origin.x,
            center_inner.origin.y + center_inner.size.height - COMPOSER_HEIGHT,
            center_inner.size.width,
            COMPOSER_HEIGHT,
        );

        let thread_bounds = Bounds::new(
            center_inner.origin.x,
            center_inner.origin.y + PANEL_HEADER_HEIGHT + 6.0,
            center_inner.size.width,
            (editor_bounds.origin.y - center_inner.origin.y - PANEL_HEADER_HEIGHT - 12.0).max(0.0),
        );

        let right_header_bounds = Bounds::new(
            right_inner.origin.x,
            right_inner.origin.y,
            right_inner.size.width,
            PANEL_HEADER_HEIGHT,
        );

        let right_body_bounds = Bounds::new(
            right_inner.origin.x,
            right_inner.origin.y + PANEL_HEADER_HEIGHT + 6.0,
            right_inner.size.width,
            (right_inner.size.height - PANEL_HEADER_HEIGHT - 6.0).max(0.0),
        );

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

fn inset(bounds: Bounds, padding: f32) -> Bounds {
    Bounds::new(
        bounds.origin.x + padding,
        bounds.origin.y + padding,
        (bounds.size.width - padding * 2.0).max(0.0),
        (bounds.size.height - padding * 2.0).max(0.0),
    )
}

fn paint_panel(cx: &mut PaintContext, bounds: Bounds) {
    let panel = Quad::new(bounds)
        .with_background(theme::bg::MUTED)
        .with_border(theme::border::DEFAULT, 1.0)
        .with_corner_radius(6.0);
    cx.scene.draw_quad(panel);
}
