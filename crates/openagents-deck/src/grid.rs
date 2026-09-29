//! The cell grid every slide lays out onto and every renderer draws from.
//!
//! Ported from the Coder repository's component core (`coder_ui_core::grid`),
//! with the snapshot format unchanged. Only the console-font fallback for
//! dashed rules is left out, because the painter draws every rule itself.
//!
//! A [`Cell`] is one glyph at one [`Intensity`] with a few flags. A [`Grid`]
//! is a fixed width of cells in rows. Rules and frames are box-drawing
//! glyphs; the painter draws them as hairlines through the cell from the
//! arms [`Cell::arms`] returns, so a renderer never parses Unicode itself.
//!
//! [`Grid::to_text`] is the glyphs alone, one line per row, and
//! [`Grid::snapshot`] adds the intensity of every cell, the flags, and the
//! press regions; the snapshot is what the golden files under `snapshots/`
//! hold.

use coder_ui::theme::Intensity;
use std::ops::Range;

/// The id of a press target a cell belongs to. A component assigns ids
/// from zero in the order it lists its targets.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct PressId(pub u32);

/// The id of a link destination in the grid's link table.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct LinkId(pub u32);

/// How one cell draws. Two cells with equal styles join into one text run
/// when a renderer draws a row.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct Style {
    pub intensity: Intensity,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    /// The cell is under a press: GPUI and HTML draw it at 80% opacity, and
    /// the terminal dims it.
    pub pressed: bool,
    /// The cell is the caret: GPUI draws its block from the font's ascent
    /// to its descent, and the terminal places the cursor on it. The glyph
    /// is `█` while the caret is on and a space while it is off.
    pub caret: bool,
    pub link: Option<LinkId>,
    pub press: Option<PressId>,
}

impl Style {
    /// A plain style at `intensity`.
    pub const fn at(intensity: Intensity) -> Style {
        Style {
            intensity,
            bold: false,
            italic: false,
            underline: false,
            strike: false,
            pressed: false,
            caret: false,
            link: None,
            press: None,
        }
    }

    pub const fn bold(mut self, bold: bool) -> Style {
        self.bold = bold;
        self
    }

    pub const fn italic(mut self, italic: bool) -> Style {
        self.italic = italic;
        self
    }

    pub const fn underline(mut self, underline: bool) -> Style {
        self.underline = underline;
        self
    }

    pub const fn strike(mut self, strike: bool) -> Style {
        self.strike = strike;
        self
    }

    pub const fn pressed(mut self, pressed: bool) -> Style {
        self.pressed = pressed;
        self
    }

    pub const fn caret(mut self, caret: bool) -> Style {
        self.caret = caret;
        self
    }

    pub const fn press(mut self, press: Option<PressId>) -> Style {
        self.press = press;
        self
    }

    pub const fn link(mut self, link: Option<LinkId>) -> Style {
        self.link = link;
        self
    }

    /// The style with its intensity lowered to `cap` when it is above it.
    pub fn capped(mut self, cap: Intensity) -> Style {
        self.intensity = self.intensity.min(cap);
        self
    }
}

/// The arms a rule glyph extends into its neighbors, and whether it is
/// dashed.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Arms {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub dashed: bool,
}

/// The light box-drawing glyphs the grid draws rules with, each beside its
/// arms.
const RULES: &[(char, Arms)] = &[
    ('─', arms(false, false, true, true, false)),
    ('│', arms(true, true, false, false, false)),
    ('╌', arms(false, false, true, true, true)),
    ('╎', arms(true, true, false, false, true)),
    ('┌', arms(false, true, false, true, false)),
    ('┐', arms(false, true, true, false, false)),
    ('└', arms(true, false, false, true, false)),
    ('┘', arms(true, false, true, false, false)),
    ('├', arms(true, true, false, true, false)),
    ('┤', arms(true, true, true, false, false)),
    ('┬', arms(false, true, true, true, false)),
    ('┴', arms(true, false, true, true, false)),
    ('┼', arms(true, true, true, true, false)),
];

