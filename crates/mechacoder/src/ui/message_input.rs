//! Message input component.

use gpui::{
    div, prelude::*, App, Context, Entity, EventEmitter, FocusHandle, Focusable,
    IntoElement, ParentElement, Render, Styled, Subscription, Window,
};
use ui_oa::text_input::{SubmitEvent, TextInput};

/// Event emitted when a message should be sent.
#[derive(Clone)]
pub struct SendMessageEvent(pub String);

/// Message input component.
pub struct MessageInput {
    /// The text input.
    input: Entity<TextInput>,
    /// Focus handle.
    focus_handle: FocusHandle,
    /// Subscription to input events.
    _subscription: Subscription,
}

impl MessageInput {
    /// Create a new message input.
    pub fn new(cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let input = cx.new(|cx| TextInput::new("Type a message...", cx));

        // Subscribe to submit events from the text input
        let subscription = cx.subscribe(&input, |this, _input, event: &SubmitEvent, cx| {
            // Forward the submit event
            cx.emit(SendMessageEvent(event.0.clone()));
            // Clear the input
            this.clear(cx);
        });

        Self {
            input,
            focus_handle,
            _subscription: subscription,
        }
    }

    /// Get the current content.
    pub fn content(&self, cx: &App) -> String {
        self.input.read(cx).content().to_string()
    }

    /// Clear the input.
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| {
            input.clear(cx);
        });
    }

    /// Set the content.
    pub fn set_content(&mut self, content: impl Into<gpui::SharedString>, cx: &mut Context<Self>) {
        let content = content.into();
        self.input.update(cx, |input, cx| {
            input.set_content(content, cx);
        });
    }

    /// Focus the input.
    pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        let handle = self.input.read(cx).focus_handle(cx);
        window.focus(&handle);
    }
}

impl Focusable for MessageInput {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<SendMessageEvent> for MessageInput {}

impl Render for MessageInput {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w_full()
            .track_focus(&self.focus_handle)
            .child(self.input.clone())
    }
}
