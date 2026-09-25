//! Lexicon-free suspects (issue #9652).
//!
//! The v13 `rationale` step lets Jev see only the comments that contain
//! one of [`crate::accept::GENERAL_MARKS`] or
//! [`crate::accept::RATIONALE_MARKS`], and part of that list came from one
//! task's own comments (`docs/coder/design/pattern-components.md`). This
//! mode drops the list: every comment and docstring attached to a
//! function, to the class that encloses it, or to the module goes to the
//! same Jev question, up to [`MAX`]. The trigger is the code's structure,
//! not a phrase.
//!
//! A comment is attached to a function when it sits inside the
//! function's body or directly above its definition (decorators and
//! attributes may sit between). A comment inside a class but outside its
//! methods is attached to the class. A module docstring is attached to
//! the module. Any other comment is attached to nothing and isn't a
//! candidate. Consecutive full-line comments form one candidate, and so
//! does a docstring or a `/* … */` block.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Candidate, Function, Source};

/// How the `rationale` source finds its comments.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CommentMode {
    /// v13's keyword gate: comments that contain a phrase from
    /// [`crate::accept::GENERAL_MARKS`] or
    /// [`crate::accept::RATIONALE_MARKS`]. The default, and in-sample for
    /// `embedding-drift-monitor`, whose comments supplied five of the
    /// phrases.
    #[default]
    Keywords,
    /// Every comment and docstring attached to a function, its class, or
    /// its module, up to [`MAX`], with no phrase filter.
    LexiconFree,
}

impl CommentMode {
    /// The mode as manifests and records spell it.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            CommentMode::Keywords => "keywords",
            CommentMode::LexiconFree => "lexicon-free",
        }
    }

    /// Whether this is the default, for `skip_serializing_if`.
    #[must_use]
    pub fn is_keywords(&self) -> bool {
        *self == CommentMode::Keywords
    }
}

/// Candidates per workspace, at most. When a workspace has more, each
/// file gives one in turn, in file order, so a large file can't crowd out
/// the rest.
pub const MAX: usize = 96;

/// The most characters of one comment a candidate carries.
pub const CHARS: usize = 300;

/// A comment or docstring and what it's attached to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attached {
    pub file: String,
    /// The 1-based line the comment starts on.
    pub line: usize,
    /// The function or class name, or `module`.
    pub owner: String,
    /// The comment's words without comment markers, on one line.
    pub text: String,
}

impl Attached {
    /// The comment as Jev reads it: `path:line: in `owner`: text`.
    #[must_use]
    pub fn state_line(&self) -> String {
        format!(
            "{}:{}: in `{}`: {}",
            self.file, self.line, self.owner, self.text
        )
    }
}

/// One comment block before attachment: its first and last 1-based
/// lines, its text, and whether it's a Python docstring.
#[derive(Clone, Debug)]
struct Block {
    first: usize,
    last: usize,
    text: String,
    docstring: bool,
    /// A comment that trails code on its line.
    trailing: bool,
}

/// The attached comments of every source file the keyword scan reads
/// ([`crate::accept::source_files`], 60 at most), bounded by [`MAX`].
#[must_use]
pub fn attached(workspace: &Path) -> Vec<Attached> {
    let mut by_file: Vec<Vec<Attached>> = Vec::new();
    for path in crate::accept::source_files(workspace, 60) {
        let Ok(text) = std::fs::read_to_string(workspace.join(&path)) else {
            continue;
        };
        let found = attached_in(&path, &text);
        if !found.is_empty() {
            by_file.push(found);
        }
    }
    bounded(by_file, MAX)
}

/// Round-robin over files until `max`, then back in file and line order.
fn bounded(by_file: Vec<Vec<Attached>>, max: usize) -> Vec<Attached> {
    let total: usize = by_file.iter().map(Vec::len).sum();
    if total <= max {
        return by_file.into_iter().flatten().collect();
    }
    let mut taken = vec![0usize; by_file.len()];
    let mut count = 0;
    'outer: loop {
        let mut progressed = false;
        for (k, file) in by_file.iter().enumerate() {
            if taken[k] < file.len() {
                taken[k] += 1;
                count += 1;
                progressed = true;
                if count >= max {
                    break 'outer;
                }
            }
        }
        if !progressed {
            break;
        }
    }
    by_file
        .into_iter()
        .zip(taken)
        .flat_map(|(file, n)| file.into_iter().take(n))
        .collect()
}

