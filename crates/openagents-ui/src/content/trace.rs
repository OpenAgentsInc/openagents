//! Trace: what an agent did, one line per step (#11621).
//!
//! A [`Trace`] is a list of [`TraceStep`]s: reasoning, a tool call, a
//! source it read, a plan, or another agent it asked. Each line has an
//! icon, a title, an optional monospace detail and description, a time or
//! count on the trailing side, and a status: done, running (the title
//! shimmers), failed, or waiting.
//!
//! A line does one of four things:
//!
//! - opens in place: with a body or child steps it is a native
//!   `<details>`, so a summary opens to its steps and a step to its output
//!   with no script, and the keyboard and screen readers get a real
//!   disclosure from the browser;
//! - links out ([`TraceStep::href`]);
//! - opens a dialog or sheet ([`TraceStep::dialog`], an invoker button for
//!   a [`crate::overlays::Dialog`] the page renders);
//! - nothing: a plain line.
//!
//! Every line's status is spoken: the icon is decorative and the status
//! word follows the title in a visually hidden span. The rail between the
//! icons is drawn by CSS (`static/components/trace.css`).

use maud::{Markup, Render, html};

use crate::actions::LoadingIndicator;
use crate::content::safe_href;
use crate::icons::{Icon, IconSize};

/// Where a step stands (`data-status`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TraceStatus {
    /// Finished.
    #[default]
    Done,
    /// Still going: the title shimmers and the icon turns.
    Running,
    /// Finished with a failure.
    Error,
    /// Not started yet.
    Pending,
}

impl TraceStatus {
    /// The `data-status` value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Done => "done",
            Self::Running => "running",
            Self::Error => "error",
            Self::Pending => "pending",
        }
    }

    /// The word a screen reader hears after the title.
    #[must_use]
    pub const fn spoken(self) -> &'static str {
        match self {
            Self::Done => "done",
            Self::Running => "in progress",
            Self::Error => "failed",
            Self::Pending => "not started",
        }
    }
}

/// What kind of step a line is: it picks the default icon (`data-kind`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TraceKind {
    /// The model thinking.
    Reasoning,
    /// A tool call.
    #[default]
    Tool,
    /// A command in a terminal.
    Command,
    /// Reading a file.
    Read,
    /// Changing a file.
    Edit,
    /// A search.
    Search,
    /// A page or document it used.
    Source,
    /// A plan or a list of steps.
    Plan,
    /// Another agent it asked, with that agent's own steps.
    Agent,
    /// A summary of several steps.
    Summary,
}

impl TraceKind {
    /// The `data-kind` value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Reasoning => "reasoning",
            Self::Tool => "tool",
            Self::Command => "command",
            Self::Read => "read",
            Self::Edit => "edit",
            Self::Search => "search",
            Self::Source => "source",
            Self::Plan => "plan",
            Self::Agent => "agent",
            Self::Summary => "summary",
        }
    }

    /// The icon a line of this kind shows unless it names its own.
    #[must_use]
    pub const fn icon(self) -> Icon {
        match self {
            Self::Reasoning => Icon::Lightbulb,
            Self::Tool => Icon::Tools,
            Self::Command => Icon::Terminal,
            Self::Read => Icon::FileDocument,
            Self::Edit => Icon::EditPencil,
            Self::Search => Icon::Search,
            Self::Source => Icon::Globe,
            Self::Plan => Icon::Tasks,
            Self::Agent => Icon::Agent,
            Self::Summary => Icon::Sparkles,
        }
    }
}

/// What a line does when chosen.
#[derive(Clone, Debug)]
enum Action {
    None,
    Link { href: String, external: bool },
    Dialog(String),
}

/// One line of a [`Trace`]: `li.oa-trace-step`.
#[derive(Clone, Debug)]
pub struct TraceStep {
    kind: TraceKind,
    icon: Option<Icon>,
    title: String,
    detail: Option<String>,
    description: Option<String>,
    meta: Option<String>,
    status: TraceStatus,
    body: Option<Markup>,
    children: Vec<TraceStep>,
    action: Action,
    open: bool,
    id: Option<String>,
}

impl TraceStep {
    /// A line of `kind` reading `title` ("Ran", "Read Cargo.toml",
    /// "Thought for 3s").
    #[must_use]
    pub fn new(kind: TraceKind, title: impl Into<String>) -> Self {
        Self {
            kind,
            icon: None,
            title: title.into(),
            detail: None,
            description: None,
            meta: None,
            status: TraceStatus::Done,
            body: None,
            children: Vec::new(),
            action: Action::None,
            open: false,
            id: None,
        }
    }

