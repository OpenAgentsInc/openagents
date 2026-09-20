//! Helpers the test binaries share: the task, the golden's text, and an
//! authored trace in the shape a sentence-driven run is expected to
//! record.
//!
//! The authored trace is a fixture, not a recording: it is the staged
//! golden with each delegation call rewritten to carry the request's own
//! list items as prompts and no self-asserted correctness, so the
//! manifest's `expects` are the only check a judge can apply. A genuine
//! recording of Coder running the task's sentence replaces it after
//! openagents#9427 lands and the golden is re-recorded as observed.

use coderbench::{Observed, Task, goldens_dir, observe, tasks_dir};
use serde_json::{Value, json};

/// The task these tests grade against.
#[allow(dead_code)]
pub fn task() -> Task {
    Task::load(&tasks_dir().join("devin-fan-out-six").join("task.json"))
        .expect("task manifest loads")
}

/// The staged golden, as text for rewriting one thing at a time.
#[allow(dead_code)]
pub fn golden_text() -> String {
    std::fs::read_to_string(goldens_dir().join("devin-fan-out-six.atif.jsonl"))
        .expect("the golden reads")
}

/// An authored trace holding the calls a sentence-driven run is expected
/// to make: the staged golden with each delegation call rewritten to the
/// request's own list item as its prompt, and the call asserting no
/// correctness of its own — the manifest's `expects` are the only check.
///
/// The staged golden predates the request carrying the questions: its
/// prompts are the staging script's wording rather than the request's
/// list items, and its `expected` and `correct` fields are that script's
/// own claims. Neither is evidence the grade reads. This fixture is what
/// the manifest's answers are tested against until a real recording
/// exists — it proves the grader, not the run.
#[allow(dead_code)]
pub fn authored_text() -> String {
    let task = task();
    let mut expects = task.grade.expects.iter();
    golden_text()
        .lines()
        .map(|line| {
            let Ok(mut record) = serde_json::from_str::<Value>(line) else {
                return line.to_string();
            };
            let Some(call) = record.pointer_mut("/step/call") else {
                return line.to_string();
            };
            if call.get("name").and_then(Value::as_str) != Some(coderbench::DELEGATE_CALL) {
                return line.to_string();
            }
            if let Some(want) = expects.next() {
                call["arguments"]["prompt"] = json!(want.prompt);
            }
            if let Some(extra) = call.get_mut("extra").and_then(Value::as_object_mut) {
                extra.remove("expected");
                extra.remove("correct");
                extra.remove("reads");
            }
            serde_json::to_string(&record).unwrap_or_else(|_| line.to_string())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The authored trace observed the way `run` judges one, with the two
/// facts only a driver sees supplied: the turn answered, and the checkout
/// was read before and after and changed nothing.
#[allow(dead_code)]
pub fn authored_run() -> Observed {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("authored.atif.jsonl");
    std::fs::write(&path, authored_text()).unwrap();
    let mut run = observe(&path).expect("the authored trace still reads");
    run.ending = coderbench::Ending::Answered;
    run.workspace = Some(coderbench::Workspace::default());
    run
}
