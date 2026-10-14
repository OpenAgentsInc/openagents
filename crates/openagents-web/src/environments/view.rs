//! The Environments markup, built from `openagents-ui` only: the list, the
//! repository picker, the setup conversation (the same activity components
//! as the demo), the Save card, and Claude Code runs.

use coder_cloud::claude_task::PauseChoice;
use coder_environment_operator::activity::{Entry, Record, Stage};
use coder_environment_operator::agent::Phase;
use coder_environment_operator::studio::claude::{self, Run, RunState};
use coder_environment_operator::studio::sign_in;
use coder_environment_operator::studio::{Status, Summary, View};
use maud::{Markup, PreEscaped, Render, html};
use openagents_ui::actions::{
    Badge, Busy, Button, ButtonLink, ButtonType, ButtonVariant, Color, ControlSize, EmptyMessage,
};
use openagents_ui::content::{
    ActivityStatus, CodeBlock, CodeSize, MarkdownRoot, MarkdownSize, PageColumn, ResultCard, Step,
    Steps, Table, Trace, TraceKind, TraceStatus, TraceStep,
};
use openagents_ui::forms::{Field, FieldAria, Input, Select, Textarea};
use openagents_ui::icons::Icon;
use openagents_ui::shell::{
    Breadcrumb, Composer, Message, NavItem, ScrollToBottom, SidebarSection,
};

/// The longest message the composer takes.
pub(crate) const MAX_MESSAGE: usize = 4_000;

const AUTHOR: &str = "OpenAgents";

pub(crate) fn status_word(status: Status) -> &'static str {
    match status {
        Status::Starting => "Starting",
        Status::Working => "Setting up",
        Status::NeedsInput => "Needs your answer",
        Status::Building => "Building",
        Status::Checking => "Checking",
        Status::ReadyToSave => "Ready to save",
        Status::Saved => "Saved",
        Status::Failed => "Stopped",
    }
}

fn short(commit: &str) -> &str {
    commit.get(..7).unwrap_or(commit)
}

fn submit(label: &str, primary: bool) -> Button {
    let b = Button::new(label)
        .kind(ButtonType::Submit)
        .size(ControlSize::Sm);
    if primary {
        b
    } else {
        b.variant(ButtonVariant::Soft).color(Color::Secondary)
    }
}

fn field<C: Render>(
    id: &str,
    label: &str,
    error: Option<&str>,
    control: impl FnOnce(FieldAria) -> C,
) -> Markup {
    let field = Field::new(id, label).error_opt(error);
    let aria = field.aria();
    field.control(control(aria)).render()
}

/// The left panel's list of environments, the current one marked.
pub(crate) fn sidebar(rows: &[Summary], current: Option<&str>) -> SidebarSection {
    SidebarSection::new("Environments")
        .id("environment-list")
        .items(rows.iter().map(|s| {
            NavItem::new(s.repository.clone(), format!("/environments/{}", s.id))
                .current(current == Some(s.id.as_str()))
        }))
        .empty("No environments yet")
}

pub(crate) fn breadcrumb(title: &str) -> Breadcrumb {
    Breadcrumb::new(title).crumb("Environments", "/environments")
}

/// `/environments`: every environment, or the empty state.
pub(crate) fn index(rows: &[Summary]) -> Markup {
    let new = ButtonLink::new("New environment", "/environments/new");
    if rows.is_empty() {
        return PageColumn::new(
            EmptyMessage::new()
                .icon(Icon::Cube)
                .title("No environments yet")
                .description(
                    "Pick a repository and an agent sets it up on a fresh computer, checks it, and saves it so Claude Code can work in it.",
                )
                .actions(new),
        )
        .centered()
        .render();
    }
    let mut table =
        Table::new()
            .label("Your environments")
            .header(["Repository", "Branch", "Status", "Saved"]);
    for s in rows {
        table = table.row([
            html! { a href=(format!("/environments/{}", s.id)) { (s.repository) } },
            html! { (s.branch) " " code { (short(&s.commit)) } },
            html! { (status_word(s.status)) },
            html! {
                @match s.saved {
                    Some(n) => { "Version " (n) }
                    None => { "Not yet" }
                }
            },
        ]);
    }
    PageColumn::new(html! {
        div.oa-page-actions { (new) }
        (table)
    })
    .wide()
    .render()
}

/// What the repository step shows.
pub(crate) struct Pick<'a> {
    /// Load the person's repositories after the page shows
    /// ([`super::REPOS`]).
    pub list: bool,
    pub repo: &'a str,
    pub error: Option<&'a str>,
}

/// HTMX, for the repository list that loads after the page shows.
pub(crate) fn htmx_head() -> Markup {
    html! {
        meta name="htmx-config" content=r#"{"allowEval":false,"allowScriptTags":false,"historyCacheSize":0,"selfRequestsOnly":true,"includeIndicatorStyles":false,"timeout":20000}"#;
        script src="/static/htmx.min.js" defer {}
    }
}

