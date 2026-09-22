//! The repository evidence selection site, at the turn level.
//!
//! A stub Jev door answers the evidence-relevance questions over the
//! candidates a sniff observed, and the agent replies against a temp
//! repository. These tests show that a chosen ranking reorders the
//! excerpts the prompt renders, that an abstention and a dead door both
//! fall back to the deterministic order, that a lone candidate asks
//! nothing, that a standing ranking is reused rather than re-asked, and
//! that the trace records the call and the selection manifest either
//! way.
//!
//! Refs #9513.

use std::net::SocketAddr;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};

use coder::Agent;
use coder::generate::{Door, StubGenerate};
use coder::repo::Repo;
use coder::trace::Recorder;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// What the stub door saw: every request body, in order.
type Seen = Arc<Mutex<Vec<Value>>>;

/// One gate answer, as the stub door replies with it.
#[derive(Clone)]
struct Pick {
    choice: &'static str,
    probabilities: Vec<(&'static str, f64)>,
}

/// A door that answers the evidence-relevance questions with `script`,
/// one pick per request, repeating the last once the script is spent.
async fn door(script: Vec<Pick>, seen: Seen) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("a port");
    let address: SocketAddr = listener.local_addr().unwrap();
    let script = Arc::new(Mutex::new(script));
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let script = Arc::clone(&script);
            let seen = Arc::clone(&seen);
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
                    seen.lock().unwrap().push(asked);
                    let pick = {
                        let mut script = script.lock().unwrap();
                        if script.len() > 1 {
                            script.remove(0)
                        } else {
                            script[0].clone()
                        }
                    };
                    let reply = answer(&pick).to_string();
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

/// The response body for one pick.
fn answer(pick: &Pick) -> Value {
    let mut probabilities = serde_json::Map::new();
    for (option, probability) in &pick.probabilities {
        probabilities.insert(option.to_string(), json!(probability));
    }
    json!({
        "model": "stub-select",
        "answers": {
            "most_relevant": {
                "type": "choice",
                "choice": pick.choice,
                "confidence": 0.9,
                "probabilities": Value::Object(probabilities),
            },
            "any_relevant": { "type": "noul", "noul": 0.9 },
            "coverage": { "type": "noul", "noul": 0.7 },
        },
    })
}

/// A temp repository holding `files`, each indexed so `git grep` names
/// it: every file carries the term the task will ask about.
fn repository(files: &[(&str, &str)]) -> (tempfile::TempDir, Repo) {
    let dir = tempfile::tempdir().unwrap();
    assert!(
        Command::new("git")
            .arg("-C")
            .arg(dir.path())
            .arg("init")
            .output()
            .expect("git runs")
            .status
            .success(),
        "git init"
    );
    for (name, content) in files {
        std::fs::write(dir.path().join(name), content).unwrap();
    }
    let staged = Command::new("git")
        .arg("-C")
        .arg(dir.path())
        .arg("add")
        .args(files.iter().map(|(name, _)| name))
        .output()
        .expect("git runs");
    assert!(staged.status.success(), "git add");
    let repo = Repo::discover(dir.path()).expect("a repo sits everywhere");
    (dir, repo)
}

/// An agent whose decision door answers `script`, whose model says
/// `line`, sitting in `repo` and recording to `logs`.
async fn agent(logs: &Path, script: Vec<Pick>, seen: Seen, repo: Repo, line: &str) -> Agent {
    let client = jev::Client::new(
        jev::Config::new()
            .api_key("ts-test-key")
            .base_url(door(script, seen).await)
            .default_model("stub-select"),
    )
    .expect("the stub door builds a client");
    let recorder = Recorder::open(logs, "stub", "stub", "/tmp/repo").expect("a trace opens");
    Agent::new(Some(client), Door::Stub(StubGenerate::saying(line)))
        .with_repo(Some(repo))
        .with_trace(Some(recorder))
}

/// The trace the agent at `logs` left: every recorded step.
fn recording(logs: &Path) -> Vec<atif::Step> {
    let logs = std::fs::read_dir(logs).unwrap().next().unwrap().unwrap();
    atif::log::read(&logs.path()).unwrap().steps
}

/// The `repository_context` extension on the step that carried it.
fn repository_context(steps: &[atif::Step]) -> &Value {
    steps
        .iter()
        .find_map(|step| step.extensions.get("repository_context"))
        .expect("a repository context step")
}

/// The decision calls the trace recorded, in order.
fn calls(steps: &[atif::Step]) -> Vec<&atif::Call> {
    steps.iter().filter_map(|step| step.call.as_ref()).collect()
}

/// The position of `needle` in `text`, or a failing report.
fn position(text: &str, needle: &str) -> usize {
    text.find(needle)
        .unwrap_or_else(|| panic!("{needle} is not in the prompt:\n{text}"))
}

/// A chosen pick reorders the rendered excerpts, the request named only
/// what the sniff observed, and the manifest records the judgment.
#[tokio::test]
async fn a_chosen_ranking_orders_the_context_and_records_the_selection() {
    let (dir, repo) = repository(&[
        ("alpha.rs", "needle in alpha\n"),
        ("beta.rs", "needle in beta\n"),
    ]);
    let logs = tempfile::tempdir().unwrap();
    let seen: Seen = Arc::new(Mutex::new(Vec::new()));
    let mut agent = agent(
        logs.path(),
        vec![Pick {
            choice: "beta.rs (the whole file)",
            probabilities: vec![
                ("beta.rs (the whole file)", 0.6),
                ("beta.rs (lines 1-1)", 0.2),
                ("alpha.rs (the whole file)", 0.1),
                ("alpha.rs (lines 1-1)", 0.05),
                ("none", 0.05),
            ],
        }],
        Arc::clone(&seen),
        repo,
        "done",
    )
    .await;
    agent.push_user("where does needle live");
    agent
        .reply(false, &mut |_| {}, &mut |_| {})
        .await
        .expect("the reply generates");
    drop(dir);

    // One ask, naming exactly the observations the sniff made plus
    // `none` — each file as a whole-file read and a first-line
    // reference, and never a byte of content.
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1, "one selection call: {seen:?}");
    let asked = &seen[0];
    assert_eq!(asked["state"]["task"], "where does needle live");
    let listed = asked["state"]["candidates"].as_array().unwrap();
    assert_eq!(listed.len(), 4, "two observations per path: {listed:?}");
    for candidate in listed {
        assert!(
            candidate.get("content").is_none() && candidate.get("text").is_none(),
            "the request names observations, never content: {candidate}"
        );
    }
    let gate = &asked["questions"]["most_relevant"]["criteria"];
    let options: Vec<&str> = gate
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        options,
        [
            "none",
            "alpha.rs (the whole file)",
            "alpha.rs (lines 1-1)",
            "beta.rs (the whole file)",
            "beta.rs (lines 1-1)",
        ]
    );
    drop(seen);

    let steps = recording(logs.path());
    let prompt = steps
        .iter()
        .find(|step| step.extensions.contains_key("repository_context"))
        .map(|step| step.message.as_str())
        .expect("the instructions step");
    assert!(
        position(prompt, "beta.rs:1:") < position(prompt, "alpha.rs:1:"),
        "the pick's excerpts render first:\n{prompt}"
    );

    let selection = &repository_context(&steps)["selection"];
    assert_eq!(selection["verdict"], "chosen");
    assert_eq!(selection["selected"]["path"], "beta.rs");
    assert_eq!(
        selection["question_set"],
        "openagents.evidence-relevance.v1"
    );
    assert_eq!(selection["model"], "stub-select");
    assert_eq!(selection["any_relevant"], json!(0.9));
    assert_eq!(selection["coverage"], json!(0.7));

    let calls = calls(&steps);
    let select = calls
        .iter()
        .find(|call| call.name == "repository/select")
        .expect("the selection call is recorded");
    assert_eq!(select.extra["route"], json!("chosen"));
    assert!(select.extra.get("error").is_none(), "{:?}", select.extra);
    assert_eq!(
        select.extra["question_set"],
        json!("openagents.evidence-relevance.v1"),
        "the call names the set it asked"
    );
}

