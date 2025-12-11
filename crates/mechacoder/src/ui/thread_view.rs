//! Thread view component for displaying conversation.

use acp::{AcpThread, AcpThreadEvent, ThreadEntry, ThreadStatus};
use gpui::{
    div, list, prelude::*, px, App, Context, Entity, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ListState, ParentElement, Render, Styled, Subscription, Window,
};
use theme::{bg, border, text};
use ui::{Button, ButtonVariant, TextInput};

use super::message_input::MessageInput;
use super::message_view::MessageView;
use super::tool_call_view::ToolCallView;

/// Thread view for displaying the conversation.
pub struct ThreadView {
    /// The ACP thread.
    thread: Entity<AcpThread>,
    /// Message input component.
    message_input: Entity<MessageInput>,
    /// Focus handle.
    focus_handle: FocusHandle,
    /// List state for entries.
    list_state: ListState,
    /// Subscription to thread events.
    _subscription: Subscription,
}

impl ThreadView {
    /// Create a new thread view.
    pub fn new(thread: Entity<AcpThread>, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();

        // Create message input
        let message_input = cx.new(|cx| MessageInput::new(cx));

        // Subscribe to thread events
        let subscription = cx.subscribe(&thread, |this, _thread, event, cx| {
            this.handle_thread_event(event, cx);
        });

        // Create list state
        let list_state = ListState::new(0, gpui::ListAlignment::Bottom, px(100.0), {
            let thread = thread.clone();
            move |ix, _window, cx| {
                let entries = thread.read(cx).entries();
                if ix < entries.len() {
                    Self::render_entry(&entries[ix], ix, cx).into_any_element()
                } else {
                    div().into_any_element()
                }
            }
        });

        Self {
            thread,
            message_input,
            focus_handle,
            list_state,
            _subscription: subscription,
        }
    }

    /// Handle a thread event.
    fn handle_thread_event(&mut self, event: &AcpThreadEvent, cx: &mut Context<Self>) {
        match event {
            AcpThreadEvent::EntryAdded(_) | AcpThreadEvent::EntryUpdated(_) => {
                // Update list count
                let entry_count = self.thread.read(cx).entries().len();
                self.list_state.reset(entry_count);
                cx.notify();
            }
            AcpThreadEvent::StatusChanged(_) => {
                cx.notify();
            }
            AcpThreadEvent::PermissionRequested { .. } => {
                cx.notify();
            }
            AcpThreadEvent::Error(error) => {
                log::error!("Thread error: {}", error);
                cx.notify();
            }
        }
    }

    /// Render a single entry.
    fn render_entry(entry: &ThreadEntry, _ix: usize, _cx: &App) -> impl IntoElement {
        match entry {
            ThreadEntry::UserMessage(msg) => {
                MessageView::user(&msg.content).into_any_element()
            }
            ThreadEntry::AssistantMessage(msg) => {
                let content = msg
                    .chunks
                    .iter()
                    .map(|chunk| match chunk {
                        acp::AssistantMessageChunk::Message { content } => content.clone(),
                        acp::AssistantMessageChunk::Thought { content } => {
                            format!("<thinking>{}</thinking>", content)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                MessageView::assistant(&content).into_any_element()
            }
            ThreadEntry::ToolCall(tc) => {
                ToolCallView::new(tc).into_any_element()
            }
        }
    }

    /// Send the current message.
    fn send_message(&mut self, cx: &mut Context<Self>) {
        let content = self.message_input.read(cx).content();
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
    fn respond_permission(&mut self, option_id: &str, cx: &mut Context<Self>) {
        self.thread.update(cx, |thread, cx| {
            thread.respond_permission(acp::acp::PermissionOptionId::new(option_id), cx);
        });
    }

    /// Render the permission prompt.
    fn render_permission_prompt(&self, cx: &App) -> Option<impl IntoElement> {
        let (tool_call, options) = self.thread.read(cx).pending_permission_info()?;

        Some(
            div()
                .p(px(16.0))
                .bg(bg::CARD)
                .border_1()
                .border_color(border::DEFAULT)
                .rounded(px(8.0))
                .flex()
                .flex_col()
                .gap(px(12.0))
                .child(
                    div()
                        .text_color(text::PRIMARY)
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(format!("Permission Required: {}", tool_call.fields.title.as_deref().unwrap_or("Unknown"))),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(8.0))
                        .children(options.iter().map(|opt| {
                            let _id = opt.option_id.to_string();
                            let label = opt.name.clone();
                            Button::new(label)
                                .variant(if opt.kind == acp::acp::PermissionOptionKind::AllowOnce
                                    || opt.kind == acp::acp::PermissionOptionKind::AllowAlways
                                {
                                    ButtonVariant::Default
                                } else {
                                    ButtonVariant::Secondary
                                })
                        })),
                ),
        )
    }

    /// Render the status bar.
    fn render_status(&self, cx: &App) -> impl IntoElement {
        let status = self.thread.read(cx).status();

        div()
            .px(px(16.0))
            .py(px(8.0))
            .border_t_1()
            .border_color(border::DEFAULT)
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(
                div().text_sm().text_color(text::SECONDARY).child(
                    match status {
                        ThreadStatus::Idle => "Ready",
                        ThreadStatus::Streaming => "Claude is typing...",
                        ThreadStatus::WaitingForConfirmation => "Waiting for permission",
                        ThreadStatus::Error(_) => "Error",
                    }
                    .to_string(),
                ),
            )
    }
}

impl Focusable for ThreadView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ThreadView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_streaming = matches!(self.thread.read(cx).status(), ThreadStatus::Streaming);
        let has_permission = self.thread.read(cx).has_pending_permission();

        div()
            .size_full()
            .flex()
            .flex_col()
            .track_focus(&self.focus_handle)
            // Message list
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .child(list(self.list_state.clone()).size_full()),
            )
            // Permission prompt
            .when(has_permission, |el| {
                if let Some(prompt) = self.render_permission_prompt(cx) {
                    el.child(div().p(px(16.0)).child(prompt))
                } else {
                    el
                }
            })
            // Message input
            .child(
                div()
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
            // Status bar
            .child(self.render_status(cx))
    }
}
