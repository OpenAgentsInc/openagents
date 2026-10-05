//! Draws a `coder-vt` grid and the pane chrome into a UI batch.
//!
//! The chrome follows OpenAgents Terminal: one hue, the white ladder of
//! `coder_ui::theme` on its near-black field. A program's own colors draw
//! as the program asked; its default colors are the ladder's.

use coder_ui::theme::{Intensity, NEAR_BLACK};
use coder_vt::{Cell, Color, CursorShape, Flags, Row};

use terminal_core::layout::Rect;
use verse_gfx::palette;
use verse_gfx::ui::{Atlas, UiBatch};

/// A ladder step at an opacity, in linear light.
#[must_use]
pub fn white(step: Intensity, alpha: f32) -> [f32; 4] {
    let [r, g, b] = palette::linear(step.color());
    [r, g, b, alpha]
}

/// The near-black field at an opacity.
#[must_use]
pub fn field(alpha: f32) -> [f32; 4] {
    let [r, g, b] = palette::linear(NEAR_BLACK);
    [r, g, b, alpha]
}

fn rgb(value: u32) -> [f32; 4] {
    let [r, g, b] = palette::linear(value);
    [r, g, b, 1.0]
}

/// xterm's 256-color palette.
#[must_use]
pub fn indexed(index: u8) -> u32 {
    const BASE: [u32; 16] = [
        0x000000, 0xcd3131, 0x0dbc79, 0xe5e510, 0x2472c8, 0xbc3fbc, 0x11a8cd, 0xe5e5e5, 0x666666,
        0xf14c4c, 0x23d18b, 0xf5f543, 0x3b8eea, 0xd670d6, 0x29b8db, 0xffffff,
    ];
    match index {
        0..=15 => BASE[usize::from(index)],
        16..=231 => {
            let i = u32::from(index - 16);
            let level = |v: u32| if v == 0 { 0 } else { 55 + v * 40 };
            (level(i / 36) << 16) | (level((i / 6) % 6) << 8) | level(i % 6)
        }
        _ => {
            let v = 8 + 10 * u32::from(index - 232);
            (v << 16) | (v << 8) | v
        }
    }
}

/// A program color, or `None` for the default.
fn program(color: Color) -> Option<[f32; 4]> {
    match color {
        Color::Default => None,
        Color::Indexed(i) => Some(rgb(indexed(i))),
        Color::Rgb(r, g, b) => Some(rgb((u32::from(r) << 16)
            | (u32::from(g) << 8)
            | u32::from(b))),
    }
}

/// Line segments of a box-drawing character: up, down, left, right, each
/// 0 (none), 1 (light), or 2 (heavy).
#[must_use]
pub fn box_lines(c: char) -> Option<[u8; 4]> {
    const U: u8 = 1;
    let n = c as u32;
    let heavy = |w: bool| if w { 2 } else { 1 };
    let shape = |up: bool, down: bool, left: bool, right: bool, w: u8| {
        [
            if up { w } else { 0 },
            if down { w } else { 0 },
            if left { w } else { 0 },
            if right { w } else { 0 },
        ]
    };
    Some(match n {
        0x2500 | 0x2504 | 0x2508 | 0x254C | 0x2550 | 0x257C | 0x257E => {
            shape(false, false, true, true, U)
        }
        0x2501 | 0x2505 | 0x2509 | 0x254D => shape(false, false, true, true, 2),
        0x2502 | 0x2506 | 0x250A | 0x254E | 0x2551 | 0x257D | 0x257F => {
            shape(true, true, false, false, U)
        }
        0x2503 | 0x2507 | 0x250B | 0x254F => shape(true, true, false, false, 2),
        0x250C..=0x250F => shape(false, true, false, true, heavy(n == 0x250F)),
        0x2510..=0x2513 => shape(false, true, true, false, heavy(n == 0x2513)),
        0x2514..=0x2517 => shape(true, false, false, true, heavy(n == 0x2517)),
        0x2518..=0x251B => shape(true, false, true, false, heavy(n == 0x251B)),
        0x251C..=0x2523 => shape(true, true, false, true, heavy(n == 0x2523)),
        0x2524..=0x252B => shape(true, true, true, false, heavy(n == 0x252B)),
        0x252C..=0x2533 => shape(false, true, true, true, heavy(n == 0x2533)),
        0x2534..=0x253B => shape(true, false, true, true, heavy(n == 0x253B)),
        0x253C..=0x254B => shape(true, true, true, true, heavy(n == 0x254B)),
        0x2552..=0x2554 | 0x256D => shape(false, true, false, true, U),
        0x2555..=0x2557 | 0x256E => shape(false, true, true, false, U),
        0x2558..=0x255A | 0x2570 => shape(true, false, false, true, U),
        0x255B..=0x255D | 0x256F => shape(true, false, true, false, U),
        0x255E..=0x2560 => shape(true, true, false, true, U),
        0x2561..=0x2563 => shape(true, true, true, false, U),
        0x2564..=0x2566 => shape(false, true, true, true, U),
        0x2567..=0x2569 => shape(true, false, true, true, U),
        0x256A..=0x256C => shape(true, true, true, true, U),
        0x2574 => shape(false, false, true, false, U),
        0x2575 => shape(true, false, false, false, U),
        0x2576 => shape(false, false, false, true, U),
        0x2577 => shape(false, true, false, false, U),
        0x2578 => shape(false, false, true, false, 2),
        0x2579 => shape(true, false, false, false, 2),
        0x257A => shape(false, false, false, true, 2),
        0x257B => shape(false, true, false, false, 2),
        _ => return None,
    })
}

