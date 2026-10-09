//! Agent activity inside a conversation: tool calls, groups of them,
//! progress steps, a result card, and a changed-files summary.
//!
//! An assistant turn in a coding or setup conversation is more than prose.
//! These builders render what the agent did as compact, scannable rows:
//!
//! - [`ToolCall`]: one call as a row (icon, verb, mono detail, status
//!   badge). With a body it is a native `<details>` that expands to the
//!   call's input and output, so it works without JavaScript.
//! - [`ToolGroup`]: several calls folded under one summary line
//!   ("Explored the repository · 9 calls").
//! - [`Steps`]: an ordered progress list, each [`Step`] done, running,
//!   failed, or waiting.
//! - [`ResultCard`]: a bordered card for an outcome to review (title,
//!   status badge, facts, body, footer).
//! - [`FileChanges`]: changed paths with added and removed line counts.
//!
//! Styles live in `static/components/activity.css`. Every builder escapes
//! the text it takes.

use maud::{Markup, Render, html};

use crate::actions::{Badge, Color, LoadingIndicator, Variant};
use crate::icons::{Icon, IconSize};

/// Where a call or step stands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ActivityStatus {
    /// Finished successfully.
    #[default]
    Done,
    /// Still running.
    Running,
    /// Finished with a failure.
    Failed,
    /// Not started.
    Waiting,
}

impl ActivityStatus {
    /// The `data-status` value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Done => "done",
            Self::Running => "running",
            Self::Failed => "failed",
            Self::Waiting => "waiting",
        }
    }

    /// The default badge label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Done => "Done",
            Self::Running => "Running",
            Self::Failed => "Failed",
            Self::Waiting => "Waiting",
        }
    }

    const fn color(self) -> Color {
        match self {
            Self::Done => Color::Success,
            Self::Running => Color::Info,
            Self::Failed => Color::Danger,
            Self::Waiting => Color::Secondary,
        }
    }

    /// A small soft badge for this status, reading `label`.
    #[must_use]
    pub fn badge(self, label: impl Into<String>) -> Badge {
        Badge::new(label)
            .color(self.color())
            .variant(Variant::Soft)
            .size(crate::actions::BadgeSize::Sm)
    }

    fn marker(self) -> Markup {
        match self {
            Self::Done => Icon::CheckCircleFilled.size(IconSize::Sm).render(),
            Self::Running => LoadingIndicator::new().decorative().render(),
            Self::Failed => Icon::ExclamationMarkCircle.size(IconSize::Sm).render(),
            Self::Waiting => Icon::EmptyCircle.size(IconSize::Sm).render(),
        }
    }
}

/// One tool call: `details.oa-tool-call` (or `div` without a body).
#[derive(Clone, Debug)]
pub struct ToolCall {
    icon: Icon,
    title: String,
    detail: Option<String>,
    status: ActivityStatus,
    status_label: Option<String>,
    body: Option<Markup>,
    open: bool,
    id: Option<String>,
}

impl ToolCall {
    /// A call shown as `icon` and `title` ("Read", "Ran", "Edited").
    #[must_use]
    pub fn new(icon: Icon, title: impl Into<String>) -> Self {
        Self {
            icon,
            title: title.into(),
            detail: None,
            status: ActivityStatus::Done,
            status_label: None,
            body: None,
            open: false,
            id: None,
        }
    }

    /// The monospace detail after the title: a path, a command, a query.
    #[must_use]
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// The status (default [`ActivityStatus::Done`]).
    #[must_use]
    pub fn status(mut self, status: ActivityStatus) -> Self {
        self.status = status;
        self
    }

    /// The badge text, when it should say more than the status
    /// ("Exit 101", "2 passed").
    #[must_use]
    pub fn status_label(mut self, label: impl Into<String>) -> Self {
        self.status_label = Some(label.into());
        self
    }

    /// What expanding the row shows (usually a [`super::CodeBlock`]).
    #[must_use]
    pub fn body(mut self, body: impl Render) -> Self {
        self.body = Some(body.render());
        self
    }

    /// Starts expanded.
    #[must_use]
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// The element id.
    #[must_use]
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    fn summary(&self, expandable: bool) -> Markup {
        let label = self
            .status_label
            .clone()
            .unwrap_or_else(|| self.status.label().to_owned());
        html! {
            span.oa-tool-call__icon aria-hidden="true" { (self.icon.size(IconSize::Sm)) }
            span.oa-tool-call__title { (self.title) }
            @if let Some(detail) = &self.detail {
                code.oa-tool-call__detail { (detail) }
            }
            span.oa-tool-call__status { (self.status.badge(label)) }
            @if expandable {
                span.oa-tool-call__chevron aria-hidden="true" { (Icon::ChevronDown.size(IconSize::Sm)) }
            }
        }
    }
}