/// A `none` pick is an abstention: the sniff's own order renders, and
/// the manifest still records the judgment it ran under.
#[tokio::test]
async fn an_abstention_keeps_the_deterministic_order_and_records_it() {
    let (dir, repo) = repository(&[
        ("alpha.rs", "needle in alpha\n"),
        ("beta.rs", "needle in beta\n"),
    ]);
    let logs = tempfile::tempdir().unwrap();
    let seen: Seen = Arc::new(Mutex::new(Vec::new()));
    let mut agent = agent(
        logs.path(),
        vec![Pick {
            choice: "none",
            probabilities: vec![
                ("none", 0.9),
                ("alpha.rs (the whole file)", 0.05),
                ("beta.rs (the whole file)", 0.05),
            ],
        }],
        Arc::clone(&seen),
        repo,
        "done",
    )
    .await;
    agent.push_user("where does needle live");
    agent
        .reply(false, &mut |_| {}, &mut |_| {})
        .await
        .expect("the reply generates");
    drop(dir);
    assert_eq!(seen.lock().unwrap().len(), 1);

    let steps = recording(logs.path());
    let prompt = steps
        .iter()
        .find(|step| step.extensions.contains_key("repository_context"))
        .map(|step| step.message.as_str())
        .expect("the instructions step");
    assert!(
        position(prompt, "alpha.rs:1:") < position(prompt, "beta.rs:1:"),
        "an abstention reorders nothing:\n{prompt}"
    );
    let selection = &repository_context(&steps)["selection"];
    assert_eq!(selection["verdict"], "abstained");
    assert!(selection["selected"].is_null());
}

