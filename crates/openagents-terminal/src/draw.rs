//! One frame: the transcript above, the composer at the foot with its
//! status rails, and a list over both when one is open. Every color comes
//! from the white ladder on near-black.

use coder_terminal::components::overlay::{Item, ListOverlay};
use coder_terminal::components::{rail, run, turn};
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
    // The rail of Coder runs under the composer (#10169), leaving the
    // transcript at least one row. The same cells as the transcript: two
    // in from the left, two free at the right.
    let rail_width = area.width.saturating_sub(3);
    let rail_rows = if app.rail_shown() {
        rail::lines(&app.rail_rows(), usize::from(rail_width), app.tick, ladder)
    } else {
        Vec::new()
    };
    // A run from the rail that is not the thread's current one: its own
    // log, without the current run's live rows.
    let other = app
        .viewed()
        .filter(|held| app.task.as_deref() != Some(held.task.as_str()))
        .and(app.run_view.and_then(|view| view.number));
    let mut composer = Composer::new(&mut app.editor, ladder)
        .prompt(prompt)
        .status(&status)
        .tokens(&tail);
    let box_height = composer.height(area.width).min(area.height);
    let rail_height = u16::try_from(rail_rows.len())
        .unwrap_or(u16::MAX)
        .min(area.height.saturating_sub(box_height + 1));
    let log = Rect::new(
        area.x,
        area.y,
        area.width,
        area.height.saturating_sub(box_height + rail_height),
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
    // One cell at the left edge and two at the right, so rows end before
    // the window's edge (owner, 2026-10-02).
    let width = log.width.saturating_sub(3);
    let mut live: Vec<Line<'static>> = Vec::new();
    let viewing = app.run_view.is_some();
    if !app.partial.is_empty() && !viewing {
        live.extend(turn::streaming(&app.partial, width, ladder));
    }
    if other.is_none() {
        if let Some(progress) = &app.progress {
            live.extend(run::lines(progress, width, ladder));
        }
        live.extend(working_line);
    }
    let scroll = match app.run_view {
        Some(view) => view.scroll,
        None => app.scroll,
    };
    let rows = if let Some(number) = other {
        app.delegations[number - 1]
            .log
            .rows(usize::from(width), |_: &Row| true)
    } else if viewing {
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
    for (offset, line) in rail_rows.iter().take(usize::from(rail_height)).enumerate() {
        buf.set_line(
            area.x + 1,
            composer_area.bottom() + offset as u16,
            line,
            rail_width,
        );
    }

    if let Some(overlay) = &app.overlay {
        let (title, items, selected, hint, empty): (String, Vec<Item>, _, _, _) = match overlay {
            Overlay::Threads(picker) => {
                picker.render(area, buf, ladder, &app.thread, now());
                return caret;
            }
            Overlay::Plugins { rows, selected } => (
                "Plugins".to_owned(),
                rows.iter()
                    .map(|plugin| Item {
                        label: plugin.name.clone(),
                        detail: match (plugin.on, plugin.key.is_some()) {
                            (Some(true), _) => format!("on · {}", plugin.about),
                            (Some(false), _) => format!("off · {}", plugin.about),
                            (None, true) => format!("installed · {}", plugin.about),
                            (None, false) => plugin.about.clone(),
                        },
                    })
                    .collect(),
                *selected,
                "Enter runs an installed one or installs a published one · Space turns it on or off · Esc close",
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
            Overlay::Background { rows, selected } => (
                "Background".to_owned(),
                rows.iter()
                    .map(|row| Item {
                        label: row.id.clone(),
                        detail: row.line.clone(),
                    })
                    .collect(),
                *selected,
                "Enter show · r run (dry run first) · p pause or resume · l log · Esc close",
                "No background rules.",
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

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}
