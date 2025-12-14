//! ChatThread widget - renders a scrollable list of messages.
//!
//! Uses virtual scrolling for efficient rendering of large message histories.

use crate::input::ChatInput;
use crate::message::MessageBubble;
use crate::tool_use::ToolUseIndicator;
use coder_domain::ids::ThreadId;
use coder_domain::{ChatEntry, ChatView};
use coder_ui_runtime::Signal;
use coder_widgets::context::{EventContext, PaintContext};
use coder_widgets::{EventResult, Widget, WidgetId};
use wgpui::scroll::ScrollContainer;
use wgpui::{Bounds, InputEvent, Point, Quad, Size};

/// Callback for when a message is sent.
pub type OnSend = Box<dyn FnMut(&str)>;

/// A chat thread widget that displays messages and handles input.
pub struct ChatThread {
    /// Widget ID.
    id: Option<WidgetId>,

    /// Thread ID.
    thread_id: ThreadId,

    /// Chat view state (reactive).
    chat_view: Signal<ChatView>,

    /// Scroll container.
    scroll: ScrollContainer,

    /// Item heights cache.
    item_heights: Vec<f32>,

    /// Estimated message height.
    estimated_height: f32,

    /// Overscan (extra items to render).
    overscan: usize,

    /// Padding between messages.
    message_padding: f32,

    /// Chat input widget.
    input: ChatInput,

    /// Input area height.
    input_height: f32,

    /// Callback for sending messages.
    on_send: Option<OnSend>,
}

impl ChatThread {
    /// Create a new chat thread.
    pub fn new(thread_id: ThreadId) -> Self {
        let chat_view = Signal::new(ChatView::new(thread_id));

        Self {
            id: None,
            thread_id,
            chat_view,
            scroll: ScrollContainer::vertical(Bounds::ZERO),
            item_heights: Vec::new(),
            estimated_height: 80.0,
            overscan: 3,
            message_padding: 12.0,
            input: ChatInput::new(),
            input_height: 64.0,
            on_send: None,
        }
    }

    /// Set the widget ID.
    pub fn id(mut self, id: WidgetId) -> Self {
        self.id = Some(id);
        self
    }

    /// Set the chat view signal.
    pub fn chat_view(mut self, view: Signal<ChatView>) -> Self {
        self.chat_view = view;
        self
    }

    /// Set the on_send callback.
    pub fn on_send<F>(mut self, f: F) -> Self
    where
        F: FnMut(&str) + 'static,
    {
        self.on_send = Some(Box::new(f));
        self
    }

    /// Set the input height.
    pub fn input_height(mut self, height: f32) -> Self {
        self.input_height = height;
        self
    }

    /// Get the chat view signal.
    pub fn get_chat_view(&self) -> &Signal<ChatView> {
        &self.chat_view
    }

    /// Update the chat view.
    pub fn set_chat_view(&mut self, view: ChatView) {
        self.chat_view.set(view);
    }

    /// Scroll to the bottom.
    pub fn scroll_to_bottom(&mut self) {
        let max = self.scroll.max_scroll();
        self.scroll.scroll_to(max);
    }

    /// Calculate total content height.
    fn content_height(&self) -> f32 {
        let view = self.chat_view.get_untracked();
        let entry_count = view.entries.len();

        // Use cached heights where available, estimate for rest
        let mut height = 0.0;
        for i in 0..entry_count {
            height += self
                .item_heights
                .get(i)
                .copied()
                .unwrap_or(self.estimated_height);
            height += self.message_padding;
        }

        // Add streaming message height if any
        if view.streaming_message.is_some() {
            height += self.estimated_height + self.message_padding;
        }

        height
    }

    /// Calculate visible range of entries.
    fn visible_range(&self, viewport_height: f32) -> std::ops::Range<usize> {
        let view = self.chat_view.get_untracked();
        let entry_count = view.entries.len();

        if entry_count == 0 {
            return 0..0;
        }

        let scroll_offset = self.scroll.scroll_offset.y;

        // Find first visible entry
        let mut y = 0.0;
        let mut first_visible = 0;
        for i in 0..entry_count {
            let height = self
                .item_heights
                .get(i)
                .copied()
                .unwrap_or(self.estimated_height);
            if y + height > scroll_offset {
                first_visible = i;
                break;
            }
            y += height + self.message_padding;
        }

        // Calculate visible count
        let visible_count = (viewport_height / self.estimated_height).ceil() as usize + 1;

        let start = first_visible.saturating_sub(self.overscan);
        let end = (first_visible + visible_count + self.overscan).min(entry_count);

        start..end
    }

    /// Get Y offset for an entry.
    fn entry_y_offset(&self, index: usize) -> f32 {
        let mut y = 0.0;
        for i in 0..index {
            y += self
                .item_heights
                .get(i)
                .copied()
                .unwrap_or(self.estimated_height);
            y += self.message_padding;
        }
        y
    }
}

