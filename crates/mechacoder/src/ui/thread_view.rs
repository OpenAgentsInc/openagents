//! Thread view component for displaying conversation.

use std::collections::HashMap;

use crate::sdk_thread::{SdkThread, SdkThreadEvent, ThreadEntry, ThreadStatus};
use gpui::{
    div, list, prelude::*, px, App, Context, Entity, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ListState, ParentElement, Render, Styled, Subscription, Window,
};
use theme_oa::{border, text, FONT_FAMILY};
use ui_oa::{Button, ButtonVariant};

use super::message_input::{MessageInput, SendMessageEvent};
use super::message_view::{MessageView, SimpleMessageView};
use super::tbench_view::{TBenchEventView, TBenchRunView};
use super::todo_panel_view::render_todo_panel;
use super::tool_call_view::ToolCallView;

/// Thread view for displaying the conversation.
pub struct ThreadView {
    /// The SDK thread.
    thread: Entity<SdkThread>,
    /// Message input component.
    message_input: Entity<MessageInput>,
    /// Focus handle.
    focus_handle: FocusHandle,
    /// List state for entries.
    list_state: ListState,
    /// Cached message views by entry index.
    message_cache: HashMap<usize, Entity<MessageView>>,
    /// Cached streaming message view.
    streaming_message: Option<Entity<MessageView>>,
    /// Last streaming content (to detect changes).
    last_streaming_content: Option<String>,
    /// Subscription to thread events.
    _thread_subscription: Subscription,
    /// Subscription to message input events.
    _input_subscription: Subscription,
}

impl ThreadView {
    /// Create a new thread view.
    pub fn new(thread: Entity<SdkThread>, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();

        // Create message input
        let message_input = cx.new(|cx| MessageInput::new(cx));

        // Subscribe to thread events
        let thread_subscription = cx.subscribe(&thread, |this, _thread, event, cx| {
            this.handle_thread_event(event, cx);
        });

        // Subscribe to message input submit events
        let input_subscription = cx.subscribe(&message_input, |this, _input, event: &SendMessageEvent, cx| {
            this.send_message_content(&event.0, cx);
        });

        // Create list state
        let list_state = ListState::new(0, gpui::ListAlignment::Bottom, px(100.0));

        Self {
            thread,
            message_input,
            focus_handle,
            list_state,
            message_cache: HashMap::new(),
            streaming_message: None,
            last_streaming_content: None,
            _thread_subscription: thread_subscription,
            _input_subscription: input_subscription,
        }
    }

    /// Get the focus handle for the message input.
    pub fn message_input_focus_handle(&self, cx: &App) -> FocusHandle {
        self.message_input.read(cx).focus_handle(cx)
    }

    /// Focus the message input.
    pub fn focus_message_input(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.message_input.update(cx, |input, cx| {
            input.focus(window, cx);
        });
    }

    /// Handle a thread event.
    fn handle_thread_event(&mut self, event: &SdkThreadEvent, cx: &mut Context<Self>) {
        match event {
            SdkThreadEvent::EntryAdded(_) | SdkThreadEvent::EntryUpdated(_) => {
                // Clear streaming state when entry is finalized
                self.streaming_message = None;
                self.last_streaming_content = None;

                // Update list count
                let entry_count = self.thread.read(cx).entries().len();
                self.list_state.reset(entry_count);
                cx.notify();
            }
            SdkThreadEvent::StatusChanged(_) => {
                cx.notify();
            }
            SdkThreadEvent::TodosUpdated => {
                // Re-render to show updated todo list
                cx.notify();
            }
            SdkThreadEvent::Error(error) => {
                log::error!("Thread error: {}", error);
                cx.notify();
            }
        }
    }

