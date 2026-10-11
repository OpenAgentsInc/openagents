//! The same render function draws the terminal and exported previews.

pub(crate) mod agents;
mod appearance;
mod models;
mod plugins;
mod resume;

use coder_terminal::{Colors, Ladder, components::turn::markdown_body};
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
    App, Mode, Screen,
    agents::{DEMOS, DemoMessage, MAIN_PLUGINS, MAIN_TOOLS, elapsed_time},
    plugin_definition::RailSlot,
    theme as t,
    tools::{delegation_lines, plugin_lines, tool_lines},
};

fn span(text: impl Into<String>, color: Color) -> Span<'static> {
    Span::styled(text.into(), Style::default().fg(color))
}

pub fn render(frame: &mut Frame, app: &mut App) {
    render_contents(frame, app);
    if app.appearance.use_system_terminal_background {
        for cell in &mut frame.buffer_mut().content {
            cell.bg = Color::Reset;
        }
    }
}

fn render_contents(frame: &mut Frame, app: &mut App) {
    if app.screen == Screen::Appearance {
        appearance::render(frame, frame.area(), app);
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
        app.delegations.len()
    })
    .min(usize::from(area.height.saturating_sub(6))) as u16;
    let composer_height = (draft.len() as u16).clamp(1, 6) + 2;
    let header_height = u16::from(app.selected_agent.is_some());
    let context_text = context_text(app);
    let context_height = u16::from(!context_text.is_empty());
    let reserved = rail_height + 1 + context_height + header_height;
    let [header, body, queued, composer, context, rail] = Layout::vertical([
        Constraint::Length(header_height),
        Constraint::Min(1),
        Constraint::Length(app.queued_prompts.len().min(3) as u16),
        Constraint::Length(composer_height.min(area.height.saturating_sub(reserved))),
        Constraint::Length(context_height),
        Constraint::Length(rail_height),
    ])
    .areas(area);
    header_view(frame, header, app);
    let pending = app
        .queued_prompts
        .iter()
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
        Screen::Plugins | Screen::PluginSettings | Screen::Appearance => unreachable!(),
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
    composer_view(
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
        rail_mode(app),
    );
    let path = repository_rail(app);
    let composer_area = Rect {
        x: terminal_x,
        width: terminal_width,
        ..composer
    };
    let path = truncate(&path, terminal_width.saturating_sub(6));
    coder_terminal::rail(
        composer_area,
        frame.buffer_mut(),
        composer.height.saturating_sub(1),
        None,
        Some((&path, Style::default().fg(t::GRAY))),
    );
    frame.render_widget(
        Paragraph::new(span(truncate(&context_text, context.width), t::GRAY)),
        context,
    );
    agent_rail(frame, rail, app);
    if app.model_picker.is_some() {
        models::render(frame, app);
    }
}