const fn arms(up: bool, down: bool, left: bool, right: bool, dashed: bool) -> Arms {
    Arms {
        up,
        down,
        left,
        right,
        dashed,
    }
}

/// The glyph for `arms`, solid unless the arms are a dashed straight line.
fn rule_glyph(arms: Arms) -> char {
    let solid = Arms {
        dashed: false,
        ..arms
    };
    RULES
        .iter()
        .find(|(_, candidate)| *candidate == arms)
        .or_else(|| RULES.iter().find(|(_, candidate)| *candidate == solid))
        .map(|(glyph, _)| *glyph)
        .unwrap_or(' ')
}

/// One cell: a glyph and its style.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Cell {
    pub glyph: char,
    pub style: Style,
}

impl Cell {
    /// A space at full intensity, which every grid starts filled with.
    pub const BLANK: Cell = Cell {
        glyph: ' ',
        style: Style::at(Intensity::Full),
    };

    pub const fn new(glyph: char, style: Style) -> Cell {
        Cell { glyph, style }
    }

    /// The arms of this cell's glyph when it is a rule, and `None` for
    /// text.
    pub fn arms(&self) -> Option<Arms> {
        RULES
            .iter()
            .find(|(glyph, _)| *glyph == self.glyph)
            .map(|(_, arms)| *arms)
    }

    /// Whether the glyph is one of the braille patterns the spinner cycles.
    pub fn is_braille(&self) -> bool {
        ('\u{2800}'..='\u{28ff}').contains(&self.glyph)
    }

    /// Whether the cell holds a space with no caret and no press.
    pub fn is_blank(&self) -> bool {
        self.glyph == ' ' && !self.style.caret
    }
}

impl Default for Cell {
    fn default() -> Cell {
        Cell::BLANK
    }
}

/// One run of a row: the cells from `col` that share `style`, as text.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TextRun {
    pub col: usize,
    pub text: String,
    pub style: Style,
}

/// A grid of cells at a fixed width. Rows grow as a component writes below
/// the last one.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Grid {
    width: usize,
    rows: Vec<Vec<Cell>>,
    links: Vec<String>,
}