/// The attached comments of one file.
#[must_use]
pub fn attached_in(path: &str, text: &str) -> Vec<Attached> {
    let ext = path.rsplit_once('.').map(|(_, ext)| ext);
    let (blocks, functions) = match ext {
        Some("py") => (python_blocks(text), super::python_functions(path, text)),
        Some("sh" | "rb") | None => return Vec::new(),
        Some(_) => (brace_blocks(text), super::brace_functions(path, text)),
    };
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    for block in blocks {
        let owner = if block.docstring && block.first == first_code_line(&lines) {
            Some("module".to_string())
        } else {
            owner_of(&block, &functions, &lines)
        };
        let Some(owner) = owner else {
            continue;
        };
        out.push(Attached {
            file: path.to_string(),
            line: block.first,
            owner,
            text: block.text,
        });
    }
    out
}

/// The 1-based first line that isn't blank, a shebang, or an encoding
/// comment.
fn first_code_line(lines: &[&str]) -> usize {
    lines
        .iter()
        .position(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with("#!") && !t.starts_with("# -*-")
        })
        .map_or(0, |i| i + 1)
}

/// The function a block is attached to: the definition right below it,
/// or else the innermost function or class whose body holds it.
fn owner_of(block: &Block, functions: &[Function], lines: &[&str]) -> Option<String> {
    if !block.trailing && !block.docstring {
        // Decorators and attributes sit between a comment and its item.
        let mut next = block.last;
        while next < lines.len() {
            let t = lines[next].trim();
            if t.starts_with('@') || t.starts_with("#[") {
                next += 1;
            } else {
                break;
            }
        }
        if let Some(f) = functions.iter().find(|f| f.line == next + 1) {
            return Some(f.name.clone());
        }
    }
    functions
        .iter()
        .filter(|f| f.line <= block.first && block.first <= f.end)
        // A docstring or comment on the definition line belongs to it.
        .max_by_key(|f| f.line)
        .map(|f| f.name.clone())
}

/// Words of a comment on one line, clipped; `None` when it has no letter.
fn clean(parts: &[String]) -> Option<String> {
    let joined = parts
        .iter()
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let words = joined.split_whitespace().collect::<Vec<_>>().join(" ");
    words
        .chars()
        .any(char::is_alphabetic)
        .then(|| crate::judge::clip(&words, CHARS))
}

/// Where a `#` comment starts on a Python line, outside strings.
fn python_hash(line: &str) -> Option<usize> {
    let mut quote: Option<char> = None;
    let mut prev = ' ';
    for (i, c) in line.char_indices() {
        match quote {
            Some(q) => {
                if c == q && prev != '\\' {
                    quote = None;
                }
            }
            None => match c {
                '"' | '\'' => quote = Some(c),
                '#' => return Some(i),
                _ => {}
            },
        }
        prev = c;
    }
    None
}