fn repos_href(page: u32) -> String {
    format!("{}?page={page}", super::REPOS)
}

/// One repository to choose.
pub(crate) struct RepoRow<'a> {
    pub full_name: &'a str,
    pub private: bool,
}

/// One page of the person's repositories, each a link to its branch step,
/// and a Show more button that loads the next page in its place.
pub(crate) fn repos(rows: &[RepoRow<'_>], page: u32, more: bool) -> Markup {
    let next = format!("env-repos-{}", page + 1);
    html! {
        @if !rows.is_empty() {
            ul.oa-chat-archive-list role="list" {
                @for row in rows {
                    li.oa-chat-archive-row {
                        span {
                            (row.full_name)
                            @if row.private { " " (Badge::new("Private")) }
                        }
                        (ButtonLink::new("Choose", format!("/environments/new?repo={}", encode(row.full_name)))
                            .size(ControlSize::Sm)
                            .variant(ButtonVariant::Soft)
                            .color(Color::Secondary)
                            .attr("aria-label", format!("Choose {}", row.full_name)))
                    }
                }
            }
        } @else if page == 1 && !more {
            p { "No repositories found." }
        }
        @if more {
            div id=(next) {
                (ButtonLink::new("Show more", repos_href(page + 1))
                    .size(ControlSize::Sm)
                    .variant(ButtonVariant::Soft)
                    .color(Color::Secondary)
                    .attr("hx-get", repos_href(page + 1))
                    .attr("hx-target", format!("#{next}"))
                    .attr("hx-swap", "outerHTML"))
            }
        }
    }
}

/// The list's place when GitHub isn't connected (or stopped accepting the
/// access): a way to connect it on Projects.
pub(crate) fn repos_unconnected(reconnect: bool) -> Markup {
    html! {
        p {
            @if reconnect {
                "GitHub stopped accepting your access. "
                a href=(crate::projects::RECONNECT) { "Reconnect GitHub" }
                " to list your repositories."
            } @else {
                a href=(crate::projects::PAGE) { "Connect GitHub on Projects" }
                " to list your repositories, or paste a public repository's address below."
            }
        }
    }
}

/// The list's place when it couldn't load, with a way to try again.
pub(crate) fn repos_failed(error: &str, page: u32) -> Markup {
    let id = format!("env-repos-{page}");
    html! {
        div id=(id) {
            p role="alert" { (error) }
            (ButtonLink::new("Try again", repos_href(page))
                .size(ControlSize::Sm)
                .variant(ButtonVariant::Soft)
                .color(Color::Secondary)
                .attr("hx-get", repos_href(page))
                .attr("hx-target", format!("#{id}"))
                .attr("hx-swap", "outerHTML"))
        }
    }
}

fn encode(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

/// `/environments/new`, step one: choose a repository.
pub(crate) fn pick(p: &Pick<'_>) -> Markup {
    PageColumn::new(html! {
        (MarkdownRoot::new(html! {
            h1 { "New environment" }
            p { "Choose a GitHub repository. An agent will work out how to install it, check the result on a fresh computer, and ask you to save it." }
        }))
        @if p.list {
            section.oa-page-form aria-labelledby="env-yours" {
                (MarkdownRoot::new(html! { h2 #env-yours { "Your repositories" } }))
                // The list loads after the page shows, newest first.
                div #env-repos-1 hx-get=(repos_href(1)) hx-trigger="load" hx-swap="outerHTML" {
                    p { (Busy::new("Loading your repositories")) }
                }
            }
        }
        form.oa-page-form action="/environments/new" method="get" {
            (field("env-repo", if p.list { "Or paste a GitHub address" } else { "GitHub repository" }, p.error, |aria| {
                Input::new("repo")
                    .id("env-repo")
                    .value(p.repo)
                    .placeholder("owner/name or https://github.com/owner/name")
                    .autocomplete("off")
                    .spellcheck(false)
                    .aria(aria)
            }))
            div.oa-page-actions { (submit("Continue", true)) }
        }
    })
    .render()
}

/// `/environments/new`, step two: choose a branch and start.
pub(crate) fn branch(
    repo: &str,
    branches: &[String],
    default: &str,
    error: Option<&str>,
) -> Markup {
    PageColumn::new(html! {
        (MarkdownRoot::new(html! {
            h1 { "Set up " (repo) }
            p { "Pick the branch to set up. The agent works on its latest commit." }
        }))
        form.oa-page-form action="/environments" method="post" {
            input type="hidden" name="repo" value=(repo);
            (field("env-branch", "Branch", error, |aria| {
                let mut select = Select::new("branch").id("env-branch").aria(aria).selected(default);
                let mut seen = false;
                for b in branches {
                    seen |= b == default;
                    select = select.option(b.clone(), b.clone());
                }
                if !seen {
                    select = select.option(default, default);
                }
                select
            }))
            div.oa-page-actions {
                (submit("Set up environment", true))
                (ButtonLink::new("Choose another repository", "/environments/new").color(Color::Secondary).variant(ButtonVariant::Outline))
            }
        }
    })
    .render()
}

fn says(markdown: &str) -> Message {
    Message::assistant(MarkdownRoot::new(PreEscaped(
        crate::markdown::render_reply(markdown),
    )))
    .author(AUTHOR)
}

/// A progress line ("Working…"): plain text, its "Status" heading kept
/// for screen readers only.
fn note(text: impl AsRef<str>) -> Message {
    Message::status(text).quiet()
}

fn acts(content: impl Render) -> Message {
    Message::assistant(content).author(AUTHOR)
}

/// What a step printed, in a small code block.
fn output(text: &str) -> CodeBlock {
    CodeBlock::new(text)
        .title("Output")
        .size(CodeSize::Sm)
        .wrap_toggle(true)
}

/// An activity row in an answer: one trace line (#11639), as a live
/// chat's steps and the trace pages draw them.
fn line(step: TraceStep) -> Trace {
    Trace::new([step]).label("What the agent did")
}

fn exit_label(exit: Option<i64>) -> String {
    match exit {
        Some(0) => "Exit 0".into(),
        Some(n) => format!("Exit {n}"),
        None => "Didn't finish".into(),
    }
}

fn ran(command: &str, exit: Option<i64>, text: &str) -> TraceStep {
    let mut call = TraceStep::new(TraceKind::Command, "Ran")
        .detail(first_line(command))
        .meta(exit_label(exit));
    if exit != Some(0) {
        call = call.status(TraceStatus::Error);
    }
    if !text.trim().is_empty() {
        call = call.body(output(text.trim_end()));
    }
    call
}

fn first_line(text: &str) -> String {
    let line = text.lines().next().unwrap_or_default();
    if text.lines().count() > 1 {
        format!("{line} …")
    } else {
        line.to_owned()
    }
}

/// A line diff of two scripts, `diff` style: removed lines start with
/// `-`, added with `+`, kept with a space.
pub(crate) fn diff(before: &str, after: &str) -> String {
    let a: Vec<&str> = before.lines().collect();
    let b: Vec<&str> = after.lines().collect();
    if a.len() * b.len() > 250_000 {
        return after.lines().map(|l| format!("+{l}\n")).collect();
    }
    let mut lcs = vec![vec![0u32; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let (mut i, mut j, mut out) = (0, 0, String::new());
    while i < a.len() || j < b.len() {
        if i < a.len() && j < b.len() && a[i] == b[j] {
            out.push_str(&format!(" {}\n", a[i]));
            i += 1;
            j += 1;
        } else if j < b.len() && (i == a.len() || lcs[i][j + 1] >= lcs[i + 1][j]) {
            out.push_str(&format!("+{}\n", b[j]));
            j += 1;
        } else {
            out.push_str(&format!("-{}\n", a[i]));
            i += 1;
        }
    }
    out
}

fn stage_status(stage: Stage) -> ActivityStatus {
    match stage {
        Stage::Started => ActivityStatus::Running,
        Stage::Passed => ActivityStatus::Done,
        Stage::Failed => ActivityStatus::Failed,
    }
}

/// The build-and-check progress of one attempt, from its records.
fn progress(records: &[&Record]) -> Steps {
    let mut build = (ActivityStatus::Waiting, String::new());
    let mut check = (ActivityStatus::Waiting, String::new());
    for r in records {
        match &r.entry {
            Entry::Build { stage, detail } => build = (stage_status(*stage), detail.clone()),
            Entry::Verify { stage, detail } => check = (stage_status(*stage), detail.clone()),
            _ => {}
        }
    }
    let step = |label: &str, (status, detail): (ActivityStatus, String)| {
        let s = Step::new(label, status);
        if detail.is_empty() {
            s
        } else {
            s.detail(detail)
        }
    };
    Steps::new("Build and check")
        .step(step("Build a clean image from the recipe", build))
        .step(step("Check it on a fresh computer", check))
}

/// The whole conversation: every record, then what is happening now.
pub(crate) fn transcript(view: &View, claude: Claude, notice: Option<&str>) -> Markup {
    let id = &view.summary.id;
    let records = &view.records;
    let mut turns: Vec<Markup> = vec![];
    let last_ready = records
        .iter()
        .rposition(|r| matches!(r.entry, Entry::Ready { .. }));
    let last_failed = records
        .iter()
        .rposition(|r| matches!(r.entry, Entry::Failed { .. }));
    let mut i = 0;
    while i < records.len() {
        let r = &records[i];
        match &r.entry {
            Entry::User { text } => turns.push(Message::user(text).render()),
            Entry::Agent { text } => turns.push(says(text).render()),
            Entry::Question { text } => turns.push(says(text).render()),
            Entry::Starting => turns.push(note("Starting a setup computer").render()),
            Entry::Retried => turns.push(note("Trying again on a new setup computer").render()),
            Entry::Source {
                ok,
                revision,
                output: text,
            } => {
                let mut call = TraceStep::new(TraceKind::Tool, "Checked out the repository")
                    .icon(Icon::Folder)
                    .detail(short(revision).to_owned());
                if !ok {
                    call = call
                        .status(TraceStatus::Error)
                        .open(true)
                        .body(output(text));
                }
                turns.push(acts(line(call)).render());
            }
            Entry::Explored { .. } => {
                let start = i;
                while i + 1 < records.len()
                    && matches!(records[i + 1].entry, Entry::Explored { .. })
                {
                    i += 1;
                }
                let calls: Vec<TraceStep> = records[start..=i]
                    .iter()
                    .filter_map(|r| match &r.entry {
                        Entry::Explored {
                            command,
                            exit,
                            output,
                        } => Some(ran(command, *exit, output)),
                        _ => None,
                    })
                    .collect();
                let n = calls.len();
                turns.push(
                    acts(line(
                        TraceStep::new(TraceKind::Summary, "Explored the repository")
                            .meta(if n == 1 {
                                "1 command".to_owned()
                            } else {
                                format!("{n} commands")
                            })
                            .children(calls),
                    ))
                    .render(),
                );
            }
            Entry::Recipe {
                revision,
                script,
                previous,
            } => {
                let call = match previous {
                    None => TraceStep::new(TraceKind::Edit, "Wrote the install recipe")
                        .detail(format!("Revision {revision}"))
                        .open(true)
                        .body(
                            CodeBlock::new(script.clone())
                                .title("Recipe")
                                .language("sh")
                                .size(CodeSize::Sm)
                                .wrap_toggle(true),
                        ),
                    Some(before) => TraceStep::new(TraceKind::Edit, "Edited the install recipe")
                        .detail(format!("Revision {revision}"))
                        .open(true)
                        .body(
                            CodeBlock::new(diff(before, script))
                                .title("Change")
                                .language("diff")
                                .diff(true)
                                .size(CodeSize::Sm)
                                .wrap_toggle(true),
                        ),
                };
                turns.push(acts(line(call)).render());
            }
            Entry::Install {
                revision,
                exit,
                output: text,
            } => {
                let mut call = TraceStep::new(TraceKind::Command, "Ran the install")
                    .detail(format!("Revision {revision}"))
                    .meta(exit_label(*exit));
                if *exit != Some(0) {
                    call = call.status(TraceStatus::Error).open(true);
                }
                if !text.trim().is_empty() {
                    call = call.body(output(text.trim_end()));
                }
                turns.push(acts(line(call)).render());
            }
            Entry::Checks { checks } => {
                let list: String = checks
                    .iter()
                    .map(|c| format!("{}: {}\n", c.name, c.command))
                    .collect();
                turns.push(
                    acts(line(
                        TraceStep::new(TraceKind::Plan, "Chose the checks")
                            .icon(Icon::CheckCircle)
                            .detail(if checks.len() == 1 {
                                "1 check".to_owned()
                            } else {
                                format!("{} checks", checks.len())
                            })
                            .body(output(list.trim_end()).title("Checks")),
                    ))
                    .render(),
                );
            }
            Entry::CheckFailed {
                name,
                command,
                exit,
                output: text,
            } => {
                let mut body = format!("$ {command}\n");
                body.push_str(text.trim_end());
                turns.push(
                    acts(line(
                        TraceStep::new(TraceKind::Command, "Failed on the fresh computer")
                            .detail(name.clone())
                            .status(TraceStatus::Error)
                            .meta(exit_label(*exit))
                            .open(true)
                            .body(output(&body)),
                    ))
                    .render(),
                );
            }
            Entry::Build { .. } | Entry::Verify { .. } => {
                let start = i;
                while i + 1 < records.len()
                    && matches!(
                        records[i + 1].entry,
                        Entry::Build { .. } | Entry::Verify { .. }
                    )
                {
                    i += 1;
                }
                let group: Vec<&Record> = records[start..=i].iter().collect();
                turns.push(acts(progress(&group)).render());
            }
            Entry::Ready { summary } => {
                if Some(i) == last_ready && view.candidate.is_some() {
                    turns.push(acts(save_card(view)).render());
                } else if !summary.is_empty() {
                    turns.push(says(summary).render());
                }
            }
            Entry::Saved { number } => {
                turns.push(
                    acts(line(
                        TraceStep::new(TraceKind::Tool, "Saved environment")
                            .icon(Icon::CheckCircle)
                            .detail(format!("Version {number}")),
                    ))
                    .render(),
                );
                turns.push(
                    says(&format!(
                        "Saved as version {number}. New Claude Code runs start from it."
                    ))
                    .render(),
                );
            }
            Entry::Failed { reason } => {
                let current = Some(i) == last_failed && matches!(view.phase, Phase::Failed { .. });
                if current {
                    turns.push(acts(failed_card(id, reason)).render());
                } else {
                    turns.push(note(shown_reason(reason)).render());
                }
            }
        }
        i += 1;
    }
    match &view.phase {
        Phase::Starting => turns.push(note("Starting a setup computer…").render()),
        Phase::Working => turns.push(note("Working…").render()),
        Phase::Building => turns.push(note("Building a clean image…").render()),
        Phase::Verifying => turns.push(note("Checking a fresh computer…").render()),
        _ => {}
    }
    // A saved version stays runnable while a new setup attempt is open or
    // has stopped (#11316).
    if view.versions.iter().any(|v| v.selected) {
        turns.push(acts(claude_card(view, claude)).render());
    }
    if let Some(notice) = notice {
        turns.push(note(notice).render());
    }
    html! {
        @for (n, turn) in turns.iter().enumerate() {
            div id=(format!("env-turn-{n}")) { (turn) }
        }
    }
}

fn save_card(view: &View) -> Markup {
    let Some(c) = &view.candidate else {
        return html! {};
    };
    let checks = c.checks.len();
    let card = ResultCard::new(format!("{} environment", view.summary.repository))
        .subtitle("Ready to save")
        .badge(ActivityStatus::Done.badge("Checked"))
        .fact("Commit", html! { code { (short(&c.commit)) } })
        .fact("Branch", view.summary.branch.clone())
        .fact("Install recipe", format!("Revision {}", c.recipe_revision))
        .fact(
            "Checks",
            if checks == 1 {
                "1 of 1 passed".to_owned()
            } else {
                format!("{checks} of {checks} passed")
            },
        )
        .fact("Image", html! { code { (c.image) } });
    let card = if c.summary.trim().is_empty() {
        card
    } else {
        card.body(
            MarkdownRoot::new(PreEscaped(crate::markdown::render(&c.summary)))
                .size(MarkdownSize::Sm),
        )
    };
    card.footer(html! {
        form action=(format!("/environments/{}/save", view.summary.id)) method="post" {
            input type="hidden" name="candidate" value=(c.digest);
            (submit("Save environment", true))
        }
    })
    .render()
}

/// A stop reason as the person reads it: a provider's raw answer (an
/// HTTP status or JSON, kept by setups that stopped before #11316) becomes
/// one plain sentence.
pub(crate) fn shown_reason(reason: &str) -> String {
    let raw = reason.contains("{\"") || reason.contains("HTTP ");
    if !raw {
        return reason.to_owned();
    }
    if reason.starts_with("The setup agent couldn't reach its model") {
        return "The setup agent's model couldn't take the request. Try again.".into();
    }
    "The setup stopped on an error. Try again.".into()
}

fn failed_card(id: &str, reason: &str) -> Markup {
    ResultCard::new("Setup stopped")
        .badge(ActivityStatus::Failed.badge("Stopped"))
        .body(MarkdownRoot::new(html! { p { (shown_reason(reason)) } }).size(MarkdownSize::Sm))
        .footer(html! {
            form action=(format!("/environments/{id}/retry")) method="post" {
                (submit("Try again", true))
            }
        })
        .render()
}

/// What the Claude Code card offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Claude {
    /// A Claude key is available: the person's own, or this server's.
    Ready,
    /// The person signed in to Claude inside this environment (#11294).
    SignedIn,
    /// The person can add their own key in Settings.
    AddKey,
    /// This server keeps no keys and names none.
    Unavailable,
}

fn claude_card(view: &View, claude: Claude) -> Markup {
    let id = &view.summary.id;
    let version = view.versions.iter().find(|v| v.selected).map(|v| v.number);
    let title = match version {
        Some(n) => format!("Run Claude Code on version {n}"),
        None => "Run Claude Code".to_owned(),
    };
    let runs = html! {
        @if !view.runs.is_empty() {
            ul {
                @for r in &view.runs {
                    li {
                        a href=(format!("/environments/{id}/runs/{}", r.id)) { (first_line(&r.prompt)) }
                        " · " (run_word(r.state))
                    }
                }
            }
        }
    };
    let card = ResultCard::new(title).subtitle(if claude == Claude::SignedIn {
        "Claude Code runs on your computer for this environment, signed in to your Claude."
    } else {
        "Claude Code starts on a fresh computer made from this environment."
    });
    if matches!(claude, Claude::Ready | Claude::SignedIn) {
        card.body(runs)
            .footer(html! {
                form action=(format!("/environments/{id}/claude")) method="post" {
                    (field("env-claude-prompt", "What should Claude Code do?", None, |aria| {
                        Textarea::new("prompt").id("env-claude-prompt").rows(3).maxlength(16_000).aria(aria)
                    }))
                    (submit("Run Claude Code here", true))
                }
            })
            .render()
    } else {
        card.body(html! {
            (MarkdownRoot::new(html! {
                p { "To run Claude Code here, sign in to Claude once inside this environment, with Anthropic's own sign-in." }
                @if claude == Claude::AddKey {
                    p { "Or add your Claude key in " a href="/settings/claude" { "Settings" } "." }
                }
            }).size(MarkdownSize::Sm))
            (runs)
        })
        .footer(html! {
            (ButtonLink::new("Sign in to Claude", sign_in_path(id, None)))
        })
        .render()
    }
}

pub(crate) fn run_word(state: RunState) -> &'static str {
    match state {
        RunState::Starting => "Starting",
        RunState::Running => "Working",
        RunState::Paused => "Paused until the usage limit resets",
        RunState::Done => "Done",
        RunState::Failed => "Failed",
        RunState::Stopped => "Stopped",
    }
}

/// The form value naming a pause choice (#11293).
pub(crate) fn choice_key(choice: PauseChoice) -> &'static str {
    match choice {
        PauseChoice::KeepWaiting => "wait",
        PauseChoice::ContinueOnYourKey(_) => "key",
        PauseChoice::ContinueWithOpenAgents => "openagents",
    }
}

/// The button a pause choice shows; waiting shows no button, since it is
/// what happens anyway.
pub(crate) fn choice_label(choice: PauseChoice) -> Option<&'static str> {
    match choice {
        PauseChoice::KeepWaiting => None,
        PauseChoice::ContinueOnYourKey(_) => Some("Continue on your key"),
        PauseChoice::ContinueWithOpenAgents => Some("Continue with OpenAgents"),
    }
}

/// The choice a form's `key` names among those `offered`.
pub(crate) fn chosen(key: &str, offered: &[PauseChoice]) -> Option<PauseChoice> {
    offered
        .iter()
        .copied()
        .find(|choice| choice_key(*choice) == key)
}

/// What a paused run's card says about its choices.
pub(crate) fn pause_line(until: Option<u64>, offered: &[PauseChoice]) -> String {
    let when = until.map_or_else(
        || "when the usage limit resets".to_owned(),
        |at| format!("at {:02}:{:02} UTC", at / 3600 % 24, at / 60 % 60),
    );
    let more = offered.iter().any(|choice| choice_label(*choice).is_some());
    if more {
        format!("Paused by a Claude usage limit. It continues on its own {when}, or now:")
    } else {
        format!("Paused by a Claude usage limit. It continues on its own {when}.")
    }
}

/// The pause choices of a paused run, as buttons posting to `action`.
pub(crate) fn pause_choices(
    action: &str,
    hidden: Markup,
    offered: &[PauseChoice],
    htmx_target: Option<&str>,
) -> Markup {
    html! {
        @for choice in offered {
            @if let Some(label) = choice_label(*choice) {
                form.oa-pause-choice method="post" action=(action)
                    hx-post=[htmx_target.map(|_| action)]
                    hx-target=[htmx_target]
                    hx-swap=[htmx_target.map(|_| "outerHTML")] {
                    (hidden)
                    input type="hidden" name="choice" value=(choice_key(*choice));
                    (submit(label, matches!(choice, PauseChoice::ContinueOnYourKey(_))))
                }
            }
        }
    }
}

/// A Claude Code run as a conversation.
#[cfg(test)]
pub(crate) fn run_transcript(run: &Run) -> Markup {
    run_transcript_with(run, &[])
}

/// [`run_transcript`], with the pause choices `offered` to the viewer
/// while a usage limit pauses it.
pub(crate) fn run_transcript_with(run: &Run, offered: &[PauseChoice]) -> Markup {
    let mut turns: Vec<Markup> = vec![Message::user(&run.prompt).render()];
    if let Some(n) = run.version {
        turns.push(note(format!("Started from environment version {n}")).render());
    }
    let steps = claude::transcript(&run.events);
    let said = steps.iter().any(|s| matches!(s, claude::Step::Said(_)));
    for s in steps {
        match s {
            claude::Step::Said(text) => turns.push(says(&text).render()),
            claude::Step::Asked(text) => turns.push(Message::user(&text).render()),
            claude::Step::Tool { title, detail } => {
                let mut call = TraceStep::new(TraceKind::Command, title);
                if !detail.is_empty() {
                    call = call.detail(first_line(&detail));
                }
                turns.push(acts(line(call)).render());
            }
        }
    }
    if !said && let Some(reply) = &run.reply {
        turns.push(says(reply).render());
    }
    match run.state {
        RunState::Starting => {
            turns.push(note("Starting a computer from the environment…").render())
        }
        RunState::Running if run.openagents => turns.push(note("OpenAgents is working…").render()),
        RunState::Running => turns.push(note("Claude Code is working…").render()),
        RunState::Paused => {
            turns.push(note(pause_line(run.paused_until, offered)).render());
            turns.push(pause_choices(
                &format!("/environments/{}/runs/{}/choose", run.environment, run.id),
                html! {},
                offered,
                None,
            ));
        }
        RunState::Done => {}
        RunState::Failed => turns.push(
            note(
                run.error
                    .clone()
                    .unwrap_or_else(|| "The run failed.".into()),
            )
            .render(),
        ),
        RunState::Stopped => turns.push(note("Stopped").render()),
    }
    // What the run has cost: its computer's time plus Claude Code's own
    // reported cost, the same figure the chat's Agents panel shows.
    if let Some(cost) = run.cost_usd {
        let word = if run.state.finished() {
            "Cost"
        } else {
            "Cost so far"
        };
        turns.push(note(format!("{word}: {}", agent_fleet::dollars(cost))).render());
    }
    // Where the Claude plan stands, as Claude Code reported it in this run
    // (#11292); a window that has reset is left out.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    if let Some(plan) = coder_engine_status::PlanUsage::from_events(&run.events).and_then(|plan| {
        plan.text(now, &|ms| {
            let unix = ms / 1000;
            format!("{:02}:{:02} UTC", unix / 3600 % 24, unix / 60 % 60)
        })
    }) {
        turns.push(note(plan).render());
    }
    if !run.state.finished() {
        turns.push(html! {
            form action=(format!("/environments/{}/runs/{}/stop", run.environment, run.id)) method="post" {
                (submit("Stop", false))
            }
        });
    }
    html! {
        @for (n, turn) in turns.iter().enumerate() {
            div id=(format!("run-turn-{n}")) { (turn) }
        }
    }
}

