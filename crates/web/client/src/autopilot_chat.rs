use std::cell::RefCell;
use std::rc::Rc;

use wgpui::{
    Bounds, Button, ButtonVariant, Component, Cursor, EventContext, EventResult, InputEvent,
    PaintContext, Point, Quad, theme,
};
use wgpui::components::Text;
use wgpui::components::molecules::{CollapsibleSection, SectionStatus};
use wgpui::components::organisms::{MarkdownView, ThreadEntry, ThreadEntryType};
use wgpui::components::sections::ThreadView;
use wgpui::markdown::MarkdownParser;

#[derive(Clone, Copy)]
pub(crate) enum AutopilotAction {
    StartClaude,
}

/// Centered overlay chat pane for the autopilot agent.
pub(crate) struct AutopilotChatPane {
    pub(crate) thread: ThreadView,
    pub(crate) visible: bool,
    pub(crate) bounds: Bounds,
    backdrop_bounds: Bounds,
    footer_bounds: Bounds,
    footer_button_bounds: Bounds,
    start_claude_visible: bool,
    start_claude_button: Button,
    event_ctx: EventContext,
    actions: Rc<RefCell<Vec<AutopilotAction>>>,
}

impl AutopilotChatPane {
    pub(crate) fn new() -> Self {
        let actions = Rc::new(RefCell::new(Vec::new()));
        let actions_handle = actions.clone();
        let start_claude_button = Button::new("Start Claude")
            .variant(ButtonVariant::Primary)
            .padding(10.0, 4.0)
            .on_click(move || {
                actions_handle
                    .borrow_mut()
                    .push(AutopilotAction::StartClaude);
            });
        Self {
            thread: ThreadView::new().auto_scroll(true),
            visible: false,
            bounds: Bounds::ZERO,
            backdrop_bounds: Bounds::ZERO,
            footer_bounds: Bounds::ZERO,
            footer_button_bounds: Bounds::ZERO,
            start_claude_visible: false,
            start_claude_button,
            event_ctx: EventContext::new(),
            actions,
        }
    }

    /// Show the chat pane with a greeting message.
    pub(crate) fn show(&mut self, github_username: &str) {
        self.visible = true;
        self.start_claude_visible = false;
        self.thread.clear();

        let greeting = format!("Hello {}, I am your first Autopilot.", github_username);
        self.push_assistant_message(&greeting);
    }

    /// Hide the chat pane.
    pub(crate) fn hide(&mut self) {
        self.visible = false;
    }

    /// Enable the CTA for starting the Claude tunnel flow.
    pub(crate) fn enable_claude_cta(&mut self) {
        self.start_claude_visible = true;
    }

    pub(crate) fn take_actions(&mut self) -> Vec<AutopilotAction> {
        let mut actions = self.actions.borrow_mut();
        if actions.is_empty() {
            Vec::new()
        } else {
            actions.drain(..).collect()
        }
    }

    /// Push an assistant message to the chat (with markdown rendering).
    pub(crate) fn push_assistant_message(&mut self, text: &str) {
        let parser = MarkdownParser::new();
        let document = parser.parse(text);
        let markdown_view = MarkdownView::new(document);
        let entry = ThreadEntry::new(
            ThreadEntryType::Assistant,
            markdown_view,
        ).copyable_text(text.to_string());
        self.thread.push_entry(entry);
    }

    /// Push a tool update message (shows as Tool type).
    pub(crate) fn push_tool_message(&mut self, tool_name: &str, status: &str) {
        let text = format!("{}: {}", tool_name, status);
        let entry = ThreadEntry::new(
            ThreadEntryType::Tool,
            Text::new(&text),
        ).copyable_text(text.clone());
        self.thread.push_entry(entry);
    }

    /// Push a collapsible tool result (for recall_knowledge and similar).
    pub(crate) fn push_collapsible_tool(&mut self, summary: &str, details: Vec<String>, status: SectionStatus) {
        let section = CollapsibleSection::new(summary)
            .status(status)
            .details(details);
        let entry = ThreadEntry::new(
            ThreadEntryType::Tool,
            section,
        ).copyable_text(summary.to_string());
        self.thread.push_entry(entry);
    }

    /// Push a system/info message.
    pub(crate) fn push_system_message(&mut self, text: &str) {
        let entry = ThreadEntry::new(
            ThreadEntryType::System,
            Text::new(text),
        );
        self.thread.push_entry(entry);
    }

    /// Push an error message.
    pub(crate) fn push_error_message(&mut self, text: &str) {
        let entry = ThreadEntry::new(
            ThreadEntryType::Error,
            Text::new(text),
        );
        self.thread.push_entry(entry);
    }

    /// Update the last tool entry with new status (for streaming updates).
    pub(crate) fn update_last_tool(&mut self, status: &str) {
        if let Some(entry) = self.thread.last_entry_mut() {
            entry.set_content(Text::new(status));
        }
    }

    /// Update the last assistant entry with new markdown content (for streaming).
    pub(crate) fn update_last_assistant(&mut self, text: &str) {
        if let Some(entry) = self.thread.last_entry_mut() {
            let parser = MarkdownParser::new();
            let document = parser.parse(text);
            let markdown_view = MarkdownView::new(document);
            entry.set_content(markdown_view);
        }
    }

    /// Push an empty assistant message placeholder (for streaming).
    pub(crate) fn push_streaming_assistant(&mut self) {
        let parser = MarkdownParser::new();
        let document = parser.parse("");
        let markdown_view = MarkdownView::new(document);
        let entry = ThreadEntry::new(
            ThreadEntryType::Assistant,
            markdown_view,
        );
        self.thread.push_entry(entry);
    }

