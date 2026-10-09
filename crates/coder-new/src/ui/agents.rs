//! The `/agents` panel: every background agent with its status, time,
//! tokens and dollars, and the keys to open, stop, message or resume one.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::{span, truncate};
use crate::{App, theme as t};

/// The panel's lines for `width` columns, without drawing them.
pub(crate) fn lines(app: &App, width: u16) -> Vec<Line<'static>> {
    let rows = app.fleet.list();
    let selected = app.agents_panel.map_or(0, |panel| panel.selected);
    let now = agent_fleet::now_ms();
    let mut lines = vec![
        Line::from(Span::styled(
            format!(
                "Agents · {} running · {} in all",
                rows.iter()
                    .filter(|row| row.status == agent_fleet::Status::Running)
                    .count(),
                rows.len()
            ),
            Style::default()
                .fg(t::TEXT_PRIMARY)
                .add_modifier(Modifier::BOLD),
        )),
        Line::default(),
    ];
    if rows.is_empty() {
        lines.push(Line::from(span(
            "No background agents yet. Ask for work in parallel, or type /agent ENGINE TASK.",
            t::GRAY_BRIGHT,
        )));
    }
    for (index, row) in rows.iter().enumerate() {
        let chosen = index == selected.min(rows.len().saturating_sub(1));
        let cost = row
            .cost_usd
            .map_or_else(|| "—".to_owned(), agent_fleet::dollars);
        let tokens = if row.tokens == 0 {
            "—".to_owned()
        } else {
            agent_fleet::token_words(row.tokens)
        };
        let facts = format!(
            "{:<8} {:>7} {:>7} {:>8}  {} · {}",
            row.status.word(),
            agent_fleet::elapsed_words(row.elapsed_seconds(now)),
            tokens,
            cost,
            row.engine,
            row.place
        );
        let status_color = match row.status {
            agent_fleet::Status::Running => t::ACCENT_MODEL,
            agent_fleet::Status::Done => t::ACCENT_SUCCESS,
            agent_fleet::Status::Failed => t::DIFF_DELETE_FG,
            agent_fleet::Status::Stopped => t::GRAY,
        };
        lines.push(Line::from(vec![
            span(if chosen { "❯ " } else { "  " }, t::ACCENT_MODEL),
            Span::styled(
                truncate(&row.name, 28),
                Style::default()
                    .fg(if chosen {
                        t::ACCENT_MODEL
                    } else {
                        t::TEXT_PRIMARY
                    })
                    .add_modifier(Modifier::BOLD),
            ),
            span("  ", t::GRAY),
            span(facts, status_color),
        ]));
        let mut detail = row.task.replace('\n', " ");
        if let Some(branch) = &row.branch {
            detail = format!("{branch} · {detail}");
        }
        if row.pending_messages > 0 {
            detail = format!("{} message(s) waiting · {detail}", row.pending_messages);
        }
        lines.push(Line::from(span(
            format!("    {}", truncate(&detail, width.saturating_sub(4))),
            t::GRAY,
        )));
    }
    lines.push(Line::default());
    lines.push(Line::from(span(
        "↑↓ choose · Enter open · s stop · m message · r resume · Esc close",
        t::GRAY,
    )));
    lines
}

pub(super) fn render(frame: &mut Frame, area: Rect, app: &App) {
    let lines = lines(app, area.width);
    let selected = app.agents_panel.map_or(0, |panel| panel.selected);
    // Keep the chosen row (two lines per agent, after the two title lines)
    // on screen.
    let row_line = 2 + selected * 2;
    let height = usize::from(area.height);
    let scroll = row_line.saturating_add(3).saturating_sub(height);
    frame.render_widget(
        Paragraph::new(lines).scroll((u16::try_from(scroll).unwrap_or(u16::MAX), 0)),
        area,
    );
}
