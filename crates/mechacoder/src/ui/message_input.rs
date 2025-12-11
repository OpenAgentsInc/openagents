//! Message input component.

use gpui::{
    div, prelude::*, px, App, Context, Entity, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, Render, Styled, Window,
};
use theme::{bg, border, text};
use ui::TextInput;

/// Message input component.
pub struct MessageInput {
    /// The text input.
    input: Entity<TextInput>,
    /// Focus handle.
    focus_handle: FocusHandle,
}

impl MessageInput {
    /// Create a new message input.
    pub fn new(cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let input = cx.new(|cx| TextInput::new("Type a message...", cx));

        Self {
            input,
            focus_handle,
        }
    }

    /// Get the current content.
    pub fn content(&self) -> String {
        // Note: TextInput may not have a content() method
        // This is a placeholder - would need to adapt based on actual API
        String::new()
    }

    /// Clear the input.
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        // Note: TextInput may not have a clear() method
        // This is a placeholder - would need to adapt based on actual API
        cx.notify();
    }

    /// Set the content.
    pub fn set_content(&mut self, content: &str, cx: &mut Context<Self>) {
        // Note: TextInput may not have this method
        // This is a placeholder
        cx.notify();
    }
}

impl Focusable for MessageInput {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for MessageInput {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w_full()
            .track_focus(&self.focus_handle)
            .child(self.input.clone())
    }
}
