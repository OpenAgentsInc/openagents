//! One frame: the transcript above, the composer at the foot with its
//! status rails, and a list over both when one is open. Every color comes
//! from the white ladder on near-black.

use coder_terminal::components::overlay::{Item, ListOverlay};
use coder_terminal::components::{run, turn};
use coder_terminal::{Composer, PROMPT, grok_spinner};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;

use crate::app::{App, Overlay};
use crate::rows::Row;
use crate::view::{Shown, paint};

/// Grok Build's spinner frame at animation tick `tick`, as one cell.
fn spinner_char(tick: u64) -> char {
    grok_spinner::frame(tick).chars().next().unwrap_or(PROMPT)
}

/// The line that says what is in progress, as Grok Build draws its
/// "Starting session…": the spinner, the words, and the timer, all in
/// its dim gray.
pub fn working(
    text: &str,
    elapsed: std::time::Duration,
    tick: u64,
    ladder: coder_terminal::Ladder,
) -> Line<'static> {
    let style = grok_spinner::style(ladder);
    Line::from(vec![
        ratatui::text::Span::raw("  "),
        ratatui::text::Span::styled(format!("{} ", grok_spinner::frame(tick)), style),
        ratatui::text::Span::styled(text.to_owned(), style),
        ratatui::text::Span::styled(format!(" {}", grok_spinner::timer(elapsed)), style),
    ])
}

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
    if app.file.is_some() {
        return draw_file(app, area, buf);
    }
    let status = app.status();
    let working_line = app
        .live_status()
        .map(|(text, elapsed)| working(text, elapsed, app.tick, ladder));
    let tail = app.tail();
    let busy = app.busy();
    let prompt = if busy { spinner_char(app.tick) } else { PROMPT };
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
    // run's latest progress. The run view shows the run alone.
    let width = log.width.saturating_sub(1);
    let mut live: Vec<Line<'static>> = Vec::new();
    let viewing = app.run_view.is_some();
    if !app.partial.is_empty() && !viewing {
        live.extend(turn::streaming(&app.partial, width, ladder));
    }
    if let Some(progress) = &app.progress {
        live.extend(run::lines(progress, width, ladder));
    }
    live.extend(working_line);
    let scroll = match app.run_view {
        Some(view) => view.scroll,
        None => app.scroll,
    };
    let rows = if viewing {
        app.run_log.rows(usize::from(width), |_: &Row| true)
    } else {
        app.transcript.rows(usize::from(width), |_: &Row| true)
    };
    let mut all: Vec<&Line<'static>> = rows;
    all.extend(live.iter());
    let shown = usize::from(log.height);
    let scroll = scroll.min(all.len().saturating_sub(shown));
    let end = all.len().saturating_sub(scroll);
    let start = end.saturating_sub(shown);
    for (offset, line) in all[start..end].iter().enumerate() {
        buf.set_line(log.x + 1, log.y + offset as u16, line, width);
    }
    match &mut app.run_view {
        Some(view) => view.scroll = scroll,
        None => app.scroll = scroll,
    }
    app.shown = Shown::capture(buf, log);
    if let Some(selection) = &app.selection {
        paint(buf, log, selection, ladder);
    }

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
                    .map(|plugin| Item {
                        label: plugin.name.clone(),
                        detail: if plugin.key.is_some() {
                            format!("installed · {}", plugin.about)
                        } else {
                            plugin.about.clone()
                        },
                    })
                    .collect(),
                *selected,
                "Enter runs an installed one · Esc close",
                "No plugins are installed or published yet.",
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

/// The file view: its path on the top row, then the file from its first
/// shown line, over the whole screen.
fn draw_file(app: &mut App, area: Rect, buf: &mut Buffer) -> (u16, u16) {
    let ladder = app.ladder;
    let base = Style::new().bg(ladder.background());
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            buf[(x, y)].reset();
            buf[(x, y)].set_style(base);
        }
    }
    let Some(file) = &app.file else {
        return (area.x, area.y);
    };
    let width = area.width.saturating_sub(1);
    let title = Line::styled(
        format!("{} · read only · Esc closes", file.path),
        ladder.style(coder_terminal::Intensity::Half),
    );
    buf.set_line(area.x + 1, area.y, &title, width);
    let body = Rect::new(
        area.x,
        area.y + 1,
        area.width,
        area.height.saturating_sub(1),
    );
    for (offset, line) in file
        .lines
        .iter()
        .skip(file.top)
        .take(usize::from(body.height))
        .enumerate()
    {
        buf.set_line(body.x + 1, body.y + offset as u16, line, width);
    }
    app.shown = Shown::capture(buf, body);
    if let Some(selection) = &app.selection {
        paint(buf, body, selection, ladder);
    }
    (area.x, area.y)
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
