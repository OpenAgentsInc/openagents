//! `delegate-fan-out` runs from its definition, and the trace reads back
//! the way the golden does.
//!
//! The program under test is the repository's own
//! `programs/delegate-fan-out.json`, the wording is the repository's own
//! `questions/`, and the steps are run by `coder::runtime`. What stands in
//! for the machine is the executor and the decision door: a shell script
//! that answers the golden's six questions, and a local HTTP server that
//! answers the two decision calls. Everything between them is the code a
//! live run uses.
//!
//! The test that matters most is the last one: the trace is handed to
//! `coderbench::observe`, the same reader that judges
//! `goldens/devin-fan-out-six.atif.jsonl`, and the task's own manifest
//! judges what it read.

use std::io::Write as _;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use coder::capability::Trust;
use coder::delegate::{Verdict, WORKTREE_DIR, boundary_supported};
use coder::program::Program;
use coder::program_authority::Grant;
use coder::questions;
use coder::runtime::{Host, Inputs, Runtime};
use coder::survey::Survey;
use coder::trace::Recorder;
use coder::turn::{self, Completion, Event};
use coder::{Agent, Delegation, Door, Repo, Route, StubGenerate, Task};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

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

/// The repository this crate lives in, whose `programs/` and `questions/`
/// the test runs from.
fn checkout() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Whether this host can put a delegation inside an enforced filesystem
/// boundary. A host without a backend refuses every `delegate` step, so a
/// case that runs one cannot run here; it says so and returns rather than
/// failing for a tool the machine lacks.
fn boundary() -> bool {
    if boundary_supported() {
        return true;
    }
    eprintln!("skipping: this host has no filesystem boundary backend (bwrap on Linux)");
    false
}

/// A checkout with one commit, the repository's programs and questions,
/// and a capability manifest for a stub executor.
///
/// A real checkout rather than a bare directory, because
/// `isolation: worktree` is a bound this host provides by making one, and
/// a test that skipped it would not be testing the program the repository
/// carries.
fn machine() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a scratch directory");
    let root = dir.path();
    for named in ["programs", "questions", "capabilities"] {
        std::fs::create_dir_all(root.join(named)).unwrap();
    }
    for named in ["programs", "questions"] {
        for entry in std::fs::read_dir(checkout().join(named)).unwrap() {
            let entry = entry.unwrap();
            std::fs::copy(entry.path(), root.join(named).join(entry.file_name())).unwrap();
        }
    }
    std::fs::write(
        root.join("capabilities").join("stub-local.json"),
        manifest(root),
    )
    .unwrap();
    std::fs::write(root.join("a.rs"), "// a file\n").unwrap();

    let run = |arguments: &[&str]| {
        let done = std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args(arguments)
            .output()
            .expect("version control runs");
        assert!(done.status.success(), "{arguments:?}");
    };
    run(&["init", "--quiet"]);
    run(&["config", "user.email", "test@example.invalid"]);
    run(&["config", "user.name", "A Test"]);
    run(&["add", "."]);
    run(&["commit", "--quiet", "-m", "a machine"]);
    dir
}

/// A capability manifest for a stub executor, in the shape
/// `capabilities/devin-local.json` has: what it enforces, what it will
/// silently ignore, and the argv that hands it a task.
fn manifest(root: &Path) -> String {
    let script = root.join("stub.sh");
    let mut body = String::from("#!/bin/sh\n");
    for (prompt, _, answer) in QUESTIONS {
        let key = prompt.split(' ').next_back().unwrap_or(prompt);
        body.push_str(&format!(
            "case \"$1\" in *\"{key}\") printf '{answer}\\n'; exit 0 ;; esac\n"
        ));
    }
    body.push_str("printf 'no answer\\n'\n");
    let mut file = std::fs::File::create(&script).unwrap();
    file.write_all(body.as_bytes()).unwrap();
    drop(file);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    capability::executor_document(
        "stub-local",
        "sh",
        vec!["sh".into(), "-c".into(), "echo stub 1.0.0".into()],
        json!({
            "summary": "Answers one question, for a test.",
            "name": "A stub executor",
            "enforces": ["minutes"],
            "cannot_enforce": ["tool_set", "role", "budget_cents", "effort"],
            "sees_repository": true,
            "concurrent_max": 6,
            "cost": "local",
            "isolation": ["worktree", "directory"],
            "invoke": ["sh", script.display().to_string()],
            "refuses": [{
                "name": "untrusted_workspace",
                "match": "Refusing to run in an untrusted workspace",
                "explanation": "The executor declines a directory nobody has trusted."
            }]
        }),
    )
    .to_string()
}

/// A decision door that answers the way the recorded episode's door did,
/// including the one answer it got wrong.
///
/// `readonly` came back at 0.17 for a fact the state asserts in as many
/// words, reproducibly, across three recordings. It is kept here rather
/// than corrected, because the program must not depend on it and the test
/// is where that is shown.
async fn door() -> String {
    door_choosing("delegate-fan-out").await
}

/// The same door, answering the selection question with a program of the
/// caller's choosing — `none` included, which is what almost every turn
/// gets.
async fn door_choosing(program: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("a port");
    let address: SocketAddr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut arrived = Vec::new();
                let mut buffer = [0u8; 4096];
                loop {
                    let Ok(read) = socket.read(&mut buffer).await else {
                        return;
                    };
                    if read == 0 {
                        break;
                    }
                    arrived.extend_from_slice(&buffer[..read]);
                    let text = String::from_utf8_lossy(&arrived).to_string();
                    let Some((head, body)) = text.split_once("\r\n\r\n") else {
                        continue;
                    };
                    let length: usize = head
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .and_then(|value| value.trim().parse().ok())
                        })
                        .unwrap_or(0);
                    if body.len() < length {
                        continue;
                    }
                    let asked: Value = serde_json::from_str(body).unwrap_or(Value::Null);
                    let reply = answers(&asked, program).to_string();
                    let _ = socket
                        .write_all(
                            format!(
                                "HTTP/1.1 200 Test\r\ncontent-type: application/json\r\n\
                                 content-length: {}\r\nconnection: close\r\n\r\n{reply}",
                                reply.len()
                            )
                            .as_bytes(),
                        )
                        .await;
                    let _ = socket.flush().await;
                    return;
                }
            });
        }
    });
    format!("http://{address}")
}