    /// An icon in place of the kind's.
    #[must_use]
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Monospace text after the title: a path, a command, a query.
    #[must_use]
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        let detail = detail.into();
        self.detail = (!detail.trim().is_empty()).then_some(detail);
        self
    }

    /// A quieter second line.
    #[must_use]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        let description = description.into();
        self.description = (!description.trim().is_empty()).then_some(description);
        self
    }

    /// Short trailing text: a time ("4s"), a count ("3 steps").
    #[must_use]
    pub fn meta(mut self, meta: impl Into<String>) -> Self {
        let meta = meta.into();
        self.meta = (!meta.trim().is_empty()).then_some(meta);
        self
    }

    /// The status (default [`TraceStatus::Done`]).
    #[must_use]
    pub fn status(mut self, status: TraceStatus) -> Self {
        self.status = status;
        self
    }

    /// What the line opens to: output, a diff, the model's words.
    #[must_use]
    pub fn body(mut self, body: impl Render) -> Self {
        self.body = Some(body.render());
        self
    }

    /// A step nested under this one (drawn after the body).
    #[must_use]
    pub fn child(mut self, step: TraceStep) -> Self {
        self.children.push(step);
        self
    }

    /// Steps nested under this one.
    #[must_use]
    pub fn children(mut self, steps: impl IntoIterator<Item = TraceStep>) -> Self {
        self.children.extend(steps);
        self
    }

    /// Start open (only for a line that opens in place).
    #[must_use]
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// The line links to `href` instead of opening in place. Unsafe
    /// targets ([`safe_href`]) leave it a plain line.
    #[must_use]
    pub fn href(mut self, href: &str) -> Self {
        if let Some(href) = safe_href(href) {
            self.action = Action::Link {
                external: super::is_external(href),
                href: href.to_owned(),
            };
        }
        self
    }

    /// The line opens the dialog (or sheet) with this id, which the page
    /// renders with [`crate::overlays::Dialog`].
    #[must_use]
    pub fn dialog(mut self, dialog_id: impl Into<String>) -> Self {
        self.action = Action::Dialog(dialog_id.into());
        self
    }

    /// An element id for the line, for links to it.
    #[must_use]
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// The step's status.
    #[must_use]
    pub fn current_status(&self) -> TraceStatus {
        self.status
    }

    fn opens(&self) -> bool {
        matches!(self.action, Action::None) && (self.body.is_some() || !self.children.is_empty())
    }

    fn row(&self) -> Markup {
        let icon = self.icon.unwrap_or_else(|| self.kind.icon());
        let running = self.status == TraceStatus::Running;
        html! {
            span.oa-trace-step__icon aria-hidden="true" {
                @if running {
                    (LoadingIndicator::new().decorative())
                } @else {
                    (icon.size(IconSize::Sm))
                }
            }
            span.oa-trace-step__text {
                span.oa-trace-step__heading {
                    span.oa-trace-step__title.oa-shimmer-text[running] { (self.title) }
                    @if let Some(detail) = &self.detail {
                        " "
                        code.oa-trace-step__detail { (detail) }
                    }
                    span.oa-visually-hidden { ", " (self.status.spoken()) }
                }
                @if let Some(description) = &self.description {
                    span.oa-trace-step__description { (description) }
                }
            }
            @if let Some(meta) = &self.meta {
                span.oa-trace-step__meta { (meta) }
            }
            @match &self.action {
                Action::Link { external: true, .. } => {
                    span.oa-trace-step__affordance aria-hidden="true" { (Icon::ArrowUpRight.size(IconSize::Xs)) }
                }
                Action::Link { .. } | Action::Dialog(_) => {
                    span.oa-trace-step__affordance aria-hidden="true" { (Icon::ChevronRight.size(IconSize::Xs)) }
                }
                Action::None => {
                    @if self.opens() {
                        span.oa-trace-step__affordance.oa-trace-step__chevron aria-hidden="true" { (Icon::ChevronDown.size(IconSize::Xs)) }
                    }
                }
            }
        }
    }
}

