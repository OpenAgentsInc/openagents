//! LoadingIndicator, LoadingDots and CircularProgress, ported from Apps SDK
//! UI `src/components/Indicator` (MIT), and [`Busy`]: a spinner with its
//! words in one row. Styles: `static/components/indicator.css`.
//!
//! Every indicator renders as inline `<span>`s, never a `<div>`: a `<div>`
//! inside a `<p>` makes the browser close the paragraph, which pushed the
//! words under the spinner ("Working", "Loading your repositories").
//!
//! Sizes come from the inherited `--indicator-size` and
//! `--circular-progress-size` tokens (Button and Badge set them per size),
//! not from inline styles, so they work under `style-src 'self'`.

use maud::{Markup, Render, html};

use super::html::{Attrs, Tag};

/// A spinning ring. Standalone it is announced as `role="status"` with the
/// label "Loading"; `decorative()` hides it from assistive technology when
/// the surrounding control already says it is busy.
#[derive(Clone, Debug)]
pub struct LoadingIndicator {
    label: Option<String>,
    attrs: Attrs,
}

impl Default for LoadingIndicator {
    fn default() -> Self {
        Self::new()
    }
}

impl LoadingIndicator {
    pub fn new() -> Self {
        Self {
            label: Some("Loading".to_string()),
            attrs: Attrs::default(),
        }
    }

    /// The announced label (default "Loading").
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// `aria-hidden`, no status role.
    pub fn decorative(mut self) -> Self {
        self.label = None;
        self
    }
}

impl_attrs!(LoadingIndicator);

impl Render for LoadingIndicator {
    fn render(&self) -> Markup {
        status(
            Tag::new("span", "oa-loading-indicator", &self.attrs),
            &self.label,
        )
        .extra(&self.attrs)
        .close(html! {})
    }
}

fn status(tag: Tag, label: &Option<String>) -> Tag {
    match label {
        Some(label) => tag.attr("role", "status").attr("aria-label", label),
        None => tag.attr("aria-hidden", "true"),
    }
}

/// Three pulsing dots, sized to a line of text (for "assistant is typing").
#[derive(Clone, Debug)]
pub struct LoadingDots {
    label: Option<String>,
    attrs: Attrs,
}

impl Default for LoadingDots {
    fn default() -> Self {
        Self::new()
    }
}

impl LoadingDots {
    pub fn new() -> Self {
        Self {
            label: Some("Loading".to_string()),
            attrs: Attrs::default(),
        }
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn decorative(mut self) -> Self {
        self.label = None;
        self
    }
}

impl_attrs!(LoadingDots);

impl Render for LoadingDots {
    fn render(&self) -> Markup {
        status(
            Tag::new("span", "oa-loading-dots", &self.attrs),
            &self.label,
        )
        .extra(&self.attrs)
        .close(html! {
            span class="oa-loading-dots-dot" {}
            span class="oa-loading-dots-dot" {}
            span class="oa-loading-dots-dot" {}
        })
    }
}

/// A determinate progress ring (0 to 100). The progress is written as the
/// circle's `stroke-dashoffset` presentation attribute, which needs no inline
/// style. Apps SDK UI's simulated progress (a JS timer) is not ported.
#[derive(Clone, Debug)]
pub struct CircularProgress {
    progress: f32,
    label: Option<String>,
    attrs: Attrs,
}

impl CircularProgress {
    pub fn new(progress: f32) -> Self {
        Self {
            progress: if progress.is_finite() {
                progress.clamp(0.0, 100.0)
            } else {
                0.0
            },
            label: Some("Progress".to_string()),
            attrs: Attrs::default(),
        }
    }

    /// The accessible name of the progressbar (default "Progress").
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn decorative(mut self) -> Self {
        self.label = None;
        self
    }
}

impl_attrs!(CircularProgress);

impl Render for CircularProgress {
    fn render(&self) -> Markup {
        let offset = 50.0 - 50.0 * (self.progress / 100.0);
        let offset = format!("{}", (offset * 100.0).round() / 100.0);
        let now = format!("{}", self.progress.round());
        let tag = Tag::new("span", "oa-circular-progress", &self.attrs);
        let tag = match &self.label {
            Some(label) => tag
                .attr("role", "progressbar")
                .attr("aria-label", label)
                .attr("aria-valuemin", "0")
                .attr("aria-valuemax", "100")
                .attr("aria-valuenow", &now),
            None => tag.attr("aria-hidden", "true"),
        };
        tag.extra(&self.attrs).close(html! {
            svg viewBox="0 0 20 20" class="oa-circular-progress-track" data-no-autosize aria-hidden="true" {
                circle cx="10" cy="10" r="8" fill="none" {}
            }
            svg viewBox="0 0 20 20" class="oa-circular-progress-track-progress" data-no-autosize aria-hidden="true" {
                circle cx="10" cy="10" r="8" fill="none" stroke-dashoffset=(offset) {}
            }
        })
    }
}

/// A spinner and its words on one line, for "Working" or "Loading your
/// repositories". Inline (`<span>`), so it is valid inside a paragraph, a
/// button, or a list item, and the words never wrap under the spinner.
///
/// ```
/// use maud::Render;
/// use openagents_ui::actions::Busy;
/// let html = Busy::new("Working").render().into_string();
/// assert!(html.starts_with(r#"<span class="oa-busy" role="status""#));
/// assert!(html.contains(">Working</span>"));
/// assert!(!html.contains("<div"));
/// ```
#[derive(Clone, Debug)]
pub struct Busy {
    text: String,
}

impl Busy {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

impl Render for Busy {
    fn render(&self) -> Markup {
        html! {
            span class="oa-busy" role="status" {
                (LoadingIndicator::new().decorative())
                span class="oa-busy-text" { (self.text) }
            }
        }
    }
}
