//! The crash-point matrix for durable runstate.
//!
//! A run's record lives outside the process, so a crash is an event the
//! record survives, not an end the record guesses at. These tests kill
//! the store at each point the acceptance names — before dispatch,
//! during execution, after a result but before settlement, and during
//! cleanup — and check what recovery finds: unfinished work marks
//! `unknown`, a written end keeps its mark, and a second claim of the
//! same run id refuses across store instances, which is the
//! cross-process ownership the file itself provides.

use std::path::PathBuf;

use coder::runstate::{Claim, Mark, Outcome, Refusal, State, Store};

fn dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "runstate-recovery-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn claim<'a>(run: &'a str, questions: &'a [String], sources: &'a [String]) -> Claim<'a> {
    Claim {
        run,
        base: "base-abc123",
        program: "program-digest",
        questions,
        sources,
    }
}

fn digests(tags: &[&str]) -> Vec<String> {
    tags.iter().map(|tag| format!("digest-{tag}")).collect()
}

#[test]
fn a_crash_before_dispatch_marks_the_claimed_run_unknown() {
    let dir = dir("before-dispatch");
    let questions = digests(&["q1"]);
    let sources = digests(&["s1"]);
    {
        let mut store = Store::open(&dir).unwrap();
        store.claim(&claim("run-a", &questions, &sources)).unwrap();
        // The process dies here: claimed, never dispatched, never settled.
    }
    let mut store = Store::open(&dir).unwrap();
    let runs = store.recover().unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].state, State::Unknown);
    // The pins survive the crash — recovery can say what the run was.
    assert_eq!(runs[0].program.as_str(), "program-digest");
}

#[test]
fn a_crash_mid_execution_leaves_the_dispatched_step_unknown() {
    let dir = dir("mid-execution");
    let questions = digests(&["q1"]);
    let sources = digests(&[]);
    {
        let mut store = Store::open(&dir).unwrap();
        store.claim(&claim("run-b", &questions, &sources)).unwrap();
        store
            .advance("run-b", Mark::step("build", State::Dispatched))
            .unwrap();
        // The process dies mid-step.
    }
    let mut store = Store::open(&dir).unwrap();
    let runs = store.recover().unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].state, State::Unknown);
    let step = runs[0]
        .steps
        .iter()
        .find(|step| step.step == "build")
        .expect("the step record survived");
    assert_eq!(step.state, State::Unknown);
}

#[test]
fn a_crash_after_result_before_settlement_keeps_what_was_written() {
    let dir = dir("before-settlement");
    let questions = digests(&[]);
    let sources = digests(&[]);
    {
        let mut store = Store::open(&dir).unwrap();
        store.claim(&claim("run-c", &questions, &sources)).unwrap();
        store
            .advance("run-c", Mark::step("answer", State::Dispatched))
            .unwrap();
        store
            .advance(
                "run-c",
                Mark::step("answer", State::Answered).result("atif:session-7"),
            )
            .unwrap();
        // The result landed; the settle never did. The process dies.
    }
    let mut store = Store::open(&dir).unwrap();
    let runs = store.recover().unwrap();
    assert_eq!(runs.len(), 1);
    // The run is unfinished and marked so, and the step is too: its
    // answer was written but nobody settled it, so whether the answer
    // reached anyone is exactly the `unknown` a reconciler checks.
    assert_eq!(runs[0].state, State::Unknown);
    let step = runs[0]
        .steps
        .iter()
        .find(|step| step.step == "answer")
        .expect("the step record survived");
    assert_eq!(step.state, State::Unknown);
    // The written answer stays in the record as evidence — recovery
    // marks over it, it does not erase it. A reconciler reads the mark
    // and the answer both, which is what reconciling unknown completion
    // against observed evidence means.
    let text = std::fs::read_to_string(dir.join("run-c.jsonl")).unwrap();
    assert!(
        text.contains(r#""state":"answered""#) && text.contains("atif:session-7"),
        "the answered mark and its result ref remain in the record"
    );
}

#[test]
fn a_torn_tail_from_a_crash_mid_append_is_not_corruption() {
    let dir = dir("torn-tail");
    let questions = digests(&[]);
    let sources = digests(&[]);
    {
        let mut store = Store::open(&dir).unwrap();
        store.claim(&claim("run-d", &questions, &sources)).unwrap();
        store
            .advance("run-d", Mark::step("work", State::Dispatched))
            .unwrap();
        // A crash mid-append leaves a partial last line.
        let path = dir.join("run-d.jsonl");
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        use std::io::Write;
        write!(file, "{{\"schema\":\"runstate/v1\",\"record\":\"run\",\"sta").unwrap();
    }
    let mut store = Store::open(&dir).unwrap();
    // The fold treats the torn tail as a crashed append, not corruption:
    // recovery still completes and marks what never finished.
    let runs = store.recover().unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].state, State::Unknown);
}

