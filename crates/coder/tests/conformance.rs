//! The consumer conformance suite: the failure modes the flagship
//! acceptance names, plus the reconciliation and profile parity the
//! release has to hold to.
//!
//! Every scenario runs the same consumer path the terminal and
//! `coder --print` run: a [`Runtime`] over a scratch checkout carrying
//! the repository's own `programs/`, `questions/`, and `sources/`,
//! asking a stub door that records each request and answers what the
//! scenario needs. What is stubbed is the door and the executor — the
//! selection, admission, grant, dispatch, runstate, and trace code is
//! the code a live run uses, and the trace a run leaves reads back with
//! the same identities and costs a hosted door would leave.

use std::io::Write as _;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use coder::Task;
use coder::capability::{self, Trust};
use coder::delegate::boundary_supported;
use coder::profiles::Profiles;
use coder::program::Program;
use coder::program_authority::Grant;
use coder::questions;
use coder::runstate::{Claim, Mark, State, Store};
use coder::runtime::{Budget, Host, Inputs, Runtime};
use coder::sites::{Problem, Sites};
use coder::survey::Survey;
use coder::trace::Recorder;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// The repository this crate lives in, whose `programs/`, `questions/`,
/// and `sources/` the suite runs from.
fn checkout() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Whether this host can put a delegation inside an enforced filesystem
/// boundary. A case that fans out says so and returns rather than
/// failing for a tool the machine lacks.
fn boundary() -> bool {
    if boundary_supported() {
        return true;
    }
    eprintln!("skipping: this host has no filesystem boundary backend (bwrap on Linux)");
    false
}

/// A scratch checkout: the repository's programs, questions, and
/// sources, a stub-executor capability, a pinned work list under
/// `.coder/`, and one git commit — `isolation: worktree` is a bound this
/// host provides by making a worktree, and the commit is what it makes
/// one from.
fn machine() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a scratch directory");
    let root = dir.path();
    for named in ["programs", "questions", "sources", "capabilities", ".coder"] {
        std::fs::create_dir_all(root.join(named)).unwrap();
    }
    for named in ["programs", "questions", "sources"] {
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
    std::fs::write(root.join(".coder").join("work-list.json"), work_list()).unwrap();
    std::fs::write(root.join("a.rs"), "// a file\n").unwrap();
    std::fs::write(root.join("b.rs"), "// another file\n").unwrap();

    let run = |arguments: &[&str]| {
        let done = std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args(arguments)
            .output()
            .expect("version control runs");
        assert!(done.status.success(), "{arguments:?}");
    };
    run(&["init"]);
    run(&["add", "-A"]);
    run(&[
        "-c",
        "user.email=conformance@example.com",
        "-c",
        "user.name=Conformance",
        "commit",
        "-m",
        "pinned",
    ]);
    dir
}

/// The pinned caller-owned case: a work list carrying one write pair
/// that collides on a path, one read-only item, and one item the list
/// itself blocks. The collision is a mechanical fact — the lookup's
/// `collisions` and `dropped` say what happened without any judgment —
/// which is what makes the deterministic baseline below comparable.
fn work_list() -> &'static str {
    r#"{"v":1,"work":[
        {"id":"01-write-shared","prompt":"Fix shared.rs and report.","writes":true,"touches":["shared.rs"],"expects":"done-one"},
        {"id":"02-write-shared","prompt":"Also fix shared.rs and report.","writes":true,"touches":["shared.rs"],"expects":"done-two"},
        {"id":"03-read-a","prompt":"Read a.rs and report what it says.","reads":"a.rs","expects":"a file"},
        {"id":"04-read-b","prompt":"Read b.rs and report what it says.","reads":"b.rs","expects":"another file"},
        {"id":"05-blocked","prompt":"Never dispatched.","blocked":"the list itself holds this item back"}
    ]}"#
}

/// The stub executor's capability manifest: a shell script that answers
/// `Final answer:` lines by matching the prompt it is handed — the
/// task's prompt is the argv's last word — declared with the isolation
/// and concurrency the programs' bounds need.
fn manifest(root: &Path) -> String {
    let script = root.join("stub.sh");
    let body = r#"#!/bin/sh
case "$1" in
  *"Fix shared.rs"*) printf 'work recorded\nFinal answer: done-one\n'; exit 0 ;;
  *"Also fix shared.rs"*) printf 'work recorded\nFinal answer: done-two\n'; exit 0 ;;
  *"Read a.rs"*) printf 'looked\nFinal answer: a file\n'; exit 0 ;;
  *"Read b.rs"*) printf 'looked\nFinal answer: another file\n'; exit 0 ;;
  *) printf 'Final answer: answered\n'; exit 0 ;;
esac
"#;
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
            "summary": "Answers one work item, for the conformance suite.",
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

/// One request as the door recorded it.
#[derive(Clone, Debug)]
struct Recorded {
    /// The `Authorization` header the request carried, when it carried
    /// one — the field a credential-free profile must never send.
    authorization: Option<String>,
    /// The `x-typesafe-retry-count` header: which dispatch of one
    /// logical call this request was, absent on the first.
    attempt: Option<String>,
    /// The parsed request body.
    body: Value,
}

/// What a door handler answers.
struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    body: Value,
}

impl Reply {
    /// A 200 carrying a System One body.
    fn answered(body: Value) -> Self {
        Reply {
            status: 200,
            headers: Vec::new(),
            body,
        }
    }

    /// A typed refusal, as the gateway's refusal envelope spells it.
    fn refused(status: u16, code: &str, message: &str) -> Self {
        Reply {
            status,
            headers: Vec::new(),
            body: json!({"error": {"code": code, "message": message}}),
        }
    }

    /// Attach a header to whatever this reply is.
    fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_string(), value.to_string()));
        self
    }
}

/// A stub decision door: records every request and answers what its
/// handler says, on a loopback port of its own.
struct Door {
    url: String,
    recorded: Arc<Mutex<Vec<Recorded>>>,
}

