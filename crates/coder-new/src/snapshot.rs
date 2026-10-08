//! SVG export from the actual Ratatui buffer, without entering a terminal.

use std::fmt::Write;

use ratatui::{
    Terminal,
    backend::TestBackend,
    style::{Color, Modifier},
};

use crate::{App, theme as t, ui};

pub fn svg(app: &mut App, width: u16, height: u16) -> String {
    let mut terminal =
        Terminal::new(TestBackend::new(width, height)).expect("the memory backend is infallible");
    terminal
        .draw(|frame| ui::render(frame, app))
        .expect("the memory backend is infallible");
    let cursor = terminal
        .get_cursor_position()
        .expect("the memory backend is infallible");
    let cursor_visible = terminal.backend().cursor_visible();
    let buffer = terminal.backend().buffer();
    let background = if app.appearance.use_system_terminal_background {
        String::new()
    } else {
        format!(
            "<rect width=\"100%\" height=\"100%\" fill=\"{}\"/>\n",
            hex(t::BG_BASE)
        )
    };
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\" role=\"img\">\n<title>Coder terminal UI preview</title>\n{background}<g font-family=\"Paper Mono, monospace\" font-size=\"15\">\n",
        u32::from(width) * 9,
        u32::from(height) * 20,
        u32::from(width) * 9,
        u32::from(height) * 20,
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
            let italic = if cell.modifier.contains(Modifier::ITALIC) {
                " font-style=\"italic\""
            } else {
                ""
            };
            let decoration = match (
                cell.modifier.contains(Modifier::UNDERLINED),
                cell.modifier.contains(Modifier::CROSSED_OUT),
            ) {
                (true, true) => " text-decoration=\"underline line-through\"",
                (true, false) => " text-decoration=\"underline\"",
                (false, true) => " text-decoration=\"line-through\"",
                (false, false) => "",
            };
            let dim = if cell.modifier.contains(Modifier::DIM) {
                " opacity=\"0.5\""
            } else {
                ""
            };
            writeln!(
                svg,
                "<text x=\"{}\" y=\"{}\" fill=\"{}\" font-weight=\"{weight}\"{italic}{decoration}{dim}>{}</text>",
                u32::from(x) * 9,
                u32::from(y) * 20 + 15,
                hex(cell.fg),
                escape(cell.symbol())
            )
            .expect("writing to a String succeeds");
        }
    }
    if cursor_visible {
        writeln!(
            svg,
            "<g><rect x=\"{}\" y=\"{}\" width=\"9\" height=\"20\" fill=\"{}\"/>",
            u32::from(cursor.x) * 9,
            u32::from(cursor.y) * 20,
            hex(t::CURSOR)
        )
        .expect("writing to a String succeeds");
        let cell = &buffer[(cursor.x, cursor.y)];
        if !cell.symbol().trim().is_empty() {
            writeln!(
                svg,
                "<text x=\"{}\" y=\"{}\" fill=\"{}\">{}</text>",
                u32::from(cursor.x) * 9,
                u32::from(cursor.y) * 20 + 15,
                hex(t::CURSOR_TEXT),
                escape(cell.symbol())
            )
            .expect("writing to a String succeeds");
        }
        svg.push_str("<animate attributeName=\"opacity\" values=\"1;0;1\" keyTimes=\"0;0.5;1\" dur=\"1s\" calcMode=\"discrete\" repeatCount=\"indefinite\"/></g>\n");
    }
    svg.push_str("</g>\n</svg>\n");
    svg
}

fn hex(color: Color) -> String {
    match color {
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        Color::Reset => hex(t::TEXT_SECONDARY),
        color => hex(ansi(color)),
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn ansi(color: Color) -> Color {
    let index = match color {
        Color::Black => 0,
        Color::Red => 1,
        Color::Green => 2,
        Color::Yellow => 3,
        Color::Blue => 4,
        Color::Magenta => 5,
        Color::Cyan => 6,
        Color::Gray => 7,
        Color::DarkGray => 8,
        Color::LightRed => 9,
        Color::LightGreen => 10,
        Color::LightYellow => 11,
        Color::LightBlue => 12,
        Color::LightMagenta => 13,
        Color::LightCyan => 14,
        Color::White => 15,
        Color::Indexed(index) if index < 16 => usize::from(index),
        Color::Indexed(index) if index < 232 => {
            let cube = index - 16;
            let channel = |level: u8| if level == 0 { 0 } else { 55 + level * 40 };
            return Color::Rgb(channel(cube / 36), channel(cube / 6 % 6), channel(cube % 6));
        }
        Color::Indexed(index) => {
            let level = 8 + (index - 232) * 10;
            return Color::Rgb(level, level, level);
        }
        color => return color,
    };
    let color = coder_ui::coder_noir::rgb(coder_ui::coder_noir::ANSI[index]);
    Color::Rgb(color.red, color.green, color.blue)
}