fn agent_rail(frame: &mut Frame, area: Rect, app: &App) {
    let agents: Vec<_> = if app.mode == Mode::Demo {
        DEMOS
            .iter()
            .map(|agent| {
                (
                    agent.name,
                    std::borrow::Cow::Borrowed(agent.task),
                    agent.tokens.to_owned(),
                    agent.elapsed_seconds.saturating_add(app.elapsed_seconds),
                )
            })
            .collect()
    } else {
        app.delegations
            .iter()
            .map(|agent| {
                if agent.background
                    && let Some(row) = app.fleet.get(&agent.id)
                {
                    let mut task = format!("{} · ", row.status.word());
                    if let Some(cost) = row.cost_usd {
                        task.push_str(&agent_fleet::dollars(cost));
                        task.push_str(" · ");
                    }
                    task.push_str(&agent.task);
                    return (
                        agent.name.as_str(),
                        std::borrow::Cow::Owned(task),
                        if row.tokens == 0 {
                            "—".into()
                        } else {
                            token_count(row.tokens)
                        },
                        row.elapsed_seconds(agent_fleet::now_ms()),
                    );
                }
                (
                    agent.name.as_str(),
                    // A finished delegation's dollars (#11179).
                    if agent.chat.cost_usd > 0.0 {
                        std::borrow::Cow::Owned(format!(
                            "{} · {}",
                            agent_fleet::dollars(agent.chat.cost_usd),
                            agent.task
                        ))
                    } else {
                        std::borrow::Cow::Borrowed(agent.task.as_str())
                    },
                    if agent.chat.tokens == 0 {
                        "—".into()
                    } else {
                        token_count(agent.chat.tokens)
                    },
                    if agent.running {
                        app.elapsed_seconds.saturating_sub(agent.started_at)
                    } else {
                        agent.elapsed_seconds
                    },
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
            if narrow {
                format!("{:>count_width$}↓", agent.2)
            } else {
                format!("{:>count_width$} tokens ↓", agent.2)
            }
        })
        .collect();
    let timed_labels: Vec<_> = agents
        .iter()
        .zip(&token_labels)
        .map(|(agent, tokens)| format!("{} · {tokens}", elapsed_time(agent.3)))
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
        let active = app.selected_agent == Some(index);
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
        let prefix = if active {
            "❯ "
        } else if narrow {
            "  "
        } else {
            "○ "
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

fn header_view(frame: &mut Frame, area: Rect, app: &App) {
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
            .and_then(|agent| {
                agent.chat.partial_model.as_deref().or_else(|| {
                    agent
                        .chat
                        .entries
                        .iter()
                        .rev()
                        .find_map(|entry| match entry {
                            crate::live::Entry::Assistant {
                                model: Some(model), ..
                            } => Some(model.as_str()),
                            _ => None,
                        })
                })
            });
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
    let cost = crate::long_session::session_cost(app);
    if cost > 0.0 {
        text.push_str(&format!(" · {}", agent_fleet::dollars(cost)));
    }
    text
}

/// The mode the input's rail names: `flash` (Tab), else `auto` unless the
/// person picked a model of their own.
fn rail_mode(app: &App) -> Option<&'static str> {
    if app.mode != Mode::Live {
        return None;
    }
    let keyed = app.plugins.enabled && app.plugins.key_configured;
    if keyed && app.plugins.model == crate::models::FLASH {
        Some(crate::models::FLASH_LABEL)
    } else if !keyed || !crate::models::pinned(&app.plugins.model) {
        Some(crate::models::AUTO)
    } else {
        None
    }
}

fn context_text(app: &App) -> String {
    let mut parts = Vec::new();
    if let Some(until) = crate::long_session::paused_until(app) {
        parts.push(format!(
            "Paused until {}",
            crate::long_session::clock(until)
        ));
    }
    // The signed-in account's name is never shown under the input.
    if let Some(update) = &app.update_line {
        parts.push(update.clone());
    }
    parts.join(" · ")
}

fn message_body(text: &str, width: u16) -> Vec<Line<'static>> {
    t::noir_lines(markdown_body(text, width, Ladder::new(Colors::True)))
}

/// The columns the transcript keeps clear at its right edge. Its rows are
/// laid out `GUTTER` narrower than the screen; only the person's band
/// reaches the edge.
const GUTTER: u16 = 2;

/// The person's message as Claude Code shows it: `❯` in column 0, wrapped
/// rows hanging under the text, on a band that runs through the gutter to
/// the screen's edge.
fn prompt(text: &str, width: u16) -> Vec<Line<'static>> {
    let mut rows = message_body(text, width.saturating_sub(2));
    if rows.is_empty() {
        rows.push(Line::default());
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
        row.style = row.style.bg(t::BG_LIGHT);
    }
    rows
}

/// An assistant reply as Claude Code shows it: `●` in column 0, then the
/// text, every further row indented two columns to hang under it.
fn bulleted(text: &str, width: u16) -> Vec<Line<'static>> {
    let mut rows = message_body(text, width.saturating_sub(2));
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
    rows
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
    if app.mode == Mode::Live {
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
            t::GRAY,
        ),
    ])];
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
                t::GRAY,
            )));
        }
    }
    for key in ["output", "stdout", "stderr", "error"] {
        if let Some(text) = output
            .get(key)
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            let inner = usize::from(width.saturating_sub(6)).max(1);
            let rows = text.lines().flat_map(|row| {
                let ranges = coder_terminal::wrap_rows(row, inner);
                if ranges.is_empty() {
                    vec![Line::from(span(RESULT_INDENT, t::GRAY_BRIGHT))]
                } else {
                    ranges
                        .into_iter()
                        .map(|range| {
                            Line::from(span(
                                format!("{RESULT_INDENT}{}", &row[range]),
                                t::GRAY_BRIGHT,
                            ))
                        })
                        .collect()
                }
            });
            // Keep every output row for plain-text exports. The screen
            // shows a five-row window over these rows.
            lines.extend(rows);
        }
    }
    mark_result(&mut lines);
    lines
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

