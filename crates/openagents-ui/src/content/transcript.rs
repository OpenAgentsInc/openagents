//! A conversation as a transcript (#11621): the scroller that holds the
//! turns, a turn by an agent, what the person sent, the notes and dividers
//! between turns, files, and clarifying questions.
//!
//! - [`MessageScroller`]: the turns in order, as a `role="log"` region.
//!   Each turn is an anchor (`#id`, `scroll-margin-top`). With
//!   [`MessageScroller::follow`] the page script
//!   (`static/components/message-scroller.js`) keeps the view at the
//!   newest words while they stream in, but only while the reader is at
//!   the bottom, and keeps the view still when earlier turns load above.
//! - [`Turn`]: one agent turn: a header (who, which model, when), the body
//!   (a [`super::Trace`] and the answer), and a footer for actions.
//! - [`Bubble`]: what the person sent, on the trailing side, with any
//!   [`Attachment`]s above it.
//! - [`Marker`]: a divider ("October 14") or a quiet note between turns.
//! - [`Attachment`] and [`Attachments`]: a file, image, folder or link.
//! - [`Questionnaire`] with its [`Question`]s: an agent's clarifying
//!   questions. With a form action it is a real form; without one it shows
//!   the questions and the answers given, with no controls, so a shared or
//!   finished conversation never shows a control that does nothing.
//!
//! Styles live in `static/components/transcript.css`. Every builder
//! escapes the text it takes.

use maud::{Markup, Render, html};

use crate::actions::{Button, ButtonType, ButtonVariant, Color, ControlSize};
use crate::content::safe_href;
use crate::icons::{Icon, IconSize};

/// The turns of a conversation: `div.oa-message-scroller`.
#[derive(Clone, Debug, Default)]
pub struct MessageScroller {
    turns: Vec<Markup>,
    label: Option<String>,
    earlier: Option<(String, String)>,
    follow: bool,
    announce: Option<bool>,
}

impl MessageScroller {
    /// An empty scroller.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a turn: a [`Turn`], a [`Bubble`], a [`Marker`], ...
    #[must_use]
    pub fn turn(mut self, turn: impl Render) -> Self {
        self.turns.push(turn.render());
        self
    }

    /// Appends several turns.
    #[must_use]
    pub fn turns<R: Render>(mut self, turns: impl IntoIterator<Item = R>) -> Self {
        self.turns.extend(turns.into_iter().map(|t| t.render()));
        self
    }

    /// The region's accessible name ("Conversation").
    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// A link above the first turn to earlier turns ("Earlier messages").
    /// Unsafe targets are dropped.
    #[must_use]
    pub fn earlier(mut self, label: impl Into<String>, href: &str) -> Self {
        if let Some(href) = safe_href(href) {
            self.earlier = Some((label.into(), href.to_owned()));
        }
        self
    }

    /// Follow new words as they stream in (a live conversation). Off for a
    /// finished one.
    #[must_use]
    pub fn follow(mut self, follow: bool) -> Self {
        self.follow = follow;
        self
    }

    /// Whether screen readers hear new turns as they arrive (`aria-live`).
    /// By default a following scroller announces and a still one doesn't.
    /// A page that redraws every turn on each update turns this off and
    /// announces through its own status line, so a redraw isn't read again.
    #[must_use]
    pub fn announce(mut self, announce: bool) -> Self {
        self.announce = Some(announce);
        self
    }
}

impl Render for MessageScroller {
    fn render(&self) -> Markup {
        html! {
            div.oa-message-scroller role="log" aria-label=[self.label.as_deref()]
                aria-live=(if self.announce.unwrap_or(self.follow) { "polite" } else { "off" })
                data-oa-message-scroller data-follow[self.follow] {
                @if let Some((label, href)) = &self.earlier {
                    a.oa-message-scroller__earlier href=(href) { (label) }
                }
                div.oa-message-scroller__turns {
                    @for turn in &self.turns { (turn) }
                }
                div.oa-message-scroller__anchor aria-hidden="true" {}
            }
        }
    }
}

