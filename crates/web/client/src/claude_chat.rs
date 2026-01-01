use std::cell::RefCell;
use std::rc::Rc;

use wgpui::{
    Bounds, Button, ButtonVariant, Cursor, EventContext, EventResult, InputEvent, PaintContext,
    Point, Quad, TextInput, theme,
};
use wgpui::components::Text;
use wgpui::components::organisms::{MarkdownView, ThreadEntry, ThreadEntryType};
use wgpui::components::sections::ThreadView;
use wgpui::markdown::MarkdownParser;

#[derive(Clone, Debug)]
pub(crate) enum ClaudeChatAction {
    SendPrompt(String),
    SendCurrentInput,
    CopyConnectCommand,
    ApproveTool,
    DenyTool,
}

/// Claude chat overlay with input and tunnel status.
pub(crate) struct ClaudeChatPane {
    pub(crate) thread: ThreadView,
    pub(crate) visible: bool,
    pub(crate) bounds: Bounds,
    backdrop_bounds: Bounds,
    header_bounds: Bounds,
    command_bounds: Bounds,
    input_bounds: Bounds,
    send_bounds: Bounds,
    copy_bounds: Bounds,
    approval_bounds: Bounds,
    approve_bounds: Bounds,
    deny_bounds: Bounds,
    status_text: String,
    connect_command: Option<String>,
    pending_tool_label: Option<String>,
    input: TextInput,
    send_button: Button,
    copy_button: Button,
    approve_button: Button,
    deny_button: Button,
    event_ctx: EventContext,
    actions: Rc<RefCell<Vec<ClaudeChatAction>>>,
}

impl ClaudeChatPane {
    pub(crate) fn new() -> Self {
        let actions = Rc::new(RefCell::new(Vec::new()));
        let submit_actions = actions.clone();
        let send_actions = actions.clone();
        let copy_actions = actions.clone();
        let approve_actions = actions.clone();
        let deny_actions = actions.clone();

        let input = TextInput::new()
            .placeholder("Ask Claude about this repo")
            .font_size(11.0)
            .padding(8.0, 5.0)
            .on_submit(move |value| {
                if !value.trim().is_empty() {
                    submit_actions
                        .borrow_mut()
                        .push(ClaudeChatAction::SendPrompt(value.to_string()));
                }
            });

        let send_button = Button::new("Send")
            .variant(ButtonVariant::Primary)
            .padding(10.0, 4.0)
            .on_click(move || {
                send_actions
                    .borrow_mut()
                    .push(ClaudeChatAction::SendCurrentInput);
            });

        let copy_button = Button::new("Copy")
            .variant(ButtonVariant::Secondary)
            .padding(8.0, 4.0)
            .on_click(move || {
                copy_actions
                    .borrow_mut()
                    .push(ClaudeChatAction::CopyConnectCommand);
            });

        let approve_button = Button::new("Approve")
            .variant(ButtonVariant::Primary)
            .padding(10.0, 4.0)
            .on_click(move || {
                approve_actions
                    .borrow_mut()
                    .push(ClaudeChatAction::ApproveTool);
            });

        let deny_button = Button::new("Deny")
            .variant(ButtonVariant::Secondary)
            .padding(10.0, 4.0)
            .on_click(move || {
                deny_actions
                    .borrow_mut()
                    .push(ClaudeChatAction::DenyTool);
            });

        Self {
            thread: ThreadView::new().auto_scroll(true),
            visible: false,
            bounds: Bounds::ZERO,
            backdrop_bounds: Bounds::ZERO,
            header_bounds: Bounds::ZERO,
            command_bounds: Bounds::ZERO,
            input_bounds: Bounds::ZERO,
            send_bounds: Bounds::ZERO,
            copy_bounds: Bounds::ZERO,
            approval_bounds: Bounds::ZERO,
            approve_bounds: Bounds::ZERO,
            deny_bounds: Bounds::ZERO,
            status_text: "idle".to_string(),
            connect_command: None,
            pending_tool_label: None,
            input,
            send_button,
            copy_button,
            approve_button,
            deny_button,
            event_ctx: EventContext::new(),
            actions,
        }
    }

    pub(crate) fn show(&mut self) {
        self.visible = true;
    }

