use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use autopilot_app::{AppEvent, SessionId, UserAction, WorkspaceId};
use wgpui::components::atoms::{Mode, Model, ToolStatus, ToolType, TrajectoryStatus};
use wgpui::components::EventContext;
use wgpui::components::molecules::SessionSearchBar;
use wgpui::components::organisms::{
    AssistantMessage, DiffLine, DiffLineKind, DiffToolCall, SearchMatch, SearchToolCall,
    TerminalToolCall, ThreadControls, ThreadEntry, ThreadEntryType, ToolCallCard, UserMessage,
};
use wgpui::components::sections::{MessageEditor, ThreadView, TrajectoryEntry, TrajectoryView};
use wgpui::components::Text;
use wgpui::input::InputEvent;
use wgpui::{Bounds, Component, EventResult, PaintContext, Point, Quad, theme};

const STATUS_HEIGHT: f32 = 120.0;
const HEADER_HEIGHT: f32 = 48.0;
const LIST_HEADER_HEIGHT: f32 = 24.0;
const LIST_ROW_HEIGHT: f32 = 28.0;
const LIST_ROW_HEIGHT_LARGE: f32 = 32.0;
const PANEL_INSET: f32 = 24.0;
const COLUMN_GAP: f32 = 32.0;
const PANEL_GAP: f32 = 24.0;
const SEARCH_HEIGHT: f32 = 44.0;
const CONTROL_HEIGHT: f32 = 44.0;
const COMPOSER_HEIGHT: f32 = 90.0;
const CONVERSATION_HEADER_HEIGHT: f32 = 28.0;
const TRAJECTORY_HEADER_HEIGHT: f32 = 24.0;
const SECTION_GAP: f32 = 12.0;

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
    Run,
    Stop,
    ModeChanged(Mode),
    ModelChanged(Model),
    SearchUpdated,
}

pub struct DesktopRoot {
    view_model: AppViewModel,
    event_context: EventContext,
    header: Text,
    status: Text,
    session_search: SessionSearchBar,
    session_title: Text,
    event_title: Text,
    session_items: Vec<Text>,
    event_items: Vec<Text>,
    conversation_title: Text,
    trajectory_title: Text,
    thread_controls: ThreadControls,
    thread_view: ThreadView,
    message_editor: MessageEditor,
    trajectory_view: TrajectoryView,
    pending_actions: Rc<RefCell<Vec<UiAction>>>,
    send_handler: Option<Box<dyn FnMut(UserAction)>>,
    cursor_position: Point,
    run_state: bool,
    current_mode: Mode,
    current_model: Model,
}

impl DesktopRoot {
    pub fn new() -> Self {
        let pending_actions: Rc<RefCell<Vec<UiAction>>> = Rc::new(RefCell::new(Vec::new()));
        let message_editor = build_message_editor(pending_actions.clone());
        let session_search = build_session_search(pending_actions.clone());
        let thread_controls =
            build_thread_controls(pending_actions.clone(), Mode::Normal, Model::CodexSonnet, false);

        let mut root = Self {
            view_model: AppViewModel::default(),
            event_context: EventContext::new(),
            header: Text::new("Autopilot Desktop (WGPUI)")
                .font_size(30.0)
                .bold()
                .color(theme::text::PRIMARY),
            status: Text::new("Waiting for events...")
                .font_size(16.0)
                .color(theme::text::MUTED),
            session_search,
            session_title: Text::new("Sessions")
                .font_size(18.0)
                .bold()
                .color(theme::text::PRIMARY),
            event_title: Text::new("Event Log")
                .font_size(18.0)
                .bold()
                .color(theme::text::PRIMARY),
            session_items: Vec::new(),
            event_items: Vec::new(),
            conversation_title: Text::new("Conversation")
                .font_size(18.0)
                .bold()
                .color(theme::text::PRIMARY),
            trajectory_title: Text::new("Plan / Trajectory")
                .font_size(18.0)
                .bold()
                .color(theme::text::PRIMARY),
            thread_controls,
            thread_view: ThreadView::new().auto_scroll(true),
            message_editor,
            trajectory_view: TrajectoryView::new().auto_scroll(true),
            pending_actions,
            send_handler: None,
            cursor_position: Point::ZERO,
            run_state: false,
            current_mode: Mode::Normal,
            current_model: Model::CodexSonnet,
        };
        root.refresh_text();
        root
    }

