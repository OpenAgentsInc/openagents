//! Serialize and restore a terminal as a NIP-TERM snapshot stream
//! (`openagents.terminal-snapshot.v1`; `coder_pty::ext`).
//!
//! A host's emulator writes [`Terminal::snapshot`]: `TERMINAL`, `STATE`,
//! the `ROWS` of each screen, the parser's `CONTINUATION`, `READY`, then
//! history pages newest first and `FINISH`. A client feeds the records to
//! a [`Restore`], which yields a terminal at `READY` that draws the screen
//! and resumes parsing where the host's parser stands, and then attaches
//! each history page with [`Terminal::attach_history`].
//!
//! The format is this profile's own, ordered as libghostty's Snapshot v1
//! is; it does not read or write libghostty snapshots.
//!
//! A snapshot is screen-first parsed state, not a checkpoint of the whole
//! emulator. It does not carry the alternate screen while the primary one
//! shows, the character sets and origin mode a saved cursor holds (a
//! restored one uses ASCII and origin off), the character a repeat request
//! (`REP`) repeats, shell-integration marks, the bell count, or pending
//! replies and clipboard writes, which the host's emulator acts on.

use coder_pty::ext::{
    self, CONTINUATION_MAX, Charset as WireCharset, Charsets, Color as WireColor, Cursor,
    FinishRecord, HISTORY_PAGE_MAX, HISTORY_ROWS_MAX, HistoryRecord, HistorySpan, Keyboard,
    Modes as WireModes, PAYLOAD_MAX, PREFIX_MAX, RECORD_HEADER, ROWS_PAGE_MAX, Record, Region,
    RowsRecord, SNAPSHOT_HISTORY_BYTES, Saved as WireSaved, ScreenKind, Shape, StateRecord, Style,
    TerminalRecord,
};
use coder_pty::wire::{Exit, Reason, Refusal, Size, TerminalRef};

use crate::cell::{Attrs, Cell, Color, Flags, Row};
use crate::continuation::{Ignore, OSC_IGNORED, Resume};
use crate::mouse::{MouseEncoding, MouseMode};
use crate::{
    Charset, CursorShape, CursorStyle, MAX_SIZE, MAX_TITLE, Modes, Saved, State, Terminal, input,
};

/// What a stream describes beyond the emulator's own state: the terminal
/// it belongs to, the last sequenced frame the state reflects, and the
/// process's exit when it already ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    pub terminal: TerminalRef,
    /// The last sequenced frame the state reflects, 0 when none.
    pub through: u64,
    pub exit: Option<Exit>,
}

/// What a JSON record payload holds besides its rows, with room to spare.
const PAGE_OVERHEAD: usize = 64;

impl Terminal {
    /// The line epoch: 1 at first, one more after every full reset (`RIS`)
    /// or scrollback erase (`ED 3`), the operations after which absolute
    /// line numbers stop naming the same text.
    #[must_use]
    pub fn line_epoch(&self) -> u64 {
        self.state.epoch
    }

    /// What a snapshot taken now carries as its `CONTINUATION`: the input
    /// the parser holds unfinished, an escape sequence or a partial UTF-8
    /// character, and empty when the parser is at rest. Unfinished input
    /// longer than [`CONTINUATION_MAX`] is replaced by a short prefix that
    /// leaves a fresh parser with the same effects from then on; `None`
    /// means a partial character the snapshot drops on both sides.
    #[must_use]
    pub fn continuation(&self) -> Option<Vec<u8>> {
        if self.skipping {
            return Some(OSC_IGNORED.to_vec());
        }
        match self.tracker.continuation() {
            Ok(bytes) => Some(bytes.to_vec()),
            Err(Resume::Equivalent { continuation, .. }) => Some(continuation.to_vec()),
            Err(Resume::SkipOsc) => Some(OSC_IGNORED.to_vec()),
            Err(Resume::Reset) => None,
        }
    }