/// What the environment page's composer is for, beside it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct DockFor {
    /// The environment has a saved version: a message changes its setup
    /// on a new setup computer, so the box says so (#11316).
    pub saved: bool,
    /// A Claude Code run of this environment that is working: its page
    /// takes messages for it, not this box (#11314).
    pub working_run: Option<String>,
}

impl DockFor {
    /// What `view` makes the composer for.
    pub(crate) fn of(view: &View) -> Self {
        Self {
            saved: view.versions.iter().any(|v| v.selected),
            working_run: view
                .runs
                .iter()
                .find(|run| !run.state.finished())
                .map(|run| run.id.clone()),
        }
    }
}

/// The docked composer that answers and steers the setup.
pub(crate) fn dock(id: &str, status: Option<&str>, oob: bool, purpose: &DockFor) -> Markup {
    let (label, placeholder) = if purpose.saved {
        (
            "Change this environment's setup",
            "Describe a change to the setup. It starts a new setup computer.",
        )
    } else {
        (
            "Message the setup agent",
            "Answer, or tell the agent what to change",
        )
    };
    let mut composer = Composer::new("env-composer", format!("/environments/{id}/message"))
        .label(label)
        .input_label(label)
        .placeholder(placeholder)
        .max_chars(MAX_MESSAGE)
        .rows(1);
    if let Some(status) = status {
        composer = composer.status(html! { (status) });
    } else if let Some(run) = &purpose.working_run {
        composer = composer.status(html! {
            "This box changes the environment's setup. To message the Claude Code run that is working, use "
            a href=(format!("/environments/{id}/runs/{run}")) { "its page" } "."
        });
    } else if purpose.saved {
        composer = composer.status(html! {
            "This box changes the environment's setup; your saved version keeps running Claude Code."
        });
    }
    html! {
        div #env-dock hx-swap-oob=[oob.then_some("outerHTML")] { (composer) }
    }
}

