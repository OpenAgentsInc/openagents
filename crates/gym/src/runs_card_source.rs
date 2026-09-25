//! Source text for the run card: the untouched files a session was shown,
//! line diffs between two versions of a file, and which changed lines are
//! code rather than comments or docstrings.
//!
//! Everything here reads text the trial directory already holds. The
//! untouched source comes from the first session's briefing, which carries
//! each source file in full under `## The current PATH`, or from a full
//! `read_file` of a file before the session first edited it. Nothing is
//! read from the task's checkout, so a file the session never saw in full
//! has no untouched text, and every number that needs it is `unknown`.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::runs_analysis::Session;

/// The most lines either side of a diff may have. Longer files are not
/// diffed, and what depends on the diff is `unknown`.
pub const DIFF_LINES: usize = 2_500;

/// A path as the card names it: relative to the session's repository, with
/// no leading `./`.
#[must_use]
pub fn relative(path: &str, repository: &str) -> String {
    let path = path.trim();
    let repository = repository.trim_end_matches('/');
    let path = if !repository.is_empty() {
        path.strip_prefix(repository)
            .map(|rest| rest.trim_start_matches('/'))
            .unwrap_or(path)
    } else {
        path
    };
    path.trim_start_matches("./").to_owned()
}

/// The files the briefing carried in full, under `## The current PATH`.
#[must_use]
pub fn briefing_files(brief: &str) -> BTreeMap<String, String> {
    const HEAD: &str = "## The current ";
    let mut files = BTreeMap::new();
    let mut rest = brief;
    while let Some(at) = rest.find(HEAD) {
        let at_line_start = at == 0 || rest[..at].ends_with('\n');
        let after = &rest[at + HEAD.len()..];
        if !at_line_start {
            rest = after;
            continue;
        }
        let Some(newline) = after.find('\n') else {
            break;
        };
        let path = after[..newline].trim().to_owned();
        let body = after[newline + 1..]
            .strip_prefix('\n')
            .unwrap_or(&after[newline + 1..]);
        // The next section starts at a blank line and a `## ` heading; the
        // file's own last newline stays with the file.
        let end = body.find("\n\n## ").map_or(body.len(), |end| end + 1);
        if !path.is_empty() {
            files.insert(path, body[..end].to_owned());
        }
        rest = &body[end..];
    }
    files
}

/// A `read_file` output's text, without the line numbers, when the output
/// shows the whole file.
fn full_read(call: &crate::runs_analysis::Call) -> Option<String> {
    if call.name != "read_file" || !call.ok {
        return None;
    }
    let start = call
        .arguments
        .get("start_line")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(1);
    if start != 1 {
        return None;
    }
    // Microluna prints a numbered line per source line; it says how many
    // lines the file has and how many it showed only in the record's extra,
    // which the analysis reader drops, so a read counts as whole when its
    // last numbered line is not followed by a truncation note.
    let mut lines = Vec::new();
    for line in call.output.lines() {
        let trimmed = line.trim_start();
        let (number, text) = trimmed.split_once('\t')?;
        let number: usize = number.trim().parse().ok()?;
        if number != lines.len() + 1 {
            return None;
        }
        lines.push(text.to_owned());
    }
    (!lines.is_empty()).then(|| {
        let mut text = lines.join("\n");
        text.push('\n');
        text
    })
}

