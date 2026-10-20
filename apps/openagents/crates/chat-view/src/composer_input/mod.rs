//! The desktop composer input, shared with browser hosts.
//!
//! `shared.rs` also compiles inside the desktop composer so its native host
//! can extend the input with dictation without exporting editing internals.
//! Both hosts use the same selection, IME, undo, chip layout, and paint code.

// Some editing helpers are used only by the desktop host of shared.rs.
#![allow(dead_code)]

use crate::composer_chips::*;
use crate::{
    composer_markdown::{self, in_code},
    composer_mentions::TextProjection,
    motion,
    theme::Theme,
};
use gpui::{
    AnyTooltip, App, Bounds, ClipboardEntry, ClipboardItem, Context, CursorStyle, DispatchPhase,
    ElementInputHandler, Entity, EntityInputHandler, EventEmitter, FocusHandle, Focusable,
    GlobalElementId, KeyBinding, KeyDownEvent, LayoutId, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point, Role, ScrollWheelEvent, SharedString,
    Style, Task, TextRun, TextStyle, UTF16Selection, UnderlineStyle, Window, WrappedLine, actions,
    div, fill, point, prelude::*, px, relative, size,
};
use std::{
    collections::{HashMap, HashSet},
    ops::Range,
    path::PathBuf,
    rc::Rc,
    time::Duration,
};
use unicode_segmentation::UnicodeSegmentation;
use web_time::Instant;

pub const TEXTAREA_MAX: f32 = 260.;
pub const TEXTAREA_PAD_V: f32 = 20.;
#[derive(Clone, Copy)]
pub enum ComposerSendBehavior {
    Enter,
    ModEnter,
}
fn platform_combo(combo: &str) -> String {
    combo.replace("mod-", "ctrl-")
}

struct ChipAttachment {
    image: Option<std::sync::Arc<gpui::Image>>,
}

/// Browser inputs receive microphone transcripts from their web host.
mod input_host {
    use super::*;
    #[derive(Default)]
    pub struct State;
    pub fn note_input() {}
    pub fn decorate(
        element: gpui::Stateful<gpui::Div>,
        _: &Context<ComposerInput>,
    ) -> gpui::Stateful<gpui::Div> {
        element
    }
}
include!("shared.rs");
define_input_actions!(web_composer);
impl ComposerInput {
    fn cancel_dictation(&mut self) {}
    fn dictation_active(&self) -> bool {
        false
    }
    fn dictation_changed(&self, _: &mut Context<Self>) {}
}

impl ComposerInput {
    /// A message field with the desktop's inline reference chips.
    pub fn message(placeholder: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        let mut input = Self::with_context(placeholder, MESSAGE_COMPOSER_CONTEXT, cx);
        input.copies_transcript_selection = true;
        input.enable_mentions();
        input
    }

    pub fn configure_message(
        &mut self,
        send_with_modifier: bool,
        completing: bool,
        cx: &mut Context<Self>,
    ) {
        self.set_key_context(
            if send_with_modifier {
                "WebMessageModEnter"
            } else {
                MESSAGE_COMPOSER_CONTEXT
            },
            cx,
        );
        self.set_mention_controls(completing, completing, cx);
    }

    pub fn composing(&self) -> bool {
        self.marked_range.is_some()
    }

    /// Insert an accepted completion as one undo step, preserving its exact text.
    pub fn replace_completion_range(
        &mut self,
        range: Range<usize>,
        text: &str,
        cx: &mut Context<Self>,
    ) {
        if self.read_only
            || self.marked_range.is_some()
            || range.start > range.end
            || !self.content.is_char_boundary(range.start)
            || !self.content.is_char_boundary(range.end)
        {
            return;
        }
        self.record_edit(&range, text);
        self.content.replace_range(range.clone(), text);
        self.edit_revision = self.edit_revision.wrapping_add(1);
        self.move_to(range.start + text.len(), cx);
        self.refresh_projection();
        self.needs_measure = true;
        self.last_edit = None;
        cx.emit(ComposerInputEvent::Edited);
        cx.notify();
    }
}

/// Install both send-key choices; a field selects its own context.
pub fn bind_message_keys(cx: &mut App) {
    let context = MESSAGE_COMPOSER_CONTEXT;
    let mut keys = input_bindings(context);
    keys.push(KeyBinding::new("enter", Submit, Some(context)));
    cx.bind_keys(keys);
    let context = "WebMessageModEnter";
    let mut keys = input_bindings(context);
    keys.push(KeyBinding::new(
        "enter",
        MessageNewlineOrAccept,
        Some(context),
    ));
    for prefix in ["cmd", "ctrl"] {
        keys.push(KeyBinding::new(
            &format!("{prefix}-enter"),
            ModifiedSubmit,
            Some(context),
        ));
        keys.push(KeyBinding::new(
            &format!("{prefix}-enter"),
            ModifiedSubmit,
            Some(MESSAGE_COMPOSER_CONTEXT),
        ));
    }
    cx.bind_keys(keys);
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::AppContext;

    #[gpui::test]
    fn completion_replacement_preserves_undo_and_unicode_boundaries(cx: &mut gpui::TestAppContext) {
        let input = cx.new(|cx| ComposerInput::message("Ask anything", cx));
        input.update(cx, |input, cx| {
            input.set_text("🦀 @file", cx);
            input.replace_completion_range(
                5..10,
                "[src/lib.rs](openagents-file:///src/lib.rs) ",
                cx,
            );
            assert!(input.text().starts_with("🦀 [src/lib.rs]"));
            assert_eq!(input.cursor_offset(), input.text().len());
            let before = input.undo_stack.pop().unwrap();
            input.restore(before, cx);
            assert_eq!(input.text(), "🦀 @file");
            input.replace_completion_range(1..2, "bad", cx);
            assert_eq!(input.text(), "🦀 @file");
            input.marked_range = Some(5..10);
            input.replace_completion_range(5..10, "bad", cx);
            assert_eq!(input.text(), "🦀 @file");
        });
    }
}
