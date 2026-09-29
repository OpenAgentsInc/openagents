//! One body grid per layout.
//!
//! Each function lays a slide's parts out at the body's width and returns a
//! grid of its own height; [`crate::frame`] centers that grid in the body
//! and draws the chrome around it. The intensity ladder carries every
//! distinction: full for the one thing the slide says, three quarters for
//! prose, half for labels, and quarter for rules.

use crate::banner;
use crate::canvas::Canvas;
use crate::grid::{Arms, Cell, Grid, Style};
use crate::prose::{self, Prose};
use crate::slide::{Layout, Metric, Row, Slide};
use coder_ui::theme::Intensity;

/// The widest a statement sets, in cells. A line longer than this is hard
/// to read from the back of a room.
pub const MEASURE: usize = 64;

/// The body of `slide` at `canvas`.
pub fn body(slide: &Slide, canvas: Canvas) -> Grid {
    let width = canvas.body_cells();
    match slide.layout() {
        Layout::Banner => banner_body(slide, width),
        Layout::Statement => statement(slide, width),
        Layout::Points => titled(slide, Prose::default().layout(&slide.body, width), width),
        Layout::Metrics => titled(slide, metrics(&slide.metrics, width), width),
        Layout::Compare => titled(slide, compare(&slide.columns, &slide.rows, width), width),
        Layout::Flow => titled(slide, flow(&slide.steps, width), width),
        Layout::Quote => quote(slide, width),
        Layout::Ask => titled(slide, ask(slide, width), width),
    }
}

/// The column `inner` starts in to sit centered in `width`.
fn center(width: usize, inner: usize) -> usize {
    width.saturating_sub(inner) / 2
}

/// A line of text centered in `width`.
fn centered_line(grid: &mut Grid, row: usize, text: &str, style: Style) {
    let width = grid.width();
    grid.put_str(center(width, text.chars().count()), row, text, style);
}

/// `laid` copied into `grid` at `row`, each of its lines centered in
/// `width` cells from `left`.
fn centered_lines(grid: &mut Grid, left: usize, row: usize, width: usize, laid: &Grid) {
    for line in 0..laid.height() {
        let used = (0..laid.width())
            .rev()
            .find(|col| laid.get(*col, line).is_some_and(|cell| !cell.is_blank()))
            .map_or(0, |col| col + 1);
        let start = left + center(width, used);
        for col in 0..used {
            if let Some(cell) = laid.get(col, line) {
                grid.put(start + col, row + line, *cell);
            }
        }
    }
}

/// The wordmark in the block face over one line, both centered.
fn banner_body(slide: &Slide, width: usize) -> Grid {
    let name = slide.title.clone().unwrap_or_default().to_uppercase();
    let face = banner_lines(&name, width);
    let lead = slide.lead.clone().unwrap_or_default();
    let kicker = slide.kicker.clone().unwrap_or_default();
    let top = if kicker.is_empty() { 0 } else { 2 };
    let height = top + face.height() + if lead.is_empty() { 0 } else { 2 };
    let mut grid = Grid::new(width, height);
    if !kicker.is_empty() {
        centered_line(&mut grid, 0, &kicker, Style::at(Intensity::Half));
    }
    grid.blit(center(width, face.width()), top, &face);
    if !lead.is_empty() {
        centered_line(
            &mut grid,
            top + face.height() + 1,
            &lead,
            Style::at(Intensity::ThreeQuarters),
        );
    }
    grid
}

/// `name` in the block face, broken between words onto as many lines as
/// it needs to fit `width` cells, each line centered, one blank row
/// between lines.
fn banner_lines(name: &str, width: usize) -> Grid {
    let mut lines: Vec<String> = Vec::new();
    for word in name.split_whitespace() {
        match lines.last_mut() {
            Some(line) if banner::width(&format!("{line} {word}")) <= width => {
                line.push(' ');
                line.push_str(word);
            }
            _ => lines.push(word.to_string()),
        }
    }
    let faces: Vec<Grid> = lines.iter().map(|line| banner::banner(line)).collect();
    let face_width = faces.iter().map(Grid::width).max().unwrap_or(0);
    let height = faces.iter().map(Grid::height).sum::<usize>() + faces.len().saturating_sub(1);
    let mut grid = Grid::new(face_width, height);
    let mut top = 0;
    for face in &faces {
        grid.blit(center(face_width, face.width()), top, face);
        top += face.height() + 1;
    }
    grid
}