    /// Get or create a message view for the given entry index.
    fn get_or_create_message_view(
        &mut self,
        entry: &ThreadEntry,
        ix: usize,
        cx: &mut App,
    ) -> Entity<MessageView> {
        if let Some(view) = self.message_cache.get(&ix) {
            return view.clone();
        }

        let view = match entry {
            ThreadEntry::UserMessage(msg) => MessageView::user(&msg.content, cx),
            ThreadEntry::AssistantMessage(msg) => {
                MessageView::assistant(&msg.content, cx)
            }
            ThreadEntry::ToolUse(_) | ThreadEntry::TBenchRun(_) | ThreadEntry::TBenchEvent(_) | ThreadEntry::TestGenMessage(_) => {
                // These don't use MessageView, handled separately
                // Return a placeholder that won't be used
                MessageView::assistant("", cx)
            }
        };

        self.message_cache.insert(ix, view.clone());
        view
    }

    /// Send a message with the given content.
    fn send_message_content(&mut self, content: &str, cx: &mut Context<Self>) {
        if content.trim().is_empty() {
            return;
        }

        let content = content.to_string();

        // Send to thread
        self.thread.update(cx, |thread, cx| {
            thread.send_message(content, cx).detach();
        });
    }

    /// Send the current message from the input field.
    fn send_message(&mut self, cx: &mut Context<Self>) {
        let content = self.message_input.read(cx).content(cx);
        if content.trim().is_empty() {
            return;
        }

        // Clear input
        self.message_input.update(cx, |input, cx| {
            input.clear(cx);
        });

        // Send to thread
        self.thread.update(cx, |thread, cx| {
            thread.send_message(content, cx).detach();
        });
    }

    /// Cancel generation.
    fn cancel(&mut self, cx: &mut Context<Self>) {
        self.thread.update(cx, |thread, cx| {
            thread.cancel(cx);
        });
    }

    /// Respond to a permission request.
    /// Note: SDK uses BypassPermissions mode, so this is a no-op for now.
    #[allow(dead_code)]
    fn respond_permission(&mut self, _option_id: &str, _cx: &mut Context<Self>) {
        // SDK currently uses BypassPermissions mode
    }

    /// Render the permission prompt.
    /// Note: SDK uses BypassPermissions mode, so this always returns None.
    #[allow(dead_code)]
    fn render_permission_prompt(&self, _cx: &App) -> Option<impl IntoElement> {
        // SDK currently uses BypassPermissions mode, no permission prompts
        None::<gpui::Empty>
    }

    /// Render the status indicator (top left corner).
    fn render_status(&self, cx: &App) -> impl IntoElement {
        let status = self.thread.read(cx).status();

        div()
            .absolute()
            .top(px(8.0))
            .left(px(12.0))
            .text_sm()
            .text_color(text::SECONDARY)
            .child(
                match status {
                    ThreadStatus::Idle => "Ready",
                    ThreadStatus::Streaming => "Claude is typing...",
                    ThreadStatus::Completed => "Done",
                    ThreadStatus::Error(_) => "Error",
                }
                .to_string(),
            )
    }

    /// Get or update the streaming message view.
    fn get_streaming_view(&mut self, content: &str, cx: &mut App) -> Entity<MessageView> {
        // Check if content changed
        let content_changed = self.last_streaming_content.as_ref() != Some(&content.to_string());

        if let Some(view) = &self.streaming_message {
            if content_changed {
                // Update existing view
                view.update(cx, |view, cx| {
                    view.update_content(content, cx);
                });
                self.last_streaming_content = Some(content.to_string());
            }
            view.clone()
        } else {
            // Create new view
            let view = MessageView::assistant(content, cx);
            self.streaming_message = Some(view.clone());
            self.last_streaming_content = Some(content.to_string());
            view
        }
    }
}

