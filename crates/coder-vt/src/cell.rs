//! The grid's cells, their attributes, and styled runs for a renderer.

/// A cell's foreground or background color as the program asked for it.
/// A renderer maps it onto its own palette.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Color {
    /// The renderer's default foreground or background.
    #[default]
    Default,
    /// One of the 256 indexed colors: 0 to 7 are the standard ANSI colors,
    /// 8 to 15 their bright forms, 16 to 231 a 6x6x6 cube, and 232 to 255
    /// a gray ramp.
    Indexed(u8),
    /// A 24-bit color.
    Rgb(u8, u8, u8),
}

/// Rendition flags a cell carries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Flags(u16);

impl Flags {
    pub const BOLD: Flags = Flags(1);
    pub const DIM: Flags = Flags(1 << 1);
    pub const ITALIC: Flags = Flags(1 << 2);
    pub const UNDERLINE: Flags = Flags(1 << 3);
    pub const BLINK: Flags = Flags(1 << 4);
    pub const INVERSE: Flags = Flags(1 << 5);
    pub const HIDDEN: Flags = Flags(1 << 6);
    pub const STRIKE: Flags = Flags(1 << 7);
    /// Text the client wrote itself, such as a gap marker, rather than
    /// output from the host. A renderer shows it distinctly.
    pub const MARKER: Flags = Flags(1 << 8);

    #[must_use]
    pub const fn empty() -> Self {
        Flags(0)
    }

    /// The flags as a bit set.
    #[must_use]
    pub const fn bits(self) -> u16 {
        self.0
    }

    /// Flags from a bit set; unknown bits are kept.
    #[must_use]
    pub const fn from_bits(bits: u16) -> Self {
        Flags(bits)
    }

    #[must_use]
    pub const fn contains(self, other: Flags) -> bool {
        self.0 & other.0 == other.0
    }

    pub fn insert(&mut self, other: Flags) {
        self.0 |= other.0;
    }

    pub fn remove(&mut self, other: Flags) {
        self.0 &= !other.0;
    }
}

impl std::ops::BitOr for Flags {
    type Output = Flags;
    fn bitor(self, other: Flags) -> Flags {
        Flags(self.0 | other.0)
    }
}

/// The rendition of a cell: colors, flags, and the hyperlink it is part
/// of.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Attrs {
    pub fg: Color,
    pub bg: Color,
    pub flags: Flags,
    /// The OSC 8 hyperlink the cell belongs to, or 0 for none; the
    /// terminal's `link` answers its target.
    pub link: u16,
}

impl Attrs {
    /// The attributes an erase leaves: the current background only, as
    /// xterm's background color erase does.
    #[must_use]
    pub fn erased(self) -> Attrs {
        Attrs {
            fg: Color::Default,
            bg: self.bg,
            flags: Flags::empty(),
            link: 0,
        }
    }
}

/// One character cell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    /// The character. A blank cell holds a space.
    pub ch: char,
    /// Zero-width characters that combine with `ch`, such as accents.
    pub combining: Vec<char>,
    pub attrs: Attrs,
    /// Columns the character occupies: 1, 2 for a wide character, or 0 for
    /// the cell a wide character on its left covers.
    pub width: u8,
}

impl Cell {
    #[must_use]
    pub fn blank(attrs: Attrs) -> Cell {
        Cell {
            ch: ' ',
            combining: Vec::new(),
            attrs,
            width: 1,
        }
    }

    pub(crate) fn spacer(attrs: Attrs) -> Cell {
        Cell {
            ch: ' ',
            combining: Vec::new(),
            attrs,
            width: 0,
        }
    }
}

impl Default for Cell {
    fn default() -> Self {
        Cell::blank(Attrs::default())
    }
}

/// One row of cells.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub cells: Vec<Cell>,
    /// The row continues on the next one because the text wrapped.
    pub wrapped: bool,
}

impl Row {
    pub(crate) fn blank(cols: usize, attrs: Attrs) -> Row {
        Row {
            cells: vec![Cell::blank(attrs); cols],
            wrapped: false,
        }
    }

    /// Blanks the row in place at `cols` columns, keeping its allocation.
    pub(crate) fn reset(&mut self, cols: usize, attrs: Attrs) {
        let blank = Cell::blank(attrs);
        if self.cells.len() == cols {
            for cell in &mut self.cells {
                cell.clone_from(&blank);
            }
        } else {
            self.cells.clear();
            self.cells.resize(cols, blank);
        }
        self.wrapped = false;
    }

    /// The row's text, wide-character spacers skipped and trailing blanks
    /// trimmed.
    #[must_use]
    pub fn text(&self) -> String {
        self.text_between(0, self.cells.len())
    }

    /// The text of columns `from` up to `to`, as [`Row::text`] reads it. A
    /// wide character counts when either of its columns is in range.
    #[must_use]
    pub fn text_between(&self, from: usize, to: usize) -> String {
        let to = to.min(self.cells.len());
        let mut text = String::new();
        let mut col = from;
        // The right half of a wide character starts at its left half.
        if col > 0 && col < to && self.cells[col].width == 0 {
            col -= 1;
        }
        for cell in self.cells.get(col..to).unwrap_or_default() {
            if cell.width == 0 {
                continue;
            }
            text.push(cell.ch);
            text.extend(cell.combining.iter());
        }
        text.trim_end_matches(' ').to_owned()
    }

    /// The row as runs of cells that share attributes, left to right,
    /// covering every column. Wide-character spacers add no text; their
    /// columns count toward the run of the character that covers them.
    #[must_use]
    pub fn runs(&self) -> Vec<Run> {
        let mut runs: Vec<Run> = Vec::new();
        for cell in &self.cells {
            match runs.last_mut() {
                Some(run) if run.attrs == cell.attrs || cell.width == 0 => {
                    run.push(cell);
                }
                _ => {
                    let mut run = Run {
                        text: String::new(),
                        attrs: cell.attrs,
                        columns: 0,
                    };
                    run.push(cell);
                    runs.push(run);
                }
            }
        }
        runs
    }
}

/// Consecutive cells with the same attributes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run {
    pub text: String,
    pub attrs: Attrs,
    /// Columns the run covers, counting wide characters as two.
    pub columns: usize,
}

impl Run {
    fn push(&mut self, cell: &Cell) {
        self.columns += 1;
        if cell.width == 0 {
            return;
        }
        self.text.push(cell.ch);
        self.text.extend(cell.combining.iter());
    }
}