impl Grid {
    /// A blank grid `width` cells wide and `height` rows tall.
    pub fn new(width: usize, height: usize) -> Grid {
        Grid {
            width,
            rows: vec![vec![Cell::BLANK; width]; height],
            links: Vec::new(),
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.rows.len()
    }

    /// The rows, top to bottom.
    pub fn rows(&self) -> impl Iterator<Item = &[Cell]> {
        self.rows.iter().map(Vec::as_slice)
    }

    /// The cell at `col`, `row`, when the grid has one there.
    pub fn get(&self, col: usize, row: usize) -> Option<&Cell> {
        self.rows.get(row).and_then(|cells| cells.get(col))
    }

    /// Adds rows until the grid is at least `height` tall.
    pub fn grow(&mut self, height: usize) {
        while self.rows.len() < height {
            self.rows.push(vec![Cell::BLANK; self.width]);
        }
    }

    /// Drops the rows from `height` down, leaving a grid at most `height`
    /// rows tall.
    pub fn truncate(&mut self, height: usize) {
        self.rows.truncate(height);
    }

    /// Writes one cell, growing the grid when `row` is below the last row.
    /// A column at or past the width is dropped.
    pub fn put(&mut self, col: usize, row: usize, cell: Cell) {
        if col >= self.width {
            return;
        }
        self.grow(row + 1);
        self.rows[row][col] = cell;
    }

    /// Writes `text` from `col` on `row` in `style`, one cell per
    /// character, and returns how many cells it wrote. Text past the width
    /// is dropped.
    pub fn put_str(&mut self, col: usize, row: usize, text: &str, style: Style) -> usize {
        let mut written = 0;
        for (offset, glyph) in text.chars().enumerate() {
            if col + offset >= self.width {
                break;
            }
            self.put(col + offset, row, Cell::new(glyph, style));
            written += 1;
        }
        written
    }

    /// Fills the `width` by `height` rectangle at `col`, `row` with `cell`.
    pub fn fill(&mut self, col: usize, row: usize, width: usize, height: usize, cell: Cell) {
        for r in row..row + height {
            for c in col..col + width {
                self.put(c, r, cell);
            }
        }
    }

    /// Copies `other` onto this grid with its top-left cell at `col`,
    /// `row`. Links are re-numbered into this grid's table; press ids are
    /// kept, so a component that blits its parts assigns their ids itself.
    pub fn blit(&mut self, col: usize, row: usize, other: &Grid) {
        let links: Vec<LinkId> = other.links.iter().map(|url| self.link(url)).collect();
        for (r, cells) in other.rows.iter().enumerate() {
            for (c, cell) in cells.iter().enumerate() {
                let mut cell = *cell;
                if let Some(LinkId(id)) = cell.style.link {
                    cell.style.link = Some(links[id as usize]);
                }
                self.put(col + c, row + r, cell);
            }
        }
    }

    /// Draws a rule glyph at `col`, `row`, joining it with the rule already
    /// there so shared frames meet in tees and crosses. A dashed arm meeting
    /// a solid one draws solid.
    pub fn rule(&mut self, col: usize, row: usize, arms: Arms, style: Style) {
        let merged = match self.get(col, row).and_then(Cell::arms) {
            Some(existing) => Arms {
                up: existing.up || arms.up,
                down: existing.down || arms.down,
                left: existing.left || arms.left,
                right: existing.right || arms.right,
                dashed: existing.dashed && arms.dashed,
            },
            None => arms,
        };
        self.put(col, row, Cell::new(rule_glyph(merged), style));
    }

    /// A horizontal rule `len` cells long from `col` on `row`.
    pub fn hrule(&mut self, col: usize, row: usize, len: usize, style: Style, dashed: bool) {
        for c in col..col + len {
            self.rule(c, row, arms(false, false, true, true, dashed), style);
        }
    }

    /// A vertical rule `len` rows long from `row` in `col`.
    pub fn vrule(&mut self, col: usize, row: usize, len: usize, style: Style, dashed: bool) {
        for r in row..row + len {
            self.rule(col, r, arms(true, true, false, false, dashed), style);
        }
    }

    /// A frame around the `width` by `height` rectangle at `col`, `row`,
    /// its rules on the rectangle's outermost cells.
    pub fn frame(
        &mut self,
        col: usize,
        row: usize,
        width: usize,
        height: usize,
        style: Style,
        dashed: bool,
    ) {
        if width < 2 || height < 2 {
            return;
        }
        let (right, bottom) = (col + width - 1, row + height - 1);
        self.hrule(col + 1, row, width - 2, style, dashed);
        self.hrule(col + 1, bottom, width - 2, style, dashed);
        self.vrule(col, row + 1, height - 2, style, dashed);
        self.vrule(right, row + 1, height - 2, style, dashed);
        self.rule(col, row, arms(false, true, false, true, dashed), style);
        self.rule(right, row, arms(false, true, true, false, dashed), style);
        self.rule(col, bottom, arms(true, false, false, true, dashed), style);
        self.rule(right, bottom, arms(true, false, true, false, dashed), style);
    }

    /// Registers `url` and returns its id, reusing the id of an equal URL.
    pub fn link(&mut self, url: &str) -> LinkId {
        if let Some(index) = self.links.iter().position(|known| known == url) {
            return LinkId(index as u32);
        }
        self.links.push(url.to_string());
        LinkId(self.links.len() as u32 - 1)
    }

    /// The URL behind `id`.
    pub fn link_url(&self, id: LinkId) -> Option<&str> {
        self.links.get(id.0 as usize).map(String::as_str)
    }

    /// The runs of `row`: maximal spans of cells with one style, so a
    /// renderer draws each row as a few text runs.
    pub fn runs(&self, row: usize) -> Vec<TextRun> {
        let mut runs: Vec<TextRun> = Vec::new();
        let Some(cells) = self.rows.get(row) else {
            return runs;
        };
        for (col, cell) in cells.iter().enumerate() {
            match runs.last_mut() {
                Some(run) if run.style == cell.style && !cell.style.caret => {
                    run.text.push(cell.glyph)
                }
                _ => runs.push(TextRun {
                    col,
                    text: cell.glyph.to_string(),
                    style: cell.style,
                }),
            }
        }
        runs
    }

    /// The columns and the rows the cells of press `id` span, when any
    /// cell carries it.
    pub fn press_bounds(&self, id: PressId) -> Option<(Range<usize>, Range<usize>)> {
        let mut cols: Option<Range<usize>> = None;
        let mut rows: Option<Range<usize>> = None;
        for (r, cells) in self.rows.iter().enumerate() {
            for (c, cell) in cells.iter().enumerate() {
                if cell.style.press != Some(id) {
                    continue;
                }
                cols = Some(match cols {
                    Some(range) => range.start.min(c)..range.end.max(c + 1),
                    None => c..c + 1,
                });
                rows = Some(match rows {
                    Some(range) => range.start.min(r)..range.end.max(r + 1),
                    None => r..r + 1,
                });
            }
        }
        Some((cols?, rows?))
    }

    /// Every press id any cell carries, in order.
    pub fn press_ids(&self) -> Vec<PressId> {
        let mut ids: Vec<PressId> = self
            .rows
            .iter()
            .flatten()
            .filter_map(|cell| cell.style.press)
            .collect();
        ids.sort();
        ids.dedup();
        ids
    }

    /// The glyphs alone, one line per row, with trailing spaces trimmed.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for cells in &self.rows {
            let line: String = cells.iter().map(|cell| cell.glyph).collect();
            out.push_str(line.trim_end());
            out.push('\n');
        }
        out
    }

    /// The golden form: the width and the height, the text, the intensity
    /// of every cell as a digit, the flags of every cell, and the press
    /// regions. A blank cell prints a space in the intensity and flag maps.
    pub fn snapshot(&self) -> String {
        let mut out = format!("width {} height {}\n", self.width, self.height());
        out.push_str(&self.to_text());
        out.push_str("--\n");
        for cells in &self.rows {
            let line: String = cells
                .iter()
                .map(|cell| {
                    if *cell == Cell::BLANK {
                        ' '
                    } else {
                        cell.style.intensity.digit()
                    }
                })
                .collect();
            out.push_str(line.trim_end());
            out.push('\n');
        }
        let flags: Vec<String> = self
            .rows
            .iter()
            .map(|cells| {
                let line: String = cells.iter().map(flag).collect();
                line.trim_end().to_string()
            })
            .collect();
        if flags.iter().any(|line| !line.is_empty()) {
            out.push_str("--\n");
            for line in flags {
                out.push_str(&line);
                out.push('\n');
            }
        }
        let presses = self.press_ids();
        if !presses.is_empty() {
            out.push_str("--\n");
            for id in presses {
                if let Some((cols, rows)) = self.press_bounds(id) {
                    out.push_str(&format!(
                        "press {} rows {}..{} cols {}..{}\n",
                        id.0, rows.start, rows.end, cols.start, cols.end
                    ));
                }
            }
        }
        out
    }
}