impl Focusable for ThreadView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ThreadView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_streaming = matches!(*self.thread.read(cx).status(), ThreadStatus::Streaming);
        // Clone streaming content to release the borrow on cx
        let streaming_content = self.thread.read(cx).streaming_content().map(|s| s.to_string());

        // Get streaming view if streaming
        let streaming_view = streaming_content.as_ref().map(|content| {
            self.get_streaming_view(content, cx)
        });

        // Create render callback for list items
        let thread = self.thread.clone();
        let message_cache = self.message_cache.clone();
        let render_item = move |ix: usize, _window: &mut Window, cx: &mut App| {
            let entries = thread.read(cx).entries();
            if ix < entries.len() {
                let entry = &entries[ix];
                match entry {
                    ThreadEntry::UserMessage(_) | ThreadEntry::AssistantMessage(_) => {
                        if let Some(view) = message_cache.get(&ix) {
                            view.clone().into_any_element()
                        } else {
                            // Fallback to simple view if not cached
                            // (this shouldn't happen in practice)
                            match entry {
                                ThreadEntry::UserMessage(msg) => {
                                    SimpleMessageView::user(&msg.content).into_any_element()
                                }
                                ThreadEntry::AssistantMessage(msg) => {
                                    SimpleMessageView::assistant(&msg.content).into_any_element()
                                }
                                _ => div().into_any_element(),
                            }
                        }
                    }
                    ThreadEntry::ToolUse(tu) => {
                        ToolCallView::from_tool_use(tu).into_any_element()
                    }
                    ThreadEntry::TBenchRun(run) => {
                        TBenchRunView::from_entry(run).into_any_element()
                    }
                    ThreadEntry::TBenchEvent(event) => {
                        TBenchEventView::from_entry(event).into_any_element()
                    }
                    ThreadEntry::TestGenMessage(msg) => {
                        // Render TestGen messages with Berkeley Mono font
                        div()
                            .px(px(16.0))
                            .py(px(8.0))
                            .border_b_1()
                            .border_color(border::SUBTLE)
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(text::SECONDARY)
                                    .font_family(FONT_FAMILY)
                                    .child(msg.message.clone())
                            )
                            .into_any_element()
                    }
                }
            } else {
                div().into_any_element()
            }
        };

        // Pre-populate cache for all entries
        // Collect entries to avoid borrow conflict
        let entries: Vec<_> = self.thread.read(cx).entries().to_vec();
        let missing_indices: Vec<_> = entries
            .iter()
            .enumerate()
            .filter_map(|(ix, entry)| {
                if !self.message_cache.contains_key(&ix) {
                    match entry {
                        ThreadEntry::UserMessage(_) | ThreadEntry::AssistantMessage(_) => {
                            Some((ix, entry.clone()))
                        }
                        _ => None,
                    }
                } else {
                    None
                }
            })
            .collect();

        for (ix, entry) in missing_indices {
            let _ = self.get_or_create_message_view(&entry, ix, cx);
        }

        // Get todo state for panel
        let todo_state = self.thread.read(cx).todo_state().clone();

        div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .items_center()
            .track_focus(&self.focus_handle)
            // Todo panel (plan mode progress)
            .child(
                div()
                    .w_full()
                    .max_w(px(768.0))
                    .child(render_todo_panel(&todo_state)),
            )
            // Message list and streaming in a shared flex container
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .max_w(px(768.0))
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    // Message history (shrinks when streaming)
                    .child(
                        div()
                            .when(streaming_view.is_some(), |el| el.flex_shrink())
                            .when(streaming_view.is_none(), |el| el.flex_1())
                            .w_full()
                            .min_h(px(100.0))
                            .overflow_hidden()
                            .child(list(self.list_state.clone(), render_item).size_full()),
                    )
                    // Streaming message (grows to fill remaining space)
                    .when_some(streaming_view, |el, view| {
                        el.child(
                            div()
                                .id("streaming-message")
                                .flex_1()
                                .w_full()
                                .min_h(px(200.0))
                                .overflow_y_scroll()
                                .px(px(16.0))
                                .child(view)
                        )
                    }),
            )
            // Message input
            .child(
                div()
                    .w_full()
                    .max_w(px(768.0))
                    .p(px(16.0))
                    .border_t_1()
                    .border_color(border::DEFAULT)
                    .flex()
                    .flex_row()
                    .gap(px(8.0))
                    .child(div().flex_1().child(self.message_input.clone()))
                    .child(if is_streaming {
                        Button::new("Cancel")
                            .variant(ButtonVariant::Secondary)
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.cancel(cx);
                            }))
                            .into_any_element()
                    } else {
                        Button::new("Send")
                            .variant(ButtonVariant::Default)
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.send_message(cx);
                            }))
                            .into_any_element()
                    }),
            )
            // Status indicator (top left corner)
            .child(self.render_status(cx))
    }
}