impl Door {
    /// Every request this door saw, in order.
    fn requests(&self) -> Vec<Recorded> {
        self.recorded.lock().unwrap().clone()
    }
}

/// Starts a door that answers each request through `handler`, recording
/// the credential and attempt headers and the body of each.
fn door(handler: impl Fn(&Value) -> Reply + Send + Sync + 'static) -> Door {
    let recorded = Arc::new(Mutex::new(Vec::<Recorded>::new()));
    let kept = recorded.clone();
    let (ready, ready_rx) = std::sync::mpsc::channel::<SocketAddr>();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("the door's runtime");
        runtime.block_on(async move {
            let listener = TcpListener::bind("127.0.0.1:0").await.expect("a port");
            ready.send(listener.local_addr().unwrap()).unwrap();
            while let Ok((mut socket, _)) = listener.accept().await {
                let handler_ref = &handler;
                let kept = kept.clone();
                // Sequential request handling keeps the recorded order
                // the dispatch order — a conformance door has no
                // concurrency to demonstrate.
                let mut arrived = Vec::new();
                let mut buffer = [0u8; 16384];
                let reply = loop {
                    let Ok(read) = socket.read(&mut buffer).await else {
                        return;
                    };
                    if read == 0 {
                        return;
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
                    let header_value = |wanted: &str| {
                        head.lines().find_map(|line| {
                            line.split_once(':').and_then(|(name, value)| {
                                name.trim()
                                    .eq_ignore_ascii_case(wanted)
                                    .then(|| value.trim().to_string())
                            })
                        })
                    };
                    let asked: Value = serde_json::from_str(&body[..length]).unwrap_or(Value::Null);
                    kept.lock().unwrap().push(Recorded {
                        authorization: header_value("authorization"),
                        attempt: header_value("x-typesafe-retry-count"),
                        body: asked.clone(),
                    });
                    break handler_ref(&asked);
                };
                let body = reply.body.to_string();
                let extra: String = reply
                    .headers
                    .iter()
                    .map(|(name, value)| format!("{name}: {value}\r\n"))
                    .collect();
                let _ = socket
                    .write_all(
                        format!(
                            "HTTP/1.1 {}\r\ncontent-type: application/json\r\n\
                             content-length: {}\r\n{}connection: close\r\n\r\n{body}",
                            reply.status,
                            body.len(),
                            extra
                        )
                        .as_bytes(),
                    )
                    .await;
                let _ = socket.flush().await;
            }
        });
    });
    let address = ready_rx.recv().expect("the door bound a port");
    Door {
        url: format!("http://{address}"),
        recorded,
    }
}

/// The typed answer one asked question gets, keyed off the type the
/// request declared for it.
fn answer_for(id: &str, question: &Value) -> Value {
    match question.get("type").and_then(Value::as_str) {
        Some("choice") => {
            // The wire's `criteria` is an object keyed by option name,
            // or an array of named or bare-string options.
            let criteria = question.get("criteria");
            let options: Vec<String> = match criteria {
                Some(Value::Object(named)) => named.keys().cloned().collect(),
                Some(Value::Array(list)) => list
                    .iter()
                    .filter_map(|option| {
                        option
                            .get("name")
                            .or_else(|| option.get("id"))
                            .and_then(Value::as_str)
                            .or_else(|| option.as_str())
                            .map(str::to_string)
                    })
                    .collect(),
                _ => Vec::new(),
            };
            let choice = if id == "program" {
                "none".to_string()
            } else {
                options
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "none".to_string())
            };
            // Probabilities are a distribution: the mass must sum to
            // one, so the unchosen options take the remainder between
            // them.
            let rest = match options.len() {
                0 | 1 => 0.0,
                n => 0.1 / (n - 1) as f64,
            };
            let probabilities = options
                .iter()
                .map(|option| {
                    (
                        option.clone(),
                        if *option == choice {
                            json!(0.9)
                        } else {
                            json!(rest)
                        },
                    )
                })
                .collect::<serde_json::Map<String, Value>>();
            json!({
                "type": "choice",
                "choice": choice,
                "confidence": 0.9,
                "probabilities": probabilities,
            })
        }
        Some("score") => json!({
            "type": "score",
            "score": 1.0,
            "confidence": 0.9,
            "legend": {}
        }),
        _ => json!({"type": "noul", "noul": 0.93}),
    }
}

/// A full System One response answering every question the request
/// asked, as `model`, with usage a token ledger can price.
fn answered(asked: &Value, model: &str, answer: impl Fn(&str, &Value) -> Value) -> Value {
    let mut answers = serde_json::Map::new();
    if let Some(questions) = asked.get("questions").and_then(Value::as_object) {
        for (id, question) in questions {
            answers.insert(id.clone(), answer(id, question));
        }
    }
    json!({
        "model": model,
        "answers": answers,
        "usage": {"input_tokens": 120, "output_tokens": 8}
    })
}

/// A door that answers every question the way a passing run needs:
/// `none` for the program choice, clearing probabilities elsewhere.
fn passing_door() -> Door {
    door(|asked| Reply::answered(answered(asked, "kev-latest", answer_for)))
}

/// A `jev` client against the door, with retries off so each test sees
/// exactly the dispatches it means to.
fn client(url: &str) -> jev::Client {
    client_with(url, 0)
}

/// A `jev` client with an explicit retry bound, for the cases that
/// measure the retry.
fn client_with(url: &str, retries: u32) -> jev::Client {
    jev::Client::new(
        jev::Config::new()
            .api_key("ts-test-key")
            .base_url(url)
            .default_model("kev-latest")
            .retry(jev::RetryPolicy {
                max_retries: retries,
                backoff_initial: Duration::from_millis(1),
                backoff_max: Duration::from_millis(5),
                backoff_jitter: 0.0,
                ..jev::RetryPolicy::default()
            }),
    )
    .expect("the stub door builds a client")
}

/// A runtime over the scratch machine, asking the door `url` names.
fn runtime_with_client(root: &Path, client: jev::Client) -> Runtime {
    Runtime::over(
        Survey::read_with(Some(root), root, &Trust::everything()),
        questions::Registry::open(&[root.join("questions")]),
        Host::with_repository(),
    )
    .asking(Some(client))
}