    /// Brings this parser to the state the continuation a snapshot just
    /// sent restores: it makes a long sequence one that is ignored,
    /// abandons a long OSC string, or drops a partial character.
    fn settle(&mut self) {
        if self.skipping {
            return;
        }
        match self.tracker.continuation() {
            Ok(_) => {}
            Err(Resume::Equivalent {
                continuation,
                poison,
            }) => {
                self.parser.advance(&mut Ignore, poison);
                self.tracker.rebase(continuation);
            }
            Err(Resume::SkipOsc) => self.abandon_osc(),
            Err(Resume::Reset) => {
                self.parser = vte::Parser::new();
                self.tracker = Default::default();
            }
        }
    }

    /// A snapshot stream: the prefix through `READY`, then at most 2,000
    /// history rows and 1 MiB of `HISTORY` records, newest first, and
    /// `FINISH`.
    ///
    /// Unfinished input longer than [`CONTINUATION_MAX`] is sent as a
    /// short equivalent ([`Terminal::continuation`]), and this parser
    /// settles into the state it restores, so this parser and the client's
    /// agree. A prefix past the 4 MiB bound refuses as `limit_exceeded` and
    /// changes nothing.
    pub fn snapshot(&mut self, binding: &Binding) -> Result<Vec<Record>, Refusal> {
        let continuation = self.continuation();
        let mut records = vec![
            Record::Terminal(self.terminal_record(binding)),
            Record::State(self.state_record()),
        ];
        let screens: &[(ScreenKind, &[Row])] = if self.state.alternate_active {
            &[
                (ScreenKind::Primary, &self.state.primary),
                (ScreenKind::Alternate, &self.state.alternate),
            ]
        } else {
            &[(ScreenKind::Primary, &self.state.primary)]
        };
        for &(screen, rows) in screens {
            for (first, page) in self.pages(rows, ROWS_PAGE_MAX) {
                records.push(Record::Rows(RowsRecord {
                    screen,
                    first: first as u16,
                    rows: page,
                }));
            }
        }
        if let Some(bytes) = &continuation
            && !bytes.is_empty()
        {
            records.push(Record::Continuation(bytes.clone()));
        }
        records.push(Record::Ready);
        let prefix: usize = records
            .iter()
            .map(|record| RECORD_HEADER + record.payload().len())
            .sum();
        if prefix > PREFIX_MAX {
            return Err(Refusal::new(
                Reason::LimitExceeded,
                "the snapshot prefix is longer than 4 MiB",
            ));
        }
        let first = self.state.dropped;
        let end = first + self.state.scrollback.len() as u64;
        let (pages, rows) = self.history_pages(end, HISTORY_ROWS_MAX, SNAPSHOT_HISTORY_BYTES);
        let complete = pages.last().map_or(end, |page| page.first) == first;
        records.extend(pages.into_iter().map(Record::History));
        records.push(Record::Finish(FinishRecord { rows, complete }));
        self.settle();
        Ok(records)
    }

    /// A history stream for a history read of `rows` rows before absolute
    /// line `before` in line epoch `epoch`: `TERMINAL`, `HISTORY` pages
    /// newest first, and `FINISH`.
    pub fn history_stream(
        &self,
        binding: &Binding,
        epoch: u64,
        before: u64,
        rows: u64,
    ) -> Result<Vec<Record>, Refusal> {
        if epoch != self.state.epoch {
            return Err(Refusal::new(
                Reason::Stale,
                "the line epoch is no longer current",
            ));
        }
        if rows == 0 || rows > HISTORY_ROWS_MAX {
            return Err(Refusal::new(
                Reason::Malformed,
                "a history read asks 1 to 2000 rows",
            ));
        }
        let first = self.state.dropped;
        let end = first + self.state.scrollback.len() as u64;
        if before > end {
            return Err(Refusal::new(
                Reason::Malformed,
                "before is past the newest history line",
            ));
        }
        if before <= first {
            return Err(Refusal::new(
                Reason::ContentUnavailable,
                "those rows left the history",
            ));
        }
        let (pages, sent) = self.history_pages(before, rows, usize::MAX);
        let complete = pages.last().map_or(before, |page| page.first) == first;
        let mut records = vec![Record::Terminal(self.terminal_record(binding))];
        records.extend(pages.into_iter().map(Record::History));
        records.push(Record::Finish(FinishRecord {
            rows: sent,
            complete,
        }));
        Ok(records)
    }