/// When something happened: a machine-readable time and the words shown.
#[derive(Clone, Debug)]
struct When {
    datetime: String,
    label: String,
}

/// One agent turn: `article.oa-turn`.
#[derive(Clone, Debug)]
pub struct Turn {
    author: String,
    icon: Icon,
    model: Option<String>,
    when: Option<When>,
    body: Vec<Markup>,
    actions: Vec<Markup>,
    meta: Option<String>,
    id: Option<String>,
}

impl Turn {
    /// A turn by `author` ("Coder", "OpenAgents").
    #[must_use]
    pub fn new(author: impl Into<String>) -> Self {
        Self {
            author: author.into(),
            icon: Icon::Agent,
            model: None,
            when: None,
            body: Vec::new(),
            actions: Vec::new(),
            meta: None,
            id: None,
        }
    }

    /// The icon beside the author (default [`Icon::Agent`]).
    #[must_use]
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = icon;
        self
    }

    /// The model that answered, shown after the author.
    #[must_use]
    pub fn model(mut self, model: impl Into<String>) -> Self {
        let model = model.into();
        self.model = (!model.trim().is_empty()).then_some(model);
        self
    }

    /// When it answered: `datetime` for the `<time>` element, `label` as
    /// shown ("13:02").
    #[must_use]
    pub fn time(mut self, datetime: impl Into<String>, label: impl Into<String>) -> Self {
        self.when = Some(When {
            datetime: datetime.into(),
            label: label.into(),
        });
        self
    }

    /// Appends to the body: a trace, the answer as Markdown, a card.
    #[must_use]
    pub fn part(mut self, part: impl Render) -> Self {
        self.body.push(part.render());
        self
    }

    /// Appends a footer action, such as a copy button.
    #[must_use]
    pub fn action(mut self, action: impl Render) -> Self {
        self.actions.push(action.render());
        self
    }

    /// Quiet footer text: time taken, words used.
    #[must_use]
    pub fn meta(mut self, meta: impl Into<String>) -> Self {
        let meta = meta.into();
        self.meta = (!meta.trim().is_empty()).then_some(meta);
        self
    }

    /// The element id, so the turn can be linked to.
    #[must_use]
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }
}

impl Render for Turn {
    fn render(&self) -> Markup {
        let footer = !self.actions.is_empty() || self.meta.is_some();
        html! {
            article.oa-turn data-from="agent" id=[self.id.as_deref()] {
                header.oa-turn__header {
                    span.oa-turn__avatar aria-hidden="true" { (self.icon.size(IconSize::Sm)) }
                    h2.oa-turn__author { (self.author) }
                    @if let Some(model) = &self.model {
                        span.oa-turn__model { (model) }
                    }
                    @if let Some(when) = &self.when {
                        time.oa-turn__time datetime=(when.datetime) { (when.label) }
                    }
                }
                div.oa-turn__body {
                    @for part in &self.body { (part) }
                }
                @if footer {
                    footer.oa-turn__footer {
                        @if !self.actions.is_empty() {
                            div.oa-turn__actions role="toolbar" aria-label="Message actions" {
                                @for action in &self.actions { (action) }
                            }
                        }
                        @if let Some(meta) = &self.meta {
                            span.oa-turn__meta { (meta) }
                        }
                    }
                }
            }
        }
    }
}

/// What the person sent: `article.oa-turn[data-from=person]` with the
/// words in a bubble on the trailing side.
#[derive(Clone, Debug)]
pub struct Bubble {
    text: Option<String>,
    markup: Option<Markup>,
    author: String,
    attachments: Vec<Attachment>,
    when: Option<When>,
    id: Option<String>,
}

