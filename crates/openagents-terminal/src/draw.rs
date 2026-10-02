//! One frame: the transcript above, the composer at the foot with its
//! status rails, and a list over both when one is open. Every color comes
//! from the white ladder on near-black.

use coder_terminal::components::overlay::{Item, ListOverlay};
use coder_terminal::components::{run, turn};
use coder_terminal::{Composer, PROMPT, frame_for};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;

use crate::app::{App, Overlay};
use crate::rows::Row;

/// The smallest screen the frame draws on.
pub const MIN_WIDTH: u16 = 12;
/// The fewest rows: the composer's three and one of transcript.
pub const MIN_HEIGHT: u16 = 4;

/// Draws `app` into `buf` over `area`; the caret's position comes back for
/// the terminal's own cursor.
pub fn draw(app: &mut App, area: Rect, buf: &mut Buffer) -> (u16, u16) {
    let ladder = app.ladder;
    // Too small for the composer's frame: draw nothing until it grows.
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        return (area.x, area.y);
    }
    let status = app.status();
    let tail = app.tail();
    let busy = app.busy();
    let prompt = if busy { frame_for(app.tick) } else { PROMPT };
    let mut composer = Composer::new(&mut app.editor, ladder)
        .prompt(prompt)
        .status(&status)
        .tokens(&tail);
    let box_height = composer.height(area.width).min(area.height);
    let log = Rect::new(
        area.x,
        area.y,
        area.width,
        area.height.saturating_sub(box_height),
    );
    let composer_area = Rect::new(area.x, area.y + log.height, area.width, box_height);

    let base = Style::new().bg(ladder.background());
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            buf[(x, y)].reset();
            buf[(x, y)].set_style(base);
        }
    }

    // The live rows under the transcript: the reply streaming in, and the
    // run's latest progress.
    let width = log.width.saturating_sub(1);
    let mut live: Vec<Line<'static>> = Vec::new();
    if !app.partial.is_empty() {
        live.extend(turn::streaming(
            &app.partial,
            frame_for(app.tick),
            width,
            ladder,
        ));
    }
    if let Some(progress) = &app.progress {
        live.extend(run::lines(progress, width, ladder));
    }
    let scroll = app.scroll;
    let rows = app.transcript.rows(usize::from(width), |_: &Row| true);
    let mut all: Vec<&Line<'static>> = rows;
    all.extend(live.iter());
    let shown = usize::from(log.height);
    let scroll = scroll.min(all.len().saturating_sub(shown));
    let end = all.len().saturating_sub(scroll);
    let start = end.saturating_sub(shown);
    for (offset, line) in all[start..end].iter().enumerate() {
        buf.set_line(log.x + 1, log.y + offset as u16, line, width);
    }
    app.scroll = scroll;

    let caret = composer.render(composer_area, buf);

    if let Some(overlay) = &app.overlay {
        let (title, items, selected, hint, empty) = match overlay {
            Overlay::Threads {
                rows,
                selected,
                query,
            } => (
                if query.is_empty() {
                    "Threads".to_owned()
                } else {
                    format!("Threads · {query}")
                },
                crate::app::shown_threads(rows, query)
                    .into_iter()
                    .map(|row| Item {
                        label: if row.title.trim().is_empty() {
                            "New thread".to_owned()
                        } else {
                            row.title.clone()
                        },
                        detail: detail(row, &app.thread),
                    })
                    .collect::<Vec<_>>(),
                *selected,
                "Type to search · Enter open · Ctrl+N new · Ctrl+A archive · Esc close",
                if query.is_empty() {
                    "No threads yet. Press Ctrl+N for a new one."
                } else {
                    "No threads match."
                },
            ),
            Overlay::Plugins { rows, selected } => (
                "Plugins".to_owned(),
                rows.iter()
                    .map(|(name, about)| Item {
                        label: name.clone(),
                        detail: about.clone(),
                    })
                    .collect(),
                *selected,
                "Ask about one in the chat · Esc close",
                "No plugins are published yet.",
            ),
            Overlay::Settings { settings, selected } => (
                "Settings".to_owned(),
                settings
                    .choices
                    .iter()
                    .map(|choice| Item {
                        label: choice.label.clone(),
                        detail: if choice.on { "on" } else { "off" }.to_owned(),
                    })
                    .collect(),
                *selected,
                "Enter turns it on or off · Esc close",
                settings
                    .problem
                    .as_deref()
                    .unwrap_or("There are no settings to change here."),
            ),
        };
        ListOverlay {
            title: &title,
            items: &items,
            selected,
            hint,
            empty,
            ladder,
        }
        .render(area, buf);
    }
    caret
}

/// A thread row's detail: open now, Coder, archived, and when.
fn detail(row: &openagents_chat::basic_chats::Summary, open: &str) -> String {
    let mut parts = Vec::new();
    if row.id == open {
        parts.push("open".to_owned());
    }
    if row.pinned {
        parts.push("pinned".to_owned());
    }
    if let Some(coder) = &row.coder {
        parts.push(match &coder.project {
            Some(project) => format!("Coder in {project}"),
            None => "Coder".to_owned(),
        });
    }
    if row.archived {
        parts.push("archived".to_owned());
    }
    parts.push(ago(row.updated, now()));
    parts.join(" · ")
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// "just now", "5m ago", "3h ago", "2d ago".
pub fn ago(then: u64, now: u64) -> String {
    let seconds = now.saturating_sub(then);
    match seconds {
        0..60 => "just now".into(),
        60..3_600 => format!("{}m ago", seconds / 60),
        3_600..86_400 => format!("{}h ago", seconds / 3_600),
        _ => format!("{}d ago", seconds / 86_400),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ages_read_plainly() {
        assert_eq!(ago(100, 130), "just now");
        assert_eq!(ago(0, 300), "5m ago");
        assert_eq!(ago(0, 3 * 3_600), "3h ago");
        assert_eq!(ago(0, 2 * 86_400 + 5), "2d ago");
        assert_eq!(ago(500, 100), "just now");
    }
}