    pub(crate) fn hide(&mut self) {
        self.visible = false;
    }

    pub(crate) fn clear(&mut self) {
        self.thread.clear();
        self.connect_command = None;
        self.pending_tool_label = None;
        self.status_text = "idle".to_string();
        self.input.set_value("");
    }

    pub(crate) fn set_status(&mut self, text: impl Into<String>) {
        self.status_text = text.into();
    }

    pub(crate) fn set_connect_command(&mut self, command: Option<String>) {
        self.connect_command = command;
    }

    pub(crate) fn set_pending_tool_label(&mut self, label: Option<String>) {
        self.pending_tool_label = label;
    }

    pub(crate) fn take_actions(&mut self) -> Vec<ClaudeChatAction> {
        let mut actions = self.actions.borrow_mut();
        if actions.is_empty() {
            Vec::new()
        } else {
            actions.drain(..).collect()
        }
    }

    pub(crate) fn take_input(&mut self) -> String {
        let value = self.input.get_value().to_string();
        self.input.set_value("");
        value
    }

    pub(crate) fn push_assistant_message(&mut self, text: &str) {
        let parser = MarkdownParser::new();
        let document = parser.parse(text);
        let markdown_view = MarkdownView::new(document);
        let entry = ThreadEntry::new(ThreadEntryType::Assistant, markdown_view)
            .copyable_text(text.to_string());
        self.thread.push_entry(entry);
    }

    pub(crate) fn push_user_message(&mut self, text: &str) {
        let entry = ThreadEntry::new(ThreadEntryType::User, Text::new(text))
            .copyable_text(text.to_string());
        self.thread.push_entry(entry);
    }

    pub(crate) fn push_tool_message(&mut self, text: &str) {
        let entry = ThreadEntry::new(ThreadEntryType::Tool, Text::new(text))
            .copyable_text(text.to_string());
        self.thread.push_entry(entry);
    }

    pub(crate) fn push_system_message(&mut self, text: &str) {
        let entry = ThreadEntry::new(ThreadEntryType::System, Text::new(text));
        self.thread.push_entry(entry);
    }

    pub(crate) fn push_error_message(&mut self, text: &str) {
        let entry = ThreadEntry::new(ThreadEntryType::Error, Text::new(text));
        self.thread.push_entry(entry);
    }

    pub(crate) fn push_streaming_assistant(&mut self) {
        let parser = MarkdownParser::new();
        let document = parser.parse("");
        let markdown_view = MarkdownView::new(document);
        let entry = ThreadEntry::new(ThreadEntryType::Assistant, markdown_view);
        self.thread.push_entry(entry);
    }

    pub(crate) fn update_last_assistant(&mut self, text: &str) {
        if let Some(entry) = self.thread.last_entry_mut() {
            let parser = MarkdownParser::new();
            let document = parser.parse(text);
            let markdown_view = MarkdownView::new(document);
            entry.set_content(markdown_view);
        }
    }