impl Bubble {
    /// A bubble with `text` as written (line breaks kept).
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: Some(text.into()),
            markup: None,
            author: "You".into(),
            attachments: Vec::new(),
            when: None,
            id: None,
        }
    }

    /// A bubble with trusted rendered markup in place of the text.
    #[must_use]
    pub fn markup(markup: impl Render) -> Self {
        Self {
            text: None,
            markup: Some(markup.render()),
            ..Self::new("")
        }
    }

    /// Who sent it, for screen readers (default "You").
    #[must_use]
    pub fn author(mut self, author: impl Into<String>) -> Self {
        self.author = author.into();
        self
    }

    /// A file sent with it, shown above the bubble.
    #[must_use]
    pub fn attachment(mut self, attachment: Attachment) -> Self {
        self.attachments.push(attachment);
        self
    }

    /// When it was sent.
    #[must_use]
    pub fn time(mut self, datetime: impl Into<String>, label: impl Into<String>) -> Self {
        self.when = Some(When {
            datetime: datetime.into(),
            label: label.into(),
        });
        self
    }

    /// The element id, so the turn can be linked to.
    #[must_use]
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }
}

impl Render for Bubble {
    fn render(&self) -> Markup {
        let empty = self.text.as_deref().is_some_and(|t| t.trim().is_empty());
        html! {
            article.oa-turn data-from="person" id=[self.id.as_deref()] {
                h2.oa-visually-hidden { (self.author) }
                @if !self.attachments.is_empty() {
                    (Attachments::new(self.attachments.clone()))
                }
                @if !empty {
                    div.oa-bubble {
                        @if let Some(markup) = &self.markup {
                            (markup)
                        } @else if let Some(text) = &self.text {
                            (text)
                        }
                    }
                }
                @if let Some(when) = &self.when {
                    time.oa-bubble__time datetime=(when.datetime) { (when.label) }
                }
            }
        }
    }
}

/// How a [`Marker`] reads (`data-tone`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MarkerTone {
    /// Quiet.
    #[default]
    Neutral,
    /// Something went wrong.
    Danger,
    /// Worth a look.
    Info,
}

impl MarkerTone {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Neutral => "neutral",
            Self::Danger => "danger",
            Self::Info => "info",
        }
    }
}

/// A divider or a note between turns: `div.oa-marker`.
#[derive(Clone, Debug)]
pub struct Marker {
    text: String,
    divider: bool,
    icon: Option<Icon>,
    tone: MarkerTone,
}

impl Marker {
    /// A rule across the column with `text` in the middle ("October 14").
    #[must_use]
    pub fn divider(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            divider: true,
            icon: None,
            tone: MarkerTone::Neutral,
        }
    }

    /// A quiet line of text ("The conversation was shortened here").
    #[must_use]
    pub fn note(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            divider: false,
            icon: None,
            tone: MarkerTone::Neutral,
        }
    }

    /// An icon before the text.
    #[must_use]
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// The tone (default [`MarkerTone::Neutral`]).
    #[must_use]
    pub fn tone(mut self, tone: MarkerTone) -> Self {
        self.tone = tone;
        self
    }
}

impl Render for Marker {
    fn render(&self) -> Markup {
        let kind = if self.divider { "divider" } else { "note" };
        html! {
            @if self.divider {
                div.oa-marker data-kind=(kind) data-tone=(self.tone.as_str()) role="separator" aria-label=(self.text) {
                    span.oa-marker__text aria-hidden="true" {
                        @if let Some(icon) = self.icon { (icon.size(IconSize::Xs)) }
                        (self.text)
                    }
                }
            } @else {
                p.oa-marker data-kind=(kind) data-tone=(self.tone.as_str()) {
                    @if let Some(icon) = self.icon {
                        span.oa-marker__icon aria-hidden="true" { (icon.size(IconSize::Xs)) }
                    }
                    span.oa-marker__text { (self.text) }
                }
            }
        }
    }
}

