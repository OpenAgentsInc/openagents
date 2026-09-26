//! Checks that keep entries general: none names a benchmark task, none
//! shares a long string with a task's tests, and every one cites a source.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::{Entry, Problem};

/// Characters two texts must share, after whitespace is collapsed, to count
/// as a quote.
pub const SHARED: usize = 40;

/// Test files larger than this are data, not test code, and are skipped.
pub const MAX_TEST_BYTES: u64 = 2_000_000;

/// Characters a summary may have, at most.
pub const SUMMARY_CHARS: usize = 600;

/// Where Terminal-Bench 4's tasks are installed.
#[must_use]
pub fn default_corpora() -> Vec<PathBuf> {
    std::env::var_os("HOME")
        .map(|home| {
            vec![
                PathBuf::from(home)
                    .join(".openagents/terminal-bench/upstream/terminal-bench-v4.0.0/tasks"),
            ]
        })
        .unwrap_or_default()
}

/// The benchmark tasks an entry must not name or quote.
#[derive(Clone, Debug, Default)]
pub struct Corpus {
    /// Task directory names.
    pub names: Vec<String>,
    /// Each task's test files: the task's name and the file's path.
    pub tests: Vec<(String, PathBuf)>,
    /// Corpus directories that weren't there.
    pub absent: Vec<PathBuf>,
}

impl Corpus {
    /// Reads the task directories under each of `dirs`. A directory that
    /// isn't there is noted and skipped.
    #[must_use]
    pub fn read(dirs: &[PathBuf]) -> Self {
        let mut corpus = Corpus::default();
        for dir in dirs {
            let Ok(listing) = std::fs::read_dir(dir) else {
                corpus.absent.push(dir.clone());
                continue;
            };
            let mut tasks: Vec<PathBuf> = listing
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.is_dir())
                .collect();
            tasks.sort();
            for task in tasks {
                let name = task
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                let mut files = Vec::new();
                walk(&task.join("tests"), &mut files);
                corpus
                    .tests
                    .extend(files.into_iter().map(|path| (name.clone(), path)));
                corpus.names.push(name);
            }
        }
        corpus
    }
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(listing) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = listing.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            walk(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// Every word of an entry the model can see, for the checks. Provenance
/// is left out: it names the runs an entry came from, and the prompt never
/// shows it.
fn full_text(entry: &Entry) -> String {
    [
        entry.id.as_str(),
        &entry.title,
        &entry.summary,
        &entry.tags.join(" "),
        &entry.applies_when,
        &entry.cites.join(" "),
        &entry.body,
    ]
    .join("\n")
}

/// `text` with every run of whitespace turned into one space.
fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Whether `name` appears in `text` as a whole name: not inside a longer
/// word or hyphenated name.
fn names(text: &str, name: &str) -> bool {
    let lower = text.to_lowercase();
    let name = name.to_lowercase();
    let part = |c: char| c.is_alphanumeric() || c == '-' || c == '_';
    lower.match_indices(&name).any(|(at, _)| {
        let before = lower[..at].chars().next_back();
        let after = lower[at + name.len()..].chars().next();
        !before.is_some_and(part) && !after.is_some_and(part)
    })
}

/// Every window of [`SHARED`] characters in `text`, by byte range.
fn windows(text: &str) -> impl Iterator<Item = &str> {
    let starts: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
    let count = starts.len();
    (0..count.saturating_sub(SHARED - 1)).map(move |i| {
        let end = starts.get(i + SHARED).copied().unwrap_or(text.len());
        &text[starts[i]..end]
    })
}

/// The problems with `entries` against `corpus`.
#[must_use]
pub fn lint(entries: &[Entry], corpus: &Corpus) -> Vec<Problem> {
    let mut problems = Vec::new();
    let texts: Vec<String> = entries.iter().map(|e| collapse(&full_text(e))).collect();
    for (entry, text) in entries.iter().zip(&texts) {
        let mut say = |message: String| {
            problems.push(Problem {
                at: entry.id.clone(),
                message,
            });
        };
        if entry.cites.is_empty() {
            say("it cites no source; add one under provenance.cites".to_string());
        }
        if text.contains(crate::write::PLACEHOLDER) {
            say(format!(
                "it still has template text marked {}; replace it",
                crate::write::PLACEHOLDER.trim_end_matches(':')
            ));
        }
        let length = entry.summary.chars().count();
        if length > SUMMARY_CHARS {
            say(format!(
                "its summary has {length} characters; keep it under {SUMMARY_CHARS}"
            ));
        }
        for name in &corpus.names {
            if names(text, name) {
                say(format!("it names the benchmark task {name}"));
            }
        }
    }
    // Every window of every entry, then one pass over the tests.
    let mut index: HashMap<&str, usize> = HashMap::new();
    for (i, text) in texts.iter().enumerate() {
        for window in windows(text) {
            index.entry(window).or_insert(i);
        }
    }
    let mut found: Vec<(usize, String)> = Vec::new();
    for (task, path) in &corpus.tests {
        if std::fs::metadata(path).map_or(true, |m| m.len() > MAX_TEST_BYTES) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let text = collapse(&text);
        for window in windows(&text) {
            if let Some(&i) = index.get(window)
                && !found.iter().any(|(j, t)| *j == i && t == task)
            {
                found.push((i, task.clone()));
                problems.push(Problem {
                    at: entries[i].id.clone(),
                    message: format!("it shares \"{window}\" with {}; reword it", path.display()),
                });
            }
        }
    }
    problems
}
