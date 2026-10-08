//! Slash commands and their suggestions above the composer.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};

use crate::theme as t;

pub use coder_ui::demo::slash::*;

pub fn render(frame: &mut Frame, area: Rect, hints: &[Command], selected: usize, demo: bool) {
    if area.width == 0 || area.height == 0 || hints.is_empty() {
        return;
    }
    let shown = hints.len().min(usize::from(area.height));
    let selected = selected.min(hints.len() - 1);
    let first = selected.saturating_sub(shown - 1);
    let usage_width = hints
        .iter()
        .map(|command| command.word().len() + 1)
        .max()
        .unwrap_or(0);
    frame.render_widget(
        Block::default().style(Style::default().bg(t::BG_BASE)),
        area,
    );
    let top = area.bottom().saturating_sub(shown as u16);
    for (offset, command) in hints.iter().enumerate().skip(first).take(shown) {
        let active = offset == selected;
        let background = if active { t::BG_LIGHT } else { t::BG_BASE };
        let usage = format!("/{}", command.word());
        let line = Line::from(vec![
            Span::styled(
                if active { " ❯ " } else { "   " },
                Style::default().fg(t::ACCENT_MODEL),
            ),
            Span::styled(
                format!("{usage:<usage_width$}  "),
                Style::default()
                    .fg(if active {
                        t::TEXT_PRIMARY
                    } else {
                        t::TEXT_SECONDARY
                    })
                    .add_modifier(if active {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ),
            Span::styled(command.about(demo), Style::default().fg(t::GRAY)),
        ])
        .style(Style::default().bg(background));
        let row = Rect {
            y: top + (offset - first) as u16,
            height: 1,
            ..area
        };
        frame.render_widget(Paragraph::new(line), row);
    }
}
