//! Browser events and draft access for the desktop's composer input.
use crate::composer::{Cancelled, CompletionKey, Edited, Submitted};
use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render, SharedString,
    Window, div, prelude::*,
};
use openagents_chat_view::composer_input::{self, ComposerInput, ComposerInputEvent};
use std::ops::Range;

pub struct MessageInput {
    input: Entity<ComposerInput>,
    send_with_modifier: bool,
    completing: bool,
}

impl MessageInput {
    pub fn new(placeholder: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        let placeholder = placeholder.into();
        let input = cx.new(|cx| ComposerInput::message(placeholder.clone(), cx));
        cx.subscribe(&input, |_, _, _, cx| cx.notify()).detach();
        cx.subscribe(
            &input,
            |this, _, event: &ComposerInputEvent, cx| match event {
                ComposerInputEvent::Submitted | ComposerInputEvent::ModifiedSubmitted => {
                    if !this.composing(cx) && !this.is_empty(cx) {
                        let text = this.take(cx);
                        cx.emit(Submitted(text));
                    }
                }
                ComposerInputEvent::Edited | ComposerInputEvent::CursorMoved => cx.emit(Edited),
                ComposerInputEvent::MentionNavigate(direction) => cx.emit(if *direction < 0 {
                    CompletionKey::Up
                } else {
                    CompletionKey::Down
                }),
                ComposerInputEvent::MentionAccept => cx.emit(CompletionKey::Accept),
                ComposerInputEvent::MentionDismiss => cx.emit(CompletionKey::Dismiss),
                _ => {}
            },
        )
        .detach();
        Self {
            input,
            send_with_modifier: false,
            completing: false,
        }
    }
    pub fn text(&self, cx: &App) -> String {
        self.input.read(cx).text().to_owned()
    }
    pub fn is_empty(&self, cx: &App) -> bool {
        self.input.read(cx).is_empty()
    }
    pub fn cursor(&self, cx: &App) -> usize {
        self.input.read(cx).cursor_offset()
    }
    pub fn composing(&self, cx: &App) -> bool {
        self.input.read(cx).composing()
    }
    pub fn set_send_with_modifier(&mut self, on: bool, cx: &mut Context<Self>) {
        self.send_with_modifier = on;
        self.input.update(cx, |input, cx| {
            input.configure_message(on, self.completing, cx)
        });
    }
    pub fn set_completing(&mut self, on: bool, cx: &mut Context<Self>) {
        self.completing = on;
        self.input.update(cx, |input, cx| {
            input.configure_message(self.send_with_modifier, on, cx)
        });
    }
    pub fn set_placeholder(
        &mut self,
        placeholder: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.input
            .update(cx, |input, cx| input.set_placeholder(placeholder, cx));
    }
    pub fn set_text(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        let text = text.into();
        self.input
            .update(cx, |input, cx| input.set_text(text.to_string(), cx));
    }
    pub fn replace_range(&mut self, range: Range<usize>, text: &str, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| {
            input.replace_completion_range(range, text, cx)
        });
    }
    pub fn take(&mut self, cx: &mut Context<Self>) -> String {
        let text = self.text(cx);
        self.set_text("", cx);
        text
    }
}
impl EventEmitter<Submitted> for MessageInput {}
impl EventEmitter<Cancelled> for MessageInput {}
impl EventEmitter<Edited> for MessageInput {}
impl EventEmitter<CompletionKey> for MessageInput {}
impl Focusable for MessageInput {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.focus_handle(cx)
    }
}
impl Render for MessageInput {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w_full()
            .min_w_0()
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" && !this.completing {
                    cx.emit(Cancelled);
                }
            }))
            .child(self.input.clone())
    }
}
pub fn bind_keys(cx: &mut App) {
    composer_input::bind_message_keys(cx);
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::AppContext;
    use std::{cell::RefCell, rc::Rc};

    #[gpui::test]
    fn restored_draft_keeps_caret_at_end_and_submits_once(cx: &mut gpui::TestAppContext) {
        let input = cx.new(|cx| MessageInput::new("Ask anything", cx));
        let sent = Rc::new(RefCell::new(Vec::new()));
        let received = sent.clone();
        cx.update(|cx| {
            cx.subscribe(&input, move |_, event: &Submitted, _| {
                received.borrow_mut().push(event.0.clone())
            })
        })
        .detach();
        input.update(cx, |input, cx| input.set_text("a draft 🦀", cx));
        input.read_with(cx, |input, cx| {
            assert_eq!(input.cursor(cx), "a draft 🦀".len())
        });
        let editor = input.read_with(cx, |input, _| input.input.clone());
        editor.update(cx, |_, cx| cx.emit(ComposerInputEvent::Submitted));
        editor.update(cx, |_, cx| cx.emit(ComposerInputEvent::Submitted));
        assert_eq!(*sent.borrow(), vec!["a draft 🦀"]);
        input.read_with(cx, |input, cx| assert!(input.is_empty(cx)));
    }

    #[gpui::test]
    fn completion_keys_do_not_send_the_draft(cx: &mut gpui::TestAppContext) {
        let input = cx.new(|cx| MessageInput::new("Ask anything", cx));
        let keys = Rc::new(RefCell::new(Vec::new()));
        let received = keys.clone();
        cx.update(|cx| {
            cx.subscribe(&input, move |_, key: &CompletionKey, _| {
                received.borrow_mut().push(*key)
            })
        })
        .detach();
        input.update(cx, |input, cx| input.set_text("@file", cx));
        let editor = input.read_with(cx, |input, _| input.input.clone());
        for event in [
            ComposerInputEvent::MentionNavigate(-1),
            ComposerInputEvent::MentionNavigate(1),
            ComposerInputEvent::MentionAccept,
            ComposerInputEvent::MentionDismiss,
        ] {
            editor.update(cx, |_, cx| cx.emit(event));
        }
        assert_eq!(
            *keys.borrow(),
            vec![
                CompletionKey::Up,
                CompletionKey::Down,
                CompletionKey::Accept,
                CompletionKey::Dismiss
            ]
        );
        input.read_with(cx, |input, cx| assert_eq!(input.text(cx), "@file"));
    }
}
