//! A recorded fan-out reads back the way the golden does.
//!
//! `crates/coderbench` judges an episode by reading its trace, so the test
//! that matters for delegation is not that the calls were made but that a
//! reader counts them. This runs six delegations against a stub executor,
//! records them, and hands the file to `coderbench::observe` — the same
//! function that reads `goldens/devin-fan-out-six.atif.jsonl`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use coder::capability::Presence;
use coder::delegate::{Bounds, DEVIN_LOCAL, Delegator, Executor, Isolation, Status, Task};
use coder::survey::Survey;
use coder::trace::{DELEGATE_CALL, Recorder};
use serde_json::Value;

/// The six questions the golden asks, with the answers it recorded.
const QUESTIONS: &[(&str, &str, &str)] = &[
    (
        "How many `pub struct` declarations are in crates/atif/src/document.rs?",
        "crates/atif/src/document.rs",
        "5",
    ),
    (
        "What are the three partition names in the Partition enum in crates/gym/src/suite.rs?",
        "crates/gym/src/suite.rs",
        "calibration, development, locked",
    ),
    (
        "What are the variant names of the Estimator enum in crates/lev/src/estimator.rs?",
        "crates/lev/src/estimator.rs",
        "L1, L2, L3",
    ),
    (
        "How many distinct kev checkpoints are named in docs/kev/model-cards.md?",
        "docs/kev/model-cards.md",
        "4",
    ),
    (
        "Which single Nostr event kind number does nips/openagents/NIP-PRG.md define?",
        "nips/openagents/NIP-PRG.md",
        "30182",
    ),
    (
        "What is the value of the ROUNDS_MAX constant in crates/coder/src/shell.rs?",
        "crates/coder/src/shell.rs",
        "3",
    ),
];