    pub fn apply_event(&mut self, event: AppEvent) {
        self.view_model.apply_event(&event);
        self.update_thread_view(&event);
        self.update_trajectory_view(&event);
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
            handled |= self.route_scroll(event, &layout);
            self.flush_ui_actions();
            return handled;
        }

        handled |= matches!(
            self.session_search
                .event(event, layout.search_bounds, &mut self.event_context),
            EventResult::Handled
        );

        handled |= matches!(
            self.thread_controls
                .event(event, layout.controls_bounds, &mut self.event_context),
            EventResult::Handled
        );

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

        handled |= matches!(
            self.trajectory_view
                .event(event, layout.trajectory_bounds, &mut self.event_context),
            EventResult::Handled
        );

        self.flush_ui_actions();
        handled
    }

    fn refresh_text(&mut self) {
        let workspace_line = self
            .view_model
            .workspace_path
            .as_ref()
            .map(|path| format!("Workspace: {}", path.display()))
            .unwrap_or_else(|| "Workspace: --".to_string());

        let session_line = match (self.view_model.session_id, &self.view_model.session_label) {
            (Some(id), Some(label)) => format!("Session: {:?} ({})", id, label),
            (Some(id), None) => format!("Session: {:?}", id),
            _ => "Session: --".to_string(),
        };

        let last_event_line = self
            .view_model
            .last_event
            .clone()
            .unwrap_or_else(|| "Last event: --".to_string());

        let count_line = format!("Event count: {}", self.view_model.event_count);
        let run_line = if self.run_state {
            "Run state: Running".to_string()
        } else {
            "Run state: Idle".to_string()
        };
        let mode_line = format!(
            "Mode: {:?} | Model: {:?}",
            self.current_mode, self.current_model
        );

        let body = format!(
            "Immediate-mode view model (Zed/GPUI-style)\n{workspace}\n{session}\n{last_event}\n{count}\n{run}\n{mode}",
            workspace = workspace_line,
            session = session_line,
            last_event = last_event_line,
            count = count_line,
            run = run_line,
            mode = mode_line
        );

        self.status.set_content(body);

        let filter = self
            .session_search
            .search_value()
            .trim()
            .to_lowercase();

        self.session_items = self
            .view_model
            .sessions
            .iter()
            .enumerate()
            .filter(|(_, session)| {
                if filter.is_empty() {
                    true
                } else {
                    let label = session
                        .label
                        .as_deref()
                        .unwrap_or("Session")
                        .to_lowercase();
                    label.contains(&filter)
                        || format!("{:?}", session.session_id)
                            .to_lowercase()
                            .contains(&filter)
                }
            })
            .map(|(index, session)| {
                let label = session
                    .label
                    .as_ref()
                    .map(|label| format!("{label} ({:?})", session.session_id))
                    .unwrap_or_else(|| format!("Session {:?}", session.session_id));
                Text::new(format!("{}. {}", index + 1, label))
                    .font_size(14.0)
                    .color(theme::text::PRIMARY)
            })
            .collect();

        self.event_items = self
            .view_model
            .event_log
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                Text::new(format!("{}. {}", index + 1, entry))
                    .font_size(13.0)
                    .color(theme::text::MUTED)
            })
            .collect();
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

    fn update_trajectory_view(&mut self, event: &AppEvent) {
        match event {
            AppEvent::WorkspaceOpened { path, .. } => {
                self.trajectory_view.push_entry(
                    TrajectoryEntry::new("Open workspace")
                        .detail(path.display().to_string())
                        .status(TrajectoryStatus::Verified),
                );
            }
            AppEvent::SessionStarted { label, .. } => {
                let detail = label.clone().unwrap_or_else(|| "Bootstrap".to_string());
                self.trajectory_view.push_entry(
                    TrajectoryEntry::new("Start session")
                        .detail(detail)
                        .status(TrajectoryStatus::Partial),
                );
            }
            AppEvent::UserActionDispatched { action, .. } => match action {
                UserAction::Message { text, .. } => {
                    self.trajectory_view.push_entry(
                        TrajectoryEntry::new("Dispatch message")
                            .detail(text.clone())
                            .status(TrajectoryStatus::Verified),
                    );
                }
                UserAction::Command { name, .. } => {
                    self.trajectory_view.push_entry(
                        TrajectoryEntry::new("Dispatch command")
                            .detail(name.clone())
                            .status(TrajectoryStatus::Verified),
                    );
                }
            },
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

    fn route_scroll(&mut self, event: &InputEvent, layout: &Layout) -> bool {
        let cursor = self.cursor_position;
        if layout.thread_bounds.contains(cursor) {
            return matches!(
                self.thread_view
                    .event(event, layout.thread_bounds, &mut self.event_context),
                EventResult::Handled
            );
        }

        if layout.trajectory_bounds.contains(cursor) {
            return matches!(
                self.trajectory_view
                    .event(event, layout.trajectory_bounds, &mut self.event_context),
                EventResult::Handled
            );
        }

        false
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
                UiAction::Run => {
                    self.run_state = true;
                    self.rebuild_thread_controls();
                    self.refresh_text();
                }
                UiAction::Stop => {
                    self.run_state = false;
                    self.rebuild_thread_controls();
                    self.refresh_text();
                }
                UiAction::ModeChanged(mode) => {
                    self.current_mode = mode;
                    self.refresh_text();
                }
                UiAction::ModelChanged(model) => {
                    self.current_model = model;
                    self.refresh_text();
                }
                UiAction::SearchUpdated => {
                    self.refresh_text();
                }
            }
        }
    }

    fn rebuild_thread_controls(&mut self) {
        let mode = self.thread_controls.current_mode();
        let model = self.thread_controls.current_model();
        let running = self.run_state;
        self.thread_controls =
            build_thread_controls(self.pending_actions.clone(), mode, model, running);
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

        self.header.paint(layout.header_bounds, cx);
        paint_panel(cx, layout.left_panel_bounds);
        paint_panel(cx, layout.right_panel_bounds);

        self.status.paint(layout.status_bounds, cx);
        self.session_search.paint(layout.search_bounds, cx);
        self.session_title.paint(layout.sessions_header_bounds, cx);
        self.event_title.paint(layout.events_header_bounds, cx);
        paint_list(cx, &mut self.session_items, layout.sessions_bounds, LIST_ROW_HEIGHT_LARGE);
        paint_list(cx, &mut self.event_items, layout.events_bounds, LIST_ROW_HEIGHT);
        self.trajectory_title
            .paint(layout.trajectory_header_bounds, cx);
        self.trajectory_view.paint(layout.trajectory_bounds, cx);

        self.thread_controls.paint(layout.controls_bounds, cx);
        self.conversation_title
            .paint(layout.conversation_header_bounds, cx);
        self.thread_view.paint(layout.thread_bounds, cx);
        self.message_editor.paint(layout.editor_bounds, cx);
    }
}