/// The run page's composer (#11314): a message for the run's next turn
/// while it works, or a follow-up run once it has ended. `waiting` are the
/// messages already waiting.
pub(crate) fn run_dock(run: &Run, waiting: &[String]) -> Markup {
    let (placeholder, hint) = if run.state.finished() {
        (
            "Ask for more. It starts a new run that picks up from this one.",
            "This run has ended. A message starts a new run with it.".to_owned(),
        )
    } else {
        (
            "Message Claude Code",
            match waiting.len() {
                0 => "Claude Code reads a message when its current turn ends, then carries on."
                    .to_owned(),
                1 => "1 message waits for its next turn. It reads it when its current turn ends."
                    .to_owned(),
                n => format!(
                    "{n} messages wait for its next turn. It reads them when its current turn ends."
                ),
            },
        )
    };
    let composer = Composer::new(
        "run-composer",
        format!("/environments/{}/runs/{}/message", run.environment, run.id),
    )
    .label("Message Claude Code")
    .input_label("Message Claude Code")
    .placeholder(placeholder)
    .max_chars(MAX_MESSAGE)
    .rows(1)
    .enhanced(false)
    .status(html! { (hint) });
    html! { div #run-dock { (composer) } }
}

/// The scrolling thread with its live stream.
pub(crate) fn thread(stream: &str, body: Markup) -> Markup {
    html! {
        section #env-thread.oa-thread aria-label="Setup conversation" {
            div.oa-thread-column hx-ext="sse" sse-connect=(stream) {
                div #env-transcript sse-swap="transcript" hx-swap="innerHTML" aria-live="polite" { (body) }
            }
        }
        (ScrollToBottom::new("#env-thread").follow())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diffs_show_removed_and_added_lines() {
        let d = diff("a\nb\nc", "a\nc\nd");
        assert_eq!(d, " a\n-b\n c\n+d\n");
    }
}

/// The sign-in page for environment `id`, coming back to `back`.
pub(crate) fn sign_in_path(id: &str, back: Option<&str>) -> String {
    let path = format!("/environments/{id}/sign-in");
    match back {
        Some(back) => format!(
            "{path}?{}",
            url::form_urlencoded::Serializer::new(String::new())
                .append_pair("back", back)
                .finish()
        ),
        None => path,
    }
}

/// The sign-in dialog's id.
pub(crate) const SIGN_IN_DIALOG: &str = "claude-sign-in";

/// Sign in to Claude inside environment `id` (#11294): a small terminal
/// running Claude Code's own `claude auth login` on the environment's
/// computer. `view` is the sign-in so far; `back` is the chat it came
/// from, whose message is sent once signed in.
pub(crate) fn sign_in_panel(id: &str, view: Option<&sign_in::View>, back: Option<&str>) -> Markup {
    let start_form = |label: &str| {
        html! {
            form method="post" action=(sign_in_url(id, "", back, None))
                hx-post=(sign_in_url(id, "", back, Some("panel")))
                hx-target="#claude-sign-in" hx-swap="outerHTML" {
                (Button::new(label).kind(ButtonType::Submit))
            }
        }
    };
    let phase = view.map(|v| &v.phase);
    html! {
        div #claude-sign-in.oa-sign-in {
            @match phase {
                None => {
                    p { "Claude Code's own sign-in opens in a terminal on your computer for this environment. Sign in with your Claude account, then paste the code Anthropic shows back here. OpenAgents never sees your login, and you sign in once for this environment." }
                    (start_form("Start sign-in"))
                }
                Some(sign_in::Phase::Starting) | Some(sign_in::Phase::Open) => {
                    (sign_in_screen(id, view, back))
                    (sign_in_input(id, back, None))
                }
                Some(sign_in::Phase::SignedIn) => {
                    p role="status" { "Signed in to Claude on your computer for this environment." }
                    @if back.is_some() {
                        // Sends the message waiting in the chat's box.
                        (Button::new("Send your message").kind(ButtonType::Submit).attr("form", "chat-form"))
                    } @else {
                        (ButtonLink::new("Back to the environment", format!("/environments/{id}")))
                    }
                }
                Some(sign_in::Phase::Failed(why)) => {
                    p role="alert" { (why) }
                    (start_form("Start again"))
                }
            }
        }
    }
}

