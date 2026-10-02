//! A test's workspace: the folder a files test starts from, and what the
//! run changed in it.
//!
//! A case with a `workspace` key is a *files test*: the run starts in a
//! scratch Git repository holding a named template and the case's own
//! `fixtures/`, Coder may write the folder and run commands in it (inside
//! the run's boundary), and checks read the files, the changes, and the
//! diff afterwards, or run a `command` check there
//! (`docs/extensions/evaluation.md`, *Files tests*). This module is the
//! pure part: the templates, and the changes and unified diff between two
//! snapshots of the folder.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::Serialize;

/// The template names a case may write, in the order the spec lists them.
pub const TEMPLATES: [&str; 4] = ["empty", "rust-crate", "python-package", "node-package"];
/// The longest diff text a run keeps; the rest is cut with a note.
pub const MAX_DIFF_BYTES: usize = 1 << 20;
/// A file larger than this is reported as changed without its lines.
const MAX_DIFF_FILE: usize = 256 * 1024;
/// The most line pairs the line diff compares for one file.
const MAX_DIFF_CELLS: usize = 4_000_000;
/// Lines of context around each hunk.
const CONTEXT: usize = 3;

/// The folder a files test starts from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Workspace {
    /// The template laid down before the case's `fixtures/`.
    pub template: String,
}

impl Workspace {
    /// The template's files, by relative path, in path order.
    #[must_use]
    pub fn files(&self) -> Vec<(String, Vec<u8>)> {
        template(&self.template)
            .unwrap_or_default()
            .into_iter()
            .map(|(path, text)| (path.to_string(), text.as_bytes().to_vec()))
            .collect()
    }
}

/// A template's files, or `None` for a name that isn't one.
#[must_use]
pub fn template(name: &str) -> Option<Vec<(&'static str, &'static str)>> {
    Some(match name {
        "empty" => Vec::new(),
        "rust-crate" => vec![
            (".gitignore", "/target\n"),
            (
                "Cargo.toml",
                "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n\n[workspace]\n",
            ),
            (
                "src/lib.rs",
                "/// Adds two numbers.\npub fn add(left: u64, right: u64) -> u64 {\n    left + right\n}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn it_adds() {\n        assert_eq!(add(2, 2), 4);\n    }\n}\n",
            ),
        ],
        "python-package" => vec![
            (".gitignore", "__pycache__/\n"),
            (
                "pyproject.toml",
                "[project]\nname = \"fixture\"\nversion = \"0.1.0\"\nrequires-python = \">=3.9\"\n",
            ),
            (
                "fixture/__init__.py",
                "def add(left, right):\n    \"\"\"Adds two numbers.\"\"\"\n    return left + right\n",
            ),
            (
                "tests/test_fixture.py",
                "import unittest\n\nfrom fixture import add\n\n\nclass AddTest(unittest.TestCase):\n    def test_adds(self):\n        self.assertEqual(add(2, 2), 4)\n\n\nif __name__ == \"__main__\":\n    unittest.main()\n",
            ),
        ],
        "node-package" => vec![
            (".gitignore", "node_modules/\n"),
            (
                "package.json",
                "{\n  \"name\": \"fixture\",\n  \"version\": \"0.1.0\",\n  \"private\": true,\n  \"scripts\": {\n    \"test\": \"node --test\"\n  }\n}\n",
            ),
            (
                "index.js",
                "/** Adds two numbers. */\nfunction add(left, right) {\n  return left + right;\n}\n\nmodule.exports = { add };\n",
            ),
            (
                "test/index.test.js",
                "const test = require(\"node:test\");\nconst assert = require(\"node:assert\");\nconst { add } = require(\"../index.js\");\n\ntest(\"adds\", () => {\n  assert.strictEqual(add(2, 2), 4);\n});\n",
            ),
        ],
        _ => return None,
    })
}

/// How a path changed between two snapshots.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// It is new.
    Added,
    /// Its bytes changed.
    Modified,
    /// It is gone.
    Deleted,
}

impl Status {
    /// The word a report writes.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Modified => "modified",
            Self::Deleted => "deleted",
        }
    }
}

/// One changed path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Change {
    /// The path, relative to the workspace, `/`-separated.
    pub path: String,
    /// How it changed.
    pub status: Status,
}

impl Change {
    /// `added src/lib.rs`, as the `changed` focus lists it.
    #[must_use]
    pub fn line(&self) -> String {
        format!("{} {}", self.status.word(), self.path)
    }
}