/// Rectangles of a block element as fractions of the cell (x, y, w, h)
/// and an opacity.
#[must_use]
pub fn block(c: char) -> Option<(Vec<[f32; 4]>, f32)> {
    let n = c as u32;
    let eighth = |k: u32| k as f32 / 8.0;
    let quad = |ul: bool, ur: bool, ll: bool, lr: bool| {
        let mut out = Vec::new();
        for (on, rect) in [
            (ul, [0.0, 0.0, 0.5, 0.5]),
            (ur, [0.5, 0.0, 0.5, 0.5]),
            (ll, [0.0, 0.5, 0.5, 0.5]),
            (lr, [0.5, 0.5, 0.5, 0.5]),
        ] {
            if on {
                out.push(rect);
            }
        }
        out
    };
    Some(match n {
        0x2580 => (vec![[0.0, 0.0, 1.0, 0.5]], 1.0),
        0x2581..=0x2587 => {
            let h = eighth(n - 0x2580);
            (vec![[0.0, 1.0 - h, 1.0, h]], 1.0)
        }
        0x2588 => (vec![[0.0, 0.0, 1.0, 1.0]], 1.0),
        0x2589..=0x258F => (vec![[0.0, 0.0, eighth(0x2590 - n), 1.0]], 1.0),
        0x2590 => (vec![[0.5, 0.0, 0.5, 1.0]], 1.0),
        0x2591 => (vec![[0.0, 0.0, 1.0, 1.0]], 0.25),
        0x2592 => (vec![[0.0, 0.0, 1.0, 1.0]], 0.5),
        0x2593 => (vec![[0.0, 0.0, 1.0, 1.0]], 0.75),
        0x2594 => (vec![[0.0, 0.0, 1.0, 0.125]], 1.0),
        0x2595 => (vec![[0.875, 0.0, 0.125, 1.0]], 1.0),
        0x2596 => (quad(false, false, true, false), 1.0),
        0x2597 => (quad(false, false, false, true), 1.0),
        0x2598 => (quad(true, false, false, false), 1.0),
        0x2599 => (quad(true, false, true, true), 1.0),
        0x259A => (quad(true, false, false, true), 1.0),
        0x259B => (quad(true, true, true, false), 1.0),
        0x259C => (quad(true, true, false, true), 1.0),
        0x259D => (quad(false, true, false, false), 1.0),
        0x259E => (quad(false, true, true, false), 1.0),
        0x259F => (quad(false, true, true, true), 1.0),
        _ => return None,
    })
}

/// Dots of a braille pattern (U+2800 to U+28FF) as (column, row) in its
/// two-by-four grid.
#[must_use]
pub fn braille(c: char) -> Option<Vec<(u8, u8)>> {
    let n = c as u32;
    if !(0x2800..=0x28FF).contains(&n) {
        return None;
    }
    // Bit order: dots 1, 2, 3 down the left, 4, 5, 6 down the right, then
    // 7 and 8 under them.
    const DOTS: [(u8, u8); 8] = [
        (0, 0),
        (0, 1),
        (0, 2),
        (1, 0),
        (1, 1),
        (1, 2),
        (0, 3),
        (1, 3),
    ];
    let bits = n - 0x2800;
    Some(
        DOTS.iter()
            .enumerate()
            .filter(|(i, _)| bits & (1 << i) != 0)
            .map(|(_, dot)| *dot)
            .collect(),
    )
}