fn runtime(root: &Path, url: &str) -> Runtime {
    runtime_with_client(root, client(url))
}

/// `delegate-fan-out`, the repository's own read-only workflow.
fn fan_out(root: &Path) -> Program {
    Program::load(&root.join("programs").join("delegate-fan-out.json"))
        .expect("the repository's own program")
}

/// `burn-down`, the repository's own writing-backlog workflow.
fn burn_down(root: &Path) -> Program {
    Program::load(&root.join("programs").join("burn-down.json"))
        .expect("the repository's own program")
}

/// The request and work a `delegate-fan-out` run is given.
fn inputs() -> Inputs {
    Inputs {
        request: "Read the listed files and answer each question.".to_string(),
        tasks: vec![
            Task::reading("What does a.rs contain?", "a.rs").expecting("a file"),
            Task::reading("What does b.rs contain?", "b.rs").expecting("another file"),
        ],
        executor: "stub-local".to_string(),
    }
}

/// The request a `burn-down` run is given: the work is the list, not
/// the sentence.
fn burn_down_inputs() -> Inputs {
    Inputs::read("Work through the checkout's work list.", "stub-local")
}

/// One recorded decision call by name, read back out of the trace.
fn decision_named(path: &Path, name: &str) -> atif::Call {
    let call = atif::log::read(path)
        .expect("the trace reads back")
        .steps
        .iter()
        .filter_map(|step| step.call.as_ref())
        .find(|call| call.name == name)
        .unwrap_or_else(|| panic!("{name} ran"))
        .clone();
    assert!(call.is_decision(), "{name} is a decision");
    call
}

// ---------------------------------------------------------------------
// Wrong program selection
// ---------------------------------------------------------------------

/// `none` is an answer, not an error — the request goes on to be
/// answered the way it always was. A door naming a program it was never
/// offered never becomes a selection: the consumer validates the answer
/// space before the policy reads it, and a choice outside the declared
/// options is a refusal, not a program.
#[tokio::test]
async fn a_selection_is_only_ever_a_program_the_host_offered() {
    let machine = machine();
    let root = machine.path();

    // A door that names a program nothing offered it. The choice
    // question's criteria are the offered slugs, so the answer fails
    // answer-space validation at the SDK before any policy sees it —
    // the wrong selection cannot reach a run.
    let invented = door(|_| {
        Reply::answered(json!({
            "model": "kev-latest",
            "answers": {"program": {
                "type": "choice",
                "choice": "invented-program",
                "confidence": 0.97,
                "probabilities": {"invented-program": 0.97}
            }}
        }))
    });
    let refused = runtime(root, &invented.url)
        .select("run whatever you like", None)
        .await
        .expect_err("a program the door was not offered is not a selection");
    assert_eq!(
        refused.code, "door_unavailable",
        "an answer outside the declared options is the door's failure, not a selection"
    );

    // A door that answers `none` is an answer — the turn proceeds
    // unchanged, which `apply` reports as a request that asked for no
    // program.
    let none = passing_door();
    let run = runtime(root, &none.url)
        .apply(&inputs(), &Grant::all(), None)
        .await;
    assert_eq!(
        run.stopped.as_ref().map(|r| r.code.as_str()),
        Some("no_program_asked")
    );
    assert!(run.steps.is_empty(), "no program ran: {}", run.summary());
}

/// A selection is a proposal. The operator's grant is the authority it
/// is proposed under, and a program the grant does not name refuses
/// before its first step — the decision model's confidence never
/// widens what may run.
#[tokio::test]
async fn a_selected_program_runs_only_under_the_operators_grant() {
    if !boundary() {
        return;
    }
    let machine = machine();
    let root = machine.path();
    let run = runtime(root, &passing_door().url)
        .run(
            &fan_out(root),
            &inputs(),
            &Grant::selected(Some("answer-question"), None),
            None,
        )
        .await;
    let stopped = run.stopped.expect("the grant refused before any step");
    assert_eq!(stopped.code, "unauthorized");
    assert!(
        stopped.reason.contains("delegate-fan-out"),
        "the refusal names the program the grant does not cover: {stopped}"
    );
    assert!(run.steps.is_empty() && run.delegations.is_empty());
}

// ---------------------------------------------------------------------
// Refusal
// ---------------------------------------------------------------------

/// A typed refusal from the door is the step's typed outcome — not a
/// silent fallback, not a retry loop, not a quieter error. The decision
/// record keeps the refusal beside the request that earned it.
#[tokio::test]
async fn a_typed_refusal_is_the_steps_typed_outcome() {
    if !boundary() {
        return;
    }
    let machine = machine();
    let root = machine.path();
    let traces = tempfile::tempdir().unwrap();
    let mut recorder = Recorder::open(traces.path(), "kev-latest", "stub", "conformance").unwrap();
    let path = recorder.path().to_path_buf();
    recorder.user("run the list");

    let refusing = door(|_| Reply::refused(403, "door_not_bound", "this key reaches no such door"));
    let run = runtime(root, &refusing.url)
        .run(
            &fan_out(root),
            &inputs(),
            &Grant::all(),
            Some(&mut recorder),
        )
        .await;
    drop(recorder);

    let stopped = run
        .stopped
        .as_ref()
        .expect("the door's refusal ended the run");
    assert_eq!(stopped.step, "independence");
    assert_eq!(
        stopped.code, "door_unauthorized",
        "a 403 is an authorization signal, not transport failure"
    );
    // The work up to the refusal is still recorded: the lookup ran, the
    // refused step did not, and nothing after it pretended to.
    assert_eq!(run.step_names(), ["select"]);
    assert!(run.delegations.is_empty());

    let decision = decision_named(&path, "independence");
    assert_eq!(
        decision.extra["attempts"][0]["outcome"],
        json!("door_unauthorized")
    );
    assert!(
        decision.extra["error"]
            .as_str()
            .is_some_and(|e| e.contains("403")),
        "the decision keeps the refusal it ended on: {}",
        decision.extra["error"]
    );
}

