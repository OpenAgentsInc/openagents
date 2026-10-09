//! The Environments markup, built from `openagents-ui` only: the list, the
//! repository picker, the setup conversation (the same activity components
//! as the demo), the Save card, and Claude Code runs.

use coder_environment_operator::activity::{Entry, Record, Stage};
use coder_environment_operator::agent::Phase;
use coder_environment_operator::studio::claude::{self, Run, RunState};
use coder_environment_operator::studio::{Status, Summary, View};
use maud::{Markup, PreEscaped, Render, html};
use openagents_ui::actions::{
    Button, ButtonLink, ButtonType, ButtonVariant, Color, ControlSize, EmptyMessage,
};
use openagents_ui::content::{
    ActivityStatus, CodeBlock, MarkdownRoot, MarkdownSize, PageColumn, ResultCard, Step, Steps,
    Table, ToolCall, ToolGroup,
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
    /// The person's repositories, when GitHub sign-in is available.
    pub mine: Option<&'a [coder_environment_operator::studio::github::Repo]>,
    pub repo: &'a str,
    pub error: Option<&'a str>,
}

/// `/environments/new`, step one: choose a repository.
pub(crate) fn pick(p: &Pick<'_>) -> Markup {
    PageColumn::new(html! {
        (MarkdownRoot::new(html! {
            h1 { "New environment" }
            p { "Choose a GitHub repository. An agent will work out how to install it, check the result on a fresh computer, and ask you to save it." }
        }))
        form action="/environments/new" method="get" {
            @if let Some(mine) = p.mine {
                @if !mine.is_empty() {
                    (field("env-pick", "Your repositories", None, |aria| {
                        let mut select = Select::new("pick").id("env-pick").aria(aria).placeholder("Choose a repository");
                        for r in mine {
                            select = select.option(r.full_name.clone(), r.full_name.clone());
                        }
                        select
                    }))
                }
            }
            (field("env-repo", if p.mine.is_some_and(|m| !m.is_empty()) { "Or paste a GitHub address" } else { "GitHub repository" }, p.error, |aria| {
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
        form action="/environments" method="post" {
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
    Message::assistant(MarkdownRoot::new(PreEscaped(crate::markdown::render(
        markdown,
    ))))
    .author(AUTHOR)
}

fn acts(content: impl Render) -> Message {
    Message::assistant(content).author(AUTHOR)
}

fn output(text: &str) -> CodeBlock {
    CodeBlock::new(text).copyable(false)
}

fn exit_label(exit: Option<i64>) -> String {
    match exit {
        Some(0) => "Exit 0".into(),
        Some(n) => format!("Exit {n}"),
        None => "Didn't finish".into(),
    }
}

fn ran(command: &str, exit: Option<i64>, text: &str) -> ToolCall {
    let mut call = ToolCall::new(Icon::Terminal, "Ran")
        .detail(first_line(command))
        .status_label(exit_label(exit));
    if exit != Some(0) {
        call = call.status(ActivityStatus::Failed);
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
pub(crate) fn transcript(view: &View, claude_ready: bool, notice: Option<&str>) -> Markup {
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
            Entry::Starting => turns.push(Message::status("Starting a setup computer").render()),
            Entry::Retried => {
                turns.push(Message::status("Trying again on a new setup computer").render())
            }
            Entry::Source {
                ok,
                revision,
                output: text,
            } => {
                let mut call = ToolCall::new(Icon::Folder, "Checked out the repository")
                    .detail(short(revision).to_owned());
                if !ok {
                    call = call
                        .status(ActivityStatus::Failed)
                        .open(true)
                        .body(output(text));
                }
                turns.push(acts(call).render());
            }
            Entry::Explored { .. } => {
                let start = i;
                while i + 1 < records.len()
                    && matches!(records[i + 1].entry, Entry::Explored { .. })
                {
                    i += 1;
                }
                let calls: Vec<ToolCall> = records[start..=i]
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
                    acts(
                        ToolGroup::new("Explored the repository")
                            .meta(if n == 1 {
                                "1 command".to_owned()
                            } else {
                                format!("{n} commands")
                            })
                            .calls(calls),
                    )
                    .render(),
                );
            }
            Entry::Recipe {
                revision,
                script,
                previous,
            } => {
                let call = match previous {
                    None => ToolCall::new(Icon::Pencil, "Wrote the install recipe")
                        .detail(format!("Revision {revision}"))
                        .open(true)
                        .body(CodeBlock::new(script.clone()).language("sh")),
                    Some(before) => ToolCall::new(Icon::Pencil, "Edited the install recipe")
                        .detail(format!("Revision {revision}"))
                        .open(true)
                        .body(CodeBlock::new(diff(before, script)).language("diff")),
                };
                turns.push(acts(call).render());
            }
            Entry::Install {
                revision,
                exit,
                output: text,
            } => {
                let mut call = ToolCall::new(Icon::Terminal, "Ran the install")
                    .detail(format!("Revision {revision}"))
                    .status_label(exit_label(*exit));
                if *exit != Some(0) {
                    call = call.status(ActivityStatus::Failed).open(true);
                }
                if !text.trim().is_empty() {
                    call = call.body(output(text.trim_end()));
                }
                turns.push(acts(call).render());
            }
            Entry::Checks { checks } => {
                let list: String = checks
                    .iter()
                    .map(|c| format!("{}: {}\n", c.name, c.command))
                    .collect();
                turns.push(
                    acts(
                        ToolCall::new(Icon::CheckCircle, "Chose the checks")
                            .detail(if checks.len() == 1 {
                                "1 check".to_owned()
                            } else {
                                format!("{} checks", checks.len())
                            })
                            .body(output(list.trim_end())),
                    )
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
                    acts(
                        ToolCall::new(Icon::Terminal, "Failed on the fresh computer")
                            .detail(name.clone())
                            .status(ActivityStatus::Failed)
                            .status_label(exit_label(*exit))
                            .open(true)
                            .body(output(&body)),
                    )
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
                    acts(
                        ToolCall::new(Icon::CheckCircle, "Saved environment")
                            .detail(format!("Version {number}")),
                    )
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
                    turns.push(Message::status(reason).render());
                }
            }
        }
        i += 1;
    }
    match &view.phase {
        Phase::Starting => turns.push(Message::status("Starting a setup computer…").render()),
        Phase::Working => turns.push(Message::status("Working…").render()),
        Phase::Building => turns.push(Message::status("Building a clean image…").render()),
        Phase::Verifying => turns.push(Message::status("Checking a fresh computer…").render()),
        Phase::Saved { .. } => turns.push(acts(claude_card(view, claude_ready)).render()),
        _ => {}
    }
    if let Some(notice) = notice {
        turns.push(Message::status(notice).render());
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

fn failed_card(id: &str, reason: &str) -> Markup {
    ResultCard::new("Setup stopped")
        .badge(ActivityStatus::Failed.badge("Stopped"))
        .body(MarkdownRoot::new(html! { p { (reason) } }).size(MarkdownSize::Sm))
        .footer(html! {
            form action=(format!("/environments/{id}/retry")) method="post" {
                (submit("Try again", true))
            }
        })
        .render()
}

fn claude_card(view: &View, ready: bool) -> Markup {
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
    let card = ResultCard::new(title)
        .subtitle("Claude Code starts on a fresh computer made from this environment.");
    if ready {
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
            (MarkdownRoot::new(html! { p { "To run Claude Code here, add your Anthropic API key to this server's settings." } }).size(MarkdownSize::Sm))
            (runs)
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

/// A Claude Code run as a conversation.
pub(crate) fn run_transcript(run: &Run) -> Markup {
    let mut turns: Vec<Markup> = vec![Message::user(&run.prompt).render()];
    if let Some(n) = run.version {
        turns.push(Message::status(format!("Started from environment version {n}")).render());
    }
    let steps = claude::transcript(&run.events);
    let said = steps.iter().any(|s| matches!(s, claude::Step::Said(_)));
    for s in steps {
        match s {
            claude::Step::Said(text) => turns.push(says(&text).render()),
            claude::Step::Tool { title, detail } => {
                let mut call = ToolCall::new(Icon::Terminal, title);
                if !detail.is_empty() {
                    call = call.detail(first_line(&detail));
                }
                turns.push(acts(call).render());
            }
        }
    }
    if !said && let Some(reply) = &run.reply {
        turns.push(says(reply).render());
    }
    match run.state {
        RunState::Starting => {
            turns.push(Message::status("Starting a computer from the environment…").render())
        }
        RunState::Running => turns.push(Message::status("Claude Code is working…").render()),
        RunState::Paused => turns.push(Message::status(run_word(run.state)).render()),
        RunState::Done => {}
        RunState::Failed => turns.push(
            Message::status(
                run.error
                    .clone()
                    .unwrap_or_else(|| "The run failed.".into()),
            )
            .render(),
        ),
        RunState::Stopped => turns.push(Message::status("Stopped").render()),
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

/// The docked composer that answers and steers the setup.
pub(crate) fn dock(id: &str, status: Option<&str>, oob: bool) -> Markup {
    let mut composer = Composer::new("env-composer", format!("/environments/{id}/message"))
        .label("Message the setup agent")
        .input_label("Message")
        .placeholder("Answer, or tell the agent what to change")
        .max_chars(MAX_MESSAGE)
        .rows(1);
    if let Some(status) = status {
        composer = composer.status(html! { (status) });
    }
    html! {
        div #env-dock hx-swap-oob=[oob.then_some("outerHTML")] { (composer) }
    }
}

/// The scrolling thread with its live stream.
pub(crate) fn thread(stream: &str, body: Markup) -> Markup {
    html! {
        section #env-thread.oa-thread aria-label="Setup conversation" {
            div.oa-thread-column hx-ext="sse" sse-connect=(stream) {
                div #env-transcript sse-swap="transcript" hx-swap="innerHTML" aria-live="polite" { (body) }
            }
        }
        (ScrollToBottom::new("#env-thread"))
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
