//! Terminal user interface and frame rendering adapted from grok-build/ratatui

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

pub struct BoxFrame {
    pub title: String,
    pub status: String,
}

impl BoxFrame {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            status: "ready".to_string(),
        }
    }

    pub fn render(&self, f: &mut Frame, area: Rect, content: &str) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(5),
                Constraint::Length(3),
            ])
            .split(area);

        // Header
        let header = Paragraph::new(Line::from(vec![
            Span::styled(" OpenAgents ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("│ "),
            Span::styled(&self.title, Style::default().fg(Color::White)),
        ]))
        .block(Block::default().borders(Borders::ALL).title("Agent Context"));
        f.render_widget(header, chunks[0]);

        // Main Body
        let body = Paragraph::new(content)
            .block(Block::default().borders(Borders::ALL).title("Transcript"))
            .wrap(Wrap { trim: true });
        f.render_widget(body, chunks[1]);

        // Footer / Status Bar
        let footer = Paragraph::new(Line::from(vec![
            Span::styled(" Status: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&self.status, Style::default().fg(Color::Green)),
            Span::raw(" │ "),
            Span::styled("Tab: effort │ Shift+Tab: lane │ Esc: exit", Style::default().fg(Color::DarkGray)),
        ]))
        .block(Block::default().borders(Borders::ALL));
        f.render_widget(footer, chunks[2]);
    }
}