    /// Attaches a history page a snapshot or history stream sent for line
    /// epoch `epoch`, ahead of the oldest line kept, and answers how many
    /// rows it attached.
    ///
    /// The page must end where the kept history begins. A page from
    /// another epoch refuses as `stale` and one that does not adjoin as
    /// `malformed`; either changes nothing. Rows past the scrollback bound
    /// are left out, oldest first.
    pub fn attach_history(&mut self, epoch: u64, page: &HistoryRecord) -> Result<usize, Refusal> {
        if epoch != self.state.epoch {
            return Err(Refusal::new(
                Reason::Stale,
                "the page belongs to another line epoch",
            ));
        }
        let state = &mut self.state;
        let count = page.rows.len() as u64;
        if page.rows.is_empty() || page.first.checked_add(count) != Some(state.dropped) {
            return Err(Refusal::new(
                Reason::Malformed,
                "the page does not end where the history begins",
            ));
        }
        let cols = state.cols;
        let mut rows = Vec::with_capacity(page.rows.len());
        for row in &page.rows {
            let mut row = state.decode_row(row, MAX_SIZE)?;
            fit(&mut row, cols);
            rows.push(row);
        }
        let room = state
            .scrollback_max
            .saturating_sub(state.scrollback.len())
            .min(rows.len());
        for row in rows.into_iter().rev().take(room) {
            state.scrollback.push_front(row);
        }
        state.dropped -= room as u64;
        self.generation += 1;
        Ok(room)
    }

    fn terminal_record(&self, binding: &Binding) -> TerminalRecord {
        TerminalRecord {
            format: 1,
            generation: binding.terminal.generation.clone(),
            terminal: binding.terminal.terminal.clone(),
            epoch: self.state.epoch,
            through: binding.through,
            size: Size::new(self.state.rows as u16, self.state.cols as u16),
            history: HistorySpan {
                first: self.state.dropped,
                count: self.state.scrollback.len() as u64,
            },
            exit: binding.exit.clone(),
        }
    }

    fn state_record(&self) -> StateRecord {
        let state = &self.state;
        let modes = &state.modes;
        let saved = |saved: &Option<Saved>| {
            saved.map(|saved| WireSaved {
                row: saved.cursor.row as u16,
                col: saved.cursor.col as u16,
                pending_wrap: saved.cursor.pending_wrap,
                pen: state.style(saved.cursor.attrs),
            })
        };
        let mut private = Vec::new();
        for (on, mode) in [
            (modes.application_cursor, 1),
            (modes.origin, 6),
            (modes.autowrap, 7),
            (modes.application_keypad, 66),
            (modes.focus_events, 1004),
            (modes.bracketed_paste, 2004),
        ] {
            if on {
                private.push(mode);
            }
        }
        private.extend(match modes.mouse {
            MouseMode::Off => None,
            MouseMode::Press => Some(9),
            MouseMode::Click => Some(1000),
            MouseMode::Drag => Some(1002),
            MouseMode::Motion => Some(1003),
        });
        private.extend(match modes.mouse_encoding {
            MouseEncoding::Default => None,
            MouseEncoding::Utf8 => Some(1005),
            MouseEncoding::Sgr => Some(1006),
            MouseEncoding::Urxvt => Some(1015),
        });
        private.sort_unstable();
        let mut ansi = Vec::new();
        if modes.insert {
            ansi.push(4);
        }
        if modes.newline {
            ansi.push(20);
        }
        let charset = |set: Charset| match set {
            Charset::Ascii => WireCharset::Ascii,
            Charset::LineDrawing => WireCharset::DecSpecial,
        };
        StateRecord {
            alternate: state.alternate_active,
            cursor: Cursor {
                row: state.cursor.row as u16,
                col: state.cursor.col as u16,
                pending_wrap: state.cursor.pending_wrap,
                visible: modes.cursor_visible,
                shape: match modes.cursor_style.shape {
                    CursorShape::Block => Shape::Block,
                    CursorShape::Underline => Shape::Underline,
                    CursorShape::Bar => Shape::Bar,
                },
                blink: modes.cursor_style.blink,
            },
            pen: state.style(state.cursor.attrs),
            saved_primary: saved(&state.saved_primary),
            saved_alternate: saved(&state.saved_alternate),
            scroll: Region {
                top: state.top as u16,
                bottom: state.bottom as u16,
            },
            tabs: (0..state.cols)
                .filter(|&col| state.tabs[col])
                .map(|col| col as u16)
                .collect(),
            modes: WireModes { private, ansi },
            charsets: Charsets {
                g0: charset(state.charsets[0]),
                g1: charset(state.charsets[1]),
                shift: state.shift as u8,
            },
            keyboard: Keyboard {
                primary: state.kitty_primary.clone(),
                alternate: state.kitty_alternate.clone(),
            },
            title: state.title.clone(),
        }
    }

