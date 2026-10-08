//! SVG export from the actual Ratatui buffer, without entering a terminal.

use std::fmt::Write;
use unicode_width::UnicodeWidthStr;

use crate::{Color, Modifier, Snapshot, theme as t};

pub fn svg(snapshot: &Snapshot) -> String {
    let width = snapshot.width;
    let height = snapshot.height;
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\" role=\"img\">\n<title>Coder terminal UI preview</title>\n<rect width=\"100%\" height=\"100%\" fill=\"{}\"/>\n<g font-family=\"Paper Mono, monospace\" font-size=\"15\">\n",
        u32::from(width) * 9,
        u32::from(height) * 20,
        u32::from(width) * 9,
        u32::from(height) * 20,
        hex(t::BG_BASE)
    );
    for y in 0..height {
        for x in 0..width {
            let cell = &snapshot.cells[usize::from(y) * usize::from(width) + usize::from(x)];
            if cell.background != t::BG_BASE && cell.background != Color::Reset {
                writeln!(
                    svg,
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"20\" fill=\"{}\"/>",
                    u32::from(x) * 9,
                    u32::from(y) * 20,
                    cell.symbol.width().max(1).min(usize::from(width - x)) * 9,
                    hex(cell.background)
                )
                .expect("writing to a String succeeds");
            }
        }
    }
    // Each row is one text selection. Exact cell positions and styles stay in
    // tspans; literal spaces remain available to the browser's clipboard.
    for y in 0..height {
        write!(
            svg,
            "<text y=\"{}\" xml:space=\"preserve\">",
            u32::from(y) * 20 + 15
        )
        .expect("writing to a String succeeds");
        let mut covered_until = 0;
        for x in 0..width {
            if x < covered_until {
                continue;
            }
            let cell = &snapshot.cells[usize::from(y) * usize::from(width) + usize::from(x)];
            covered_until = x + cell.symbol.width().max(1).min(usize::from(width - x)) as u16;
            let weight = if cell.modifiers.contains(Modifier::BOLD) {
                "bold"
            } else {
                "normal"
            };
            let italic = if cell.modifiers.contains(Modifier::ITALIC) {
                " font-style=\"italic\""
            } else {
                ""
            };
            let decoration = match (
                cell.modifiers.contains(Modifier::UNDERLINED),
                cell.modifiers.contains(Modifier::CROSSED_OUT),
            ) {
                (true, true) => " text-decoration=\"underline line-through\"",
                (true, false) => " text-decoration=\"underline\"",
                (false, true) => " text-decoration=\"line-through\"",
                (false, false) => "",
            };
            let dim = if cell.modifiers.contains(Modifier::DIM) {
                " opacity=\"0.5\""
            } else {
                ""
            };
            write!(
                svg,
                "<tspan x=\"{}\" y=\"{}\" fill=\"{}\" font-weight=\"{weight}\"{italic}{decoration}{dim}>{}</tspan>",
                u32::from(x) * 9,
                u32::from(y) * 20 + 15,
                hex(cell.foreground),
                escape(cell.symbol.as_str())
            )
            .expect("writing to a String succeeds");
        }
        svg.push_str("</text>\n");
    }
    if let Some((cursor_x, cursor_y)) = snapshot.cursor {
        writeln!(
            svg,
            "<g style=\"user-select:none;pointer-events:none\"><rect x=\"{}\" y=\"{}\" width=\"9\" height=\"20\" fill=\"{}\"/>",
            u32::from(cursor_x) * 9,
            u32::from(cursor_y) * 20,
            hex(t::CURSOR)
        )
        .expect("writing to a String succeeds");
        let cell =
            &snapshot.cells[usize::from(cursor_y) * usize::from(width) + usize::from(cursor_x)];
        if !cell.symbol.as_str().trim().is_empty() {
            writeln!(
                svg,
                "<text x=\"{}\" y=\"{}\" fill=\"{}\">{}</text>",
                u32::from(cursor_x) * 9,
                u32::from(cursor_y) * 20 + 15,
                hex(t::CURSOR_TEXT),
                escape(cell.symbol.as_str())
            )
            .expect("writing to a String succeeds");
        }
        svg.push_str("</g>\n");
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