/// What an [`Attachment`] is: it picks the icon (`data-kind`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AttachmentKind {
    /// A file.
    #[default]
    File,
    /// Source code.
    Code,
    /// A picture.
    Image,
    /// A folder.
    Folder,
    /// A web page.
    Link,
}

impl AttachmentKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Code => "code",
            Self::Image => "image",
            Self::Folder => "folder",
            Self::Link => "link",
        }
    }

    const fn icon(self) -> Icon {
        match self {
            Self::File => Icon::FileDocument,
            Self::Code => Icon::FileCode,
            Self::Image => Icon::FileImage,
            Self::Folder => Icon::Folder,
            Self::Link => Icon::Link,
        }
    }

    /// A guess from a file name's extension.
    #[must_use]
    pub fn of_name(name: &str) -> Self {
        let lower = name.to_ascii_lowercase();
        let ext = lower.rsplit_once('.').map_or("", |(_, e)| e);
        match ext {
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "avif" => Self::Image,
            "rs" | "ts" | "js" | "py" | "go" | "c" | "h" | "cpp" | "swift" | "kt" | "java"
            | "rb" | "sh" | "toml" | "json" | "yaml" | "yml" | "css" | "html" | "sql" => Self::Code,
            _ if lower.ends_with('/') => Self::Folder,
            _ => Self::File,
        }
    }
}

/// A file, image, folder or link: `.oa-attachment`, a link when it has an
/// address.
#[derive(Clone, Debug)]
pub struct Attachment {
    name: String,
    kind: AttachmentKind,
    meta: Option<String>,
    href: Option<String>,
    preview: Option<String>,
}

impl Attachment {
    /// An attachment named `name`, its kind guessed from the name.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            kind: AttachmentKind::of_name(&name),
            name,
            meta: None,
            href: None,
            preview: None,
        }
    }

    /// The kind, in place of the guess.
    #[must_use]
    pub fn kind(mut self, kind: AttachmentKind) -> Self {
        self.kind = kind;
        self
    }

    /// A short second line: a size, "Changed", "+12 −3".
    #[must_use]
    pub fn meta(mut self, meta: impl Into<String>) -> Self {
        let meta = meta.into();
        self.meta = (!meta.trim().is_empty()).then_some(meta);
        self
    }

    /// Where it opens. Unsafe targets are dropped.
    #[must_use]
    pub fn href(mut self, href: &str) -> Self {
        self.href = safe_href(href).map(str::to_owned);
        self
    }

    /// A picture to show in place of the icon (same-site or `https`).
    #[must_use]
    pub fn preview(mut self, src: &str) -> Self {
        self.preview = safe_href(src)
            .filter(|s| s.starts_with('/') || s.to_ascii_lowercase().starts_with("https://"))
            .map(str::to_owned);
        self
    }

    fn inner(&self) -> Markup {
        html! {
            span.oa-attachment__tile aria-hidden="true" {
                @if let Some(src) = &self.preview {
                    img.oa-attachment__preview src=(src) alt="" loading="lazy";
                } @else {
                    (self.kind.icon().size(IconSize::Sm))
                }
            }
            span.oa-attachment__text {
                span.oa-attachment__name { (self.name) }
                @if let Some(meta) = &self.meta {
                    span.oa-attachment__meta { (meta) }
                }
            }
        }
    }
}

impl Render for Attachment {
    fn render(&self) -> Markup {
        let kind = self.kind.as_str();
        html! {
            @if let Some(href) = &self.href {
                @let external = super::is_external(href);
                a.oa-attachment data-kind=(kind) href=(href)
                    target=[external.then_some("_blank")]
                    rel=[external.then_some("noopener noreferrer")] { (self.inner()) }
            } @else {
                span.oa-attachment data-kind=(kind) { (self.inner()) }
            }
        }
    }
}

/// A row of [`Attachment`]s: `ul.oa-attachments`.
#[derive(Clone, Debug, Default)]
pub struct Attachments {
    items: Vec<Attachment>,
    label: Option<String>,
}

