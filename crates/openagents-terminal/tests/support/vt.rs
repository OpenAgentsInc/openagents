//! A small terminal emulator for the end-to-end test: enough of VT100 and
//! xterm to read back what ratatui's crossterm backend draws (cursor moves,
//! clears, text) as a grid of characters. Colors and modes are ignored; the
//! raw bytes are kept for checks on them.

pub struct Grid {
    pub rows: usize,
    pub cols: usize,
    cells: Vec<Vec<char>>,
    row: usize,
    col: usize,
    state: State,
    params: String,
    utf8: Vec<u8>,
    /// Every byte the program wrote.
    pub raw: Vec<u8>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Ground,
    Escape,
    Csi,
    Osc,
    OscEscape,
}

impl Grid {
    pub fn new(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            cells: vec![vec![' '; cols]; rows],
            row: 0,
            col: 0,
            state: State::Ground,
            params: String::new(),
            utf8: Vec::new(),
            raw: Vec::new(),
        }
    }

    /// The screen as text, one line per row, trailing blanks trimmed.
    pub fn text(&self) -> String {
        self.cells
            .iter()
            .map(|row| row.iter().collect::<String>().trim_end().to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        self.raw.extend_from_slice(bytes);
        for &byte in bytes {
            self.byte(byte);
        }
    }

    fn byte(&mut self, byte: u8) {
        match self.state {
            State::Ground => match byte {
                0x1b => self.state = State::Escape,
                b'\r' => self.col = 0,
                b'\n' => self.down(),
                0x08 => self.col = self.col.saturating_sub(1),
                0x07 => {}
                byte if byte < 0x20 => {}
                byte => {
                    self.utf8.push(byte);
                    if let Ok(text) = std::str::from_utf8(&self.utf8) {
                        let text = text.to_owned();
                        self.utf8.clear();
                        for c in text.chars() {
                            self.put(c);
                        }
                    } else if self.utf8.len() >= 4 {
                        self.utf8.clear();
                    }
                }
            },
            State::Escape => match byte {
                b'[' => {
                    self.params.clear();
                    self.state = State::Csi;
                }
                b']' => self.state = State::Osc,
                _ => self.state = State::Ground,
            },
            State::Csi => {
                if (0x40..=0x7e).contains(&byte) {
                    self.csi(byte);
                    self.state = State::Ground;
                } else {
                    self.params.push(byte as char);
                }
            }
            State::Osc => match byte {
                0x07 => self.state = State::Ground,
                0x1b => self.state = State::OscEscape,
                _ => {}
            },
            State::OscEscape => self.state = State::Ground,
        }
    }

    fn put(&mut self, c: char) {
        if self.col >= self.cols {
            self.col = 0;
            self.down();
        }
        self.cells[self.row][self.col] = c;
        self.col += 1;
    }

    fn down(&mut self) {
        if self.row + 1 >= self.rows {
            self.cells.remove(0);
            self.cells.push(vec![' '; self.cols]);
        } else {
            self.row += 1;
        }
    }

    fn numbers(&self) -> Vec<usize> {
        self.params
            .trim_start_matches(['?', '>', '='])
            .split(';')
            .map(|part| {
                part.trim_end_matches(|c: char| !c.is_ascii_digit())
                    .parse()
                    .unwrap_or(0)
            })
            .collect()
    }

    fn csi(&mut self, last: u8) {
        if self.params.starts_with('?') || self.params.contains(' ') {
            // Modes and the cursor's shape.
            return;
        }
        let numbers = self.numbers();
        let first = |default: usize| match numbers.first() {
            Some(0) | None => default,
            Some(n) => *n,
        };
        match last {
            b'H' | b'f' => {
                let row = first(1);
                let col = match numbers.get(1) {
                    Some(0) | None => 1,
                    Some(n) => *n,
                };
                self.row = (row - 1).min(self.rows - 1);
                self.col = (col - 1).min(self.cols - 1);
            }
            b'A' => self.row = self.row.saturating_sub(first(1)),
            b'B' => self.row = (self.row + first(1)).min(self.rows - 1),
            b'C' => self.col = (self.col + first(1)).min(self.cols - 1),
            b'D' => self.col = self.col.saturating_sub(first(1)),
            b'G' => self.col = (first(1) - 1).min(self.cols - 1),
            b'd' => self.row = (first(1) - 1).min(self.rows - 1),
            b'J' => match numbers.first().copied().unwrap_or(0) {
                2 | 3 => {
                    for row in &mut self.cells {
                        row.fill(' ');
                    }
                }
                0 => {
                    self.cells[self.row][self.col..].fill(' ');
                    for row in &mut self.cells[self.row + 1..] {
                        row.fill(' ');
                    }
                }
                _ => {}
            },
            b'K' => match numbers.first().copied().unwrap_or(0) {
                0 => self.cells[self.row][self.col..].fill(' '),
                1 => self.cells[self.row][..=self.col.min(self.cols - 1)].fill(' '),
                _ => self.cells[self.row].fill(' '),
            },
            _ => {}
        }
    }
}

#[test]
fn the_grid_reads_moves_clears_and_text() {
    let mut grid = Grid::new(3, 10);
    grid.feed(b"\x1b[?1049h\x1b[2J\x1b[2;3Hhi\x1b[38;2;255;255;255mthere\x1b[0m");
    grid.feed("\x1b[1;1H·≈".as_bytes());
    grid.feed(b"\x1b]12;#FFFFFF\x07\x1b[1 q");
    assert_eq!(grid.text(), "·≈\n  hithere\n");
    grid.feed(b"\x1b[2;5H\x1b[K");
    assert_eq!(grid.text(), "·≈\n  hi\n");
}
