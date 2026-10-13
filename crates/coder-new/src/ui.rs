//! The same render function draws the terminal and exported previews.

pub(crate) mod agents;
mod appearance;
mod models;
mod plugins;
mod preferences;
mod resume;
mod rewind;

use coder_terminal::selection::{self, RowCopy};
use coder_terminal::{
    Colors, Ladder,
    components::turn::{markdown_body, markdown_rows},
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::{
    App, Mode, Screen, ViewMode,
    agents::{DEMOS, DemoMessage, MAIN_PLUGINS, MAIN_TOOLS, elapsed_time},
    plugin_definition::RailSlot,
    theme as t,
    tools::{delegation_lines, plugin_lines, tool_lines},
};

fn span(text: impl Into<String>, color: Color) -> Span<'static> {
    Span::styled(text.into(), Style::default().fg(color))
}

pub fn render(frame: &mut Frame, app: &mut App) {
    // Set again below only when the input is drawn.
    app.composer_text = None;
    render_contents(frame, app);
    // The terminal's own background shows through where Coder paints its
    // base; the bands on top (diff rows, your messages, Run output) stay.
    if app.appearance.use_system_terminal_background {
        for cell in &mut frame.buffer_mut().content {
            if cell.bg == t::BG_BASE {
                cell.bg = Color::Reset;
            }
        }
    }
}

fn render_contents(frame: &mut Frame, app: &mut App) {
    if app.screen == Screen::Appearance {
        appearance::render(frame, frame.area(), app);
        return;
    }
    if app.screen == Screen::Preferences {
        preferences::render(frame, frame.area(), app);
        return;
    }
    // Both composers wrap at the terminal width less the frame and prompt gutter, and
    // Up/Down move through exactly that layout.
    app.composer_width = frame
        .area()
        .width
        .saturating_sub(if app.mode == Mode::Demo { 3 } else { 4 })
        .max(1);
    if app.mode == Mode::Demo {
        let mut demo = app.demo_view();
        coder_demo_ui::render(frame, &mut demo);
        app.scroll = demo.scroll;
        return;
    }
    let area = frame.area();
    let terminal_width = area.width;
    let terminal_x = area.x;
    frame.render_widget(
        Block::default().style(Style::default().bg(t::BG_BASE).fg(t::TEXT_SECONDARY)),
        area,
    );
    if area.width < 24 || area.height < 12 {
        frame.render_widget(
            Paragraph::new("Coder\nResize to continue.").wrap(Wrap { trim: false }),
            area,
        );
        return;
    }

    let top_padding = u16::from(app.screen != Screen::Conversation || app.selected_agent.is_some());
    let area = Rect {
        x: area.x + 2,
        y: area.y + top_padding,
        width: area.width.saturating_sub(4),
        height: area.height.saturating_sub(top_padding),
    };
    if let Some(screen) = &app.question {
        frame.render_widget(
            Paragraph::new(screen.text()).wrap(Wrap { trim: false }),
            area,
        );
        return;
    }
    if let Some(event) = &app.disclosure_event {
        let text = if event["kind"] == "action" {
            crate::ops_tool::question_screen(event)
        } else if event["kind"] == "computer" {
            format!(
                "Run this on {}?\n\n{}\n\nIt asks because {}. This approves this one action only.\n\nY: confirm · N: reject · Esc: cancel · PgUp/PgDn: review",
                event["host"].as_str().unwrap_or_default(),
                event["command"].as_str().unwrap_or_default(),
                event["why"].as_str().unwrap_or_default(),
            )
        } else {
            let input = serde_json::to_string_pretty(&event["input"]).unwrap_or_default();
            format!(
                "Send this exact lookup to Brainstorm?\n\nRecipient: {}\n\n{}\n\nNo files or conversation are added. This approves this lookup input only.\n\nY: confirm · N: reject · Esc: cancel · PgUp/PgDn: review",
                event["recipient"].as_str().unwrap_or_default(),
                input
            )
        };
        let paragraph = Paragraph::new(text).wrap(Wrap { trim: false });
        let lines = paragraph.line_count(area.width);
        let max_scroll = lines
            .saturating_sub(usize::from(area.height))
            .min(usize::from(u16::MAX)) as u16;
        app.disclosure_scroll = app.disclosure_scroll.min(max_scroll);
        app.disclosure_seen |= app.disclosure_scroll == max_scroll;
        frame.render_widget(paragraph.scroll((app.disclosure_scroll, 0)), area);
        return;
    }
    if app.resume_picker.is_some() {
        resume::render(frame, area, app);
        return;
    }
    if app.rewind_picker.is_some() {
        rewind::render(frame, area, app);
        return;
    }
    if app.agents_panel.is_some() {
        agents::render(frame, area, app);
        return;
    }
    if matches!(app.screen, Screen::Plugins | Screen::PluginSettings) {
        plugins::render(frame, area, app);
        if app.model_picker.is_some() {
            models::render(frame, app);
        }
        return;
    }
    let (draft, cursor) = app.draft.wrapped(app.composer_width);
    let rail_height = (if app.mode == Mode::Demo {
        DEMOS.len()
    } else {
        app.rail_agents().len()
    })
    .min(usize::from(area.height.saturating_sub(6))) as u16;
    let composer_height = (draft.len() as u16).clamp(1, 6) + 2;
    let header_height = if new_chat_heading(app) {
        2
    } else {
        u16::from(app.selected_agent.is_some())
    };
    // One blank row above the input, so the transcript's last message is
    // never pressed against the input's top rule.
    let padding_height = 1;
    // Under the input sits the agent rail alone; everything else rides the
    // input's own rules (owner, 2026-10-12).
    let reserved = rail_height + 1 + padding_height + header_height;
    let [header, body, queued, _padding, composer, rail] = Layout::vertical([
        Constraint::Length(header_height),
        Constraint::Min(1),
        Constraint::Length(
            app.queued_prompts
                .iter()
                .filter(|p| p.editable)
                .count()
                .min(3) as u16,
        ),
        Constraint::Length(padding_height),
        Constraint::Length(composer_height.min(area.height.saturating_sub(reserved))),
        Constraint::Length(rail_height),
    ])
    .areas(area);
    header_view(frame, header, app);
    // The queued rows are the person's pending messages; a notice (a
    // monitor line, an agent's report) is a delivery, not input.
    let pending = app
        .queued_prompts
        .iter()
        .filter(|p| p.editable)
        .map(|p| {
            Line::from(span(
                format!("Queued: {}", p.text.replace('\n', " ")),
                t::GRAY,
            ))
        })
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(pending), queued);
    match app.screen {
        // The transcript runs edge to edge, as Claude Code's does: the
        // person's `❯` and the replies' `●` sit in column 0.
        Screen::Conversation => conversation(
            frame,
            Rect {
                x: terminal_x,
                width: terminal_width,
                ..body
            },
            app,
        ),
        Screen::Plugins | Screen::PluginSettings | Screen::Appearance | Screen::Preferences => {
            unreachable!()
        }
    }
    let hints = app.slash_hints();
    let height = (hints.len() as u16).min(composer.y.saturating_sub(body.y));
    crate::slash::render(
        frame,
        Rect {
            x: terminal_x,
            width: terminal_width,
            y: composer.y.saturating_sub(height),
            height,
        },
        &hints,
        app.slash_selected,
        app.mode == Mode::Demo,
    );
    let working = working_label(app);
    // A transcript selection reached the clipboard: the input's top-left
    // rail says so for a moment, where it never pushes the transcript.
    let copied = app
        .copied_until
        .is_some_and(|until| std::time::Instant::now() < until);
    // What failed to load when the Claude session started flashes on the
    // input's top-right rail for a few seconds, never as a transcript row.
    let load_warning = app
        .on_claude()
        .then(|| crate::claude_stream::load_warning(std::time::Instant::now()))
        .flatten();
    let drawn = composer_view(
        frame,
        Rect {
            x: terminal_x,
            width: terminal_width,
            ..composer
        },
        &draft,
        cursor,
        app.selected_agent.is_none(),
        app.model_picker.is_none() && !app.footer_focused,
        &app.plugins,
        &app.composer,
        app.composer_history.position(),
        app.composer_selected && !app.draft.text.is_empty(),
        // OpenAgents picks the model here, so the rail says `auto` and never
        // the name of whichever vendor answered.
        rail_harness(app).as_deref(),
        top_rail(app).as_deref(),
        load_warning.as_deref(),
        working.as_deref(),
        copied,
    );
    app.composer_text = drawn;
    let path = repository_rail(app);
    let composer_area = Rect {
        x: terminal_x,
        width: terminal_width,
        ..composer
    };
    // A passing status shares the bottom rail with the path, on the left.
    let status = app
        .rail_status
        .as_ref()
        .filter(|(until, _)| std::time::Instant::now() < *until)
        .map(|(_, text)| text.as_str());
    let status_width = status.map_or(0, |s| s.chars().count() as u16 + 3);
    let path = truncate(&path, terminal_width.saturating_sub(6 + status_width));
    coder_terminal::rail(
        composer_area,
        frame.buffer_mut(),
        composer.height.saturating_sub(1),
        status.map(|s| (s, Style::default().fg(t::GRAY))),
        Some((&path, Style::default().fg(t::GRAY))),
    );
    agent_rail(frame, rail, app);
    if app.model_picker.is_some() {
        models::render(frame, app);
    }
}

/// A delegation's name on the rail: the agent and, once it says, its model
/// in parentheses, as "Claude Code (Haiku 5.5)" or "Codex (gpt-6.1-sol)".
fn agent_label<'a>(name: &'a str, model: Option<&str>) -> std::borrow::Cow<'a, str> {
    match model.filter(|model| !model.is_empty() && !name.contains('(')) {
        Some(model) => std::borrow::Cow::Owned(format!("{name} ({})", model_words(model))),
        None => std::borrow::Cow::Borrowed(name),
    }
}

/// The model an agent last named: the reply it is writing, else the newest
/// finished reply that said, so the rail keeps it between replies.
fn agent_model(chat: &crate::live::Chat) -> Option<&str> {
    chat.partial_model.as_deref().or_else(|| {
        chat.entries.iter().rev().find_map(|entry| match entry {
            crate::live::Entry::Assistant {
                model: Some(model), ..
            } => Some(model.as_str()),
            _ => None,
        })
    })
}

/// What a running agent is doing, on one line: the line it is writing, else
/// its newest step: a thought, or a tool and what it was given ("Bash ·
/// run the tests"). `None` before it has done anything.
fn agent_activity(chat: &crate::live::Chat) -> Option<String> {
    fn latest_line(text: &str) -> Option<String> {
        text.lines()
            .map(str::trim)
            .rfind(|line| !line.is_empty())
            .map(str::to_owned)
    }
    if let Some(line) = latest_line(&chat.partial) {
        return Some(line);
    }
    chat.entries.iter().rev().find_map(|entry| match entry {
        crate::live::Entry::Tool { name, output, .. } if name == crate::claude_stream::THINKING => {
            latest_line(crate::claude_stream::entry_text(output))
        }
        crate::live::Entry::Tool { name, .. } if name == crate::claude_stream::NOTICE => None,
        crate::live::Entry::Tool { name, input, .. } => {
            let tool = crate::claude_tools::display_name(name);
            // A shell call's own description reads better than its command.
            let what = input
                .get("description")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| crate::tools::compact_params(input, 120));
            Some(if what.is_empty() {
                tool
            } else {
                format!("{tool} · {what}")
            })
        }
        crate::live::Entry::Assistant { text, .. } => latest_line(text),
        crate::live::Entry::User(_) | crate::live::Entry::Delegation { .. } => None,
    })
}

/// A model's name as a person reads it: a Claude model by its picker name
/// ("Opus 5.5"), any other by its own short name.
fn model_words(model: &str) -> String {
    let bare = model.rsplit('/').next().unwrap_or(model);
    crate::claude::MODELS
        .iter()
        .find(|(id, _, _)| *id == bare)
        .map_or_else(
            || crate::models::label(model),
            |(_, name, _)| (*name).to_owned(),
        )
}