    pub(crate) fn calculate_bounds(&mut self, viewport_width: f32, viewport_height: f32) {
        self.backdrop_bounds = Bounds::new(0.0, 0.0, viewport_width, viewport_height);

        let pane_width = 720.0_f32.min(viewport_width - 48.0);
        let pane_height = viewport_height - 80.0;
        let pane_x = (viewport_width - pane_width) / 2.0;
        let pane_y = 40.0;

        self.bounds = Bounds::new(pane_x, pane_y, pane_width, pane_height);

        let header_height = 28.0;
        self.header_bounds = Bounds::new(
            self.bounds.origin.x + 12.0,
            self.bounds.origin.y + 10.0,
            self.bounds.size.width - 24.0,
            header_height,
        );

        if self.connect_command.is_some() {
            let command_height = 34.0;
            self.command_bounds = Bounds::new(
                self.bounds.origin.x + 12.0,
                self.header_bounds.origin.y + self.header_bounds.size.height + 6.0,
                self.bounds.size.width - 24.0,
                command_height,
            );
        } else {
            self.command_bounds = Bounds::ZERO;
            self.copy_bounds = Bounds::ZERO;
        }

        if self.pending_tool_label.is_some() {
            let approval_height = 34.0;
            self.approval_bounds = Bounds::new(
                self.bounds.origin.x + 12.0,
                self.bounds.origin.y + self.bounds.size.height - 52.0 - approval_height,
                self.bounds.size.width - 24.0,
                approval_height,
            );
        } else {
            self.approval_bounds = Bounds::ZERO;
            self.approve_bounds = Bounds::ZERO;
            self.deny_bounds = Bounds::ZERO;
        }

        let input_height = 36.0;
        self.input_bounds = Bounds::new(
            self.bounds.origin.x + 12.0,
            self.bounds.origin.y + self.bounds.size.height - input_height - 12.0,
            self.bounds.size.width - 140.0,
            input_height,
        );
        self.send_bounds = Bounds::new(
            self.bounds.origin.x + self.bounds.size.width - 116.0,
            self.input_bounds.origin.y,
            104.0,
            input_height,
        );

        if self.connect_command.is_some() {
            let copy_width = 70.0;
            self.copy_bounds = Bounds::new(
                self.command_bounds.origin.x + self.command_bounds.size.width - copy_width,
                self.command_bounds.origin.y + 4.0,
                copy_width,
                self.command_bounds.size.height - 8.0,
            );
        }

        if self.pending_tool_label.is_some() {
            let approve_width = 84.0;
            let deny_width = 76.0;
            self.approve_bounds = Bounds::new(
                self.approval_bounds.origin.x + self.approval_bounds.size.width - approve_width - deny_width - 8.0,
                self.approval_bounds.origin.y + 4.0,
                approve_width,
                self.approval_bounds.size.height - 8.0,
            );
            self.deny_bounds = Bounds::new(
                self.approval_bounds.origin.x + self.approval_bounds.size.width - deny_width,
                self.approval_bounds.origin.y + 4.0,
                deny_width,
                self.approval_bounds.size.height - 8.0,
            );
        }
    }

    pub(crate) fn paint(&mut self, cx: &mut PaintContext) {
        if !self.visible {
            return;
        }

        cx.scene.draw_quad(
            Quad::new(self.backdrop_bounds).with_background(theme::bg::APP.with_alpha(0.88)),
        );

        cx.scene.draw_quad(
            Quad::new(self.bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0)
                .with_corner_radius(4.0),
        );

        let header_text = format!("Claude - {}", self.status_text);
        Text::new(&header_text).paint(self.header_bounds, cx);

        if let Some(command) = &self.connect_command {
            let label_bounds = Bounds::new(
                self.command_bounds.origin.x,
                self.command_bounds.origin.y + 6.0,
                self.command_bounds.size.width - self.copy_bounds.size.width - 8.0,
                self.command_bounds.size.height - 12.0,
            );
            let command_text = format!("Run: {}", command);
            Text::new(&command_text).paint(label_bounds, cx);
            self.copy_button.paint(self.copy_bounds, cx);
        }

        if let Some(label) = &self.pending_tool_label {
            let label_bounds = Bounds::new(
                self.approval_bounds.origin.x,
                self.approval_bounds.origin.y + 6.0,
                self.approval_bounds.size.width - self.approve_bounds.size.width - self.deny_bounds.size.width - 16.0,
                self.approval_bounds.size.height - 12.0,
            );
            Text::new(label).paint(label_bounds, cx);
            self.approve_button.paint(self.approve_bounds, cx);
            self.deny_button.paint(self.deny_bounds, cx);
        }

        let thread_top = if self.connect_command.is_some() {
            self.command_bounds.origin.y + self.command_bounds.size.height + 6.0
        } else {
            self.header_bounds.origin.y + self.header_bounds.size.height + 8.0
        };
        let thread_bottom = if self.pending_tool_label.is_some() {
            self.approval_bounds.origin.y - 8.0
        } else {
            self.input_bounds.origin.y - 8.0
        };
        let thread_bounds = Bounds::new(
            self.bounds.origin.x + 12.0,
            thread_top,
            self.bounds.size.width - 24.0,
            thread_bottom - thread_top,
        );

        self.thread.paint(thread_bounds, cx);
        self.input.paint(self.input_bounds, cx);
        self.send_button.paint(self.send_bounds, cx);
    }