/// The one-letter flag a cell prints in the snapshot: the caret, a press,
/// then the marks in order; a space for none.
fn flag(cell: &Cell) -> char {
    let style = cell.style;
    if style.caret {
        'c'
    } else if style.pressed {
        'p'
    } else if style.bold {
        'b'
    } else if style.italic {
        'i'
    } else if style.underline {
        'u'
    } else if style.strike {
        's'
    } else if style.link.is_some() {
        'l'
    } else {
        ' '
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HALF: Style = Style::at(Intensity::Half);

    /// Text past the width is dropped, and a row below the last grows the
    /// grid.
    #[test]
    fn put_clips_at_the_width_and_grows_downward() {
        let mut grid = Grid::new(4, 1);
        assert_eq!(grid.put_str(1, 0, "abcdef", HALF), 3);
        grid.put(0, 2, Cell::new('x', HALF));
        assert_eq!(grid.height(), 3);
        assert_eq!(grid.to_text(), " abc\n\nx\n");
    }

    /// A frame's corners, and two frames sharing a rule meet in tees.
    #[test]
    fn frames_join_where_they_share_a_rule() {
        let mut grid = Grid::new(6, 5);
        grid.frame(0, 0, 6, 3, HALF, false);
        grid.frame(0, 2, 6, 3, HALF, false);
        assert_eq!(grid.to_text(), "┌────┐\n│    │\n├────┤\n│    │\n└────┘\n");
        assert_eq!(
            grid.get(0, 2).unwrap().arms(),
            Some(arms(true, true, false, true, false))
        );
    }

    /// A dashed frame draws dashed glyphs, and a solid rule over a dashed
    /// one draws solid.
    #[test]
    fn dashed_rules_yield_to_solid_ones() {
        let mut grid = Grid::new(4, 3);
        grid.frame(0, 0, 4, 3, HALF, true);
        assert_eq!(grid.to_text(), "┌╌╌┐\n╎  ╎\n└╌╌┘\n");
        grid.hrule(1, 0, 2, HALF, false);
        assert_eq!(grid.to_text(), "┌──┐\n╎  ╎\n└╌╌┘\n");
    }

    /// Runs split where the style changes and at every caret cell.
    #[test]
    fn runs_split_on_style_and_on_the_caret() {
        let mut grid = Grid::new(8, 1);
        grid.put_str(0, 0, "ab", Style::at(Intensity::Full));
        grid.put_str(2, 0, "cd", HALF);
        grid.put(4, 0, Cell::new('█', Style::at(Intensity::Full).caret(true)));
        let runs = grid.runs(0);
        let texts: Vec<&str> = runs.iter().map(|run| run.text.as_str()).collect();
        assert_eq!(texts, ["ab", "cd", "█", "   "]);
        assert_eq!(runs[2].col, 4);
    }

    /// A press region is the bounds of the cells carrying its id, and a
    /// blit keeps ids and re-numbers links.
    #[test]
    fn presses_and_links_survive_a_blit() {
        let mut part = Grid::new(6, 1);
        let link = part.link("https://example.com");
        part.put_str(
            0,
            0,
            "[ go ]",
            Style::at(Intensity::Full)
                .press(Some(PressId(3)))
                .link(Some(link)),
        );
        let mut whole = Grid::new(10, 3);
        whole.link("https://other.example");
        whole.blit(2, 1, &part);
        assert_eq!(whole.press_bounds(PressId(3)), Some((2..8, 1..2)));
        let cell = whole.get(2, 1).unwrap();
        assert_eq!(
            whole.link_url(cell.style.link.unwrap()),
            Some("https://example.com")
        );
        assert!(whole.press_bounds(PressId(0)).is_none());
    }

    /// The snapshot carries the text, the intensity map, the flags, and
    /// the press regions.
    #[test]
    fn the_snapshot_records_every_axis() {
        let mut grid = Grid::new(6, 1);
        grid.put_str(
            0,
            0,
            "[ go ]",
            Style::at(Intensity::Half)
                .bold(true)
                .press(Some(PressId(0))),
        );
        assert_eq!(
            grid.snapshot(),
            "width 6 height 1\n[ go ]\n--\n222222\n--\nbbbbbb\n--\npress 0 rows 0..1 cols 0..6\n"
        );
    }
}