fn agent_rail(frame: &mut Frame, area: Rect, app: &App) {
    // A finished delegation leaves the rail after a while (owner, #11434).
    let rail = app.rail_agents();
    let agents: Vec<_> = if app.mode == Mode::Demo {
        DEMOS
            .iter()
            .map(|agent| {
                (
                    std::borrow::Cow::Borrowed(agent.name),
                    std::borrow::Cow::Borrowed(agent.task),
                    agent.tokens.to_owned(),
                    agent.elapsed_seconds.saturating_add(app.elapsed_seconds),
                    RailState::Running,
                )
            })
            .collect()
    } else {
        rail.iter()
            .map(|index| &app.delegations[*index])
            .map(|agent| {
                if agent.background
                    && let Some(row) = app.fleet.get(&agent.id)
                {
                    let mut task = format!("{} · ", row.status.word());
                    if let Some(cost) = row.cost_usd {
                        task.push_str(&agent_fleet::dollars(cost));
                        task.push_str(" · ");
                    }
                    match agent_activity(&agent.chat).filter(|_| agent.running) {
                        Some(doing) => task.push_str(&doing),
                        None => task.push_str(&agent.task),
                    }
                    return (
                        agent_label(&agent.name, agent_model(&agent.chat)),
                        std::borrow::Cow::Owned(task),
                        if row.tokens == 0 {
                            String::new()
                        } else {
                            token_count(row.tokens)
                        },
                        row.elapsed_seconds(agent_fleet::now_ms()),
                        RailState::of(agent),
                    );
                }
                (
                    agent_label(&agent.name, agent_model(&agent.chat)),
                    // What a running agent is doing right now: its tool, its
                    // thinking, or the line it is writing; its task until
                    // it has done anything.
                    if agent.running
                        && let Some(doing) = agent_activity(&agent.chat)
                    {
                        std::borrow::Cow::Owned(doing)
                    // A finished delegation's dollars (#11179).
                    } else if agent.chat.cost_usd > 0.0 {
                        std::borrow::Cow::Owned(format!(
                            "{} · {}",
                            agent_fleet::dollars(agent.chat.cost_usd),
                            agent.task
                        ))
                    } else {
                        std::borrow::Cow::Borrowed(agent.task.as_str())
                    },
                    // A figure Coder does not have is left out, never
                    // shown as a dash or 0s (#11482).
                    if agent.chat.tokens == 0 {
                        String::new()
                    } else {
                        token_count(agent.chat.tokens)
                    },
                    if agent.running {
                        app.elapsed_seconds.saturating_sub(agent.started_at)
                    } else {
                        agent.elapsed_seconds
                    },
                    RailState::of(agent),
                )
            })
            .collect()
    };
    if area.height == 0 {
        return;
    }
    let name_width = (2 + agents
        .iter()
        .map(|agent| agent.0.width())
        .max()
        .unwrap_or(0)) as u16;
    let narrow = area.width < 32;
    let count_width = agents
        .iter()
        .map(|agent| agent.2.width())
        .max()
        .unwrap_or(0);
    let token_labels: Vec<_> = agents
        .iter()
        .map(|agent| {
            if agent.2.is_empty() {
                String::new()
            } else if narrow {
                format!("{:>count_width$}↓", agent.2)
            } else {
                format!("{:>count_width$} tokens ↓", agent.2)
            }
        })
        .collect();
    let timed_labels: Vec<_> = agents
        .iter()
        .zip(&token_labels)
        .map(|(agent, tokens)| {
            // No time yet (an agent that just started, or one whose end
            // came with none) is left out too.
            match (agent.3, tokens.is_empty()) {
                (0, _) => tokens.clone(),
                (seconds, true) => elapsed_time(seconds),
                (seconds, false) => format!("{} · {tokens}", elapsed_time(seconds)),
            }
        })
        .collect();
    let show_elapsed = usize::from(area.width)
        >= usize::from(name_width)
            + 1
            + timed_labels
                .iter()
                .map(|text| text.width())
                .max()
                .unwrap_or(0);
    let first = app
        .selected_agent
        .and_then(|selected| rail.iter().position(|index| *index == selected))
        .unwrap_or(agents.len().saturating_sub(1))
        .saturating_sub(usize::from(area.height).saturating_sub(1));
    for (index, agent) in agents
        .iter()
        .enumerate()
        .skip(first)
        .take(usize::from(area.height))
    {
        let row = Rect {
            y: area.y + (index - first) as u16,
            height: 1,
            ..area
        };
        let active = app.selected_agent == rail.get(index).copied() && app.mode != Mode::Demo
            || app.mode == Mode::Demo && app.selected_agent == Some(index);
        if active {
            frame.render_widget(
                Block::default().style(Style::default().bg(t::BG_DARK)),
                Rect {
                    x: frame.area().x,
                    width: frame.area().width,
                    ..row
                },
            );
        }
        let suffix = if show_elapsed {
            &timed_labels[index]
        } else {
            &token_labels[index]
        };
        let token_width = suffix.width() as u16;
        let [activity, _gap, tokens] = Layout::horizontal([
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(token_width),
        ])
        .areas(row);
        // The mark says where the agent stands (#11482).
        let prefix = if active {
            "❯ "
        } else if narrow {
            "  "
        } else {
            agent.4.mark()
        };
        let name = Rect {
            width: name_width.min(activity.width),
            ..activity
        };
        let task_width = activity.width.saturating_sub(name_width + 2);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                span(prefix, if active { t::ACCENT_MODEL } else { t::GRAY }),
                Span::styled(
                    agent.0.to_owned(),
                    Style::default()
                        .fg(if active {
                            t::ACCENT_MODEL
                        } else {
                            t::TEXT_SECONDARY
                        })
                        .add_modifier(Modifier::BOLD),
                ),
            ])),
            name,
        );
        if task_width > 0 {
            let task = Rect {
                x: activity.x + name_width + 2,
                width: task_width,
                ..activity
            };
            frame.render_widget(
                Paragraph::new(span(truncate(&agent.1, task_width), t::GRAY)),
                task,
            );
        }
        frame.render_widget(
            Paragraph::new(span(suffix, t::GRAY)).right_aligned(),
            tokens,
        );
    }
}

/// Where an agent on the rail stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RailState {
    Running,
    Done,
    Failed,
}

impl RailState {
    fn of(agent: &crate::live::Delegation) -> Self {
        if agent.running {
            Self::Running
        } else if agent.chat.notice.is_some() {
            Self::Failed
        } else {
            Self::Done
        }
    }

    const fn mark(self) -> &'static str {
        match self {
            Self::Running => "● ",
            Self::Done => "✓ ",
            Self::Failed => "✗ ",
        }
    }
}

fn token_count(tokens: u64) -> String {
    if tokens >= 1_000 {
        format!("{:.1}k", tokens as f64 / 1_000.0)
    } else {
        tokens.to_string()
    }
}

pub(crate) fn truncate(text: &str, width: u16) -> String {
    if text.width() <= usize::from(width) {
        return text.into();
    }
    let mut result = String::new();
    let mut cells = 0;
    for grapheme in text.graphemes(true) {
        let next = grapheme.width();
        if cells + next >= usize::from(width) {
            break;
        }
        result.push_str(grapheme);
        cells += next;
    }
    if width > 0 {
        result.push('…');
    }
    result
}

fn new_chat_heading(app: &App) -> bool {
    matches!(app.mode, Mode::Live | Mode::Solver)
        && app.screen == Screen::Conversation
        && app.selected_agent.is_none()
        && app.live.entries.is_empty()
        && app.live.partial.is_empty()
        && !app.live.busy
}

fn header_view(frame: &mut Frame, area: Rect, app: &App) {
    if new_chat_heading(app) {
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(span("OpenAgents Coder", t::TEXT_PRIMARY)),
                Line::from(span(
                    concat!("v", env!("CARGO_PKG_VERSION")),
                    t::GRAY_BRIGHT,
                )),
            ]),
            area,
        );
        return;
    }
    let agent = if app.screen == Screen::Conversation {
        app.selected_agent.and_then(|index| {
            if app.mode == Mode::Demo {
                DEMOS.get(index).map(|agent| agent.name)
            } else {
                app.delegations.get(index).map(|agent| agent.name.as_str())
            }
        })
    } else {
        None
    };
    if let Some(agent) = agent {
        let model = app
            .selected_agent
            .and_then(|index| app.delegations.get(index))
            .and_then(|agent| agent_model(&agent.chat));
        let mut title = vec![Span::styled(
            agent.to_owned(),
            Style::default()
                .fg(t::ACCENT_MODEL)
                .add_modifier(Modifier::BOLD),
        )];
        if let Some(model) = model {
            title.push(span(
                truncate(
                    &format!(" · {}", crate::models::label(model)),
                    area.width.saturating_sub(agent.width() as u16),
                ),
                t::GRAY_BRIGHT,
            ));
        }
        frame.render_widget(Paragraph::new(Line::from(title)), area);
    }
}

fn display_directory(path: &std::path::Path, home: Option<&std::path::Path>) -> String {
    if let Some(relative) = home.and_then(|home| path.strip_prefix(home).ok()) {
        if relative.as_os_str().is_empty() {
            "~".into()
        } else {
            format!("~/{}", relative.display())
        }
    } else {
        path.display().to_string()
    }
}

fn repository_rail(app: &App) -> String {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let directory = app
        .cwd
        .as_deref()
        .map(|path| display_directory(path, home.as_deref()))
        .unwrap_or_else(|| "openagents".into());
    let mut text = directory;
    if let Some(branch) = &app.branch {
        text.push_str(&format!(" ({branch})"));
    }
    let extra = context_text(app);
    if !extra.is_empty() {
        text.push_str(" · ");
        text.push_str(&extra);
    }
    text
}