// ---------------------------------------------------------------------
// Quota exhaustion
// ---------------------------------------------------------------------

/// `quota_exhausted` is a budget answer, not a dead door: the SDK's own
/// bounded retry honors the server's `Retry-After`, and when the bound
/// is spent the refusal stands — typed, named, and never silently
/// retried forever.
#[tokio::test]
async fn quota_exhaustion_is_bounded_then_terminal() {
    if !boundary() {
        return;
    }
    let machine = machine();
    let root = machine.path();
    let exhausted = door(|_| {
        Reply::refused(429, "quota_exhausted", "the day's quota is spent")
            .header("retry-after", "0")
    });
    // Two retries is the SDK default's own bound: three dispatches
    // total, each a recorded attempt of one logical call.
    let run = runtime_with_client(root, client_with(&exhausted.url, 2))
        .run(&fan_out(root), &inputs(), &Grant::all(), None)
        .await;

    let stopped = run.stopped.expect("quota refusal ended the run");
    assert_eq!(stopped.step, "independence");
    assert_eq!(stopped.code, "door_rate_limited");

    let requests = exhausted.requests();
    assert_eq!(
        requests.len(),
        3,
        "the retry bound held: first try plus two"
    );
    let attempts: Vec<Option<String>> = requests.iter().map(|r| r.attempt.clone()).collect();
    assert_eq!(
        attempts,
        vec![None, Some("1".to_string()), Some("2".to_string())],
        "each retry kept the request's identity and bumped its attempt count"
    );
    // Every dispatch asked the same state — a retried request is one
    // logical call, not three new spends.
    assert!(
        requests
            .iter()
            .all(|r| r.body.get("state") == requests[0].body.get("state")),
        "a retry asks the same state, never a new one"
    );
}

// ---------------------------------------------------------------------
// Artifact mismatch
// ---------------------------------------------------------------------

/// An answer reporting a model the function never admitted is the
/// refusal `unbound_model` — a served-artifact claim the policy holds
/// against the set's `models`, not a read a caller treats as an answer.
#[tokio::test]
async fn an_answer_from_an_unadmitted_artifact_is_the_refusal() {
    if !boundary() {
        return;
    }
    let machine = machine();
    let root = machine.path();

    // Pin the independence set to one admitted artifact, on the scratch
    // copy — the repository's own set admits any model, and the policy
    // that would catch a smuggled one has to exist to be tested. The
    // request itself keeps asking for the admitted model, so what the
    // policy catches is the answer's claim.
    let set_path = root.join("questions").join("independence-v2.json");
    let mut set: Value =
        serde_json::from_str(&std::fs::read_to_string(&set_path).unwrap()).unwrap();
    set["policy"] = json!({"v": 1, "models": ["kev-latest"]});
    std::fs::write(&set_path, set.to_string()).unwrap();

    let smuggled = door(|asked| Reply::answered(answered(asked, "smuggled-model", answer_for)));
    let run = runtime(root, &smuggled.url)
        .run(&fan_out(root), &inputs(), &Grant::all(), None)
        .await;
    let stopped = run.stopped.expect("the mismatched answer refused");
    assert_eq!(stopped.step, "independence");
    assert_eq!(stopped.code, "unbound_model");
    assert!(
        stopped.reason.contains("smuggled-model"),
        "the refusal names the artifact that answered: {stopped}"
    );

    // The admitted artifact answers the same question cleanly.
    let admitted = door(|asked| Reply::answered(answered(asked, "kev-latest", answer_for)));
    let run = runtime(root, &admitted.url)
        .run(&fan_out(root), &inputs(), &Grant::all(), None)
        .await;
    assert_eq!(run.stopped, None, "{:?}", run.stopped);
    assert_eq!(run.answered(), 2);
}

// ---------------------------------------------------------------------
// Cancellation and restart
// ---------------------------------------------------------------------

/// A deadline spent mid-dispatch is the caller's end, recorded as
/// `cancelled` — never the step's refusal and never `unknown`, which is
/// the mark only a crash leaves. The run's record settles `cancelled`,
/// so a resumer reads a deliberate end rather than ambiguous work.
#[tokio::test]
async fn a_cancelled_run_marks_the_callers_end_not_the_works() {
    if !boundary() {
        return;
    }
    let machine = machine();
    let root = machine.path();
    let runstate = tempfile::tempdir().unwrap();

    // A door that takes four seconds; the run's budget gives it 200ms.
    let slow = door(|asked| {
        std::thread::sleep(Duration::from_secs(4));
        Reply::answered(answered(asked, "kev-latest", answer_for))
    });
    let run = runtime(root, &slow.url)
        .with_budget(Budget {
            deadline: Some(Duration::from_millis(200)),
            ..Budget::default()
        })
        .with_runstate(runstate.path())
        .run(&burn_down(root), &burn_down_inputs(), &Grant::all(), None)
        .await;

    let stopped = run.stopped.as_ref().expect("the deadline ended the run");
    assert_eq!(stopped.code, "budget_exceeded", "{stopped}");
    // The lookup answered inside its time; the dispatched decide did
    // not come back, and what did not come back is not a refusal the
    // work gave.
    assert_eq!(run.step_names(), ["select"]);

    let mut store = Store::open(runstate.path()).expect("the runstate store opens");
    let recovered = store.recover().expect("recovery reads");
    assert!(
        recovered.is_empty(),
        "a settled record is not recovered work: {recovered:?}"
    );
}