/// Python comment blocks: runs of full-line `#` comments, trailing `#`
/// comments, and docstrings. A triple-quoted string is a docstring when
/// it opens the module or follows a line that ends with a colon.
fn python_blocks(text: &str) -> Vec<Block> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<Block> = Vec::new();
    let mut run: Option<(usize, Vec<String>)> = None;
    let mut previous_code: Option<&str> = None;
    let flush = |run: &mut Option<(usize, Vec<String>)>, out: &mut Vec<Block>, last: usize| {
        if let Some((first, parts)) = run.take()
            && let Some(text) = clean(&parts)
        {
            out.push(Block {
                first,
                last,
                text,
                docstring: false,
                trailing: false,
            });
        }
    };
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let t = line.trim();
        if t.starts_with('#') {
            if t.starts_with("#!") || t.starts_with("# -*-") {
                i += 1;
                continue;
            }
            match &mut run {
                Some((_, parts)) => parts.push(t.trim_start_matches('#').to_string()),
                None => run = Some((i + 1, vec![t.trim_start_matches('#').to_string()])),
            }
            i += 1;
            continue;
        }
        flush(&mut run, &mut out, i);
        if t.is_empty() {
            i += 1;
            continue;
        }
        let bare = t.trim_start_matches(['r', 'R', 'u', 'U', 'b', 'B']);
        let quote = if bare.starts_with("\"\"\"") {
            Some("\"\"\"")
        } else if bare.starts_with("'''") {
            Some("'''")
        } else {
            None
        };
        if let Some(quote) = quote {
            // The string's extent.
            let opened = &bare[3..];
            let mut parts = Vec::new();
            let mut last = i;
            if let Some(end) = opened.find(quote) {
                parts.push(opened[..end].to_string());
            } else {
                parts.push(opened.to_string());
                let mut k = i + 1;
                while k < lines.len() {
                    if let Some(end) = lines[k].find(quote) {
                        parts.push(lines[k][..end].to_string());
                        break;
                    }
                    parts.push(lines[k].to_string());
                    k += 1;
                }
                last = k.min(lines.len() - 1);
            }
            let docstring = previous_code.is_none_or(|p| p.trim_end().ends_with(':'));
            if docstring && let Some(text) = clean(&parts) {
                out.push(Block {
                    first: i + 1,
                    last: last + 1,
                    text,
                    docstring: true,
                    trailing: false,
                });
            }
            previous_code = Some(lines[last]);
            i = last + 1;
            continue;
        }
        if let Some(at) = python_hash(line)
            && let Some(text) = clean(&[line[at..].trim_start_matches('#').to_string()])
        {
            out.push(Block {
                first: i + 1,
                last: i + 1,
                text,
                docstring: false,
                trailing: true,
            });
        }
        previous_code = Some(line.split('#').next().unwrap_or(line));
        i += 1;
    }
    flush(&mut run, &mut out, lines.len());
    out
}

/// Where a `//` or `/*` comment starts on a brace-language line, outside
/// strings.
fn brace_comment(line: &str) -> Option<usize> {
    let mut quote: Option<char> = None;
    let mut prev = ' ';
    let chars: Vec<(usize, char)> = line.char_indices().collect();
    for (n, &(i, c)) in chars.iter().enumerate() {
        match quote {
            Some(q) => {
                if c == q && prev != '\\' {
                    quote = None;
                }
            }
            None => match c {
                '"' | '`' => quote = Some(c),
                '/' if matches!(chars.get(n + 1), Some((_, '/' | '*'))) => return Some(i),
                _ => {}
            },
        }
        prev = c;
    }
    None
}

/// Brace-language comment blocks: runs of full-line `//` comments,
/// `/* … */` blocks, and trailing `//` comments.
fn brace_blocks(text: &str) -> Vec<Block> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<Block> = Vec::new();
    let mut run: Option<(usize, Vec<String>)> = None;
    let flush = |run: &mut Option<(usize, Vec<String>)>, out: &mut Vec<Block>, last: usize| {
        if let Some((first, parts)) = run.take()
            && let Some(text) = clean(&parts)
        {
            out.push(Block {
                first,
                last,
                text,
                docstring: false,
                trailing: false,
            });
        }
    };
    let strip = |l: &str| {
        l.trim()
            .trim_start_matches("/**")
            .trim_start_matches("/*")
            .trim_end_matches("*/")
            .trim_start_matches('*')
            .trim_start_matches('/')
            .to_string()
    };
    let mut i = 0;
    while i < lines.len() {
        let t = lines[i].trim();
        if t.starts_with("//") {
            match &mut run {
                Some((_, parts)) => parts.push(strip(t)),
                None => run = Some((i + 1, vec![strip(t)])),
            }
            i += 1;
            continue;
        }
        flush(&mut run, &mut out, i);
        if t.starts_with("/*") {
            let mut parts = vec![strip(t)];
            let mut k = i;
            while !lines[k].contains("*/") && k + 1 < lines.len() {
                k += 1;
                parts.push(strip(lines[k]));
            }
            if let Some(text) = clean(&parts) {
                out.push(Block {
                    first: i + 1,
                    last: k + 1,
                    text,
                    docstring: false,
                    trailing: false,
                });
            }
            i = k + 1;
            continue;
        }
        if !t.is_empty()
            && let Some(at) = brace_comment(lines[i])
            && let Some(text) = clean(&[strip(&lines[i][at..])])
        {
            out.push(Block {
                first: i + 1,
                last: i + 1,
                text,
                docstring: false,
                trailing: true,
            });
        }
        i += 1;
    }
    flush(&mut run, &mut out, lines.len());
    out
}