fn entry_lines(entry: &crate::live::Entry, width: u16, phase: u8) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    match entry {
        crate::live::Entry::User(text) => {
            lines.extend(prompt(text, width));
        }
        // The reply's model and time stay in the entry (exports read them);
        // the transcript shows neither.
        crate::live::Entry::Assistant { text, .. } => {
            reply_lines(&mut lines, text, width);
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
                return lines;
            }
            if name == crate::issue_run::DECISION {
                lines.extend(crate::issue_run::cards::decision_lines(
                    input, output, *running, width, phase,
                ));
                lines.push(Line::default());
                return lines;
            }
            if name == crate::issue_run::SUMMARY {
                lines.extend(crate::issue_run::cards::summary_lines(input, output, width));
                lines.push(Line::default());
                return lines;
            }
            if name == "Run" {
                lines.extend(run_lines(input, output, *running, width, phase));
                lines.push(Line::default());
                return lines;
            }
            if crate::file_tools::is_tool(name) {
                lines.extend(crate::tools::file_tool_lines(
                    name, input, output, *running, width, phase,
                ));
                lines.push(Line::default());
                return lines;
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
                _ if output.get("error").is_some() => "Failed".into(),
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
                span(detail, t::GRAY_BRIGHT),
            ]));
        }
    }
    lines.push(Line::default());
    lines
}

#[derive(Default)]
pub struct TranscriptCache {
    width: u16,
    entries: Vec<CachedEntry>,
    partial: Option<CachedEntry>,
    pub builds: usize,
    run_regions: Vec<(usize, Rect)>,
}