/// Where the sign-in's screen is read from, keeping `back`.
fn sign_in_url(id: &str, tail: &str, back: Option<&str>, part: Option<&str>) -> String {
    let mut query = url::form_urlencoded::Serializer::new(String::new());
    if let Some(back) = back {
        query.append_pair("back", back);
    }
    if let Some(part) = part {
        query.append_pair("part", part);
    }
    let query = query.finish();
    let path = format!("/environments/{id}/sign-in{tail}");
    if query.is_empty() {
        path
    } else {
        format!("{path}?{query}")
    }
}

/// The line the person types into the terminal: the code Anthropic shows,
/// or Enter alone. It is sent straight to the computer and kept nowhere.
pub(crate) fn sign_in_input(id: &str, back: Option<&str>, error: Option<&str>) -> Markup {
    let field = Field::new("claude-sign-in-text", "Type or paste here, then Send").error_opt(error);
    html! {
        form #claude-sign-in-input.oa-sign-in-input method="post"
            action=(sign_in_url(id, "/input", back, None))
            hx-post=(sign_in_url(id, "/input", back, Some("input")))
            hx-swap="outerHTML" autocomplete="off" {
            (field.clone().control(
                Input::new("text")
                    .maxlength(2048)
                    .autocomplete("off")
                    .spellcheck(false)
                    .invalid(error.is_some())
                    .aria(field.aria())
            ))
            (Button::new("Send").kind(ButtonType::Submit))
        }
    }
}

/// The terminal's screen, read again every two seconds while sign-in is
/// open; once it ends, the whole panel comes back in its place.
pub(crate) fn sign_in_screen(id: &str, view: Option<&sign_in::View>, back: Option<&str>) -> Markup {
    html! {
        div #claude-sign-in-screen hx-get=(sign_in_url(id, "/screen", back, None))
            hx-trigger="every 2s" hx-swap="outerHTML" {
            @match view {
                Some(v) if v.phase == sign_in::Phase::Starting || v.screen.is_empty() => {
                    p role="status" { "Starting your computer for this environment. This can take a minute or two." }
                }
                Some(v) => {
                    @if let Some(link) = &v.link {
                        p {
                            a href=(link) target="_blank" rel="noopener noreferrer" { "Open Anthropic's sign-in page ↗" }
                        }
                    }
                    pre.oa-sign-in-screen aria-live="polite" { (v.screen) }
                }
                None => {}
            }
        }
    }
}