impl Render for ToolCall {
    fn render(&self) -> Markup {
        let status = self.status.as_str();
        match &self.body {
            Some(body) => html! {
                details.oa-tool-call data-status=(status) id=[self.id.as_deref()] open[self.open] {
                    summary.oa-tool-call__summary { (self.summary(true)) }
                    div.oa-tool-call__body { (body) }
                }
            },
            None => html! {
                div.oa-tool-call data-status=(status) id=[self.id.as_deref()] {
                    div.oa-tool-call__summary { (self.summary(false)) }
                }
            },
        }
    }
}

/// Several calls folded under one line: `details.oa-tool-group`.
#[derive(Clone, Debug)]
pub struct ToolGroup {
    title: String,
    meta: Option<String>,
    calls: Vec<ToolCall>,
    open: bool,
}

impl ToolGroup {
    /// A group summarized as `title` ("Explored the repository").
    #[must_use]
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            meta: None,
            calls: Vec::new(),
            open: false,
        }
    }

    /// Quiet text after the title; the call count by default.
    #[must_use]
    pub fn meta(mut self, meta: impl Into<String>) -> Self {
        self.meta = Some(meta.into());
        self
    }

    /// Adds a call.
    #[must_use]
    pub fn call(mut self, call: ToolCall) -> Self {
        self.calls.push(call);
        self
    }

    /// Adds calls.
    #[must_use]
    pub fn calls(mut self, calls: impl IntoIterator<Item = ToolCall>) -> Self {
        self.calls.extend(calls);
        self
    }

    /// Starts expanded.
    #[must_use]
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }
}

impl Render for ToolGroup {
    fn render(&self) -> Markup {
        let meta = self.meta.clone().unwrap_or_else(|| match self.calls.len() {
            1 => "1 call".to_owned(),
            n => format!("{n} calls"),
        });
        html! {
            details.oa-tool-group open[self.open] {
                summary.oa-tool-group__summary {
                    span.oa-tool-group__title { (self.title) }
                    span.oa-tool-group__meta { (meta) }
                    span.oa-tool-call__chevron aria-hidden="true" { (Icon::ChevronDown.size(IconSize::Sm)) }
                }
                div.oa-tool-group__calls {
                    @for call in &self.calls { (call) }
                }
            }
        }
    }
}

/// One entry in [`Steps`].
#[derive(Clone, Debug)]
pub struct Step {
    label: String,
    detail: Option<String>,
    status: ActivityStatus,
}

impl Step {
    /// A step reading `label` with `status`.
    #[must_use]
    pub fn new(label: impl Into<String>, status: ActivityStatus) -> Self {
        Self {
            label: label.into(),
            detail: None,
            status,
        }
    }

    /// A quiet line under the label.
    #[must_use]
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// An ordered progress list: `ol.oa-steps` in a labelled region.
#[derive(Clone, Debug)]
pub struct Steps {
    label: String,
    steps: Vec<Step>,
}

impl Steps {
    /// A list whose accessible name and visible heading is `label`.
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            steps: Vec::new(),
        }
    }

    /// Adds a step.
    #[must_use]
    pub fn step(mut self, step: Step) -> Self {
        self.steps.push(step);
        self
    }
}

