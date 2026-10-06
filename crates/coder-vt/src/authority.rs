//! A host's authoritative emulator for one terminal
//! (`coder_pty::emulator`): the one parse of the terminal's output that
//! answers the program's queries and reports its side effects.

use std::sync::Arc;

use coder_pty::emulator::{Effects, Emulator, Factory};
use coder_pty::wire::Size;

use crate::Terminal;
use crate::shell::Event;

/// A terminal the host parses its output into, and what it last reported.
#[derive(Debug)]
pub struct Authority {
    terminal: Terminal,
    bells: u64,
    title: String,
    directory: Option<String>,
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
        }
    }

    /// A factory that makes one per terminal, for `coder_pty::host::Config`.
    #[must_use]
    pub fn factory(scrollback: usize) -> Factory {
        Arc::new(move |size| Box::new(Authority::new(size, scrollback)) as Box<dyn Emulator>)
    }

    /// The parsed terminal, for snapshots and history reads.
    #[must_use]
    pub fn terminal(&self) -> &Terminal {
        &self.terminal
    }

    /// The parsed terminal, mutably: a snapshot can abandon unfinished
    /// input.
    pub fn terminal_mut(&mut self) -> &mut Terminal {
        &mut self.terminal
    }
}

impl Emulator for Authority {
    fn output(&mut self, bytes: &[u8]) -> Effects {
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
            if let Event::Directory(dir) = mark.event
                && self.directory.as_ref() != Some(&dir)
            {
                self.directory = Some(dir.clone());
                directory = Some(dir);
            }
        }
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
}
