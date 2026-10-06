//! The same render function draws the terminal and exported previews.

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
};

use crate::{App, Screen, theme as t};

fn span(text: impl Into<String>, color: Color) -> Span<'static> {
    Span::styled(text.into(), Style::default().fg(color))
}

pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(t::BG_BASE).fg(t::TEXT_SECONDARY)),
        area,
    );
    if area.width < 24 || area.height < 12 {
        frame.render_widget(
            Paragraph::new("Coder · UI preview\nResize to continue.\nCtrl+C to quit.")
                .wrap(Wrap { trim: false }),
            area,
        );
        return;
    }

    let area = area.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    let (draft, cursor) = app.draft.wrapped(area.width.saturating_sub(6));
    let composer_height = (draft.len() as u16).clamp(3, 6) + 2;
    let [header, body, _gap, composer, status, help] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(composer_height.min(area.height.saturating_sub(7))),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(area);
    header_view(frame, header);
    match app.screen {
        Screen::Welcome => welcome(frame, body),
        Screen::Conversation => conversation(frame, body, app),
    }
    composer_view(frame, composer, &draft, cursor, app.draft.text.is_empty());

    let [left, right] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(status);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            span("  ◇ ", t::ACCENT_SKILL),
            span("6 plugins", t::GRAY_BRIGHT),
            span("  ·  ", t::GRAY_DIM),
            span("24,000 sats", t::GRAY_BRIGHT),
        ])),
        left,
    );
    frame.render_widget(
        Paragraph::new(span("Sample data  ", t::GRAY)).right_aligned(),
        right,
    );
    let hints = if area.width >= 90 {
        "  Enter preview message · Alt+Enter newline · Tab switch view · PgUp/PgDn scroll · Ctrl+C quit"
    } else if area.width >= 55 {
        "  Enter preview · Tab switch view · Ctrl+C quit"
    } else {
        "Tab view · Ctrl+C quit"
    };
    frame.render_widget(Paragraph::new(span(hints, t::GRAY_DIM)), help);
}

fn header_view(frame: &mut Frame, area: Rect) {
    let (label, label_width) = if area.width >= 55 {
        ("UI preview · Grok Night", 24)
    } else {
        ("Preview", 7)
    };
    let [brand, preview] =
        Layout::horizontal([Constraint::Min(12), Constraint::Length(label_width)])
            .areas(Rect { height: 1, ..area });
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            span("◆ ", t::ACCENT_MODEL),
            Span::styled(
                "Coder",
                Style::default()
                    .fg(t::TEXT_PRIMARY)
                    .add_modifier(Modifier::BOLD),
            ),
        ])),
        brand,
    );
    frame.render_widget(
        Paragraph::new(span(label, t::GRAY)).right_aligned(),
        preview,
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            span("openagents", t::PATH),
            span("  /  main", t::GRAY),
        ])),
        Rect {
            y: area.y + 1,
            height: 1,
            ..area
        },
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
    let mut lines = vec![
        Line::default().style(Style::default().bg(t::BG_LIGHT)),
        prompt("Sketch the new Coder terminal. Start with the screen."),
        Line::default().style(Style::default().bg(t::BG_LIGHT)),
        Line::default(),
        Line::from(vec![
            span(" ◇ ", t::GRAY_DIM),
            span("Read 3 files, Searched 2 patterns", t::GRAY_BRIGHT),
        ]),
        Line::from(vec![
            span(" ◆ ", t::ACCENT_SUCCESS),
            span("Create ", t::GRAY_BRIGHT),
            span("crates/coder-new", t::PATH),
        ]),
        Line::default(),
        Line::from(Span::styled(
            "Conversation first",
            Style::default()
                .fg(t::ACCENT_MODEL)
                .add_modifier(Modifier::BOLD),
        )),
        Line::default(),
        Line::from(span(
            "Keep the work in one conversation. Tool calls stay compact, and your next message is always within reach.",
            t::TEXT_SECONDARY,
        )),
        Line::default(),
        Line::from(vec![
            span("  • ", t::GRAY),
            span(
                "One quiet column for messages and results.",
                t::TEXT_SECONDARY,
            ),
        ]),
        Line::from(vec![
            span("  • ", t::GRAY),
            span("A composer that stays on screen.", t::TEXT_SECONDARY),
        ]),
        Line::from(vec![
            span("  • ", t::GRAY),
            span(
                "Plugins for tools, models, and workflows.",
                t::TEXT_SECONDARY,
            ),
        ]),
        Line::default(),
        Line::from(vec![
            span("Next: ", t::TEXT_SECONDARY),
            span("the layout", t::MD_CODE),
            span(", then the interactions.", t::TEXT_SECONDARY),
        ]),
        Line::default(),
        Line::from(span(" src/main.rs", t::GRAY)).style(Style::default().bg(t::BG_DARK)),
        Line::from(span(" - let app = OldTerminal::new();", t::DIFF_DELETE_FG))
            .style(Style::default().bg(t::DIFF_DELETE_BG)),
        Line::from(span(" + let app = Coder::new();", t::DIFF_INSERT_FG))
            .style(Style::default().bg(t::DIFF_INSERT_BG)),
        Line::default(),
    ];
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

fn composer_view(frame: &mut Frame, area: Rect, draft: &[String], cursor: (u16, u16), empty: bool) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(t::PROMPT_BORDER_ACTIVE))
        .style(Style::default().bg(t::BG_BASE))
        .title(Line::from(span(" Message ", t::TEXT_SECONDARY)));
    let inner = block.inner(area).inner(Margin {
        horizontal: 1,
        vertical: 0,
    });
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
    let text = if empty {
        Text::from(span("Describe what you want to build…", t::GRAY_DIM))
    } else {
        Text::from(
            draft
                .iter()
                .map(|line| Line::from(span(line.clone(), t::TEXT_PRIMARY)))
                .collect::<Vec<_>>(),
        )
    };
    frame.render_widget(Paragraph::new(text).scroll((scroll, 0)), text_area);
    frame.set_cursor_position((
        text_area.x + cursor.0.min(text_area.width.saturating_sub(1)),
        text_area.y + cursor.1.saturating_sub(scroll),
    ));
}