struct CachedEntry {
    width: u16,
    entry: crate::live::Entry,
    lines: Vec<Line<'static>>,
    run_output: Option<(usize, Vec<Line<'static>>, usize, bool)>,
}

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
            let width = usize::from(self.width);
            line.spans.push(Span::raw(
                " ".repeat(width.saturating_sub(line.width() + 1)),
            ));
            line.spans.push(span(
                if max == 0 || index == thumb {
                    "█"
                } else {
                    "│"
                },
                t::GRAY,
            ));
            line.style = line.style.bg(t::BG_LIGHT);
            self.lines.push(line);
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
        }
        self.lines.push(Line::default());
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
                            ..area
                        },
                    ));
                }
            }
            top += cached.lines.len();
        }
    }

    fn refresh(&mut self, chat: &crate::live::Chat, width: u16, phase: u8) {
        let resized = self.width != width;
        self.width = width;
        self.entries.truncate(chat.entries.len());
        for (index, entry) in chat.entries.iter().enumerate() {
            if self
                .entries
                .get(index)
                .is_none_or(|cached| resized || cached.entry != *entry)
            {
                let previous_offset = self
                    .entries
                    .get(index)
                    .and_then(|c| c.run_output.as_ref())
                    .map(|r| (r.2, r.3));
                let mut cached = CachedEntry {
                    width,
                    entry: entry.clone(),
                    lines: entry_lines(entry, width, phase),
                    run_output: None,
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
                cached.lines = entry_lines(entry, width, phase);
                cached.bound_run(width, phase, offset);
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
                .is_none_or(|cached| resized || cached.entry != entry)
            {
                self.partial = Some(CachedEntry {
                    width,
                    lines: entry_lines(&entry, width, phase),
                    entry,
                    run_output: None,
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
/// draws it (`coder issue-run --plain`).
#[must_use]
pub fn transcript_text(entries: &[crate::live::Entry], width: u16) -> Vec<String> {
    entries
        .iter()
        .flat_map(|entry| entry_lines(entry, width, 0))
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
        .flat_map(|entry| entry_lines(entry, width, app.animation_frame))
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
    let chat = app
        .selected_agent
        .and_then(|index| app.delegations.get_mut(index))
        .map_or(&mut app.live, |agent| &mut agent.chat);
    let mut cache = std::mem::take(&mut chat.cache);
    cache.refresh(chat, content, phase);
    let mut tail = Vec::new();
    if chat.busy {
        tail.extend(wrap_display(
            vec![Line::from(vec![
                span(format!("{} ", crate::tools::spinner(phase)), t::GRAY),
                span("Working", t::GRAY),
            ])],
            content,
        ));
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
    let following = app.scroll == u16::MAX;
    let position = app.scroll.min(max_scroll);
    if !following {
        app.scroll = position;
    }
    cache.locate_runs(area, usize::from(position));
    let visible = cache.visible(&tail, usize::from(position), usize::from(area.height));
    frame.render_widget(Paragraph::new(visible), area);
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
) {
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
    for contribution in contributions {
        let offset = match contribution.slot {
            RailSlot::ComposerTopRight => 0,
            RailSlot::ComposerBottomRight => area.height.saturating_sub(1),
        };
        let text = composer_rail_text(&contribution.text, area.width.saturating_sub(6));
        coder_terminal::rail(
            area,
            frame.buffer_mut(),
            offset,
            if offset == 0 { left } else { None },
            Some((
                &text,
                Style::default().fg(match contribution.text.as_str() {
                    crate::models::AUTO => t::ACCENT_SKILL,
                    crate::models::FLASH_LABEL => t::ACCENT_MODEL,
                    _ => t::GRAY,
                }),
            )),
        );
    }
    if inner.width < 4 || inner.height == 0 {
        return;
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
}

#[cfg(test)]
mod export_notice_tests {
    #[test]
    fn history_position_appears_on_the_top_left_rail() {
        let mut app = App::default();
        app.mode = Mode::Live;
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
        assert!(text.contains("auto"));
        app.composer_arrow(false);
        assert!(!rail(&mut app, 60).contains("History"));
        app.composer_arrow(true);
        app.composer_history.reset();
        assert!(!rail(&mut app, 60).contains("History"));
    }

    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn auto_label_uses_blue() {
        let mut app = App::default();
        app.mode = Mode::Live;
        let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
        terminal.draw(|frame| render(frame, &mut app)).unwrap();
        let buffer = terminal.backend().buffer();
        let mut found = false;
        for y in 0..20 {
            for x in 0..57 {
                let text: String = (x..x + 4).map(|x| buffer[(x, y)].symbol()).collect();
                if text == crate::models::AUTO {
                    found = true;
                    for x in x..x + 4 {
                        assert_eq!(buffer[(x, y)].fg, t::ACCENT_SKILL);
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

    #[test]
    fn tab_switches_between_auto_and_flash_only_with_an_openrouter_key() {
        use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
        let tab = || Event::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
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
        app.handle(tab());
        assert_eq!(app.plugins.model, crate::models::AUTO);
        assert!(
            app.notice
                .as_deref()
                .is_some_and(|notice| notice.contains("OpenRouter API key"))
        );

        app.plugins.key_configured = true;
        app.handle(tab());
        assert_eq!(app.plugins.model, crate::models::FLASH);
        assert!(app.plugins.enabled);
        let shown = rail(&mut app);
        assert!(shown.contains("flash") && !shown.contains("deepseek"));
        app.handle(tab());
        assert_eq!(app.plugins.model, crate::models::AUTO);
        assert!(rail(&mut app).contains("auto"));
    }

    #[test]
    fn populated_footer_keeps_its_row_below_the_composer() {
        let mut app = App::default();
        app.mode = Mode::Live;
        app.account = Some("Christopher David".into());
        app.update_line = Some("Update ready".into());
        let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
        terminal.draw(|frame| render(frame, &mut app)).unwrap();
        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(0, 18)].symbol(), "└");
        let text: String = (0..60).map(|x| buffer[(x, 19)].symbol()).collect();
        // The account's name never shows under the input.
        assert_eq!(text.trim(), "Update ready");
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
mod streaming_tests {
    use super::TranscriptCache;

    fn shown(partial: &str) -> String {
        let mut chat = crate::live::Chat::default();
        chat.partial = partial.into();
        let mut cache = TranscriptCache::default();
        cache.refresh(&chat, 60, 0);
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
                t::DIFF_DELETE_FG,
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
            let lines = entry_lines(&entry, 60, 0);
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
                    t::DIFF_DELETE_FG,
                ),
                (false, serde_json::json!({"exit": 1}), t::DIFF_DELETE_FG),
                (false, serde_json::json!({}), t::ACCENT_SUCCESS),
            ] {
                let entry = crate::live::Entry::Tool {
                    name: name.into(),
                    input: serde_json::json!({}),
                    output,
                    running,
                };
                let lines = entry_lines(&entry, 60, 0);
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
            (
                serde_json::json!({"error": "Failed"}),
                "× ",
                t::DIFF_DELETE_FG,
            ),
            (serde_json::json!({"exit": 1}), "× ", t::DIFF_DELETE_FG),
            (
                serde_json::json!({"timed_out": true}),
                "× ",
                t::DIFF_DELETE_FG,
            ),
            (
                serde_json::json!({"canceled": true}),
                "× ",
                t::DIFF_DELETE_FG,
            ),
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
        cache.refresh(&chat, 40, 0);
        assert_eq!(cache.entries[0].run_output.as_ref().unwrap().2, 3);
        output(&mut chat, 12, true);
        cache.refresh(&chat, 40, 1);
        assert_eq!(cache.entries[0].run_output.as_ref().unwrap().2, 7);
        cache.locate_runs(Rect::new(0, 0, 42, 20), 0);
        assert!(!cache.scroll_run(0, 19, true));
        let area = cache.run_regions[0].1;
        assert!(cache.scroll_run(39, area.y, true));
        assert_eq!(cache.entries[0].run_output.as_ref().unwrap().2, 4);
        output(&mut chat, 16, false);
        cache.refresh(&chat, 40, 2);
        assert_eq!(cache.entries[0].run_output.as_ref().unwrap().2, 4);
        cache.refresh(&chat, 30, 3);
        assert_eq!(cache.entries[0].run_output.as_ref().unwrap().2, 4);
        assert!(!cache.entries[0].run_output.as_ref().unwrap().3);
    }

    #[test]
    fn five_output_rows_have_a_background_and_right_hand_scrollbar() {
        let mut chat = crate::live::Chat::default();
        output(&mut chat, 10, false);
        let mut cache = TranscriptCache::default();
        cache.refresh(&chat, 40, 0);
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
                buffer[(39, y as u16)].symbol(),
                if y == start + 4 { "█" } else { "│" }
            );
        }
        let exported = transcript_text(&chat.entries, 40).join("\n");
        assert!(exported.contains("row 1\n"));
        assert!(exported.contains("row 10"));
        assert!(!exported.contains('█'));
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
    fn repository_and_user_cost_are_on_bottom_right_rail() {
        let mut app = App::default();
        app.mode = Mode::Live;
        app.cwd = Some("/work/repo".into());
        app.branch = Some("feature".into());
        app.live.cost_usd = 0.25;
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 20)).unwrap();
        terminal.draw(|frame| render(frame, &mut app)).unwrap();
        let buffer = terminal.backend().buffer();
        let row = (0..20)
            .find(|&y| (0..4).any(|x| buffer[(x, y)].symbol() == "❯"))
            .unwrap();
        let text: String = (0..80).map(|x| buffer[(x, row + 1)].symbol()).collect();
        assert!(text.contains("/work/repo (feature) · $0.25"), "{text}");
        assert!(text.trim_end_matches(['─', '┘']).ends_with(' '), "{text}");
    }
}
