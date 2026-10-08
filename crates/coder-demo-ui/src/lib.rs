//! Portable terminal-cell rendering for Coder's local demo.

mod slash;
mod theme;
mod tools;
mod ui;

pub use coder_ui::demo::{
    DemoState as App, Draft, Mode, Screen, agents, brainstorm, cloud_settings, models,
    plugin_definition, plugins,
};
pub use ratatui::style::{Color, Modifier};
use ratatui::{
    Terminal,
    backend::TestBackend,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
};
use rust_native::view::RichRun;
pub use ui::render;

/// A terminal cell from the original Ratatui renderer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    pub symbol: String,
    pub foreground: Color,
    pub background: Color,
    pub modifiers: Modifier,
    pub skip: bool,
}

/// An original terminal frame. Cells are in row order, including wide-character continuations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub width: u16,
    pub height: u16,
    pub cells: Vec<Cell>,
    pub cursor: Option<(u16, u16)>,
}

/// Render without a terminal, a runtime, or access to the filesystem.
pub fn capture(app: &mut App, width: u16, height: u16) -> Snapshot {
    let width = width.clamp(1, 240);
    let height = height.clamp(1, 160);
    let mut terminal =
        Terminal::new(TestBackend::new(width, height)).expect("the memory backend is infallible");
    let buffer = terminal
        .draw(|frame| render(frame, app))
        .expect("the memory backend is infallible")
        .buffer
        .clone();
    let position = terminal
        .get_cursor_position()
        .expect("the memory backend is infallible");
    Snapshot {
        width,
        height,
        cells: buffer
            .content
            .iter()
            .map(|cell| Cell {
                symbol: cell.symbol().to_owned(),
                foreground: cell.fg,
                background: cell.bg,
                modifiers: cell.modifier,
                #[allow(deprecated)]
                skip: cell.skip,
            })
            .collect(),
        cursor: (terminal.backend().cursor_visible() && app.cursor_blink_frame < 4)
            .then_some((position.x, position.y)),
    }
}

/// Clickable rows use the same terminal geometry as the renderer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    Agent(usize),
    Plugin(usize),
    Composer,
}

pub fn hit(app: &App, width: u16, height: u16, x: u16, y: u16) -> Option<Hit> {
    if width < 24 || height < 12 || x >= width || y >= height || app.model_picker.is_some() {
        return None;
    }
    let area = Rect::new(2, 1, width.saturating_sub(4), height.saturating_sub(2));
    if app.screen == Screen::Plugins {
        let start = area.y + 2 + u16::from(area.width >= 56);
        let index = usize::from(y.checked_sub(start)?);
        return (index < plugin_definition::DEFINITIONS.len()).then_some(Hit::Plugin(index));
    }
    if app.screen != Screen::Conversation {
        return None;
    }
    let (draft, _) = app.draft.wrapped(width.saturating_sub(3));
    let rail_height = if app.mode == Mode::Demo {
        agents::DEMOS
            .len()
            .min(usize::from(area.height.saturating_sub(6))) as u16
    } else {
        0
    };
    let composer_height = (draft.len() as u16).clamp(1, 6) + 2;
    let [_, _, composer, _, rail] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(composer_height.min(area.height.saturating_sub(rail_height + 3))),
        Constraint::Length(1),
        Constraint::Length(rail_height),
    ])
    .areas(area);
    if y >= composer.y && y < composer.bottom() {
        return Some(Hit::Composer);
    }
    if y >= rail.y && y < rail.bottom() {
        let first = app
            .selected_agent
            .unwrap_or(agents::DEMOS.len().saturating_sub(1))
            .saturating_sub(usize::from(rail.height).saturating_sub(1));
        let index = first + usize::from(y - rail.y);
        return (index < agents::DEMOS.len()).then_some(Hit::Agent(index));
    }
    None
}

fn rich_lines(rows: Vec<Vec<RichRun>>) -> Vec<Line<'static>> {
    rows.into_iter()
        .map(|row| {
            Line::from(
                row.into_iter()
                    .map(|run| {
                        let mut style = Style::default();
                        let rgb = |c: rust_native::style::Color| Color::Rgb(c.red, c.green, c.blue);
                        style.fg = run.foreground.map(rgb);
                        style.bg = run.background.map(rgb);
                        for (enabled, modifier) in [
                            (run.bold, Modifier::BOLD),
                            (run.italic, Modifier::ITALIC),
                            (run.underline, Modifier::UNDERLINED),
                            (run.strike, Modifier::CROSSED_OUT),
                            (run.dim, Modifier::DIM),
                        ] {
                            if enabled {
                                style = style.add_modifier(modifier);
                            }
                        }
                        Span::styled(run.text, style)
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect()
}

fn rail(
    area: Rect,
    buffer: &mut ratatui::buffer::Buffer,
    offset: u16,
    left: Option<(&str, Style)>,
    right: Option<(&str, Style)>,
) {
    use unicode_width::UnicodeWidthStr;
    let y = area.top() + offset;
    if y >= buffer.area.bottom() || area.width < 6 {
        return;
    }
    let mut room = usize::from(area.width) - 4;
    if let Some((text, style)) = left.filter(|(text, _)| !text.is_empty()) {
        let named = format!(" {text} ");
        let taken = named.width();
        if taken <= room {
            room -= taken;
            buffer.set_string(area.left() + 2, y, named, style);
        }
    }
    if let Some((text, style)) = right.filter(|(text, _)| !text.is_empty()) {
        let named = format!(" {text} ");
        let taken = named.width();
        if taken <= room {
            buffer.set_string(area.right() - 2 - taken as u16, y, named, style);
        }
    }
}

mod html;
pub use html::{html, html_row};
mod svg;
pub use svg::svg;
