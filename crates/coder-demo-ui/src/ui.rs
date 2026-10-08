//! The same render function draws the terminal and exported previews.

mod models;
mod plugins;

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

    let area = Rect {
        x: area.x + 2,
        y: area.y + 1,
        width: area.width.saturating_sub(4),
        height: area.height.saturating_sub(2),
    };
    if matches!(app.screen, Screen::Plugins | Screen::PluginSettings) {
        plugins::render(frame, area, app);
        if app.model_picker.is_some() {
            models::render(frame, app);
        }
        return;
    }
    let (draft, cursor) = app.draft.wrapped(terminal_width.saturating_sub(3));
    let rail_height = if app.mode == Mode::Demo {
        DEMOS.len().min(usize::from(area.height.saturating_sub(6))) as u16
    } else {
        0
    };
    let composer_height = (draft.len() as u16).clamp(1, 6) + 2;
    let reserved = rail_height + 3;
    let [header, body, composer, context, rail] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(composer_height.min(area.height.saturating_sub(reserved))),
        Constraint::Length(1),
        Constraint::Length(rail_height),
    ])
    .areas(area);
    header_view(frame, header, app);
    match app.screen {
        Screen::Conversation => conversation(frame, body, app),
        Screen::Plugins | Screen::PluginSettings => unreachable!(),
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
        app.model_picker.is_none(),
        &app.plugins,
        (app.mode == Mode::Live && !(app.plugins.enabled && app.plugins.key_configured))
            .then_some("auto"),
    );
    context_view(frame, context, app);
    agent_rail(frame, rail, app);
    if app.model_picker.is_some() {
        models::render(frame, app);
    }
}

fn agent_rail(frame: &mut Frame, area: Rect, app: &App) {
    if app.mode != Mode::Demo {
        return;
    }
    let agents: Vec<_> = DEMOS
        .iter()
        .map(|agent| {
            (
                agent.name,
                agent.task,
                agent.tokens.to_owned(),
                agent.elapsed_seconds.saturating_add(app.elapsed_seconds),
            )
        })
        .collect();
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
                Paragraph::new(span(truncate(agent.1, task_width), t::GRAY)),
                task,
            );
        }
        frame.render_widget(
            Paragraph::new(span(suffix, t::GRAY)).right_aligned(),
            tokens,
        );
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
    let agent = if app.screen == Screen::Conversation && app.mode == Mode::Demo {
        app.selected_agent
            .and_then(|index| DEMOS.get(index).map(|agent| agent.name))
    } else {
        None
    };
    let title_width = agent.map_or(0, |name| name.width() as u16);
    if let Some(agent) = agent {
        frame.render_widget(
            Paragraph::new(Span::styled(
                agent.to_owned(),
                Style::default()
                    .fg(t::ACCENT_MODEL)
                    .add_modifier(Modifier::BOLD),
            )),
            Rect {
                width: title_width.min(area.width),
                ..area
            },
        );
    }
}

fn context_view(frame: &mut Frame, area: Rect, app: &App) {
    let directory = app
        .cwd
        .as_ref()
        .and_then(|path| path.file_name())
        .and_then(|name| name.to_str())
        .unwrap_or("openagents");
    let branch = app.branch.as_deref().unwrap_or("main");
    let suffix = format!(" / {branch}");
    let suffix_width = suffix.width().min(usize::from(u16::MAX)) as u16;
    let spans = if suffix_width < area.width {
        vec![
            span(
                truncate(directory, area.width - suffix_width),
                t::TEXT_PRIMARY,
            ),
            span(suffix, t::GRAY),
        ]
    } else {
        vec![span(
            truncate(&format!("{directory}{suffix}"), area.width),
            t::TEXT_PRIMARY,
        )]
    };
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn message_body(text: &str, width: u16) -> Vec<Line<'static>> {
    crate::rich_lines(coder_ui::components::markdown::lines(
        text,
        usize::from(width),
    ))
}