impl Attachments {
    /// The attachments, in order.
    #[must_use]
    pub fn new(items: impl IntoIterator<Item = Attachment>) -> Self {
        Self {
            items: items.into_iter().collect(),
            label: None,
        }
    }

    /// The list's accessible name ("Files changed").
    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

impl Render for Attachments {
    fn render(&self) -> Markup {
        html! {
            ul.oa-attachments role="list" aria-label=[self.label.as_deref()] {
                @for item in &self.items { li { (item) } }
            }
        }
    }
}

/// One clarifying question in a [`Questionnaire`].
#[derive(Clone, Debug)]
pub struct Question {
    name: String,
    prompt: String,
    options: Vec<(String, Option<String>)>,
    multiple: bool,
    other: bool,
    answer: Vec<String>,
}

impl Question {
    /// A question; `name` is its form field name.
    #[must_use]
    pub fn new(name: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            name: name
                .into()
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
                .collect(),
            prompt: prompt.into(),
            options: Vec::new(),
            multiple: false,
            other: false,
            answer: Vec::new(),
        }
    }

    /// A choice, with an optional line explaining it.
    #[must_use]
    pub fn option(mut self, label: impl Into<String>, description: Option<&str>) -> Self {
        self.options
            .push((label.into(), description.map(str::to_owned)));
        self
    }

    /// Allow more than one choice (default: one).
    #[must_use]
    pub fn multiple(mut self, multiple: bool) -> Self {
        self.multiple = multiple;
        self
    }

    /// Offer a box for another answer (default: no).
    #[must_use]
    pub fn other(mut self, other: bool) -> Self {
        self.other = other;
        self
    }

    /// The answer given: chosen labels, or free text.
    #[must_use]
    pub fn answer<S: Into<String>>(mut self, answer: impl IntoIterator<Item = S>) -> Self {
        self.answer = answer.into_iter().map(Into::into).collect();
        self
    }

    fn form(&self, index: usize, prefix: &str) -> Markup {
        let field = if self.name.is_empty() {
            format!("q{index}")
        } else {
            self.name.clone()
        };
        let input = if self.multiple { "checkbox" } else { "radio" };
        html! {
            fieldset.oa-questionnaire__question {
                legend.oa-questionnaire__prompt { (self.prompt) }
                @for (n, (label, description)) in self.options.iter().enumerate() {
                    @let id = format!("{prefix}-{field}-{n}");
                    label.oa-questionnaire__option for=(id) {
                        input id=(id) type=(input) name=(field) value=(label)
                            checked[self.answer.contains(label)];
                        span.oa-questionnaire__option-text {
                            span.oa-questionnaire__option-label { (label) }
                            @if let Some(description) = description {
                                span.oa-questionnaire__option-description { (description) }
                            }
                        }
                    }
                }
                @if self.other || self.options.is_empty() {
                    @let id = format!("{prefix}-{field}-other");
                    label.oa-questionnaire__other for=(id) {
                        span.oa-visually-hidden[!self.options.is_empty()] {
                            @if self.options.is_empty() { "Your answer" } @else { "Something else" }
                        }
                        input.oa-questionnaire__other-input id=(id) type="text"
                            name=(format!("{field}-other"))
                            placeholder=(if self.options.is_empty() { "Your answer" } else { "Something else" });
                    }
                }
            }
        }
    }