/// One sentence at full intensity, in a column centered on the canvas,
/// with the slide's kicker over it.
fn statement(slide: &Slide, width: usize) -> Grid {
    let measure = MEASURE.min(width);
    // The script wraps its source at a comfortable column; a sentence
    // reflows at the measure rather than keeping the file's line breaks.
    let laid = prose::text(&slide.body, Style::at(Intensity::Full), measure);
    let kicker = slide.kicker.clone().unwrap_or_default();
    let top = if kicker.is_empty() { 0 } else { 2 };
    let mut grid = Grid::new(width, top + laid.height());
    if !kicker.is_empty() {
        grid.put_str(
            center(width, measure),
            0,
            &kicker,
            Style::at(Intensity::Half),
        );
    }
    grid.blit(center(width, measure), top, &laid);
    grid
}

/// `body` under the slide's kicker and title, with a hairline between the
/// title and the body. A slide with neither keeps its body alone.
///
/// A slide keeps one full-intensity element. When the body already holds
/// it (a metric, a flow's stages, a table's first column), the title drops
/// to three quarters and keeps its weight; otherwise the title is the one
/// thing the slide says, at full.
fn titled(slide: &Slide, body: Grid, width: usize) -> Grid {
    let title = slide.title.clone().unwrap_or_default();
    let kicker = slide.kicker.clone().unwrap_or_default();
    if title.is_empty() && kicker.is_empty() {
        return body;
    }
    let top = if kicker.is_empty() { 0 } else { 1 };
    let mut grid = Grid::new(width, top + body.height() + 3);
    if !kicker.is_empty() {
        grid.put_str(0, 0, &kicker, Style::at(Intensity::Half));
    }
    let title_intensity = if has_full(&body) {
        Intensity::ThreeQuarters
    } else {
        Intensity::Full
    };
    grid.put_str(0, top, &title, Style::at(title_intensity).bold(true));
    grid.hrule(0, top + 1, width, Style::at(Intensity::Quarter), false);
    grid.blit(0, top + 3, &body);
    grid
}

/// Whether any cell of `grid` draws at full intensity.
pub fn has_full(grid: &Grid) -> bool {
    grid.rows()
        .flatten()
        .any(|cell| !cell.is_blank() && cell.style.intensity == Intensity::Full)
}

/// Numbers side by side, each over its label. A short value draws in the
/// block face; when any value is too long for it, every value draws as
/// text at full intensity. A label wraps inside its column.
fn metrics(cells: &[Metric], width: usize) -> Grid {
    if cells.is_empty() {
        return Grid::new(width, 0);
    }
    let column = width / cells.len();
    let faced = cells.iter().all(|metric| {
        banner::covers(metric.shown()) && banner::width(metric.shown()) + 2 <= column
    });
    let value_rows = if faced { banner::GLYPH_ROWS } else { 1 };
    let labels: Vec<Grid> = cells
        .iter()
        .map(|metric| {
            prose::text(
                &metric.label,
                Style::at(Intensity::Half),
                column.saturating_sub(4).max(1),
            )
        })
        .collect();
    let label_rows = labels.iter().map(Grid::height).max().unwrap_or(0);
    let mut grid = Grid::new(width, value_rows + 1 + label_rows);
    for (index, metric) in cells.iter().enumerate() {
        let left = index * column;
        let shown = metric.shown();
        if faced {
            let face = banner::banner(shown);
            let placed = if metric.is_unfilled() {
                dim(&face, Intensity::Half)
            } else {
                face
            };
            grid.blit(left + center(column, placed.width()), 0, &placed);
        } else {
            let style = if metric.is_unfilled() {
                Style::at(Intensity::Half)
            } else {
                Style::at(Intensity::Full)
            };
            grid.put_str(
                left + center(column, shown.chars().count()),
                0,
                shown,
                style.bold(true),
            );
        }
        centered_lines(&mut grid, left, value_rows + 1, column, &labels[index]);
    }
    grid
}

/// A copy of `grid` with every cell no brighter than `cap`.
fn dim(grid: &Grid, cap: Intensity) -> Grid {
    let mut dimmed = Grid::new(grid.width(), grid.height());
    for row in 0..grid.height() {
        for col in 0..grid.width() {
            if let Some(cell) = grid.get(col, row) {
                dimmed.put(col, row, Cell::new(cell.glyph, cell.style.capped(cap)));
            }
        }
    }
    dimmed
}

