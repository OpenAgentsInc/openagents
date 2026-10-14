//! The same render function draws the terminal and exported previews.

pub(crate) mod agents;
mod appearance;
mod model_tip;
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
    let rail_height = rail_height(app).min(usize::from(area.height.saturating_sub(6))) as u16;
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
        text.lines().rev().find_map(preview_words)
    }
    fn first_line(text: &str) -> Option<String> {
        text.lines().find_map(preview_words)
    }
    if let Some(line) = latest_line(&chat.partial) {
        return Some(line);
    }
    chat.entries.iter().rev().find_map(|entry| match entry {
        crate::live::Entry::Tool { name, output, .. } if name == crate::claude_stream::THINKING => {
            latest_line(crate::claude_stream::entry_text(output))
        }
        // A notice is what just happened to the agent ("ran on OpenCode's
        // free Cat Jam instead", #11575).
        crate::live::Entry::Tool { name, output, .. } if name == crate::claude_stream::NOTICE => {
            latest_line(crate::claude_stream::entry_text(output))
        }
        crate::live::Entry::Tool { name, input, .. } => {
            // An agent's own lowercase tool id ("bash") reads as a name.
            let tool = crate::claude_tools::display_name(name);
            let mut chars = tool.chars();
            let tool = chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().collect::<String>() + chars.as_str()
            });
            // A shell call's own description reads better than its command;
            // the folder it runs in says nothing about what it is doing.
            let mut shown = input.clone();
            if let Some(fields) = shown.as_object_mut() {
                fields.remove("cwd");
            }
            let what = input
                .get("description")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| crate::tools::compact_params(&shown, 120));
            // The home folder as `~`, so a path shows what is inside it.
            let what = match user_home::home_string() {
                Ok(home) if home.len() > 1 => what.replace(&home, "~"),
                _ => what,
            };
            Some(if what.is_empty() {
                tool
            } else {
                format!("{tool} · {what}")
            })
        }
        crate::live::Entry::Assistant { text, .. } => first_line(text),
        crate::live::Entry::User(_) | crate::live::Entry::Delegation { .. } => None,
    })
}

/// One line of an agent's Markdown as the rail previews it: the words,
/// without a code fence, a heading's or list's or quote's marker, or the
/// bold and code marks around them (#11575: a reply ending in a fence
/// previewed as a bare "```"). `None` for a line with no words.
fn preview_words(line: &str) -> Option<String> {
    let line = line.trim();
    if line.starts_with("```") || line.starts_with("~~~") {
        return None;
    }
    let line = line.trim_start_matches(['#', '>', ' ']);
    let line = line
        .strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))
        .or_else(|| line.strip_prefix("+ "))
        .unwrap_or(line);
    let words = line.replace("**", "").replace('`', "");
    let words = words.trim();
    words
        .chars()
        .any(char::is_alphanumeric)
        .then(|| words.to_owned())
}

/// A model's name as a person reads it: a Claude model by its picker name
/// ("Opus 5.5"), any other by its own short name.
fn model_words(model: &str) -> String {
    // OpenCode's free models by the names its picker shows ("Cat Jam").
    if model.starts_with("opencode/") {
        let label = crate::opencode_chat::rail_label(model);
        if let Some(name) = label
            .strip_prefix("OpenCode (")
            .and_then(|rest| rest.strip_suffix(')'))
        {
            return name.to_owned();
        }
    }
    let bare = model.rsplit('/').next().unwrap_or(model);
    crate::claude::MODELS
        .iter()
        .find(|(id, _, _)| *id == bare)
        .map_or_else(
            || crate::models::label(model),
            |(_, name, _)| (*name).to_owned(),
        )
}

/// The keys the agent rail answers to, on the line above it. Every key named
/// here works: arrows move the selection and Esc goes back to the main chat.
pub(crate) const RAIL_HINT: &str = "↑/↓ to select · esc for main";

/// One row on the agent rail: who, what it is doing, and its figures.
struct RailRow {
    name: String,
    activity: String,
    tokens: String,
    seconds: u64,
}

/// The rail's rows: the main chat first, then each agent, as Claude Code
/// lists its agents (owner, 2026-10-13).
fn rail_rows(app: &App) -> Vec<RailRow> {
    let main = RailRow {
        name: "main".into(),
        activity: if app.live.busy {
            agent_activity(&app.live).unwrap_or_default()
        } else {
            String::new()
        },
        tokens: if app.live.tokens == 0 {
            String::new()
        } else {
            token_count(app.live.tokens)
        },
        seconds: 0,
    };
    let mut rows = vec![main];
    if app.mode == Mode::Demo {
        rows.extend(DEMOS.iter().map(|agent| RailRow {
            name: agent.name.into(),
            activity: agent.task.into(),
            tokens: agent.tokens.to_owned(),
            seconds: agent.elapsed_seconds.saturating_add(app.elapsed_seconds),
        }));
        return rows;
    }
    // A finished delegation leaves the rail after a while (owner, #11434).
    rows.extend(app.rail_agents().into_iter().map(|index| {
        let agent = &app.delegations[index];
        let name = agent_label(&agent.name, agent_model(&agent.chat)).into_owned();
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
            return RailRow {
                name,
                activity: task,
                tokens: if row.tokens == 0 {
                    String::new()
                } else {
                    token_count(row.tokens)
                },
                seconds: row.elapsed_seconds(agent_fleet::now_ms()),
            };
        }
        RailRow {
            name,
            // What a running agent is doing right now: its tool, its
            // thinking, or the line it is writing; its task until it has
            // done anything.
            activity: if agent.running
                && let Some(doing) = agent_activity(&agent.chat)
            {
                doing
            // A finished delegation's dollars (#11179).
            } else if agent.chat.cost_usd > 0.0 {
                format!(
                    "{} · {}",
                    agent_fleet::dollars(agent.chat.cost_usd),
                    agent.task
                )
            } else {
                agent.task.clone()
            },
            // A figure Coder does not have is left out, never shown as a
            // dash or 0s (#11482).
            tokens: if agent.chat.tokens == 0 {
                String::new()
            } else {
                token_count(agent.chat.tokens)
            },
            seconds: if agent.running {
                app.elapsed_seconds.saturating_sub(agent.started_at)
            } else {
                agent.elapsed_seconds
            },
        }
    }));
    rows
}

/// The rail's height: the hint, the main chat and every agent, or nothing
/// while no agent is on it.
pub(crate) fn rail_height(app: &App) -> usize {
    let agents = if app.mode == Mode::Demo {
        DEMOS.len()
    } else {
        app.rail_agents().len()
    };
    if agents == 0 { 0 } else { agents + 2 }
}

/// Which rail row is selected: the main chat (0) or an agent's.
fn rail_selected(app: &App) -> usize {
    let Some(selected) = app.selected_agent else {
        return 0;
    };
    if app.mode == Mode::Demo {
        return selected + 1;
    }
    app.rail_agents()
        .iter()
        .position(|index| *index == selected)
        .map_or(0, |at| at + 1)
}