/// What the stub door answers, read off the question ids the request
/// carries.
fn answers(asked: &Value, program: &str) -> Value {
    let questions = asked.get("questions").and_then(Value::as_object);
    let asked_about = |id: &str| questions.is_some_and(|questions| questions.contains_key(id));
    if asked_about("program") {
        return json!({
            "model": "kev-latest",
            "answers": {"program": {
                "type": "choice",
                "choice": program,
                "confidence": 0.83,
                "probabilities": {
                    "delegate-fan-out": 0.87,
                    "review-changes": 0.0,
                    "answer-question": 0.13,
                    "run-suite": 0.0
                }
            }}
        });
    }
    // Classify's own questions, for the turns that reach it: a program
    // request never does, and every other turn does.
    if asked_about("action") {
        return json!({
            "model": "kev-latest",
            "answers": {
                "action": {
                    "type": "choice",
                    "choice": "respond",
                    "confidence": 0.9,
                    "probabilities": {"respond": 0.9, "clarify": 0.06, "end": 0.02, "none": 0.02}
                },
                "needs_code": {"type": "noul", "noul": 0.2},
                "risk": {"type": "score", "score": 0.0, "confidence": 0.9, "legend": {}},
                "progress": {"type": "score", "score": 1.0, "confidence": 0.8, "legend": {}}
            }
        });
    }
    if asked_about("independent") {
        return json!({
            "model": "kev-latest",
            "answers": {
                "independent": {"type": "noul", "noul": 0.93},
                "readonly": {"type": "noul", "noul": 0.17},
                "needs_tool_restriction": {"type": "noul", "noul": 0.54}
            }
        });
    }
    let mut per_requirement = serde_json::Map::new();
    for id in questions.into_iter().flatten().map(|(id, _)| id) {
        per_requirement.insert(id.clone(), json!({"type": "noul", "noul": 0.91}));
    }
    json!({"model": "kev-latest", "answers": per_requirement})
}

/// A runtime over the scratch machine, asking the stub door.
async fn runtime(root: &Path) -> Runtime {
    let client = jev::Client::new(
        jev::Config::new()
            .api_key("ts-test-key")
            .base_url(door().await)
            .default_model("kev-latest"),
    )
    .expect("the stub door builds a client");
    Runtime::over(
        Survey::read_with(Some(root), root, &Trust::everything()),
        questions::Registry::open(&[root.join("questions")]),
        Host::with_repository(),
    )
    .asking(Some(client))
}

/// The capability manifest the scratch machine declares.
fn declared(root: &Path) -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(root.join("capabilities").join("stub-local.json")).unwrap(),
    )
    .unwrap()
}

/// Declares it differently, for a case about what a manifest says.
fn redeclare(root: &Path, manifest: &Value) {
    std::fs::write(
        root.join("capabilities").join("stub-local.json"),
        manifest.to_string(),
    )
    .unwrap();
}

fn inputs() -> Inputs {
    Inputs {
        request: "Delegate six instances of Devin, one for each of these six read-only questions."
            .to_string(),
        tasks: QUESTIONS
            .iter()
            .map(|(prompt, reads, answer)| Task::reading(prompt, reads).expecting(answer))
            .collect(),
        executor: "stub-local".to_string(),
    }
}

/// `delegate-fan-out`, with its `fan_out` step's bounds replaced. The rest
/// of the program is the repository's own.
fn program_with(root: &Path, bounds: Value) -> Program {
    let mut program = fan_out(root);
    let step = program
        .steps
        .iter_mut()
        .find(|step| step.name == "fan_out")
        .expect("the program fans out");
    step.bounds = bounds.as_object().cloned().unwrap_or_default();
    program
}

fn fan_out(root: &Path) -> Program {
    Program::load(&root.join("programs").join("delegate-fan-out.json"))
        .expect("the repository's own program")
}

/// One recorded call by name, read back out of the trace.
fn call_named(path: &Path, name: &str) -> atif::Call {
    atif::log::read(path)
        .expect("the trace reads back")
        .steps
        .iter()
        .filter_map(|step| step.call.as_ref())
        .find(|call| call.name == name)
        .unwrap_or_else(|| panic!("{name} ran"))
        .clone()
}

/// One recorded decision call by name.
fn decision_named(path: &Path, name: &str) -> atif::Call {
    let call = call_named(path, name);
    assert!(call.is_decision(), "{name} is a decision");
    call
}

/// Every step runs, in the order the program lists them, and the bounds
/// the program states are the bounds the run held to.
#[tokio::test]
async fn the_first_program_runs_from_its_definition() {
    // Every delegation runs under an enforced filesystem boundary; a
    // platform without one refuses rather than spawning.
    if !boundary_supported() {
        return;
    }
    let machine = machine();
    let root = machine.path();
    let run = runtime(root)
        .await
        .run(&fan_out(root), &inputs(), &Grant::all(), None)
        .await;

    assert_eq!(run.stopped, None, "{:?}", run.stopped);
    assert_eq!(
        run.step_names(),
        ["select", "independence", "admit", "fan_out", "accept"],
        "the order is the program's, not the code's"
    );
    assert_eq!(run.delegations.len(), 6);
    assert_eq!(
        run.correct(),
        (6, 6),
        "{}\n{:#?}",
        run.summary(),
        run.delegations
    );

    // The bounds came off the program. Six at once, one checkout each,
    // five minutes — no, sixty: the step says sixty.
    assert!(
        run.delegations
            .iter()
            .all(|delegation| delegation.concurrent_max == 6)
    );
    assert!(
        run.delegations
            .iter()
            .all(|delegation| delegation.task.isolation == coder::Isolation::Worktree)
    );
    assert!(
        run.delegations
            .iter()
            .all(|delegation| delegation.task.bounds.declared() == &json!({"minutes": 60})),
        "the wall bound is the one the fan_out step states"
    );
    let mut checkouts: Vec<&Path> = run
        .delegations
        .iter()
        .map(|delegation| delegation.workdir.as_path())
        .collect();
    checkouts.sort_unstable();
    checkouts.dedup();
    assert_eq!(checkouts.len(), 6, "six delegations, six checkouts");
    assert!(
        std::fs::read_dir(root.join(WORKTREE_DIR))
            .map(|entries| entries.count())
            .unwrap_or(0)
            == 0,
        "no checkout outlives the step that made it"
    );

    // The independence answer that was wrong did not decide anything. The
    // program fans out on `independent`, and admission is the
    // deterministic check.
    let independence = &run.answers["independence"];
    assert_eq!(independence["readonly"]["noul"], json!(0.17));
    assert_eq!(independence["independent"]["noul"], json!(0.93));
    assert_eq!(run.answers["accept"].as_object().unwrap().len(), 6);
}

