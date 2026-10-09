//! ShimmerText, ported from Apps SDK UI `src/components/ShimmerText` (MIT).
//! Styles: `static/components/shimmer-text.css`.

use maud::{Markup, Render, html};

use super::html::{Attrs, Tag};

/// The element ShimmerText renders as (default `Div`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ShimmerTag {
    #[default]
    Div,
    Span,
    P,
    H1,
    H2,
    H3,
    H4,
    H5,
    H6,
}

impl ShimmerTag {
    fn name(self) -> &'static str {
        match self {
            Self::Div => "div",
            Self::Span => "span",
            Self::P => "p",
            Self::H1 => "h1",
            Self::H2 => "h2",
            Self::H3 => "h3",
            Self::H4 => "h4",
            Self::H5 => "h5",
            Self::H6 => "h6",
        }
    }
}

/// Text with a moving highlight, for in-progress labels ("Thinking").
/// `idle(true)` (`data-idle`) stops the animation; an HTMX swap or a script
/// can toggle the attribute, matching React's ShimmerableText.
#[derive(Clone, Debug)]
pub struct ShimmerText {
    text: String,
    tag: ShimmerTag,
    idle: bool,
    attrs: Attrs,
}

impl ShimmerText {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            tag: ShimmerTag::Div,
            idle: false,
            attrs: Attrs::default(),
        }
    }

    pub fn tag(mut self, tag: ShimmerTag) -> Self {
        self.tag = tag;
        self
    }

    pub fn idle(mut self, idle: bool) -> Self {
        self.idle = idle;
        self
    }
}

impl_attrs!(ShimmerText);

impl Render for ShimmerText {
    fn render(&self) -> Markup {
        Tag::new(self.tag.name(), "oa-shimmer-text", &self.attrs)
            .flag("data-idle", self.idle)
            .extra(&self.attrs)
            .close(html! { (self.text) })
    }
}
