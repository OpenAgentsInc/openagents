//! No machine talk in the chat's copy (#11031): every string literal in this
//! crate that reads like words is checked against the `oa-copy` lexicon.

use std::path::Path;

/// Files with no user-facing copy.
const SKIP: &[&str] = &[];

/// Lexicon terms this crate may show, each with its reason.
const ALLOW: &[&str] = &[];

#[test]
fn chat_copy_has_no_machine_talk() {
    let hits = oa_copy::scan_dir(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        SKIP,
        ALLOW,
    );
    assert!(
        hits.is_empty(),
        "machine talk in user-facing copy (rewrite it in plain words, see AGENTS.md):\n{}",
        hits.join("\n")
    );
}
