//! Block search (#10731): find retained command blocks by their command
//! and captured output, filtered by exit status and directory. Search reads
//! only what the pane already holds. It runs nothing, attaches nothing, and
//! reports what it could not read instead of guessing at it.
//!
//! A query is words to find, case-insensitive, all of which must appear in
//! the command or the output, and two filters:
//!
//! - `status:ok`, `status:fail`, `status:running`, or `status:N` for exit
//!   status `N`.
//! - `dir:TEXT`, a directory containing `TEXT`.

use crate::blocks::{Block, Blocks};

/// The longest query read, in characters.
pub const MAX_QUERY: usize = 256;
/// The most matches returned, newest first.
pub const MAX_MATCHES: usize = 100;

/// Which exit status a block must have.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Ok,
    Failed,
    Running,
    Code(i32),
}

/// A parsed query.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Query {
    pub words: Vec<String>,
    pub status: Option<Status>,
    pub dir: Option<String>,
}

impl Query {
    /// Reads `text`. A filter that does not parse is searched as a word.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let text: String = text.chars().take(MAX_QUERY).collect();
        let mut query = Query::default();
        for word in text.split_whitespace() {
            if let Some(status) = word.strip_prefix("status:") {
                let status = match status {
                    "ok" => Some(Status::Ok),
                    "fail" | "failed" => Some(Status::Failed),
                    "running" => Some(Status::Running),
                    code => code.parse().ok().map(Status::Code),
                };
                if status.is_some() {
                    query.status = status;
                    continue;
                }
            }
            if let Some(dir) = word.strip_prefix("dir:")
                && !dir.is_empty()
            {
                query.dir = Some(dir.to_owned());
                continue;
            }
            query.words.push(word.to_lowercase());
        }
        query
    }

    /// Whether the query asks for anything.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.words.is_empty() && self.status.is_none() && self.dir.is_none()
    }

    fn admits(&self, block: &Block) -> bool {
        let status = match &self.status {
            None => true,
            Some(Status::Running) => block.end.is_none(),
            Some(Status::Ok) => block.status == Some(0),
            Some(Status::Failed) => block.status.is_some_and(|code| code != 0),
            Some(Status::Code(code)) => block.status == Some(*code),
        };
        let dir = self.dir.as_ref().is_none_or(|dir| {
            block
                .cwd
                .as_deref()
                .is_some_and(|cwd| cwd.contains(dir.as_str()))
        });
        status && dir && {
            let command = block.command.to_lowercase();
            let output = block.output.to_lowercase();
            self.words
                .iter()
                .all(|word| command.contains(word) || output.contains(word))
        }
    }
}

/// What a search found, and what it could not look at.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Found {
    /// Matching block identities, newest first, at most [`MAX_MATCHES`].
    pub ids: Vec<u64>,
    /// How many blocks matched, including any past the bound.
    pub total: usize,
    /// Older blocks the pane no longer keeps.
    pub forgotten: u64,
    /// Searched blocks whose output was cut, by scrollback or by size, so a
    /// word may be in the part that is gone.
    pub cut: usize,
    /// Searched blocks still running, whose output is read only when they
    /// finish.
    pub running: usize,
}

impl Found {
    /// Whether some output could not be searched.
    #[must_use]
    pub fn incomplete(&self) -> bool {
        self.forgotten > 0 || self.cut > 0 || self.running > 0
    }

    /// One line saying what the search could not read.
    #[must_use]
    pub fn gaps(&self) -> Option<String> {
        let mut parts = Vec::new();
        if self.forgotten > 0 {
            parts.push(format!(
                "{} older blocks are no longer kept",
                self.forgotten
            ));
        }
        if self.cut > 0 {
            parts.push(format!("{} blocks' output was cut", self.cut));
        }
        if self.running > 0 {
            parts.push(format!("{} running blocks are not read yet", self.running));
        }
        (!parts.is_empty()).then(|| format!("incomplete: {}", parts.join("; ")))
    }
}

/// Searches `blocks` for `query`, newest first.
#[must_use]
pub fn search(blocks: &Blocks, query: &Query) -> Found {
    let mut found = Found {
        forgotten: blocks
            .records
            .front()
            .map_or(0, |block| block.id.saturating_sub(1)),
        ..Found::default()
    };
    for block in blocks.records.iter().rev() {
        if block.end.is_none() {
            found.running += 1;
        } else if block.truncated {
            found.cut += 1;
        }
        if query.admits(block) {
            found.total += 1;
            if found.ids.len() < MAX_MATCHES {
                found.ids.push(block.id);
            }
        }
    }
    found
}

/// An open block search in a pane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Find {
    pub pane: crate::layout::PaneId,
    pub text: String,
    pub found: Found,
    /// Which match shows, an index into `found.ids`.
    pub at: usize,
}

impl crate::Application {
    /// Opens block search in the focused pane.
    pub fn open_find(&mut self) {
        if let Some(pane) = self.focus_id() {
            self.find = Some(Find {
                pane,
                text: String::new(),
                found: Found::default(),
                at: 0,
            });
        }
    }

