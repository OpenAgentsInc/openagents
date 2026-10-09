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