    /// `rows` as pages of at most `limit` rows whose payloads fit a record,
    /// each with the index of its first row.
    fn pages<'a>(
        &self,
        rows: impl IntoIterator<Item = &'a Row>,
        limit: usize,
    ) -> Vec<(usize, Vec<ext::Row>)> {
        let mut pages = Vec::new();
        let (mut page, mut bytes, mut first) = (Vec::new(), PAGE_OVERHEAD, 0);
        for (index, row) in rows.into_iter().enumerate() {
            let (row, size) = self.state.encode_row(row);
            if !page.is_empty() && (page.len() == limit || bytes + size > PAYLOAD_MAX) {
                pages.push((first, std::mem::take(&mut page)));
                bytes = PAGE_OVERHEAD;
                first = index;
            }
            page.push(row);
            bytes += size;
        }
        if !page.is_empty() {
            pages.push((first, page));
        }
        pages
    }

    /// History pages newest first, ending at absolute line `before`, at
    /// most `rows` rows and `budget` record bytes in all, and how many rows
    /// they hold.
    fn history_pages(&self, before: u64, rows: u64, budget: usize) -> (Vec<HistoryRecord>, u64) {
        let state = &self.state;
        let end = before.saturating_sub(state.dropped) as usize;
        let start = end.saturating_sub(rows as usize);
        let mut out = Vec::new();
        let (mut sent, mut spent) = (0u64, 0usize);
        let mut index = end;
        while index > start {
            // Collect one page backwards from `index`.
            let mut page = Vec::new();
            let mut bytes = PAGE_OVERHEAD;
            while index > start && page.len() < HISTORY_PAGE_MAX {
                let (row, size) = state.encode_row(&state.scrollback[index - 1]);
                if !page.is_empty() && bytes + size > PAYLOAD_MAX {
                    break;
                }
                page.push(row);
                bytes += size;
                index -= 1;
            }
            let record_bytes = RECORD_HEADER + bytes;
            if spent + record_bytes > budget {
                break;
            }
            spent += record_bytes;
            sent += page.len() as u64;
            page.reverse();
            out.push(HistoryRecord {
                first: state.dropped + index as u64,
                rows: page,
            });
        }
        (out, sent)
    }
}

/// Rebuilds a terminal from the records of a snapshot stream, through
/// `READY`. Nothing is trusted until every record before `READY` checks;
/// the first error ends the restore and yields no terminal.
#[derive(Debug)]
pub struct Restore {
    scrollback: usize,
    binding: Option<TerminalRecord>,
    /// The terminal being rebuilt, handed out only at `READY`.
    terminal: Option<Terminal>,
    state: Option<StateRecord>,
    /// Rows received of the primary screen and of the alternate one.
    primary: usize,
    alternate: usize,
    continuation: Vec<u8>,
    phase: Phase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Terminal,
    State,
    Rows,
    Continuation,
    Done,
    Failed,
}

