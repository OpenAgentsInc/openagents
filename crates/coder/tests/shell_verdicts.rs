//! The shell judge's verdicts, at the turn level.
//!
//! A stub Jev door answers the shell questions with a scripted verdict per
//! round, and the turn runs read-only plans against it. These tests show
//! that `retry` changes the next generation and stops after its bound,
//! that `pass` and `stop` keep their meaning, and that a `damage` number
//! only stops the loop when the door says it is a calibrated probability.
//!
//! Refs #9396, #9397.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use coder::agent::{Exhausted, RETRIES_MAX};
use coder::classify::DAMAGE_FAMILY;
use coder::generate::{Door, StubGenerate};
use coder::trace::Recorder;
use coder::{Agent, Ending, Permit, ShellEvent};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// One shell verdict, as the stub door will answer it.
#[derive(Clone)]
struct Verdict {
    choice: &'static str,
    damage: f64,
    calibration: Option<Value>,
}

/// A verdict from a hosted door that says nothing about calibration.
fn choosing(choice: &'static str) -> Verdict {
    Verdict {
        choice,
        damage: 0.05,
        calibration: None,
    }
}

/// A verdict whose `damage` came from a door that says `state` about it.
fn damaged(damage: f64, extensions: Value) -> Verdict {
    Verdict {
        choice: "pass",
        damage,
        calibration: Some(extensions),
    }
}

/// What the stub door saw: every request body, in order.
type Seen = Arc<Mutex<Vec<Value>>>;

/// A door that answers the shell questions with `script`, one verdict per
/// request, repeating the last once the script is spent.
async fn door(script: Vec<Verdict>, seen: Seen) -> String {
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
                    let verdict = {
                        let mut script = script.lock().unwrap();
                        if script.len() > 1 {
                            script.remove(0)
                        } else {
                            script[0].clone()
                        }
                    };
                    let reply = answer(&verdict).to_string();
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

/// The response body for one verdict.
fn answer(verdict: &Verdict) -> Value {
    let mut probabilities = json!({ "pass": 0.0, "retry": 0.0, "stop": 0.0 });
    probabilities[verdict.choice] = json!(1.0);
    let mut body = json!({
        "model": "stub-judge",
        "answers": {
            "outcome": {
                "type": "choice",
                "choice": verdict.choice,
                "confidence": 0.9,
                "probabilities": probabilities,
            },
            "useful": { "type": "noul", "noul": 0.5 },
            "damage": { "type": "noul", "noul": verdict.damage },
        },
    });
    if let Some(extensions) = &verdict.calibration {
        body["extensions"] = extensions.clone();
    }
    body
}

/// A read-only plan of one command.
fn plan() -> String {
    json!({
        "v": 1,
        "commands": [{ "command": "printf observed", "why": "look" }],
    })
    .to_string()
}

/// An agent whose judge answers `script` and whose model always plans,
/// recording to a trace under `dir`.
async fn agent(dir: &std::path::Path, script: Vec<Verdict>, seen: Seen) -> Agent {
    let client = jev::Client::new(
        jev::Config::new()
            .api_key("ts-test-key")
            .base_url(door(script, seen).await)
            .default_model("stub-judge"),
    )
    .expect("the stub door builds a client");
    let recorder = Recorder::open(dir, "stub", "stub", "/tmp/repo").expect("a trace opens");
    Agent::new(Some(client), Door::Stub(StubGenerate::saying(plan()))).with_trace(Some(recorder))
}

/// Runs one executing turn and returns what it produced, how many rounds
/// ran, the verdict lines shown, and the trace text.
async fn turn(mut agent: Agent) -> (coder::Turned, usize, Vec<String>, String) {
    agent.push_user("look at the repository");
    let mut ran = 0usize;
    let mut verdicts = Vec::new();
    let turned = agent
        .turn(
            false,
            Permit::executing(),
            &mut |_| {},
            &mut |_| {},
            &mut |event| match event {
                ShellEvent::Ran(_) => ran += 1,
                ShellEvent::Verdict(line) => verdicts.push(line),
                ShellEvent::Proposed(_) => {}
            },
        )
        .await
        .expect("the turn completes");
    let path = agent.trace_path().expect("a trace path").to_path_buf();
    agent.finish_trace();
    let trace = std::fs::read_to_string(path).expect("the trace reads");
    (turned, ran, verdicts, trace)
}

/// A `retry` verdict changes the next generation's instructions, and the
/// request that asked for it names the damage family so the door can
/// serve its calibration.
#[tokio::test]
async fn a_retry_is_acted_on() {
    let dir = tempfile::tempdir().unwrap();
    let seen: Seen = Arc::default();
    let script = vec![choosing("retry"), choosing("pass")];
    let (turned, ran, _, trace) = turn(agent(dir.path(), script, Arc::clone(&seen)).await).await;
    assert_eq!(
        ran,
        Permit::executing().rounds(),
        "one retry did not end the loop"
    );
    assert_eq!(turned.exhausted, Some(Exhausted::Rounds(ran)));
    assert!(
        trace.contains("Do not run the same commands again"),
        "the generation after a retry was not told: {trace}"
    );
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), ran);
    assert_eq!(seen[0]["extensions"]["family"], DAMAGE_FAMILY);
    assert_eq!(seen[0]["extensions"]["estimator"], true);
}