/// A crash is the honest case `cancelled` exists to be told apart from:
/// the record the process never settled recovers `unknown`, and what
/// may run again is a ruling the resumer reads, not a replay the store
/// performs. A step whose declared effects could have produced a
/// non-idempotent change rules `NeedsDecision`; a resuming grant that
/// does not cover them rules `OutsideAuthority`; a pin that resolves to
/// nothing is unrecorded, and unrecorded is never replayable.
#[tokio::test]
async fn a_crash_leaves_unknown_and_the_restart_is_a_decision() {
    if !boundary() {
        return;
    }
    let machine = machine();
    let root = machine.path();
    let runstate = tempfile::tempdir().unwrap();
    let program = burn_down(root);

    // The crash: claimed, dispatched, never settled — the same records
    // the runtime itself writes, driven by hand to the point the
    // process died.
    {
        let mut store = Store::open(runstate.path()).unwrap();
        store
            .claim(&Claim {
                run: "run-burn-down-crash",
                base: "base-abc123",
                program: &coder::child::digest(&program),
                questions: &[],
                sources: &[],
                owner: 0,
            })
            .unwrap();
        store
            .advance("run-burn-down-crash", Mark::run(State::Dispatched))
            .unwrap();
        store
            .advance(
                "run-burn-down-crash",
                Mark::step("select", State::Answered).result("the list"),
            )
            .unwrap();
        store
            .advance(
                "run-burn-down-crash",
                Mark::step("fan_out", State::Dispatched),
            )
            .unwrap();
        // The process dies here.
    }

    let mut store = Store::open(runstate.path()).unwrap();
    let recovered = store.recover().unwrap();
    let run = recovered
        .iter()
        .find(|run| run.run == "run-burn-down-crash")
        .expect("the crashed run recovered");
    assert_eq!(run.state, State::Unknown);
    // Every non-terminal record marks unknown — an answered step in a
    // run that never settled is evidence, not an end anyone integrated.
    for step in &run.steps {
        assert_eq!(
            step.state,
            State::Unknown,
            "{} is recovery's mark now",
            step.step
        );
    }

    // Reconciliation rules each unknown record against the effects the
    // pinned program declares and the resuming session's ceiling. The
    // read-only lookup replays on its own idempotence; the dispatched
    // delegation could have written or spent, and that is the
    // operator's call.
    let runtime = Runtime::over(
        Survey::read_with(Some(root), root, &Trust::everything()),
        questions::Registry::open(&[root.join("questions")]),
        Host::with_repository(),
    );
    let rulings = runtime.rulings(run, &Grant::all(), |_, _| {
        coder::reattach::Observation::Unsupported
    });
    let ruling_of = |rulings: &coder::runtime::Rulings, subject: &str| {
        rulings
            .reconcile
            .iter()
            .find(|ruling| ruling.subject == subject)
            .unwrap_or_else(|| panic!("{subject} was ruled"))
            .ruling
    };
    assert_eq!(
        ruling_of(&rulings, "step:select"),
        coder::reconcile::Reconciliation::Replayable,
        "reads are their own idempotence"
    );
    assert_eq!(
        ruling_of(&rulings, "step:fan_out"),
        coder::reconcile::Reconciliation::NeedsDecision,
        "a dispatched delegate could have written or spent — replay is the operator's call"
    );
    assert_eq!(
        ruling_of(&rulings, "run"),
        coder::reconcile::Reconciliation::NeedsDecision,
        "the run's union includes work the record cannot rule out"
    );

    // A resuming session whose ceiling does not cover the declared
    // effects rules OutsideAuthority — a crash never widens a grant.
    // The reads-only lookup still replays: narrowing a ceiling does not
    // invent ambiguity.
    let narrow = Grant::selected(Some("burn-down"), Some("reads"));
    let rulings = runtime.rulings(run, &narrow, |_, _| {
        coder::reattach::Observation::Unsupported
    });
    assert_eq!(
        ruling_of(&rulings, "step:fan_out"),
        coder::reconcile::Reconciliation::OutsideAuthority,
        "replay would exceed the resuming session's ceiling"
    );
    assert_eq!(
        ruling_of(&rulings, "step:select"),
        coder::reconcile::Reconciliation::Replayable,
        "reads stay replayable under a narrowed ceiling"
    );

    // A run whose pin resolves to nothing in this host's registry is
    // unrecorded — and unrecorded is never replayable.
    {
        let mut store = Store::open(runstate.path()).unwrap();
        store
            .claim(&Claim {
                run: "run-unpinned",
                base: "base-abc123",
                program: "sha256:0000000000000000000000000000000000000000000000000000000000000000",
                questions: &[],
                sources: &[],
                owner: 0,
            })
            .unwrap();
        store
            .advance("run-unpinned", Mark::step("anything", State::Dispatched))
            .unwrap();
    }
    let mut store = Store::open(runstate.path()).unwrap();
    let recovered = store.recover().unwrap();
    let unpinned = recovered
        .iter()
        .find(|run| run.run == "run-unpinned")
        .expect("the unpinned run recovered");
    let rulings = runtime.rulings(unpinned, &Grant::all(), |_, _| {
        coder::reattach::Observation::Unsupported
    });
    assert!(
        rulings.reconcile.iter().all(|ruling| ruling.ruling
            == coder::reconcile::Reconciliation::NeedsDecision
            && ruling.effects.is_none()),
        "a pin that resolves to nothing declares nothing: {:#?}",
        rulings.reconcile
    );
}

// ---------------------------------------------------------------------
// Partial work
// ---------------------------------------------------------------------

/// A run that stops mid-program reports exactly what ran and what did
/// not: the steps that answered stay answered, the refusal names where
/// it stopped, and the coverage a reader gets is against the program's
/// own declaration — never implied completeness.
#[tokio::test]
async fn partial_work_reports_its_coverage_against_the_program() {
    if !boundary() {
        return;
    }
    let machine = machine();
    let root = machine.path();
    let program = fan_out(root);

    // Independence clears, the executor answers, and the acceptance
    // call comes back a 5xx — the work is done and the verdict is not.
    // The decide calls are the first and second requests this door
    // sees: pass the first, fail the second.
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = calls.clone();
    let failing_accept = door(move |asked| {
        let n = counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if n == 0 {
            Reply::answered(answered(asked, "kev-latest", answer_for))
        } else {
            Reply::refused(503, "door_unavailable", "the door went away")
        }
    });
    let run = runtime(root, &failing_accept.url)
        .run(&program, &inputs(), &Grant::all(), None)
        .await;

    let stopped = run.stopped.as_ref().expect("the run stopped at acceptance");
    assert_eq!(stopped.step, "accept");
    assert_eq!(stopped.code, "door_unavailable");
    assert!(!run.finished());
    // Four of the program's five steps ran; the coverage is the
    // program's own list, and the delegations' answers remain the
    // evidence they are.
    assert_eq!(
        run.step_names(),
        ["select", "independence", "admit", "fan_out"]
    );
    assert_eq!(program.steps.len(), 5);
    assert_eq!(run.delegations.len(), 2);
    assert_eq!(run.answered(), 2, "the partial answers are still answers");
}