impl Restore {
    /// A restore whose terminal keeps at most `scrollback` history lines.
    #[must_use]
    pub fn new(scrollback: usize) -> Self {
        Restore {
            scrollback,
            binding: None,
            terminal: None,
            state: None,
            primary: 0,
            alternate: 0,
            continuation: Vec::new(),
            phase: Phase::Terminal,
        }
    }

    /// The stream's binding, once `TERMINAL` arrived.
    #[must_use]
    pub fn binding(&self) -> Option<&TerminalRecord> {
        self.binding.as_ref()
    }

    /// Takes the next record of the stream. At `READY` it answers the
    /// restored terminal; every record before answers nothing. Later
    /// records refuse: history pages go to [`Terminal::attach_history`].
    pub fn push(&mut self, record: &Record) -> Result<Option<Terminal>, Refusal> {
        if self.phase == Phase::Failed {
            return Err(malformed("the restore already failed"));
        }
        let result = self.accept(record);
        if result.is_err() {
            self.phase = Phase::Failed;
            self.terminal = None;
        }
        result
    }

    fn accept(&mut self, record: &Record) -> Result<Option<Terminal>, Refusal> {
        match (self.phase, record) {
            (Phase::Terminal, Record::Terminal(binding)) => {
                if binding.format != 1 {
                    return Err(Refusal::new(
                        Reason::UnsupportedVersion,
                        "a stream format other than 1",
                    ));
                }
                let fits = |n: u16| (1..=MAX_SIZE).contains(&usize::from(n));
                if !fits(binding.size.rows) || !fits(binding.size.cols) {
                    return Err(malformed("a terminal size must be 1 to 1024"));
                }
                if binding.epoch == 0
                    || binding
                        .history
                        .first
                        .checked_add(binding.history.count)
                        .is_none()
                {
                    return Err(malformed(
                        "the line epoch or the history span is out of range",
                    ));
                }
                self.terminal = Some(Terminal::new(
                    usize::from(binding.size.rows),
                    usize::from(binding.size.cols),
                    self.scrollback,
                ));
                self.binding = Some(binding.clone());
                self.phase = Phase::State;
            }
            (Phase::State, Record::State(state)) => {
                self.check_state(state)?;
                self.state = Some(state.clone());
                self.phase = Phase::Rows;
            }
            (Phase::Rows, Record::Rows(page)) => self.rows(page)?,
            (Phase::Continuation, Record::Continuation(bytes)) if self.continuation.is_empty() => {
                if bytes.is_empty() || bytes.len() > CONTINUATION_MAX {
                    return Err(malformed("a continuation holds 1 to 4096 bytes"));
                }
                self.continuation.clone_from(bytes);
            }
            (Phase::Continuation, Record::Ready) => {
                let terminal = self.build()?;
                self.phase = Phase::Done;
                return Ok(Some(terminal));
            }
            _ => return Err(malformed(format!("{:?} out of order", record.tag()))),
        }
        Ok(None)
    }

    fn size(&self) -> (usize, usize) {
        let size = self.binding.as_ref().map_or(Size::new(1, 1), |b| b.size);
        (usize::from(size.rows), usize::from(size.cols))
    }

    fn check_state(&self, state: &StateRecord) -> Result<(), Refusal> {
        let (rows, cols) = self.size();
        let within = |row: u16, col: u16| usize::from(row) < rows && usize::from(col) < cols;
        if !within(state.cursor.row, state.cursor.col) {
            return Err(malformed("the cursor lies outside the terminal"));
        }
        for saved in [&state.saved_primary, &state.saved_alternate]
            .into_iter()
            .flatten()
        {
            if !within(saved.row, saved.col) {
                return Err(malformed("a saved cursor lies outside the terminal"));
            }
        }
        let (top, bottom) = (
            usize::from(state.scroll.top),
            usize::from(state.scroll.bottom),
        );
        // One row has no region with top above bottom; it is the whole
        // screen.
        if !(top <= bottom && bottom < rows && (top < bottom || rows == 1)) {
            return Err(malformed("the scrolling region is out of range"));
        }
        if state.tabs.iter().any(|&tab| usize::from(tab) >= cols) {
            return Err(malformed("a tab stop lies outside the terminal"));
        }
        if state.charsets.shift > 1 {
            return Err(malformed("the active character set is 0 or 1"));
        }
        if state.keyboard.primary.len() > input::kitty::STACK
            || state.keyboard.alternate.len() > input::kitty::STACK
        {
            return Err(malformed("a keyboard flag stack holds at most 16"));
        }
        if state.title.len() > 1024 || state.title.chars().any(char::is_control) {
            return Err(malformed("the title is too long or has a control"));
        }
        Ok(())
    }