    pub(crate) fn handle_event(&mut self, event: InputEvent) -> EventResult {
        if !self.visible {
            return EventResult::Ignored;
        }

        let mut handled = EventResult::Ignored;
        let thread_top = if self.connect_command.is_some() {
            self.command_bounds.origin.y + self.command_bounds.size.height + 6.0
        } else {
            self.header_bounds.origin.y + self.header_bounds.size.height + 8.0
        };
        let thread_bottom = if self.pending_tool_label.is_some() {
            self.approval_bounds.origin.y - 8.0
        } else {
            self.input_bounds.origin.y - 8.0
        };
        let thread_bounds = Bounds::new(
            self.bounds.origin.x + 12.0,
            thread_top,
            self.bounds.size.width - 24.0,
            thread_bottom - thread_top,
        );

        match &event {
            InputEvent::Scroll { .. } => {
                handled = self.thread.event(&event, thread_bounds, &mut self.event_ctx);
            }
            InputEvent::MouseMove { x, y } => {
                let point = Point::new(*x, *y);
                if self.input_bounds.contains(point) {
                    handled = self.input.event(&event, self.input_bounds, &mut self.event_ctx);
                }
                if self.send_bounds.contains(point) {
                    let _ = self
                        .send_button
                        .event(&event, self.send_bounds, &mut self.event_ctx);
                }
                if self.connect_command.is_some() && self.copy_bounds.contains(point) {
                    let _ = self
                        .copy_button
                        .event(&event, self.copy_bounds, &mut self.event_ctx);
                }
                if self.pending_tool_label.is_some() {
                    let _ = self
                        .approve_button
                        .event(&event, self.approve_bounds, &mut self.event_ctx);
                    let _ = self
                        .deny_button
                        .event(&event, self.deny_bounds, &mut self.event_ctx);
                }
                if thread_bounds.contains(point) {
                    handled = self.thread.event(&event, thread_bounds, &mut self.event_ctx);
                }
            }
            InputEvent::MouseDown { x, y, .. } | InputEvent::MouseUp { x, y, .. } => {
                let point = Point::new(*x, *y);
                if self.input_bounds.contains(point) {
                    handled = self.input.event(&event, self.input_bounds, &mut self.event_ctx);
                }
                if self.send_bounds.contains(point) {
                    handled = merge_event_result(
                        handled,
                        self.send_button
                            .event(&event, self.send_bounds, &mut self.event_ctx),
                    );
                }
                if self.connect_command.is_some() && self.copy_bounds.contains(point) {
                    handled = merge_event_result(
                        handled,
                        self.copy_button
                            .event(&event, self.copy_bounds, &mut self.event_ctx),
                    );
                }
                if self.pending_tool_label.is_some() {
                    handled = merge_event_result(
                        handled,
                        self.approve_button
                            .event(&event, self.approve_bounds, &mut self.event_ctx),
                    );
                    handled = merge_event_result(
                        handled,
                        self.deny_button
                            .event(&event, self.deny_bounds, &mut self.event_ctx),
                    );
                }
                if thread_bounds.contains(point) {
                    handled = merge_event_result(
                        handled,
                        self.thread.event(&event, thread_bounds, &mut self.event_ctx),
                    );
                }
            }
            InputEvent::KeyDown { .. } => {
                if self.input.is_focused() {
                    handled = self.input.event(&event, self.input_bounds, &mut self.event_ctx);
                }
            }
            _ => {}
        }

        if matches!(handled, EventResult::Handled) {
            handled
        } else {
            EventResult::Handled
        }
    }

    pub(crate) fn cursor(&self, _point: Point) -> Cursor {
        if !self.visible {
            Cursor::Default
        } else {
            Cursor::Default
        }
    }

    pub(crate) fn contains(&self, point: Point) -> bool {
        self.visible && self.backdrop_bounds.contains(point)
    }
}

impl Default for ClaudeChatPane {
    fn default() -> Self {
        Self::new()
    }
}

fn merge_event_result(a: EventResult, b: EventResult) -> EventResult {
    if matches!(a, EventResult::Handled) || matches!(b, EventResult::Handled) {
        EventResult::Handled
    } else {
        EventResult::Ignored
    }
}