/// The untouched text of every file the first session saw in full, by
/// path relative to its repository, with where each came from.
#[must_use]
pub fn untouched(sessions: &[Session]) -> BTreeMap<String, (String, &'static str)> {
    let mut found: BTreeMap<String, (String, &'static str)> = BTreeMap::new();
    let Some(first) = sessions.first() else {
        return found;
    };
    for (path, text) in briefing_files(&first.brief) {
        found.insert(relative(&path, &first.repository), (text, "briefing"));
    }
    let mut edited: BTreeSet<String> = BTreeSet::new();
    for call in &first.calls {
        for path in edited_paths(call) {
            edited.insert(relative(&path, &first.repository));
        }
        if let Some(text) = full_read(call) {
            let path = call
                .arguments
                .get("path")
                .and_then(serde_json::Value::as_str)
                .map(|path| relative(path, &first.repository))
                .unwrap_or_default();
            if !path.is_empty() && !edited.contains(&path) && !found.contains_key(&path) {
                found.insert(path, (text, "read before the first edit"));
            }
        }
    }
    found
}

/// The paths an `apply_patch` or `write_file` call changed.
#[must_use]
pub fn edited_paths(call: &crate::runs_analysis::Call) -> Vec<String> {
    if !call.ok {
        return Vec::new();
    }
    match call.name.as_str() {
        "apply_patch" => {
            let patch = call
                .arguments
                .get("patch")
                .or_else(|| call.arguments.get("input"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            patch
                .lines()
                .filter_map(|line| {
                    ["*** Update File: ", "*** Add File: ", "*** Delete File: "]
                        .iter()
                        .find_map(|head| line.strip_prefix(head))
                })
                .map(|path| path.trim().to_owned())
                .collect()
        }
        "write_file" | "edit_file" | "create_file" => call
            .arguments
            .get("path")
            .and_then(serde_json::Value::as_str)
            .map(|path| vec![path.to_owned()])
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// Which lines a diff changed, 1-based, on each side.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Changed {
    /// Lines of the old text that the new text removed or replaced.
    pub old: BTreeSet<usize>,
    /// Lines of the new text that the old text didn't have.
    pub new: BTreeSet<usize>,
}

impl Changed {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.old.is_empty() && self.new.is_empty()
    }
}

/// A line diff by longest common subsequence, or `None` when either side is
/// longer than [`DIFF_LINES`] after the common ends are set aside.
#[must_use]
pub fn changed(old: &str, new: &str) -> Option<Changed> {
    let a: Vec<&str> = old.lines().collect();
    let b: Vec<&str> = new.lines().collect();
    let prefix = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let suffix = a[prefix..]
        .iter()
        .rev()
        .zip(b[prefix..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let a_mid = &a[prefix..a.len() - suffix];
    let b_mid = &b[prefix..b.len() - suffix];
    if a_mid.len() > DIFF_LINES || b_mid.len() > DIFF_LINES {
        return None;
    }
    let (n, m) = (a_mid.len(), b_mid.len());
    let mut table = vec![0u16; (n + 1) * (m + 1)];
    let at = |i: usize, j: usize| i * (m + 1) + j;
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            table[at(i, j)] = if a_mid[i] == b_mid[j] {
                table[at(i + 1, j + 1)] + 1
            } else {
                table[at(i + 1, j)].max(table[at(i, j + 1)])
            };
        }
    }
    let mut out = Changed::default();
    let (mut i, mut j) = (0, 0);
    while i < n || j < m {
        if i < n && j < m && a_mid[i] == b_mid[j] {
            i += 1;
            j += 1;
        } else if j < m && (i == n || table[at(i, j + 1)] >= table[at(i + 1, j)]) {
            out.new.insert(prefix + j + 1);
            j += 1;
        } else {
            out.old.insert(prefix + i + 1);
            i += 1;
        }
    }
    Some(out)
}

/// What a source line is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Code,
    /// Part of a docstring: a string statement of its own, with the line
    /// its block starts on.
    Doc(usize),
    Comment,
    Blank,
}

/// Each line's kind. Python files are scanned for comments and docstrings;
/// in any other file every nonblank line is code.
#[must_use]
pub fn kinds(text: &str, path: &str) -> Vec<Kind> {
    let python = path.ends_with(".py");
    let mut out = Vec::new();
    // An open triple-quoted string: its delimiter, and the line its block
    // starts on when it's a docstring.
    let mut open: Option<(&'static str, Option<usize>)> = None;
    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        let trimmed = line.trim();
        if let Some((delimiter, doc)) = open {
            out.push(doc.map_or(Kind::Code, Kind::Doc));
            if trimmed.matches(delimiter).count() % 2 == 1 {
                open = None;
            }
            continue;
        }
        if trimmed.is_empty() {
            out.push(Kind::Blank);
            continue;
        }
        if !python {
            out.push(Kind::Code);
            continue;
        }
        if trimmed.starts_with('#') {
            out.push(Kind::Comment);
            continue;
        }
        let body = trimmed.trim_start_matches(['r', 'R', 'u', 'U', 'b', 'B', 'f', 'F']);
        let delimiter = ["\"\"\"", "'''"]
            .into_iter()
            .find(|delimiter| body.starts_with(delimiter));
        if let Some(delimiter) = delimiter {
            out.push(Kind::Doc(number));
            if body.matches(delimiter).count() % 2 == 1 {
                open = Some((delimiter, Some(number)));
            }
            continue;
        }
        out.push(Kind::Code);
        for delimiter in ["\"\"\"", "'''"] {
            if trimmed.matches(delimiter).count() % 2 == 1 {
                open = Some((delimiter, None));
                break;
            }
        }
    }
    out
}

/// What a change between two versions of one file touched.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Change {
    pub path: String,
    /// Changed lines that are code, on either side.
    pub code_lines: usize,
    pub docstring_lines: usize,
    pub comment_lines: usize,
    pub blank_lines: usize,
    /// Distinct docstrings the change touched, counted on the new side, or
    /// the old side for a docstring the change removed.
    pub docstrings: usize,
}

impl Change {
    /// Whether the change touched no code.
    #[must_use]
    pub fn behavior_unchanged(&self) -> bool {
        self.code_lines == 0
    }

    /// The change in words: `one docstring, no code`, `12 code lines`.
    #[must_use]
    pub fn words(&self) -> String {
        if self.code_lines > 0 {
            let mut text = format!(
                "{} code line{}",
                self.code_lines,
                if self.code_lines == 1 { "" } else { "s" }
            );
            if self.docstrings > 0 {
                text.push_str(&format!(
                    ", {} docstring{}",
                    self.docstrings,
                    if self.docstrings == 1 { "" } else { "s" }
                ));
            }
            return text;
        }
        let mut parts = Vec::new();
        if self.docstrings > 0 {
            parts.push(format!(
                "{} docstring{}",
                self.docstrings,
                if self.docstrings == 1 { "" } else { "s" }
            ));
        }
        if self.comment_lines > 0 {
            parts.push(format!(
                "{} comment line{}",
                self.comment_lines,
                if self.comment_lines == 1 { "" } else { "s" }
            ));
        }
        if parts.is_empty() {
            parts.push("whitespace".to_owned());
        }
        format!("{}, no code", parts.join(", "))
    }
}

/// Classifies the change from `old` to `new`, or `None` when the diff is
/// too large.
#[must_use]
pub fn classify(path: &str, old: &str, new: &str) -> Option<Change> {
    let diff = changed(old, new)?;
    let old_kinds = kinds(old, path);
    let new_kinds = kinds(new, path);
    let mut change = Change {
        path: path.to_owned(),
        ..Change::default()
    };
    let mut new_blocks = BTreeSet::new();
    let mut old_blocks = BTreeSet::new();
    let mut count = |kind: Option<&Kind>, blocks: &mut BTreeSet<usize>| match kind {
        Some(Kind::Code) | None => change.code_lines += 1,
        Some(Kind::Doc(start)) => {
            change.docstring_lines += 1;
            blocks.insert(*start);
        }
        Some(Kind::Comment) => change.comment_lines += 1,
        Some(Kind::Blank) => change.blank_lines += 1,
    };
    for line in &diff.new {
        count(new_kinds.get(line - 1), &mut new_blocks);
    }
    for line in &diff.old {
        count(old_kinds.get(line - 1), &mut old_blocks);
    }
    change.docstrings = new_blocks.len().max(old_blocks.len());
    Some(change)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_briefing_carries_each_file_until_the_next_heading() {
        let brief = "# Task\n\n## The current a.py\n\nx = 1\n\ny = 2\n\n## The current b.py\n\nz = 3\n\n## Likely defects\n\n- a.py:1\n";
        let files = briefing_files(brief);
        assert_eq!(files["a.py"], "x = 1\n\ny = 2\n");
        assert_eq!(files["b.py"], "z = 3\n");
    }

    #[test]
    fn a_diff_marks_replaced_and_inserted_lines_on_each_side() {
        let changed = changed("a\nb\nc\nd\n", "a\nB\nc\nd\ne\n").unwrap();
        assert_eq!(changed.old, BTreeSet::from([2]));
        assert_eq!(changed.new, BTreeSet::from([2, 5]));
        assert!(super::changed("same\n", "same\n").unwrap().is_empty());
    }

    #[test]
    fn a_docstring_only_change_is_one_docstring_and_no_code() {
        let old = "\"\"\"Module.\n\nAssumes unit vectors.\n\"\"\"\nimport numpy\n\n\ndef f(x):\n    \"\"\"One line.\"\"\"\n    return x  # keep\n";
        let new = "\"\"\"Module.\n\nNormalizes by the norms.\n\"\"\"\nimport numpy\n\n\ndef f(x):\n    \"\"\"One line.\"\"\"\n    return x  # keep\n";
        let change = classify("m.py", old, new).unwrap();
        assert_eq!(change.code_lines, 0);
        assert_eq!(change.docstrings, 1);
        assert!(change.behavior_unchanged());
        assert_eq!(change.words(), "1 docstring, no code");
        let code = classify("m.py", old, &old.replace("return x", "return -x")).unwrap();
        assert_eq!(code.code_lines, 2);
        assert!(!code.behavior_unchanged());
    }

    #[test]
    fn kinds_follow_docstrings_comments_and_code_strings() {
        let text = "# c\n\"\"\"doc\nmore\n\"\"\"\nx = \"\"\"not a\ndoc\"\"\"\n\ny = 1\n";
        assert_eq!(
            kinds(text, "a.py"),
            vec![
                Kind::Comment,
                Kind::Doc(2),
                Kind::Doc(2),
                Kind::Doc(2),
                Kind::Code,
                Kind::Code,
                Kind::Blank,
                Kind::Code,
            ]
        );
        assert_eq!(kinds("# c\n", "a.sh"), vec![Kind::Code]);
    }

    #[test]
    fn paths_are_relative_to_the_repository() {
        assert_eq!(relative("/app/drift/a.py", "/app"), "drift/a.py");
        assert_eq!(relative("./drift/a.py", "/app"), "drift/a.py");
        assert_eq!(relative("drift/a.py", ""), "drift/a.py");
    }
}