    fn rows(&mut self, page: &RowsRecord) -> Result<(), Refusal> {
        let (rows, cols) = self.size();
        let alternate = self.state.as_ref().is_some_and(|state| state.alternate);
        let (screen, received) = if self.primary < rows {
            (ScreenKind::Primary, self.primary)
        } else {
            (ScreenKind::Alternate, self.alternate)
        };
        if page.screen != screen || usize::from(page.first) != received {
            return Err(malformed("ROWS out of order"));
        }
        if page.rows.is_empty()
            || page.rows.len() > ROWS_PAGE_MAX
            || received + page.rows.len() > rows
        {
            return Err(malformed(
                "a ROWS page holds 1 to 64 rows within the screen",
            ));
        }
        let terminal = self.terminal.as_mut().expect("bound before rows");
        for (offset, row) in page.rows.iter().enumerate() {
            let mut decoded = terminal.state.decode_row(row, cols)?;
            fit(&mut decoded, cols);
            let grid = match screen {
                ScreenKind::Primary => &mut terminal.state.primary,
                ScreenKind::Alternate => &mut terminal.state.alternate,
            };
            grid[received + offset] = decoded;
        }
        match screen {
            ScreenKind::Primary => self.primary += page.rows.len(),
            ScreenKind::Alternate => self.alternate += page.rows.len(),
        }
        if self.primary == rows && (!alternate || self.alternate == rows) {
            self.phase = Phase::Continuation;
        }
        Ok(())
    }

    fn build(&mut self) -> Result<Terminal, Refusal> {
        let binding = self.binding.as_ref().expect("bound");
        let record = self.state.take().expect("state");
        let mut terminal = self.terminal.take().expect("bound");
        terminal.state.epoch = binding.epoch;
        terminal.state.dropped = binding.history.end();
        terminal.state.apply(&record)?;
        if !self.continuation.is_empty() {
            terminal.parser.advance(&mut Ignore, &self.continuation);
            terminal.tracker.track(&self.continuation);
        }
        Ok(terminal)
    }
}

fn malformed(detail: impl Into<String>) -> Refusal {
    Refusal::new(Reason::Malformed, detail)
}

/// Pads or cuts a decoded row to `cols`, never leaving half a wide
/// character.
fn fit(row: &mut Row, cols: usize) {
    row.cells.resize(cols, Cell::default());
    if let Some(last) = row.cells.last_mut()
        && last.width == 2
    {
        *last = Cell::default();
    }
}