/// A table of rows against columns: the first column at full intensity,
/// the rest at three quarters, the row labels and headers at half, and a
/// hairline under the headers. Each column but the last is as wide as its
/// widest cell and a gap; the last takes the rest of the width.
fn compare(columns: &[String], rows: &[Row], width: usize) -> Grid {
    if rows.is_empty() {
        return Grid::new(width, 0);
    }
    const GAP: usize = 3;
    let label_width = rows
        .iter()
        .map(|row| row.label.chars().count())
        .max()
        .unwrap_or(0)
        + GAP;
    let count = columns
        .len()
        .max(rows.iter().map(|row| row.cells.len()).max().unwrap_or(0))
        .max(1);
    let widest = |index: usize| {
        let header = columns.get(index).map_or(0, |name| name.chars().count());
        rows.iter()
            .filter_map(|row| row.cells.get(index))
            .map(|cell| cell.chars().count())
            .max()
            .unwrap_or(0)
            .max(header)
    };
    let mut lefts = Vec::with_capacity(count);
    let mut left = label_width;
    for index in 0..count {
        lefts.push(left);
        left += widest(index) + GAP;
    }
    if left - GAP > width {
        // Too wide to size by content: share the width evenly.
        let column = width.saturating_sub(label_width) / count;
        lefts = (0..count)
            .map(|index| label_width + index * column)
            .collect();
    }
    let room = |index: usize| {
        let end = lefts
            .get(index + 1)
            .map_or(width, |next| next.saturating_sub(1));
        end.saturating_sub(lefts[index])
    };
    let mut grid = Grid::new(width, rows.len() * 2 + 2);
    for (index, name) in columns.iter().enumerate().take(count) {
        grid.put_str(
            lefts[index],
            0,
            &cut(name, room(index)),
            Style::at(Intensity::Half),
        );
    }
    grid.hrule(0, 1, width, Style::at(Intensity::Quarter), false);
    for (index, row) in rows.iter().enumerate() {
        let at = 2 + index * 2;
        grid.put_str(0, at, &row.label, Style::at(Intensity::Half));
        for (column_index, cell) in row.cells.iter().enumerate().take(count) {
            let style = if column_index == 0 {
                Style::at(Intensity::Full)
            } else {
                Style::at(Intensity::ThreeQuarters)
            };
            let cell = cut(cell, room(column_index));
            grid.put_str(lefts[column_index], at, &cell, style);
        }
    }
    grid
}

/// The stages of a run, as boxes joined left to right by a hairline.
fn flow(steps: &[String], width: usize) -> Grid {
    if steps.is_empty() {
        return Grid::new(width, 0);
    }
    let joins = steps.len().saturating_sub(1);
    let join = 3;
    let box_width = (width.saturating_sub(joins * join)) / steps.len();
    let mut grid = Grid::new(width, 3);
    for (index, step) in steps.iter().enumerate() {
        let left = index * (box_width + join);
        grid.frame(left, 0, box_width, 3, Style::at(Intensity::Quarter), false);
        let label = cut(step, box_width.saturating_sub(4));
        grid.put_str(
            left + center(box_width, label.chars().count()),
            1,
            &label,
            Style::at(Intensity::Full),
        );
    }
    // Each join runs from one box's right edge to the next box's left edge,
    // drawn once both frames are down so the rules meet in tees.
    // The tees keep the frame's intensity; the join between them is half.
    let (edge_style, style) = (Style::at(Intensity::Quarter), Style::at(Intensity::Half));
    for index in 0..joins {
        let edge = index * (box_width + join) + box_width - 1;
        let right = Arms {
            right: true,
            ..Arms::default()
        };
        let left = Arms {
            left: true,
            ..Arms::default()
        };
        grid.rule(edge, 1, right, edge_style);
        grid.hrule(edge + 1, 1, join, style, false);
        grid.rule(edge + join + 1, 1, left, edge_style);
    }
    grid
}

/// A framed passage, the slide's one full-intensity element, over its
/// attribution.
fn quote(slide: &Slide, width: usize) -> Grid {
    let measure = MEASURE.min(width.saturating_sub(8));
    let laid = prose::text(
        &slide.body,
        Style::at(Intensity::Full).italic(true),
        measure,
    );
    let frame_width = measure + 6;
    let kicker = slide.kicker.clone().unwrap_or_default();
    let top = if kicker.is_empty() { 0 } else { 2 };
    let framed = laid.height() + 4;
    let height = top + framed + if slide.lead.is_some() { 2 } else { 0 };
    let mut grid = Grid::new(width, height);
    let left = center(width, frame_width);
    if !kicker.is_empty() {
        grid.put_str(left, 0, &kicker, Style::at(Intensity::Half));
    }
    grid.frame(
        left,
        top,
        frame_width,
        framed,
        Style::at(Intensity::Quarter),
        false,
    );
    grid.blit(left + 3, top + 2, &laid);
    if let Some(lead) = &slide.lead {
        centered_line(&mut grid, height - 1, lead, Style::at(Intensity::Half));
    }
    grid
}