impl Render for TraceStep {
    fn render(&self) -> Markup {
        let content = html! {
            @if let Some(body) = &self.body {
                div.oa-trace-step__body { (body) }
            }
            @if !self.children.is_empty() {
                (Trace::new(self.children.clone()).nested())
            }
        };
        html! {
            li.oa-trace-step id=[self.id.as_deref()] data-kind=(self.kind.as_str()) data-status=(self.status.as_str()) {
                @match &self.action {
                    Action::Link { href, external } => {
                        a.oa-trace-step__row href=(href)
                            target=[external.then_some("_blank")]
                            rel=[external.then_some("noopener noreferrer")] { (self.row()) }
                    }
                    Action::Dialog(id) => {
                        button.oa-trace-step__row type="button" command="show-modal"
                            commandfor=(id) aria-haspopup="dialog" { (self.row()) }
                    }
                    Action::None => {
                        @if self.opens() {
                            details.oa-trace-step__disclosure open[self.open] {
                                summary.oa-trace-step__row { (self.row()) }
                                div.oa-trace-step__content { (content) }
                            }
                        } @else {
                            div.oa-trace-step__row { (self.row()) }
                        }
                    }
                }
            }
        }
    }
}

/// A list of [`TraceStep`]s: `ol.oa-trace`, with a rail joining the icons.
#[derive(Clone, Debug, Default)]
pub struct Trace {
    steps: Vec<TraceStep>,
    label: Option<String>,
    nested: bool,
}

impl Trace {
    /// A trace of `steps`, in order.
    #[must_use]
    pub fn new(steps: impl IntoIterator<Item = TraceStep>) -> Self {
        Self {
            steps: steps.into_iter().collect(),
            label: None,
            nested: false,
        }
    }

    /// The list's accessible name ("What the agent did").
    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    fn nested(mut self) -> Self {
        self.nested = true;
        self
    }

    /// Whether it has no steps.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
}

impl Render for Trace {
    fn render(&self) -> Markup {
        html! {
            ol.oa-trace data-nested[self.nested] aria-label=[self.label.as_deref()] {
                @for step in &self.steps { (step) }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::CodeBlock;

    #[test]
    fn a_line_opens_to_its_output_and_nests() {
        let html = Trace::new([TraceStep::new(TraceKind::Summary, "Worked for 2m")
            .meta("3 steps")
            .open(true)
            .child(
                TraceStep::new(TraceKind::Command, "Ran")
                    .detail("cargo test <x>")
                    .body(CodeBlock::new("ok")),
            )
            .child(TraceStep::new(TraceKind::Reasoning, "Thinking").status(TraceStatus::Running))])
        .label("What the agent did")
        .render()
        .into_string();
        assert!(
            html.starts_with("<ol class=\"oa-trace\" aria-label=\"What the agent did\">"),
            "{html}"
        );
        assert!(html.contains("<details class=\"oa-trace-step__disclosure\" open><summary class=\"oa-trace-step__row\">"), "{html}");
        assert!(
            html.contains("<ol class=\"oa-trace\" data-nested>"),
            "{html}"
        );
        assert!(
            html.contains("<code class=\"oa-trace-step__detail\">cargo test &lt;x&gt;</code>"),
            "{html}"
        );
        assert!(
            html.contains("data-kind=\"command\" data-status=\"done\""),
            "{html}"
        );
        // Running: the title shimmers, the status is spoken.
        assert!(
            html.contains("class=\"oa-trace-step__title oa-shimmer-text\">Thinking"),
            "{html}"
        );
        assert!(html.contains(", in progress</span>"), "{html}");
        assert!(
            html.contains("class=\"oa-trace-step__title\">Ran"),
            "{html}"
        );
    }

    #[test]
    fn links_dialogs_and_plain_lines() {
        let link = TraceStep::new(TraceKind::Source, "Read the docs")
            .href("https://example.com/a")
            .render()
            .into_string();
        assert!(link.contains("<a class=\"oa-trace-step__row\" href=\"https://example.com/a\" target=\"_blank\" rel=\"noopener noreferrer\">"), "{link}");
        let unsafe_link = TraceStep::new(TraceKind::Source, "x")
            .href("javascript:alert(1)")
            .render()
            .into_string();
        assert!(
            unsafe_link.contains("<div class=\"oa-trace-step__row\">"),
            "{unsafe_link}"
        );
        let dialog = TraceStep::new(TraceKind::Plan, "Plan")
            .dialog("plan-1")
            .status(TraceStatus::Error)
            .render()
            .into_string();
        assert!(
            dialog
                .contains("command=\"show-modal\" commandfor=\"plan-1\" aria-haspopup=\"dialog\""),
            "{dialog}"
        );
        assert!(dialog.contains(", failed</span>"), "{dialog}");
        let plain = TraceStep::new(TraceKind::Tool, "Loaded tools")
            .render()
            .into_string();
        assert!(!plain.contains("details"), "{plain}");
        assert!(!plain.contains("oa-trace-step__chevron"), "{plain}");
    }
}