impl State {
    /// Sets everything a `STATE` record holds. The record was checked
    /// against the size; mode numbers this terminal does not keep are
    /// ignored, as an unknown `DECSET` is.
    fn apply(&mut self, record: &StateRecord) -> Result<(), Refusal> {
        let charset = |set: WireCharset| match set {
            WireCharset::Ascii => Charset::Ascii,
            WireCharset::DecSpecial => Charset::LineDrawing,
        };
        let mut modes = Modes {
            autowrap: false,
            cursor_visible: record.cursor.visible,
            cursor_style: CursorStyle {
                shape: match record.cursor.shape {
                    Shape::Block => CursorShape::Block,
                    Shape::Underline => CursorShape::Underline,
                    Shape::Bar => CursorShape::Bar,
                },
                blink: record.cursor.blink,
            },
            ..Modes::default()
        };
        for &mode in &record.modes.private {
            match mode {
                1 => modes.application_cursor = true,
                6 => modes.origin = true,
                7 => modes.autowrap = true,
                66 => modes.application_keypad = true,
                9 => modes.mouse = MouseMode::Press,
                1000 => modes.mouse = MouseMode::Click,
                1002 => modes.mouse = MouseMode::Drag,
                1003 => modes.mouse = MouseMode::Motion,
                1004 => modes.focus_events = true,
                1005 => modes.mouse_encoding = MouseEncoding::Utf8,
                1006 => modes.mouse_encoding = MouseEncoding::Sgr,
                1015 => modes.mouse_encoding = MouseEncoding::Urxvt,
                2004 => modes.bracketed_paste = true,
                _ => {}
            }
        }
        for &mode in &record.modes.ansi {
            match mode {
                4 => modes.insert = true,
                20 => modes.newline = true,
                _ => {}
            }
        }
        let pen = self.attrs(&record.pen)?;
        let mut saved = |wire: &Option<WireSaved>| -> Result<Option<Saved>, Refusal> {
            let Some(wire) = wire else {
                return Ok(None);
            };
            Ok(Some(Saved {
                cursor: crate::Cursor {
                    row: usize::from(wire.row),
                    col: usize::from(wire.col),
                    attrs: self.attrs(&wire.pen)?,
                    pending_wrap: wire.pending_wrap,
                },
                origin: false,
                charsets: [Charset::Ascii; 2],
                shift: 0,
            }))
        };
        let saved_primary = saved(&record.saved_primary)?;
        let saved_alternate = saved(&record.saved_alternate)?;
        self.modes = modes;
        self.alternate_active = record.alternate;
        self.cursor = crate::Cursor {
            row: usize::from(record.cursor.row),
            col: usize::from(record.cursor.col),
            attrs: pen,
            pending_wrap: record.cursor.pending_wrap,
        };
        self.saved_primary = saved_primary;
        self.saved_alternate = saved_alternate;
        self.top = usize::from(record.scroll.top);
        self.bottom = usize::from(record.scroll.bottom);
        self.tabs = vec![false; self.cols];
        for &tab in &record.tabs {
            self.tabs[usize::from(tab)] = true;
        }
        self.charsets = [charset(record.charsets.g0), charset(record.charsets.g1)];
        self.shift = usize::from(record.charsets.shift);
        let flags = |stack: &[u8]| -> Vec<u8> {
            stack
                .iter()
                .map(|flags| flags & input::kitty::SUPPORTED)
                .collect()
        };
        self.kitty_primary = flags(&record.keyboard.primary);
        self.kitty_alternate = flags(&record.keyboard.alternate);
        self.title = record
            .title
            .chars()
            .filter(|c| !c.is_control())
            .take(MAX_TITLE)
            .collect();
        self.damage.fill(true);
        Ok(())
    }

    fn style(&self, attrs: Attrs) -> Style {
        let color = |color: Color| match color {
            Color::Default => WireColor::Default,
            Color::Indexed(n) => WireColor::Index(n),
            Color::Rgb(r, g, b) => WireColor::Rgb([r, g, b]),
        };
        Style {
            fg: color(attrs.fg),
            bg: color(attrs.bg),
            flags: attrs.flags.bits() & 0xff,
            link: usize::from(attrs.link)
                .checked_sub(1)
                .and_then(|index| self.links.get(index))
                .cloned(),
        }
    }

    fn attrs(&mut self, style: &Style) -> Result<Attrs, Refusal> {
        let color = |color: WireColor| match color {
            WireColor::Default => Color::Default,
            WireColor::Index(n) => Color::Indexed(n),
            WireColor::Rgb([r, g, b]) => Color::Rgb(r, g, b),
        };
        if style.flags & !0xff != 0 {
            return Err(malformed("a style sets an unknown flag"));
        }
        let link = match &style.link {
            None => 0,
            Some(target)
                if !target.is_empty()
                    && target.len() <= crate::MAX_LINK
                    && !target.chars().any(char::is_control) =>
            {
                self.link_id(target)
            }
            Some(_) => {
                return Err(malformed(
                    "a link target is empty, too long, or has a control",
                ));
            }
        };
        Ok(Attrs {
            fg: color(style.fg),
            bg: color(style.bg),
            flags: Flags::from_bits(style.flags),
            link,
        })
    }