/// A powerline separator: which way it points and whether it is solid.
#[must_use]
pub fn powerline(c: char) -> Option<(bool, bool)> {
    Some(match c as u32 {
        0xE0B0 => (true, true),
        0xE0B1 => (true, false),
        0xE0B2 => (false, true),
        0xE0B3 => (false, false),
        _ => return None,
    })
}

/// Whether the overlay draws `c` from shapes rather than a font glyph.
#[must_use]
pub fn shaped(c: char) -> bool {
    box_lines(c).is_some() || block(c).is_some() || braille(c).is_some() || powerline(c).is_some()
}

/// A stand-in for a private-use powerline glyph that fonts lack.
#[must_use]
pub fn substitute(c: char) -> char {
    match c as u32 {
        // The branch symbol, as the alternative-key symbol.
        0xE0A0 => '⎇',
        0xE0A1 => '¶',
        0xE0A2 => '🔒',
        _ => c,
    }
}

/// Draws a shape character in the cell, returning false when `c` is not
/// one.
fn glyph_shape(
    batch: &mut UiBatch,
    atlas: &Atlas,
    cell: [f32; 4],
    c: char,
    color: [f32; 4],
) -> bool {
    let [x, y, w, h] = cell;
    if let Some(lines) = box_lines(c) {
        let thin = (w / 8.0).round().max(1.0);
        let (cx, cy) = (
            (x + w / 2.0 - thin / 2.0).floor(),
            (y + h / 2.0 - thin / 2.0).floor(),
        );
        let t = |weight: u8| if weight == 2 { thin * 2.0 } else { thin };
        let [up, down, left, right] = lines;
        if up > 0 {
            batch.rect(atlas, cx, y, t(up), cy - y + t(up), color);
        }
        if down > 0 {
            batch.rect(atlas, cx, cy, t(down), y + h - cy, color);
        }
        if left > 0 {
            batch.rect(atlas, x, cy, cx - x + t(left), t(left), color);
        }
        if right > 0 {
            batch.rect(atlas, cx, cy, x + w - cx, t(right), color);
        }
        return true;
    }
    if let Some((rects, alpha)) = block(c) {
        let color = [color[0], color[1], color[2], color[3] * alpha];
        for [fx, fy, fw, fh] in rects {
            batch.rect(atlas, x + fx * w, y + fy * h, fw * w, fh * h, color);
        }
        return true;
    }
    if let Some(dots) = braille(c) {
        // Square dots on a two-by-four grid, centered in the cell.
        let size = (w / 4.0).min(h / 8.0).round().max(1.0);
        let (step_x, step_y) = (w / 2.0, h / 4.0);
        for (col, row) in dots {
            let dx = x + (f32::from(col) + 0.5) * step_x - size / 2.0;
            let dy = y + (f32::from(row) + 0.5) * step_y - size / 2.0;
            batch.rect(atlas, dx.round(), dy.round(), size, size, color);
        }
        return true;
    }
    if let Some((right, solid)) = powerline(c) {
        let (near, far) = if right { (x, x + w) } else { (x + w, x) };
        let mid = y + h / 2.0;
        if solid {
            batch.triangle(atlas, [near, y], [far, mid], [near, y + h], color);
        } else {
            let t = (w / 8.0).round().max(1.0);
            batch.line(atlas, [near, y], [far, mid], t, color);
            batch.line(atlas, [far, mid], [near, y + h], t, color);
        }
        return true;
    }
    false
}

/// What one pane shows this frame.
pub struct Grid<'a> {
    /// The rows to draw, top to bottom.
    pub rows: Vec<&'a Row>,
}

/// The cell size the atlas draws: its advance and line height.
#[must_use]
pub fn cell_size(atlas: &Atlas) -> [f32; 2] {
    [atlas.advance.max(1.0), atlas.line.max(1.0)]
}

/// The default foreground, a program color, or the bold and marker steps.
fn foreground(cell: &Cell) -> [f32; 4] {
    let flags = cell.attrs.flags;
    let mut fg = if flags.contains(Flags::INVERSE) {
        program(cell.attrs.bg).unwrap_or(field(1.0))
    } else {
        match program(cell.attrs.fg) {
            Some(color) => color,
            None if flags.contains(Flags::BOLD) => white(Intensity::Full, 1.0),
            None if flags.contains(Flags::MARKER) => white(Intensity::Half, 1.0),
            None => white(Intensity::ThreeQuarters, 1.0),
        }
    };
    if flags.contains(Flags::DIM) {
        fg[3] *= 0.6;
    }
    fg
}

