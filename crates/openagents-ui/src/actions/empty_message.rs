//! EmptyMessage, ported from Apps SDK UI `src/components/EmptyMessage`
//! (MIT). Styles: `static/components/empty-message.css`.

use maud::{Markup, Render, html};

use super::Color;
use super::html::{Attrs, Tag};

/// How the container fills the space around it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum EmptyMessageFill {
    /// Full width and height of the parent.
    #[default]
    Static,
    /// Absolutely positioned over the nearest positioned ancestor.
    Absolute,
    None,
}

impl EmptyMessageFill {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Absolute => "absolute",
            Self::None => "none",
        }
    }
}

/// Icon badge size: `Sm` 32px, `Md` 40px (default).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum EmptyMessageIconSize {
    Sm,
    #[default]
    Md,
}

impl EmptyMessageIconSize {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sm => "sm",
            Self::Md => "md",
        }
    }
}

/// A centered empty state: icon badge, title, description and an action
/// row. Icon and title colors: Secondary (default), Danger, Warning.
#[derive(Clone, Debug, Default)]
pub struct EmptyMessage {
    fill: EmptyMessageFill,
    icon: Option<Markup>,
    icon_size: EmptyMessageIconSize,
    icon_color: Option<Color>,
    title: Option<String>,
    title_color: Option<Color>,
    description: Option<String>,
    actions: Option<Markup>,
    attrs: Attrs,
}

impl EmptyMessage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fill(mut self, fill: EmptyMessageFill) -> Self {
        self.fill = fill;
        self
    }

    pub fn icon(mut self, icon: impl Render) -> Self {
        self.icon = Some(icon.render());
        self
    }

    pub fn icon_size(mut self, size: EmptyMessageIconSize) -> Self {
        self.icon_size = size;
        self
    }

    pub fn icon_color(mut self, color: Color) -> Self {
        self.icon_color = Some(color);
        self
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn title_color(mut self, color: Color) -> Self {
        self.title_color = Some(color);
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// The action row (for example a Button).
    pub fn actions(mut self, actions: impl Render) -> Self {
        self.actions = Some(actions.render());
        self
    }
}

impl_attrs!(EmptyMessage);

impl Render for EmptyMessage {
    fn render(&self) -> Markup {
        let icon_color = self.icon_color.unwrap_or(Color::Secondary).as_str();
        let title_color = self.title_color.unwrap_or(Color::Secondary).as_str();
        Tag::new("div", "oa-empty-message", &self.attrs)
            .attr("data-fill", self.fill.as_str())
            .extra(&self.attrs)
            .close(html! {
                @if let Some(icon) = &self.icon {
                    div class="oa-empty-message-icon" data-size=(self.icon_size.as_str()) data-color=(icon_color) aria-hidden="true" { (icon) }
                }
                @if let Some(title) = &self.title {
                    div class="oa-empty-message-title" data-color=(title_color) { (title) }
                }
                @if let Some(description) = &self.description {
                    div class="oa-empty-message-description" { (description) }
                }
                @if let Some(actions) = &self.actions {
                    div class="oa-empty-message-action-row" { (actions) }
                }
            })
    }
}