/// Retries in a row stop at their bound, as a typed exhaustion the reply
/// names rather than another round.
#[tokio::test]
async fn the_retry_bound_is_a_typed_stop() {
    let dir = tempfile::tempdir().unwrap();
    let (turned, ran, _, _) =
        turn(agent(dir.path(), vec![choosing("retry")], Arc::default()).await).await;
    assert_eq!(ran, RETRIES_MAX);
    assert!(
        ran < Permit::executing().rounds(),
        "the bound is the permit's"
    );
    assert_eq!(turned.exhausted, Some(Exhausted::Retries(RETRIES_MAX)));
    assert!(
        matches!(turned.ending, Ending::Refused { .. }),
        "{:?}",
        turned.ending
    );
    assert!(
        turned.text.contains("retry") && turned.text.contains("in a row"),
        "{}",
        turned.text
    );
}

/// A `pass` between retries resets the count, so only consecutive retries
/// reach the bound.
#[tokio::test]
async fn a_pass_resets_the_retry_count() {
    let dir = tempfile::tempdir().unwrap();
    let script = vec![
        choosing("retry"),
        choosing("pass"),
        choosing("retry"),
        choosing("pass"),
    ];
    let (turned, ran, _, _) = turn(agent(dir.path(), script, Arc::default()).await).await;
    assert_eq!(ran, Permit::executing().rounds());
    assert_eq!(turned.exhausted, Some(Exhausted::Rounds(ran)));
}

/// `pass` still runs the permit's rounds, and `stop` still ends the loop
/// on the first round.
#[tokio::test]
async fn pass_and_stop_are_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let (turned, ran, _, _) =
        turn(agent(dir.path(), vec![choosing("pass")], Arc::default()).await).await;
    assert_eq!(ran, Permit::executing().rounds());
    assert_eq!(turned.exhausted, Some(Exhausted::Rounds(ran)));

    let (turned, ran, _, _) =
        turn(agent(dir.path(), vec![choosing("stop")], Arc::default()).await).await;
    assert_eq!(ran, 1);
    assert_eq!(turned.exhausted, Some(Exhausted::Stopped));
}

/// A door with an admitted calibration map for the damage family routes
/// `damage` at the threshold to a stop.
#[tokio::test]
async fn a_calibrated_door_routes_damage() {
    let dir = tempfile::tempdir().unwrap();
    let fitted = json!({ "calibration": { "state": "calibrated", "family": DAMAGE_FAMILY } });
    let script = vec![damaged(0.75, fitted)];
    let (turned, ran, verdicts, _) = turn(agent(dir.path(), script, Arc::default()).await).await;
    assert_eq!(ran, 1);
    assert_eq!(turned.exhausted, Some(Exhausted::Stopped));
    assert!(verdicts[0].contains("stop"), "{}", verdicts[0]);
}

/// A door without one does not fire the stop, however high the number:
/// eight draws that all agreed read 1.0, which is the estimator's ceiling
/// at a resolution of 1/8, not a probability of one. The verdict line says
/// so.
#[tokio::test]
async fn an_uncalibrated_door_does_not_fire_the_stop() {
    let dir = tempfile::tempdir().unwrap();
    let raw = json!({
        "calibration": { "state": "uncalibrated" },
        "estimator": { "damage": { "samples": 8, "resolution": 0.125 } },
    });
    let script = vec![damaged(1.0, raw)];
    let (turned, ran, verdicts, _) = turn(agent(dir.path(), script, Arc::default()).await).await;
    assert_eq!(
        ran,
        Permit::executing().rounds(),
        "an uncalibrated number stopped the loop"
    );
    assert_eq!(turned.exhausted, Some(Exhausted::Rounds(ran)));
    assert!(
        verdicts[0].contains("uncalibrated") && verdicts[0].contains("1/8"),
        "{}",
        verdicts[0]
    );
    assert!(!verdicts[0].contains("stop"), "{}", verdicts[0]);
}

/// A hosted door that says nothing about calibration is read the same
/// way: the number is shown as unstated and does not route.
#[tokio::test]
async fn an_unstated_door_does_not_fire_the_stop() {
    let dir = tempfile::tempdir().unwrap();
    let mut verdict = choosing("pass");
    verdict.damage = 0.9;
    let (turned, ran, verdicts, _) =
        turn(agent(dir.path(), vec![verdict], Arc::default()).await).await;
    assert_eq!(ran, Permit::executing().rounds());
    assert_eq!(turned.exhausted, Some(Exhausted::Rounds(ran)));
    assert!(verdicts[0].contains("unstated"), "{}", verdicts[0]);
}
