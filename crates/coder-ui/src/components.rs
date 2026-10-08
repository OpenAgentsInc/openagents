//! Pure Coder compositions over validated Rust Native semantic values.

pub mod brainstorm;
pub mod conversation;
pub mod diff;
pub mod markdown;
pub mod syntax;

use crate::{catalog::CatalogIntent, source_theme as t};
use rust_native::{
    style::{Color, TextWeight},
    view::{Axis, Element, Node, RichRun, TextRole},
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

pub type Component = Node<CatalogIntent>;

pub fn node(key: impl Into<String>, element: Element<CatalogIntent>) -> Component {
    Node {
        key: key.into(),
        style: t::style(),
        element,
    }
}

pub fn column(key: impl Into<String>, children: Vec<Component>) -> Component {
    node(
        key,
        Element::Stack {
            axis: Axis::Vertical,
            children,
        },
    )
}

pub fn row(key: impl Into<String>, children: Vec<Component>) -> Component {
    node(
        key,
        Element::Stack {
            axis: Axis::Horizontal,
            children,
        },
    )
}

pub fn text(key: impl Into<String>, value: impl Into<String>, color: Color) -> Component {
    let mut node = node(
        key,
        Element::Text {
            value: value.into(),
            role: TextRole::Terminal,
        },
    );
    node.style.foreground = Some(color);
    node.style.min_height = Some(20);
    node
}

pub fn run(text: impl Into<String>, color: Color) -> RichRun {
    RichRun {
        text: text.into(),
        foreground: Some(color),
        ..RichRun::default()
    }
}

pub fn rich(key: impl Into<String>, runs: Vec<RichRun>) -> Component {
    let mut node = node(
        key,
        Element::RichText {
            runs,
            role: TextRole::Terminal,
        },
    );
    node.style.min_height = Some(20);
    node
}

pub fn blank(key: impl Into<String>) -> Component {
    text(key, " ", t::TEXT_SECONDARY)
}

pub fn action(
    key: impl Into<String>,
    label: impl Into<String>,
    intent: CatalogIntent,
) -> Component {
    node(
        key,
        Element::Button {
            label: label.into(),
            enabled: true,
            icon: None,
            shortcut: None,
            intent,
        },
    )
}

pub fn choice(
    key: impl Into<String>,
    label: impl Into<String>,
    selected: bool,
    intent: CatalogIntent,
    children: Vec<Component>,
) -> Component {
    node(
        key,
        Element::Choice {
            label: label.into(),
            selected,
            enabled: true,
            intent,
            children,
        },
    )
}

pub fn field(
    key: impl Into<String>,
    label: impl Into<String>,
    value: impl Into<String>,
    secret: bool,
    multiline: bool,
    intent: CatalogIntent,
) -> Component {
    node(
        key,
        Element::Field {
            label: label.into(),
            value: if secret { String::new() } else { value.into() },
            placeholder: String::new(),
            secret,
            multiline,
            enabled: true,
            max_bytes: 8_192,
            on_change: intent,
        },
    )
}

pub fn truncate(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.into();
    }
    let mut result = String::new();
    let mut used = 0;
    for grapheme in text.graphemes(true) {
        let cells = grapheme.width();
        if used + cells >= width {
            break;
        }
        result.push_str(grapheme);
        used += cells;
    }
    if width > 0 {
        result.push('…');
    }
    result
}

/// Wrap without splitting a grapheme or losing any source style.
pub fn wrap(runs: &[RichRun], width: usize) -> Vec<Vec<RichRun>> {
    let text = runs.iter().map(|run| run.text.as_str()).collect::<String>();
    wrap_ranges(&text, width)
        .into_iter()
        .map(|range| {
            let mut offset = 0;
            runs.iter()
                .filter_map(|run| {
                    let end = offset + run.text.len();
                    let start = offset.max(range.start);
                    let stop = end.min(range.end);
                    let piece = (start < stop).then(|| {
                        let mut piece = run.clone();
                        piece.text = run.text[start - offset..stop - offset].into();
                        piece
                    });
                    offset = end;
                    piece
                })
                .collect()
        })
        .collect()
}

/// Word wrapping follows the public terminal contract; ranges are grapheme safe.
pub fn wrap_ranges(text: &str, width: usize) -> Vec<std::ops::Range<usize>> {
    let mut rows = Vec::new();
    let width = width.max(1);
    let mut start = 0;
    loop {
        let end = text[start..]
            .find('\n')
            .map_or(text.len(), |offset| start + offset);
        let mut cursor = start;
        loop {
            let mut cells = 0;
            let mut stop = cursor;
            let mut space = None;
            let mut overflow = false;
            for (offset, grapheme) in text[cursor..end].grapheme_indices(true) {
                let at = cursor + offset;
                let next = grapheme.width().max(1);
                if cells + next > width {
                    overflow = true;
                    break;
                }
                if grapheme == " " && at > cursor {
                    space = Some(at);
                }
                cells += next;
                stop = at + grapheme.len();
            }
            if !overflow {
                rows.push(cursor..end);
                break;
            }
            let (row_end, mut next) = if let Some(space) = space {
                (space, space + 1)
            } else if stop > cursor {
                (stop, stop)
            } else {
                let next = cursor + text[cursor..end].graphemes(true).next().map_or(0, str::len);
                (next, next)
            };
            rows.push(cursor..row_end);
            while next < end && text[next..].starts_with(' ') {
                next += 1;
            }
            cursor = next;
            if cursor >= end {
                break;
            }
        }
        if end == text.len() {
            break;
        }
        start = end + 1;
    }
    rows
}

pub fn rows(key: &str, lines: Vec<Vec<RichRun>>) -> Component {
    column(
        key,
        lines
            .into_iter()
            .enumerate()
            .map(|(i, runs)| rich(format!("{key}-{i}"), runs))
            .collect(),
    )
}

pub fn bold(mut run: RichRun) -> RichRun {
    run.bold = true;
    run
}

pub fn selected(mut node: Component) -> Component {
    fn band(node: &mut Component) {
        node.style.background = Some(t::BG_DARK);
        if let Element::Stack { children, .. } | Element::Choice { children, .. } =
            &mut node.element
        {
            for child in children {
                band(child);
            }
        }
    }
    band(&mut node);
    node.style.weight = Some(TextWeight::Bold);
    node
}

/// Source timer formatting, retained independently of the animation clock.
pub fn elapsed(seconds: u64) -> String {
    let minutes = seconds / 60 % 60;
    let hours = seconds / 3_600;
    if hours >= 24 {
        format!("{}d {}h {minutes}m", hours / 24, hours % 24)
    } else if hours > 0 {
        format!("{hours}h {minutes}m {}s", seconds % 60)
    } else if seconds >= 60 {
        format!("{minutes}m {}s", seconds % 60)
    } else {
        format!("{seconds}s")
    }
}

pub fn spinner(phase: u8) -> &'static str {
    ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"][usize::from(phase) % 8]
}

pub fn pulse(phase: u8) -> Color {
    let level = [100_u16, 85, 65, 45, 35, 55, 75, 95][usize::from(phase) % 8];
    Color::rgb(
        (u16::from(t::ACCENT_DELEGATE.red) * level / 100) as u8,
        0,
        (u16::from(t::ACCENT_DELEGATE.blue) * level / 100) as u8,
    )
}