fn build_message_editor(pending: Rc<RefCell<Vec<UiAction>>>) -> MessageEditor {
    let pending_send = pending.clone();
    MessageEditor::new()
        .mode(Mode::Normal)
        .show_mode_badge(false)
        .show_keybinding_hint(true)
        .on_send(move |value| {
            pending_send
                .borrow_mut()
                .push(UiAction::SendMessage(value));
        })
}

fn build_session_search(pending: Rc<RefCell<Vec<UiAction>>>) -> SessionSearchBar {
    let pending_search = pending.clone();
    SessionSearchBar::new().on_search(move |_value| {
        pending_search.borrow_mut().push(UiAction::SearchUpdated);
    })
}

fn build_thread_controls(
    pending: Rc<RefCell<Vec<UiAction>>>,
    mode: Mode,
    model: Model,
    running: bool,
) -> ThreadControls {
    let pending_run = pending.clone();
    let pending_stop = pending.clone();
    let pending_mode = pending.clone();
    let pending_model = pending.clone();

    ThreadControls::new()
        .mode(mode)
        .model(model)
        .running(running)
        .on_run(move || pending_run.borrow_mut().push(UiAction::Run))
        .on_stop(move || pending_stop.borrow_mut().push(UiAction::Stop))
        .on_mode_change(move |mode| pending_mode.borrow_mut().push(UiAction::ModeChanged(mode)))
        .on_model_change(move |model| pending_model.borrow_mut().push(UiAction::ModelChanged(model)))
}