/// Draws `cell`'s character with its top-left at `(x, y)` in `fg`: a
/// shape, a font glyph centered on a wide character's two columns, a
/// slanted italic, a doubled bold for program colors, and combining marks
/// over it. A character the atlas lacks draws as `?`.
pub fn glyph(batch: &mut UiBatch, atlas: &Atlas, x: f32, y: f32, cell: &Cell, fg: [f32; 4]) {
    let [cw, ch] = cell_size(atlas);
    let width = cw * f32::from(cell.width.max(1));
    let c = substitute(cell.ch);
    if glyph_shape(batch, atlas, [x, y, width, ch], c, fg) {
        return;
    }
    let mut buf = [0u8; 4];
    let c = if atlas.has_glyph(c) { c } else { '?' };
    let text: &str = c.encode_utf8(&mut buf);
    // A glyph wider or narrower than its columns is centered on them.
    let advance = atlas.glyph_box(c).map_or(cw, |g| g.advance);
    let x = if cell.width == 2 || advance > cw * 1.2 {
        (x + (width - advance) / 2.0).round()
    } else {
        x
    };
    let start = batch.vertices.len();
    batch.text(atlas, x, y, text, fg);
    let flags = cell.attrs.flags;
    if flags.contains(Flags::BOLD) && cell.attrs.fg != Color::Default {
        // A program color has no brighter step; thicken instead.
        batch.text(atlas, x + 1.0, y, text, fg);
    }
    for &mark in &cell.combining {
        if let Some(mark_box) = atlas.glyph_box(mark) {
            let mut buf = [0u8; 4];
            // Fonts place marks differently against the pen; center the
            // mark's bitmap over the character's columns instead.
            let pen =
                x + (width.min(advance.max(cw)) - mark_box.size[0]) / 2.0 - mark_box.offset[0];
            batch.text(atlas, pen, y, mark.encode_utf8(&mut buf), fg);
        }
    }
    if flags.contains(Flags::ITALIC) {
        // Slant about the baseline.
        let baseline = y + atlas.ascent;
        for vertex in &mut batch.vertices[start..] {
            vertex.pos[0] += (baseline - vertex.pos[1]) * 0.2;
        }
    }
}

/// Draws `grid` with its top-left cell at `origin`. `links` underlines
/// hyperlinked cells.
pub fn grid(batch: &mut UiBatch, atlas: &Atlas, origin: [f32; 2], grid: &Grid<'_>) {
    let [cw, ch] = cell_size(atlas);
    let default_fg = white(Intensity::ThreeQuarters, 1.0);
    // Backgrounds first, so glyphs draw over them, one rectangle per run
    // of cells that share a background.
    for (r, row) in grid.rows.iter().enumerate() {
        let y = origin[1] + r as f32 * ch;
        let mut run: Option<(usize, [f32; 4])> = None;
        for (col, cell) in row.cells.iter().enumerate() {
            let bg = if cell.attrs.flags.contains(Flags::INVERSE) {
                Some(program(cell.attrs.fg).unwrap_or(default_fg))
            } else {
                program(cell.attrs.bg)
            };
            match (run, bg) {
                (Some((_, color)), Some(bg)) if color == bg => {}
                _ => {
                    if let Some((from, color)) = run.take() {
                        let x = origin[0] + from as f32 * cw;
                        batch.rect(atlas, x, y, (col - from) as f32 * cw, ch, color);
                    }
                    run = bg.map(|bg| (col, bg));
                }
            }
        }
        if let Some((from, color)) = run {
            let x = origin[0] + from as f32 * cw;
            batch.rect(atlas, x, y, (row.cells.len() - from) as f32 * cw, ch, color);
        }
    }
    for (r, row) in grid.rows.iter().enumerate() {
        let y = origin[1] + r as f32 * ch;
        for (col, cell) in row.cells.iter().enumerate() {
            if cell.width == 0 {
                continue;
            }
            let flags = cell.attrs.flags;
            let x = origin[0] + col as f32 * cw;
            let width = cw * f32::from(cell.width);
            let fg = foreground(cell);
            if cell.ch != ' ' && !flags.contains(Flags::HIDDEN) {
                glyph(batch, atlas, x, y, cell, fg);
            }
            if flags.contains(Flags::UNDERLINE) {
                batch.rect(atlas, x, y + ch - 2.0, width, 1.0, fg);
            } else if cell.attrs.link != 0 {
                // A hyperlink: a faint underline until it is underlined.
                batch.rect(
                    atlas,
                    x,
                    y + ch - 2.0,
                    width,
                    1.0,
                    white(Intensity::Half, 0.7),
                );
            }
            if flags.contains(Flags::STRIKE) {
                batch.rect(atlas, x, y + ch / 2.0, width, 1.0, fg);
            }
        }
    }
}