/// What the input's top-right rule carries after the harness's name: the
/// Claude plan's windows and how full the context is when Claude Code runs
/// the chat ("Claude (Opus 5.5) · 5h 12% · week 15%"), else what the chat
/// has cost, which is metered spend; the Claude plan is paid by the month
/// rather than by the turn. Nothing of Claude's shows on another harness
/// or before the first turn (#11483).
fn top_rail(app: &App) -> Option<String> {
    let mut parts = Vec::new();
    if app.on_claude() {
        if let Some(plan) = crate::claude_plan::rail_text(atif::now_ms() / 1000) {
            parts.push(plan);
        }
        if let Some(context) = app
            .live
            .claude_session
            .as_ref()
            .and_then(|session| crate::claude_stream::context_line(&session.id))
        {
            parts.push(context);
        }
    }
    // Codex's windows, the same way, only while Codex runs the chat
    // (#11510).
    if app.on_codex() {
        let windows = crate::codex::windows_text(atif::now_ms() / 1000);
        if !windows.is_empty() {
            parts.push(windows);
        }
    }
    let cost = crate::long_session::session_cost(app);
    if cost > 0.0 && !app.on_claude() && !app.on_codex() {
        parts.push(agent_fleet::dollars(cost));
    }
    // How full the Coder loop's context is, as Claude's rail shows its
    // own (#11519).
    if let Some(context) = crate::long_session::context_line(app) {
        parts.push(context);
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// A model id as a person reads it: Claude's and Codex's names from
/// /models, else the model's own words capitalized
/// (`google/gemini-3.8-flash` reads "Gemini 3.8 Flash").
fn rail_model_name(model: &str) -> String {
    if crate::claude::MODELS.iter().any(|(id, _, _)| *id == model) {
        return crate::claude::name(model);
    }
    let base = model.split(':').next().unwrap_or(model);
    let name = base.rsplit('/').next().unwrap_or(base);
    name.split(['-', '_'])
        .filter(|word| !word.is_empty())
        .map(|word| match word {
            "gpt" | "glm" | "oss" => word.to_ascii_uppercase(),
            _ => {
                let mut chars = word.chars();
                chars.next().map_or_else(String::new, |first| {
                    first.to_uppercase().chain(chars).collect()
                })
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Which harness ran the chat, on the input's top-right rail: "Claude
/// (Haiku 5.5)" or "Coder (auto)" (the model's own name when the person
/// chose one in /models), `solver` while `coder solver` watches a run.
/// Before a chat's first turn is routed it says only `coder`, the product,
/// or the model the person chose. There is no mode to name (#11453).
fn rail_harness(app: &App) -> Option<String> {
    use crate::harness_plugin::Harness;
    if app.mode == Mode::Solver {
        return Some(crate::solver::LABEL.into());
    }
    if app.mode != Mode::Live {
        return None;
    }
    let keyed = app.plugins.enabled && app.plugins.key_configured;
    // Before the chat's first turn nothing is routed and nothing is
    // named: the rail never reads as a mode to pick (owner, 2026-10-12).
    let harness = app.live.runs_on?;
    match harness {
        Harness::ClaudeCode => Some(claude_rail_label()),
        Harness::CoderLoop => {
            let model = if keyed {
                crate::models::label(&app.plugins.model)
            } else {
                "auto".into()
            };
            // On auto, the model the last turn ran, as the transcript
            // knows it (#11519).
            let model = if model == crate::models::AUTO_LABEL {
                app.live
                    .entries
                    .iter()
                    .rev()
                    .find_map(|entry| match entry {
                        crate::live::Entry::Assistant {
                            model: Some(model), ..
                        } if !model.is_empty() && crate::models::pinned(model) => {
                            Some(rail_model_name(model))
                        }
                        _ => None,
                    })
                    .unwrap_or_else(|| "auto".into())
            } else {
                model
            };
            Some(format!("{CODER_RAIL} ({model})"))
        }
        // "Codex (GPT-6.1-Sol)": the model the next turn runs (#11510).
        Harness::Codex => Some(format!(
            "Codex ({})",
            crate::codex::name(&crate::codex::turn_model(app.live.pinned_model.clone()))
        )),
        // The model the chat's OpenCode session runs, named (#11507).
        Harness::OpenCode => Some(opencode_rail_label(app)),
    }
}

/// "OpenCode (opencode/big-pickle)": the model the chat's OpenCode session
/// last ran, else the one its next turn runs.
pub(crate) fn opencode_rail_label(app: &App) -> String {
    let model = app.live.opencode_session.as_ref().map_or_else(
        || crate::opencode_chat::model(app.live.pinned_model.clone()),
        |session| session.model.clone(),
    );
    crate::opencode_chat::rail_label(&model)
}

/// The rail's word for Coder's own loop, which the rail colors as its own.
const CODER_RAIL: &str = "Coder";

/// The rail's word for Claude Code, which the rail colors as its own.
const CLAUDE_RAIL: &str = "Claude";

/// "Claude" and, in parentheses, the model Claude Code runs, as /models
/// names it.
pub(crate) fn claude_rail_label() -> String {
    let model = crate::claude::choice().model;
    let name = crate::claude::MODELS
        .iter()
        .find(|(id, _, _)| *id == model)
        .map_or(model.as_str(), |(_, name, _)| *name);
    format!("{CLAUDE_RAIL} ({name})")
}

/// The spinner and `Working` the input's rail shows while the chat above it
/// is answering, or `None` when it is idle.
///
/// The label rides the input's top-left rail, where the mode's own name is
/// on the right, so it stays put instead of scrolling away with the
/// transcript. It follows the same chat the composer does: the selected
/// delegation's, else the main one.
fn working_label(app: &App) -> Option<String> {
    let chat = app
        .selected_agent
        .and_then(|index| app.delegations.get(index))
        .map_or(&app.live, |agent| &agent.chat);
    // A message waiting for its route is already being worked on.
    let routing = app.selected_agent.is_none() && app.routing_pending();
    if !chat.busy && !routing {
        return None;
    }
    let mut label = format!("{} Working", crate::tools::spinner(app.animation_frame));
    // How long the turn has run and what it has written, as Claude Code
    // shows them: "(34m 8s · ↓ 45.6k tokens)".
    if let Some(started) = chat.reply_started_at {
        let mut facts = vec![agent_fleet::elapsed_words(started.elapsed().as_secs())];
        let tokens = turn_output_tokens(chat);
        if tokens > 0 {
            facts.push(format!("↓ {} tokens", agent_fleet::token_words(tokens)));
        }
        label.push_str(&format!(" ({})", facts.join(" · ")));
    }
    // While the model's thinking streams in, the rail says so to the right.
    if chat.thinking {
        label.push_str(" · Thinking");
    }
    Some(label)
}

/// The tokens the running turn has written: the output its model calls
/// reported, or, before any call reports, its streamed words at about four
/// characters a token.
fn turn_output_tokens(chat: &crate::live::Chat) -> u64 {
    let start = chat
        .entries
        .iter()
        .rposition(|entry| matches!(entry, crate::live::Entry::User(_)))
        .map_or(0, |index| index + 1);
    let reported: u64 = chat
        .rounds
        .iter()
        .filter(|(index, _)| *index >= start)
        .map(|(_, usage)| usage.completion_tokens)
        .sum();
    if reported > 0 {
        return reported;
    }
    let written: usize = chat.entries[start.min(chat.entries.len())..]
        .iter()
        .map(|entry| match entry {
            crate::live::Entry::Assistant { text, .. } => text.len(),
            crate::live::Entry::Tool { input, output, .. } => {
                input.to_string().len() + output["text"].as_str().map_or(0, str::len)
            }
            _ => 0,
        })
        .sum::<usize>()
        + chat.partial.len();
    (written / 4) as u64
}

fn context_text(app: &App) -> String {
    let mut parts = Vec::new();
    if let Some(until) = crate::long_session::paused_until(app) {
        parts.push(format!(
            "Paused until {}",
            crate::long_session::clock(until)
        ));
    }
    // Claude's plan windows are Claude's, not Coder's: they ride the top
    // rail beside Claude's name while Claude runs the chat, and never sit
    // here next to the path ([`top_rail`], #11483).
    // How full the claude session's context is rides the top rail ([`top_rail`]).
    // The signed-in account's name is never shown under the input.
    if let Some(update) = &app.update_line {
        parts.push(update.clone());
    }
    parts.join(" · ")
}

fn message_body(text: &str, width: u16) -> Vec<Line<'static>> {
    t::noir_lines(markdown_body(text, width, Ladder::new(Colors::True)))
}

/// [`message_body`] with what a copy takes from each row.
fn message_rows(text: &str, width: u16) -> (Vec<Line<'static>>, Vec<RowCopy>) {
    let (lines, copy) = markdown_rows(text, width, Ladder::new(Colors::True));
    (t::noir_lines(lines), copy)
}

/// The columns the transcript keeps clear at its right edge. Its rows are
/// laid out `GUTTER` narrower than the screen; only the person's band
/// reaches the edge.
const GUTTER: u16 = 2;

/// The person's message as Claude Code shows it: `❯` in column 0, wrapped
/// rows hanging under the text, on a band that runs through the gutter to
/// the screen's edge.
fn prompt(text: &str, width: u16) -> Vec<Line<'static>> {
    prompt_rows(text, width).0
}

/// [`prompt`] with what a copy takes from each row: the text, not the
/// `❯` or the band's padding.
/// Rows a message the person sent shows in the compact view; the rest
/// waits behind ` …` until Ctrl+O, as Grok Build collapses a sent prompt.
const PROMPT_COLLAPSED_ROWS: usize = 3;

fn prompt_rows(text: &str, width: u16) -> (Vec<Line<'static>>, Vec<RowCopy>) {
    prompt_rows_limited(text, width, None)
}

/// [`prompt_rows`] with attachment notes drawn as `[Image #1]` and, given
/// `max_rows`, the message cut to that many rows ending in ` …`.
fn prompt_rows_limited(
    text: &str,
    width: u16,
    max_rows: Option<usize>,
) -> (Vec<Line<'static>>, Vec<RowCopy>) {
    let text = crate::attachments::shown(text);
    let (mut rows, copy) = message_rows(&text, width.saturating_sub(2));
    let mut copy: Vec<RowCopy> = copy.into_iter().map(|row| row.shifted(2)).collect();
    if let Some(max) = max_rows
        && rows.len() > max
    {
        // Wrap two cells narrower so the ` …` fits on the last row kept.
        let (narrow, narrow_copy) = message_rows(&text, width.saturating_sub(4));
        rows = narrow;
        copy = narrow_copy.into_iter().map(|row| row.shifted(2)).collect();
        rows.truncate(max);
        copy.truncate(max);
        if let Some(last) = rows.last_mut() {
            last.spans.push(span(" …", t::TEXT_SECONDARY));
        }
    }
    if rows.is_empty() {
        rows.push(Line::default());
        copy.push(RowCopy::lead(2));
    }
    for (index, row) in rows.iter_mut().enumerate() {
        row.spans.insert(
            0,
            span(if index == 0 { "❯ " } else { "  " }, t::TEXT_SECONDARY),
        );
        let band = usize::from(width.saturating_add(GUTTER));
        let used = row.width();
        if used < band {
            row.spans.push(Span::raw(" ".repeat(band - used)));
        }
        row.style = row.style.bg(t::USER_MESSAGE_BG);
    }
    (rows, copy)
}

/// An assistant reply as Claude Code shows it: `●` in column 0, then the
/// text, every further row indented two columns to hang under it.
fn bulleted(text: &str, width: u16) -> Vec<Line<'static>> {
    bulleted_rows(text, width).0
}

/// [`bulleted`] with what a copy takes from each row: the text, not the
/// `●` or the hanging indent.
fn bulleted_rows(text: &str, width: u16) -> (Vec<Line<'static>>, Vec<RowCopy>) {
    let (mut rows, copy) = message_rows(text, width.saturating_sub(2));
    let mut first = true;
    for row in &mut rows {
        let blank = row.spans.iter().all(|span| span.content.trim().is_empty());
        let lead = if first && !blank {
            first = false;
            span("● ", t::TEXT_PRIMARY)
        } else {
            Span::raw("  ")
        };
        row.spans.insert(0, lead);
    }
    (rows, copy.into_iter().map(|row| row.shifted(2)).collect())
}

fn wrap_display(lines: Vec<Line<'static>>, width: u16) -> Vec<Line<'static>> {
    let mut rows = Vec::new();
    for line in lines {
        for range in coder_terminal::wrap_rows(&line.to_string(), usize::from(width).max(1)) {
            let mut offset = 0;
            let spans = line
                .spans
                .iter()
                .filter_map(|span| {
                    let end = offset + span.content.len();
                    let start = range.start.max(offset);
                    let stop = range.end.min(end);
                    let piece = (start < stop).then(|| {
                        Span::styled(
                            span.content[start - offset..stop - offset].to_owned(),
                            span.style,
                        )
                    });
                    offset = end;
                    piece
                })
                .collect::<Vec<_>>();
            rows.push(Line {
                spans,
                style: line.style,
                alignment: line.alignment,
            });
        }
    }
    rows
}

fn conversation(frame: &mut Frame, area: Rect, app: &mut App) {
    let content = area.width.saturating_sub(GUTTER);
    // claude's conversation is drawn like a live chat's.
    if matches!(app.mode, Mode::Live | Mode::Solver) {
        live_conversation(frame, area, app);
        return;
    }
    let mut lines = if app.mode == Mode::Live {
        live_lines(app, content)
    } else if let Some(agent) = app.selected_agent.and_then(|index| DEMOS.get(index)) {
        let mut lines = Vec::new();
        for message in agent.conversation.iter() {
            match message {
                DemoMessage::User(text) => lines.extend(prompt(text, content)),
                DemoMessage::Tool(call) => {
                    lines.extend(wrap_display(
                        tool_lines(call, app.animation_frame, content),
                        content,
                    ));
                }
                DemoMessage::Plugin(call) => lines.extend(wrap_display(
                    plugin_lines(call, app.animation_frame),
                    content,
                )),
                DemoMessage::Assistant(text) => {
                    lines.extend(bulleted(text, content));
                }
            }
            lines.push(Line::default());
        }
        lines
    } else {
        let mut lines = prompt("Review the terminal with four agents.", content);
        lines.push(Line::default());
        for call in &MAIN_TOOLS {
            lines.extend(wrap_display(
                tool_lines(call, app.animation_frame, content),
                content,
            ));
            lines.push(Line::default());
        }
        for call in &MAIN_PLUGINS {
            lines.extend(wrap_display(
                plugin_lines(call, app.animation_frame),
                content,
            ));
            lines.push(Line::default());
        }
        for agent in &DEMOS {
            lines.extend(delegation_lines(agent, app.animation_frame, content));
            lines.push(Line::default());
        }
        lines
    };
    for message in app.messages.iter().filter(|_| app.mode == Mode::Demo) {
        lines.extend(prompt(message, content));
        lines.push(Line::default());
        lines.extend(wrap_display(
            vec![Line::from(span(
                "Preview message added. No agent is connected.",
                t::GRAY,
            ))],
            content,
        ));
        lines.push(Line::default());
    }
    if let Some(notice) = &app.notice {
        lines.extend(wrap_display(
            notice
                .lines()
                .map(|text| Line::from(span(text, t::GRAY)))
                .collect(),
            content,
        ));
        lines.push(Line::default());
    }
    let paragraph = Paragraph::new(Text::from(lines));
    let max_scroll = paragraph
        .line_count(area.width)
        .saturating_sub(usize::from(area.height))
        .min(usize::from(u16::MAX)) as u16;
    app.scroll = app.scroll.min(max_scroll);
    frame.render_widget(paragraph.scroll((app.scroll, 0)), area);
}

fn run_lines(
    input: &serde_json::Value,
    output: &serde_json::Value,
    running: bool,
    width: u16,
    phase: u8,
) -> Vec<Line<'static>> {
    run_rows(input, output, running, width, phase).0
}

/// [`run_lines`] with what a copy takes from each row: the command's
/// output without the `⎿` or the indent, wrapped rows joined back.
fn run_rows(
    input: &serde_json::Value,
    output: &serde_json::Value,
    running: bool,
    width: u16,
    phase: u8,
) -> (Vec<Line<'static>>, Vec<RowCopy>) {
    let result_lead = RESULT_INDENT.len() as u16;
    let command = input
        .get("command")
        .and_then(|v| v.as_str())
        .or_else(|| output.get("command").and_then(|v| v.as_str()))
        .or_else(|| input.as_str())
        .unwrap_or_default();
    let (glyph, color) =
        crate::tools::outcome_header(running, crate::tools::output_failed(output), phase);
    let mut lines = vec![Line::from(vec![
        span(format!("{glyph} "), color),
        Span::styled(
            "Run",
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        span(
            format!(
                " {}",
                truncate(&command.replace(['\n', '\r'], " "), width.saturating_sub(7))
            ),
            t::TEXT_SECONDARY,
        ),
    ])];
    let mut copy = vec![RowCopy::lead(2)];
    if let Some(fields) = output.as_object() {
        let status = fields
            .iter()
            .filter(|(key, _)| {
                !matches!(
                    key.as_str(),
                    "command" | "output" | "stdout" | "stderr" | "error" | "activity"
                )
            })
            .map(|(key, value)| {
                if matches!(key.as_str(), "elapsed_seconds" | "silent_seconds") {
                    let label = if key == "elapsed_seconds" {
                        "elapsed"
                    } else {
                        "silent"
                    };
                    return format!("{label}: {value}s");
                }
                format!(
                    "{key}: {}",
                    value
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| value.to_string())
                )
            })
            .collect::<Vec<_>>()
            .join(" · ");
        if !status.is_empty() {
            lines.push(Line::from(span(
                format!(
                    "{RESULT_INDENT}{}",
                    truncate(&status, width.saturating_sub(5))
                ),
                t::TEXT_SECONDARY,
            )));
            copy.push(RowCopy::lead(result_lead));
        }
    }
    for key in ["output", "stdout", "stderr", "error"] {
        if let Some(text) = output
            .get(key)
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            let inner = usize::from(width.saturating_sub(6)).max(1);
            for row in text.lines() {
                let ranges = coder_terminal::wrap_rows(row, inner);
                if ranges.is_empty() {
                    lines.push(Line::from(span(RESULT_INDENT, t::TEXT_SECONDARY)));
                    copy.push(RowCopy::lead(result_lead));
                    continue;
                }
                let mut previous_end: Option<usize> = None;
                for range in ranges {
                    copy.push(RowCopy {
                        joiner: previous_end.map(|end| row[end..range.start].to_owned()),
                        ..RowCopy::lead(result_lead)
                    });
                    previous_end = Some(range.end);
                    // Keep every output row for plain-text exports. The
                    // screen shows a five-row window over these rows.
                    lines.push(Line::from(span(
                        format!("{RESULT_INDENT}{}", &row[range]),
                        t::TEXT_SECONDARY,
                    )));
                }
            }
        }
    }
    mark_result(&mut lines);
    (lines, copy)
}

/// The indent result rows sit at, under a tool's name (Claude Code's
/// hanging indent).
const RESULT_INDENT: &str = "     ";
/// The mark on a tool's first result row.
const RESULT_MARK: &str = "  ⎿  ";

/// Put [`RESULT_MARK`] on the first row after a tool's header that starts
/// with [`RESULT_INDENT`].
fn mark_result(lines: &mut [Line<'static>]) {
    let Some(row) = lines.get_mut(1) else {
        return;
    };
    let Some(first) = row.spans.first_mut() else {
        return;
    };
    if let Some(rest) = first.content.strip_prefix(RESULT_INDENT) {
        let rest = rest.to_owned();
        let style = first.style;
        row.spans.splice(
            0..1,
            [span(RESULT_MARK, t::GRAY_DIM), Span::styled(rest, style)],
        );
    }
}

/// Whether a tool's rows are a card of their own rather than a call with
/// parameters: a compaction summary, a usage-limit pause, a plan, or a
/// decision or summary card. The compact view leaves these alone; they say
/// something the person needs, not what a call passed.
fn is_card(name: &str) -> bool {
    matches!(
        name,
        crate::long_session::COMPACT_TOOL
            | crate::long_session::LIMIT_TOOL
            | crate::issue_run::DECISION
            | crate::issue_run::SUMMARY
            | crate::plan::TOOL
            | crate::claude_stream::THINKING
            | crate::claude_stream::NOTICE
    )
}

/// Rows a copy has no layout for take [`crate::tools::plain_copy`].
fn fill_copy(lines: &[Line<'static>], copy: &mut Vec<RowCopy>) {
    let known = copy.len();
    copy.extend(lines.iter().skip(known).map(crate::tools::plain_copy));
}

/// Whether an entry is a reply, drawn with `●` in column 0.
/// Whether `entry` keeps one blank row above it even under a compact tool
/// row: a reply, the person's own message, and the model's thinking.
fn is_reply(entry: &crate::live::Entry) -> bool {
    match entry {
        crate::live::Entry::Assistant { .. } | crate::live::Entry::User(_) => true,
        crate::live::Entry::Tool { name, .. } => name == crate::claude_stream::THINKING,
        _ => false,
    }
}

/// Whether a row draws nothing.
fn is_blank_row(line: &Line<'static>) -> bool {
    line.spans.iter().all(|span| span.content.trim().is_empty())
}

/// An entry's rows and, row for row, what a copy takes from each.
///
/// In [`ViewMode::Compact`] a tool call is one row — its name and its
/// parameters — and no blank row follows it, so calls run together. A
/// finished Edit or Write keeps its diff.
fn entry_rows(
    entry: &crate::live::Entry,
    width: u16,
    phase: u8,
    view: ViewMode,
) -> (Vec<Line<'static>>, Vec<RowCopy>) {
    let mut lines = Vec::new();
    let mut copy = Vec::new();
    let exact = |lines: &mut Vec<Line<'static>>,
                 copy: &mut Vec<RowCopy>,
                 (rows, rows_copy): (Vec<Line<'static>>, Vec<RowCopy>)| {
        fill_copy(lines, copy);
        lines.extend(rows);
        copy.extend(rows_copy);
    };
    if let crate::live::Entry::Tool {
        name,
        input,
        output,
        running,
    } = entry
        && view == ViewMode::Compact
        && !is_card(name)
        // A finished edit always shows its diff, in either view.
        && !(matches!(name.as_str(), "Edit" | "Write") && !*running)
    {
        let (glyph, color) =
            crate::tools::outcome_header(*running, crate::tools::output_failed(output), phase);
        let mut spans = vec![
            span(format!("{glyph} "), color),
            Span::styled(
                name.clone(),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
        ];
        // The glyph, its space, the name's space, and the separator.
        let room = width.saturating_sub(name.width() as u16 + 3);
        // A failed call says why in words, not the fields it was sent
        // (#11482); Ctrl+O still shows them.
        let params = match output.get("error").and_then(serde_json::Value::as_str) {
            Some(error) => truncate(error.lines().next().unwrap_or_default(), room),
            None => crate::tools::compact_params(input, room),
        };
        if !params.is_empty() {
            spans.push(span(" ", t::GRAY_DIM));
            spans.push(span(params, t::TEXT_SECONDARY));
        }
        lines.push(Line::from(spans));
        fill_copy(&lines, &mut copy);
        return (lines, copy);
    }
    match entry {
        crate::live::Entry::User(text) => {
            let max_rows = (view == ViewMode::Compact).then_some(PROMPT_COLLAPSED_ROWS);
            exact(
                &mut lines,
                &mut copy,
                prompt_rows_limited(text, width, max_rows),
            );
        }
        // The reply's model and time stay in the entry (exports read them);
        // the transcript shows neither.
        crate::live::Entry::Assistant { text, .. } => {
            exact(&mut lines, &mut copy, bulleted_rows(&shown(text), width));
        }
        crate::live::Entry::Tool {
            name,
            input,
            output,
            running,
        } => {
            // A compaction summary or a usage-limit pause (#11179).
            if let Some(rows) =
                crate::long_session::entry_lines(name, input, output, *running, width, phase)
            {
                lines.extend(rows);
                lines.push(Line::default());
                fill_copy(&lines, &mut copy);
                return (lines, copy);
            }
            // The model's thinking, dim and slanted, with no heading.
            if name == crate::claude_stream::THINKING {
                let (mut rows, rows_copy) = message_rows(
                    crate::claude_stream::entry_text(output),
                    width.saturating_sub(2),
                );
                for row in &mut rows {
                    for part in &mut row.spans {
                        part.style = part.style.fg(t::GRAY).add_modifier(Modifier::ITALIC);
                    }
                    row.spans.insert(0, Span::raw("  "));
                }
                exact(
                    &mut lines,
                    &mut copy,
                    (
                        rows,
                        rows_copy.into_iter().map(|row| row.shifted(2)).collect(),
                    ),
                );
                lines.push(Line::default());
                fill_copy(&lines, &mut copy);
                return (lines, copy);
            }
            // A retry or something that failed to load: one quiet line.
            if name == crate::claude_stream::NOTICE {
                lines.push(Line::from(span(
                    truncate(crate::claude_stream::entry_text(output), width),
                    t::GRAY,
                )));
                lines.push(Line::default());
                fill_copy(&lines, &mut copy);
                return (lines, copy);
            }
            if name == crate::issue_run::DECISION {
                lines.extend(crate::issue_run::cards::decision_lines(
                    input, output, *running, width, phase,
                ));
                lines.push(Line::default());
                fill_copy(&lines, &mut copy);
                return (lines, copy);
            }
            if name == crate::issue_run::SUMMARY {
                lines.extend(crate::issue_run::cards::summary_lines(input, output, width));
                lines.push(Line::default());
                fill_copy(&lines, &mut copy);
                return (lines, copy);
            }
            if name == crate::plan::TOOL
                && let Some(plan) = crate::plan::text(output)
            {
                lines.push(Line::from(span("Plan", t::TEXT_PRIMARY)));
                exact(&mut lines, &mut copy, message_rows(&plan, width));
                lines.push(Line::default());
                fill_copy(&lines, &mut copy);
                return (lines, copy);
            }
            if name == "Run" {
                exact(
                    &mut lines,
                    &mut copy,
                    run_rows(input, output, *running, width, phase),
                );
                lines.push(Line::default());
                fill_copy(&lines, &mut copy);
                return (lines, copy);
            }
            if crate::file_tools::is_tool(name) {
                exact(
                    &mut lines,
                    &mut copy,
                    crate::tools::file_tool_rows(name, input, output, *running, width, phase),
                );
                lines.push(Line::default());
                fill_copy(&lines, &mut copy);
                return (lines, copy);
            }
            let (glyph, color) =
                crate::tools::outcome_header(*running, crate::tools::output_failed(output), phase);
            let native = matches!(name.as_str(), "Run" | "Read" | "Edit" | "Search");
            let label = if native { name.as_str() } else { "Plugin" };
            lines.push(Line::from(vec![
                span(format!("{glyph} "), color),
                Span::styled(
                    label.to_owned(),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                span(
                    truncate(
                        &if native {
                            String::new()
                        } else {
                            format!(" {name}")
                        },
                        width.saturating_sub(9),
                    ),
                    t::TEXT_PRIMARY,
                ),
            ]));
            lines.extend(crate::tools::parameter_lines(input, width));
            if *running {
                lines.push(Line::from(vec![
                    span("  ⎿  ", t::GRAY_DIM),
                    span("Running", color),
                ]));
            } else {
                if crate::brainstorm::is_tool(name) {
                    lines.extend(message_body(&crate::brainstorm::summary(output), width));
                } else {
                    lines.extend(crate::tools::parameter_lines(output, width));
                }
            }
        }
        crate::live::Entry::Delegation {
            name,
            task,
            running,
            output,
            progress,
            ..
        } => {
            let (glyph, color) =
                crate::tools::outcome_header(*running, crate::tools::output_failed(output), phase);
            lines.push(Line::from(vec![
                span(format!("{glyph} "), color),
                Span::styled(
                    "Delegate",
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                span(format!(" {name}"), t::TEXT_PRIMARY),
            ]));
            let status = match progress {
                // Jev's estimate while a Microcoder run reports its steps.
                Some((step, complete)) if *running => {
                    crate::bundled_runtime::RuntimeEvent::progress_line(*step, *complete)
                }
                _ if *running => "Running".into(),
                // A Claude plan limit stopped it (#11295); a background
                // agent's choices are in /agents.
                _ if output
                    .get("error")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(coder_engine_status::limit::says_limited) =>
                {
                    "Paused by the Claude plan limit".into()
                }
                // A finished agent's output carries `"error": null`.
                _ if output.get("error").is_some_and(|error| !error.is_null()) => "Failed".into(),
                _ => "Done".into(),
            };
            let detail =
                format!(
                    "{} · {status}",
                    truncate(
                        task,
                        width.saturating_sub(10_u16.saturating_add(
                            u16::try_from(status.chars().count()).unwrap_or(u16::MAX)
                        ))
                    )
                );
            lines.push(Line::from(vec![
                span("  ⎿  ", t::GRAY_DIM),
                span(detail, t::TEXT_SECONDARY),
            ]));
        }
    }
    lines.push(Line::default());
    fill_copy(&lines, &mut copy);
    (lines, copy)
}

#[derive(Default)]
pub struct TranscriptCache {
    width: u16,
    /// The view the cached rows were laid out in; a switch rebuilds them.
    mode: ViewMode,
    entries: Vec<CachedEntry>,
    partial: Option<CachedEntry>,
    pub builds: usize,
    run_regions: Vec<(usize, Rect)>,
    /// The rows drawn under the entries (notices).
    tail: Vec<Line<'static>>,
    /// Where the transcript was last drawn and the row shown at its top.
    view: Option<(Rect, usize)>,
    /// Selected transcript text, drawn highlighted (see
    /// `coder_terminal::selection`).
    pub(crate) selection: Option<selection::Selection>,
    /// A mouse drag selecting text: where it started and where the
    /// pointer is now.
    pub(crate) drag: Option<Drag>,
}

/// A drag across the transcript: the cell it started on and the pointer's
/// last screen cell. The selection's moving end follows the pointer on
/// every draw, so it stays under it while the view scrolls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Drag {
    pub anchor: selection::Point,
    pub mouse: (u16, u16),
}

struct CachedEntry {
    width: u16,
    entry: crate::live::Entry,
    lines: Vec<Line<'static>>,
    /// What a copy takes from each of `lines`.
    copy: Vec<RowCopy>,
    /// Whether a blank row was appended so a reply under this entry starts
    /// with the one blank row every reply has above it.
    gap_after: bool,
    run_output: Option<(usize, Vec<Line<'static>>, usize, bool)>,
    /// What a copy takes from each of a Run's output rows.
    run_copy: Vec<RowCopy>,
}

/// Columns at a Run box's left edge where the wheel scrolls the box.
const RUN_GRAB: u16 = 8;
const RUN_OUTPUT_HEIGHT: usize = 5;

impl CachedEntry {
    fn bound_run(&mut self, width: u16, phase: u8, previous: Option<(usize, bool)>) {
        self.run_output = None;
        let crate::live::Entry::Tool {
            name,
            input,
            output,
            running,
        } = &self.entry
        else {
            return;
        };
        if name != "Run" {
            return;
        }
        let mut metadata = output.clone();
        if let Some(fields) = metadata.as_object_mut() {
            for key in ["output", "stdout", "stderr", "error"] {
                fields.remove(key);
            }
        }
        let start = run_lines(input, &metadata, *running, width, phase).len();
        let end = self.lines.len().saturating_sub(1);
        if end <= start {
            return;
        }
        let rows = self.lines[start..end].to_vec();
        self.run_copy = self
            .copy
            .get(start..end)
            .map(<[_]>::to_vec)
            .unwrap_or_default();
        let max = rows.len().saturating_sub(RUN_OUTPUT_HEIGHT);
        let (offset, following) = previous.unwrap_or((max, true));
        let offset = if following { max } else { offset.min(max) };
        self.run_output = Some((start, rows, offset, following));
        self.show_run();
    }

    fn show_run(&mut self) {
        let Some((start, rows, offset, _)) = &self.run_output else {
            return;
        };
        self.lines.truncate(*start);
        self.copy.truncate(*start);
        let height = rows.len().min(RUN_OUTPUT_HEIGHT);
        let max = rows.len().saturating_sub(RUN_OUTPUT_HEIGHT);
        let thumb = if max == 0 {
            0
        } else {
            offset * height.saturating_sub(1) / max
        };
        for (index, mut line) in rows
            .iter()
            .skip(*offset)
            .take(RUN_OUTPUT_HEIGHT)
            .cloned()
            .enumerate()
        {
            // The scrollbar is the box's left edge, where the wheel scrolls
            // the box (`RUN_GRAB`); elsewhere the wheel scrolls the chat.
            let width = usize::from(self.width);
            line.spans.insert(
                0,
                span(
                    if max == 0 || index == thumb {
                        "█ "
                    } else {
                        "│ "
                    },
                    t::GRAY,
                ),
            );
            line.spans
                .push(Span::raw(" ".repeat(width.saturating_sub(line.width()))));
            line.style = line.style.bg(t::BG_LIGHT);
            self.lines.push(line);
            self.copy.push(
                self.run_copy
                    .get(offset + index)
                    .cloned()
                    .unwrap_or_default()
                    .shifted(2),
            );
        }
        if rows.len() > RUN_OUTPUT_HEIGHT {
            self.lines.push(Line::from(span(
                format!(
                    "     {}–{} of {}",
                    offset + 1,
                    (offset + RUN_OUTPUT_HEIGHT).min(rows.len()),
                    rows.len()
                ),
                t::GRAY_DIM,
            )));
            self.copy.push(RowCopy {
                omit: true,
                ..RowCopy::default()
            });
        }
        self.lines.push(Line::default());
        self.copy.push(RowCopy::default());
    }
}

impl TranscriptCache {
    pub(crate) fn scroll_run(&mut self, column: u16, row: u16, up: bool) -> bool {
        let Some((index, _)) = self
            .run_regions
            .iter()
            .find(|(_, area)| area.contains((column, row).into()))
        else {
            return false;
        };
        let cached = &mut self.entries[*index];
        let Some((_, rows, offset, following)) = &mut cached.run_output else {
            return false;
        };
        let max = rows.len().saturating_sub(RUN_OUTPUT_HEIGHT);
        *following = false;
        *offset = if up {
            offset.saturating_sub(3)
        } else {
            offset.saturating_add(3).min(max)
        };
        cached.show_run();
        true
    }

    fn locate_runs(&mut self, area: Rect, position: usize) {
        self.run_regions.clear();
        let mut top = 0;
        for (index, cached) in self.entries.iter().enumerate() {
            if let Some((start, rows, _, _)) = &cached.run_output {
                let first = top + start;
                let end = first + rows.len().min(RUN_OUTPUT_HEIGHT);
                let visible_start = first.max(position);
                let visible_end = end.min(position + usize::from(area.height));
                if visible_start < visible_end {
                    self.run_regions.push((
                        index,
                        Rect {
                            y: area.y + (visible_start - position) as u16,
                            height: (visible_end - visible_start) as u16,
                            width: area.width.min(RUN_GRAB),
                            ..area
                        },
                    ));
                }
            }
            top += cached.lines.len();
        }
    }

    fn refresh(&mut self, chat: &crate::live::Chat, width: u16, phase: u8, view: ViewMode) {
        let resized = self.width != width;
        // A view switch re-lays every entry, like a resize.
        let changed = resized || self.mode != view;
        self.width = width;
        self.mode = view;
        if resized {
            // Rows rewrap at a new width; a selection would land on other text.
            self.selection = None;
            self.drag = None;
        }
        self.entries.truncate(chat.entries.len());
        for (index, entry) in chat.entries.iter().enumerate() {
            if self
                .entries
                .get(index)
                .is_none_or(|cached| changed || cached.entry != *entry)
            {
                let previous_offset = self
                    .entries
                    .get(index)
                    .and_then(|c| c.run_output.as_ref())
                    .map(|r| (r.2, r.3));
                let (lines, copy) = entry_rows(entry, width, phase, view);
                let mut cached = CachedEntry {
                    width,
                    entry: entry.clone(),
                    lines,
                    copy,
                    gap_after: false,
                    run_output: None,
                    run_copy: Vec::new(),
                };
                cached.bound_run(width, phase, previous_offset);
                if index < self.entries.len() {
                    self.entries[index] = cached;
                } else {
                    self.entries.push(cached);
                }
                self.builds += 1;
            } else if matches!(
                entry,
                crate::live::Entry::Tool { running: true, .. }
                    | crate::live::Entry::Delegation { running: true, .. }
            ) {
                let cached = &mut self.entries[index];
                let offset = cached.run_output.as_ref().map(|r| (r.2, r.3));
                (cached.lines, cached.copy) = entry_rows(entry, width, phase, view);
                // Rebuilt rows carry no gap the loop below pushed.
                cached.gap_after = false;
                cached.bound_run(width, phase, offset);
            }
        }
        // A compact tool row carries no trailing blank so calls run together;
        // an item under it that is a reply takes the one blank row every
        // item has above it. A row that already ends in a blank (a verbose
        // item) takes none, so a reply never shows two blanks above it.
        // `gap_after` means the entry's last row is a gap this loop pushed:
        // `ends_blank` looks past it, or the gap reads as the entry's own
        // blank and is pushed and popped on alternating draws (#11395).
        let streaming = !chat.partial.is_empty();
        for index in 0..self.entries.len() {
            let follows_reply = match self.entries.get(index + 1) {
                Some(next) => is_reply(&next.entry),
                None => streaming,
            };
            let cached = &mut self.entries[index];
            let ends_blank = cached
                .lines
                .iter()
                .nth_back(usize::from(cached.gap_after))
                .is_some_and(is_blank_row);
            let needed = follows_reply && !ends_blank;
            if needed && !cached.gap_after {
                cached.lines.push(Line::default());
                cached.copy.push(RowCopy::default());
                cached.gap_after = true;
            } else if !needed && cached.gap_after {
                cached.lines.pop();
                cached.copy.pop();
                cached.gap_after = false;
            }
        }
        if chat.partial.is_empty() {
            self.partial = None;
        } else {
            let entry = crate::live::Entry::Assistant {
                elapsed_ms: None,
                // Only the part that renders cleanly so far: no half-written
                // fence, table row, link, emphasis, or component statement
                // shows raw (#11112, #11187).
                text: markdown_stream::renderable(&shown(&chat.partial)).into_owned(),
                model: chat.partial_model.clone(),
            };
            if self
                .partial
                .as_ref()
                .is_none_or(|cached| changed || cached.entry != entry)
            {
                let (lines, copy) = entry_rows(&entry, width, phase, view);
                self.partial = Some(CachedEntry {
                    width,
                    lines,
                    copy,
                    entry,
                    gap_after: false,
                    run_output: None,
                    run_copy: Vec::new(),
                });
                self.builds += 1;
            }
        }
    }

    fn blocks(&self) -> impl Iterator<Item = &CachedEntry> {
        self.entries.iter().chain(self.partial.iter())
    }

    pub(crate) fn count(&self) -> usize {
        self.blocks().map(|cached| cached.lines.len()).sum()
    }

    /// A transcript row's drawn text and what a copy takes from it.
    fn row(&self, mut index: usize) -> Option<(String, &RowCopy)> {
        static PLAIN: RowCopy = RowCopy {
            lead: 0,
            skip: Vec::new(),
            joiner: None,
            omit: false,
        };
        for cached in self.blocks() {
            if index < cached.lines.len() {
                return Some((
                    crate::tools::line_text(&cached.lines[index]),
                    cached.copy.get(index).unwrap_or(&PLAIN),
                ));
            }
            index -= cached.lines.len();
        }
        self.tail
            .get(index)
            .map(|line| (crate::tools::line_text(line), &PLAIN))
    }

    fn rows_total(&self) -> usize {
        self.count() + self.tail.len()
    }

    /// The transcript cell under a screen cell, as last drawn. `clamp`
    /// takes a pointer outside the transcript (a drag past its edge) to
    /// the nearest cell; otherwise such a pointer has none.
    pub(crate) fn point_at(&self, column: u16, row: u16, clamp: bool) -> Option<selection::Point> {
        let (area, first) = self.view?;
        let total = self.rows_total();
        if area.width == 0 || area.height == 0 || total == 0 {
            return None;
        }
        if !clamp && !area.contains((column, row).into()) {
            return None;
        }
        let column = column.clamp(area.x, area.right() - 1);
        let row = row.clamp(area.y, area.bottom() - 1);
        let index = first + usize::from(row - area.y);
        if !clamp && index >= total {
            return None;
        }
        Some(selection::Point {
            row: index.min(total - 1),
            col: column - area.x,
        })
    }

    /// The selected text as a copy takes it.
    pub(crate) fn selected_text(&self) -> Option<String> {
        let selection = self.selection?;
        let text = selection::copy_text(&selection, |index| self.row(index));
        (!text.is_empty()).then_some(text)
    }

    /// Select the word under `at` (a double-click).
    pub(crate) fn select_word(&mut self, at: selection::Point) {
        let span = self
            .row(at.row)
            .and_then(|(text, copy)| selection::word_at(&text, copy, at.col));
        self.selection = span.map(|(start, end)| selection::Selection {
            anchor: selection::Point {
                row: at.row,
                col: start,
            },
            head: selection::Point {
                row: at.row,
                col: end,
            },
        });
    }

    /// Select the whole line through `at`, its soft wraps included (a
    /// triple-click).
    pub(crate) fn select_line(&mut self, at: selection::Point) {
        let (first, last) = selection::line_rows(at.row, self.rows_total(), |index| {
            self.row(index)
                .is_some_and(|(_, copy)| copy.joiner.is_some())
        });
        self.selection = Some(selection::Selection {
            anchor: selection::Point { row: first, col: 0 },
            head: selection::Point {
                row: last,
                col: u16::MAX - 1,
            },
        });
    }

    /// How far a drag held past the transcript's edge scrolls now.
    pub(crate) fn drag_scroll(&self) -> i16 {
        match (self.drag, self.view) {
            (Some(drag), Some((area, _))) => selection::edge_scroll(drag.mouse.1, area),
            _ => 0,
        }
    }

    /// The selection's moving end follows the drag's pointer.
    pub(crate) fn follow_drag(&mut self) {
        let Some(drag) = self.drag else {
            return;
        };
        if let Some(head) = self.point_at(drag.mouse.0, drag.mouse.1, true)
            && (head != drag.anchor || self.selection.is_some())
        {
            self.selection = Some(selection::Selection {
                anchor: drag.anchor,
                head,
            });
        }
    }

    /// Light the selection over the rows drawn in `area` from `first`.
    fn paint_selection(&self, buf: &mut ratatui::buffer::Buffer, area: Rect, first: usize) {
        let Some(selected) = self.selection else {
            return;
        };
        let rows: Vec<(String, RowCopy)> = (first..first + usize::from(area.height))
            .map_while(|index| self.row(index).map(|(text, copy)| (text, copy.clone())))
            .collect();
        selection::paint(
            buf,
            area,
            first,
            &rows,
            &selected,
            Style::default().fg(t::BG_BASE).bg(t::TEXT_PRIMARY),
        );
    }

    fn visible(
        &self,
        tail: &[Line<'static>],
        mut offset: usize,
        height: usize,
    ) -> Vec<Line<'static>> {
        let mut rows = Vec::with_capacity(height);
        for block in self
            .blocks()
            .map(|cached| cached.lines.as_slice())
            .chain(std::iter::once(tail))
        {
            if offset >= block.len() {
                offset -= block.len();
                continue;
            }
            rows.extend(
                block
                    .iter()
                    .skip(offset)
                    .take(height.saturating_sub(rows.len()))
                    .cloned(),
            );
            offset = 0;
            if rows.len() >= height {
                break;
            }
        }
        rows
    }
}

/// The conversation as plain text rows, `width` cells wide, as the screen
/// draws it in the verbose view (`coder issue-run --plain`).
#[must_use]
pub fn transcript_text(entries: &[crate::live::Entry], width: u16) -> Vec<String> {
    transcript_lines(entries, width, ViewMode::Verbose)
}

/// [`transcript_text`] in a chosen view.
#[must_use]
pub fn transcript_lines(entries: &[crate::live::Entry], width: u16, view: ViewMode) -> Vec<String> {
    entries
        .iter()
        .flat_map(|entry| entry_rows(entry, width, 0, view).0)
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect()
}

fn live_lines(app: &App, width: u16) -> Vec<Line<'static>> {
    let mut lines = app
        .live
        .entries
        .iter()
        .flat_map(|entry| entry_rows(entry, width, app.animation_frame, app.view).0)
        .collect::<Vec<_>>();
    if !app.live.partial.is_empty() {
        reply_lines(
            &mut lines,
            &markdown_stream::renderable(&shown(&app.live.partial)),
            width,
        );
    }
    lines
}

fn live_conversation(frame: &mut Frame, area: Rect, app: &mut App) {
    let content = area.width.saturating_sub(GUTTER);
    let phase = app.animation_frame;
    let notice = app.notice.clone();
    // Read before `chat` borrows `app`'s delegations.
    let view = app.view;
    // A message waiting for its route shows at once where it will land,
    // as the person's message, instead of leaving the input blank (#11460).
    let held = app
        .selected_agent
        .is_none()
        .then(|| app.held_message().map(str::to_owned))
        .flatten();
    let chat = app
        .selected_agent
        .and_then(|index| app.delegations.get_mut(index))
        .map_or(&mut app.live, |agent| &mut agent.chat);
    let mut cache = std::mem::take(&mut chat.cache);
    cache.refresh(chat, content, phase, view);
    let mut tail = Vec::new();
    if let Some(held) = held {
        tail.extend(prompt(&held, content));
        tail.push(Line::default());
    }
    for (message, color) in [
        (chat.notice.as_ref(), t::DIFF_DELETE_FG),
        (notice.as_ref(), t::GRAY),
    ] {
        if let Some(message) = message {
            tail.extend(message_body(message, content).into_iter().map(|mut line| {
                line.style.fg = Some(color);
                for span in &mut line.spans {
                    span.style.fg = Some(color);
                }
                line
            }));
            tail.push(Line::default());
        }
    }
    let count = cache.count() + tail.len();
    let max_scroll = count
        .saturating_sub(usize::from(area.height))
        .min(usize::from(u16::MAX)) as u16;
    app.scroll_max = max_scroll;
    // Back at the bottom after scrolling up: follow new output again.
    if app.scroll >= max_scroll {
        app.scroll = u16::MAX;
    }
    let position = app.scroll.min(max_scroll);
    cache.locate_runs(area, usize::from(position));
    let visible = cache.visible(&tail, usize::from(position), usize::from(area.height));
    frame.render_widget(Paragraph::new(visible), area);
    cache.tail = tail;
    cache.view = Some((area, usize::from(position)));
    cache.follow_drag();
    cache.paint_selection(frame.buffer_mut(), area, usize::from(position));
    if app.scroll < max_scroll {
        frame.render_widget(
            Paragraph::new("↓")
                .alignment(ratatui::layout::Alignment::Right)
                .style(Style::default().bg(t::BG_DARK).fg(t::ACCENT_MODEL)),
            Rect {
                x: area.x + area.width.saturating_sub(1),
                y: area.y + area.height - 1,
                width: 1,
                height: 1,
            },
        );
    }
    chat.cache = cache;
}

fn reply_lines(lines: &mut Vec<Line<'static>>, text: &str, width: u16) {
    lines.extend(bulleted(&shown(text), width));
}

/// A reply as the terminal shows it: each component block
/// (```` ```openui-lang ````) as its Markdown fallback, with links written
/// out, numbered steps, and each command a code block (#11187).
fn shown(text: &str) -> std::borrow::Cow<'_, str> {
    if text.contains(openui_lang::LANG) {
        std::borrow::Cow::Owned(openui_lang::embed::fallback(text))
    } else {
        std::borrow::Cow::Borrowed(text)
    }
}

fn composer_rail_text(text: &str, width: u16) -> String {
    if text.width() <= usize::from(width) {
        return text.to_owned();
    }
    if let Some((model, options)) = text.split_once(':') {
        let suffix = format!(":{options}");
        let suffix_width = suffix.width();
        if suffix_width < usize::from(width) {
            return format!("{}{}", truncate(model, width - suffix_width as u16), suffix);
        }
    }
    truncate(text, width)
}

/// Paints `width` cells of the right rail `text` drawn at `offset` in
/// `style`, ending `skip` cells before its end, when the rail drew: the
/// mode's name keeps its own color after the gray plan and cost, and a
/// load warning its own before it.
fn accent_tail(
    buf: &mut ratatui::buffer::Buffer,
    area: Rect,
    offset: u16,
    text: &str,
    skip: u16,
    width: u16,
    style: Style,
) {
    let y = area.top() + offset;
    let end = area.right().saturating_sub(3);
    let first = end.saturating_sub(text.width() as u16);
    // The rail draws nothing when it does not fit: leave the rule alone.
    if y >= buf.area.bottom() || first <= area.left() || buf[(first, y)].symbol() == "─" {
        return;
    }
    let end = end.saturating_sub(skip);
    for x in end.saturating_sub(width).max(first)..end {
        buf[(x, y)].set_style(style);
    }
}

fn composer_view(
    frame: &mut Frame,
    area: Rect,
    draft: &[String],
    cursor: (u16, u16),
    main_selected: bool,
    cursor_visible: bool,
    plugins: &crate::plugins::Plugins,
    composer: &crate::composer_state::ComposerState,
    history_position: Option<(usize, usize)>,
    selected: bool,
    fallback_model: Option<&str>,
    top_extra: Option<&str>,
    warning: Option<&str>,
    working: Option<&str>,
    copied: bool,
) -> Option<(Rect, u16)> {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(t::PROMPT_BORDER_ACTIVE))
        .style(Style::default().bg(t::BG_BASE));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    coder_terminal::frame(
        area,
        frame.buffer_mut(),
        Style::default().fg(t::PROMPT_BORDER_ACTIVE),
    );
    let mut left_labels = Vec::new();
    // The spinner rides the top-left rail, where a person watching the
    // reply sees it without the transcript scrolling it away. While a
    // reply is going, nothing else claims that spot.
    if let Some(working) = working {
        left_labels.push(working.to_owned());
    }
    if copied {
        left_labels.push("Copied".to_owned());
    }
    if let Some((position, total)) = history_position {
        left_labels.push(format!("History {position}/{total}"));
    }
    left_labels.extend(
        composer
            .images
            .iter()
            .enumerate()
            .map(|(index, image)| crate::attachments::chip_for(&image.source, index + 1)),
    );
    let left_text = left_labels.join("  ");
    let left = Some((left_text.as_str(), Style::default().fg(t::GRAY)));
    coder_terminal::rail(area, frame.buffer_mut(), 0, left, None);
    let contributions = if let Some(model) = fallback_model {
        crate::plugin_definition::resolve_composer_rails(
            &[crate::plugin_definition::FALLBACK_PROVIDER],
            |_, _| Some(model),
        )
    } else {
        plugins.composer_rails()
    };
    let mut top_drawn = false;
    for contribution in contributions {
        let offset = match contribution.slot {
            RailSlot::ComposerTopRight => 0,
            RailSlot::ComposerBottomRight => area.height.saturating_sub(1),
        };
        // Before the first turn the loop's rail names the product the way
        // the turn label does, "Coder", never the lowercase id (#11483).
        let named = if contribution.text == crate::models::AUTO_LABEL {
            CODER_RAIL
        } else {
            contribution.text.as_str()
        };
        let label = composer_rail_text(named, area.width.saturating_sub(6));
        let text = contribution.text.as_str();
        // Which engine ran is a quiet fact in gray, never a colored
        // control that reads as a mode (owner, 2026-10-12).
        let _ = text;
        let accent = Style::default().fg(t::GRAY);
        // The plan and the cost sit before the mode's name, in gray, and a
        // load warning flashes just before the name in yellow.
        let mut extra = top_extra.filter(|_| offset == 0);
        let mut warning = warning.filter(|_| offset == 0);
        // The agent's name always shows: when the plan, the cost or a
        // warning would push the rail past the room left of the input's
        // top-left labels, they give way first, never the name.
        let left_taken = if offset == 0 && !left_text.is_empty() {
            left_text.width() + 2
        } else {
            0
        };
        let room = usize::from(area.width)
            .saturating_sub(4)
            .saturating_sub(left_taken);
        let fits = |extra: Option<&str>, warning: Option<&str>| {
            extra
                .into_iter()
                .chain(warning)
                .chain([label.as_str()])
                .collect::<Vec<_>>()
                .join(" · ")
                .width()
                + 2
                <= room
        };
        if !fits(extra, warning) {
            extra = None;
        }
        if !fits(extra, warning) {
            warning = None;
        }
        // The name comes first and what is its own follows it, so a
        // plan's windows read as that harness's: "Claude (Opus 5.5) · 5h
        // 12% · week 15%" (#11483).
        let text = warning
            .into_iter()
            .chain([label.as_str()])
            .chain(extra)
            .collect::<Vec<_>>()
            .join(" · ");
        let plain = extra.is_none() && warning.is_none();
        coder_terminal::rail(
            area,
            frame.buffer_mut(),
            offset,
            if offset == 0 { left } else { None },
            Some((
                &text,
                if plain {
                    accent
                } else {
                    Style::default().fg(t::GRAY)
                },
            )),
        );
        if !plain {
            let label_width = label.width() as u16;
            let after_label = extra.map_or(0, |extra| extra.width() as u16 + 3);
            accent_tail(
                frame.buffer_mut(),
                area,
                offset,
                &text,
                after_label,
                label_width,
                accent,
            );
            if let Some(warning) = warning {
                accent_tail(
                    frame.buffer_mut(),
                    area,
                    offset,
                    &text,
                    after_label + label_width + 3,
                    warning.width() as u16,
                    Style::default().fg(t::WARNING),
                );
            }
        }
        top_drawn |= offset == 0;
    }
    let unlabeled = top_extra
        .into_iter()
        .chain(warning)
        .collect::<Vec<_>>()
        .join(" · ");
    if !top_drawn && !unlabeled.is_empty() {
        coder_terminal::rail(
            area,
            frame.buffer_mut(),
            0,
            left,
            Some((&unlabeled, Style::default().fg(t::GRAY))),
        );
        if let Some(warning) = warning {
            accent_tail(
                frame.buffer_mut(),
                area,
                0,
                &unlabeled,
                0,
                warning.width() as u16,
                Style::default().fg(t::WARNING),
            );
        }
    }
    if inner.width < 4 || inner.height == 0 {
        return None;
    }
    frame.render_widget(
        Paragraph::new(span(
            if composer.mode == crate::composer_state::InputMode::Bash {
                "!"
            } else {
                "❯"
            },
            if main_selected {
                t::TEXT_SECONDARY
            } else {
                t::GRAY_DIM
            },
        )),
        inner,
    );
    let text_area = Rect {
        x: inner.x + 2,
        width: inner.width - 2,
        ..inner
    };
    let scroll = cursor.1.saturating_sub(text_area.height.saturating_sub(1));
    let text = Text::from(
        draft
            .iter()
            .map(|line| {
                // Cmd+A selected the whole input: show it as a selection.
                let style = Style::default().fg(t::TEXT_PRIMARY);
                Line::from(Span::styled(
                    line.clone(),
                    if selected {
                        style.add_modifier(Modifier::REVERSED)
                    } else {
                        style
                    },
                ))
            })
            .collect::<Vec<_>>(),
    );
    frame.render_widget(Paragraph::new(text).scroll((scroll, 0)), text_area);
    if cursor_visible {
        frame.set_cursor_position((
            text_area.x + cursor.0.min(text_area.width.saturating_sub(1)),
            text_area.y + cursor.1.saturating_sub(scroll),
        ));
    }
    Some((text_area, scroll))
}

#[cfg(test)]
mod export_notice_tests {
    #[test]
    fn history_position_appears_on_the_top_left_rail() {
        let mut app = App::default();
        app.mode = Mode::Live;
        app.live.runs_on = Some(crate::harness_plugin::Harness::CoderLoop);
        for i in 0..100 {
            app.draft.text = format!("prompt {i}");
            app.record_prompt();
        }
        app.draft = crate::Draft::default();
        let rail = |app: &mut App, width| {
            let mut terminal = Terminal::new(TestBackend::new(width, 20)).unwrap();
            terminal.draw(|frame| render(frame, app)).unwrap();
            let buffer = terminal.backend().buffer();
            let row = (1..20).find(|&y| buffer[(1, y)].symbol() == "❯").unwrap();
            (0..width)
                .map(|x| buffer[(x, row - 1)].symbol())
                .collect::<String>()
        };
        assert!(!rail(&mut app, 60).contains("History"));
        app.composer_arrow(true);
        assert!(rail(&mut app, 60).starts_with("┌─ History 100/100 "));
        app.composer_arrow(true);
        assert!(rail(&mut app, 60).starts_with("┌─ History 99/100 "));
        for _ in 0..100 {
            app.composer_arrow(true);
        }
        assert!(rail(&mut app, 24).starts_with("┌─ History 1/100 "));
        for _ in 0..99 {
            app.draft.cursor = app.draft.text.len();
            app.composer_arrow(false);
        }
        let text = rail(&mut app, 60);
        assert!(text.starts_with("┌─ History 100/100 "));
        assert!(text.contains("Coder (auto)"));
        app.composer_arrow(false);
        assert!(!rail(&mut app, 60).contains("History"));
        app.composer_arrow(true);
        app.composer_history.reset();
        assert!(!rail(&mut app, 60).contains("History"));
    }

    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn the_engine_label_is_quiet_and_absent_before_routing() {
        let mut app = App::default();
        app.mode = Mode::Live;
        let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
        terminal.draw(|frame| render(frame, &mut app)).unwrap();
        let rows: Vec<String> = (0..20)
            .map(|y| {
                (0..60)
                    .map(|x| terminal.backend().buffer()[(x, y)].symbol().to_owned())
                    .collect()
            })
            .collect();
        let top = rows
            .iter()
            .rev()
            .find(|row| row.starts_with('┌'))
            .expect("the input's top rule")
            .clone();
        // Before the first turn the input's rule names no engine and no mode.
        assert!(!top.contains("coder") && !top.contains("Coder"), "{top}");
        app.live.runs_on = Some(crate::harness_plugin::Harness::CoderLoop);
        terminal.draw(|frame| render(frame, &mut app)).unwrap();
        let buffer = terminal.backend().buffer();
        let mut found = false;
        let label = "Coder (auto)";
        let n = label.chars().count() as u16;
        for y in 0..20 {
            for x in 0..60 - n {
                let text: String = (x..x + n).map(|x| buffer[(x, y)].symbol()).collect();
                if text == label {
                    found = true;
                    for x in x..x + n {
                        assert_eq!(buffer[(x, y)].fg, t::GRAY);
                    }
                }
            }
        }
        assert!(found);
    }

    #[test]
    fn composer_has_square_corners_and_side_walls() {
        for (mode, glyph) in [
            (crate::composer_state::InputMode::Prompt, "❯"),
            (crate::composer_state::InputMode::Bash, "!"),
        ] {
            let mut app = App::default();
            app.mode = Mode::Live;
            app.composer.mode = mode;
            app.draft.text = "hello".into();
            let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
            terminal.draw(|frame| render(frame, &mut app)).unwrap();
            let buffer = terminal.backend().buffer();
            let row = (0..20).find(|&y| buffer[(1, y)].symbol() == glyph).unwrap();
            assert_eq!(buffer[(2, row)].symbol(), " ");
            assert_eq!(buffer[(3, row)].symbol(), "h");
            assert_eq!(app.composer_width, 56);
            assert_eq!(buffer[(0, row)].symbol(), "│");
            assert_eq!(buffer[(59, row)].symbol(), "│");
            for (x, y, glyph) in [
                (0, row - 1, "┌"),
                (59, row - 1, "┐"),
                (0, row + 1, "└"),
                (59, row + 1, "┘"),
            ] {
                assert_eq!(buffer[(x, y)].symbol(), glyph);
            }
        }
    }

    #[test]
    fn empty_footer_does_not_leave_a_row_below_the_composer() {
        for (width, height) in [(24, 12), (60, 20), (100, 30)] {
            let mut app = App::default();
            app.mode = Mode::Live;
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal.draw(|frame| render(frame, &mut app)).unwrap();
            let buffer = terminal.backend().buffer();
            assert_eq!(buffer[(0, height - 1)].symbol(), "└");
            assert_eq!(buffer[(width - 1, height - 1)].symbol(), "┘");
        }
    }

    /// There is no mode switch: Shift+Tab and Tab change nothing, and the
    /// rail names the harness the chat ran on (#11453).
    #[test]
    fn shift_tab_switches_nothing_and_the_rail_names_the_harness() {
        use crate::harness_plugin::Harness;
        use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
        let press = |code| Event::Key(KeyEvent::new(code, KeyModifiers::NONE));
        let rail = |app: &mut App| {
            let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
            terminal.draw(|frame| render(frame, app)).unwrap();
            let buffer = terminal.backend().buffer();
            (0..20)
                .map(|y| (0..60).map(|x| buffer[(x, y)].symbol()).collect::<String>())
                .collect::<Vec<_>>()
                .join("\n")
        };
        let mut app = App::default();
        app.mode = Mode::Live;
        app.plugins.model = crate::models::AUTO.into();
        app.plugins.enabled = true;
        app.plugins.key_configured = true;
        app.live.runs_on = Some(Harness::CoderLoop);
        for code in [KeyCode::Tab, KeyCode::BackTab, KeyCode::BackTab] {
            app.handle(press(code));
            assert_eq!(app.plugins.model, crate::models::AUTO);
            assert_eq!(app.mode, Mode::Live);
            assert_eq!(app.live.runs_on, Some(Harness::CoderLoop));
        }
        let shown = rail(&mut app);
        assert!(shown.contains("Coder (auto)"), "{shown}");
        assert!(!shown.contains("openrouter") && !shown.contains("claude"));
        // A model chosen in /models is named after the loop.
        app.plugins.model = crate::models::OPENROUTER.into();
        assert!(rail(&mut app).contains("Coder (deepseek-v4.1-flash)"));
        // #11519: on auto, the model the last turn ran and how full the
        // context is.
        app.plugins.model = crate::models::AUTO.into();
        app.live.entries.push(crate::live::Entry::User("hi".into()));
        app.live.entries.push(crate::live::Entry::Assistant {
            text: "Hello.".into(),
            model: Some("google/gemini-3.8-flash".into()),
            elapsed_ms: None,
        });
        let shown = rail(&mut app);
        assert!(shown.contains("Coder (Gemini 3.8 Flash)"), "{shown}");
        assert!(shown.contains("Context 0%"), "{shown}");
    }

    #[test]
    fn the_rail_names_claude_and_its_model_without_asking_a_login() {
        let mut app = App::default();
        app.mode = Mode::Live;
        app.live.runs_on = Some(crate::harness_plugin::Harness::ClaudeCode);
        let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
        terminal.draw(|frame| render(frame, &mut app)).unwrap();
        let buffer = terminal.backend().buffer();
        let shown: String = (0..20)
            .map(|y| (0..60).map(|x| buffer[(x, y)].symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            shown.contains(&format!("Claude ({})", crate::claude::MODEL_LABEL)),
            "{shown}"
        );
        // Drawing the rail must not read this computer's login: a test
        // environment never starts `claude auth status`.
    }

    #[test]
    fn a_load_warning_flashes_in_yellow_before_the_mode() {
        let width = 80;
        let mut terminal = Terminal::new(TestBackend::new(width, 6)).unwrap();
        terminal
            .draw(|frame| {
                composer_view(
                    frame,
                    frame.area(),
                    &[String::new()],
                    (0, 0),
                    true,
                    false,
                    &crate::plugins::Plugins::default(),
                    &crate::composer_state::ComposerState::default(),
                    None,
                    false,
                    Some("Claude (Haiku 5.5)"),
                    Some("ctx 40%"),
                    Some("stripe needs sign-in"),
                    None,
                    false,
                );
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let rail: String = (0..width).map(|x| buffer[(x, 0)].symbol()).collect();
        assert!(
            rail.contains("stripe needs sign-in · Claude (Haiku 5.5) · ctx 40%"),
            "{rail}"
        );
        let at = |word: &str| {
            let cell = rail.find(word).unwrap();
            // Every cell before is one column wide on this rail but the rule.
            rail[..cell].chars().count() as u16
        };
        assert_eq!(buffer[(at("stripe"), 0)].fg, t::WARNING);
        assert_eq!(buffer[(at("sign-in"), 0)].fg, t::WARNING);
        assert_eq!(buffer[(at("ctx"), 0)].fg, t::GRAY);
        assert_eq!(buffer[(at("Claude"), 0)].fg, t::GRAY);
    }

    /// Claude's plan windows are Claude's: they show only while Claude runs
    /// the chat, right after its name, and never at startup, on the path's
    /// rail, or beside Coder's own loop, whose startup name is "Coder"
    /// (#11483).
    #[test]
    fn claudes_plan_windows_sit_beside_claude_only() {
        use crate::harness_plugin::Harness;
        let _serial = crate::claude_plan::TEST_SERIAL
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        crate::claude_plan::reset_for_test(1);
        let now = atif::now_ms() / 1000;
        crate::claude_plan::observe_rate_limit(
            &serde_json::json!({"rateLimitType": "five_hour", "utilization": 0.12,
                "resetsAt": now + 3600, "status": "allowed"}),
            now,
        );
        let plan = crate::claude_plan::rail_text(now).expect("the plan's windows");
        assert!(plan.starts_with("5h 12%"), "{plan}");
        let rail = |app: &mut App| {
            let mut terminal = Terminal::new(TestBackend::new(100, 20)).unwrap();
            terminal.draw(|frame| render(frame, app)).unwrap();
            let buffer = terminal.backend().buffer();
            (0..20)
                .map(|y| {
                    (0..100)
                        .map(|x| buffer[(x, y)].symbol())
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        let mut app = App::default();
        app.mode = Mode::Live;
        app.cwd = Some("/tmp/repo".into());
        app.plugins.model = crate::models::AUTO.into();
        app.plugins.enabled = true;
        // Startup: the product's name, and no plan anywhere.
        let shown = rail(&mut app);
        assert!(shown.contains("Coder"), "{shown}");
        assert!(
            !shown.contains("coder") && !shown.contains("5h "),
            "{shown}"
        );
        // Coder's own loop answers: still no Claude plan.
        app.live.runs_on = Some(Harness::CoderLoop);
        let shown = rail(&mut app);
        assert!(
            shown.contains("Coder (auto)") && !shown.contains("5h "),
            "{shown}"
        );
        // Claude runs the chat: its windows follow its name.
        app.live.runs_on = Some(Harness::ClaudeCode);
        let shown = rail(&mut app);
        let expected = format!("{} · {plan}", claude_rail_label());
        assert!(shown.contains(&expected), "{shown}");
        assert!(!repository_rail(&app).contains("5h "));
        crate::claude_plan::reset_for_test(1);
    }

    /// A long warning or plan line gives way on a narrow input; the
    /// agent's name never does.
    #[test]
    fn a_long_rail_drops_the_extras_and_keeps_the_agent() {
        let width = 48;
        let mut terminal = Terminal::new(TestBackend::new(width, 6)).unwrap();
        terminal
            .draw(|frame| {
                composer_view(
                    frame,
                    frame.area(),
                    &[String::new()],
                    (0, 0),
                    true,
                    false,
                    &crate::plugins::Plugins::default(),
                    &crate::composer_state::ComposerState::default(),
                    None,
                    false,
                    Some("Claude (Haiku 5.5)"),
                    Some("5h 33% · week 83% · ctx 40%"),
                    Some("stripe and linear need sign-in"),
                    None,
                    false,
                );
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let rail: String = (0..width).map(|x| buffer[(x, 0)].symbol()).collect();
        assert!(rail.contains("Claude (Haiku 5.5)"), "{rail}");
    }

    #[test]
    fn scrolling_back_to_the_bottom_follows_new_output_again() {
        let mut app = App::default();
        app.mode = Mode::Live;
        for i in 0..40 {
            app.live
                .entries
                .push(crate::live::Entry::User(format!("message {i}")));
        }
        let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
        let mut draw = |app: &mut App| {
            terminal.draw(|frame| render(frame, app)).unwrap();
        };
        app.scroll = u16::MAX;
        draw(&mut app);
        assert_eq!(app.scroll, u16::MAX);
        // Scrolled up: it stays put while output arrives.
        app.scroll = 3;
        app.live
            .entries
            .push(crate::live::Entry::User("newer".into()));
        draw(&mut app);
        assert_eq!(app.scroll, 3);
        // Scrolled back down past the end: following again.
        app.scroll = app.scroll_max.saturating_add(3);
        draw(&mut app);
        assert_eq!(app.scroll, u16::MAX);
    }

    /// A gap row above each reply used to be pushed and popped on
    /// alternating draws, which slid the whole transcript a row every tick
    /// (#11395).
    #[test]
    fn a_replies_gap_rows_do_not_oscillate() {
        let mut app = App::default();
        app.mode = Mode::Live;
        app.live.busy = true;
        for i in 0..25 {
            app.live.entries.push(crate::live::Entry::Tool {
                name: "Read".into(),
                input: serde_json::json!({"path": format!("/tmp/file-{i}.rs")}),
                output: serde_json::json!({}),
                running: false,
            });
            app.live.entries.push(crate::live::Entry::Assistant {
                text: format!("reply {i}"),
                model: None,
                elapsed_ms: None,
            });
        }
        app.live.entries.push(crate::live::Entry::Tool {
            name: "Run".into(),
            input: serde_json::json!({"command": "sleep 60"}),
            output: serde_json::json!({"stdout": "one\ntwo\nthree\n"}),
            running: true,
        });
        app.scroll = u16::MAX;
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        let mut positions = Vec::new();
        let mut counts = Vec::new();
        for phase in 0..16u8 {
            app.animation_frame = phase % 8;
            terminal.draw(|frame| render(frame, &mut app)).unwrap();
            positions.push(app.live.cache.view.unwrap().1);
            counts.push(app.live.cache.count());
        }
        assert!(
            counts.windows(2).all(|pair| pair[0] == pair[1]),
            "{counts:?}"
        );
        assert!(
            positions.windows(2).all(|pair| pair[0] == pair[1]),
            "{positions:?}"
        );
    }

    /// A message the person sends, or the model's thinking, under a compact
    /// tool row keeps one blank row above it (owner, 2026-10-12).
    #[test]
    fn a_message_or_a_thought_under_a_tool_row_has_a_blank_above() {
        let tool = || crate::live::Entry::Tool {
            name: "Bash".into(),
            input: serde_json::json!({"command": "ls"}),
            output: serde_json::json!({}),
            running: false,
        };
        let mut app = App::default();
        app.mode = Mode::Live;
        app.live.entries = vec![
            tool(),
            crate::live::Entry::User("in coder-new".into()),
            tool(),
            crate::live::Entry::Tool {
                name: crate::claude_stream::THINKING.into(),
                input: serde_json::Value::Null,
                output: serde_json::json!({"text": "checking the plan file"}),
                running: false,
            },
        ];
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| render(frame, &mut app)).unwrap();
        let buffer = terminal.backend().buffer();
        let rows: Vec<String> = (0..24)
            .map(|y| (0..80).map(|x| buffer[(x, y)].symbol()).collect::<String>())
            .collect();
        for needle in ["in coder-new", "checking the plan file"] {
            let at = rows.iter().position(|row| row.contains(needle)).unwrap();
            assert!(rows[at - 1].trim().is_empty(), "{needle}: {rows:#?}");
            assert!(rows[at - 2].contains("ls"), "{needle}: {rows:#?}");
        }
    }

    #[test]
    fn the_terminal_background_keeps_diff_bands() {
        let mut app = App::default();
        app.mode = Mode::Live;
        // The bands are the verbose view's diff rows.
        app.view = ViewMode::Verbose;
        app.appearance.use_system_terminal_background = true;
        let diff = crate::file_tools::unified("a\nold line\nb\n", "a\nnew line\nb\n");
        app.live.entries.push(crate::live::Entry::Tool {
            name: "Edit".into(),
            input: serde_json::json!({"path": "src/x.rs"}),
            output: serde_json::json!({
                "path": "src/x.rs", "diff": diff.text, "added": diff.added, "removed": diff.removed,
            }),
            running: false,
        });
        let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
        terminal.draw(|frame| render(frame, &mut app)).unwrap();
        let cells = &terminal.backend().buffer().content;
        assert!(cells.iter().any(|cell| cell.bg == t::DIFF_DELETE_BG));
        assert!(cells.iter().any(|cell| cell.bg == t::DIFF_INSERT_BG));
        assert!(cells.iter().all(|cell| cell.bg != t::BG_BASE));
    }

    #[test]
    fn nothing_but_the_agent_rail_sits_under_the_input() {
        let mut app = App::default();
        app.mode = Mode::Live;
        app.account = Some("Christopher David".into());
        app.update_line = Some("Update ready".into());
        let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
        terminal.draw(|frame| render(frame, &mut app)).unwrap();
        let buffer = terminal.backend().buffer();
        // The input's bottom rule is the last row, and it carries the note.
        assert_eq!(buffer[(0, 19)].symbol(), "└");
        let text: String = (0..60).map(|x| buffer[(x, 19)].symbol()).collect();
        assert!(text.contains("Update ready"), "{text}");
        // The account's name never shows.
        assert!(!text.contains("Christopher"), "{text}");
    }

    #[test]
    fn a_sent_message_shows_image_chips_and_collapses_in_the_compact_view() {
        let entry = crate::live::Entry::User(format!(
            "{} last\n\n[Image #1: /Users/me/.openagents/coder/attachments/ab.png]",
            "word ".repeat(60)
        ));
        let text = |view| {
            entry_rows(&entry, 60, 0, view)
                .0
                .iter()
                .map(|line| line.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        };
        let compact = text(ViewMode::Compact);
        assert_eq!(
            compact.lines().filter(|row| !row.trim().is_empty()).count(),
            3,
            "{compact}"
        );
        assert!(compact.contains(" …"), "{compact}");
        assert!(!compact.contains("last"), "{compact}");
        let verbose = text(ViewMode::Verbose);
        assert!(verbose.contains("[Image #1]"), "{verbose}");
        assert!(!verbose.contains("/Users/me"), "{verbose}");
    }

    #[test]
    fn wrapped_input_keeps_the_side_walls_clear() {
        for width in [24, 60, 100] {
            let mut app = App::default();
            app.mode = Mode::Live;
            app.draft.text = "x".repeat(usize::from(width - 3));
            app.draft.cursor = app.draft.text.len();
            let mut terminal = Terminal::new(TestBackend::new(width, 20)).unwrap();
            terminal.draw(|frame| render(frame, &mut app)).unwrap();
            let buffer = terminal.backend().buffer();
            let row = (0..20).find(|&y| buffer[(1, y)].symbol() == "❯").unwrap();
            for y in row..=row + 1 {
                assert_eq!(buffer[(0, y)].symbol(), "│");
                assert_eq!(buffer[(width - 1, y)].symbol(), "│");
            }
            assert_eq!(buffer[(3, row + 1)].symbol(), "x");
            assert_eq!(app.composer_width, width - 4);
        }
    }

    #[test]
    fn select_all_shows_the_input_as_a_selection() {
        let mut app = App::default();
        app.mode = Mode::Live;
        app.draft.text = "pick me".into();
        app.draft.cursor = app.draft.text.len();
        let selected = |app: &mut App| {
            let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
            terminal.draw(|frame| render(frame, app)).unwrap();
            let buffer = terminal.backend().buffer().clone();
            buffer
                .content()
                .iter()
                .any(|cell| cell.symbol() == "p" && cell.modifier.contains(Modifier::REVERSED))
        };
        assert!(!selected(&mut app));
        app.composer_selected = true;
        assert!(selected(&mut app));
    }

    #[test]
    fn the_composer_cursor_is_placed_and_never_hidden() {
        // The terminal blinks the block itself; the renderer must not hide it,
        // or the two blinks fight and the caret reads wrong (#11267).
        for width in [24, 60, 100] {
            let mut app = App::default();
            app.mode = Mode::Live;
            app.draft.text = "the caret sits after this".into();
            app.draft.cursor = app.draft.text.len();
            let mut terminal = Terminal::new(TestBackend::new(width, 20)).unwrap();
            terminal.draw(|frame| render(frame, &mut app)).unwrap();
            let backend = terminal.backend();
            assert!(backend.cursor_visible(), "the caret draws at width {width}");
            let x = backend.cursor_position().x;
            assert!(
                x >= 3 && x <= app.composer_width,
                "the caret sits inside the input at width {width}, got {x}"
            );
        }
    }

    #[test]
    fn scroll_arrow_does_not_repaint_notice_row() {
        let mut app = App::default();
        app.notice = Some("Exported ATIF to chat.json. Path copied to clipboard. ".repeat(5));
        app.scroll = 0;
        let mut terminal = Terminal::new(TestBackend::new(40, 3)).unwrap();
        terminal
            .draw(|frame| live_conversation(frame, frame.area(), &mut app))
            .unwrap();
        let buffer = terminal.backend().buffer();
        assert_ne!(buffer[(0, 2)].symbol(), " ");
        assert_eq!(buffer[(0, 2)].fg, t::GRAY);
        assert_eq!(buffer[(9, 2)].fg, t::GRAY);
        assert_eq!(buffer[(39, 2)].symbol(), "↓");
        assert_eq!(buffer[(39, 2)].fg, t::ACCENT_MODEL);
    }
}

#[cfg(test)]
mod agent_rail_tests {
    use crate::live::{Chat, Entry};

    #[test]
    fn a_running_agent_shows_what_it_is_doing_and_keeps_its_model() {
        let mut chat = Chat {
            entries: vec![Entry::User("11515".into())],
            ..Chat::default()
        };
        assert_eq!(super::agent_activity(&chat), None);
        assert_eq!(super::agent_model(&chat), None);
        chat.entries.push(Entry::Assistant {
            text: "Reading the issue.".into(),
            model: Some("claude-haiku-5-5".into()),
            elapsed_ms: None,
        });
        chat.entries.push(Entry::Tool {
            name: "Bash".into(),
            input: serde_json::json!({"command": "cargo test -p coder-new", "description": "Run the crate's tests"}),
            output: serde_json::Value::Null,
            running: true,
        });
        // The model stays after the reply that named it has finished.
        assert_eq!(super::agent_model(&chat), Some("claude-haiku-5-5"));
        assert_eq!(
            super::agent_activity(&chat).as_deref(),
            Some("Bash · Run the crate's tests")
        );
        chat.entries.push(Entry::Tool {
            name: crate::claude_stream::THINKING.into(),
            input: serde_json::Value::Null,
            output: serde_json::json!({"text": "The snapshot needs the turn's start."}),
            running: false,
        });
        assert_eq!(
            super::agent_activity(&chat).as_deref(),
            Some("The snapshot needs the turn's start.")
        );
        chat.partial = "Writing the restore step\nnow".into();
        assert_eq!(super::agent_activity(&chat).as_deref(), Some("now"));
    }
}

#[cfg(test)]
mod streaming_tests {
    use super::{TranscriptCache, ViewMode};

    fn shown(partial: &str) -> String {
        let mut chat = crate::live::Chat::default();
        chat.partial = partial.into();
        let mut cache = TranscriptCache::default();
        cache.refresh(&chat, 60, 0, ViewMode::Verbose);
        cache
            .blocks()
            .flat_map(|cached| cached.lines.iter())
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// A reply still streaming shows no half-written Markdown: each case
    /// from #11112 is held back until it completes, and plain text and the
    /// code so far show.
    #[test]
    fn a_streaming_reply_never_shows_half_written_markdown() {
        for (partial, hidden) in [
            ("Intro\n\n| a | b |\n|--", "|"),
            ("Intro\n\n| a | b |\n|---|---|\n| 1 |", "| 1"),
            ("Intro and [the docs](https://openagents", "]("),
            ("Intro\n\n1. One\n2.", "2."),
            ("Intro **bold te", "**"),
            ("Intro\n\n##", "##"),
            ("Intro\n\n```rust\nfn main", "```"),
        ] {
            let text = shown(partial);
            assert!(text.contains("Intro"), "{partial:?}: {text}");
            assert!(
                !text.contains(hidden),
                "{partial:?} shows {hidden:?}: {text}"
            );
        }
        assert!(shown("Intro\n\n```rust\nfn main").contains("fn main"));
    }

    const BLOCK: &str = "Connect it here.\n\n```openui-lang\nroot = Card(\"On your computer\", [Steps([install]), Button(\"Approve sign-in\", href=\"/device\")])\ninstall = Step(\"Install Coder\", [Command(\"curl -fsSL https://openagents.com/cli/install.sh | bash\")])\n```\n";

    /// A component block shows as readable Markdown: numbered steps, the
    /// command, and the link written out; never its statements (#11187).
    #[test]
    fn a_component_block_shows_as_markdown() {
        let mut lines = Vec::new();
        super::reply_lines(&mut lines, BLOCK, 80);
        let text = lines
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        for expected in [
            "Install Coder",
            "curl -fsSL https://openagents.com/cli/install.sh | bash",
            "https://openagents.com/device",
        ] {
            assert!(text.contains(expected), "{expected}: {text}");
        }
        for raw in ["root =", "Steps(", "openui-lang"] {
            assert!(!text.contains(raw), "{raw}: {text}");
        }
        // While it streams, no statement or half-written command shows.
        for (at, _) in BLOCK.char_indices() {
            let text = shown(&BLOCK[..at]);
            for raw in ["root", "Card(", "href"] {
                assert!(
                    !text.contains(raw),
                    "{:?} shows {raw}: {text}",
                    &BLOCK[..at]
                );
            }
            assert!(!text.contains("curl") || text.contains("bash"), "{text}");
        }
    }
}

#[cfg(test)]
mod run_viewport_tests {
    #[test]
    fn delegate_label_and_indicator_match_the_outcome() {
        for (running, output, color) in [
            (true, serde_json::json!({}), t::GRAY),
            (
                false,
                serde_json::json!({"error": "Failed"}),
                t::TOOL_FAILED,
            ),
            (false, serde_json::json!({}), t::ACCENT_SUCCESS),
        ] {
            let entry = crate::live::Entry::Delegation {
                id: "test".into(),
                name: "Coder".into(),
                task: "Example".into(),
                running,
                output,
                progress: None,
            };
            let lines = entry_rows(&entry, 60, 0, ViewMode::Verbose).0;
            for span in &lines[0].spans[..2] {
                assert_eq!(span.style.fg, Some(color));
            }
        }
    }

    #[test]
    fn all_tool_headers_use_outcome_colors() {
        for name in [
            "Run",
            "Read",
            "Write",
            "Edit",
            "Grep",
            "Glob",
            "Search",
            "example.plugin",
        ] {
            for (running, output, color) in [
                (true, serde_json::json!({"error": "old error"}), t::GRAY),
                (
                    false,
                    serde_json::json!({"error": "Failed"}),
                    t::TOOL_FAILED,
                ),
                (false, serde_json::json!({"exit": 1}), t::TOOL_FAILED),
                (false, serde_json::json!({}), t::ACCENT_SUCCESS),
            ] {
                let entry = crate::live::Entry::Tool {
                    name: name.into(),
                    input: serde_json::json!({}),
                    output,
                    running,
                };
                let lines = entry_rows(&entry, 60, 0, ViewMode::Verbose).0;
                for span in &lines[0].spans[..2] {
                    assert_eq!(span.style.fg, Some(color), "{name}: {running}");
                }
            }
        }
    }

    #[test]
    fn run_indicators_use_outcome_colors() {
        for (output, glyph, color) in [
            (serde_json::json!({"exit": 0}), "● ", t::ACCENT_SUCCESS),
            (serde_json::json!({"error": "Failed"}), "× ", t::TOOL_FAILED),
            (serde_json::json!({"exit": 1}), "× ", t::TOOL_FAILED),
            (serde_json::json!({"timed_out": true}), "× ", t::TOOL_FAILED),
            (serde_json::json!({"canceled": true}), "× ", t::TOOL_FAILED),
        ] {
            let lines = run_lines(
                &serde_json::json!({"command": "example"}),
                &output,
                false,
                60,
                0,
            );
            assert_eq!(lines[0].spans[0].content, glyph);
            assert_eq!(lines[0].spans[0].style.fg, Some(color));
            assert_eq!(lines[0].spans[1].style.fg, Some(color));
            let running = run_lines(&serde_json::json!({}), &output, true, 60, 0);
            assert_eq!(running[0].spans[0].style.fg, Some(t::GRAY));
            assert_eq!(
                running[0].spans[0].content,
                format!("{} ", crate::tools::spinner(0))
            );
        }
    }

    #[test]
    fn run_status_uses_short_times_without_duplicate_labels() {
        let lines = super::run_lines(
            &serde_json::json!({"command": "sleep 8"}),
            &serde_json::json!({"elapsed_seconds": 8, "silent_seconds": 4, "activity": "Waiting"}),
            true,
            100,
            0,
        );
        let text = lines
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("elapsed: 8s · silent: 4s"), "{text}");
        for removed in [
            "Running",
            "activity",
            "Waiting",
            "elapsed_seconds",
            "silent_seconds",
        ] {
            assert!(!text.contains(removed), "{text}");
        }
    }

    use super::*;

    fn output(chat: &mut crate::live::Chat, count: usize, running: bool) {
        chat.entries = vec![crate::live::Entry::Tool {
            name: "Run".into(),
            input: serde_json::json!({"command": "example"}),
            output: serde_json::json!({"stdout": (1..=count).map(|n| format!("row {n}\n")).collect::<String>()}),
            running,
        }];
    }

    #[test]
    fn follows_new_output_until_the_person_scrolls_even_after_resize() {
        let mut chat = crate::live::Chat::default();
        let mut cache = TranscriptCache::default();
        output(&mut chat, 8, true);
        cache.refresh(&chat, 40, 0, ViewMode::Verbose);
        assert_eq!(cache.entries[0].run_output.as_ref().unwrap().2, 3);
        output(&mut chat, 12, true);
        cache.refresh(&chat, 40, 1, ViewMode::Verbose);
        assert_eq!(cache.entries[0].run_output.as_ref().unwrap().2, 7);
        cache.locate_runs(Rect::new(0, 0, 42, 20), 0);
        assert!(!cache.scroll_run(0, 19, true));
        let area = cache.run_regions[0].1;
        // Away from the left edge the wheel scrolls the chat, not the box.
        assert!(!cache.scroll_run(39, area.y, true));
        assert!(!cache.scroll_run(RUN_GRAB, area.y, true));
        assert_eq!(cache.entries[0].run_output.as_ref().unwrap().2, 7);
        assert!(cache.scroll_run(1, area.y, true));
        assert_eq!(cache.entries[0].run_output.as_ref().unwrap().2, 4);
        output(&mut chat, 16, false);
        cache.refresh(&chat, 40, 2, ViewMode::Verbose);
        assert_eq!(cache.entries[0].run_output.as_ref().unwrap().2, 4);
        cache.refresh(&chat, 30, 3, ViewMode::Verbose);
        assert_eq!(cache.entries[0].run_output.as_ref().unwrap().2, 4);
        assert!(!cache.entries[0].run_output.as_ref().unwrap().3);
    }

    #[test]
    fn five_output_rows_have_a_background_and_left_hand_scrollbar() {
        let mut chat = crate::live::Chat::default();
        output(&mut chat, 10, false);
        let mut cache = TranscriptCache::default();
        cache.refresh(&chat, 40, 0, ViewMode::Verbose);
        let cached = &cache.entries[0];
        let start = cached.run_output.as_ref().unwrap().0;
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(42, 10)).unwrap();
        terminal
            .draw(|frame| frame.render_widget(Paragraph::new(cached.lines.clone()), frame.area()))
            .unwrap();
        let buffer = terminal.backend().buffer();
        for y in start..start + 5 {
            assert_eq!(buffer[(20, y as u16)].bg, t::BG_LIGHT);
            assert_eq!(buffer[(39, y as u16)].bg, t::BG_LIGHT);
            assert_eq!(
                buffer[(0, y as u16)].symbol(),
                if y == start + 4 { "█" } else { "│" }
            );
            assert_ne!(buffer[(39, y as u16)].symbol(), "│");
        }
        let exported = transcript_text(&chat.entries, 40).join("\n");
        assert!(exported.contains("row 1\n"));
        assert!(exported.contains("row 10"));
        assert!(!exported.contains('█'));
    }

    /// A truncated box says which rows it is showing and no more:
    /// instructions for the wheel do not belong under a command.
    #[test]
    fn a_truncated_box_shows_only_its_row_count() {
        let mut chat = crate::live::Chat::default();
        output(&mut chat, 10, false);
        let mut cache = TranscriptCache::default();
        cache.refresh(&chat, 40, 0, ViewMode::Verbose);
        let shown = cache.entries[0]
            .lines
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(shown.contains("6–10 of 10"), "{shown}");
        assert!(!shown.contains("scroll"), "{shown}");
        assert!(!shown.contains("left edge"), "{shown}");
    }
}

#[cfg(test)]
mod repository_tests {
    use super::*;
    #[test]
    fn paths_use_home_prefix_only_for_descendants() {
        use std::path::Path;
        let home = Some(Path::new("/home/person"));
        assert_eq!(
            display_directory(Path::new("/home/person/openagents"), home),
            "~/openagents"
        );
        assert_eq!(display_directory(Path::new("/home/person"), home), "~");
        assert_eq!(
            display_directory(Path::new("/home/person-other/repo"), home),
            "/home/person-other/repo"
        );
    }
    #[test]
    fn repository_is_bottom_right_and_cost_top_right() {
        let mut app = App::default();
        app.mode = Mode::Live;
        app.cwd = Some("/work/repo".into());
        app.branch = Some("feature".into());
        app.live.cost_usd = 0.25;
        app.live.runs_on = Some(crate::harness_plugin::Harness::CoderLoop);
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 20)).unwrap();
        terminal.draw(|frame| render(frame, &mut app)).unwrap();
        let buffer = terminal.backend().buffer();
        let row = (0..20)
            .find(|&y| (0..4).any(|x| buffer[(x, y)].symbol() == "❯"))
            .unwrap();
        let text: String = (0..80).map(|x| buffer[(x, row + 1)].symbol()).collect();
        assert!(text.contains("/work/repo (feature)"), "{text}");
        assert!(!text.contains("$0.25"), "{text}");
        assert!(text.trim_end_matches(['─', '┘']).ends_with(' '), "{text}");
        let top: String = (0..80).map(|x| buffer[(x, row - 1)].symbol()).collect();
        assert!(top.contains("Coder (auto) · $0.25"), "{top}");
    }
}
