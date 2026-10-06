//! A host's authoritative emulator for one terminal
//! (`coder_pty::emulator`): the one parse of the terminal's output that
//! answers the program's queries, reports its side effects, writes the
//! snapshot and history streams a joining device restores from, and keeps
//! the terminal's block journal.

use std::sync::Arc;

use coder_pty::emulator::{Effects, Emulator, Emulators, Factory, HistoryRead};
use coder_pty::ext::{BlockPage, Record};
use coder_pty::wire::{Exit, Refusal, Size, TerminalRef};

use crate::Terminal;
use crate::journal::Journal;
use crate::shell::Event;
use crate::snapshot::Binding;

/// A terminal the host parses its output into, and what it last reported.
#[derive(Debug)]
pub struct Authority {
    terminal: Terminal,
    bells: u64,
    title: String,
    directory: Option<String>,
    journal: Journal,
    at_prompt: bool,
    buffer: Option<String>,
    prompt_through: Option<u64>,
}

/// Makes an [`Authority`] for each terminal.
#[derive(Clone, Copy, Debug)]
struct Authorities {
    scrollback: usize,
}

impl Emulators for Authorities {
    fn make(&self, size: Size) -> Box<dyn Emulator> {
        Box::new(Authority::new(size, self.scrollback))
    }

    fn snapshots(&self) -> bool {
        true
    }

    fn blocks(&self) -> bool {
        true
    }
}

impl Authority {
    /// An emulator at `size` keeping at most `scrollback` history lines.
    #[must_use]
    pub fn new(size: Size, scrollback: usize) -> Self {
        Authority {
            terminal: Terminal::new(usize::from(size.rows), usize::from(size.cols), scrollback),
            bells: 0,
            title: String::new(),
            directory: None,
            journal: Journal::default(),
            at_prompt: false,
            buffer: None,
            prompt_through: None,
        }
    }

    /// A factory that makes one per terminal, each keeping at most
    /// `scrollback` history lines, for `coder_pty::host::Config`.
    #[must_use]
    pub fn factory(scrollback: usize) -> Factory {
        Arc::new(Authorities { scrollback })
    }

    /// The parsed terminal.
    #[must_use]
    pub fn terminal(&self) -> &Terminal {
        &self.terminal
    }

    /// The terminal's block journal.
    #[must_use]
    pub fn journal(&self) -> &Journal {
        &self.journal
    }
}

impl Emulator for Authority {
    fn empty_prompt(&self) -> bool {
        self.at_prompt
            && !self.terminal.alternate_screen()
            && self.buffer.as_ref().is_none_or(String::is_empty)
    }

    fn prompt_through(&self) -> Option<u64> {
        self.prompt_through
    }

    fn output(&mut self, bytes: &[u8], seq: u64) -> Effects {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| {
                u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
            });
        let terminal = &mut self.terminal;
        terminal.feed(bytes);
        let bells = terminal.bells() - self.bells;
        self.bells = terminal.bells();
        let title = (terminal.title() != self.title).then(|| {
            self.title = terminal.title().to_owned();
            self.title.clone()
        });
        let mut directory = None;
        for mark in terminal.take_shell_marks() {
            match &mark.event {
                Event::Prompt => {
                    self.prompt_through = Some(seq);
                    self.at_prompt = true;
                    self.buffer = None;
                }
                Event::Buffer(buffer) if self.at_prompt => {
                    self.prompt_through = Some(seq);
                    self.buffer = Some(buffer.clone());
                }
                Event::Gap | Event::Output => {
                    self.at_prompt = false;
                    self.buffer = None;
                }
                _ => {}
            }
            self.journal.mark(&mark, seq, now, terminal);
            if let Event::Directory(dir) = mark.event
                && self.directory.as_ref() != Some(&dir)
            {
                self.directory = Some(dir.clone());
                directory = Some(dir);
            }
        }
        self.journal.output(seq, terminal.alternate_screen());
        Effects {
            replies: terminal.take_replies(),
            bells: u32::try_from(bells).unwrap_or(u32::MAX),
            title,
            directory,
            clipboard: terminal.take_clipboard(),
        }
    }

    fn resize(&mut self, size: Size) {
        self.terminal
            .resize(usize::from(size.rows), usize::from(size.cols));
    }

    fn snapshot(
        &mut self,
        terminal: &TerminalRef,
        through: u64,
        exit: Option<Exit>,
    ) -> Option<Result<Vec<Record>, Refusal>> {
        let binding = Binding {
            terminal: terminal.clone(),
            through,
            exit,
        };
        Some(self.terminal.snapshot(&binding))
    }

    fn history(
        &self,
        terminal: &TerminalRef,
        read: &HistoryRead,
    ) -> Option<Result<Vec<Record>, Refusal>> {
        let binding = Binding {
            terminal: terminal.clone(),
            through: read.through,
            exit: read.exit,
        };
        Some(
            self.terminal
                .history_stream(&binding, read.epoch, read.before, read.rows),
        )
    }

    fn blocks(&self, before: Option<u64>, limit: u16) -> Option<Result<BlockPage, Refusal>> {
        Some(self.journal.page(before, limit))
    }

    fn attribute(&mut self, origin: coder_pty::ext::Origin) {
        self.journal.attribute(origin);
    }

    fn blocks_from(
        &self,
        from: u64,
        before: Option<u64>,
        limit: u16,
    ) -> Option<Result<BlockPage, Refusal>> {
        Some(self.journal.page_from(from, before, limit))
    }
}
