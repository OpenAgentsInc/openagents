//! Browser presentation as selectable HTML text with contiguous style runs.

use crate::{
    Cell, Color, Modifier, Snapshot,
    svg::{escape, hex},
    theme,
};
use std::fmt::Write;
use unicode_width::UnicodeWidthStr;

/// One row's inner markup, grouping adjacent cells instead of creating a node per cell.
pub fn html_row(cells: &[Cell]) -> String {
    let mut html = String::new();
    let mut x = 0;
    while x < cells.len() {
        let first = &cells[x];
        let start = x;
        let mut text = String::new();
        while x < cells.len() {
            let cell = &cells[x];
            if cell.foreground != first.foreground
                || cell.background != first.background
                || cell.modifiers != first.modifiers
            {
                break;
            }
            text.push_str(&cell.symbol);
            x += cell.symbol.width().max(1).min(cells.len() - x);
        }
        let mut foreground = first.foreground;
        let mut background = if first.background == Color::Reset {
            theme::BG_BASE
        } else {
            first.background
        };
        if first.modifiers.contains(Modifier::REVERSED) {
            std::mem::swap(&mut foreground, &mut background);
        }
        let mut style = format!(
            "width:{}px;color:{};background:{};",
            (x - start) * 9,
            hex(foreground),
            hex(background)
        );
        if first.modifiers.contains(Modifier::BOLD) {
            style.push_str("font-weight:700;");
        }
        if first.modifiers.contains(Modifier::ITALIC) {
            style.push_str("font-style:italic;");
        }
        if first.modifiers.contains(Modifier::DIM) {
            style.push_str("opacity:.5;");
        }
        let underline = first.modifiers.contains(Modifier::UNDERLINED);
        let strike = first.modifiers.contains(Modifier::CROSSED_OUT);
        if underline || strike {
            style.push_str("text-decoration:");
            if underline {
                style.push_str(" underline");
            }
            if strike {
                style.push_str(" line-through");
            }
            style.push(';');
        }
        write!(
            html,
            "<span class=\"demo-run\" style=\"{style}\">{}</span>",
            escape(&text)
        )
        .unwrap();
    }
    html
}

/// A complete initial frame; subsequent browser renders replace only changed rows.
pub fn html(snapshot: &Snapshot) -> String {
    let mut html = format!(
        "<div class=\"demo-grid\" style=\"width:{}px;height:{}px\">",
        u32::from(snapshot.width) * 9,
        u32::from(snapshot.height) * 20
    );
    for (y, row) in snapshot
        .cells
        .chunks(usize::from(snapshot.width))
        .enumerate()
    {
        write!(
            html,
            "<div class=\"demo-row\" data-demo-row=\"{y}\">{}</div>",
            html_row(row)
        )
        .unwrap();
    }
    let (style, text) = match snapshot.cursor {
        Some((x, y)) => (
            format!("left:{}px;top:{}px", x * 9, y * 20),
            escape(
                &snapshot.cells[usize::from(y) * usize::from(snapshot.width) + usize::from(x)]
                    .symbol,
            ),
        ),
        None => ("display:none".into(), String::new()),
    };
    write!(
        html,
        "<span class=\"demo-cursor\" aria-hidden=\"true\" style=\"{style}\">{text}</span></div>"
    )
    .unwrap();
    html
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{App, capture};

    #[test]
    fn html_keeps_text_in_bounded_style_runs_and_escapes_markup() {
        let mut app = App::default();
        app.paste("<script> & hello");
        let snapshot = capture(&mut app, 110, 36);
        let html = html(&snapshot);
        assert!(!html.contains("<svg"));
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt; &amp; hello"));
        assert_eq!(html.matches("class=\"demo-row\"").count(), 36);
        assert!(html.matches("class=\"demo-run\"").count() < 500);
        assert!(html.contains("class=\"demo-cursor\""));
    }

    #[test]
    fn wide_characters_consume_their_continuation_cells() {
        let cell = |symbol: &str| Cell {
            symbol: symbol.into(),
            foreground: Color::White,
            background: Color::Black,
            modifiers: Modifier::empty(),
            skip: false,
        };
        let row = html_row(&[cell("界"), cell(" "), cell("!")]);
        assert!(row.contains("width:27px"));
        assert!(row.contains(">界!</span>"));
    }
}
