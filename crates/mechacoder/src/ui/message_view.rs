//! Message view component for displaying messages.

use acp::ContentBlock;
use gpui::{div, px, IntoElement, ParentElement, Styled};
use theme_oa::{bg, border, text};

/// Message view for displaying a single message.
pub struct MessageView {
    /// The role (user or assistant).
    role: MessageRole,
    /// The message content.
    content: String,
}

/// Message role.
#[derive(Clone, Debug, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
}

impl MessageView {
    /// Create a new user message view.
    pub fn user(content: &ContentBlock) -> Self {
        let text = match content {
            ContentBlock::Text(t) => t.clone(),
            ContentBlock::Image { .. } => "[Image]".to_string(),
        };
        Self {
            role: MessageRole::User,
            content: text,
        }
    }

    /// Create a new assistant message view.
    pub fn assistant(content: &str) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.to_string(),
        }
    }
}

impl IntoElement for MessageView {
    type Element = gpui::Div;

    fn into_element(self) -> Self::Element {
        let is_user = self.role == MessageRole::User;

        div()
            .px(px(16.0))
            .py(px(12.0))
            .flex()
            .flex_col()
            .gap(px(4.0))
            // Role label
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(if is_user {
                        text::PRIMARY
                    } else {
                        text::SECONDARY
                    })
                    .child(if is_user { "You" } else { "Claude" }),
            )
            // Message content
            .child(
                div()
                    .px(px(12.0))
                    .py(px(8.0))
                    .rounded(px(8.0))
                    .bg(if is_user { bg::CARD } else { bg::SURFACE })
                    .border_1()
                    .border_color(border::DEFAULT)
                    .text_color(text::PRIMARY)
                    .child(self.content),
            )
    }
}