/// The paths that differ between `before` and `after`, in path order.
#[must_use]
pub fn changes(
    before: &BTreeMap<String, Vec<u8>>,
    after: &BTreeMap<String, Vec<u8>>,
) -> Vec<Change> {
    let mut out = Vec::new();
    for (path, bytes) in after {
        match before.get(path) {
            None => out.push(Change {
                path: path.clone(),
                status: Status::Added,
            }),
            Some(old) if old != bytes => out.push(Change {
                path: path.clone(),
                status: Status::Modified,
            }),
            Some(_) => {}
        }
    }
    for path in before.keys() {
        if !after.contains_key(path) {
            out.push(Change {
                path: path.clone(),
                status: Status::Deleted,
            });
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// A unified diff of every change, `git diff` style (`a/` and `b/`
/// prefixes, three lines of context), at most [`MAX_DIFF_BYTES`].
#[must_use]
pub fn unified(before: &BTreeMap<String, Vec<u8>>, after: &BTreeMap<String, Vec<u8>>) -> String {
    let mut out = String::new();
    for change in changes(before, after) {
        let old = before.get(&change.path).map(Vec::as_slice);
        let new = after.get(&change.path).map(Vec::as_slice);
        let (from, to) = match change.status {
            Status::Added => ("/dev/null".to_string(), format!("b/{}", change.path)),
            Status::Deleted => (format!("a/{}", change.path), "/dev/null".to_string()),
            Status::Modified => (format!("a/{}", change.path), format!("b/{}", change.path)),
        };
        let _ = writeln!(out, "diff --git a/{0} b/{0}", change.path);
        let (Some(old_text), Some(new_text)) = (text(old), text(new)) else {
            let _ = writeln!(out, "Binary files {from} and {to} differ");
            continue;
        };
        if old_text.len() > MAX_DIFF_FILE || new_text.len() > MAX_DIFF_FILE {
            let _ = writeln!(out, "Large files {from} and {to} differ");
            continue;
        }
        let _ = writeln!(out, "--- {from}\n+++ {to}");
        out.push_str(&hunks(&lines(old_text), &lines(new_text)));
        if out.len() > MAX_DIFF_BYTES {
            break;
        }
    }
    if out.len() > MAX_DIFF_BYTES {
        let mut cut = MAX_DIFF_BYTES;
        while !out.is_char_boundary(cut) {
            cut -= 1;
        }
        out.truncate(cut);
        out.push_str("\n[the diff is cut at 1 MiB]\n");
    }
    out
}

/// A side's text: empty for a missing side, `None` for bytes that aren't
/// UTF-8 text or hold a NUL.
fn text(bytes: Option<&[u8]>) -> Option<&str> {
    let bytes = bytes.unwrap_or_default();
    if bytes.contains(&0) {
        return None;
    }
    std::str::from_utf8(bytes).ok()
}

fn lines(text: &str) -> Vec<&str> {
    text.split_inclusive('\n').collect()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Op {
    Same,
    Del,
    Add,
}

/// The edit script from `old` to `new` by longest common subsequence,
/// after trimming the common head and tail.
fn script(old: &[&str], new: &[&str]) -> Vec<(Op, usize, usize)> {
    let head = old.iter().zip(new).take_while(|(a, b)| a == b).count();
    let tail = old[head..]
        .iter()
        .rev()
        .zip(new[head..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let (a, b) = (&old[head..old.len() - tail], &new[head..new.len() - tail]);
    let mut ops: Vec<(Op, usize, usize)> = (0..head).map(|i| (Op::Same, i, i)).collect();
    if a.len().saturating_mul(b.len()) > MAX_DIFF_CELLS {
        ops.extend((0..a.len()).map(|i| (Op::Del, head + i, head)));
        ops.extend((0..b.len()).map(|j| (Op::Add, head + a.len(), head + j)));
    } else {
        // table[i][j]: the LCS length of a[i..] and b[j..].
        let width = b.len() + 1;
        let mut table = vec![0u32; (a.len() + 1) * width];
        for i in (0..a.len()).rev() {
            for j in (0..b.len()).rev() {
                table[i * width + j] = if a[i] == b[j] {
                    table[(i + 1) * width + j + 1] + 1
                } else {
                    table[(i + 1) * width + j].max(table[i * width + j + 1])
                };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < a.len() || j < b.len() {
            if i < a.len() && j < b.len() && a[i] == b[j] {
                ops.push((Op::Same, head + i, head + j));
                i += 1;
                j += 1;
            } else if i < a.len()
                && (j == b.len() || table[(i + 1) * width + j] >= table[i * width + j + 1])
            {
                // Deletions before additions, as `git diff` writes them.
                ops.push((Op::Del, head + i, head + j));
                i += 1;
            } else {
                ops.push((Op::Add, head + i, head + j));
                j += 1;
            }
        }
    }
    let (old_end, new_end) = (old.len() - tail, new.len() - tail);
    ops.extend((0..tail).map(|k| (Op::Same, old_end + k, new_end + k)));
    ops
}

fn hunks(old: &[&str], new: &[&str]) -> String {
    let ops = script(old, new);
    let changed: Vec<usize> = ops
        .iter()
        .enumerate()
        .filter(|(_, (op, _, _))| *op != Op::Same)
        .map(|(index, _)| index)
        .collect();
    let mut out = String::new();
    let mut at = 0;
    while at < changed.len() {
        let start = changed[at].saturating_sub(CONTEXT);
        let mut end = changed[at];
        while at < changed.len() && changed[at] <= end + 2 * CONTEXT {
            end = changed[at];
            at += 1;
        }
        let end = (end + CONTEXT + 1).min(ops.len());
        let slice = &ops[start..end];
        let old_count = slice.iter().filter(|(op, _, _)| *op != Op::Add).count();
        let new_count = slice.iter().filter(|(op, _, _)| *op != Op::Del).count();
        let (_, old_start, new_start) = slice[0];
        let first = |start: usize, count: usize| if count == 0 { start } else { start + 1 };
        let _ = writeln!(
            out,
            "@@ -{},{old_count} +{},{new_count} @@",
            first(old_start, old_count),
            first(new_start, new_count)
        );
        for (op, i, j) in slice {
            let (mark, line) = match op {
                Op::Same => (' ', old[*i]),
                Op::Del => ('-', old[*i]),
                Op::Add => ('+', new[*j]),
            };
            out.push(mark);
            out.push_str(line);
            if !line.ends_with('\n') {
                out.push_str("\n\\ No newline at end of file\n");
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(files: &[(&str, &str)]) -> BTreeMap<String, Vec<u8>> {
        files
            .iter()
            .map(|(p, t)| ((*p).to_string(), t.as_bytes().to_vec()))
            .collect()
    }

    #[test]
    fn every_template_name_has_files_and_unknown_names_none() {
        for name in TEMPLATES {
            assert!(template(name).is_some(), "{name}");
        }
        assert!(template("rust-crate").unwrap().len() >= 2);
        assert!(template("cobol").is_none());
    }

    #[test]
    fn changes_name_added_modified_and_deleted_paths() {
        let before = snap(&[("a", "1\n"), ("b", "2\n"), ("c", "3\n")]);
        let after = snap(&[("a", "1\n"), ("b", "two\n"), ("d", "4\n")]);
        let lines: Vec<String> = changes(&before, &after).iter().map(Change::line).collect();
        assert_eq!(lines, ["modified b", "deleted c", "added d"]);
    }

    #[test]
    fn the_diff_is_unified_with_context() {
        let before = snap(&[(
            "src/lib.rs",
            "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\n",
        )]);
        let after = snap(&[
            (
                "src/lib.rs",
                "one\ntwo\nthree\nFOUR\nfive\nsix\nseven\neight\nnine\n",
            ),
            ("NEW.md", "hello"),
        ]);
        let diff = unified(&before, &after);
        assert!(diff.contains("--- /dev/null\n+++ b/NEW.md\n@@ -0,0 +1,1 @@\n+hello\n\\ No newline at end of file\n"), "{diff}");
        assert!(
            diff.contains("--- a/src/lib.rs\n+++ b/src/lib.rs\n"),
            "{diff}"
        );
        assert!(
            diff.contains("@@ -1,8 +1,9 @@\n one\n two\n three\n-four\n+FOUR\n five\n"),
            "{diff}"
        );
        assert!(diff.contains("+nine\n"), "{diff}");
        assert!(unified(&before, &before).is_empty());
    }

    #[test]
    fn binary_files_are_named_not_shown() {
        let before = BTreeMap::new();
        let after = BTreeMap::from([("blob".to_string(), vec![0u8, 1, 2])]);
        assert!(unified(&before, &after).contains("Binary files /dev/null and b/blob differ"));
    }
}
