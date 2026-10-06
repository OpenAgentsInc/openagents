//! Fullscreen recent-conversation picker.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::{span, truncate};
use crate::{App, theme as t};

pub(super) fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let picker = app.resume_picker.as_mut().unwrap();
    let footer_rows = 2 + u16::from(picker.error.is_some());
    picker.page = usize::from(area.height.saturating_sub(footer_rows + 2) / 2).max(1);
    let selected = picker.selected.min(picker.sessions.len().saturating_sub(1));
    let start = selected
        .saturating_sub(picker.page / 2)
        .min(picker.sessions.len().saturating_sub(picker.page));
    row(
        frame,
        area,
        0,
        Line::from(Span::styled(
            "Resume · recent conversations",
            Style::default()
                .fg(t::TEXT_PRIMARY)
                .add_modifier(Modifier::BOLD),
        )),
    );
    if picker.sessions.is_empty() {
        row(
            frame,
            area,
            2,
            Line::from(span("No conversations to resume.", t::GRAY_BRIGHT)),
        );
    }
    for (index, session) in picker
        .sessions
        .iter()
        .enumerate()
        .skip(start)
        .take(picker.page)
    {
        let active = index == selected;
        let line = Line::from(vec![
            span(
                if active { "❯ " } else { "  " },
                if active { t::TEXT_PRIMARY } else { t::GRAY },
            ),
            span(
                truncate(
                    &format!("{}. {}", index + 1, session.title),
                    area.width.saturating_sub(2),
                ),
                if active {
                    t::TEXT_PRIMARY
                } else {
                    t::TEXT_SECONDARY
                },
            ),
        ])
        .style(Style::default().bg(if active { t::BG_DARK } else { t::BG_BASE }));
        let y = 2 + (index - start) as u16 * 2;
        row(frame, area, y, line);
        let updated = atif::now_ms().saturating_sub(session.updated_ms) / 1_000;
        let age = match updated {
            0..60 => "just now".into(),
            60..3600 => format!("{}m ago", updated / 60),
            3600..86400 => format!("{}h ago", updated / 3600),
            _ => format!("{}d ago", updated / 86400),
        };
        let cwd = session
            .cwd
            .as_deref()
            .map(|path| path.to_string_lossy())
            .unwrap_or_default();
        let detail = format!(
            "  {age} · {} entries · {} · {cwd}",
            session.entries, session.id
        );
        row(
            frame,
            area,
            y + 1,
            Line::from(span(truncate(&detail, area.width), t::GRAY_BRIGHT)),
        );
    }
    if let Some(error) = &picker.error {
        row(
            frame,
            area,
            area.height.saturating_sub(3),
            Line::from(span(truncate(error, area.width), t::DIFF_DELETE_FG)),
        );
    }
    row(
        frame,
        area,
        area.height.saturating_sub(1),
        Line::from(span(
            truncate("↑/↓ Choose · Enter Resume · Esc Back", area.width),
            t::GRAY_BRIGHT,
        )),
    );
}

fn row(frame: &mut Frame, area: Rect, offset: u16, line: Line<'static>) {
    if offset < area.height {
        frame.render_widget(
            Paragraph::new(line),
            Rect {
                y: area.y + offset,
                height: 1,
                ..area
            },
        );
    }
}
