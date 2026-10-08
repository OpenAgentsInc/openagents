//! Terminal appearance preferences.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Paragraph, Wrap},
};

use super::span;
use crate::{App, theme as t};

pub(super) fn render(frame: &mut Frame, area: Rect, app: &App) {
    let enabled = app.appearance.use_system_terminal_background;
    let mut lines = vec![
        Line::from(Span::styled(
            "Appearance",
            Style::default()
                .fg(t::TEXT_PRIMARY)
                .add_modifier(Modifier::BOLD),
        )),
        Line::default(),
        Line::from(vec![
            span("❯ ", t::ACCENT_MODEL),
            Span::styled(
                "Use System Terminal Background",
                Style::default()
                    .fg(t::TEXT_PRIMARY)
                    .add_modifier(Modifier::BOLD),
            ),
            span(
                if enabled { " [ on ]" } else { " [ off ]" },
                t::ACCENT_MODEL,
            ),
        ]),
        Line::default(),
        Line::from(span(
            "On uses your terminal's background. Off uses Coder's background.",
            t::TEXT_SECONDARY,
        )),
    ];
    if let Some(error) = &app.appearance_error {
        lines.push(Line::default());
        lines.push(Line::from(span(error, t::DIFF_DELETE_FG)));
    }
    let hints = if area.width >= 29 {
        "Space/Enter Toggle · Esc Back"
    } else {
        "Space/Enter Toggle\nEsc Back"
    };
    let hints_height = hints.lines().count() as u16;
    frame.render_widget(
        Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false }),
        Rect {
            height: area.height.saturating_sub(hints_height + 1),
            ..area
        },
    );
    frame.render_widget(
        Paragraph::new(hints).style(Style::default().fg(t::GRAY)),
        Rect {
            y: area.bottom().saturating_sub(hints_height),
            height: hints_height,
            ..area
        },
    );
}