    /// Calculate bounds for centered overlay.
    pub(crate) fn calculate_bounds(&mut self, viewport_width: f32, viewport_height: f32) {
        // Full viewport backdrop
        self.backdrop_bounds = Bounds::new(0.0, 0.0, viewport_width, viewport_height);

        // Centered pane: max 600px wide, leave 60px top margin, 40px bottom
        let pane_width = 600.0_f32.min(viewport_width - 48.0);
        let pane_height = viewport_height - 100.0;
        let pane_x = (viewport_width - pane_width) / 2.0;
        let pane_y = 60.0;

        self.bounds = Bounds::new(pane_x, pane_y, pane_width, pane_height);

        self.footer_bounds = Bounds::ZERO;
        self.footer_button_bounds = Bounds::ZERO;
        if self.start_claude_visible {
            let footer_height = 36.0;
            self.footer_bounds = Bounds::new(
                self.bounds.origin.x + 12.0,
                self.bounds.origin.y + self.bounds.size.height - footer_height - 12.0,
                self.bounds.size.width - 24.0,
                footer_height,
            );
            let button_width = 140.0;
            self.footer_button_bounds = Bounds::new(
                self.footer_bounds.origin.x + self.footer_bounds.size.width - button_width,
                self.footer_bounds.origin.y + 4.0,
                button_width,
                self.footer_bounds.size.height - 8.0,
            );
        }
    }

    /// Paint the overlay.
    pub(crate) fn paint(&mut self, cx: &mut PaintContext) {
        if !self.visible {
            return;
        }

        // Semi-transparent backdrop
        cx.scene.draw_quad(
            Quad::new(self.backdrop_bounds)
                .with_background(theme::bg::APP.with_alpha(0.85)),
        );

        // Pane background with border - uniform color, no header
        cx.scene.draw_quad(
            Quad::new(self.bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0)
                .with_corner_radius(4.0),
        );

        let footer_height = if self.start_claude_visible {
            self.footer_bounds.size.height + 12.0
        } else {
            0.0
        };

        // Thread content area
        let content_bounds = Bounds::new(
            self.bounds.origin.x + 12.0,
            self.bounds.origin.y + 12.0,
            self.bounds.size.width - 24.0,
            self.bounds.size.height - 24.0 - footer_height,
        );

        self.thread.paint(content_bounds, cx);

        if self.start_claude_visible {
            // Divider line
            cx.scene.draw_quad(
                Quad::new(Bounds::new(
                    self.footer_bounds.origin.x,
                    self.footer_bounds.origin.y,
                    self.footer_bounds.size.width,
                    1.0,
                ))
                .with_background(theme::border::DEFAULT),
            );

            let label_bounds = Bounds::new(
                self.footer_bounds.origin.x,
                self.footer_bounds.origin.y + 8.0,
                self.footer_bounds.size.width - self.footer_button_bounds.size.width - 8.0,
                self.footer_bounds.size.height - 16.0,
            );
            Text::new("Run the full Claude agent locally.").paint(label_bounds, cx);
            self.start_claude_button
                .paint(self.footer_button_bounds, cx);
        }
    }

    /// Handle input events.
    pub(crate) fn handle_event(&mut self, event: InputEvent) -> EventResult {
        if !self.visible {
            return EventResult::Ignored;
        }

        let footer_height = if self.start_claude_visible {
            self.footer_bounds.size.height + 12.0
        } else {
            0.0
        };

        // Calculate content bounds for thread
        let content_bounds = Bounds::new(
            self.bounds.origin.x + 12.0,
            self.bounds.origin.y + 12.0,
            self.bounds.size.width - 24.0,
            self.bounds.size.height - 24.0 - footer_height,
        );

        // Forward scroll and mouse events to thread
        match &event {
            InputEvent::Scroll { .. } => {
                // Forward scroll to thread (Scroll only has dx/dy deltas, no position)
                let mut events = EventContext::new();
                return self.thread.event(&event, content_bounds, &mut events);
            }
            InputEvent::MouseMove { x, y } => {
                let point = Point::new(*x, *y);
                if self.start_claude_visible && self.footer_bounds.contains(point) {
                    let _ = self
                        .start_claude_button
                        .event(&event, self.footer_button_bounds, &mut self.event_ctx);
                }
                if content_bounds.contains(point) {
                    return self.thread.event(&event, content_bounds, &mut self.event_ctx);
                }
            }
            InputEvent::MouseDown { x, y, .. } | InputEvent::MouseUp { x, y, .. } => {
                let point = Point::new(*x, *y);
                if self.start_claude_visible && self.footer_bounds.contains(point) {
                    let result = self
                        .start_claude_button
                        .event(&event, self.footer_button_bounds, &mut self.event_ctx);
                    if matches!(result, EventResult::Handled) {
                        return result;
                    }
                }
                if self.bounds.contains(point) && content_bounds.contains(point) {
                    return self.thread.event(&event, content_bounds, &mut self.event_ctx);
                }
            }
            _ => {}
        }

        // Consume all events when visible to prevent click-through
        if self.visible {
            EventResult::Handled
        } else {
            EventResult::Ignored
        }
    }

    /// Get cursor for current state.
    pub(crate) fn cursor(&self, point: Point) -> Cursor {
        if !self.visible {
            return Cursor::Default;
        }

        if self.bounds.contains(point) {
            Cursor::Default
        } else {
            Cursor::Default
        }
    }

    /// Check if a point is within the overlay.
    pub(crate) fn contains(&self, point: Point) -> bool {
        self.visible && self.backdrop_bounds.contains(point)
    }
}

impl Default for AutopilotChatPane {
    fn default() -> Self {
        Self::new()
    }
}
