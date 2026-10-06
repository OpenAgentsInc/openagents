//! A host's block journal for one terminal (NIP-TERM's blocks feature):
//! each command's record from the shell-integration marks, without its
//! output, and the sequence numbers of that output.
//!
//! Marks are advisory. A program can print OSC 133 itself, so a record
//! shapes how a client draws and navigates and never authorizes anything.
//! A command that ends without its end mark is `abandoned`, not finished,
//! and every record is `unattributed` until an attributed operation can
//! start a command.

use std::collections::VecDeque;

use coder_pty::ext::{
    BLOCK_LIMIT_MAX, BLOCK_TEXT_MAX, Block, BlockPage, BlockState, Lines, Origin, SeqRange,
};
use coder_pty::wire::{Reason, Refusal};

use crate::Terminal;
use crate::shell::{Event, Mark};

/// The most blocks a journal keeps; older ones leave it.
pub const JOURNAL_MAX: usize = 256;

/// Where a command's input began, by absolute line and column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Point {
    line: u64,
    col: usize,
}

/// One terminal's block journal.
#[derive(Debug, Default)]
pub struct Journal {
    blocks: VecDeque<Block>,
    next: u64,
    /// The running block's number.
    active: Option<u64>,
    /// The command line the shell's hook reported for the next block.
    command: Option<String>,
    input: Option<Point>,
    dir: String,
}

impl Journal {
    /// Applies one mark the emulator parsed from the output frame `seq`,
    /// at Unix milliseconds `now`.
    pub(crate) fn mark(&mut self, mark: &Mark, seq: u64, now: u64, terminal: &Terminal) {
        let point = Point {
            line: mark.line,
            col: mark.col,
        };
        match &mark.event {
            Event::Directory(dir) => self.dir = bounded(dir).0,
            Event::Command(command) => self.command = Some(command.clone()),
            Event::Input => self.input = Some(point),
            Event::Prompt | Event::Gap => {
                self.abandon(now);
                self.input = None;
                self.command = None;
            }
            Event::Output => {
                if self.active.is_some() {
                    return;
                }
                let text = self.command.take().unwrap_or_else(|| {
                    self.input
                        .map(|input| typed(terminal, input, point))
                        .unwrap_or_default()
                });
                let (command, command_truncated) = bounded(text.trim());
                self.next += 1;
                if self.blocks.len() == JOURNAL_MAX {
                    self.blocks.pop_front();
                }
                self.blocks.push_back(Block {
                    block: self.next,
                    origin: Origin::Unattributed,
                    command,
                    command_truncated,
                    dir: self.dir.clone(),
                    started: Some(now),
                    ended: None,
                    status: None,
                    state: BlockState::Running,
                    alternate: false,
                    output: Some(SeqRange { from: seq, to: seq }),
                    retained: false,
                    lines: Some(Lines {
                        epoch: terminal.line_epoch(),
                        start: point.line,
                        end: point.line,
                    }),
                });
                self.active = Some(self.next);
            }
            Event::Finished { status } => {
                let Some(block) = self.running() else {
                    return;
                };
                block.ended = Some(now);
                block.status = *status;
                block.state = BlockState::Finished;
                if let Some(output) = &mut block.output {
                    output.to = seq.max(output.from);
                }
                block.lines = block.lines.and_then(|lines| {
                    (lines.epoch == terminal.line_epoch() && point.line >= lines.start).then_some(
                        Lines {
                            end: point.line,
                            ..lines
                        },
                    )
                });
                self.active = None;
            }
            Event::Buffer(_) | Event::Word(_) | Event::Table(_) | Event::Request(_) => {}
        }
    }

    /// Output frame `seq` arrived while the alternate screen shows when
    /// `alternate` is set.
    pub(crate) fn output(&mut self, seq: u64, alternate: bool) {
        let Some(block) = self.running() else {
            return;
        };
        if alternate {
            // A full-screen program's output is not a transcript.
            block.alternate = true;
            block.output = None;
        } else if let Some(output) = &mut block.output {
            output.to = seq.max(output.from);
        }
    }

    fn running(&mut self) -> Option<&mut Block> {
        let active = self.active?;
        self.blocks
            .iter_mut()
            .rev()
            .find(|block| block.block == active)
    }

    /// A new prompt or lost marks end the running block without its end
    /// mark.
    fn abandon(&mut self, now: u64) {
        if let Some(block) = self.running() {
            block.state = BlockState::Abandoned;
            block.ended = Some(now);
        }
        self.active = None;
    }

    /// A page of at most `limit` blocks older than `before`, or the newest,
    /// newest first. `retained` is false on every block; the host, which
    /// holds the replay buffer, fills it.
    pub fn page(&self, before: Option<u64>, limit: u16) -> Result<BlockPage, Refusal> {
        if limit == 0 || limit > BLOCK_LIMIT_MAX {
            return Err(Refusal::new(
                Reason::Malformed,
                "a block page asks 1 to 32 blocks",
            ));
        }
        let oldest = self.blocks.front().map(|block| block.block);
        let newest = self.blocks.back().map(|block| block.block);
        // Every block below `before` left the journal: blocks 1 up to the
        // oldest kept one are gone.
        if let Some(before) = before
            && before > 1
            && oldest.is_none_or(|oldest| before <= oldest)
            && self.next > 0
        {
            return Err(Refusal::new(
                Reason::ContentUnavailable,
                "those blocks left the journal",
            ));
        }
        let blocks: Vec<Block> = self
            .blocks
            .iter()
            .rev()
            .filter(|block| before.is_none_or(|before| block.block < before))
            .take(usize::from(limit))
            .cloned()
            .collect();
        let more = blocks
            .last()
            .zip(oldest)
            .is_some_and(|(last, oldest)| last.block > oldest);
        let mut page = BlockPage {
            newest,
            oldest,
            blocks,
            more,
        };
        while !page.fits() && !page.blocks.is_empty() {
            page.blocks.pop();
            page.more = true;
        }
        Ok(page)
    }
}

/// `text` without control characters, cut to the record bound at a
/// character boundary, and whether it was cut.
fn bounded(text: &str) -> (String, bool) {
    let clean: String = text.chars().filter(|c| !c.is_control()).collect();
    if clean.len() <= BLOCK_TEXT_MAX {
        return (clean, false);
    }
    let mut end = BLOCK_TEXT_MAX;
    while !clean.is_char_boundary(end) {
        end -= 1;
    }
    (clean[..end].to_owned(), true)
}

/// The text typed between the input mark and the output mark.
fn typed(terminal: &Terminal, from: Point, to: Point) -> String {
    let dropped = terminal.history_dropped();
    let mut text = String::new();
    for line in from.line.max(dropped)..=to.line {
        let Some(row) = terminal.line((line - dropped) as usize) else {
            break;
        };
        let start = if line == from.line { from.col } else { 0 };
        let end = if line == to.line {
            to.col
        } else {
            row.cells.len()
        };
        text.push_str(&row.text_between(start, end));
        if text.len() > BLOCK_TEXT_MAX * 2 {
            break;
        }
    }
    text
}