    fn answered(&self) -> Markup {
        let others: Vec<&String> = self
            .answer
            .iter()
            .filter(|a| !self.options.iter().any(|(label, _)| label == *a))
            .collect();
        html! {
            div.oa-questionnaire__question {
                p.oa-questionnaire__prompt { (self.prompt) }
                @if self.answer.is_empty() {
                    p.oa-questionnaire__unanswered { "Not answered" }
                } @else {
                    ul.oa-questionnaire__answers role="list" {
                        @for (label, _) in &self.options {
                            @let chosen = self.answer.contains(label);
                            li.oa-questionnaire__answer data-chosen[chosen] {
                                span.oa-questionnaire__mark aria-hidden="true" {
                                    @if chosen { (Icon::Check.size(IconSize::Xs)) }
                                }
                                (label)
                                @if chosen { span.oa-visually-hidden { ", chosen" } }
                            }
                        }
                        @for other in others {
                            li.oa-questionnaire__answer data-chosen {
                                span.oa-questionnaire__mark aria-hidden="true" { (Icon::Check.size(IconSize::Xs)) }
                                (other)
                            }
                        }
                    }
                }
            }
        }
    }
}

/// An agent's clarifying questions: `.oa-questionnaire`.
#[derive(Clone, Debug)]
pub struct Questionnaire {
    title: String,
    questions: Vec<Question>,
    action: Option<String>,
    fields: Vec<(String, String)>,
    submit: String,
    id: String,
}

impl Questionnaire {
    /// Questions under `title` ("A few questions first"); `id` keeps its
    /// field ids unique on the page.
    #[must_use]
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            questions: Vec::new(),
            action: None,
            fields: Vec::new(),
            submit: "Send answers".into(),
            id: id
                .into()
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
                .collect(),
        }
    }

    /// Appends a question.
    #[must_use]
    pub fn question(mut self, question: Question) -> Self {
        self.questions.push(question);
        self
    }

    /// Post the answers to `action`: it becomes a form. Without one it
    /// shows the answers given and no controls. Unsafe targets are dropped.
    #[must_use]
    pub fn action(mut self, action: &str) -> Self {
        self.action = safe_href(action).map(str::to_owned);
        self
    }

    /// A hidden field sent with the answers, such as a CSRF token.
    #[must_use]
    pub fn field(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.fields.push((name.into(), value.into()));
        self
    }

    /// The send button's label (default "Send answers").
    #[must_use]
    pub fn submit_label(mut self, label: impl Into<String>) -> Self {
        self.submit = label.into();
        self
    }
}