/// Draws the cursor over cell `cell` at `(x, y)`: a block that inverts its
/// character, an underline, or a bar while the pane has focus, and an
/// outline otherwise.
pub fn cursor(
    batch: &mut UiBatch,
    atlas: &Atlas,
    [x, y]: [f32; 2],
    cell: Option<&Cell>,
    shape: CursorShape,
    focused: bool,
) {
    let [cw, ch] = cell_size(atlas);
    let width = cw * f32::from(cell.map_or(1, |c| c.width.max(1)));
    if !focused {
        batch.frame(atlas, x, y, width, ch, 1.0, white(Intensity::Half, 1.0));
        return;
    }
    let color = white(Intensity::Full, 0.9);
    match shape {
        CursorShape::Block => {
            batch.rect(atlas, x, y, width, ch, color);
            if let Some(cell) = cell.filter(|c| c.ch != ' ' && c.width != 0) {
                glyph(batch, atlas, x, y, cell, field(1.0));
            }
        }
        CursorShape::Underline => {
            let t = (ch / 10.0).round().max(2.0);
            batch.rect(atlas, x, y + ch - t, width, t, color);
        }
        CursorShape::Bar => {
            let t = (cw / 6.0).round().max(2.0);
            batch.rect(atlas, x, y, t, ch, color);
        }
    }
}

/// The rectangle the grid fills inside a pane's rectangle, below its
/// title bar, as [`chrome`] leaves it.
#[must_use]
pub fn inner(rect: Rect, cell: [f32; 2]) -> Rect {
    let bar = cell[1] + 4.0;
    let pad = 4.0;
    Rect::new(
        rect.x + pad,
        rect.y + bar + 2.0,
        (rect.w - 2.0 * pad).max(0.0),
        (rect.h - bar - 2.0 - pad).max(0.0),
    )
}

/// A pane's frame and title bar; `flash` lights the bar for a bell.
/// Returns the rectangle left for the grid.
pub fn chrome(
    batch: &mut UiBatch,
    atlas: &Atlas,
    rect: Rect,
    title: &str,
    detail: &str,
    focused: bool,
    flash: bool,
) -> Rect {
    let [cw, ch] = cell_size(atlas);
    let bar = ch + 4.0;
    // The title bar: a faint band, the title present when focused.
    batch.rect(
        atlas,
        rect.x,
        rect.y,
        rect.w,
        bar,
        if flash {
            white(Intensity::Full, 0.55)
        } else {
            white(Intensity::Quarter, if focused { 0.45 } else { 0.25 })
        },
    );
    let title_color = if focused {
        white(Intensity::Full, 1.0)
    } else {
        white(Intensity::Half, 1.0)
    };
    let columns = ((rect.w - 2.0 * cw) / cw).max(0.0) as usize;
    let mut line: String = title.chars().take(columns).collect();
    let used = line.chars().count();
    if !detail.is_empty() && used + 3 < columns {
        let rest: String = detail.chars().take(columns - used - 3).collect();
        let x = batch.text(atlas, rect.x + cw, rect.y + 2.0, &line, title_color);
        batch.text(
            atlas,
            rect.x + cw + x + cw,
            rect.y + 2.0,
            &format!("· {rest}"),
            white(
                if focused {
                    Intensity::ThreeQuarters
                } else {
                    Intensity::Quarter
                },
                1.0,
            ),
        );
        line.clear();
    }
    if !line.is_empty() {
        batch.text(atlas, rect.x + cw, rect.y + 2.0, &line, title_color);
    }
    let border = if focused {
        white(Intensity::Full, 1.0)
    } else {
        white(Intensity::Quarter, 1.0)
    };
    batch.frame(atlas, rect.x, rect.y, rect.w, rect.h, 1.0, border);
    inner(rect, [cw, ch])
}
