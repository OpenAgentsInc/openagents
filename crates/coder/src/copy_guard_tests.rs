//! No machine talk in what a person reads from `coder` (#11031): the help
//! and messages of its command line. The answer bank and the product
//! answers are guarded by their own lints (`router::bank::lint`,
//! `product_kb` tests). The rest of this crate is engine code whose errors
//! go to logs and developers, where precise terms belong.

use std::path::Path;

/// The files whose strings a person reads, each with the lexicon terms it
/// may show and why.
const FILES: &[(&str, &[&str])] = &[
    ("cli.rs", &[]),
    // `--cursor` is the real flag that pages through a task's steps.
    ("task/cli.rs", &["cursor"]),
];

#[test]
fn command_line_copy_has_no_machine_talk() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut hits = Vec::new();
    for (file, allow) in FILES {
        let text = std::fs::read_to_string(src.join(file)).expect("the source file");
        for (line, v) in oa_copy::source_violations(&text, allow) {
            hits.push(format!("{file}:{line}: {:?} in {:?}", v.term, v.context));
        }
    }
    assert!(
        hits.is_empty(),
        "machine talk in user-facing copy (rewrite it in plain words, see AGENTS.md):\n{}",
        hits.join("\n")
    );
}
