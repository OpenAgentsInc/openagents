//! The apply-patch format, reimplemented from OpenAI's Codex (Apache-2.0).
//!
//! GPT-6 models are trained on this format, so Microluna's patch tool
//! takes it unchanged. The grammar follows `codex-rs/apply-patch/src/parser.rs`:
//!
//! ```text
//! *** Begin Patch
//! *** Add File: <path>
//! +<line>
//! *** Delete File: <path>
//! *** Update File: <path>
//! *** Move to: <path>          (optional)
//! @@ <context line>            (optional, repeatable)
//!  <unchanged line>
//! -<removed line>
//! +<added line>
//! *** End of File              (optional: the chunk ends the file)
//! *** End Patch
//! ```
//!
//! A chunk's old lines are found with decreasing strictness — exact, then
//! ignoring trailing whitespace, then ignoring surrounding whitespace — as
//! `codex-rs/apply-patch/src/seek_sequence.rs` does. This module parses and
//! rewrites text only; [`crate::tools::Workspace`] reads and writes files.

use std::fmt;

/// One file operation in a patch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Hunk {
    /// Create a file with these contents.
    Add {
        /// The path, as written in the patch.
        path: String,
        /// The whole new file.
        contents: String,
    },
    /// Remove a file.
    Delete {
        /// The path, as written in the patch.
        path: String,
    },
    /// Change a file in place, and optionally rename it.
    Update {
        /// The path, as written in the patch.
        path: String,
        /// The new path, when the file moves.
        move_to: Option<String>,
        /// The changes, in file order.
        chunks: Vec<Chunk>,
    },
}

/// One run of changes inside an updated file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Chunk {
    /// A line to find before the change, from `@@ <line>`.
    pub context: Option<String>,
    /// The lines the chunk replaces: context and removed lines.
    pub old: Vec<String>,
    /// The lines that replace them: context and added lines.
    pub new: Vec<String>,
    /// Whether the chunk must match at the end of the file.
    pub eof: bool,
}

/// Why a patch couldn't be parsed or applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatchError(pub String);

impl fmt::Display for PatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for PatchError {}

fn error(message: impl Into<String>) -> PatchError {
    PatchError(message.into())
}

/// Parses a whole patch.
///
/// # Errors
///
/// A [`PatchError`] naming the first line that breaks the grammar.
pub fn parse(text: &str) -> Result<Vec<Hunk>, PatchError> {
    let lines: Vec<&str> = text.trim().lines().collect();
    let (Some(first), Some(last)) = (lines.first(), lines.last()) else {
        return Err(error("the patch is empty"));
    };
    if first.trim() != "*** Begin Patch" {
        return Err(error("the first line must be '*** Begin Patch'"));
    }
    if last.trim() != "*** End Patch" {
        return Err(error("the last line must be '*** End Patch'"));
    }
    let body = &lines[1..lines.len() - 1];
    let mut hunks = Vec::new();
    let mut at = 0;
    while at < body.len() {
        let line = body[at].trim();
        if line.is_empty() {
            at += 1;
        } else if let Some(path) = line.strip_prefix("*** Add File: ") {
            at += 1;
            let mut contents = String::new();
            while at < body.len() && !body[at].trim_start().starts_with("*** ") {
                let Some(added) = body[at].strip_prefix('+') else {
                    return Err(error(format!(
                        "line {} of an added file must start with '+'",
                        at + 2
                    )));
                };
                contents.push_str(added);
                contents.push('\n');
                at += 1;
            }
            hunks.push(Hunk::Add {
                path: path.trim().to_string(),
                contents,
            });
        } else if let Some(path) = line.strip_prefix("*** Delete File: ") {
            hunks.push(Hunk::Delete {
                path: path.trim().to_string(),
            });
            at += 1;
        } else if let Some(path) = line.strip_prefix("*** Update File: ") {
            at += 1;
            let mut move_to = None;
            if let Some(next) = body.get(at)
                && let Some(to) = next.trim().strip_prefix("*** Move to: ")
            {
                move_to = Some(to.trim().to_string());
                at += 1;
            }
            let (chunks, next) = update_chunks(body, at)?;
            at = next;
            if chunks.is_empty() && move_to.is_none() {
                return Err(error(format!(
                    "the update to {} changes nothing",
                    path.trim()
                )));
            }
            hunks.push(Hunk::Update {
                path: path.trim().to_string(),
                move_to,
                chunks,
            });
        } else {
            return Err(error(format!(
                "line {} is not a file operation: '{line}'",
                at + 2
            )));
        }
    }
    if hunks.is_empty() {
        return Err(error("the patch has no file operations"));
    }
    Ok(hunks)
}

