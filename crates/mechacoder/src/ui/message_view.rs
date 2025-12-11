//! Message view component for displaying messages with markdown rendering.

use acp::ContentBlock;
use gpui::{
    div, px, App, AppContext, Context, Entity, IntoElement, ParentElement, Refineable, Render,
    Styled, TextStyle, TextStyleRefinement, UnderlineStyle, Window,
};
use markdown::{Markdown, MarkdownElement, MarkdownStyle};
use theme_oa::{bg, border, text};

/// Message view for displaying a single message with markdown.
pub struct MessageView {
    /// The role (user or assistant).
    role: MessageRole,
    /// The markdown entity for rendering.
    markdown: Entity<Markdown>,
}

/// Message role.
#[derive(Clone, Debug, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
}

impl MessageView {
    /// Create a new user message view.
    pub fn user(content: &ContentBlock, cx: &mut App) -> Entity<Self> {
        let text = match content {
            ContentBlock::Text(t) => t.clone(),
            ContentBlock::Image { .. } => "[Image]".to_string(),
        };
        cx.new(|cx| {
            let markdown = cx.new(|cx| Markdown::new(text.into(), None, None, cx));
            Self {
                role: MessageRole::User,
                markdown,
            }
        })
    }

    /// Create a new assistant message view.
    pub fn assistant(content: &str, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let markdown = cx.new(|cx| Markdown::new(content.to_string().into(), None, None, cx));
            Self {
                role: MessageRole::Assistant,
                markdown,
            }
        })
    }

    /// Update the content of this message view.
    pub fn update_content(&mut self, content: &str, cx: &mut Context<Self>) {
        self.markdown.update(cx, |md, cx| {
            md.reset(content.to_string().into(), cx);
        });
    }

    /// Get the markdown style.
    fn markdown_style(&self, _window: &Window, cx: &App) -> MarkdownStyle {
        let colors = theme::ActiveTheme::theme(cx).colors();

        let mut text_style = TextStyle::default();
        text_style.refine(&TextStyleRefinement {
            color: Some(text::PRIMARY),
            ..Default::default()
        });

        MarkdownStyle {
            base_text_style: text_style,
            code_block: gpui::StyleRefinement::default()
                .px(px(12.0))
                .py(px(8.0))
                .rounded(px(6.0))
                .bg(bg::SURFACE),
            inline_code: TextStyleRefinement {
                background_color: Some(bg::SURFACE),
                ..Default::default()
            },
            block_quote: TextStyleRefinement {
                color: Some(text::SECONDARY),
                ..Default::default()
            },
            link: TextStyleRefinement {
                color: Some(colors.link_text_hover),
                underline: Some(UnderlineStyle {
                    thickness: px(1.),
                    color: Some(colors.link_text_hover),
                    wavy: false,
                }),
                ..Default::default()
            },
            rule_color: border::DEFAULT,
            block_quote_border_color: border::DEFAULT,
            syntax: theme::ActiveTheme::theme(cx).syntax().clone(),
            selection_background_color: colors.text_accent.opacity(0.3),
            ..Default::default()
        }
    }
}

impl Render for MessageView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_user = self.role == MessageRole::User;
        let style = self.markdown_style(window, cx);

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
            // Message content with markdown
            .child(
                div()
                    .px(px(12.0))
                    .py(px(8.0))
                    .rounded(px(8.0))
                    .bg(if is_user { bg::CARD } else { bg::SURFACE })
                    .border_1()
                    .border_color(border::DEFAULT)
                    .text_color(text::PRIMARY)
                    .child(MarkdownElement::new(self.markdown.clone(), style)),
            )
    }
}

/// Simple text message view (for cases where markdown isn't needed).
pub struct SimpleMessageView {
    /// The role (user or assistant).
    role: MessageRole,
    /// The message content.
    content: String,
}

impl SimpleMessageView {
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

impl IntoElement for SimpleMessageView {
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