/// The operator's sentence picks the program, and the program runs.
#[tokio::test]
async fn a_request_selects_its_program_and_runs_it() {
    if !boundary_supported() {
        return;
    }
    let machine = machine();
    let root = machine.path();
    let run = runtime(root)
        .await
        .apply(&inputs(), &Grant::all(), None)
        .await;

    assert_eq!(run.program.as_deref(), Some("delegate-fan-out"));
    assert_eq!(run.stopped, None, "{:?}", run.stopped);
    assert_eq!(run.answered(), 6);
}

/// The trace reads back the way the golden does: the calls the task grades
/// on, in the order the golden has them, judged by the reader that judges
/// the golden.
#[tokio::test]
async fn the_recorded_run_is_the_path_the_task_expects() {
    if !boundary_supported() {
        return;
    }
    let machine = machine();
    let root = machine.path();
    let traces = tempfile::tempdir().unwrap();
    let mut recorder = Recorder::open(
        traces.path(),
        "kev-latest",
        "stub",
        &root.display().to_string(),
    )
    .unwrap();
    let path = recorder.path().to_path_buf();

    let mut inputs = inputs();
    for task in &mut inputs.tasks {
        task.expected = None;
    }
    recorder.user(&inputs.request);
    let runtime = runtime(root).await;
    runtime.survey().record(&mut recorder, None);
    let run = runtime
        .apply(&inputs, &Grant::all(), Some(&mut recorder))
        .await;
    // A session that ran to the end closes itself, and the grade reads
    // the end record: a trace without one is a session that stopped.
    recorder.finish(atif::log::ENDED);
    drop(recorder);

    assert_eq!(run.stopped, None, "{:?}", run.stopped);

    let recording = atif::log::read(&path).expect("the trace reads back");
    let names: Vec<&str> = recording
        .steps
        .iter()
        .filter_map(|step| step.call.as_ref())
        .map(|call| call.name.as_str())
        .collect();
    assert_eq!(
        names,
        [
            "capability_probe",
            "program_registry",
            "program",
            "program_authority",
            "task_select",
            "independence",
            "admission_check",
            "delegate",
            "delegate",
            "delegate",
            "delegate",
            "delegate",
            "delegate",
            "accept",
        ],
        "the golden's order, with the lookup and the acceptance the golden stopped short of"
    );

    // The decision calls carry the question set that answered, which is
    // the half a program is not allowed to carry.
    let decisions: Vec<&atif::Call> = recording
        .steps
        .iter()
        .filter_map(|step| step.call.as_ref())
        .filter(|call| call.is_decision())
        .collect();
    assert_eq!(decisions.len(), 3);
    assert_eq!(
        decisions[1].extra["question_set"],
        json!("openagents.independence.v2")
    );
    assert!(
        decisions[1].extra["set_digest"].as_str().is_some_and(
            |digest| digest.len() == 64 && digest.chars().all(|c| c.is_ascii_hexdigit())
        ),
        "two runs of the same program asked the same wording, and the digest says so"
    );
    assert_eq!(decisions[1].extra["gate"], json!("independent"));

    let mut task = coderbench::Task::load(
        &coderbench::tasks_dir()
            .join("devin-fan-out-six")
            .join("task.json"),
    )
    .expect("the task manifest loads");
    // This fixture has its own prompts and fixed answers. Keep those
    // expectations in the grader; the runtime received none of them.
    task.grade.expects = QUESTIONS
        .iter()
        .map(|(prompt, _, answer)| coderbench::ExpectedAnswer {
            prompt: (*prompt).to_string(),
            answer: (*answer).to_string(),
        })
        .collect();
    let mut observed = coderbench::observe(&path).expect("the grader reads the trace");
    assert_eq!(observed.program.as_deref(), Some("delegate-fan-out"));
    assert_eq!(observed.delegations.len(), 6);
    assert!(
        observed
            .delegations
            .iter()
            .all(|call| call.correct.is_none())
    );
    assert!(observed.writes.is_empty());

    // The grade wants two facts a trace cannot carry: how the turn ended,
    // which only an exit code says, and what the workspace looked like
    // before and after, which only whoever started the run can compare.
    // This test is that caller — it drove the runtime in process against a
    // machine it built — so it states them rather than leaving the grade
    // unverifiable on evidence it has.
    observed.ending = coderbench::Ending::Answered;
    observed.workspace = Some(coderbench::Workspace::default());

    let judgment = task.judge(&observed);
    assert!(
        !judgment.faults.iter().any(|fault| matches!(
            fault,
            coderbench::Fault::DecisionMissing { .. } | coderbench::Fault::CheckMissing { .. }
        )),
        "every decision and check the task names was made: {:?}",
        judgment.faults
    );
    assert!(judgment.passed(), "{:?}", judgment.faults);
}

/// A step whose bounds this host cannot enforce does not run, and neither
/// does the program that carried it.
#[tokio::test]
async fn a_bound_this_host_cannot_enforce_refuses_before_anything_runs() {
    let machine = machine();
    let root = machine.path();
    let runtime = runtime(root).await;

    for (bounds, expected) in [
        // A shape nobody here can make. Running it in the shared
        // directory instead is the substitution the rule forbids.
        (json!({"isolation": "vm", "minutes": 60}), "vm checkout"),
        // A bound key this host keeps nothing for.
        (json!({"budget_cents": 500}), "cannot enforce budget_cents"),
        // A key it knows, carrying a value it cannot hold to.
        (json!({"concurrent_max": 0}), "count above zero"),
    ] {
        let program = program_with(root, bounds.clone());
        let refused = runtime
            .admit(&program)
            .expect_err(&format!("{bounds} is not enforceable here"));
        assert_eq!(refused.step, "fan_out");
        assert_eq!(refused.code, "bound_unenforceable");
        assert!(refused.reason.contains(expected), "{refused}");

        let run = runtime.run(&program, &inputs(), &Grant::all(), None).await;
        assert!(run.steps.is_empty(), "nothing ran: {:?}", run.step_names());
        assert!(run.delegations.is_empty());
        assert_eq!(run.stopped, Some(refused));
    }
}

/// A host that cannot make a checkout refuses the step that asked for one,
/// rather than running it in the directory it shares.
#[tokio::test]
async fn a_host_that_cannot_isolate_refuses_the_fan_out() {
    let machine = machine();
    let root = machine.path();
    let runtime = runtime(root).await.on(Host::without_repository());
    let refused = runtime
        .admit(&fan_out(root))
        .expect_err("worktree is not a shape this host makes");
    assert_eq!(refused.step, "fan_out");
    assert!(refused.reason.contains("worktree checkout"), "{refused}");
}

