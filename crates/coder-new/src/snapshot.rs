//! SVG export from the actual Ratatui buffer, without entering a terminal.

use std::fmt::Write;

use ratatui::{
    Terminal,
    backend::TestBackend,
    style::{Color, Modifier},
};

use crate::{App, agents::AgentView, theme as t, ui};

pub fn svg(app: &mut App, width: u16, height: u16) -> String {
    let mut terminal =
        Terminal::new(TestBackend::new(width, height)).expect("the memory backend is infallible");
    terminal
        .draw(|frame| ui::render(frame, app))
        .expect("the memory backend is infallible");
    let buffer = terminal.backend().buffer();
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\" role=\"img\">\n<title>Coder terminal UI preview</title>\n<rect width=\"100%\" height=\"100%\" fill=\"{}\"/>\n<g font-family=\"DejaVu Sans Mono, monospace\" font-size=\"15\">\n",
        u32::from(width) * 9,
        u32::from(height) * 20,
        u32::from(width) * 9,
        u32::from(height) * 20,
        hex(t::BG_BASE)
    );
    for y in 0..height {
        for x in 0..width {
            let cell = &buffer[(x, y)];
            if cell.bg != t::BG_BASE && cell.bg != Color::Reset {
                writeln!(
                    svg,
                    "<rect x=\"{}\" y=\"{}\" width=\"9\" height=\"20\" fill=\"{}\"/>",
                    u32::from(x) * 9,
                    u32::from(y) * 20,
                    hex(cell.bg)
                )
                .expect("writing to a String succeeds");
            }
            if cell.symbol().trim().is_empty() {
                continue;
            }
            let weight = if cell.modifier.contains(Modifier::BOLD) {
                "bold"
            } else {
                "normal"
            };
            writeln!(
                svg,
                "<text x=\"{}\" y=\"{}\" fill=\"{}\" font-weight=\"{weight}\">{}</text>",
                u32::from(x) * 9,
                u32::from(y) * 20 + 15,
                hex(cell.fg),
                escape(cell.symbol())
            )
            .expect("writing to a String succeeds");
        }
    }
    svg.push_str("</g>\n");
    if width >= 24 && height >= 12 && app.agents.view == AgentView::Composer {
        let cursor = terminal
            .get_cursor_position()
            .expect("the memory backend is infallible");
        writeln!(
            svg,
            "<rect x=\"{}\" y=\"{}\" width=\"2\" height=\"20\" fill=\"{}\"/>",
            u32::from(cursor.x) * 9,
            u32::from(cursor.y) * 20,
            hex(t::TEXT_SECONDARY)
        )
        .expect("writing to a String succeeds");
    }
    svg.push_str("</svg>\n");
    svg
}

fn hex(color: Color) -> String {
    match color {
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        _ => "#c8c8c8".into(),
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