impl Render for Questionnaire {
    fn render(&self) -> Markup {
        let heading = format!("{}-title", self.id);
        html! {
            @if let Some(action) = &self.action {
                form.oa-questionnaire method="post" action=(action) aria-labelledby=(heading) {
                    p.oa-questionnaire__title id=(heading) {
                        span aria-hidden="true" { (Icon::QuestionMarkCircle.size(IconSize::Sm)) }
                        (self.title)
                    }
                    @for (name, value) in &self.fields {
                        input type="hidden" name=(name) value=(value);
                    }
                    @for (index, question) in self.questions.iter().enumerate() {
                        (question.form(index, &self.id))
                    }
                    div.oa-questionnaire__actions {
                        (Button::new(self.submit.clone())
                            .kind(ButtonType::Submit)
                            .size(ControlSize::Sm)
                            .variant(ButtonVariant::Solid)
                            .color(Color::Primary))
                    }
                }
            } @else {
                section.oa-questionnaire data-answered aria-labelledby=(heading) {
                    p.oa-questionnaire__title id=(heading) {
                        span aria-hidden="true" { (Icon::QuestionMarkCircle.size(IconSize::Sm)) }
                        (self.title)
                    }
                    @for question in &self.questions { (question.answered()) }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scroller_turns_and_bubbles() {
        let html = MessageScroller::new()
            .label("Conversation")
            .earlier("Earlier messages", "/trace/x?before=4")
            .follow(true)
            .turn(Marker::divider("October 14"))
            .turn(
                Bubble::new("hi <b>")
                    .id("turn-1")
                    .attachment(Attachment::new("shot.png").meta("12 KB")),
            )
            .turn(
                Turn::new("Coder")
                    .model("gemini")
                    .time("2026-10-14T13:02:00Z", "13:02")
                    .part(html! { p { "Hello" } })
                    .meta("3s"),
            )
            .render()
            .into_string();
        assert!(html.contains("role=\"log\" aria-label=\"Conversation\" aria-live=\"polite\" data-oa-message-scroller data-follow"), "{html}");
        assert!(html.contains("href=\"/trace/x?before=4\""), "{html}");
        assert!(
            html.contains("role=\"separator\" aria-label=\"October 14\""),
            "{html}"
        );
        assert!(
            html.contains("<div class=\"oa-bubble\">hi &lt;b&gt;</div>"),
            "{html}"
        );
        assert!(html.contains("data-kind=\"image\""), "{html}");
        assert!(html.contains("<h2 class=\"oa-turn__author\">Coder</h2><span class=\"oa-turn__model\">gemini</span>"), "{html}");
        assert!(
            html.contains(
                "<time class=\"oa-turn__time\" datetime=\"2026-10-14T13:02:00Z\">13:02</time>"
            ),
            "{html}"
        );
        assert!(
            html.contains("<span class=\"oa-turn__meta\">3s</span>"),
            "{html}"
        );
        // A page that redraws every turn announces elsewhere (#11639).
        let quiet = MessageScroller::new()
            .follow(true)
            .announce(false)
            .render()
            .into_string();
        assert!(
            quiet.contains("aria-live=\"off\" data-oa-message-scroller data-follow"),
            "{quiet}"
        );
        let still = MessageScroller::new().render().into_string();
        assert!(still.contains("aria-live=\"off\""), "{still}");
        assert!(!still.contains("data-follow"), "{still}");
    }

    #[test]
    fn a_questionnaire_is_a_form_only_with_an_action() {
        let question = Question::new("scope", "Which crates?")
            .option("Web", Some("The website"))
            .option("UI", None)
            .multiple(true)
            .other(true)
            .answer(["UI", "Both, really"]);
        let shown = Questionnaire::new("q1", "A few questions")
            .question(question.clone())
            .render()
            .into_string();
        assert!(!shown.contains("<input"), "{shown}");
        assert!(!shown.contains("<button"), "{shown}");
        assert!(shown.contains("data-chosen"), "{shown}");
        assert!(shown.contains("Both, really"), "{shown}");
        let form = Questionnaire::new("q1", "A few questions")
            .question(question)
            .action("/chat/x/answers")
            .field("csrf", "t")
            .render()
            .into_string();
        assert!(
            form.contains(
                "<form class=\"oa-questionnaire\" method=\"post\" action=\"/chat/x/answers\""
            ),
            "{form}"
        );
        assert!(
            form.contains("type=\"checkbox\" name=\"scope\" value=\"UI\" checked"),
            "{form}"
        );
        assert!(form.contains("name=\"scope-other\""), "{form}");
        assert!(form.contains("type=\"submit\""), "{form}");
        let unsafe_form = Questionnaire::new("q", "x")
            .action("javascript:alert(1)")
            .render()
            .into_string();
        assert!(!unsafe_form.contains("<form"), "{unsafe_form}");
    }

    #[test]
    fn attachments_link_only_to_safe_places() {
        let html = Attachments::new([
            Attachment::new("src/main.rs")
                .href("/files/main.rs")
                .meta("Changed"),
            Attachment::new("evil").href("javascript:x"),
            Attachment::new("pic.png").preview("http://insecure.example/a.png"),
        ])
        .label("Files")
        .render()
        .into_string();
        assert!(
            html.contains("<a class=\"oa-attachment\" data-kind=\"code\" href=\"/files/main.rs\">"),
            "{html}"
        );
        assert!(
            html.contains("<span class=\"oa-attachment\" data-kind=\"file\">"),
            "{html}"
        );
        assert!(!html.contains("insecure"), "{html}");
        assert_eq!(
            Marker::note("x")
                .tone(MarkerTone::Danger)
                .render()
                .into_string()
                .contains("data-tone=\"danger\""),
            true
        );
    }
}