/// A step kind this host does not run refuses the whole program. It is
/// never skipped: a program whose unknown steps are skipped is a different
/// program.
#[tokio::test]
async fn a_step_kind_this_host_does_not_run_refuses_the_program() {
    let machine = machine();
    let root = machine.path();
    let runtime = runtime(root).await;

    // A kind the registry does not know at all never becomes a program.
    let exotic = root.join("exotic.json");
    std::fs::write(
        &exotic,
        r#"{"definition":{"v":1,"requires":[],"id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:openagents/exotic","summary":"no","input":{"digest":"sha256:a2c799262a3ce3c19ef5cdd983bf3d12b43ab3c426227091b909dcb7054738c0","size":17,"media_type":"application/schema+json"},"output":{"digest":"sha256:a2c799262a3ce3c19ef5cdd983bf3d12b43ab3c426227091b909dcb7054738c0","size":17,"media_type":"application/schema+json"},"bounds":{},"result":{"from":"input","pointer":""},"steps":[{"name":"two","kind":"teleport","after":[],"input":{"from":"input","pointer":""},"output":{"digest":"sha256:a2c799262a3ce3c19ef5cdd983bf3d12b43ab3c426227091b909dcb7054738c0","size":17,"media_type":"application/schema+json"},"bounds":{},"on_error":"stop"}]},"binding":{"steps":{}}}"#,
    )
    .unwrap();
    let reason = Program::load(&exotic).expect_err("an unknown kind is refused");
    assert!(reason.contains("teleport"), "{reason}");

    // A module step whose guest bytes are not pinned is refused at
    // admission, before the step in front of it runs.
    let composed = root.join("composed.json");
    std::fs::write(
        &composed,
        r#"{"definition":{"v":1,"requires":[],"id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:openagents/composed","summary":"no","input":{"digest":"sha256:a2c799262a3ce3c19ef5cdd983bf3d12b43ab3c426227091b909dcb7054738c0","size":17,"media_type":"application/schema+json"},"output":{"digest":"sha256:a2c799262a3ce3c19ef5cdd983bf3d12b43ab3c426227091b909dcb7054738c0","size":17,"media_type":"application/schema+json"},"bounds":{},"result":{"from":"step:one","pointer":"/value"},"steps":[{"name":"one","kind":"query","after":[],"input":{"from":"input","pointer":""},"output":{"digest":"sha256:a2c799262a3ce3c19ef5cdd983bf3d12b43ab3c426227091b909dcb7054738c0","size":17,"media_type":"application/schema+json"},"bounds":{},"on_error":"stop"},{"name":"two","kind":"module","after":["one"],"input":{"from":"step:one","pointer":"/value"},"output":{"digest":"sha256:a2c799262a3ce3c19ef5cdd983bf3d12b43ab3c426227091b909dcb7054738c0","size":17,"media_type":"application/schema+json"},"bounds":{},"on_error":"stop"}]},"binding":{"steps":{"one":{"bounds":{"max_results":4}},"two":{"module":{"sha256":"00"}}}}}"#,
    )
    .unwrap();
    let program = Program::load(&composed).expect("composition parses");
    let refused = runtime
        .admit(&program)
        .expect_err("a module step without guest bytes is not admitted");
    assert_eq!(refused.step, "two");
    assert_eq!(refused.code, "content_unavailable");

    let run = runtime.run(&program, &inputs(), &Grant::all(), None).await;
    assert!(
        run.steps.is_empty(),
        "the query step in front of it did not run either: {:?}",
        run.step_names()
    );
}

/// An executor declaration cannot authorize a bound the host cannot verify.
#[tokio::test]
async fn an_executor_claim_does_not_establish_enforcement() {
    let machine = machine();
    let root = machine.path();
    let mut manifest = declared(root);
    manifest["binding"]["claims_enforced"] = json!(["minutes", "memory_mb"]);
    redeclare(root, &manifest);
    let mut program = fan_out(root);
    let delegate = program
        .steps
        .iter_mut()
        .find(|step| step.kind == coder::program::Kind::Delegate)
        .unwrap();
    delegate.bounds.insert("memory_mb".to_string(), json!(128));
    let delegate_name = delegate.name.clone();
    let run = runtime(root)
        .await
        .run(&program, &inputs(), &Grant::all(), None)
        .await;
    let stopped = run
        .stopped
        .expect("an unsupported bound refuses before any step runs");
    assert_eq!(stopped.step, delegate_name);
    assert_eq!(stopped.code, "bound_unenforceable");
    assert!(run.steps.is_empty());
    assert!(run.delegations.is_empty());
}

/// The supervisor's deadline does not depend on an executor's promise.
#[tokio::test]
async fn the_host_holds_minutes_even_when_the_executor_does_not() {
    if !boundary_supported() {
        return;
    }
    for ignored in [json!([]), json!(["minutes"])] {
        let machine = machine();
        let root = machine.path();
        let mut manifest = declared(root);
        manifest["binding"]["claims_enforced"] = json!([]);
        manifest["binding"]["claims_not_enforced"] = ignored;
        redeclare(root, &manifest);
        let run = runtime(root)
            .await
            .run(&fan_out(root), &inputs(), &Grant::all(), None)
            .await;
        assert!(run.finished(), "{:?}", run.stopped);
        assert_eq!(run.answered(), 6);
    }
}

/// Every bound the fan-out names is held by somebody, and the trace says
/// by whom.
#[tokio::test]
async fn the_check_records_who_holds_each_bound() {
    if !boundary() {
        return;
    }
    let machine = machine();
    let root = machine.path();
    let traces = tempfile::tempdir().unwrap();
    let mut recorder = Recorder::open(
        traces.path(),
        "kev-latest",
        "stub",
        &root.display().to_string(),
    )
    .unwrap();
    let path = recorder.path().to_path_buf();
    runtime(root)
        .await
        .run(
            &fan_out(root),
            &inputs(),
            &Grant::all(),
            Some(&mut recorder),
        )
        .await;
    drop(recorder);

    let recording = atif::log::read(&path).unwrap();
    let check = recording
        .steps
        .iter()
        .filter_map(|step| step.call.as_ref())
        .find(|call| call.name == "admission_check")
        .expect("the check ran");
    assert_eq!(check.extra["admitted"], json!(true));
    assert_eq!(check.extra["enforcement"]["minutes"], json!("host"));
    assert_eq!(check.extra["enforcement"]["concurrent_max"], json!("host"));
    assert_eq!(check.extra["enforcement"]["isolation"], json!("host"));
    assert_eq!(check.extra["required"], json!([]));
}

