//! The web chat goldens, offline (`bench/web-chat/goldens-v1.json`,
//! `docs/web/chat-goldens.md`): every prepared answer and product note a
//! golden accepts exists, shows on the website, and says what the golden
//! requires and nothing it forbids. The live runs are the `chat-goldens`
//! binary's `http` and `router` modes.

use coder::chat_goldens::{self, Set};
use coder::generate::DEFAULT_DOOR_URL;
use coder::router;

#[test]
fn every_accepted_answer_meets_its_golden() {
    let set = Set::fixture();
    let root = knowledge::product::repository();
    let corpus = knowledge::product::Corpus::load(&knowledge::product::default_dir(), Some(&root))
        .expect("the product corpus loads");
    let notes = chat_goldens::note_answers(&corpus);
    let facts = chat_goldens::web_facts(&router::worker_facts(
        "google/gemini-3.8-flash",
        Some(DEFAULT_DOOR_URL),
        &router::Seams::default(),
    ));
    let problems = chat_goldens::check(&set, router::Bank::builtin(), &facts, &notes);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// The file on disk is the one compiled in, so the binary's default and
/// the test read the same expectations.
#[test]
fn the_checked_in_file_is_the_compiled_in_set() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(chat_goldens::PATH);
    let disk = std::fs::read_to_string(path).expect("the goldens file");
    assert_eq!(disk, chat_goldens::FIXTURE);
}