/// The agent rail under the input, laid out as Claude Code's agent list: a
/// key hint, then a row per chat with an empty dot, the selected one with a
/// `❯` cursor and a filled white dot, each with what it is doing and, at the
/// right, its time and tokens; `↓ N more` when the list runs past the room.
fn agent_rail(frame: &mut Frame, area: Rect, app: &App) {
    if area.height == 0 {
        return;
    }
    let rows = rail_rows(app);
    let selected = rail_selected(app).min(rows.len() - 1);
    // The hint goes when the rail has room for one row only.
    let hint = area.height >= 2;
    if hint {
        frame.render_widget(
            Paragraph::new(span(truncate(RAIL_HINT, area.width), t::GRAY)),
            Rect { height: 1, ..area },
        );
    }
    let list = Rect {
        y: area.y + u16::from(hint),
        height: area.height - u16::from(hint),
        ..area
    };
    let room = usize::from(list.height);
    // Past the room, the last line says how many rows are below, and the
    // window scrolls to keep the selection in view.
    let shown = if rows.len() > room {
        room.saturating_sub(1).max(1)
    } else {
        rows.len()
    };
    let first = selected.saturating_sub(shown - 1);
    let hidden_below = rows.len() - (first + shown).min(rows.len());
    let narrow = area.width < 32;
    const GUTTER: u16 = 4;
    let name_width = (2 + rows.iter().map(|row| row.name.width()).max().unwrap_or(0)) as u16;
    let count_width = rows.iter().map(|row| row.tokens.width()).max().unwrap_or(0);
    let token_labels: Vec<_> = rows
        .iter()
        .map(|row| {
            if row.tokens.is_empty() {
                String::new()
            } else if narrow {
                format!("↓{:>count_width$}", row.tokens)
            } else {
                format!("↓ {:>count_width$} tokens", row.tokens)
            }
        })
        .collect();
    let timed_labels: Vec<_> = rows
        .iter()
        .zip(&token_labels)
        .map(|(row, tokens)| {
            // No time yet (an agent that just started, or one whose end
            // came with none) is left out too.
            match (row.seconds, tokens.is_empty()) {
                (0, _) => tokens.clone(),
                (seconds, true) => elapsed_time(seconds),
                (seconds, false) => format!("{} · {tokens}", elapsed_time(seconds)),
            }
        })
        .collect();
    let show_elapsed = usize::from(area.width)
        >= usize::from(GUTTER + name_width)
            + 1
            + timed_labels
                .iter()
                .map(|text| text.width())
                .max()
                .unwrap_or(0);
    for (offset, index) in (first..first + shown).enumerate() {
        let row = &rows[index];
        let line = Rect {
            y: list.y + offset as u16,
            height: 1,
            ..list
        };
        let active = index == selected;
        let suffix = if show_elapsed {
            &timed_labels[index]
        } else {
            &token_labels[index]
        };
        let [left, _gap, figures] = Layout::horizontal([
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(suffix.width() as u16),
        ])
        .areas(line);
        // A filled white dot and the cursor for the selected row, an empty
        // dot for the rest (owner, 2026-10-13).
        let (mark, name_color, dim) = if active {
            ("❯ ● ", t::TEXT_PRIMARY, t::GRAY_BRIGHT)
        } else {
            ("  ○ ", t::TEXT_SECONDARY, t::GRAY)
        };
        let bold = if active {
            Modifier::BOLD
        } else {
            Modifier::empty()
        };
        let activity_width = left.width.saturating_sub(GUTTER + name_width);
        let mut spans = vec![
            Span::styled(mark, Style::default().fg(name_color).add_modifier(bold)),
            Span::styled(
                truncate(&row.name, left.width.saturating_sub(GUTTER)),
                Style::default().fg(name_color).add_modifier(bold),
            ),
        ];
        if activity_width > 0 && !row.activity.is_empty() {
            let pad = usize::from(name_width) - row.name.width();
            spans.push(Span::raw(" ".repeat(pad)));
            spans.push(Span::styled(
                truncate(&row.activity, activity_width),
                Style::default().fg(dim).add_modifier(bold),
            ));
        }
        frame.render_widget(Paragraph::new(Line::from(spans)), left);
        frame.render_widget(
            Paragraph::new(Span::styled(
                suffix.clone(),
                Style::default().fg(dim).add_modifier(bold),
            ))
            .right_aligned(),
            figures,
        );
    }
    if hidden_below > 0 && room > shown {
        frame.render_widget(
            Paragraph::new(span(
                truncate(&format!("  ↓ {hidden_below} more"), list.width),
                t::GRAY,
            )),
            Rect {
                y: list.y + shown as u16,
                height: 1,
                ..list
            },
        );
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
    let home = user_home::home_os().map(std::path::PathBuf::from);
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
/// windows of the plan that pays for the chat's turns, whichever harness
/// runs them ("Claude (Opus 5.5) · 5h 12% · week 15%", "Coder (Haiku 5.5)
/// · Claude 5h 12% · week 15%"), and how full the context is; else what
/// the chat has cost, which is metered spend; a plan is paid by the month
/// rather than by the turn. No plan shows before the first turn or where
/// none pays (#11483, #11537).
fn top_rail(app: &App) -> Option<String> {
    use crate::harness_pick::Pick;
    use crate::harness_plugin::Harness;
    let now = atif::now_ms() / 1000;
    // The harness whose facts these are: the one picked, else the one
    // Auto routed to (#11524).
    let pick = app.current_pick();
    let shown = match pick {
        Pick::Harness(harness) => Some(harness),
        Pick::Auto => app
            .live
            .runs_on
            .filter(|_| !app.live.pinned && app.live.agent.is_none()),
        Pick::Agent { .. } => None,
    };
    // The chat is on what the rail names, so its session's facts are its.
    let running = shown.is_some() && shown == app.live.runs_on && app.live.agent.is_none();
    // Before Auto routes anything, the chat's own cost and context still
    // show; an agent's turns are not metered here.
    let own = running || (pick == Pick::Auto && shown.is_none());
    let cached_usage = |harness: Harness| {
        crate::harness_pick::cached(&Pick::Harness(harness).key())
            .and_then(|entry| entry.usage_now(now).map(str::to_owned))
    };
    let mut parts = Vec::new();
    // Whichever plan pays for the chat's turns shows its windows beside
    // the harness's name (#11483, #11537): Claude's on Claude Code and on
    // the Coder loop when the Claude plan answers it, Codex's the same
    // way (#11510).
    // The Coder loop's plan is read off the chat's own turns, so only
    // while the chat runs there.
    let paying = shown
        .filter(|harness| *harness != Harness::CoderLoop || running)
        .and_then(|harness| plan_paying(app, harness));
    // Another harness's name says whose plan the windows are.
    let whose = |name: &str, windows: String| {
        if shown == Some(Harness::CoderLoop) {
            format!("{name} {windows}")
        } else {
            windows
        }
    };
    match paying {
        Some(PlanPaying::Claude) => {
            if let Some(plan) =
                crate::claude_plan::rail_text(now).or_else(|| cached_usage(Harness::ClaudeCode))
            {
                parts.push(whose(CLAUDE_RAIL, plan));
            }
        }
        Some(PlanPaying::Codex) => {
            let windows = crate::codex::windows_text(now);
            if !windows.is_empty() {
                parts.push(whose("Codex", windows));
            } else if let Some(windows) = cached_usage(Harness::Codex) {
                parts.push(whose("Codex", windows));
            }
        }
        None => {}
    }
    if shown == Some(Harness::ClaudeCode)
        && running
        && let Some(context) = app
            .live
            .claude_session
            .as_ref()
            .and_then(|session| crate::claude_stream::context_line(&session.id))
    {
        parts.push(context);
    }
    // A turn the person's own OpenRouter key paid says so (#11550).
    if paying.is_none()
        && shown == Some(Harness::CoderLoop)
        && running
        && app.byok_turn()
        && served_model(app).is_some_and(|model| {
            crate::harness_plugin::paid_by(Harness::CoderLoop, false, &model, true)
                == crate::harness_plugin::Payer::OpenRouterKey
        })
    {
        parts.push(OPENROUTER_PAYER.to_owned());
    }
    let metered = paying.is_none();
    let cost = crate::long_session::session_cost(app);
    if cost > 0.0 && metered && own {
        parts.push(agent_fleet::dollars(cost));
    }
    // How full the Coder loop's context is, as Claude's rail shows its
    // own (#11519).
    if own && let Some(context) = crate::long_session::context_line(app) {
        parts.push(context);
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// A model id as a person reads it: Claude's and Codex's names from
/// /models, else the model's own words capitalized
/// (`google/gemini-3.8-flash` reads "Gemini 3.8 Flash").
pub(crate) fn rail_model_name(model: &str) -> String {
    // Cursor names a variant in brackets, "composer-2.5[fast=true]": each
    // switch that is on reads as a word, "Composer 2.5 Fast" (#11556).
    if let Some((base, rest)) = model.split_once('[') {
        let mut name = rail_model_name(base);
        for setting in rest.trim_end_matches(']').split(',') {
            if let Some((key, "true")) = setting.trim().split_once('=') {
                name.push(' ');
                name.push_str(&rail_model_name(key));
            }
        }
        return name;
    }
    // The Codex login's turns name their model `codex:<id>`.
    if let Some(model) = model.strip_prefix("codex:") {
        return crate::codex::name(model);
    }
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

/// The input's top-right rail (#11524): the harness pick, `Auto` until
/// Auto has routed a turn and then what ran ("Auto → Claude (Opus 5.5)"),
/// or the harness the person picked with Shift+Tab ("Codex (GPT-6.1-Sol)",
/// "Devin"). `solver` while `coder solver` watches a run.
fn rail_harness(app: &App) -> Option<String> {
    use crate::harness_pick::Pick;
    if app.mode == Mode::Solver {
        return Some(crate::solver::LABEL.into());
    }
    if app.mode != Mode::Live {
        return None;
    }
    match app.current_pick() {
        Pick::Auto => {
            // What Auto routed, once it has; a pin waiting to be lifted
            // at the next message is not Auto's.
            let routed = app
                .live
                .runs_on
                .filter(|_| !app.live.pinned && app.live.agent.is_none());
            Some(match routed {
                // A model picked in /models for the Coder loop pins it:
                // "Coder (Claude Fable 5.1)", no "Auto" (#11556).
                Some(harness @ crate::harness_plugin::Harness::CoderLoop)
                    if coder_model_picked(app) =>
                {
                    harness_label(app, harness)
                }
                // "Auto picked Claude": the chat can still move (#11537).
                Some(harness) => format!("Auto → {}", harness_label(app, harness)),
                None => "Auto".into(),
            })
        }
        Pick::Harness(harness) => Some(harness_label(app, harness)),
        Pick::Agent { id, name } => Some(agent_rail_label(&id, &name)),
    }
}

/// An ACP agent on the rail: its name and, in parentheses, the model its
/// next turn runs, in the agent's own words ("Grok Build (Grok 4.7)",
/// "Devin CLI (SWE-2 Medium)", #11556). That is the pick in /models, else
/// the default its own CLI names (Grok's `grok models`, Oh My Pi's first
/// chat model), never a model an earlier session reported, which may be
/// stale; until the list is read the rail names none. Devin's default is
/// Coder's fastest SWE-2, and a session Devin starts on another model is
/// refused (#11583), so its rail names that default too. A Clef model is never named: it
/// can't chat, and Oh My Pi with nothing else reads "(no chat model)".
pub(crate) fn agent_rail_label(id: &str, name: &str) -> String {
    use crate::agent_models as agents;
    agent_rail_from(
        id,
        name,
        &AgentFacts {
            choice: agents::choice(id),
            listed: agents::listed(id).is_some(),
            default: agents::default_model(id),
            cached: crate::harness_pick::cached(&format!("agent:{id}"))
                .and_then(|entry| entry.model),
        },
        |model| agents::display(id, model),
    )
}

/// What the rail knows about an ACP agent's model.
#[derive(Clone, Debug, Default)]
pub(crate) struct AgentFacts {
    /// The person's pick in /models.
    pub choice: Option<String>,
    /// Whether the agent's own model list was read this run.
    pub listed: bool,
    /// The default Coder runs it on ([`crate::agent_models::default_model`]).
    pub default: Option<String>,
    /// The model its last session reported, from the harness facts.
    pub cached: Option<String>,
}

/// [`agent_rail_label`] from its facts; `display` names a model.
pub(crate) fn agent_rail_from(
    id: &str,
    name: &str,
    facts: &AgentFacts,
    display: impl Fn(&str) -> String,
) -> String {
    use crate::agent_models::{self as agents, is_clef};
    let usable = |model: &Option<String>| {
        model
            .clone()
            .filter(|model| !model.is_empty() && !is_clef(model))
    };
    let cached = usable(&facts.cached);
    let model = usable(&facts.choice).or_else(|| {
        if agents::lists_models(id) {
            // The list's default, once read. While it is read the rail
            // names no model: a cached one may be stale (owner,
            // 2026-10-13: the rail said Grok 4.6, then 4.7 ran).
            usable(&facts.default)
        } else {
            cached.or_else(|| usable(&facts.default))
        }
    });
    match model {
        Some(model) => format!("{name} ({})", display(&model)),
        None if id == agents::OMP && facts.listed => format!("{name} (no chat model)"),
        None => name.to_owned(),
    }
}

/// A harness's name and, in parentheses, the model its next turn runs:
/// "Claude (Haiku 5.5)", "Coder (auto)", "Codex (GPT-6.1-Sol)".
fn harness_label(app: &App, harness: crate::harness_plugin::Harness) -> String {
    use crate::harness_plugin::Harness;
    // A pin's model is the chat's harness's, not another one's.
    let pinned_model = app
        .live
        .pinned_model
        .clone()
        .filter(|_| app.live.runs_on == Some(harness));
    match harness {
        Harness::ClaudeCode => claude_rail_label(),
        Harness::CoderLoop => match coder_rail_model(app) {
            Some(model) => format!("{CODER_RAIL} ({model})"),
            // A model not known yet is left out until it is (#11537).
            None => CODER_RAIL.into(),
        },
        // "Codex (GPT-6.1-Sol)": the model the next turn runs (#11510).
        Harness::Codex => format!(
            "Codex ({})",
            crate::codex::name(&crate::codex::turn_model(pinned_model))
        ),
        // The model the chat's OpenCode session runs, named (#11507).
        Harness::OpenCode => {
            let model = app.live.opencode_session.as_ref().map_or_else(
                || crate::opencode_chat::model(pinned_model),
                |session| session.model.clone(),
            );
            crate::opencode_chat::rail_label(&model)
        }
    }
}

/// The model the Coder loop's next turn runs, as the rail names it: the
/// person's choice in /models, else (on auto) the gateway model picked
/// while signed in (#11526), else the model the chat's latest turn ran
/// (#11519), else `None` until one is known (#11537).
fn coder_rail_model(app: &App) -> Option<String> {
    if let Some(chosen) = coder_chosen_model(app) {
        return Some(chosen);
    }
    if app.account.is_some()
        && let Some(picked) = &app.gateway_model
        && !inference::retired::is_retired(picked)
    {
        return Some(rail_model_name(picked));
    }
    // A no-harness turn's one model (#11548): "Coder (Gemini 4 Argon)".
    if let Some(model) = &app.no_harness_model {
        return Some(rail_model_name(model));
    }
    served_model(app).map(|model| rail_model_name(&model))
}

/// The model the person chose for the Coder loop in /models (OpenRouter
/// BYOK), by its name: "Claude Fable 5.1" (#11556).
fn coder_chosen_model(app: &App) -> Option<String> {
    let keyed = app.plugins.enabled && app.plugins.key_configured;
    let base = app.plugins.model.split(':').next().unwrap_or_default();
    (keyed && crate::models::pinned(base)).then(|| {
        crate::models::openrouter_catalog()
            .into_iter()
            .find(|model| model.id == base)
            .map_or_else(|| rail_model_name(base), |model| model.name)
    })
}

/// Whether the person chose the Coder loop's model in /models: a pick
/// pins it, so the rail drops "Auto" (#11556).
fn coder_model_picked(app: &App) -> bool {
    coder_chosen_model(app).is_some()
        || (app.account.is_some()
            && app
                .gateway_model
                .as_deref()
                .is_some_and(crate::models::pinned))
}

/// The model the chat's latest Coder loop turn ran, as the transcript
/// knows it: the one answering now while a turn streams, else the latest
/// reply's.
fn served_model(app: &App) -> Option<String> {
    let usable = |model: &str| {
        !model.is_empty() && crate::models::pinned(model) && !inference::retired::is_retired(model)
    };
    app.live
        .partial_model
        .clone()
        .filter(|model| app.live.busy && usable(model))
        .or_else(|| {
            app.live.entries.iter().rev().find_map(|entry| match entry {
                crate::live::Entry::Assistant {
                    model: Some(model), ..
                } if usable(model) => Some(model.clone()),
                _ => None,
            })
        })
}

/// A subscription plan that pays for a turn, whose windows the rail shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlanPaying {
    Claude,
    Codex,
}

/// The plan that pays for the turns of `harness` in this chat (#11537):
/// Claude Code's are the Claude plan's and Codex's the Codex plan's; the
/// Coder loop's are a plan's when its latest turn ran on this computer's
/// Claude Code or Codex login.
fn plan_paying(app: &App, harness: crate::harness_plugin::Harness) -> Option<PlanPaying> {
    use crate::harness_plugin::Harness;
    match harness {
        Harness::ClaudeCode => Some(PlanPaying::Claude),
        Harness::Codex => Some(PlanPaying::Codex),
        Harness::OpenCode => None,
        Harness::CoderLoop => {
            let model = served_model(app)?;
            if model.starts_with("codex:") {
                Some(PlanPaying::Codex)
            } else if model.starts_with("claude-") && !model.contains('/') {
                Some(PlanPaying::Claude)
            } else {
                None
            }
        }
    }
}

/// "OpenCode (Cat Jam)": the model the chat's OpenCode session
/// last ran, else the one its next turn runs.
pub(crate) fn opencode_rail_label(app: &App) -> String {
    let model = app.live.opencode_session.as_ref().map_or_else(
        || crate::opencode_chat::model(app.live.pinned_model.clone()),
        |session| session.model.clone(),
    );
    crate::opencode_chat::rail_label(&model)
}

/// What the rail says when the person's own OpenRouter key paid the
/// Coder loop's turn (#11550).
pub(crate) const OPENROUTER_PAYER: &str = "your OpenRouter key";

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
        // The same words the chat's account copy shows ([`atif::row`]).
        let params = truncate(&atif::row::summary(input, output), room);
        if !params.is_empty() {
            spans.push(span(" ", t::GRAY_DIM));
            spans.push(span(
                params,
                crate::tools::input_color(*running, crate::tools::output_failed(output)),
            ));
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
    /// The model of the message last clicked, shown at its row's right end
    /// for a moment (`model_tip`). Display only.
    pub(crate) model_tip: Option<model_tip::ModelTip>,
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
    /// Where the entry sits in a finished run of steps the compact view
    /// shows as one line (#11638).
    fold: Option<Fold>,
}

/// A finished run of a turn's steps, drawn in the compact view as one line
/// that Ctrl+O opens, as the web's trace sits under one "Worked" line
/// (#11638). An edit keeps its diff under that line.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Fold {
    /// The run's first step: the line, with how many steps there were and
    /// how many failed.
    Head { steps: usize, failed: usize },
    /// A later step of the run.
    Member,
}

/// A tool call the run's line stands for: not a card (thinking, a plan,
/// a notice) the person needs to read.
fn is_step(entry: &crate::live::Entry) -> bool {
    matches!(entry, crate::live::Entry::Tool { name, .. } if !is_card(name))
}

/// Each entry's place in a folded run. A run folds once it has two or
/// more steps, none still running, and something came after it: a reply,
/// the person's next message, or a reply that is streaming.
fn folds(entries: &[crate::live::Entry], streaming: bool) -> Vec<Option<Fold>> {
    let mut out = vec![None; entries.len()];
    let mut start = 0;
    while start < entries.len() {
        if !is_step(&entries[start]) {
            start += 1;
            continue;
        }
        let end = entries[start..]
            .iter()
            .position(|entry| !is_step(entry))
            .map_or(entries.len(), |at| start + at);
        let run = &entries[start..end];
        let settled = run
            .iter()
            .all(|entry| !matches!(entry, crate::live::Entry::Tool { running: true, .. }));
        if run.len() >= 2 && settled && (end < entries.len() || streaming) {
            let failed = run
                .iter()
                .filter(|entry| {
                    matches!(entry, crate::live::Entry::Tool { output, .. }
                        if crate::tools::output_failed(output))
                })
                .count();
            out[start] = Some(Fold::Head {
                steps: run.len(),
                failed,
            });
            for slot in &mut out[start + 1..end] {
                *slot = Some(Fold::Member);
            }
        }
        start = end;
    }
    out
}

/// The rows a folded step shows: the run's one line on its first step,
/// and an edit's diff, which stays readable without opening the run.
fn fold_rows(
    entry: &crate::live::Entry,
    fold: &Fold,
    (lines, copy): (Vec<Line<'static>>, Vec<RowCopy>),
    width: u16,
) -> (Vec<Line<'static>>, Vec<RowCopy>) {
    let edit = matches!(entry, crate::live::Entry::Tool { name, .. }
        if matches!(name.as_str(), "Edit" | "Write"));
    let (mut rows, mut rows_copy) = if edit {
        (lines, copy)
    } else {
        (Vec::new(), Vec::new())
    };
    if let Fold::Head { steps, failed } = fold {
        // The turn went on past a failed step; the words say it failed.
        let (glyph, color) = crate::tools::outcome_header(false, false, 0);
        let mut spans = vec![
            span(format!("{glyph} "), color),
            Span::styled(
                "Worked",
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
        ];
        let mut said = format!(" · {steps} steps");
        match failed {
            0 => {}
            1 => said.push_str(" · 1 didn't work"),
            n => said.push_str(&format!(" · {n} didn't work")),
        }
        let room = width.saturating_sub(8);
        let said = truncate(&said, room);
        let hint = " · Ctrl+O opens them";
        let fits = said.width() + hint.width() <= usize::from(room);
        spans.push(span(said, t::TEXT_SECONDARY));
        if fits {
            spans.push(span(hint, t::GRAY_DIM));
        }
        rows.insert(0, Line::from(spans));
        rows_copy.insert(0, RowCopy::lead(2));
    }
    (rows, rows_copy)
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
            self.model_tip = None;
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
                    fold: None,
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
                cached.fold = None;
                cached.bound_run(width, phase, offset);
            }
        }
        let streaming = !chat.partial.is_empty();
        // A finished run of steps reads as one line in the compact view
        // (#11638); Ctrl+O's verbose view shows every step.
        let wanted = if view == ViewMode::Compact {
            folds(&chat.entries, streaming)
        } else {
            vec![None; self.entries.len()]
        };
        for (cached, fold) in self.entries.iter_mut().zip(wanted) {
            if cached.fold != fold {
                let rows = entry_rows(&cached.entry, width, phase, view);
                (cached.lines, cached.copy) = match &fold {
                    Some(fold) => fold_rows(&cached.entry, fold, rows, width),
                    None => rows,
                };
                cached.gap_after = false;
                cached.fold = fold;
                cached.bound_run(width, phase, None);
            }
        }
        // A compact tool row carries no trailing blank so calls run together;
        // an item under it that is a reply takes the one blank row every
        // item has above it. A row that already ends in a blank (a verbose
        // item) takes none, so a reply never shows two blanks above it.
        // `gap_after` means the entry's last row is a gap this loop pushed:
        // `ends_blank` looks past it, or the gap reads as the entry's own
        // blank and is pushed and popped on alternating draws (#11395).
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
                    fold: None,
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

    /// A plain click on transcript row `row`: show the model of the
    /// message there, or put the label down when it has none yet.
    pub(crate) fn show_model(&mut self, row: usize, now: std::time::Instant) {
        let entries: Vec<&crate::live::Entry> = self.blocks().map(|cached| &cached.entry).collect();
        let mut index = row;
        let mut at = None;
        for (number, cached) in self.blocks().enumerate() {
            if index < cached.lines.len() {
                at = Some(number);
                break;
            }
            index -= cached.lines.len();
        }
        self.model_tip = at
            .and_then(|at| model_tip::turn_model(&entries, at))
            .map(|model| model_tip::ModelTip {
                row,
                label: model_tip::label(model),
                shown_at: now,
            });
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
    if let Some(tip) = &cache.model_tip {
        model_tip::paint(
            frame.buffer_mut(),
            area,
            usize::from(position),
            tip,
            std::time::Instant::now(),
        );
    }
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
        assert!(text.contains("Coder") && !text.contains("(auto)"));
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
        // No placeholder model before one is known (#11537).
        assert!(!rows.iter().any(|row| row.contains("(auto)")));
        let label = "Coder";
        let n = label.chars().count() as u16;
        for y in 0..20 {
            // The input's rule, not the startup banner's name.
            if buffer[(0, y)].symbol() != "┌" {
                continue;
            }
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
        assert!(
            shown.contains("Coder") && !shown.contains("(auto)"),
            "{shown}"
        );
        assert!(!shown.contains("openrouter") && !shown.contains("claude"));
        // A model chosen in /models is named after the loop.
        app.plugins.model = crate::models::OPENROUTER.into();
        assert!(rail(&mut app).contains("Coder (DeepSeek V4.1 Flash)"));
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
        // No call has reported its tokens yet: no guessed figure (#11537).
        assert!(!shown.contains("Context"), "{shown}");
        // #11526: a gateway model picked in /models, while signed in.
        app.gateway_model = Some("zai/glm-5.3-flash".into());
        assert!(rail(&mut app).contains("Coder (Gemini 3.8 Flash)"));
        app.account = Some("Fixture".into());
        let shown = rail(&mut app);
        assert!(shown.contains("Coder (GLM 5.3 Flash)"), "{shown}");
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
        // Coder's own loop, its model not known yet: no placeholder and
        // no Claude plan (#11537).
        app.live.runs_on = Some(Harness::CoderLoop);
        let shown = rail(&mut app);
        assert!(
            shown.contains("Auto → Coder") && !shown.contains("(auto)") && !shown.contains("5h "),
            "{shown}"
        );
        // The Coder loop answered through a gateway model: still none.
        app.live.entries.push(crate::live::Entry::Assistant {
            elapsed_ms: None,
            text: "I'm Coder.".into(),
            model: Some("google/gemini-2.5-flash-lite".into()),
        });
        let shown = rail(&mut app);
        assert!(
            shown.contains("Coder (Gemini 2.5 Flash Lite)") && !shown.contains("5h "),
            "{shown}"
        );
        // The Claude plan paid the Coder loop's turn: its windows follow
        // the label, named as Claude's (#11537).
        app.live.entries.push(crate::live::Entry::Assistant {
            elapsed_ms: None,
            text: "I'm Coder.".into(),
            model: Some(crate::claude::MODEL.into()),
        });
        let shown = rail(&mut app);
        let expected = format!("Coder (Haiku 5.5) · Claude {plan}");
        assert!(shown.contains(&expected), "{shown}");
        app.live.entries.clear();
        // Claude runs the chat: its windows follow its name.
        app.live.runs_on = Some(Harness::ClaudeCode);
        let shown = rail(&mut app);
        let expected = format!("{} · {plan}", claude_rail_label());
        // "Auto picked Claude" (#11537).
        assert!(shown.contains(&format!("Auto → {expected}")), "{shown}");
        assert!(!repository_rail(&app).contains("5h "));
        // Pinned, the chat names its harness alone, with no Auto.
        app.live.pinned = true;
        let shown = rail(&mut app);
        assert!(
            shown.contains(&expected) && !shown.contains("Auto"),
            "{shown}"
        );
        app.live.pinned = false;
        crate::claude_plan::reset_for_test(1);
    }

    /// The plan that pays the Coder loop's turn is read off the model it
    /// ran (#11537): the Codex login's `codex:` models are the Codex
    /// plan's, Claude's bare ids the Claude plan's, a gateway or
    /// OpenRouter model none.
    #[test]
    fn the_coder_loops_plan_is_the_login_that_answered() {
        use crate::harness_plugin::Harness;
        let mut app = App::default();
        app.mode = Mode::Live;
        app.live.runs_on = Some(Harness::CoderLoop);
        let ran = |app: &mut App, model: &str| {
            app.live.entries = vec![crate::live::Entry::Assistant {
                elapsed_ms: None,
                text: "hi".into(),
                model: Some(model.into()),
            }];
            plan_paying(app, Harness::CoderLoop)
        };
        assert_eq!(plan_paying(&app, Harness::CoderLoop), None);
        assert_eq!(ran(&mut app, "codex:gpt-6-luna"), Some(PlanPaying::Codex));
        assert_eq!(ran(&mut app, "claude-opus-5-5"), Some(PlanPaying::Claude));
        assert_eq!(ran(&mut app, "anthropic/claude-fable-5.1"), None);
        assert_eq!(ran(&mut app, "google/gemini-2.5-flash-lite"), None);
        assert_eq!(plan_paying(&app, Harness::Codex), Some(PlanPaying::Codex));
        assert_eq!(rail_model_name("codex:gpt-6-luna"), "GPT-6-Luna");
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

    /// A finished turn's steps sit under one "Worked" line in the compact
    /// view, which says how many failed and keeps an edit's diff; Ctrl+O
    /// opens every step, and a run still working is never folded (#11638).
    #[test]
    fn a_finished_turns_steps_fold_under_one_line_that_ctrl_o_opens() {
        use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
        let diff = crate::file_tools::unified(
            "fn total() -> u32 {\n    1 + 1\n}\n",
            "fn total() -> u32 {\n    2 + 2\n}\n",
        );
        let mut app = App::default();
        app.mode = Mode::Live;
        app.live.entries = vec![
            crate::live::Entry::User("Fix the total in src/sum.rs".into()),
            crate::live::Entry::Tool {
                name: crate::claude_stream::THINKING.into(),
                input: serde_json::Value::Null,
                output: serde_json::json!({"text": "The total is computed in src/sum.rs."}),
                running: false,
            },
            crate::live::Entry::Tool {
                name: "Read".into(),
                input: serde_json::json!({"path": "src/sum.rs"}),
                output: serde_json::json!({"content": "fn total() -> u32 {\n    1 + 1\n}\n"}),
                running: false,
            },
            crate::live::Entry::Tool {
                name: "Run".into(),
                input: serde_json::json!({"command": "cargo test sum"}),
                output: serde_json::json!({"stdout": "test sum ... FAILED", "exit_code": 101, "error": "1 test failed"}),
                running: false,
            },
            crate::live::Entry::Tool {
                name: "Edit".into(),
                input: serde_json::json!({"path": "src/sum.rs"}),
                output: serde_json::json!({
                    "path": "src/sum.rs", "diff": diff.text, "added": diff.added, "removed": diff.removed,
                }),
                running: false,
            },
            crate::live::Entry::Tool {
                name: "Run".into(),
                input: serde_json::json!({"command": "cargo test sum"}),
                output: serde_json::json!({"stdout": "test sum ... ok"}),
                running: false,
            },
            crate::live::Entry::Assistant {
                text: "Fixed: `total` now returns 4, and the test passes.".into(),
                model: None,
                elapsed_ms: None,
            },
        ];
        let screen = |app: &mut App| {
            let mut terminal = Terminal::new(TestBackend::new(80, 30)).unwrap();
            terminal.draw(|frame| render(frame, app)).unwrap();
            let buffer = terminal.backend().buffer();
            (0..30)
                .map(|y| {
                    (0..80)
                        .map(|x| buffer[(x, y)].symbol())
                        .collect::<String>()
                        .trim_end()
                        .to_owned()
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        let folded = screen(&mut app);
        assert!(
            folded.contains("● Worked · 4 steps · 1 didn't work · Ctrl+O opens them"),
            "{folded}"
        );
        assert!(!folded.contains("cargo test sum"), "{folded}");
        assert!(!folded.contains("Read src/sum.rs"), "{folded}");
        // The edit's diff stays, as does the thinking and the answer.
        assert!(folded.contains("2 + 2"), "{folded}");
        assert!(folded.contains("The total is computed"), "{folded}");
        assert!(folded.contains("Fixed: total now returns 4"), "{folded}");
        assert_eq!(folded.matches("Worked").count(), 1, "{folded}");
        app.handle(Event::Key(KeyEvent::new(
            KeyCode::Char('o'),
            KeyModifiers::CONTROL,
        )));
        assert_eq!(app.view, ViewMode::Verbose);
        let opened = screen(&mut app);
        assert!(!opened.contains("Worked"), "{opened}");
        assert!(opened.contains("cargo test sum"), "{opened}");
        assert!(opened.contains("1 test failed"), "{opened}");
        app.handle(Event::Key(KeyEvent::new(
            KeyCode::Char('o'),
            KeyModifiers::CONTROL,
        )));
        // Still working: the steps show one by one.
        app.live.entries.pop();
        if let Some(crate::live::Entry::Tool { running, .. }) = app.live.entries.last_mut() {
            *running = true;
        }
        let working = screen(&mut app);
        assert!(!working.contains("Worked"), "{working}");
        assert!(working.contains("cargo test sum"), "{working}");
        if let Ok(directory) = std::env::var("OPENAGENTS_TRANSCRIPT_CAPTURE") {
            for (name, text) in [
                ("folded", &folded),
                ("opened", &opened),
                ("working", &working),
            ] {
                std::fs::write(
                    std::path::Path::new(&directory)
                        .join(format!("transcript-terminal-{name}.txt")),
                    text,
                )
                .unwrap();
            }
        }
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

    /// The agent rail alone, drawn `width` x `height`, one string per row.
    fn rail(app: &crate::App, width: u16, height: u16) -> Vec<String> {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| super::agent_rail(frame, frame.area(), app))
            .unwrap();
        let buffer = terminal.backend().buffer();
        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect()
    }

    fn agent(name: &str, task: &str, tokens: u64, started_at: u64) -> crate::live::Delegation {
        crate::live::Delegation {
            id: name.into(),
            name: name.into(),
            task: task.into(),
            chat: Chat {
                tokens,
                ..Chat::default()
            },
            started_at,
            elapsed_seconds: 0,
            running: true,
            draft: Default::default(),
            composer: Default::default(),
            scroll: 0,
            background: false,
        }
    }

    /// Five agents under the main chat, as fakes: no harness runs.
    fn five_agents() -> crate::App {
        let mut app = crate::App::default();
        app.mode = crate::Mode::Live;
        app.elapsed_seconds = 20_000;
        for (name, task, tokens, started) in [
            (
                "general-purpose",
                "Waiting on film_shot build",
                752_100,
                2_800,
            ),
            (
                "general-purpose",
                "Reading comb in draconia_hair.py",
                732_000,
                3_772,
            ),
            ("Explore", "Reading Argon routing records", 416_300, 10_986),
            ("general-purpose", "Closing issue #11556", 339_400, 13_626),
            ("Plan", "Drafting the plan", 1_200, 19_990),
        ] {
            app.delegations.push(agent(name, task, tokens, started));
        }
        app
    }

    #[test]
    fn the_rail_lists_main_then_agents_with_dots_a_cursor_and_right_aligned_figures() {
        let mut app = five_agents();
        app.selected_agent = Some(1);
        let rows = rail(&app, 100, 8);
        let shown = rows.join("\n");
        assert_eq!(rows[0], super::RAIL_HINT, "{shown}");
        assert!(rows[1].starts_with("  ○ main"), "{shown}");
        // The selected row: the cursor left of a filled dot; the rest an
        // empty dot behind a two-space gutter.
        assert!(rows[3].starts_with("❯ ● general-purpose"), "{shown}");
        for row in [&rows[1], &rows[2], &rows[4], &rows[5], &rows[6]] {
            assert!(row.starts_with("  ○ "), "{shown}");
        }
        assert_eq!(shown.matches('●').count(), 1, "{shown}");
        assert_eq!(shown.matches('❯').count(), 1, "{shown}");
        // Activity starts in one column on every row.
        let column = |row: &str, text: &str| row[..row.find(text).unwrap()].chars().count();
        let activity = column(&rows[2], "Waiting on");
        assert_eq!(column(&rows[3], "Reading comb"), activity);
        assert_eq!(column(&rows[4], "Reading Argon"), activity);
        assert_eq!(column(&rows[6], "Drafting"), activity);
        // Time, then tokens, flush right, their dots and arrows aligned.
        assert!(rows[2].ends_with("4h 46m 40s · ↓ 752.1k tokens"), "{shown}");
        assert!(rows[3].ends_with("4h 30m 28s · ↓ 732.0k tokens"), "{shown}");
        assert!(rows[6].ends_with("10s · ↓   1.2k tokens"), "{shown}");
        for row in &rows[2..7] {
            assert_eq!(row.chars().count(), 100, "{shown}");
            assert_eq!(column(row, "↓"), column(&rows[2], "↓"), "{shown}");
        }
        assert!(!shown.contains("more"), "{shown}");
    }

    #[test]
    fn a_long_rail_says_how_many_more_and_scrolls_with_the_selection() {
        let mut app = five_agents();
        // The hint, three rows, and the overflow line.
        let rows = rail(&app, 100, 5);
        let shown = rows.join("\n");
        assert!(rows[1].starts_with("❯ ● main"), "{shown}");
        assert_eq!(rows[4], "  ↓ 3 more", "{shown}");
        // Down walks the selection onto the agents; the window follows.
        app.composer_arrow(false);
        assert_eq!(app.selected_agent, Some(0));
        app.composer_arrow(false);
        app.composer_arrow(false);
        app.composer_arrow(false);
        assert_eq!(app.selected_agent, Some(3));
        let rows = rail(&app, 100, 5);
        let shown = rows.join("\n");
        assert!(rows[3].starts_with("❯ ● general-purpose"), "{shown}");
        assert!(rows[3].contains("Closing issue #11556"), "{shown}");
        assert!(!shown.contains("○ main"), "{shown}");
        assert_eq!(rows[4], "  ↓ 1 more", "{shown}");
        app.composer_arrow(false);
        let rows = rail(&app, 100, 5);
        assert!(rows[3].starts_with("❯ ● Plan"), "{}", rows.join("\n"));
        assert!(!rows.join("\n").contains("more"));
        // Up walks back to the main chat.
        for _ in 0..5 {
            app.composer_arrow(true);
        }
        assert_eq!(app.selected_agent, None);
        let rows = rail(&app, 100, 5);
        assert!(rows[1].starts_with("❯ ● main"), "{}", rows.join("\n"));
        assert_eq!(rows[4], "  ↓ 3 more");
    }

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
        // An agent that opens a call with only its folder shows the tool
        // by name, never the folder.
        chat.entries.push(Entry::Tool {
            name: "bash".into(),
            input: serde_json::json!({"cwd": "/Users/someone/code/slugkit"}),
            output: serde_json::Value::Null,
            running: true,
        });
        assert_eq!(super::agent_activity(&chat).as_deref(), Some("Bash"));
        // OpenCode's read names its file as filePath.
        chat.entries.push(Entry::Tool {
            name: "read".into(),
            input: serde_json::json!({"filePath": "src/slugify.js"}),
            output: serde_json::Value::Null,
            running: true,
        });
        assert_eq!(
            super::agent_activity(&chat).as_deref(),
            Some("Read · src/slugify.js")
        );
        chat.partial = "Writing the restore step\nnow".into();
        assert_eq!(super::agent_activity(&chat).as_deref(), Some("now"));
    }

    /// #11575: a reply in Markdown previews as its words, never a bare
    /// code fence, and a notice (OpenCode's free-model retry) shows as what
    /// just happened.
    #[test]
    fn an_agent_preview_is_words_not_a_code_fence() {
        let mut chat = Chat {
            entries: vec![Entry::User("Read Cargo.toml".into())],
            ..Chat::default()
        };
        chat.entries.push(Entry::Assistant {
            text: "```\n## **Edition** is `2024`\n- rust 1.97\n```".into(),
            model: None,
            elapsed_ms: None,
        });
        assert_eq!(
            super::agent_activity(&chat).as_deref(),
            Some("Edition is 2024")
        );
        chat.partial = "The members are:\n```toml".into();
        assert_eq!(
            super::agent_activity(&chat).as_deref(),
            Some("The members are:")
        );
        chat.partial = "```".into();
        assert_eq!(
            super::agent_activity(&chat).as_deref(),
            Some("Edition is 2024")
        );
        chat.partial.clear();
        chat.entries.push(Entry::Tool {
            name: crate::claude_stream::NOTICE.into(),
            input: serde_json::Value::Null,
            output: serde_json::json!({"text": "OpenCode's cursor/composer-2.5 failed, so it ran on OpenCode's free Cat Jam. Trying again."}),
            running: false,
        });
        assert!(
            super::agent_activity(&chat)
                .unwrap()
                .contains("free Cat Jam")
        );
        assert_eq!(super::model_words("opencode/cat-jam-free"), "Cat Jam");
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
        assert!(top.contains("Auto → Coder · $0.25"), "{top}");
    }
}

/// #11543: a chat's account copy on openagents.com says what Coder's
/// terminal says about each step: the same tool row words, from the same
/// function ([`atif::row`]), and the model's thinking as its words.
#[cfg(test)]
mod account_copy_tests {
    use crate::live::{Chat, Entry};
    use serde_json::json;

    /// A compact tool row's words, without its outcome glyph.
    fn terminal_row(entry: &Entry) -> String {
        let (lines, _) = super::entry_rows(entry, 400, 0, super::ViewMode::Compact);
        let text: String = lines[0]
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect();
        text.split_once(' ')
            .map(|(_, rest)| rest.to_owned())
            .unwrap()
    }

    #[test]
    fn the_account_copy_shows_each_step_as_the_terminal_does() {
        let tools = vec![
            Entry::Tool {
                name: "OpenAgents CLI".into(),
                input: json!({"arguments": ["projects", "show", "finances"]}),
                output: json!({"exit": 0, "stdout": "{}"}),
                running: false,
            },
            Entry::Tool {
                name: "Read".into(),
                input: json!({"file_path": "src/main.rs", "limit": 40}),
                output: json!({"text": "fn main() {}"}),
                running: false,
            },
            Entry::Tool {
                name: "Bash".into(),
                input: json!({"command": "ls missing"}),
                output: json!({"error": "No such folder: missing\nmore"}),
                running: false,
            },
        ];
        let thinking = Entry::Tool {
            name: crate::claude_stream::THINKING.into(),
            input: serde_json::Value::Null,
            output: json!({"text": "I'll check the help first."}),
            running: false,
        };
        let mut entries = vec![Entry::User("Look at my projects.".into())];
        entries.push(thinking);
        entries.extend(tools.iter().cloned());
        let chat = Chat {
            entries,
            ..Chat::default()
        };
        let document = crate::trajectory::document(
            &chat,
            "20261013T145740Z-f35279d8",
            "auto",
            std::path::Path::new("/tmp"),
        );
        let sent = coder_sync::upload(&document, "Studio", &secret_screen::Screen::shapes());
        let rows: Vec<(String, String)> = sent.body["messages"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|m| m["role"] != "user")
            .map(|m| {
                (
                    m["role"].as_str().unwrap().to_owned(),
                    m["text"].as_str().unwrap().to_owned(),
                )
            })
            .collect();
        let mut expected = vec![(
            "thinking".to_owned(),
            "I'll check the help first.".to_owned(),
        )];
        expected.extend(tools.iter().map(|entry| {
            let Entry::Tool { name, .. } = entry else {
                unreachable!()
            };
            (
                "tool".to_owned(),
                format!(
                    "{name} {}",
                    terminal_row(entry)
                        .trim_start_matches(name.as_str())
                        .trim_start()
                ),
            )
        }));
        assert_eq!(rows, expected);
        assert_eq!(rows[1].1, "OpenAgents CLI projects show finances");
        assert_eq!(rows[3].1, "Bash No such folder: missing");
    }
}

#[cfg(test)]
mod agent_rail_label_tests {
    use super::{AgentFacts, agent_rail_from};

    fn words(model: &str) -> String {
        super::rail_model_name(model)
    }

    /// Grok Build's rail names the default `grok models` lists, not the
    /// model an older session cached in the harness facts (owner,
    /// 2026-10-13): Grok 4.7 runs, so the rail never says Grok 4.6.
    #[test]
    fn grok_names_its_listed_default_over_a_cached_model() {
        let facts = AgentFacts {
            listed: true,
            default: Some("grok-4.7".into()),
            cached: Some("grok-4.6".into()),
            ..AgentFacts::default()
        };
        assert_eq!(
            agent_rail_from("grok-build", "Grok Build", &facts, words),
            "Grok Build (Grok 4.7)"
        );
        // Before `grok models` is read, no model rather than a stale one.
        let reading = AgentFacts {
            listed: false,
            default: None,
            ..facts.clone()
        };
        assert_eq!(
            agent_rail_from("grok-build", "Grok Build", &reading, words),
            "Grok Build"
        );
        // A pick in /models wins.
        let picked = AgentFacts {
            choice: Some("grok-4.5".into()),
            ..facts
        };
        assert_eq!(
            agent_rail_from("grok-build", "Grok Build", &picked, words),
            "Grok Build (Grok 4.5)"
        );
    }

    /// Oh My Pi with only Clef models never names one: it reads "(no chat
    /// model)" once its list is read.
    #[test]
    fn oh_my_pi_without_a_chat_model_never_names_clef() {
        let facts = AgentFacts {
            listed: true,
            default: None,
            cached: Some("ollama/clef:latest".into()),
            choice: Some("clef-flash:latest".into()),
        };
        let rail = agent_rail_from("oh-my-pi", "Oh My Pi", &facts, words);
        assert_eq!(rail, "Oh My Pi (no chat model)");
        assert!(!rail.to_lowercase().contains("clef"));
        let reading = AgentFacts {
            listed: false,
            ..facts
        };
        assert_eq!(
            agent_rail_from("oh-my-pi", "Oh My Pi", &reading, words),
            "Oh My Pi"
        );
    }

    /// Devin's rail names the model its next turn asks for, the pick else
    /// Coder's SWE-2 default, never the account default an earlier
    /// session fell back to (#11583).
    #[test]
    fn devin_names_the_model_its_next_turn_asks_for() {
        let facts = AgentFacts {
            listed: true,
            default: Some("swe-2-medium".into()),
            cached: Some("swe-1-6-slow".into()),
            ..AgentFacts::default()
        };
        let display = |model: &str| crate::agent_models::display("devin-cli", model);
        assert_eq!(
            agent_rail_from("devin-cli", "Devin CLI", &facts, display),
            "Devin CLI (SWE-2 Medium)"
        );
        let picked = AgentFacts {
            choice: Some("swe-2-high".into()),
            ..facts
        };
        assert_eq!(
            agent_rail_from("devin-cli", "Devin CLI", &picked, display),
            "Devin CLI (SWE-2 High)"
        );
    }
}
