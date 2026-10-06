//! Command records anchored to the emulator's absolute primary-screen lines.

use coder_vt::{
    Terminal,
    shell::{Event, Mark},
};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

pub const MAX_BLOCKS: usize = 256;
pub const MAX_OUTPUT: usize = 16 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub line: u64,
    pub col: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    pub id: u64,
    pub command: String,
    pub cwd: Option<String>,
    pub start: Position,
    pub end: Option<Position>,
    pub status: Option<i32>,
    pub started_ms: u64,
    pub elapsed_ms: Option<u64>,
    pub output: String,
    pub truncated: bool,
    pub collapsed: bool,
    /// The command entered the alternate screen while it ran: its output
    /// is a full-screen program's, not lines.
    #[serde(default)]
    pub alternate: bool,
}

#[derive(Default)]
pub struct Blocks {
    pub records: VecDeque<Block>,
    pub cwd: Option<String>,
    pub buffer: Option<String>,
    /// The shell's report on the buffer's first word, for routing.
    pub word: Option<String>,
    /// The shell's command table, as its hook last reported it.
    pub table: Option<String>,
    pub at_prompt: bool,
    /// Explicit prompt and editor marks observed since this projection started.
    pub prompt_revision: u64,
    pub request: Option<String>,
    input: Option<Position>,
    command: Option<String>,
    active: Option<u64>,
    next: u64,
}

impl Blocks {
    /// Restores retained command anchors only when they belong to the current screen epoch.
    pub fn restore_journal(&mut self, page: &coder_pty::ext::BlockPage, vt: &Terminal) {
        self.records.clear();
        self.cwd = page.blocks.first().map(|b| b.dir.clone());
        self.next = page.newest.unwrap_or(0);
        self.active = None;
        for block in page.blocks.iter().rev() {
            let Some(lines) = block.lines.filter(|lines| lines.epoch == vt.line_epoch()) else {
                continue;
            };
            self.records.push_back(Block {
                id: block.block,
                command: block.command.clone(),
                cwd: Some(block.dir.clone()),
                start: Position {
                    line: lines.start,
                    col: 0,
                },
                end: Some(Position {
                    line: lines.end,
                    col: 0,
                }),
                status: block.status,
                started_ms: block.started.unwrap_or(0),
                elapsed_ms: block
                    .ended
                    .zip(block.started)
                    .map(|(end, start)| end.saturating_sub(start)),
                output: String::new(),
                truncated: true,
                collapsed: false,
                alternate: block.alternate,
            });
        }
    }

    /// Consumes advisory marks after applying a bounded frame of output.
    pub fn update(&mut self, vt: &mut Terminal, now_ms: u64) {
        for mark in vt.take_shell_marks() {
            self.apply(mark, vt, now_ms);
        }
        if vt.alternate_screen()
            && let Some(id) = self.active
            && let Some(block) = self.records.iter_mut().find(|block| block.id == id)
        {
            block.alternate = true;
        }
    }

    fn apply(&mut self, mark: Mark, vt: &Terminal, now_ms: u64) {
        let point = Position {
            line: mark.line,
            col: mark.col,
        };
        match mark.event {
            Event::Gap => {
                self.active = None;
                self.input = None;
                self.command = None;
                self.buffer = None;
                self.word = None;
                self.at_prompt = false;
                self.request = None;
            }
            Event::Directory(cwd) => self.cwd = Some(cwd),
            Event::Buffer(buffer) if self.at_prompt => {
                self.prompt_revision = self.prompt_revision.saturating_add(1);
                self.buffer = Some(buffer);
            }
            Event::Word(word) if self.at_prompt => self.word = Some(word),
            Event::Table(table) => self.table = Some(table),
            Event::Request(request) if self.at_prompt => self.request = Some(request),
            Event::Buffer(_) | Event::Word(_) | Event::Request(_) => {}
            Event::Command(command) => self.command = Some(command),
            Event::Prompt => {
                self.prompt_revision = self.prompt_revision.saturating_add(1);
                // A missing completion mark leaves an uncertain block, not a success.
                self.active = None;
                self.input = None;
                self.command = None;
                self.buffer = None;
                self.word = None;
                self.at_prompt = true;
            }
            Event::Input => self.input = Some(point),
            Event::Output => {
                self.at_prompt = false;
                self.buffer = None;
                self.word = None;
                if self.active.is_some() {
                    return;
                }
                let command = self
                    .command
                    .take()
                    .or_else(|| {
                        let input = self.input.as_ref()?;
                        Some(extract(vt, input, &point).0.trim().to_owned())
                    })
                    .unwrap_or_default();
                self.next += 1;
                let id = self.next;
                if self.records.len() == MAX_BLOCKS {
                    self.records.pop_front();
                }
                self.records.push_back(Block {
                    id,
                    command,
                    cwd: self.cwd.clone(),
                    start: point,
                    end: None,
                    status: None,
                    started_ms: now_ms,
                    elapsed_ms: None,
                    output: String::new(),
                    truncated: false,
                    collapsed: false,
                    alternate: false,
                });
                self.active = Some(id);
            }
            Event::Finished { status } => {
                if let Some(id) = self.active.take()
                    && let Some(block) = self.records.iter_mut().find(|block| block.id == id)
                {
                    if point.line < block.start.line {
                        return;
                    }
                    (block.output, block.truncated) = extract(vt, &block.start, &point);
                    block.end = Some(point);
                    block.status = status;
                    block.elapsed_ms = Some(now_ms.saturating_sub(block.started_ms));
                }
            }
        }
    }