#[test]
fn a_settled_record_is_no_crash_recovery_business() {
    let dir = dir("settled");
    let questions = digests(&["q1"]);
    let sources = digests(&["s1"]);
    {
        let mut store = Store::open(&dir).unwrap();
        store.claim(&claim("run-e", &questions, &sources)).unwrap();
        store
            .advance("run-e", Mark::step("work", State::Dispatched))
            .unwrap();
        store
            .advance("run-e", Mark::step("work", State::Answered))
            .unwrap();
        store
            .settle("run-e", Outcome::Answered, "commit-abc")
            .unwrap();
        // The process dies after settling — cleanup's crash point.
    }
    let mut store = Store::open(&dir).unwrap();
    let runs = store.recover().unwrap();
    assert!(runs.is_empty(), "a settled run is not unfinished");
    let run = store.get("run-e").unwrap().unwrap();
    assert_eq!(run.state, State::Settled);
    assert_eq!(run.outcome, Some(Outcome::Answered));
}

#[test]
fn recovery_is_idempotent_across_duplicate_resumes() {
    let dir = dir("dup-resume");
    let questions = digests(&[]);
    let sources = digests(&[]);
    {
        let mut store = Store::open(&dir).unwrap();
        store.claim(&claim("run-f", &questions, &sources)).unwrap();
        store
            .advance("run-f", Mark::step("work", State::Dispatched))
            .unwrap();
    }
    // First resume marks the unfinished work.
    let mut first = Store::open(&dir).unwrap();
    let runs = first.recover().unwrap();
    assert_eq!(runs.len(), 1);
    drop(first);
    // A second resume finds nothing unfinished twice — each unfinished
    // record (the run and its dispatched step) took one unknown mark,
    // and recovery does not pile another on.
    let mut second = Store::open(&dir).unwrap();
    let runs = second.recover().unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].state, State::Unknown);
    // The record file itself shows exactly two unknown marks — one per
    // unfinished record, none per resume.
    let text = std::fs::read_to_string(dir.join("run-f.jsonl")).unwrap();
    let unknowns = text.lines().filter(|line| line.contains(r#""state":"unknown""#)).count();
    assert_eq!(unknowns, 2, "one unknown mark per record, none per resume");
}

#[test]
fn a_second_process_cannot_claim_an_owned_run() {
    let dir = dir("cross-process");
    let questions = digests(&["q1"]);
    let sources = digests(&[]);
    // Two stores over the same directory are two processes' view of it:
    // the claim file's create_new is the ownership, not a lock table.
    let mut first = Store::open(&dir).unwrap();
    first.claim(&claim("run-g", &questions, &sources)).unwrap();
    let mut second = Store::open(&dir).unwrap();
    let refused = second.claim(&claim("run-g", &questions, &sources));
    assert!(
        matches!(refused, Err(Refusal::Claimed { .. })),
        "the second process's claim refused: {refused:?}"
    );
    // And the first owner's record is untouched by the refused claim.
    let run = second.get("run-g").unwrap().unwrap();
    assert_eq!(run.state, State::Pending);
}

#[test]
fn a_retained_worktree_survives_the_crash_that_kept_it() {
    let dir = dir("worktree");
    let questions = digests(&[]);
    let sources = digests(&[]);
    let worktree = PathBuf::from("/tmp/run-h-worktree");
    {
        let mut store = Store::open(&dir).unwrap();
        store.claim(&claim("run-h", &questions, &sources)).unwrap();
        store
            .advance(
                "run-h",
                Mark::task("issue-1", 1, State::Dispatched).retaining(worktree.clone()),
            )
            .unwrap();
        // Crash with the worktree still claimed.
    }
    let mut store = Store::open(&dir).unwrap();
    let runs = store.recover().unwrap();
    assert_eq!(runs.len(), 1);
    let task = runs[0]
        .tasks
        .iter()
        .find(|task| task.task == "issue-1")
        .expect("the task record survived");
    assert_eq!(task.state, State::Unknown);
    assert_eq!(task.worktree.as_deref(), Some(worktree.as_path()));
}
