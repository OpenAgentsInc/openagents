//! Fullscreen `/rewind` picker (#11515): the latest messages, each with
//! the files its turn changed.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
};

use super::{span, truncate};
use crate::{App, theme as t};

/// The rows a message's detail takes under it.
const DETAIL_ROWS: u16 = 2;

pub(super) fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let picker = app.rewind_picker.as_mut().unwrap();
    let per = 1 + DETAIL_ROWS;
    picker.page = usize::from(area.height.saturating_sub(4) / per).max(1);
    let selected = picker.selected.min(picker.choices.len().saturating_sub(1));
    let start = selected
        .saturating_sub(picker.page / 2)
        .min(picker.choices.len().saturating_sub(picker.page));
    row(
        frame,
        area,
        0,
        Line::from(Span::styled(
            "Rewind · undo a turn's file changes and return to its message",
            Style::default()
                .fg(t::TEXT_PRIMARY)
                .add_modifier(Modifier::BOLD),
        )),
    );
    for (index, choice) in picker
        .choices
        .iter()
        .enumerate()
        .skip(start)
        .take(picker.page)
    {
        let active = index == selected;
        let prompt = choice.prompt.lines().next().unwrap_or_default();
        let line = Line::from(vec![
            span(
                if active { "❯ " } else { "  " },
                if active { t::TEXT_PRIMARY } else { t::GRAY },
            ),
            span(
                truncate(prompt, area.width.saturating_sub(2)),
                if active {
                    t::TEXT_PRIMARY
                } else {
                    t::TEXT_SECONDARY
                },
            ),
        ])
        .style(Style::default().bg(if active { t::BG_DARK } else { t::BG_BASE }));
        let y = 2 + (index - start) as u16 * per;
        row(frame, area, y, line);
        let count = match choice.files.len() {
            1 => "1 file".to_owned(),
            count => format!("{count} files"),
        };
        let files = if choice.files.is_empty() {
            "  No file changes".to_owned()
        } else {
            format!("  {count} changed: {}", choice.files.join(", "))
        };
        row(
            frame,
            area,
            y + 1,
            Line::from(span(truncate(&files, area.width), t::GRAY_BRIGHT)),
        );
        if choice.partly() {
            let why = if choice.unrestorable.is_empty() {
                "  Partly restorable: too many changed files to keep them all".to_owned()
            } else {
                format!(
                    "  Partly restorable: can't restore {}",
                    choice.unrestorable.join(", ")
                )
            };
            row(
                frame,
                area,
                y + 2,
                Line::from(span(truncate(&why, area.width), t::DIFF_DELETE_FG)),
            );
        }
    }
    row(
        frame,
        area,
        area.height.saturating_sub(1),
        Line::from(span(
            truncate("↑/↓ Choose · Enter Rewind · Esc Back", area.width),
            t::GRAY_BRIGHT,
        )),
    );
}

fn row(frame: &mut Frame, area: Rect, offset: u16, line: Line<'static>) {
    if offset < area.height {
        frame.render_widget(
            ratatui::widgets::Paragraph::new(line),
            Rect {
                y: area.y + offset,
                height: 1,
                ..area
            },
        );
    }
}
