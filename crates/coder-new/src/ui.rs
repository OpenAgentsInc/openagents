//! The same render function draws the terminal and exported previews.

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::{App, Screen, agents::DEMOS, theme as t};

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
    let (draft, cursor) = app.draft.wrapped(area.width.saturating_sub(2));
    let cramped = area.height < 14;
    let header_height = if cramped { 2 } else { 3 };
    let gap_height = u16::from(!cramped);
    let rail_height = DEMOS.len() as u16 + u16::from(!cramped);
    let status_height = u16::from(!cramped);
    let help_height = u16::from(!cramped);
    let composer_height = (draft.len() as u16).clamp(1, 6) + 2;
    let reserved = header_height + gap_height + rail_height + status_height + help_height + 1;
    let [header, body, _gap, composer, rail, status, help] = Layout::vertical([
        Constraint::Length(header_height),
        Constraint::Min(1),
        Constraint::Length(gap_height),
        Constraint::Length(composer_height.min(area.height.saturating_sub(reserved))),
        Constraint::Length(rail_height),
        Constraint::Length(status_height),
        Constraint::Length(help_height),
    ])
    .areas(area);
    header_view(frame, header);
    match app.screen {
        Screen::Welcome => welcome(frame, body),
        Screen::Conversation => conversation(frame, body, app),
    }
    composer_view(frame, composer, &draft, cursor);
    agent_rail(frame, rail);

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

fn agent_rail(frame: &mut Frame, area: Rect) {
    let offset = area.height.saturating_sub(DEMOS.len() as u16);
    for (index, agent) in DEMOS.iter().enumerate() {
        let row = Rect {
            y: area.y + offset + index as u16,
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
        let [activity, tokens] =
            Layout::horizontal([Constraint::Min(1), Constraint::Length(token_width)]).areas(row);
        let prefix = if narrow { "" } else { "  ○ " };
        let name_width = prefix.width() + agent.name.width();
        let task_width = usize::from(activity.width).saturating_sub(name_width + 3);
        let mut spans = vec![
            span(prefix, t::GRAY),
            Span::styled(
                agent.name,
                Style::default()
                    .fg(t::TEXT_SECONDARY)
                    .add_modifier(Modifier::BOLD),
            ),
        ];
        if task_width > 0 {
            spans.push(span(": ", t::GRAY));
            spans.push(span(truncate(agent.task, task_width as u16), t::GRAY));
        }
        frame.render_widget(Paragraph::new(Line::from(spans)), activity);
        frame.render_widget(
            Paragraph::new(span(suffix, t::GRAY)).right_aligned(),
            tokens,
        );
    }
}

fn truncate(text: &str, width: u16) -> String {
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
