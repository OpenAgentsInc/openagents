//! The same render function draws the terminal and exported previews.

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
    App, Screen,
    agents::{DEMOS, DemoMessage, MAIN_PLUGINS, MAIN_TOOLS},
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
    let (draft, cursor) = app.draft.wrapped(terminal_width.saturating_sub(2));
    let rail_height = DEMOS.len() as u16;
    let composer_height = (draft.len() as u16).clamp(1, 6) + 2;
    let reserved = rail_height + 3;
    let [header, body, _gap, composer, rail] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(composer_height.min(area.height.saturating_sub(reserved))),
        Constraint::Length(rail_height),
    ])
    .areas(area);
    header_view(frame, header);
    match app.screen {
        Screen::Welcome => welcome(frame, body),
        Screen::Conversation => conversation(frame, body, app),
    }
    composer_view(
        frame,
        Rect {
            x: terminal_x,
            width: terminal_width,
            ..composer
        },
        &draft,
        cursor,
    );
    agent_rail(frame, rail, app.selected_agent);
}

fn agent_rail(frame: &mut Frame, area: Rect, selected: Option<usize>) {
    for (index, agent) in DEMOS.iter().enumerate() {
        let row = Rect {
            y: area.y + index as u16,
            height: 1,
            ..area
        };
        let narrow = area.width < 32;
        let suffix = if narrow {
            format!("↓ {}", agent.tokens)
        } else {
            format!("↓ {} tokens", agent.tokens)
        };
        let token_width = suffix.width() as u16;
        let [activity, _gap, tokens] = Layout::horizontal([
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(token_width),
        ])
        .areas(row);
        let active = selected == Some(index);
        let prefix = match (narrow, active) {
            (true, true) => "❯ ",
            (true, false) => "  ",
            (false, true) => "❯ ",
            (false, false) => "○ ",
        };
        let name_width = (prefix.width()
            + DEMOS
                .iter()
                .map(|demo| demo.name.width())
                .max()
                .unwrap_or(0)) as u16;
        let name = Rect {
            width: name_width.min(activity.width),
            ..activity
        };
        let task_width = activity.width.saturating_sub(name_width + 2);
        let spans = vec![
            span(prefix, if active { t::ACCENT_MODEL } else { t::GRAY }),
            Span::styled(
                agent.name,
                Style::default()
                    .fg(if active {
                        t::ACCENT_MODEL
                    } else {
                        t::TEXT_SECONDARY
                    })
                    .add_modifier(Modifier::BOLD),
            ),
        ];
        frame.render_widget(Paragraph::new(Line::from(spans)), name);
        if task_width > 0 {
            let task = Rect {
                x: activity.x + name_width + 2,
                width: task_width,
                ..activity
            };
            frame.render_widget(
                Paragraph::new(span(truncate(agent.task, task_width), t::GRAY)),
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

fn header_view(frame: &mut Frame, area: Rect) {
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            span("◆ ", t::ACCENT_MODEL),
            Span::styled(
                "Coder",
                Style::default()
                    .fg(t::TEXT_PRIMARY)
                    .add_modifier(Modifier::BOLD),
            ),
            span("  openagents", t::PATH),
            span(" / main", t::GRAY),
        ])),
        area,
    );
}

fn welcome(frame: &mut Frame, area: Rect) {
    let height = area.height.min(9);
    let area = Rect {
        y: area.y + area.height.saturating_sub(height) / 2,
        height,
        ..area
    };
    frame.render_widget(
        Paragraph::new(Text::from(vec![
            Line::from(Span::styled(
                "What do you want to build?",
                Style::default()
                    .fg(t::TEXT_PRIMARY)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::default(),
            Line::from(span(
                "A place to think, make changes, and keep going.",
                t::TEXT_SECONDARY,
            )),
            Line::default(),
            Line::from(vec![
                span("◇ ", t::ACCENT_SKILL),
                span("Tools and models are plugins.", t::GRAY_BRIGHT),
            ]),
            Line::from(vec![
                span("◇ ", t::ACCENT_MODEL),
                span("Discover them on Nostr. Pay in sats.", t::GRAY_BRIGHT),
            ]),
            Line::default(),
            Line::from(span(
                "Type below, or press Tab to see a sample conversation.",
                t::GRAY,
            )),
        ]))
        .wrap(Wrap { trim: false }),
        area,
    );
}

fn prompt(text: impl Into<String>) -> Line<'static> {
    Line::from(vec![
        span(" ❯ ", t::TEXT_SECONDARY),
        span(text, t::TEXT_PRIMARY),
    ])
    .style(Style::default().bg(t::BG_LIGHT))
}

fn conversation(frame: &mut Frame, area: Rect, app: &mut App) {
    let mut lines = if let Some(agent) = app.selected_agent.and_then(|index| DEMOS.get(index)) {
        let mut lines = vec![
            Line::from(Span::styled(
                format!("{} · Demo conversation", agent.name),
                Style::default()
                    .fg(t::ACCENT_MODEL)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::default(),
        ];
        for (index, message) in agent.conversation.iter().enumerate() {
            match message {
                DemoMessage::User(text) => lines.push(prompt(*text)),
                DemoMessage::Tool(call) => {
                    lines.extend(tool_lines(call, app.animation_frame, area.width));
                }
                DemoMessage::Plugin(call) => lines.extend(plugin_lines(call, app.animation_frame)),
                DemoMessage::Assistant(text) => {
                    lines.push(Line::from(span(*text, t::TEXT_SECONDARY)));
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
        let mut lines = vec![
            prompt("Review the terminal with four agents."),
            Line::default(),
        ];
        for call in &MAIN_TOOLS {
            lines.extend(tool_lines(call, app.animation_frame, area.width));
        }
        for call in &MAIN_PLUGINS {
            lines.extend(plugin_lines(call, app.animation_frame));
        }
        lines.push(Line::default());
        for agent in &DEMOS {
            lines.extend(delegation_lines(agent, app.animation_frame, area.width));
        }
        lines.push(Line::default());
        lines
    };
    for message in &app.messages {
        for (index, line) in message.split('\n').enumerate() {
            lines.push(if index == 0 {
                prompt(line)
            } else {
                Line::from(span(format!("   {line}"), t::TEXT_PRIMARY))
                    .style(Style::default().bg(t::BG_LIGHT))
            });
        }
        lines.extend([
            Line::default(),
            Line::from(span(
                "Preview message added. No agent is connected.",
                t::GRAY,
            )),
            Line::default(),
        ]);
    }
    let paragraph = Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false });
    let max_scroll = paragraph
        .line_count(area.width)
        .saturating_sub(usize::from(area.height))
        .min(usize::from(u16::MAX)) as u16;
    app.scroll = app.scroll.min(max_scroll);
    frame.render_widget(paragraph.scroll((app.scroll, 0)), area);
}

fn composer_view(frame: &mut Frame, area: Rect, draft: &[String], cursor: (u16, u16)) {
    let block = Block::default()
        .borders(Borders::TOP | Borders::BOTTOM)
        .border_style(Style::default().fg(t::PROMPT_BORDER_ACTIVE))
        .style(Style::default().bg(t::BG_BASE));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width < 3 || inner.height == 0 {
        return;
    }
    frame.render_widget(Paragraph::new(span("❯", t::TEXT_SECONDARY)), inner);
    let text_area = Rect {
        x: inner.x + 2,
        width: inner.width - 2,
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
    frame.set_cursor_position((
        text_area.x + cursor.0.min(text_area.width.saturating_sub(1)),
        text_area.y + cursor.1.saturating_sub(scroll),
    ));
}
