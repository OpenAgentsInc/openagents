//! Helpers the test binaries share: the task, the golden's text, and an
//! authored trace in the shape a sentence-driven run is expected to
//! record.
//!
//! Authored variants derive from the observed golden for regression tests.
//! They can change prompts or correctness claims and supply synthetic driver
//! facts. They test the grader; they are not additional observed episodes.

use coderbench::{Observed, Task, goldens_dir, observe, tasks_dir};
use serde_json::{Value, json};

/// The task these tests grade against.
#[allow(dead_code)]
pub fn task() -> Task {
    Task::load(&tasks_dir().join("devin-fan-out-six").join("task.json"))
        .expect("task manifest loads")
}

/// The observed golden, as text for authoring independent test variants.
#[allow(dead_code)]
pub fn golden_text() -> String {
    std::fs::read_to_string(goldens_dir().join("devin-fan-out-six.atif.jsonl"))
        .expect("the golden reads")
}

/// An authored trace with the manifest's prompts and no self-asserted
/// correctness. The manifest's expectations are the only answer check.
/// Rewriting this in memory does not change the retained recording.
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
        .map(|line| line + "\n")
        .collect()
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