/// Labeled facts on the left and prose on the right, a hairline between
/// them.
fn ask(slide: &Slide, width: usize) -> Grid {
    let left_width = width / 3;
    let right_width = width.saturating_sub(left_width + 3);
    let facts = slide.metrics.len() * 3;
    let prose = Prose::default().layout(&slide.body, right_width);
    let height = facts.max(prose.height()).max(1);
    let mut grid = Grid::new(width, height);
    for (index, metric) in slide.metrics.iter().enumerate() {
        let row = index * 3;
        grid.put_str(0, row, &metric.label, Style::at(Intensity::Half));
        let style = if metric.is_unfilled() {
            Style::at(Intensity::Half)
        } else {
            Style::at(Intensity::Full)
        };
        grid.put_str(0, row + 1, metric.shown(), style.bold(true));
    }
    grid.vrule(
        left_width + 1,
        0,
        height,
        Style::at(Intensity::Quarter),
        false,
    );
    grid.blit(left_width + 3, 0, &prose);
    grid
}

/// `text` when it fits in `room` cells, or its first cells and a mark.
fn cut(text: &str, room: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= room || room == 0 {
        return text.to_string();
    }
    let mut cut: String = chars[..room - 1].iter().collect();
    cut.push('…');
    cut
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slide::Metric;

    fn slide(layout: Layout) -> Slide {
        Slide {
            id: "test".to_string(),
            layout: Some(layout),
            ..Slide::default()
        }
    }

    /// The banner draws the name in the face with its line under it.
    #[test]
    fn the_banner_draws_the_name_and_its_line() {
        let mut slide = slide(Layout::Banner);
        slide.title = Some("Test-time".to_string());
        slide.lead = Some("The capabilities an agent gains while it runs".to_string());
        let grid = body(&slide, Canvas::DEFAULT);
        assert!(grid.height() > banner::GLYPH_ROWS);
        assert!(grid.to_text().contains("while it runs"));
    }

    /// A banner name too wide for one line breaks between words, each line
    /// inside the body.
    #[test]
    fn a_wide_banner_breaks_between_words() {
        let mut slide = slide(Layout::Banner);
        slide.title = Some("Test-Time Capabilities".to_string());
        let grid = body(&slide, Canvas::DEFAULT);
        assert_eq!(grid.height(), 2 * banner::GLYPH_ROWS + 1);
        assert!(grid.width() <= Canvas::DEFAULT.body_cells());
    }

    /// A statement sets no wider than the measure, whatever the canvas.
    #[test]
    fn a_statement_keeps_its_measure() {
        let mut slide = slide(Layout::Statement);
        slide.body = "Admit a tool into the run. ".repeat(12);
        let grid = body(&slide, Canvas::DEFAULT);
        for line in grid.to_text().lines() {
            assert!(
                line.trim().chars().count() <= MEASURE,
                "a line sets wider than the measure: {line}"
            );
        }
    }

    /// A metric with no value draws the mark, so an empty cell never reads
    /// as a number, and the labels draw under the values.
    #[test]
    fn an_unfilled_metric_draws_the_mark() {
        let mut slide = slide(Layout::Metrics);
        slide.metrics = vec![
            Metric {
                value: String::new(),
                label: "adoptions".to_string(),
            },
            Metric {
                value: "3/3".to_string(),
                label: "results a second trainer confirmed, in the hosted record".to_string(),
            },
        ];
        let grid = body(&slide, Canvas::DEFAULT);
        let text = grid.to_text();
        assert!(text.contains("adoptions"));
        assert!(text.contains("second trainer"));
        assert!(text.contains(crate::slide::UNFILLED));
    }

    /// The flow draws one box per stage inside the width it is given, under
    /// its title.
    #[test]
    fn the_flow_draws_a_box_a_stage() {
        let mut slide = slide(Layout::Flow);
        slide.steps = ["discover", "admit", "measure", "adopt"]
            .iter()
            .map(|step| step.to_string())
            .collect();
        let grid = body(&slide, Canvas::DEFAULT);
        assert_eq!(grid.height(), 3);
        slide.title = Some("The life of a capability".to_string());
        let titled = body(&slide, Canvas::DEFAULT);
        assert_eq!(titled.height(), 6);
        let text = titled.to_text();
        for step in ["discover", "admit", "measure", "adopt"] {
            assert!(text.contains(step), "the flow lost {step}");
        }
    }

    /// A kicker draws over the title at half intensity.
    #[test]
    fn a_kicker_sits_over_the_title() {
        let mut slide = slide(Layout::Points);
        slide.kicker = Some("PART I".to_string());
        slide.title = Some("The idea".to_string());
        slide.body = "- **One.** thing".to_string();
        let grid = body(&slide, Canvas::DEFAULT);
        let text = grid.to_text();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "PART I");
        assert_eq!(lines[1], "The idea");
        assert_eq!(grid.get(0, 0).unwrap().style.intensity, Intensity::Half);
    }
}