    /// A row on the wire and its JSON size. Trailing blanks in the default
    /// style are left out. A row too large for a record loses its links.
    fn encode_row(&self, row: &Row) -> (ext::Row, usize) {
        let encoded = self.encode_row_with(row, true);
        let size = json_len(&encoded);
        if size + PAGE_OVERHEAD <= PAYLOAD_MAX {
            return (encoded, size);
        }
        let encoded = self.encode_row_with(row, false);
        let size = json_len(&encoded);
        (encoded, size)
    }

    fn encode_row_with(&self, row: &Row, links: bool) -> ext::Row {
        let plain = Style::plain();
        let style = |attrs: Attrs| {
            let mut style = self.style(attrs);
            if !links {
                style.link = None;
            }
            style
        };
        let end = row
            .cells
            .iter()
            .rposition(|cell| {
                !(cell.width == 1
                    && cell.ch == ' '
                    && cell.combining.is_empty()
                    && style(cell.attrs) == plain)
            })
            .map_or(0, |last| last + 1);
        let mut runs: Vec<ext::Run> = Vec::new();
        let mut last_attrs = None;
        for cell in &row.cells[..end] {
            if cell.width == 0 && !runs.is_empty() {
                // The right half of a wide character counts toward its run.
                if let Some(run) = runs.last_mut() {
                    run.cells += 1;
                }
                continue;
            }
            let width = usize::from(cell.width.max(1));
            let mut text = String::new();
            if cell.width == 0 {
                // A right half with no left half shows as a blank.
                text.push(' ');
            } else {
                text.push(cell.ch);
                for &mark in &cell.combining {
                    if text.len() + mark.len_utf8() > 16 * width {
                        break;
                    }
                    text.push(mark);
                }
            }
            match runs.last_mut() {
                Some(run) if last_attrs == Some(cell.attrs) => {
                    run.text.push_str(&text);
                    run.cells += 1;
                }
                _ => {
                    runs.push(ext::Run {
                        text,
                        cells: 1,
                        style: style(cell.attrs),
                    });
                    last_attrs = Some(cell.attrs);
                }
            }
        }
        ext::Row {
            wrapped: row.wrapped,
            runs,
        }
    }

    /// A wire row as cells, at most `cols` wide. Every character must have
    /// a width and the columns must match each run's count.
    fn decode_row(&mut self, row: &ext::Row, cols: usize) -> Result<Row, Refusal> {
        let mut cells: Vec<Cell> = Vec::new();
        for run in &row.runs {
            let attrs = self.attrs(&run.style)?;
            let start = cells.len();
            if run.text.len() > usize::from(run.cells) * 16 {
                return Err(malformed("a run's text is longer than its cells allow"));
            }
            for character in run.text.chars() {
                match unicode_width::UnicodeWidthChar::width(character) {
                    None => return Err(malformed("a run has a control character")),
                    Some(0) => {
                        let Some(cell) =
                            cells[start..].iter_mut().rev().find(|cell| cell.width > 0)
                        else {
                            return Err(malformed("a run starts with a combining character"));
                        };
                        if cell.combining.len() < 8 {
                            cell.combining.push(character);
                        }
                    }
                    Some(width) => {
                        cells.push(Cell {
                            ch: character,
                            combining: Vec::new(),
                            attrs,
                            width: width.min(2) as u8,
                        });
                        if width >= 2 {
                            cells.push(Cell::spacer(attrs));
                        }
                    }
                }
                if cells.len() > cols {
                    return Err(malformed("a row is wider than the terminal"));
                }
            }
            if cells.len() - start != usize::from(run.cells) || run.cells == 0 {
                return Err(malformed("a run's cells do not match its text"));
            }
        }
        Ok(Row {
            cells,
            wrapped: row.wrapped,
        })
    }
}

fn json_len(row: &ext::Row) -> usize {
    serde_json::to_vec(row).map_or(usize::MAX / 2, |bytes| bytes.len() + 1)
}