// ---------------------------------------------------------------------
// Stale inputs
// ---------------------------------------------------------------------

/// A question set's wording is the function: a file that drifts from
/// the digest a site was written against is drift the inventory
/// reports, not a different question asked quietly under the same name.
#[test]
fn a_reworded_question_set_is_drift_the_inventory_reports() {
    let machine = machine();
    let root = machine.path();

    // The repository's own registry has one known entry: v1 of the
    // independence wording is retained for comparability and no site
    // binds it — an honest `UnboundSet`, not drift.
    let inventory = Sites::inventory(root);
    assert_eq!(
        inventory.problems(),
        vec![Problem::UnboundSet {
            set: "openagents.independence.v1".to_string()
        }],
        "the registry's standing problems: {:?}",
        inventory.problems()
    );

    // Reword the selection set — the digest is of the wording, so the
    // edit lands inside `questions`.
    let set_path = root.join("questions").join("program.json");
    let mut set: Value =
        serde_json::from_str(&std::fs::read_to_string(&set_path).unwrap()).unwrap();
    set["questions"]["program"]["instructions"] = json!("A differently worded selection question.");
    std::fs::write(&set_path, set.to_string()).unwrap();

    let problems = Sites::inventory(root).problems();
    let drift = problems.iter().find_map(|problem| match problem {
        Problem::DigestDrift { site, set, .. } => Some((site.clone(), set.clone())),
        _ => None,
    });
    assert!(
        drift.is_some(),
        "a reworded set reports digest drift, got: {problems:?}"
    );
}

// ---------------------------------------------------------------------
// Missing verification
// ---------------------------------------------------------------------

/// A step that demands independent host evidence the host never
/// installed refuses at admission — the program that would run its
/// acceptance unchecked is a different program, and this host will not
/// run it. An answer that cannot be scored refuses the same way: a
/// `requires_scorable_answer` step whose answer carries no probability
/// from a named model is `unscorable_answer`, not a pass.
#[tokio::test]
async fn work_that_cannot_be_verified_never_passes() {
    if !boundary() {
        return;
    }
    let machine = machine();
    let root = machine.path();

    // A program whose check step asks for a gated verdict the host has
    // no plan for: admission refuses before the first step dispatches.
    let mut gated = fan_out(root);
    let admit = gated
        .steps
        .iter_mut()
        .find(|step| step.name == "admit")
        .expect("the program's admission step");
    admit
        .bounds
        .insert("refuse_on".to_string(), json!("gate_not_met"));
    let host = runtime(root, &passing_door().url);
    let refused = host
        .admit(&gated)
        .expect_err("a gated check with no installed plan cannot run");
    assert_eq!(refused.code, "check_unavailable");
    assert!(
        refused.reason.contains("verification plan"),
        "the refusal says what is missing: {refused}"
    );

    // A scorable answer that arrives with no model behind it is the
    // refusal, not the read.
    let mut scorable = fan_out(root);
    let independence = scorable
        .steps
        .iter_mut()
        .find(|step| step.name == "independence")
        .expect("the program's independence step");
    independence
        .bounds
        .insert("requires_scorable_answer".to_string(), json!(true));
    let unnamed = door(|asked| Reply::answered(answered(asked, "", answer_for)));
    let run = runtime(root, &unnamed.url)
        .run(&scorable, &inputs(), &Grant::all(), None)
        .await;
    let stopped = run.stopped.expect("the unscorable answer refused");
    assert_eq!(stopped.step, "independence");
    assert_eq!(stopped.code, "unscorable_answer");
}

// ---------------------------------------------------------------------
// Reconciliation
// ---------------------------------------------------------------------

/// Every program step and every decision attempt reconciles: the
/// runstate record pins the program, sets, and sources by digest; the
/// trace's decision calls carry the set identity, the attempts with
/// their models and outcomes, and the usage the token ledger counted.
/// A run that says it spent reports tokens the trace also shows, and a
/// dispatch no usage arrived for is counted apart rather than for free.
#[tokio::test]
async fn every_attempt_reconciles_with_identities_outcomes_and_costs() {
    if !boundary() {
        return;
    }
    let machine = machine();
    let root = machine.path();
    let traces = tempfile::tempdir().unwrap();
    let runstate = tempfile::tempdir().unwrap();
    let mut recorder = Recorder::open(traces.path(), "kev-latest", "stub", "conformance").unwrap();
    let path = recorder.path().to_path_buf();
    recorder.user("run the list");

    let program = fan_out(root);
    let run = runtime(root, &passing_door().url)
        .with_runstate(runstate.path())
        .run(&program, &inputs(), &Grant::all(), Some(&mut recorder))
        .await;
    recorder.finish(atif::log::ENDED);
    drop(recorder);
    assert_eq!(run.stopped, None, "{:?}", run.stopped);

    // The decision calls carry the set they asked from, digested — two
    // runs under different wording are never the same measurement.
    let recording = atif::log::read(&path).expect("the trace reads back");
    let decisions: Vec<&atif::Call> = recording
        .steps
        .iter()
        .filter_map(|step| step.call.as_ref())
        .filter(|call| call.is_decision())
        .collect();
    assert_eq!(decisions.len(), 2, "independence and accept");
    for decision in &decisions {
        assert!(decision.extra["question_set"].as_str().is_some());
        assert!(
            decision.extra["set_digest"]
                .as_str()
                .is_some_and(|digest| !digest.is_empty()),
            "every decision names the wording it asked: {decision:?}"
        );
        let attempts = decision.extra["attempts"]
            .as_array()
            .expect("a decision records its attempts");
        assert_eq!(attempts.len(), 1, "one primary dispatch, answered");
        let attempt = &attempts[0];
        assert_eq!(attempt["model"], json!("kev-latest"));
        assert_eq!(attempt["outcome"], json!("answered"));
        assert!(attempt["input_tokens"].is_number() || attempt["outcome"] != "answered");
    }

    // The token ledger is the sum of what the recorded attempts
    // reported — nothing counted twice, nothing free.
    let reported: u64 = decisions
        .iter()
        .flat_map(|decision| {
            decision.extra["attempts"]
                .as_array()
                .cloned()
                .unwrap_or_default()
        })
        .map(|attempt| {
            attempt["input_tokens"].as_u64().unwrap_or(0)
                + attempt["output_tokens"].as_u64().unwrap_or(0)
        })
        .sum();
    assert_eq!(run.tokens.counted, reported);
    assert_eq!(run.tokens.unmetered, 0, "every dispatch was priced");

    // The runstate record settled answered, pinned to the digests the
    // run actually ran under.
    let mut store = Store::open(runstate.path()).unwrap();
    assert!(store.recover().unwrap().is_empty(), "the run settled");
}

