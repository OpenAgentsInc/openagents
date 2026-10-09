//! Badge, ported from Apps SDK UI `src/components/Badge` (MIT).
//! Styles: `static/components/badge.css`.

use maud::{Markup, Render, html};

use super::html::{Attrs, Tag};
use super::{Color, Variant};

/// Badge height: `Sm` 18px, `Md` 22px, `Lg` 24px.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BadgeSize {
    #[default]
    Sm,
    Md,
    Lg,
}

impl BadgeSize {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sm => "sm",
            Self::Md => "md",
            Self::Lg => "lg",
        }
    }
}

/// A small status label. Colors: Secondary (default), Success, Danger,
/// Warning, Info, Discovery. Variants: Soft (default), Solid, Outline.
///
/// Rendered as a `<span>` (React uses a `<div>`) so it is valid inside
/// buttons, links and paragraphs; the styles are class-based either way.
#[derive(Clone, Debug)]
pub struct Badge {
    label: String,
    color: Color,
    variant: Variant,
    size: BadgeSize,
    pill: bool,
    start: Option<Markup>,
    end: Option<Markup>,
    attrs: Attrs,
}

impl Badge {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            color: Color::Secondary,
            variant: Variant::Soft,
            size: BadgeSize::Sm,
            pill: false,
            start: None,
            end: None,
            attrs: Attrs::default(),
        }
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    pub fn variant(mut self, variant: Variant) -> Self {
        self.variant = variant;
        self
    }

    pub fn size(mut self, size: BadgeSize) -> Self {
        self.size = size;
        self
    }

    pub fn pill(mut self, pill: bool) -> Self {
        self.pill = pill;
        self
    }

    /// Leading icon or indicator (for example a `LoadingIndicator`).
    pub fn icon_start(mut self, icon: impl Render) -> Self {
        self.start = Some(icon.render());
        self
    }

    pub fn icon_end(mut self, icon: impl Render) -> Self {
        self.end = Some(icon.render());
        self
    }
}

impl_attrs!(Badge);

impl Render for Badge {
    fn render(&self) -> Markup {
        let has_icons = self.start.is_some() || self.end.is_some();
        Tag::new("span", "oa-badge", &self.attrs)
            .attr("data-color", self.color.as_str())
            .attr("data-size", self.size.as_str())
            .flag("data-pill", self.pill)
            .attr("data-variant", self.variant.as_str())
            .extra(&self.attrs)
            .close(html! {
                @if let Some(start) = &self.start { (start) }
                @if has_icons { span { (self.label) } } @else { (self.label) }
                @if let Some(end) = &self.end { (end) }
            })
    }
}