fn paint_list(cx: &mut PaintContext, items: &mut [Text], bounds: Bounds, row_height: f32) {
    let mut y = bounds.origin.y;
    for item in items {
        let row_bounds = Bounds::new(bounds.origin.x, y, bounds.size.width, row_height);
        item.paint(row_bounds, cx);
        y += row_height;
        if y > bounds.origin.y + bounds.size.height {
            break;
        }
    }
}

#[derive(Clone, Debug)]
struct SessionSummary {
    session_id: SessionId,
    label: Option<String>,
}

fn format_event(event: &AppEvent) -> String {
    match event {
        AppEvent::WorkspaceOpened { path, .. } => {
            format!("Last event: WorkspaceOpened ({})", path.display())
        }
        AppEvent::SessionStarted { session_id, .. } => {
            format!("Last event: SessionStarted ({:?})", session_id)
        }
        AppEvent::UserActionDispatched { action, .. } => match action {
            UserAction::Message { text, .. } => format!("Last event: Message ({})", text),
            UserAction::Command { name, .. } => format!("Last event: Command ({})", name),
        },
    }
}

struct Layout {
    header_bounds: Bounds,
    left_panel_bounds: Bounds,
    right_panel_bounds: Bounds,
    status_bounds: Bounds,
    search_bounds: Bounds,
    sessions_bounds: Bounds,
    events_bounds: Bounds,
    sessions_header_bounds: Bounds,
    events_header_bounds: Bounds,
    trajectory_header_bounds: Bounds,
    trajectory_bounds: Bounds,
    controls_bounds: Bounds,
    conversation_header_bounds: Bounds,
    thread_bounds: Bounds,
    editor_bounds: Bounds,
}

impl Layout {
    fn new(bounds: Bounds) -> Self {
        let header_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            HEADER_HEIGHT,
        );