// ---------------------------------------------------------------------
// Decision profiles
// ---------------------------------------------------------------------

/// A local profile never invents a credential: `direct_local` over a
/// loopback door sends no `Authorization` header even when a provider
/// key sits in the environment it could have inherited, and a key set
/// for the profile itself refuses at resolution rather than riding
/// along quietly. The hosted profile sends the key it was given; the
/// relay profile builds no HTTP client at all — its decisions travel
/// the relay, which is where the relay-transport evidence lives.
#[tokio::test]
async fn the_local_profile_sends_no_credential_anywhere() {
    if !boundary() {
        return;
    }
    let machine = machine();
    let root = machine.path();
    let local_door = passing_door();

    // A stray provider key in scope must not leak into a local call.
    let env = {
        let url = local_door.url.clone();
        move |name: &str| match name {
            "CODER_DECISION_PROFILE" => Some("direct_local".to_string()),
            "CODER_DECISION_URL" => Some(url.clone()),
            "CODER_DECISION_MODEL" => Some("kev-latest".to_string()),
            "TYPESAFE_API_KEY" => Some("ts-inherited-key".to_string()),
            "OPENAGENTS_API_KEY" => Some("oak_inherited".to_string()),
            _ => None,
        }
    };
    let profile = Profiles::new()
        .resolve(env)
        .expect("the local profile resolves");
    assert!(profile.is_local());
    let client = profile.client().expect("the local client builds");
    let run = runtime_with_client(root, client)
        .run(&fan_out(root), &inputs(), &Grant::all(), None)
        .await;
    assert_eq!(run.stopped, None, "{:?}", run.stopped);
    assert!(
        local_door
            .requests()
            .iter()
            .all(|request| request.authorization.is_none()),
        "a local profile sends no Authorization header, inherited keys or none"
    );

    // A key set for the profile itself is a mistake named at
    // resolution — never a credential quietly sent to a local door.
    let keyed_env = {
        let url = local_door.url.clone();
        move |name: &str| match name {
            "CODER_DECISION_PROFILE" => Some("direct_local".to_string()),
            "CODER_DECISION_URL" => Some(url.clone()),
            "CODER_DECISION_MODEL" => Some("kev-latest".to_string()),
            "CODER_DECISION_KEY" => Some("ts-should-not-be-here".to_string()),
            _ => None,
        }
    };
    assert!(
        Profiles::new().resolve(keyed_env).is_err(),
        "the local profile refuses a configured key rather than sending it"
    );

    // The hosted profile sends the credential it was given, as a bearer
    // token — the only header shape a keyed door reads.
    let hosted_door = passing_door();
    let hosted_env = {
        let url = hosted_door.url.clone();
        move |name: &str| match name {
            "CODER_DECISION_PROFILE" => Some("hosted_http".to_string()),
            "CODER_DECISION_URL" => Some(url.clone()),
            "CODER_DECISION_MODEL" => Some("kev-latest".to_string()),
            "CODER_DECISION_KEY" => Some("ts-hosted-key".to_string()),
            _ => None,
        }
    };
    let hosted = Profiles::new()
        .resolve(hosted_env)
        .expect("the hosted profile resolves");
    assert!(!hosted.is_local());
    let run = runtime_with_client(root, hosted.client().unwrap())
        .run(&fan_out(root), &inputs(), &Grant::all(), None)
        .await;
    assert_eq!(run.stopped, None, "{:?}", run.stopped);
    assert!(
        hosted_door
            .requests()
            .iter()
            .all(|request| request.authorization.as_deref() == Some("Bearer ts-hosted-key")),
        "the hosted profile sends its credential, and only its credential"
    );

    // The relay profile resolves — and builds no System One HTTP
    // client, because a relay's decisions travel the relay. That is
    // the honest boundary the acceptance names: the profile exists
    // where the service capability does, and says so where it does not.
    let relay_env = |name: &str| match name {
        "CODER_DECISION_PROFILE" => Some("relay".to_string()),
        "CODER_DECISION_RELAY" => Some("wss://relay.example.com".to_string()),
        "CODER_DECISION_WORKER" => {
            Some("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798".to_string())
        }
        _ => None,
    };
    let relay = Profiles::new()
        .resolve(relay_env)
        .expect("the relay profile resolves");
    match relay.client() {
        Err(coder::profiles::Refusal::Unsupported { profile, .. }) => {
            assert_eq!(profile, "relay");
        }
        other => panic!("a relay profile builds no HTTP client: {other:?}"),
    }
}

// ---------------------------------------------------------------------
// The baseline comparison
// ---------------------------------------------------------------------