fn update_chunks(body: &[&str], mut at: usize) -> Result<(Vec<Chunk>, usize), PatchError> {
    let mut chunks: Vec<Chunk> = Vec::new();
    let mut current: Option<Chunk> = None;
    while at < body.len() {
        let raw = body[at];
        let trimmed = raw.trim();
        if trimmed == "*** End of File" {
            current.get_or_insert_with(Chunk::default).eof = true;
            at += 1;
            continue;
        }
        if trimmed.starts_with("*** ") {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix("@@") {
            if let Some(done) = current.take() {
                chunks.push(done);
            }
            let rest = rest.trim();
            current = Some(Chunk {
                context: (!rest.is_empty()).then(|| rest.to_string()),
                ..Chunk::default()
            });
            at += 1;
            continue;
        }
        let chunk = current.get_or_insert_with(Chunk::default);
        if raw.is_empty() {
            chunk.old.push(String::new());
            chunk.new.push(String::new());
        } else if let Some(kept) = raw.strip_prefix(' ') {
            chunk.old.push(kept.to_string());
            chunk.new.push(kept.to_string());
        } else if let Some(removed) = raw.strip_prefix('-') {
            chunk.old.push(removed.to_string());
        } else if let Some(added) = raw.strip_prefix('+') {
            chunk.new.push(added.to_string());
        } else {
            return Err(error(format!(
                "line {} of an update must start with ' ', '-', '+', or '@@': '{raw}'",
                at + 2
            )));
        }
        at += 1;
    }
    if let Some(done) = current {
        chunks.push(done);
    }
    Ok((chunks, at))
}

/// Applies an update's chunks to a file's text.
///
/// # Errors
///
/// A [`PatchError`] when a context line or a chunk's old lines can't be
/// found.
pub fn apply(original: &str, chunks: &[Chunk]) -> Result<String, PatchError> {
    let mut lines: Vec<String> = original.split('\n').map(str::to_string).collect();
    if lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    let mut replacements: Vec<(usize, usize, Vec<String>)> = Vec::new();
    let mut cursor = 0;
    for chunk in chunks {
        if let Some(context) = &chunk.context {
            let Some(found) = seek(&lines, std::slice::from_ref(context), cursor, false) else {
                return Err(error(format!("can't find the context line '{context}'")));
            };
            cursor = found + 1;
        }
        if chunk.old.is_empty() {
            let at = if chunk.context.is_some() && !chunk.eof {
                cursor
            } else {
                lines.len()
            };
            replacements.push((at, 0, chunk.new.clone()));
            continue;
        }
        let mut old = chunk.old.as_slice();
        let mut new = chunk.new.as_slice();
        let mut found = seek(&lines, old, cursor, chunk.eof);
        if found.is_none() && old.last().is_some_and(String::is_empty) {
            old = &old[..old.len() - 1];
            if new.last().is_some_and(String::is_empty) {
                new = &new[..new.len() - 1];
            }
            found = seek(&lines, old, cursor, chunk.eof);
        }
        let Some(at) = found else {
            return Err(error(format!(
                "can't find these lines:\n{}",
                chunk.old.join("\n")
            )));
        };
        replacements.push((at, old.len(), new.to_vec()));
        cursor = at + old.len();
    }
    replacements.sort_by_key(|(at, _, _)| *at);
    for (at, removed, added) in replacements.into_iter().rev() {
        lines.splice(at..at + removed, added);
    }
    let mut text = lines.join("\n");
    text.push('\n');
    Ok(text)
}

/// Finds `pattern` in `lines` at or after `start`, exactly, then ignoring
/// trailing whitespace, then ignoring surrounding whitespace. With `eof`,
/// the end of the file is tried first.
fn seek(lines: &[String], pattern: &[String], start: usize, eof: bool) -> Option<usize> {
    if pattern.is_empty() {
        return Some(start);
    }
    if pattern.len() > lines.len() {
        return None;
    }
    let last = lines.len() - pattern.len();
    let from = if eof { last } else { start };
    let tests: [fn(&str, &str) -> bool; 3] = [
        |a, b| a == b,
        |a, b| a.trim_end() == b.trim_end(),
        |a, b| a.trim() == b.trim(),
    ];
    for same in tests {
        for at in from.min(last + 1)..=last {
            if lines[at..at + pattern.len()]
                .iter()
                .zip(pattern)
                .all(|(line, want)| same(line, want))
            {
                return Some(at);
            }
        }
    }
    if eof && start < from {
        return seek(lines, pattern, start, false);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn update(patch: &str) -> Vec<Chunk> {
        match parse(patch).unwrap().remove(0) {
            Hunk::Update { chunks, .. } => chunks,
            other => panic!("not an update: {other:?}"),
        }
    }

    #[test]
    fn parses_every_operation() {
        let hunks = parse(
            "*** Begin Patch\n*** Add File: a.txt\n+one\n+two\n*** Delete File: b.txt\n\
             *** Update File: c.txt\n*** Move to: d.txt\n@@ fn main\n-old\n+new\n*** End Patch\n",
        )
        .unwrap();
        assert_eq!(
            hunks[0],
            Hunk::Add {
                path: "a.txt".into(),
                contents: "one\ntwo\n".into()
            }
        );
        assert_eq!(
            hunks[1],
            Hunk::Delete {
                path: "b.txt".into()
            }
        );
        let Hunk::Update {
            move_to, chunks, ..
        } = &hunks[2]
        else {
            panic!("not an update");
        };
        assert_eq!(move_to.as_deref(), Some("d.txt"));
        assert_eq!(chunks[0].context.as_deref(), Some("fn main"));
        assert_eq!(chunks[0].old, vec!["old"]);
        assert_eq!(chunks[0].new, vec!["new"]);
    }

    #[test]
    fn refuses_a_patch_without_its_envelope() {
        assert!(parse("*** Add File: a\n+x\n").is_err());
        assert!(parse("*** Begin Patch\n*** End Patch").is_err());
        assert!(parse("*** Begin Patch\nrandom\n*** End Patch").is_err());
    }

    #[test]
    fn replaces_lines_after_a_context_marker() {
        let chunks =
            update("*** Begin Patch\n*** Update File: f\n@@ second\n x\n-y\n+Y\n*** End Patch");
        let text = "first\nx\ny\nsecond\nx\ny\n";
        assert_eq!(apply(text, &chunks).unwrap(), "first\nx\ny\nsecond\nx\nY\n");
    }

    #[test]
    fn tolerates_whitespace_drift_in_old_lines() {
        let chunks =
            update("*** Begin Patch\n*** Update File: f\n-  value = 1\n+value = 2\n*** End Patch");
        assert_eq!(apply("value = 1   \n", &chunks).unwrap(), "value = 2\n");
    }

    #[test]
    fn a_pure_addition_after_context_lands_there() {
        let chunks = update("*** Begin Patch\n*** Update File: f\n@@ a\n+b\n*** End Patch");
        assert_eq!(apply("a\nc\n", &chunks).unwrap(), "a\nb\nc\n");
    }

    #[test]
    fn an_end_of_file_chunk_matches_the_last_occurrence() {
        let chunks =
            update("*** Begin Patch\n*** Update File: f\n-x\n+z\n*** End of File\n*** End Patch");
        assert_eq!(apply("x\ny\nx\n", &chunks).unwrap(), "x\ny\nz\n");
    }

    #[test]
    fn missing_lines_are_an_error_that_names_them() {
        let chunks = update("*** Begin Patch\n*** Update File: f\n-absent\n+x\n*** End Patch");
        let error = apply("present\n", &chunks).unwrap_err();
        assert!(error.0.contains("absent"));
    }
}