        let body_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + HEADER_HEIGHT + 16.0,
            bounds.size.width,
            bounds.size.height - HEADER_HEIGHT - 16.0,
        );

        let left_width = (body_bounds.size.width * 0.48).max(0.0);
        let right_width = (body_bounds.size.width - left_width - PANEL_GAP).max(0.0);

        let left_panel_bounds = Bounds::new(
            body_bounds.origin.x,
            body_bounds.origin.y,
            left_width,
            body_bounds.size.height,
        );

        let right_panel_bounds = Bounds::new(
            body_bounds.origin.x + left_width + PANEL_GAP,
            body_bounds.origin.y,
            right_width,
            body_bounds.size.height,
        );

        let left_inner = inset(left_panel_bounds, PANEL_INSET);
        let right_inner = inset(right_panel_bounds, PANEL_INSET);

        let status_bounds = Bounds::new(
            left_inner.origin.x,
            left_inner.origin.y,
            left_inner.size.width,
            STATUS_HEIGHT,
        );

        let search_bounds = Bounds::new(
            left_inner.origin.x,
            status_bounds.origin.y + STATUS_HEIGHT + SECTION_GAP,
            left_inner.size.width,
            SEARCH_HEIGHT,
        );

        let remaining_height = left_inner.size.height
            - STATUS_HEIGHT
            - SEARCH_HEIGHT
            - SECTION_GAP * 2.0;
        let list_height = (remaining_height * 0.55).max(140.0).min(remaining_height.max(0.0));
        let trajectory_height = (remaining_height - list_height - SECTION_GAP).max(0.0);

        let list_top = search_bounds.origin.y + SEARCH_HEIGHT + SECTION_GAP;
        let column_width = (left_inner.size.width - COLUMN_GAP) * 0.5;

        let sessions_header_bounds = Bounds::new(
            left_inner.origin.x,
            list_top,
            column_width.max(0.0),
            LIST_HEADER_HEIGHT,
        );
        let events_header_bounds = Bounds::new(
            left_inner.origin.x + column_width + COLUMN_GAP,
            list_top,
            column_width.max(0.0),
            LIST_HEADER_HEIGHT,
        );

        let sessions_bounds = Bounds::new(
            sessions_header_bounds.origin.x,
            sessions_header_bounds.origin.y + LIST_HEADER_HEIGHT,
            sessions_header_bounds.size.width,
            (list_height - LIST_HEADER_HEIGHT).max(0.0),
        );
        let events_bounds = Bounds::new(
            events_header_bounds.origin.x,
            events_header_bounds.origin.y + LIST_HEADER_HEIGHT,
            events_header_bounds.size.width,
            (list_height - LIST_HEADER_HEIGHT).max(0.0),
        );

        let trajectory_header_bounds = Bounds::new(
            left_inner.origin.x,
            list_top + list_height + SECTION_GAP,
            left_inner.size.width,
            TRAJECTORY_HEADER_HEIGHT,
        );
        let trajectory_bounds = Bounds::new(
            left_inner.origin.x,
            trajectory_header_bounds.origin.y + TRAJECTORY_HEADER_HEIGHT,
            left_inner.size.width,
            (trajectory_height - TRAJECTORY_HEADER_HEIGHT).max(0.0),
        );

        let controls_bounds = Bounds::new(
            right_inner.origin.x,
            right_inner.origin.y,
            right_inner.size.width,
            CONTROL_HEIGHT,
        );

        let conversation_header_bounds = Bounds::new(
            right_inner.origin.x,
            controls_bounds.origin.y + CONTROL_HEIGHT + SECTION_GAP,
            right_inner.size.width,
            CONVERSATION_HEADER_HEIGHT,
        );

        let editor_bounds = Bounds::new(
            right_inner.origin.x,
            right_inner.origin.y + right_inner.size.height - COMPOSER_HEIGHT,
            right_inner.size.width,
            COMPOSER_HEIGHT,
        );

        let thread_top =
            conversation_header_bounds.origin.y + CONVERSATION_HEADER_HEIGHT + SECTION_GAP;
        let thread_bounds = Bounds::new(
            right_inner.origin.x,
            thread_top,
            right_inner.size.width,
            (editor_bounds.origin.y - thread_top - SECTION_GAP).max(0.0),
        );

        Self {
            header_bounds,
            left_panel_bounds,
            right_panel_bounds,
            status_bounds,
            search_bounds,
            sessions_bounds,
            events_bounds,
            sessions_header_bounds,
            events_header_bounds,
            trajectory_header_bounds,
            trajectory_bounds,
            controls_bounds,
            conversation_header_bounds,
            thread_bounds,
            editor_bounds,
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
        .with_background(theme::bg::SURFACE)
        .with_corner_radius(14.0);
    cx.scene.draw_quad(panel);
}