/// A door that never answers costs the turn its ordering, not its
/// context: the deterministic order renders and the trace holds the
/// failure.
#[tokio::test]
async fn a_dead_door_falls_back_to_the_deterministic_order() {
    let (dir, repo) = repository(&[
        ("alpha.rs", "needle in alpha\n"),
        ("beta.rs", "needle in beta\n"),
    ]);
    let logs = tempfile::tempdir().unwrap();
    let closed = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = closed.local_addr().unwrap();
    drop(closed);
    let client = jev::Client::new(
        jev::Config::new()
            .api_key("ts-test-key")
            .base_url(format!("http://{address}"))
            .default_model("stub-select"),
    )
    .expect("a client against a dead port still builds");
    let recorder = Recorder::open(logs.path(), "stub", "stub", "/tmp/repo").unwrap();
    let mut agent = Agent::new(Some(client), Door::Stub(StubGenerate::saying("done")))
        .with_repo(Some(repo))
        .with_trace(Some(recorder));
    agent.push_user("where does needle live");
    agent
        .reply(false, &mut |_| {}, &mut |_| {})
        .await
        .expect("the reply generates despite the dead door");
    drop(dir);

    let steps = recording(logs.path());
    let prompt = steps
        .iter()
        .find(|step| step.extensions.contains_key("repository_context"))
        .map(|step| step.message.as_str())
        .expect("the instructions step");
    assert!(
        position(prompt, "alpha.rs:1:") < position(prompt, "beta.rs:1:"),
        "a dead door reorders nothing:\n{prompt}"
    );
    assert!(
        repository_context(&steps).get("selection").is_none(),
        "no ranking, no selection record"
    );
    let calls = calls(&steps);
    let select = calls
        .iter()
        .find(|call| call.name == "repository/select")
        .expect("the failed call is recorded");
    assert!(
        select.extra["error"].as_str().is_some(),
        "the failure says why: {:?}",
        select.extra
    );
    assert!(select.extra.get("route").is_none());
}