/// The lexicon-free `rationale` candidates: each attached comment, whose
/// evidence is its [`Attached::state_line`].
#[must_use]
pub fn candidates(workspace: &Path) -> Vec<Candidate> {
    attached(workspace)
        .into_iter()
        .map(|a| Candidate {
            kind: Source::Rationale,
            file: a.file.clone(),
            line: a.line,
            text: format!("in `{}`: {}", a.owner, a.text),
            evidence: Value::String(a.state_line()),
        })
        .collect()
}

/// What the mode adds to an implementation record: nothing for the
/// keyword gate, so its digests don't change.
#[must_use]
pub fn record(mode: CommentMode) -> Option<BTreeMap<&'static str, Value>> {
    (mode == CommentMode::LexiconFree).then(|| {
        BTreeMap::from([
            ("mode", Value::from(mode.word())),
            ("max", Value::from(MAX)),
            ("chars", Value::from(CHARS)),
        ])
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PY: &str = r#""""Module docstring: what this file does."""
import math

THRESHOLD = 0.5  # module level, attached to nothing


# Scores a pair.
@cache
def score(a, b):
    """Score two values."""
    # The raw difference, kept on purpose.
    # A second line of the same block.
    d = a - b  # signed
    return d


class Window:
    """A sliding window."""

    # Class-level note.
    size = 3

    def push(self, x):
        s = "not # a comment"
        return x


def bare():
    x = """not a docstring"""
    return x
"#;

    #[test]
    fn python_comments_attach_to_their_function_class_or_module() {
        let found = attached_in("m.py", PY);
        let got: Vec<(usize, &str, &str)> = found
            .iter()
            .map(|a| (a.line, a.owner.as_str(), a.text.as_str()))
            .collect();
        assert_eq!(
            got,
            vec![
                (1, "module", "Module docstring: what this file does."),
                (7, "score", "Scores a pair."),
                (10, "score", "Score two values."),
                (
                    11,
                    "score",
                    "The raw difference, kept on purpose. A second line of the same block."
                ),
                (13, "score", "signed"),
                (18, "Window", "A sliding window."),
                (20, "Window", "Class-level note."),
            ]
        );
    }

    const TS: &str = r#"// File header, attached to nothing.
import { x } from "y";

/**
 * Adds two numbers.
 */
export function add(a: number, b: number): number {
  // Rounds on purpose.
  const url = "http://example.com"; // trailing note
  return a + b;
}
"#;

    #[test]
    fn brace_comments_attach_to_their_function() {
        let found = attached_in("m.ts", TS);
        let got: Vec<(usize, &str, &str)> = found
            .iter()
            .map(|a| (a.line, a.owner.as_str(), a.text.as_str()))
            .collect();
        assert_eq!(
            got,
            vec![
                (4, "add", "Adds two numbers."),
                (8, "add", "Rounds on purpose."),
                (9, "add", "trailing note"),
            ]
        );
    }

    #[test]
    fn the_bound_takes_files_in_turn() {
        let file = |name: &str, n: usize| {
            (1..=n)
                .map(|line| Attached {
                    file: name.to_string(),
                    line,
                    owner: "f".to_string(),
                    text: "t".to_string(),
                })
                .collect::<Vec<_>>()
        };
        let kept = bounded(vec![file("a", 10), file("b", 2), file("c", 10)], 8);
        let count = |name: &str| kept.iter().filter(|a| a.file == name).count();
        assert_eq!((count("a"), count("b"), count("c")), (3, 2, 3));
        assert_eq!(kept.first().map(|a| a.file.as_str()), Some("a"));
        let all = bounded(vec![file("a", 2)], 8);
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn the_keyword_gate_is_the_default_and_leaves_records_unchanged() {
        assert_eq!(CommentMode::default(), CommentMode::Keywords);
        assert!(record(CommentMode::Keywords).is_none());
        let lexicon_free = record(CommentMode::LexiconFree).expect("a record");
        assert_eq!(lexicon_free["mode"], "lexicon-free");
        assert_eq!(
            serde_json::to_value(CommentMode::LexiconFree).unwrap(),
            "lexicon-free"
        );
    }

    #[test]
    fn a_candidate_carries_its_owner_in_the_state_line() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("m.py"), PY).unwrap();
        let found = candidates(dir.path());
        assert_eq!(found.len(), 7);
        assert_eq!(found[1].evidence, "m.py:7: in `score`: Scores a pair.");
        assert!(found.iter().all(|c| c.kind == Source::Rationale));
    }
}