impl Widget for ChatThread {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Calculate content bounds (excluding input area)
        let content_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            bounds.size.height - self.input_height,
        );

        // Update scroll container
        self.scroll.set_viewport(content_bounds);
        self.scroll.set_content_size(Size::new(
            content_bounds.size.width,
            self.content_height(),
        ));

        let view = self.chat_view.get_untracked();
        let visible_range = self.visible_range(content_bounds.size.height);

        // Draw background
        cx.scene.draw_quad(
            Quad::new(content_bounds).with_background(wgpui::theme::bg::APP),
        );

        // Push clip
        cx.scene.push_clip(content_bounds);

        // Render visible entries
        for i in visible_range {
            if let Some(entry) = view.entries.get(i) {
                let y_offset = self.entry_y_offset(i);
                let entry_y = content_bounds.origin.y + y_offset - self.scroll.scroll_offset.y;

                // Skip if off screen
                if entry_y + self.estimated_height < content_bounds.origin.y
                    || entry_y > content_bounds.origin.y + content_bounds.size.height
                {
                    continue;
                }

                match entry {
                    ChatEntry::Message(msg_view) => {
                        let msg_bounds = Bounds::new(
                            content_bounds.origin.x + 16.0,
                            entry_y,
                            content_bounds.size.width - 32.0,
                            self.item_heights
                                .get(i)
                                .copied()
                                .unwrap_or(self.estimated_height),
                        );
                        let mut bubble = MessageBubble::new(&msg_view.content, msg_view.role);
                        bubble.paint(msg_bounds, cx);
                    }
                    ChatEntry::ToolUse(tool_view) => {
                        let tool_bounds = Bounds::new(
                            content_bounds.origin.x + 32.0, // Indented
                            entry_y,
                            content_bounds.size.width - 64.0,
                            self.item_heights
                                .get(i)
                                .copied()
                                .unwrap_or(self.estimated_height),
                        );
                        let mut indicator = ToolUseIndicator::new(
                            &tool_view.tool_name,
                            tool_view.status,
                        );
                        indicator.paint(tool_bounds, cx);
                    }
                }
            }
        }

        // Render streaming message at bottom
        if let Some(streaming) = &view.streaming_message {
            if !streaming.is_complete {
                let y_offset = self.content_height() - self.estimated_height;
                let streaming_y =
                    content_bounds.origin.y + y_offset - self.scroll.scroll_offset.y;

                let streaming_bounds = Bounds::new(
                    content_bounds.origin.x + 16.0,
                    streaming_y,
                    content_bounds.size.width - 32.0,
                    self.estimated_height,
                );

                let mut bubble =
                    MessageBubble::new(&streaming.content_so_far, coder_domain::message::Role::Assistant)
                        .streaming(true);
                bubble.paint(streaming_bounds, cx);
            }
        }

        cx.scene.pop_clip();

        // Draw input area
        let input_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + bounds.size.height - self.input_height,
            bounds.size.width,
            self.input_height,
        );

        // Input area background
        cx.scene.draw_quad(
            Quad::new(input_bounds)
                .with_background(wgpui::theme::bg::SURFACE)
                .with_border(wgpui::theme::border::DEFAULT, 1.0),
        );

        // Paint input widget
        let input_widget_bounds = Bounds::new(
            input_bounds.origin.x + 12.0,
            input_bounds.origin.y + 12.0,
            input_bounds.size.width - 24.0,
            input_bounds.size.height - 24.0,
        );
        self.input.paint(input_widget_bounds, cx);
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        let content_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            bounds.size.height - self.input_height,
        );

        let input_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + bounds.size.height - self.input_height,
            bounds.size.width,
            self.input_height,
        );

        // Handle scroll in content area
        match event {
            InputEvent::Wheel { delta, .. } => {
                if content_bounds.contains(Point::new(
                    content_bounds.origin.x + content_bounds.size.width / 2.0,
                    content_bounds.origin.y + content_bounds.size.height / 2.0,
                )) {
                    self.scroll.scroll_by(*delta);
                    return EventResult::Handled;
                }
            }
            _ => {}
        }

        // Forward events to input
        let result = self.input.event(event, input_bounds, cx);
        if result.is_handled() {
            return result;
        }

        EventResult::Ignored
    }

    fn id(&self) -> Option<WidgetId> {
        self.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_thread_creation() {
        let thread_id = ThreadId::new();
        let thread = ChatThread::new(thread_id).id(1);

        assert_eq!(thread.id, Some(1));
        assert_eq!(thread.thread_id, thread_id);
    }

    #[test]
    fn test_visible_range_empty() {
        let thread_id = ThreadId::new();
        let thread = ChatThread::new(thread_id);

        let range = thread.visible_range(500.0);
        assert_eq!(range, 0..0);
    }

    #[test]
    fn test_content_height() {
        let thread_id = ThreadId::new();
        let thread = ChatThread::new(thread_id);

        // Empty view should have zero height
        assert_eq!(thread.content_height(), 0.0);
    }
}