    pub fn get(&self, id: u64) -> Option<&Block> {
        self.records.iter().find(|block| block.id == id)
    }

    pub fn collapse(&mut self, id: u64) -> bool {
        if let Some(block) = self.records.iter_mut().find(|block| block.id == id) {
            block.collapsed = !block.collapsed;
            true
        } else {
            false
        }
    }
}

/// What a running block has printed so far, up to the cursor.
#[must_use]
pub fn live(vt: &Terminal, block: &Block) -> String {
    let (row, col) = vt.cursor();
    let to = Position {
        line: vt.history_dropped() + (vt.scrollback_len() + row) as u64,
        col,
    };
    if to.line < block.start.line {
        return String::new();
    }
    extract(vt, &block.start, &to).0
}

fn extract(vt: &Terminal, from: &Position, to: &Position) -> (String, bool) {
    let dropped = vt.history_dropped();
    let last = dropped + (vt.scrollback_len() + vt.rows()) as u64;
    let mut truncated = from.line < dropped;
    let mut text = String::new();
    for absolute in from.line.max(dropped)..=to.line.min(last.saturating_sub(1)) {
        let Some(row) = vt.line((absolute - dropped) as usize) else {
            break;
        };
        let start = if absolute == from.line { from.col } else { 0 };
        let end = if absolute == to.line {
            to.col
        } else {
            row.cells.len()
        };
        let piece = row.text_between(start, end);
        let separator = if !row.wrapped && absolute < to.line {
            "\n"
        } else {
            ""
        };
        if text.len() + piece.len() + separator.len() > MAX_OUTPUT {
            truncated = true;
            break;
        }
        text.push_str(&piece);
        text.push_str(separator);
    }
    (text.trim_end_matches('\n').to_owned(), truncated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_keep_status_directory_output_and_distinct_reruns() {
        let mut vt = Terminal::new(4, 80, 100);
        let mut blocks = Blocks::default();
        vt.feed(b"\x1b]7;file:///tmp/repo\x07\x1b]133;A\x07$ \x1b]133;B\x07false\r\n\x1b]777;openagents;command;66616c7365\x07\x1b]133;C\x07");
        blocks.update(&mut vt, 10);
        vt.feed(b"failed\r\n\x1b]133;D;1\x07\x1b]133;A\x07");
        blocks.update(&mut vt, 30);
        let block = blocks.get(1).unwrap();
        assert_eq!(block.command, "false");
        assert_eq!(block.cwd.as_deref(), Some("/tmp/repo"));
        assert_eq!(block.status, Some(1));
        assert_eq!(block.output, "failed");
        assert_eq!(block.elapsed_ms, Some(20));
        vt.feed(b"\x1b]777;openagents;command;66616c7365\x07\x1b]133;C\x07\x1b]133;D;1\x07");
        blocks.update(&mut vt, 40);
        assert_eq!(blocks.records.len(), 2);
        assert_ne!(blocks.records[0].id, blocks.records[1].id);
    }

    #[test]
    fn forged_marks_and_gaps_never_complete_a_previous_command() {
        let mut vt = Terminal::new(2, 80, 2);
        let mut blocks = Blocks::default();
        vt.feed(b"\x1b]133;C\x07");
        blocks.update(&mut vt, 0);
        vt.mark("lost");
        vt.feed(b"\x1b]133;D;0\x07");
        blocks.update(&mut vt, 10);
        assert!(blocks.records[0].status.is_none());
        assert!(blocks.records[0].end.is_none());
    }
}