/// One observed path asks nothing: the deterministic order already
/// names its own most relevant, whichever of its observations a pick
/// would choose.
#[tokio::test]
async fn a_lone_candidate_asks_nothing() {
    let (dir, repo) = repository(&[("only.rs", "needle alone\n")]);
    let logs = tempfile::tempdir().unwrap();
    let seen: Seen = Arc::new(Mutex::new(Vec::new()));
    let mut agent = agent(
        logs.path(),
        vec![Pick {
            choice: "none",
            probabilities: vec![("none", 1.0)],
        }],
        Arc::clone(&seen),
        repo,
        "done",
    )
    .await;
    agent.push_user("where does needle live");
    agent
        .reply(false, &mut |_| {}, &mut |_| {})
        .await
        .expect("the reply generates");
    drop(dir);
    assert!(
        seen.lock().unwrap().is_empty(),
        "a lone candidate spent a decision call"
    );
    let steps = recording(logs.path());
    assert!(
        repository_context(&steps).get("selection").is_none(),
        "an unasked question records no selection"
    );
}

/// A standing ranking answers again while task, evidence, wording,
/// policy, and artifact all match: a second reply on the same task
/// spends no second call.
#[tokio::test]
async fn a_standing_ranking_is_reused_not_reasked() {
    let (dir, repo) = repository(&[
        ("alpha.rs", "needle in alpha\n"),
        ("beta.rs", "needle in beta\n"),
    ]);
    let logs = tempfile::tempdir().unwrap();
    let seen: Seen = Arc::new(Mutex::new(Vec::new()));
    let mut agent = agent(
        logs.path(),
        vec![Pick {
            choice: "beta.rs (the whole file)",
            probabilities: vec![
                ("beta.rs (the whole file)", 0.6),
                ("beta.rs (lines 1-1)", 0.2),
                ("alpha.rs (the whole file)", 0.1),
                ("alpha.rs (lines 1-1)", 0.05),
                ("none", 0.05),
            ],
        }],
        Arc::clone(&seen),
        repo,
        "done",
    )
    .await;
    agent.push_user("where does needle live");
    agent
        .reply(false, &mut |_| {}, &mut |_| {})
        .await
        .expect("the first reply generates");
    agent
        .reply(false, &mut |_| {}, &mut |_| {})
        .await
        .expect("the second reply generates");
    drop(dir);
    assert_eq!(
        seen.lock().unwrap().len(),
        1,
        "the same evidence under the same task asks once"
    );
}

/// A new task is a new state: the cached ranking is dropped and the
/// door is asked again.
#[tokio::test]
async fn a_new_task_asks_again() {
    let (dir, repo) = repository(&[
        ("alpha.rs", "needle in alpha\n"),
        ("beta.rs", "needle in beta\n"),
    ]);
    let logs = tempfile::tempdir().unwrap();
    let seen: Seen = Arc::new(Mutex::new(Vec::new()));
    let mut agent = agent(
        logs.path(),
        vec![Pick {
            choice: "beta.rs (the whole file)",
            probabilities: vec![
                ("beta.rs (the whole file)", 0.6),
                ("beta.rs (lines 1-1)", 0.2),
                ("alpha.rs (the whole file)", 0.1),
                ("alpha.rs (lines 1-1)", 0.05),
                ("none", 0.05),
            ],
        }],
        Arc::clone(&seen),
        repo,
        "done",
    )
    .await;
    agent.push_user("where does needle live");
    agent
        .reply(false, &mut |_| {}, &mut |_| {})
        .await
        .expect("the first reply generates");
    agent.push_user("show me needle again");
    agent
        .reply(false, &mut |_| {}, &mut |_| {})
        .await
        .expect("the second reply generates");
    drop(dir);
    assert_eq!(
        seen.lock().unwrap().len(),
        2,
        "a new task asks the question again"
    );
}
