//! No machine talk in the terminal's copy (#11031): every string literal in
//! this crate that reads like words is checked against the `oa-copy`
//! lexicon.

use std::path::Path;

/// Files with no user-facing copy.
const SKIP: &[&str] = &[];

/// Lexicon terms a file may show, each with its reason.
const ALLOW_IN: &[(&str, &[&str])] = &[
    // Cursor is the name of a coding agent the terminal can run.
    ("acp_discovery.rs", &["cursor"]),
    // The deploy tool's answer and its production step name the web image
    // by its sha256 digest: the exact value the owner approves.
    ("ops_tool.rs", &["digest"]),
];

#[test]
fn terminal_copy_has_no_machine_talk() {
    let hits = oa_copy::scan_dir_allowing(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        SKIP,
        &[],
        ALLOW_IN,
    );
    assert!(
        hits.is_empty(),
        "machine talk in user-facing copy (rewrite it in plain words, see AGENTS.md):\n{}",
        hits.join("\n")
    );
}

/// Phrases from the program's own plumbing that error messages here used to
/// show: a process group's flag, an operating system's error code, a
/// boolean printed as a word (#11091).
const PLUMBING: &[&str] = &[
    "process group",
    "group cleared",
    "os error",
    "cleanup confirmed",
    ": true",
    ": false",
];

/// Plumbing a file names without showing it, each with its reason.
const PLUMBING_ALLOWED: &[(&str, &str)] = &[
    // `main.rs` finds the operating system's code suffix to remove it.
    ("main.rs", "os error"),
];

#[test]
fn terminal_errors_show_no_plumbing() {
    let mut hits = Vec::new();
    let mut stack = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let Some(name) = path.file_name().map(|name| name.to_string_lossy()) else {
                continue;
            };
            if !name.ends_with(".rs") || name.ends_with("tests.rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path).unwrap();
            for string in oa_copy::source_strings(&source) {
                if !oa_copy::looks_like_copy(&string.text) {
                    continue;
                }
                let lower = string.text.to_lowercase();
                for phrase in PLUMBING {
                    let allowed = PLUMBING_ALLOWED
                        .iter()
                        .any(|(file, allowed)| name == *file && allowed == phrase);
                    if !allowed && lower.contains(phrase) {
                        hits.push(format!("{}:{}: {phrase}", path.display(), string.line));
                    }
                }
            }
        }
    }
    assert!(
        hits.is_empty(),
        "plumbing in user-facing copy (say what happened in plain words):\n{}",
        hits.join("\n")
    );
}
