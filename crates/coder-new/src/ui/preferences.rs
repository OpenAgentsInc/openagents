//! The screen for what Coder should favor when OpenAgents picks a model.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Paragraph, Wrap},
};

use super::span;
use crate::{
    App,
    preferences::{AXES, MAX, MIN},
    theme as t,
};

fn bar(value: u8) -> String {
    let value = usize::from(value.clamp(MIN, MAX));
    let cells = usize::from(MAX);
    format!("{}{}", "█".repeat(value), "░".repeat(cells - value))
}

pub(super) fn render(frame: &mut Frame, area: Rect, app: &App) {
    let mut lines = vec![
        Line::from(Span::styled(
            "Preferences",
            Style::default()
                .fg(t::TEXT_PRIMARY)
                .add_modifier(Modifier::BOLD),
        )),
        Line::default(),
    ];
    let width = AXES
        .iter()
        .map(|axis| axis.label().len())
        .max()
        .unwrap_or(0);
    for (index, axis) in AXES.iter().enumerate() {
        let active = index == app.preferences.selected;
        let value = app.preferences.value(*axis);
        lines.push(Line::from(vec![
            span(if active { "❯ " } else { "  " }, t::ACCENT_MODEL),
            Span::styled(
                format!("{:<width$}", axis.label()),
                Style::default()
                    .fg(t::TEXT_PRIMARY)
                    .add_modifier(if active {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ),
            span("  ", t::TEXT_SECONDARY),
            span(bar(value), t::ACCENT_MODEL),
            span(format!("  {value}/{MAX}"), t::TEXT_SECONDARY),
        ]));
    }
    lines.push(Line::default());
    lines.push(Line::from(span(
        "OpenAgents uses these when it picks a model for you.",
        t::TEXT_SECONDARY,
    )));
    lines.push(Line::default());
    lines.push(Line::from(Span::styled(
        "Engines",
        Style::default()
            .fg(t::TEXT_PRIMARY)
            .add_modifier(Modifier::BOLD),
    )));
    match &app.preferences.routing {
        Ok(summary) if summary.is_empty() => lines.push(Line::from(span(
            "No preference yet. Ask Coder to use an engine more, less, or never.",
            t::TEXT_SECONDARY,
        ))),
        Ok(summary) => {
            for line in summary {
                lines.push(Line::from(span(format!("  {line}"), t::TEXT_PRIMARY)));
            }
        }
        Err(error) => lines.push(Line::from(span(error, t::DIFF_DELETE_FG))),
    }
    if let Some(path) = &app.preferences.routing_path {
        lines.push(Line::from(span(
            format!("Every change, with your notes: {}", path.display()),
            t::TEXT_SECONDARY,
        )));
    }
    if let Some(error) = &app.preferences_error {
        lines.push(Line::default());
        lines.push(Line::from(span(error, t::DIFF_DELETE_FG)));
    }
    let hints = if area.width >= 43 {
        "Left/Right or -/+ Adjust · Up/Down Move · Esc Back"
    } else {
        "Left/Right or -/+ Adjust\nUp/Down Move\nEsc Back"
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