    /// Handles a key while block search is open in the focused pane.
    /// Returns whether search took it; while open it takes every key, so
    /// nothing reaches the shell.
    pub fn find_key(&mut self, key: &crate::KeyIn) -> bool {
        use crate::input::KeyCode;
        let focus = self.focus_id();
        let control = self.mods.control_key() || self.mods.super_key();
        let Some(find) = &mut self.find else {
            return false;
        };
        if Some(find.pane) != focus {
            return false;
        }
        match key.code {
            KeyCode::Escape => {
                self.find = None;
                return true;
            }
            KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::ArrowDown => {
                if !find.found.ids.is_empty() {
                    find.at = (find.at + 1).min(find.found.ids.len() - 1);
                }
            }
            KeyCode::ArrowUp => find.at = find.at.saturating_sub(1),
            KeyCode::Backspace => {
                find.text.pop();
                self.refind();
            }
            _ if control => return true,
            _ => {
                let Some(text) = &key.text else {
                    return true;
                };
                find.text.extend(text.chars().filter(|c| !c.is_control()));
                let end = find
                    .text
                    .char_indices()
                    .nth(MAX_QUERY)
                    .map_or(find.text.len(), |(at, _)| at);
                find.text.truncate(end);
                self.refind();
            }
        }
        self.show_found();
        true
    }

    /// Runs the open search again over the pane's blocks.
    fn refind(&mut self) {
        let Some(find) = &mut self.find else {
            return;
        };
        let query = Query::parse(&find.text);
        find.found = match self.panes.get(&find.pane) {
            Some(pane) if !query.is_empty() => search(&pane.session.blocks, &query),
            _ => Found::default(),
        };
        find.at = 0;
    }

    /// Selects and scrolls to the match that shows.
    fn show_found(&mut self) {
        let Some(find) = &self.find else {
            return;
        };
        if let Some(&id) = find.found.ids.get(find.at) {
            let pane = find.pane;
            self.show_block(pane, id);
        }
    }

    /// The search line, while a search is open in the focused pane.
    #[must_use]
    pub fn find_prompt(&self) -> Option<String> {
        let find = self.find.as_ref()?;
        if Some(find.pane) != self.focus_id() {
            return None;
        }
        let place = match find.found.total {
            0 if find.text.trim().is_empty() => {
                "words, status:ok|fail|running|N, dir:TEXT".to_owned()
            }
            0 => "no match".to_owned(),
            total if total > find.found.ids.len() => format!(
                "{} of the newest {} of {total}",
                find.at + 1,
                find.found.ids.len()
            ),
            total => format!("{} of {total}", find.at + 1),
        };
        let gaps = find
            .found
            .gaps()
            .map_or_else(String::new, |gaps| format!(" · {gaps}"));
        Some(format!(
            "find blocks: {}  {place} · Enter or Down older, Up newer, Esc done; Ctrl+B y d r act on it{gaps}",
            find.text
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::Position;

    fn block(id: u64, command: &str, cwd: &str, status: Option<i32>, output: &str) -> Block {
        Block {
            id,
            command: command.into(),
            cwd: Some(cwd.into()),
            start: Position { line: id, col: 0 },
            end: status.map(|_| Position { line: id, col: 1 }),
            status,
            started_ms: 0,
            elapsed_ms: None,
            output: output.into(),
            truncated: false,
            collapsed: false,
            alternate: false,
        }
    }

    fn fixture() -> Blocks {
        let mut blocks = Blocks::default();
        blocks.records.extend([
            block(
                3,
                "cargo test",
                "/work/app",
                Some(101),
                "test résumé ... FAILED",
            ),
            block(4, "cargo test", "/work/app", Some(0), "test résumé ... ok"),
            block(5, "ls", "/work/lib", Some(0), "Cargo.toml src"),
            block(6, "make", "/work/lib", None, ""),
        ]);
        blocks.records[1].truncated = true;
        blocks
    }

    #[test]
    fn queries_match_command_and_output_with_status_and_directory_filters() {
        let blocks = fixture();
        let ids = |text: &str| search(&blocks, &Query::parse(text)).ids;
        // Repeated commands keep their own identities, newest first.
        assert_eq!(ids("cargo test"), [4, 3]);
        assert_eq!(ids("cargo status:fail"), [3]);
        assert_eq!(ids("status:101"), [3]);
        assert_eq!(ids("status:ok dir:lib"), [5]);
        assert_eq!(ids("RÉSUMÉ"), [4, 3]);
        assert_eq!(ids("status:running"), [6]);
        assert_eq!(ids("cargo.toml"), [5]);
        assert!(ids("nothing-like-this").is_empty());
        // A filter that does not parse is a word.
        assert_eq!(Query::parse("status:maybe").words, ["status:maybe"]);
        assert!(Query::parse("  ").is_empty());
    }

    #[test]
    fn a_search_says_what_it_could_not_read() {
        let blocks = fixture();
        let found = search(&blocks, &Query::parse("cargo"));
        assert_eq!(found.forgotten, 2);
        assert_eq!(found.cut, 1);
        assert_eq!(found.running, 1);
        assert_eq!(
            found.gaps().as_deref(),
            Some(
                "incomplete: 2 older blocks are no longer kept; 1 blocks' output was cut; 1 running blocks are not read yet"
            )
        );
        let mut many = Blocks::default();
        for id in 1..=(MAX_MATCHES as u64 + 20) {
            many.records.push_back(block(id, "echo", "/", Some(0), ""));
        }
        let found = search(&many, &Query::parse("echo"));
        assert_eq!(found.ids.len(), MAX_MATCHES);
        assert_eq!(found.total, MAX_MATCHES + 20);
        assert_eq!(found.ids[0], MAX_MATCHES as u64 + 20);
        assert!(!found.incomplete());
    }
}
