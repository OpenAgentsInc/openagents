//! The same render function draws the terminal and exported previews.

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
        return;
    }
    let (draft, cursor) = app.draft.wrapped(terminal_width.saturating_sub(3));
    let rail_height = if app.mode == Mode::Demo {
        DEMOS.len() as u16
    } else {
        0
    };
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
    header_view(frame, header, app);
    match app.screen {
        Screen::Welcome => welcome(frame, body),
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
        app.mode == Mode::Live || app.selected_agent.is_none(),
    );
    if app.mode == Mode::Demo {
        agent_rail(frame, rail, app);
    }
}

fn agent_rail(frame: &mut Frame, area: Rect, app: &App) {
    let name_width = (2 + DEMOS
        .iter()
        .map(|demo| demo.name.width())
        .max()
        .unwrap_or(0)) as u16;
    let narrow = area.width < 32;
    let count_width = DEMOS
        .iter()
        .map(|agent| agent.tokens.width())
        .max()
        .unwrap_or(0);
    let token_labels: Vec<_> = DEMOS
        .iter()
        .map(|agent| {
            if narrow {
                format!("{:>count_width$}↓", agent.tokens)
            } else {
                format!("{:>count_width$} tokens ↓", agent.tokens)
            }
        })
        .collect();
    let timed_labels: Vec<_> = DEMOS
        .iter()
        .zip(&token_labels)
        .map(|(agent, tokens)| {
            format!(
                "{} · {tokens}",
                elapsed_time(agent.elapsed_seconds.saturating_add(app.elapsed_seconds))
            )
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
    for (index, agent) in DEMOS.iter().enumerate() {
        let row = Rect {
            y: area.y + index as u16,
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
        let prefix = match (narrow, active) {
            (true, true) => "❯ ",
            (true, false) => "  ",
            (false, true) => "❯ ",
            (false, false) => "○ ",
        };
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

fn header_view(frame: &mut Frame, area: Rect, app: &App) {
    let agent = if app.screen == Screen::Conversation && app.mode == Mode::Demo {
        app.selected_agent.and_then(|index| DEMOS.get(index))
    } else {
        None
    };
    let title_width = agent.map_or(0, |agent| agent.name.width() as u16);
    let context_width = area
        .width
        .saturating_sub(if agent.is_some() { title_width + 2 } else { 0 })
        .min(if app.mode == Mode::Live { 24 } else { 17 });
    let context = Rect {
        x: area.right().saturating_sub(context_width),
        width: context_width,
        ..area
    };
    let mut context_spans = Vec::new();
    if app.mode == Mode::Live {
        context_spans.push(span("live · ", t::ACCENT_MODEL));
    }
    context_spans.extend([
        span(
            truncate(
                "openagents",
                context_width.saturating_sub(if app.mode == Mode::Live { 14 } else { 7 }),
            ),
            t::PATH,
        ),
        span(" / main", t::GRAY),
    ]);
    frame.render_widget(
        Paragraph::new(Line::from(context_spans)).right_aligned(),
        context,
    );
    if let Some(agent) = agent {
        frame.render_widget(
            Paragraph::new(Span::styled(
                agent.name,
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
    let mut lines = if app.mode == Mode::Live {
        live_lines(app)
    } else if let Some(agent) = app.selected_agent.and_then(|index| DEMOS.get(index)) {
        let mut lines = Vec::new();
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
    for message in app.messages.iter().filter(|_| app.mode == Mode::Demo) {
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
    if let Some(notice) = &app.notice {
        lines.extend(
            notice
                .lines()
                .map(|text| Line::from(span(text, t::GRAY_BRIGHT))),
        );
        lines.push(Line::default());
    }
    let paragraph = Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false });
    let max_scroll = paragraph
        .line_count(area.width)
        .saturating_sub(usize::from(area.height))
        .min(usize::from(u16::MAX)) as u16;
    app.scroll = app.scroll.min(max_scroll);
    frame.render_widget(paragraph.scroll((app.scroll, 0)), area);
}

fn live_lines(app: &App) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    if app.live.entries.is_empty() {
        lines.extend([
            Line::from(span(
                if app.plugins.enabled && app.plugins.key_configured {
                    "Ask OpenRouter a question."
                } else {
                    "Connect OpenRouter BYOK in /plugins to start."
                },
                t::TEXT_SECONDARY,
            )),
            Line::from(span("Your messages go directly to OpenRouter.", t::GRAY)),
            Line::default(),
        ]);
    }
    for entry in &app.live.entries {
        match entry {
            crate::live::Entry::User(text) => {
                for (index, line) in text.lines().enumerate() {
                    lines.push(if index == 0 {
                        prompt(line)
                    } else {
                        Line::from(span(format!("   {line}"), t::TEXT_PRIMARY))
                            .style(Style::default().bg(t::BG_LIGHT))
                    });
                }
            }
            crate::live::Entry::Assistant(text) => lines.extend(
                text.lines()
                    .map(|line| Line::from(span(line, t::TEXT_SECONDARY))),
            ),
        }
        lines.push(Line::default());
    }
    if !app.live.partial.is_empty() {
        lines.extend(
            app.live
                .partial
                .lines()
                .map(|line| Line::from(span(line, t::TEXT_SECONDARY))),
        );
        lines.push(Line::default());
    }
    if app.live.busy {
        lines.push(Line::from(vec![
            span(
                format!("{} ", crate::tools::spinner(app.animation_frame)),
                t::ACCENT_MODEL,
            ),
            span("OpenRouter is replying…", t::GRAY),
        ]));
    }
    if let Some(notice) = &app.live.notice {
        lines.extend(
            notice
                .lines()
                .map(|line| Line::from(span(line, t::DIFF_DELETE_FG))),
        );
        lines.push(Line::default());
    }
    lines
}

fn composer_view(
    frame: &mut Frame,
    area: Rect,
    draft: &[String],
    cursor: (u16, u16),
    main_selected: bool,
) {
    let block = Block::default()
        .borders(Borders::TOP | Borders::BOTTOM)
        .border_style(Style::default().fg(t::PROMPT_BORDER_ACTIVE))
        .style(Style::default().bg(t::BG_BASE));
    let inner = block.inner(area);
    frame.render_widget(block, area);
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
    frame.set_cursor_position((
        text_area.x + cursor.0.min(text_area.width.saturating_sub(1)),
        text_area.y + cursor.1.saturating_sub(scroll),
    ));
}
