//! Alert component for displaying messages

use gpui::prelude::*;
use gpui::*;
use theme_oa::{bg, text, border, status};

/// Alert variant styles
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum AlertVariant {
    #[default]
    Default,
    Destructive,
}

/// An alert box for displaying important messages
///
/// # Example
/// ```
/// Alert::new("Heads up!").description("You can add components.")
/// Alert::new("Error").variant(AlertVariant::Destructive).description("Something went wrong.")
/// ```
#[derive(IntoElement)]
pub struct Alert {
    title: SharedString,
    description: Option<SharedString>,
    variant: AlertVariant,
}

impl Alert {
    /// Create a new alert with a title
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            description: None,
            variant: AlertVariant::Default,
        }
    }

    /// Set the alert description
    pub fn description(mut self, desc: impl Into<SharedString>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Set the alert variant
    pub fn variant(mut self, variant: AlertVariant) -> Self {
        self.variant = variant;
        self
    }
}

impl RenderOnce for Alert {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let (bg_color, text_color, border_color) = match self.variant {
            AlertVariant::Default => (bg::SURFACE, text::PRIMARY, border::DEFAULT),
            AlertVariant::Destructive => (status::ERROR_BG, status::ERROR, status::ERROR_BORDER),
        };

        let mut content = div().flex().flex_col().gap(px(4.0));

        // Title
        content = content.child(
            div()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .child(self.title)
        );

        // Description
        if let Some(desc) = self.description {
            content = content.child(
                div()
                    .text_sm()
                    .opacity(0.9)
                    .child(desc)
            );
        }

        div()
            .w_full()
            .p(px(16.0))
            .rounded(px(8.0))
            .bg(bg_color)
            .text_color(text_color)
            .border_1()
            .border_color(border_color)
            .child(content)
    }
}