/// A stub executor that answers each question the way the golden's
/// delegates did, after a pause long enough to show the fan-out is
/// concurrent.
fn stub(dir: &Path) -> PathBuf {
    // The prompt is the last argument, after the ones the adapter sends.
    let mut script = String::from(
        "#!/bin/sh\nsleep 0.3\nfor a in \"$@\"; do prompt=\"$a\"; done\ncase \"$prompt\" in\n",
    );
    for (prompt, _, answer) in QUESTIONS {
        let key = prompt.split(' ').next_back().unwrap_or(prompt);
        script.push_str(&format!("  *\"{key}\") printf '{answer}\\n' ;;\n"));
    }
    script.push_str("  *) printf 'no answer\\n' ;;\nesac\n");
    let path = dir.join("devin");
    std::fs::write(&path, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    path
}

/// An executor over a stub, the shape `survey::executor` builds over a
/// probed manifest: a resolved path, no arguments before the prompt, and
/// the refusal the manifest declares. It answers to the slug the golden
/// records, because a trace that named the stub would not read back the
/// way a golden does.
fn executor(binary: impl Into<PathBuf>) -> Executor {
    Executor::new(DEVIN_LOCAL, binary, Vec::new()).refusing(
        "untrusted_workspace",
        "Refusing to run in an untrusted workspace",
    )
}

/// The repository the live cases run in. `CODER_DELEGATE_DIR` moves it,
/// which matters because an executor refuses a checkout nobody trusts.
fn repository() -> PathBuf {
    match std::env::var_os("CODER_DELEGATE_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("the repository root"),
    }
}

/// The live executor, resolved the way a run resolves one: from the
/// manifest in `capabilities/`, probed against the directory the
/// delegations will run in. The probe runs under the operator's trust,
/// so the manifest must be approved first:
///
/// ```text
/// cargo run -p capability --bin capability-trust -- approve devin-local --in .
/// ```
fn live_executor(repository: &Path) -> Executor {
    let survey = Survey::read(Some(repository), repository);
    let found = survey
        .capability(DEVIN_LOCAL)
        .expect("the repository declares devin-local");
    assert!(
        matches!(found.presence, Presence::Present { .. }),
        "{}",
        found.message()
    );
    survey
        .executor(DEVIN_LOCAL)
        .expect("a present capability drives an executor")
}

fn tasks() -> Vec<Task> {
    QUESTIONS
        .iter()
        .map(|(prompt, reads, answer)| Task::reading(prompt, reads).expecting(answer))
        .collect()
}

#[tokio::test]
async fn six_recorded_delegations_read_back_as_the_golden_does() {
    // Every delegation runs under an enforced filesystem boundary; on a
    // platform without one, nothing here can spawn.
    if !coder::delegate::boundary_supported() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let delegator = Delegator::new(executor(stub(dir.path())))
        .in_directory(dir.path())
        .bounded_to(6);

    let started = std::time::Instant::now();
    let delegations = delegator.fan_out(tasks()).await;
    let wall = started.elapsed();
    let summed: Duration = delegations.iter().map(|d| d.elapsed).sum();

    assert_eq!(delegations.len(), 6);
    assert!(delegations.iter().all(|d| d.correct() == Some(true)));
    assert!(
        wall < summed,
        "the six ran concurrently: {wall:?} of wall clock against {summed:?} summed"
    );

    let mut recorder = Recorder::open(dir.path(), "kev-latest", "stub", "/tmp/repo").unwrap();
    let path = recorder.path().to_path_buf();
    recorder
        .user("Delegate six instances of Devin, one for each of these six read-only questions.");
    for delegation in &delegations {
        recorder.delegation(delegation);
    }
    drop(recorder);

    // The reader that judges an episode counts six delegations, finds them
    // all correct, and sees no writes.
    let run = coderbench::observe(&path).expect("the trace observes");
    assert_eq!(run.delegations.len(), 6);
    assert!(run.delegations.iter().all(|d| d.correct == Some(true)));
    assert!(run.writes.is_empty(), "a read-only fan-out writes nothing");
    assert_eq!(
        run.checks
            .iter()
            .filter(|check| check.name == DELEGATE_CALL)
            .count(),
        6
    );

    // And the call carries the arguments the golden's calls carry.
    let recording = atif::log::read(&path).unwrap();
    let call = recording.steps[1].call.as_ref().unwrap();
    assert_eq!(call.name, DELEGATE_CALL);
    assert_eq!(call.arguments["agent"], "devin-local");
    assert_eq!(call.arguments["isolation"], "directory");
    assert_eq!(call.arguments["prompt"], QUESTIONS[0].0);
    assert_eq!(call.arguments["bounds"]["minutes"], 5);
    assert_eq!(call.output, QUESTIONS[0].2);
    assert_eq!(call.outcome, atif::Outcome::Completed);
    assert_eq!(call.extra["capability"], "devin-local");
    assert_eq!(call.extra["reads"], QUESTIONS[0].1);
    assert_eq!(call.extra["expected"], QUESTIONS[0].2);
    assert_eq!(call.extra["correct"], true);
    assert_eq!(call.extra["wrote"], Value::Null);
    assert_eq!(call.extra["concurrent_max"], 6);
    // The resolved path is recorded, because a bare name is what failed
    // the first time this ran.
    assert!(
        Path::new(call.extra["executor_path"].as_str().unwrap()).is_absolute(),
        "the executor is recorded by absolute path"
    );
    assert!(
        recording.steps[1]
            .message
            .starts_with("Delegated: How many")
    );
}

/// A refusal, a timeout, and a failure are three outcomes in the trace,
/// not one.
#[tokio::test]
async fn the_three_ways_a_delegation_does_not_answer_stay_apart() {
    if !coder::delegate::boundary_supported() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let write = |name: &str, body: &str| {
        let path = dir.path().join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        path
    };
    // A bound loose enough that a busy machine does not turn the refusal
    // or the failure into a timeout. The timeout case states its own.
    // Each task states an expected answer, so a delegation that never
    // answered is graded wrong rather than left unjudged.
    let task = || {
        Task::reading("anything", "a.rs")
            .expecting("an answer")
            .bounded(Bounds::within(Duration::from_secs(20)))
    };
    let brief = || {
        Task::reading("anything", "a.rs")
            .expecting("an answer")
            .bounded(Bounds::within(Duration::from_millis(250)))
    };

    let refusing = write(
        "refusing",
        "echo 'Error: Refusing to run in an untrusted workspace: /private/tmp' >&2; exit 1",
    );
    let slow = write("slow", "sleep 30");
    let broken = write("broken", "echo boom >&2; exit 3");

    let mut recorder = Recorder::open(dir.path(), "kev-latest", "stub", "/tmp/repo").unwrap();
    let path = recorder.path().to_path_buf();
    for (binary, task) in [(&refusing, task()), (&slow, brief()), (&broken, task())] {
        let delegation = Delegator::new(executor(binary)).run(task).await;
        recorder.delegation(&delegation);
    }
    let mut worktree = task();
    worktree.isolation = Isolation::Worktree;
    let unisolated = Delegator::new(executor(&refusing)).run(worktree).await;
    assert_eq!(
        unisolated.status,
        Status::Refused("isolation_unavailable".into())
    );
    recorder.delegation(&unisolated);
    drop(recorder);

    let recording = atif::log::read(&path).unwrap();
    let calls: Vec<_> = recording
        .steps
        .iter()
        .filter_map(|step| step.call.as_ref())
        .collect();

    // The executor's own refusal is a call that never ran.
    assert_eq!(calls[0].outcome, atif::Outcome::Cancelled);
    assert_eq!(calls[0].extra["refusal"], "untrusted_workspace");
    assert_eq!(calls[0].extra["status"], "refused: untrusted_workspace");

    // A bound that expired and a non-zero exit both failed, and the trace
    // says which is which.
    assert_eq!(calls[1].outcome, atif::Outcome::Failed);
    assert_eq!(calls[1].extra["status"], "timed out");
    assert!(calls[1].extra.get("exit_code").is_none());

    assert_eq!(calls[2].outcome, atif::Outcome::Failed);
    assert_eq!(calls[2].extra["status"], "failed: exit 3");
    assert_eq!(calls[2].extra["exit_code"], 3);

    // The host's own refusal is a refusal too: it is about the request,
    // and nothing was attempted.
    assert_eq!(calls[3].outcome, atif::Outcome::Cancelled);
    assert_eq!(calls[3].extra["refusal"], "isolation_unavailable");

    // A delegation that did not answer is still a delegation the reader
    // counts, and it did not answer correctly.
    let run = coderbench::observe(&path).unwrap();
    assert_eq!(run.delegations.len(), 4);
    assert!(run.delegations.iter().all(|d| d.correct == Some(false)));
}

/// The live check, against the Devin CLI this operator has.
///
/// Ignored by default: it needs the binary, it costs about 20 seconds, and
/// the executor refuses a directory it does not trust. Run it from a
/// checkout the executor trusts:
///
/// ```text
/// cargo +1.97.1 test -p coder --test delegation -- --ignored --nocapture
/// ```
#[tokio::test]
#[ignore = "needs the Devin CLI and a workspace it trusts"]
async fn a_live_delegation_answers_one_read_only_question() {
    let repository = repository();
    let executor = live_executor(&repository);
    assert!(executor.binary.is_absolute());

    let delegation = Delegator::new(executor)
        .in_repository(&repository)
        .run(
            Task::reading(
                "What is the value of the ROUNDS_MAX constant in crates/coder/src/shell.rs? \
                 Answer with a single integer and nothing else.",
                "crates/coder/src/shell.rs",
            )
            .expecting("3"),
        )
        .await;

    println!("{}", delegation.line());
    assert_eq!(delegation.status, Status::Answered, "{}", delegation.detail);
    assert_eq!(delegation.correct(), Some(true), "{}", delegation.output);
}

/// The golden's six questions, live and at once, judged by the reader that
/// judges the golden.
///
/// Ignored for the same reasons as the single live delegation, and it
/// costs a minute. The answers are the ones the golden recorded, so a
/// rename that moves one of these files fails this test the way it
/// invalidated the recording.
#[tokio::test]
#[ignore = "needs the Devin CLI and a workspace it trusts"]
async fn a_live_fan_out_of_six_runs_in_parallel() {
    let repository = repository();
    let delegator = Delegator::new(live_executor(&repository))
        .in_repository(&repository)
        .bounded_to(6);

    let live: Vec<Task> = QUESTIONS
        .iter()
        .map(|(prompt, reads, answer)| {
            Task::reading(
                &format!("{prompt} Answer with the answer alone and nothing else."),
                reads,
            )
            .expecting(answer)
        })
        .collect();

    let started = std::time::Instant::now();
    let delegations = delegator.fan_out(live).await;
    let wall = started.elapsed();
    let summed: Duration = delegations.iter().map(|d| d.elapsed).sum();

    let dir = tempfile::tempdir().unwrap();
    let mut recorder = Recorder::open(
        dir.path(),
        "kev-latest",
        "stub",
        &repository.display().to_string(),
    )
    .unwrap();
    let path = recorder.path().to_path_buf();
    for delegation in &delegations {
        println!("{} — {}", delegation.line(), delegation.recorded_output());
        recorder.delegation(delegation);
    }
    drop(recorder);

    println!("{wall:?} of wall clock against {summed:?} summed, at a width of 6");
    assert!(wall < summed, "the six ran concurrently");

    let run = coderbench::observe(&path).expect("the trace observes");
    assert_eq!(run.delegations.len(), 6);
    assert!(run.delegations.iter().all(|d| d.correct == Some(true)));
}