fn prompt(text: &str, width: u16) -> Vec<Line<'static>> {
    let mut rows = message_body(text, width.saturating_sub(3));
    if rows.is_empty() {
        rows.push(Line::default());
    }
    for (index, row) in rows.iter_mut().enumerate() {
        row.spans.insert(
            0,
            span(if index == 0 { " ❯ " } else { "   " }, t::TEXT_SECONDARY),
        );
        row.style = row.style.bg(t::BG_LIGHT);
    }
    rows
}

fn wrap_display(lines: Vec<Line<'static>>, width: u16) -> Vec<Line<'static>> {
    let mut rows = Vec::new();
    for line in lines {
        for range in coder_ui::components::wrap_ranges(&line.to_string(), usize::from(width).max(1))
        {
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
    let mut lines = if app.mode != Mode::Demo {
        Vec::new()
    } else if let Some(agent) = app.selected_agent.and_then(|index| DEMOS.get(index)) {
        let mut lines = Vec::new();
        for (index, message) in agent.conversation.iter().enumerate() {
            match message {
                DemoMessage::User(text) => lines.extend(prompt(text, area.width)),
                DemoMessage::Tool(call) => {
                    lines.extend(wrap_display(
                        tool_lines(call, app.animation_frame, area.width),
                        area.width,
                    ));
                }
                DemoMessage::Plugin(call) => lines.extend(wrap_display(
                    plugin_lines(call, app.animation_frame),
                    area.width,
                )),
                DemoMessage::Assistant(text) => {
                    lines.extend(message_body(text, area.width));
                }
            }
            let grouped = matches!(message, DemoMessage::Tool(_) | DemoMessage::Plugin(_))
                && matches!(
                    agent.conversation.get(index + 1),
                    Some(DemoMessage::Tool(_) | DemoMessage::Plugin(_))
                );
            if !grouped {
                lines.push(Line::default());
            }
        }
        lines
    } else {
        let mut lines = prompt("Review the terminal with four agents.", area.width);
        lines.push(Line::default());
        for call in &MAIN_TOOLS {
            lines.extend(wrap_display(
                tool_lines(call, app.animation_frame, area.width),
                area.width,
            ));
        }
        for call in &MAIN_PLUGINS {
            lines.extend(wrap_display(
                plugin_lines(call, app.animation_frame),
                area.width,
            ));
        }
        lines.push(Line::default());
        for agent in &DEMOS {
            lines.extend(delegation_lines(agent, app.animation_frame, area.width));
        }
        lines.push(Line::default());
        lines
    };
    for message in app.messages.iter().filter(|_| app.mode == Mode::Demo) {
        lines.extend(prompt(message, area.width));
        lines.push(Line::default());
        lines.extend(wrap_display(
            vec![Line::from(span(
                "Preview message added. No agent is connected.",
                t::GRAY,
            ))],
            area.width,
        ));
        lines.push(Line::default());
    }
    if let Some(notice) = &app.notice {
        lines.extend(wrap_display(
            notice
                .lines()
                .map(|text| Line::from(span(text, t::GRAY)))
                .collect(),
            area.width,
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
    fallback_model: Option<&str>,
) {
    let block = Block::default()
        .borders(Borders::TOP | Borders::BOTTOM)
        .border_style(Style::default().fg(t::PROMPT_BORDER_ACTIVE))
        .style(Style::default().bg(t::BG_BASE));
    let inner = block.inner(area);
    frame.render_widget(block, area);
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
        crate::rail(
            area,
            frame.buffer_mut(),
            offset,
            None,
            Some((&text, Style::default().fg(t::GRAY))),
        );
    }
    if inner.width < 4 || inner.height == 0 {
        return;
    }
    frame.render_widget(
        Paragraph::new(span(
            " ❯",
            if main_selected {
                t::TEXT_SECONDARY
            } else {
                t::GRAY_DIM
            },
        )),
        inner,
    );
    let text_area = Rect {
        x: inner.x + 3,
        width: inner.width - 3,
        ..inner
    };
    let scroll = cursor.1.saturating_sub(text_area.height.saturating_sub(1));
    let text = Text::from(
        draft
            .iter()
            .map(|line| Line::from(span(line.clone(), t::TEXT_PRIMARY)))
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