/// An executor this machine cannot reach is a route nobody was offered,
/// and the step that needed it refuses rather than failing six times.
#[tokio::test]
async fn an_absent_executor_refuses_the_delegate_step() {
    if !boundary() {
        return;
    }
    let machine = machine();
    let root = machine.path();
    let mut inputs = inputs();
    inputs.executor = "not-a-capability-here".to_string();

    let run = runtime(root)
        .await
        .run(&fan_out(root), &inputs, &Grant::all(), None)
        .await;

    let stopped = run.stopped.expect("there is no executor");
    assert_eq!(stopped.step, "admit", "the check names it first");
    assert_eq!(stopped.code, "capability_undeclared");
    assert!(run.delegations.is_empty());
}

/// A program that fans out over nothing is not a fan-out.
#[tokio::test]
async fn a_lookup_that_found_no_work_refuses() {
    if !boundary() {
        return;
    }
    let machine = machine();
    let root = machine.path();
    let mut inputs = inputs();
    inputs.tasks.clear();

    let run = runtime(root)
        .await
        .run(&fan_out(root), &inputs, &Grant::all(), None)
        .await;
    let stopped = run.stopped.expect("nothing to delegate");
    assert_eq!(stopped.step, "select");
    assert_eq!(stopped.code, "no_tasks");
}