/// The comparison the acceptance asks for: the complete decision
/// workflow against the deterministic baseline — the same pinned work
/// list, the same executor, the same mechanical verdicts — reporting
/// quality, error direction, coverage, time, and cost. Writing the
/// report to `CODER_CONFORMANCE_OUT` makes it the retained artifact a
/// measurement document cites; unset, the assertions still stand.
#[tokio::test]
async fn the_workflow_reports_against_its_deterministic_baseline() {
    if !boundary() {
        return;
    }
    let machine = machine();
    let root = machine.path();

    // Arm one: the complete workflow — query, semantic independence,
    // admission, fan-out, per-requirement acceptance.
    let started = std::time::Instant::now();
    let decision_door = passing_door();
    let decided = runtime(root, &decision_door.url)
        .run(&burn_down(root), &burn_down_inputs(), &Grant::all(), None)
        .await;
    let decision_ms = started.elapsed().as_millis() as u64;
    let decision_dispatches = decision_door.requests().len();
    assert_eq!(decided.stopped, None, "{:?}", decided.stopped);

    // Arm two: the deterministic baseline — the same steps minus both
    // decide steps. Mechanical admission still holds; the collision is
    // still mechanical; no judgment is asked and no token is spent.
    let baseline_program = deterministic_burn_down(root);
    let started = std::time::Instant::now();
    let baseline = runtime(root, "http://127.0.0.1:1")
        .run(&baseline_program, &burn_down_inputs(), &Grant::all(), None)
        .await;
    let baseline_ms = started.elapsed().as_millis() as u64;
    assert_eq!(baseline.stopped, None, "{:?}", baseline.stopped);
    assert_eq!(baseline.tokens.counted, 0, "the baseline spends nothing");

    // The mechanics both arms share: the colliding second write and the
    // blocked item are dropped by the lookup, before any judgment.
    let selected = decided
        .selection
        .as_ref()
        .expect("the lookup recorded what it selected");
    assert_eq!(
        selected.selected().len(),
        3,
        "the collision and the block are mechanical: {:?}",
        selected.collisions
    );
    assert_eq!(
        selected.selected().len(),
        baseline
            .selection
            .as_ref()
            .expect("the baseline's lookup")
            .selected()
            .len(),
        "the deterministic baseline sees the same mechanical floor"
    );

    // Quality is the mechanical verdict, not the door's: items that
    // stated an answer are graded against it under both arms.
    let (d_right, d_total) = decided.correct();
    let (b_right, b_total) = baseline.correct();

    let report = json!({
        "case": "burn-down over the pinned five-item work list (one write collision, one blocked item)",
        "decision_workflow": {
            "steps": decided.step_names(),
            "delegations": decided.delegations.len(),
            "answered": decided.answered(),
            "correct": d_right,
            "graded": d_total,
            "tally": decided.tally().to_string(),
            "decision_dispatches": decision_dispatches,
            "decision_tokens": decided.tokens.counted,
            "unmetered": decided.tokens.unmetered,
            "wall_ms": decision_ms,
            "retained_worktrees": decided.delegations.iter().filter(|d| d.retained.is_some()).count(),
        },
        "deterministic_baseline": {
            "steps": baseline.step_names(),
            "delegations": baseline.delegations.len(),
            "answered": baseline.answered(),
            "correct": b_right,
            "graded": b_total,
            "tally": baseline.tally().to_string(),
            "decision_dispatches": 0,
            "decision_tokens": 0,
            "wall_ms": baseline_ms,
            "retained_worktrees": baseline.delegations.iter().filter(|d| d.retained.is_some()).count(),
        },
        "error_direction": {
            "missed_program": "a `none` answer costs one ordinary turn, recoverable by rephrasing",
            "spurious_program": "a selection is a proposal — the grant refuses what it does not name before any step",
            "semantic_independence": "cannot waive a mechanical collision; it can only admit what mechanics already allows",
        }
    });

    if let Some(out) = std::env::var_os("CODER_CONFORMANCE_OUT") {
        std::fs::write(&out, format!("{report:#}\n")).expect("the report writes");
        eprintln!("comparison report written to {}", Path::new(&out).display());
    }

    // The comparison stands on its assertions: both arms covered the
    // same selected work; the decision arm paid for a semantic gate and
    // per-item acceptance, and the report says what that bought.
    assert_eq!(decided.delegations.len(), baseline.delegations.len());
    assert_eq!(d_total, b_total);
    assert!(decision_dispatches >= 2, "independence plus acceptance");
    assert!(decided.tokens.counted > 0);
    // A writing item's worktree outlives its step — the reviewable
    // retained checkout the workflow promises.
    assert!(
        decided
            .delegations
            .iter()
            .filter(|d| d.task.writes)
            .all(|d| d.retained.is_some()),
        "a writing delegation retains its worktree for review"
    );
}

/// `burn-down` without its `decide` steps — the deterministic arm of
/// the comparison, written into the scratch registry.
fn deterministic_burn_down(root: &Path) -> Program {
    let mut program: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("programs").join("burn-down.json")).unwrap(),
    )
    .unwrap();
    let removed = ["independence", "accept"];
    let steps: Vec<Value> = program["definition"]["steps"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|step| step["kind"].as_str() != Some("decide"))
        .map(|step| {
            let mut step = step.clone();
            if let Some(after) = step["after"].as_array_mut() {
                after.retain(|name| !removed.contains(&name.as_str().unwrap_or_default()));
            }
            step
        })
        .collect();
    program["definition"]["steps"] = json!(steps);
    program["definition"]["id"] = json!(
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb:openagents/burn-down-deterministic"
    );
    program["definition"]["result"] = json!({"from": "step:fan_out", "pointer": "/value"});
    program["definition"]["summary"] =
        json!("The deterministic arm: work list, admission, fan-out, no judgments.");
    program["binding"]["name"] = json!("Deterministic burn-down");
    if let Some(binding_steps) = program["binding"]["steps"].as_object_mut() {
        binding_steps.remove("independence");
        binding_steps.remove("accept");
    }
    let path = root.join("programs").join("burn-down-deterministic.json");
    std::fs::write(&path, program.to_string()).unwrap();
    Program::load(&path).expect("the deterministic variant loads")
}