impl Render for Steps {
    fn render(&self) -> Markup {
        let done = self
            .steps
            .iter()
            .filter(|s| s.status == ActivityStatus::Done)
            .count();
        html! {
            section.oa-steps aria-label=(self.label) {
                header.oa-steps__header {
                    span.oa-steps__title { (self.label) }
                    span.oa-steps__count { (done) " of " (self.steps.len()) }
                }
                ol.oa-steps__list {
                    @for step in &self.steps {
                        li.oa-step data-status=(step.status.as_str()) {
                            span.oa-step__marker { (step.status.marker()) }
                            span.oa-step__text {
                                span.oa-step__label { (step.label) }
                                span.oa-visually-hidden { ", " (step.status.label()) }
                                @if let Some(detail) = &step.detail {
                                    span.oa-step__detail { (detail) }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// A card for an outcome to review: `section.oa-result-card`.
#[derive(Clone, Debug)]
pub struct ResultCard {
    title: String,
    subtitle: Option<String>,
    badge: Option<Badge>,
    facts: Vec<(String, Markup)>,
    body: Option<Markup>,
    footer: Option<Markup>,
}

impl ResultCard {
    /// A card headed `title`.
    #[must_use]
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            subtitle: None,
            badge: None,
            facts: Vec::new(),
            body: None,
            footer: None,
        }
    }

    /// A quiet line under the title.
    #[must_use]
    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// The badge beside the title (for example [`ActivityStatus::badge`]).
    #[must_use]
    pub fn badge(mut self, badge: Badge) -> Self {
        self.badge = Some(badge);
        self
    }

    /// A labelled value in the card's fact list.
    #[must_use]
    pub fn fact(mut self, term: impl Into<String>, value: impl Render) -> Self {
        self.facts.push((term.into(), value.render()));
        self
    }

    /// Content under the facts (a code block, a step list).
    #[must_use]
    pub fn body(mut self, body: impl Render) -> Self {
        self.body = Some(body.render());
        self
    }

    /// A footer line or actions.
    #[must_use]
    pub fn footer(mut self, footer: impl Render) -> Self {
        self.footer = Some(footer.render());
        self
    }
}

impl Render for ResultCard {
    fn render(&self) -> Markup {
        html! {
            section.oa-result-card aria-label=(self.title) {
                header.oa-result-card__header {
                    div.oa-result-card__heading {
                        h3.oa-result-card__title { (self.title) }
                        @if let Some(subtitle) = &self.subtitle {
                            p.oa-result-card__subtitle { (subtitle) }
                        }
                    }
                    @if let Some(badge) = &self.badge { (badge) }
                }
                @if !self.facts.is_empty() {
                    dl.oa-result-card__facts {
                        @for (term, value) in &self.facts {
                            div { dt { (term) } dd { (value) } }
                        }
                    }
                }
                @if let Some(body) = &self.body {
                    div.oa-result-card__body { (body) }
                }
                @if let Some(footer) = &self.footer {
                    footer.oa-result-card__footer { (footer) }
                }
            }
        }
    }
}

/// Changed files with line counts: `ul.oa-file-changes`.
#[derive(Clone, Debug, Default)]
pub struct FileChanges {
    files: Vec<(String, u32, u32)>,
}

impl FileChanges {
    /// An empty list.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A changed `path` with `added` and `removed` lines.
    #[must_use]
    pub fn file(mut self, path: impl Into<String>, added: u32, removed: u32) -> Self {
        self.files.push((path.into(), added, removed));
        self
    }
}

impl Render for FileChanges {
    fn render(&self) -> Markup {
        let added: u32 = self.files.iter().map(|f| f.1).sum();
        let removed: u32 = self.files.iter().map(|f| f.2).sum();
        let count = self.files.len();
        html! {
            div.oa-file-changes {
                p.oa-file-changes__total {
                    (count) @if count == 1 { " file changed" } @else { " files changed" }
                    span.oa-file-changes__added { "+" (added) }
                    span.oa-file-changes__removed { "\u{2212}" (removed) }
                }
                ul.oa-file-changes__list role="list" {
                    @for (path, added, removed) in &self.files {
                        li {
                            code.oa-file-changes__path { (path) }
                            span.oa-file-changes__added aria-label=(format!("{added} added")) { "+" (added) }
                            span.oa-file-changes__removed aria-label=(format!("{removed} removed")) { "\u{2212}" (removed) }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::CodeBlock;

    #[test]
    fn a_call_with_output_is_a_details_row_and_without_is_static() {
        let open = ToolCall::new(Icon::Terminal, "Ran")
            .detail("cargo fetch --locked <x>")
            .status(ActivityStatus::Failed)
            .status_label("Exit 101")
            .body(CodeBlock::new("error"))
            .render()
            .into_string();
        assert!(open.starts_with("<details class=\"oa-tool-call\" data-status=\"failed\">"));
        assert!(open.contains("cargo fetch --locked &lt;x&gt;"));
        assert!(open.contains("Exit 101"));
        assert!(open.contains("data-color=\"danger\""));
        let row = ToolCall::new(Icon::Search, "Searched")
            .render()
            .into_string();
        assert!(row.starts_with("<div class=\"oa-tool-call\" data-status=\"done\">"));
        assert!(!row.contains("oa-tool-call__chevron"));
    }

    #[test]
    fn groups_count_calls_and_steps_count_done() {
        let group = ToolGroup::new("Explored")
            .call(ToolCall::new(Icon::Search, "Searched"))
            .call(ToolCall::new(Icon::FileDocument, "Read"))
            .render()
            .into_string();
        assert!(group.contains("2 calls"));
        let steps = Steps::new("Setup")
            .step(Step::new("Install", ActivityStatus::Done))
            .step(Step::new("Build", ActivityStatus::Running).detail("clean base"))
            .render()
            .into_string();
        assert!(steps.contains("1 of 2"));
        assert!(steps.contains("data-status=\"running\""));
        assert!(steps.contains("clean base"));
    }

    #[test]
    fn cards_and_file_changes_escape_and_total() {
        let card = ResultCard::new("Root <Rust>")
            .badge(ActivityStatus::Done.badge("Verified"))
            .fact("Image", "snap-02")
            .render()
            .into_string();
        assert!(card.contains("Root &lt;Rust&gt;"));
        assert!(card.contains("<dt>Image</dt><dd>snap-02</dd>"));
        let files = FileChanges::new()
            .file("src/a.rs", 3, 1)
            .file("src/b.rs", 2, 0)
            .render()
            .into_string();
        assert!(files.contains("2 files changed"));
        assert!(files.contains("+5"));
    }
}