/// `max_results` bounds the lookup rather than describing it, and a step
/// that truncates says which work it dropped.
#[tokio::test]
async fn the_lookup_holds_to_its_own_bound() {
    if !boundary() {
        return;
    }
    let machine = machine();
    let root = machine.path();
    let mut program = fan_out(root);
    program.steps[0]
        .bounds
        .insert("max_results".to_string(), json!(2));
    program.steps[0]
        .bounds
        .insert("on_overflow".to_string(), json!("truncate"));
    let mut inputs = inputs();
    inputs.tasks = inputs.tasks.into_iter().cycle().take(20).collect();

    let traces = tempfile::tempdir().unwrap();
    let mut recorder = Recorder::open(
        traces.path(),
        "kev-latest",
        "stub",
        &root.display().to_string(),
    )
    .unwrap();
    let path = recorder.path().to_path_buf();
    let run = runtime(root)
        .await
        .run(&program, &inputs, &Grant::all(), Some(&mut recorder))
        .await;
    drop(recorder);

    assert_eq!(run.stopped, None, "{:?}", run.stopped);
    assert_eq!(
        run.delegations.len(),
        2,
        "a lookup that answered with more than the step allows ran unbounded"
    );

    let lookup = call_named(&path, "task_select");
    assert_eq!(lookup.extra["overflow"], json!("truncated"));
    assert_eq!(lookup.extra["found"], json!(20));
    assert_eq!(lookup.extra["selected"], json!(["t1", "t2"]));
    assert_eq!(
        lookup.extra["dropped"]
            .as_array()
            .expect("the dropped work is named")
            .len(),
        18,
        "a run that fanned out to two of twenty says which eighteen it left"
    );
    assert_eq!(lookup.extra["dropped"][0]["id"], json!("t3"));
    assert!(
        lookup.extra["dropped"][0]["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("max_results of 2")),
        "{:?}",
        lookup.extra["dropped"][0]
    );
}

/// The repository's own `select` step refuses instead of truncating, and
/// the trace records the refusal as the lookup's answer.
///
/// Refusing is the stricter of the two and it is the one a burndown wants:
/// the work is not independent, the gate that should catch that is the one
/// #9414 measured at 11 of 12 wrong answers above the floor, and a lookup
/// that quietly chose six of twenty-one would be making the selection
/// nobody reviewed.
#[tokio::test]
async fn a_lookup_over_its_bound_refuses_rather_than_choosing() {
    if !boundary() {
        return;
    }
    let machine = machine();
    let root = machine.path();
    let mut program = fan_out(root);
    program.steps[0]
        .bounds
        .insert("max_results".to_string(), json!(4));
    let mut inputs = inputs();
    inputs.tasks = inputs.tasks.into_iter().cycle().take(21).collect();

    let traces = tempfile::tempdir().unwrap();
    let mut recorder = Recorder::open(
        traces.path(),
        "kev-latest",
        "stub",
        &root.display().to_string(),
    )
    .unwrap();
    let path = recorder.path().to_path_buf();
    let run = runtime(root)
        .await
        .run(&program, &inputs, &Grant::all(), Some(&mut recorder))
        .await;
    drop(recorder);

    let stopped = run.stopped.as_ref().expect("twenty-one is more than four");
    assert_eq!(stopped.step, "select");
    assert_eq!(stopped.code, "too_many_results");
    assert!(run.steps.is_empty(), "{:?}", run.step_names());
    assert!(run.delegations.is_empty());

    let lookup = call_named(&path, "task_select");
    assert_eq!(lookup.extra["overflow"], json!("refused"));
    assert_eq!(lookup.extra["found"], json!(21));
    assert_eq!(lookup.arguments["on_overflow"], json!("refuse"));
}

/// A query step names a source, and a slug this host cannot resolve
/// refuses before the first step runs.
#[tokio::test]
async fn a_source_this_host_does_not_resolve_refuses_the_program() {
    let machine = machine();
    let root = machine.path();
    let mut program = fan_out(root);
    program.steps[0].source = Some("the-open-backlog".to_string());

    let runtime = runtime(root).await;
    let refused = runtime
        .admit(&program)
        .expect_err("this host has no such source");
    assert_eq!(refused.step, "select");
    assert_eq!(refused.code, "source_unresolved");
    assert!(refused.reason.contains("the-open-backlog"), "{refused}");

    let run = runtime.run(&program, &inputs(), &Grant::all(), None).await;
    assert!(run.steps.is_empty(), "{:?}", run.step_names());
    assert_eq!(run.stopped, Some(refused));
}

/// Work looked up from a source and work handed in by the request take the
/// same path: the same ordering, the same bound, the same record, and the
/// same delegations at the end of it.
///
/// The list this reads has a shared path and a declared order, which is
/// what the open backlog has. The shared path is recorded and the declared
/// order is enforced.
#[tokio::test]
async fn a_file_source_is_the_path_an_explicit_list_takes() {
    if !boundary_supported() {
        return;
    }
    let machine = machine();
    let root = machine.path();
    std::fs::create_dir_all(root.join("sources")).unwrap();
    std::fs::write(
        root.join("sources").join("backlog.json"),
        json!({
            "v": 1,
            "slug": "backlog",
            "name": "A work list, for a test",
            "summary": "Reads the work list this checkout carries.",
            "from": {"file": {"path": ".coder/work-list.json"}},
            "order": "id"
        })
        .to_string(),
    )
    .unwrap();
    std::fs::create_dir_all(root.join(".coder")).unwrap();
    // Out of order on purpose, so the recorded order is the source's
    // answer rather than the file's.
    std::fs::write(
        root.join(".coder").join("work-list.json"),
        json!({
            "v": 1,
            "work": [
                {"id": "9401", "prompt": QUESTIONS[0].0, "reads": QUESTIONS[0].1,
                 "after": ["9391"]},
                {"id": "9393", "prompt": QUESTIONS[3].0, "reads": QUESTIONS[3].1},
                {"id": "9391", "prompt": QUESTIONS[1].0, "reads": QUESTIONS[1].1},
                {"id": "9392", "prompt": QUESTIONS[2].0, "reads": QUESTIONS[2].1,
                 "touches": [QUESTIONS[1].1]}
            ]
        })
        .to_string(),
    )
    .unwrap();

    let mut program = fan_out(root);
    program.steps[0].source = Some("backlog".to_string());

    let traces = tempfile::tempdir().unwrap();
    let mut recorder = Recorder::open(
        traces.path(),
        "kev-latest",
        "stub",
        &root.display().to_string(),
    )
    .unwrap();
    let path = recorder.path().to_path_buf();
    let run = runtime(root)
        .await
        .run(&program, &inputs(), &Grant::all(), Some(&mut recorder))
        .await;
    drop(recorder);

    assert_eq!(run.stopped, None, "{:?}", run.stopped);
    assert_eq!(
        run.step_names(),
        ["select", "independence", "admit", "fan_out", "accept"],
        "a queried list runs the same five steps an explicit one does"
    );
    assert_eq!(run.delegations.len(), 3);
    assert_eq!(run.answered(), 3);

    let lookup = call_named(&path, "task_select");
    assert_eq!(lookup.arguments["source"], json!("backlog"));
    assert_eq!(
        lookup.arguments["from"],
        json!("file .coder/work-list.json")
    );
    assert_eq!(lookup.arguments["order"], json!("id"));
    assert_eq!(
        lookup.extra["ordered"],
        json!(["9391", "9392", "9393", "9401"]),
        "the order is the source's, and the trace records it"
    );
    assert_eq!(lookup.extra["selected"], json!(["9391", "9392", "9393"]));
    assert_eq!(lookup.extra["overflow"], json!("none"));

    // #9391 must land before #9401, and that is in the list rather than in
    // a judgment about it.
    assert_eq!(lookup.extra["dropped"][0]["id"], json!("9401"));
    assert!(
        lookup.extra["dropped"][0]["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("9391")),
        "{:?}",
        lookup.extra["dropped"][0]
    );

    // Two items touch one file. The lookup says so; it does not decide
    // what to do about it.
    assert_eq!(
        lookup.extra["collisions"],
        json!([{"path": QUESTIONS[1].1, "work": ["9391", "9392"]}])
    );

    // And the decision that follows reads the collision rather than
    // working it out.
    let independence = decision_named(&path, "independence");
    assert_eq!(
        independence.arguments["state"]["collisions"][0]["path"],
        json!(QUESTIONS[1].1)
    );
}

/// The burn-down, from the repository's own program and source: every
/// delegate is briefed that the common Git directory is sealed and where
/// a commit goes instead, and `accept` judges each item against the
/// answer it states under `expects` — a stated answer passes or fails,
/// and an item that states none is unverifiable, never passed.
#[tokio::test]
async fn the_burn_down_briefs_its_delegates_and_judges_what_each_item_expects() {
    if !boundary_supported() {
        return;
    }
    let machine = machine();
    let root = machine.path();
    std::fs::create_dir_all(root.join("sources")).unwrap();
    std::fs::copy(
        checkout().join("sources").join("work-list.json"),
        root.join("sources").join("work-list.json"),
    )
    .unwrap();
    std::fs::create_dir_all(root.join(".coder")).unwrap();
    std::fs::write(
        root.join(".coder").join("work-list.json"),
        json!({
            "v": 1,
            "work": [
                {"id": "1-right", "prompt": QUESTIONS[0].0, "reads": QUESTIONS[0].1,
                 "expects": QUESTIONS[0].2},
                {"id": "2-wrong", "prompt": QUESTIONS[1].0, "reads": QUESTIONS[1].1,
                 "expects": "not what the stub says"},
                {"id": "3-unstated", "prompt": QUESTIONS[2].0, "reads": QUESTIONS[2].1}
            ]
        })
        .to_string(),
    )
    .unwrap();
    let program = Program::load(&root.join("programs").join("burn-down.json"))
        .expect("the repository's own program");
    let briefing = program
        .steps
        .iter()
        .find(|step| step.name == "fan_out")
        .and_then(|step| step.rest.get("briefing"))
        .and_then(Value::as_str)
        .expect("the burn-down briefs its delegates");
    for said in [
        "common Git directory is sealed",
        "scratch Git directory",
        "reviewer fetches",
    ] {
        assert!(briefing.contains(said), "the briefing says {said:?}");
    }

    let traces = tempfile::tempdir().unwrap();
    let mut recorder = Recorder::open(
        traces.path(),
        "kev-latest",
        "stub",
        &root.display().to_string(),
    )
    .unwrap();
    let path = recorder.path().to_path_buf();
    let run = runtime(root)
        .await
        .run(&program, &inputs(), &Grant::all(), Some(&mut recorder))
        .await;
    drop(recorder);

    assert_eq!(run.stopped, None, "{:?}", run.stopped);
    assert_eq!(run.delegations.len(), 3);
    assert_eq!(run.answered(), 3, "{:#?}", run.delegations);
    for delegation in &run.delegations {
        assert!(
            delegation.task.prompt.starts_with(briefing),
            "every delegate reads the briefing first: {:?}",
            delegation.task.prompt
        );
    }

    assert_eq!(
        run.verdicts(),
        [
            ("t1".to_string(), Verdict::Passed),
            ("t2".to_string(), Verdict::Failed),
            ("t3".to_string(), Verdict::Unverifiable),
        ],
        "{}",
        run.summary()
    );
    assert_eq!(run.correct(), (1, 2));
    assert!(
        run.summary().contains("1 passed, 1 failed, 1 unverifiable"),
        "{}",
        run.summary()
    );
    let accept = run
        .steps
        .iter()
        .find(|step| step.name == "accept")
        .expect("the acceptance ran");
    assert_eq!(accept.output, "1 passed, 1 failed, 1 unverifiable");

    let decided = decision_named(&path, "accept");
    let requirements = &decided.arguments["state"]["requirements"];
    assert_eq!(requirements["t1"]["expects"], json!(QUESTIONS[0].2));
    assert_eq!(requirements["t1"]["verdict"], json!("passed"));
    assert_eq!(requirements["t2"]["verdict"], json!("failed"));
    assert_eq!(requirements["t3"]["expects"], Value::Null);
    assert_eq!(requirements["t3"]["verdict"], json!("unverifiable"));
}

/// A plan whose tasks touch six different files reads the way it always
/// did. The collision record is added when there is one, so the answers
/// #9414 measured stay comparable to the ones this asks for now.
#[tokio::test]
async fn a_plan_with_no_collisions_says_nothing_about_collisions() {
    if !boundary() {
        return;
    }
    let machine = machine();
    let root = machine.path();
    let traces = tempfile::tempdir().unwrap();
    let mut recorder = Recorder::open(
        traces.path(),
        "kev-latest",
        "stub",
        &root.display().to_string(),
    )
    .unwrap();
    let path = recorder.path().to_path_buf();
    runtime(root)
        .await
        .run(
            &fan_out(root),
            &inputs(),
            &Grant::all(),
            Some(&mut recorder),
        )
        .await;
    drop(recorder);

    let state = &decision_named(&path, "independence").arguments["state"];
    assert_eq!(state["collisions"], Value::Null);
    assert_eq!(state["tasks"][0]["id"], json!("t1"));
    assert_eq!(state["tasks"][0]["touches"], Value::Null);
    assert_eq!(state["tasks"][0]["after"], Value::Null);
    assert!(
        call_named(&path, "task_select")
            .extra
            .get("collisions")
            .is_none(),
        "the lookup found none, so it says nothing about them"
    );
}

/// The concurrency bound is the step's, and it is a bound rather than an
/// ambition.
#[tokio::test]
async fn the_fan_out_runs_at_the_width_the_step_states() {
    if !boundary() {
        return;
    }
    let machine = machine();
    let root = machine.path();
    let mut program = fan_out(root);
    let fan_out_step = program
        .steps
        .iter_mut()
        .find(|step| step.name == "fan_out")
        .unwrap();
    fan_out_step
        .bounds
        .insert("concurrent_max".to_string(), json!(1));

    let run = runtime(root)
        .await
        .run(&program, &inputs(), &Grant::all(), None)
        .await;
    assert_eq!(run.stopped, None, "{:?}", run.stopped);
    assert!(
        run.delegations
            .iter()
            .all(|delegation: &Delegation| delegation.concurrent_max == 1),
        "the width the program states is the width the trace records"
    );
}

/// A `decide` step whose question this host has no wording for refuses,
/// because a step that carried its own wording would have been refused
/// when the program was read.
#[tokio::test]
async fn a_question_with_no_wording_refuses() {
    let machine = machine();
    let root = machine.path();
    std::fs::remove_file(root.join("questions").join("independence-v2.json")).unwrap();

    let runtime = runtime(root).await;
    let refused = runtime
        .admit(&fan_out(root))
        .expect_err("the wording is gone");
    assert_eq!(refused.step, "independence");
    assert_eq!(refused.code, "question_unresolved");
    assert!(
        refused.reason.contains("openagents.independence.v2"),
        "{refused}"
    );
}

/// A `refuse_below` bound is a floor the step refuses under, and the
/// refusal stops the program.
#[tokio::test]
async fn an_answer_below_the_floor_stops_the_program() {
    if !boundary() {
        return;
    }
    let machine = machine();
    let root = machine.path();
    let mut program = fan_out(root);
    let step = program
        .steps
        .iter_mut()
        .find(|step| step.name == "independence")
        .unwrap();
    step.bounds.insert("refuse_below".to_string(), json!(0.95));

    let run = runtime(root)
        .await
        .run(&program, &inputs(), &Grant::all(), None)
        .await;
    assert_eq!(run.step_names(), ["select"]);
    let stopped = run.stopped.expect("0.93 is below 0.95");
    assert_eq!(stopped.step, "independence");
    assert_eq!(stopped.code, "below_floor");
    assert!(run.delegations.is_empty());
}

/// The sentence an operator types for a fan-out: the request, and the
/// work it carries, one item per line.
fn sentence() -> String {
    let mut request = String::from(
        "Delegate six instances of Devin, one for each of these six read-only questions.\n",
    );
    for (prompt, _, _) in QUESTIONS {
        request.push_str(&format!(
            "- {prompt} Answer with the value and nothing else.\n"
        ));
    }
    request
}

/// An agent over the scratch machine, asking a door that selects
/// `program`, and generating from the stub.
async fn agent(root: &Path, program: &'static str) -> Agent {
    let client = jev::Client::new(
        jev::Config::new()
            .api_key("ts-test-key")
            .base_url(door_choosing(program).await)
            .default_model("kev-latest"),
    )
    .expect("the stub door builds a client");
    Agent::new(Some(client), Door::Stub(StubGenerate::default()))
        .with_repo(Repo::discover(root))
        .with_survey(Survey::read_with(Some(root), root, &Trust::everything()))
        .with_program_grant(Some("all"))
}

/// The operator's sentence reaches the runtime, through the turn the
/// terminal and `coder --print` both run.
///
/// This is the seam the whole path was missing: everything under it was
/// exercised by the tests above and by nothing a person typed.
#[tokio::test]
async fn a_sentence_runs_the_program_through_a_turn() {
    if !boundary() {
        return;
    }
    let machine = machine();
    let root = machine.path();
    let mut agent = agent(root, "delegate-fan-out").await;

    let mut selected: Vec<String> = Vec::new();
    let finished = turn::run(&mut agent, sentence(), &mut |event| {
        if let Event::Program(slug) = event {
            selected.push(slug);
        }
    })
    .await
    .expect("the turn finished");

    assert_eq!(selected, ["delegate-fan-out"], "the terminal was told");
    let run = finished.program.expect("the turn ran a program");
    assert_eq!(run.program.as_deref(), Some("delegate-fan-out"));
    assert_eq!(run.stopped, None, "{:?}", run.stopped);
    assert_eq!(
        run.step_names(),
        ["select", "independence", "admit", "fan_out", "accept"]
    );
    assert_eq!(run.delegations.len(), 6, "one per item of the list");
    assert_eq!(finished.reply, run.summary());
    assert_eq!(finished.completion, Completion::Answered);
    assert_eq!(
        finished.route, None,
        "a turn that ran a program took no classify route, and says so"
    );
}

/// `none` is the answer almost every turn gets, and on `none` the turn is
/// the turn it was before any of this existed: classified, routed,
/// answered by Generate.
#[tokio::test]
async fn a_turn_that_asks_for_no_program_is_unchanged() {
    let machine = machine();
    let root = machine.path();
    let mut agent = agent(root, "none").await;

    let mut selected: Vec<String> = Vec::new();
    let mut streamed = String::new();
    let finished = turn::run(
        &mut agent,
        "What does ROUNDS_MAX do?".to_string(),
        &mut |event| match event {
            Event::Program(slug) => selected.push(slug),
            Event::Delta(delta) => streamed.push_str(&delta),
            _ => {}
        },
    )
    .await
    .expect("the turn finished");

    assert!(selected.is_empty(), "nothing was selected: {selected:?}");
    assert!(finished.program.is_none());
    assert_eq!(finished.route, Some(Route::Respond));
    assert_eq!(finished.completion, Completion::Answered);
    assert_eq!(
        finished.reply, streamed,
        "Generate answered, as it always has"
    );
}

/// A program selected for a request that carries no work stops at the
/// lookup, and nothing is delegated.
///
/// This is the structural half of the false-positive bound: the wrong
/// program on an ordinary turn costs a reply, not six subprocesses,
/// because the fan-out runs over the work the request lists and an
/// ordinary turn lists none.
#[tokio::test]
async fn a_program_chosen_for_a_request_with_no_work_delegates_nothing() {
    if !boundary() {
        return;
    }
    // Both shapes a wrong selection takes: the program that looks the work
    // up first, and the one that hands it straight over. Hosted Jev picked
    // the second one for two of thirty-one real turns, so this is the case
    // the measurement found rather than one imagined for a test.
    for (program, step) in [
        ("delegate-fan-out", "select"),
        ("answer-question", "answer"),
    ] {
        let machine = machine();
        let root = machine.path();
        let mut agent = agent(root, program).await;

        let finished = turn::run(
            &mut agent,
            "What does ROUNDS_MAX do?".to_string(),
            &mut |_| {},
        )
        .await
        .expect("the turn finished");

        let run = finished.program.expect("the door named a program");
        let stopped = run.stopped.expect("there was no work to do");
        assert_eq!(stopped.step, step, "{program} stopped at the wrong step");
        assert_eq!(stopped.code, "no_tasks");
        assert!(run.delegations.is_empty(), "nothing was spawned");
        assert_eq!(
            finished.completion,
            Completion::Declined,
            "a run that stopped declined, and the exit code says so"
        );
    }
}

/// The live check: the repository's own program, its own capability
/// manifest, its own question sets, a real decision door, and the Devin
/// CLI on this computer.
///
/// Ignored by default: it needs the executor, a door, and about two
/// minutes. The executor refuses a checkout nobody has trusted, and the
/// worktrees the `fan_out` step's `isolation` bound calls for are made
/// **under** the repository for exactly that reason, so run it from a
/// checkout the executor trusts:
///
/// ```text
/// cargo +1.97.1 test -p coder --test program_run -- --ignored --nocapture
/// ```
#[tokio::test]
#[ignore = "needs an executor, a decision door, and a workspace the executor trusts"]
async fn the_first_program_runs_live() {
    let root = match std::env::var_os("CODER_DELEGATE_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => checkout().canonicalize().expect("the repository root"),
    };
    let runtime = Runtime::open(Some(&root), &root);
    let found = runtime
        .survey()
        .capability("devin-local")
        .expect("the repository declares devin-local");
    println!("{}", found.message());

    let traces = tempfile::tempdir().unwrap();
    let mut recorder =
        Recorder::open(traces.path(), "live", "door", &root.display().to_string()).unwrap();
    let path = recorder.path().to_path_buf();
    let mut inputs = inputs();
    inputs.executor = "devin-local".to_string();
    inputs.tasks = QUESTIONS
        .iter()
        .map(|(prompt, reads, answer)| {
            Task::reading(
                &format!("{prompt} Answer with the value and nothing else."),
                reads,
            )
            .expecting(answer)
        })
        .collect();
    recorder.user(&inputs.request);
    runtime.survey().record(&mut recorder, None);
    let run = runtime
        .apply(&inputs, &Grant::all(), Some(&mut recorder))
        .await;
    drop(recorder);

    for step in &run.steps {
        println!("{:>12}  {}", step.name, step.output);
    }
    for delegation in &run.delegations {
        println!("{:>12}  {}", delegation.task.head(40), delegation.line());
    }
    println!("{}", run.summary());
    println!("trace {}", path.display());

    assert_eq!(run.stopped, None, "{:?}", run.stopped);
    assert_eq!(run.program.as_deref(), Some("delegate-fan-out"));
    let observed = coderbench::observe(&path).expect("the grader reads the trace");
    let task = coderbench::Task::load(
        &coderbench::tasks_dir()
            .join("devin-fan-out-six")
            .join("task.json"),
    )
    .unwrap();
    let judgment = task.judge(&observed);
    println!("{}: {:?}", judgment.verdict, judgment.faults);
    assert!(
        !judgment.faults.iter().any(|fault| matches!(
            fault,
            coderbench::Fault::DecisionMissing { .. } | coderbench::Fault::CheckMissing { .. }
        )),
        "{:?}",
        judgment.faults
    );
}

/// A writing item cannot widen a read-only program grant after source lookup.
#[tokio::test]
async fn writing_work_list_data_cannot_grant_write_authority() {
    if !boundary_supported() {
        return;
    }
    let machine = machine();
    let root = machine.path();
    let mut inputs = inputs();
    inputs.tasks[0].writes = true;
    let grant = Grant::selected(
        Some("delegate-fan-out"),
        Some("reads,delegation,network,subprocesses,spend"),
    );
    let run = runtime(root)
        .await
        .run(&fan_out(root), &inputs, &grant, None)
        .await;
    assert!(run.delegations.is_empty());
    let refused = run.stopped.unwrap();
    assert_eq!(refused.code, "unauthorized");
    assert!(refused.reason.contains("writes"));
}

/// Even a confident selection from ordinary prose cannot grant an executor.
#[tokio::test]
async fn a_wrong_selection_has_no_authority_from_bullet_prose() {
    if !boundary_supported() {
        return;
    }
    let machine = machine();
    let mut agent = agent(machine.path(), "delegate-fan-out")
        .await
        .with_program_grant(Some("none"));
    let mut programs = Vec::new();
    let finished = turn::run(
        &mut agent,
        "Explain this list:\n- item one\n- item two".into(),
        &mut |event| {
            if let turn::Event::Program(slug) = event {
                programs.push(slug);
            }
        },
    )
    .await
    .unwrap();
    assert!(finished.program.is_none(), "an ungranted program never runs");
    assert!(programs.is_empty(), "no program execution was announced");
    assert!(finished.reply.contains("stub door"), "{}", finished.reply);
}
