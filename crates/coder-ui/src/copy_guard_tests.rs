//! No machine talk in Coder's views (#11031): every string literal in this
//! crate that reads like words is checked against the `oa-copy` lexicon.

use std::path::Path;

/// Files with no user-facing copy.
const SKIP: &[&str] = &[];

/// Lexicon terms this crate may show, each with its reason.
const ALLOW: &[&str] = &[
    // The text cursor in the composer (and the Cursor coding agent), not a
    // paging cursor.
    "cursor",
];

/// Lexicon terms a file may show, each with its reason.
const ALLOW_IN: &[(&str, &[&str])] = &[
    // The demo must draw exactly the pinned original frames
    // (`crates/coder-new/tests/demo_parity.rs`), whose one tool row says
    // "scroll retained".
    ("demo/agents.rs", &["retained"]),
];

#[test]
fn coder_view_copy_has_no_machine_talk() {
    let hits = oa_copy::scan_dir_allowing(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        SKIP,
        ALLOW,
        ALLOW_IN,
    );
    assert!(
        hits.is_empty(),
        "machine talk in user-facing copy (rewrite it in plain words, see AGENTS.md):\n{}",
        hits.join("\n")
    );
}
