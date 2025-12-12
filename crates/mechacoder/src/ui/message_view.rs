//! Message view component for displaying messages with markdown rendering.

use gpui::{
    div, prelude::*, px, App, AppContext, Context, Entity, IntoElement, ParentElement, Refineable, Render,
    SharedString, Styled, TextStyle, TextStyleRefinement, UnderlineStyle, Window,
};
use markdown::{Markdown, MarkdownElement, MarkdownStyle};
use theme_oa::{bg, border, text, FONT_FAMILY};

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
    pub fn user(content: &str, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let markdown = cx.new(|cx| Markdown::new(content.to_string().into(), None, None, cx));
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

    /// Update the content of this message view (for streaming).
    pub fn update_content(&mut self, content: &str, cx: &mut Context<Self>) {
        self.markdown.update(cx, |md, cx| {
            // Use replace instead of reset to avoid clearing content before re-parse
            md.replace(content.to_string(), cx);
        });
    }

    /// Get the markdown style.
    fn markdown_style(&self, _window: &Window, _cx: &App) -> MarkdownStyle {
        // Use theme_oa colors directly instead of Zed's global theme
        let link_color = gpui::hsla(210.0 / 360.0, 0.8, 0.6, 1.0); // Blue link color

        let mut text_style = TextStyle::default();
        text_style.font_family = SharedString::from(FONT_FAMILY);
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
                color: Some(link_color),
                underline: Some(UnderlineStyle {
                    thickness: px(1.),
                    color: Some(link_color),
                    wavy: false,
                }),
                ..Default::default()
            },
            rule_color: border::DEFAULT,
            block_quote_border_color: border::DEFAULT,
            syntax: std::sync::Arc::new(theme::SyntaxTheme::default()),
            selection_background_color: gpui::hsla(210.0 / 360.0, 0.8, 0.6, 0.3),
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
            .when(is_user, |el| el.justify_end())
            // Message content with markdown
            .child(
                div()
                    .max_w(px(600.0))
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
    pub fn user(content: &str) -> Self {
        Self {
            role: MessageRole::User,
            content: content.to_string(),
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

        let mut container = div()
            .px(px(16.0))
            .py(px(12.0))
            .font_family(FONT_FAMILY)
            .flex();

        if is_user {
            container = container.justify_end();
        }

        container.child(
            div()
                .max_w(px(600.0))
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
