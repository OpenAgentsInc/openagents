//! `coder --print` drives a turn from a script, and the trace it leaves is
//! the trace the terminal leaves.
//!
//! These tests run the built binary rather than calling into the library,
//! because the thing being tested is the binary's contract: what goes to
//! standard output, where the trace lands, and what the exit code says.
//! Every run is given a temporary trace directory and stripped of the
//! credentials a developer's shell may be carrying, so the door is the
//! stub and the turn is the same turn on every machine.

use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;

/// The variables that would otherwise let one machine's environment decide
/// what these tests measure.
const CREDENTIALS: [&str; 17] = [
    "TYPESAFE_API_KEY",
    "TYPESAFE_BASE_URL",
    "TYPESAFE_DEFAULT_MODEL",
    "OPENAGENTS_API_KEY",
    "OPENAGENTS_BASE_URL",
    "CODER_DECISION_PROFILE",
    "CODER_DECISION_URL",
    "CODER_DECISION_MODEL",
    "CODER_DECISION_KEY",
    "CODER_DECISION_RELAY",
    "CODER_DECISION_WORKER",
    "CODER_DOOR_KEY",
    "CODER_AI_GATEWAY_KEY",
    "CODER_DOOR_URL",
    "CODER_MODEL",
    "CODER_WORKER",
    "CODER_RELAY",
];

/// Runs the binary with `arguments`, recording into `traces`.
fn coder(arguments: &[&str], traces: &Path) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_coder"));
    for name in CREDENTIALS {
        command.env_remove(name);
    }
    command
        .env_remove("CODER_TRACE")
        .env("CODER_TRACE_DIR", traces)
        .args(arguments)
        .output()
        .expect("the binary runs")
}

/// What the stub door answers with, which is what a turn with no
/// credentials produces.
fn stub_answer(text: &str) -> bool {
    text.contains("stub door")
}

/// A headless turn writes the reply to standard output, exits 0, and
/// leaves the trace where it was told to.
#[test]
fn a_headless_turn_prints_a_reply_and_records_it() {
    let dir = tempfile::tempdir().unwrap();
    let trace = dir.path().join("one.atif.jsonl");
    let output = coder(
        &[
            "-p",
            "what crates are here",
            "--trace",
            &trace.display().to_string(),
        ],
        dir.path(),
    );

    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let reply = String::from_utf8(output.stdout).unwrap();
    assert!(stub_answer(&reply), "{reply:?}");

    // The trace is read back, and it is the same record the terminal
    // writes: the person's turn, the note about the missing classifier,
    // the instructions, and the answer.
    let recording = atif::log::read(&trace).expect("the trace reads back");
    assert_eq!(recording.session.state, atif::log::ENDED);
    let document = recording.document();
    let steps = document["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 4, "{document:#}");
    assert_eq!(steps[0]["source"], "user");
    assert_eq!(steps[0]["message"], "what crates are here");
    assert!(
        steps[1]["message"]
            .as_str()
            .unwrap()
            .contains("decision door")
    );
    assert_eq!(steps[2]["extra"]["kind"], "instructions");
    assert_eq!(steps[3]["source"], "agent");
    assert_eq!(steps[3]["message"], reply.trim_end());
    assert_eq!(document["extra"]["directive"], "what crates are here");
}

/// With `--json` the stream ends with the report as one object on
/// standard output: the reply, the trace it came from, and how the turn
/// finished.
#[test]
fn a_headless_turn_reports_one_json_object() {
    let dir = tempfile::tempdir().unwrap();
    let output = coder(&["-p", "--json", "count the crates"], dir.path());

    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let last = stdout.lines().last().expect("a report line");
    let report: Value = serde_json::from_str(last).expect("one JSON object");
    assert!(stub_answer(report["reply"].as_str().unwrap()));
    assert_eq!(report["outcome"], "answered");
    assert_eq!(report["route"], "respond");
    assert!(report["error"].is_null());
    assert!(report["cause"].is_null());
    assert!(report["refusal"].is_null());

    // The trace the report names is a trace, and it holds this turn.
    let trace = report["trace"].as_str().expect("a trace path");
    let recording = atif::log::read(Path::new(trace)).expect("the trace reads back");
    assert_eq!(recording.session.directive, "count the crates");
    assert_eq!(recording.session.state, atif::log::ENDED);
    assert_eq!(recording.document()["steps"][3]["message"], report["reply"]);
}

/// A prompt with newlines comes from a file, without shell quoting.
#[test]
fn a_prompt_reads_from_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let prompt = dir.path().join("ask.txt");
    std::fs::write(&prompt, "count the crates\nthen name them\n").unwrap();
    let output = coder(
        &["--prompt-file", &prompt.display().to_string(), "--json"],
        dir.path(),
    );

    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let last = stdout.lines().last().expect("a report line");
    let report: Value = serde_json::from_str(last).unwrap();
    let trace = report["trace"].as_str().expect("a trace path");
    let recording = atif::log::read(Path::new(trace)).unwrap();
    assert_eq!(
        recording.document()["steps"][0]["message"],
        "count the crates\nthen name them\n"
    );
}

/// A named trace that cannot be opened ends the run. A caller that named a
/// file is going to read it back, and an unrecorded turn that exits 0
/// would read as a recorded one.
#[test]
fn a_named_trace_that_cannot_be_opened_ends_the_run() {
    let dir = tempfile::tempdir().unwrap();
    let trace = dir.path().join("taken.atif.jsonl");
    std::fs::write(&trace, "").unwrap();
    let output = coder(
        &["-p", "hello", "--trace", &trace.display().to_string()],
        dir.path(),
    );

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("cannot record"),
        "{output:?}"
    );
}

/// A failed turn says why in a field, not only in a sentence.
///
/// A relay that would not take the job, a worker that never answered, and
/// a worker that declined are three states, and a harness that had to read
/// prose to tell them apart would be matching on wording.
#[test]
fn a_failed_turn_reports_its_cause_as_a_field() {
    let dir = tempfile::tempdir().unwrap();
    let trace = dir.path().join("taken.atif.jsonl");
    std::fs::write(&trace, "").unwrap();
    let output = coder(
        &[
            "-p",
            "--json",
            "hello",
            "--trace",
            &trace.display().to_string(),
        ],
        dir.path(),
    );

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let report: Value = serde_json::from_slice(&output.stdout).expect("one JSON object");
    assert_eq!(report["outcome"], "failed");
    assert_eq!(report["cause"], "trace");
    assert!(report["refusal"].is_null());
    assert!(report["reply"].is_null());
    assert!(report["error"].as_str().unwrap().contains("cannot record"));
}

/// A wrong command line is its own kind of wrong, and its exit code says
/// so rather than passing for a turn that failed.
#[test]
fn a_wrong_command_line_exits_with_the_usage_code() {
    let dir = tempfile::tempdir().unwrap();
    let output = coder(&["-p"], dir.path());
    assert_eq!(output.status.code(), Some(64), "{output:?}");
    assert!(output.stdout.is_empty());
    let said = String::from_utf8_lossy(&output.stderr);
    assert!(said.contains("--print needs a prompt"), "{said}");
    assert!(said.contains("Usage:"), "{said}");
}

/// `--help` says what the flags are and exits 0.
#[test]
fn help_says_what_the_flags_are() {
    let dir = tempfile::tempdir().unwrap();
    let output = coder(&["--help"], dir.path());
    assert_eq!(output.status.code(), Some(0));
    let said = String::from_utf8_lossy(&output.stdout);
    for flag in ["--print", "--prompt-file", "--trace", "--json"] {
        assert!(said.contains(flag), "{said}");
    }
}

/// An environment that names two doors ends the run rather than picking
/// one of them.
///
/// `CODER_DOOR_KEY` asks for an own-key door and `CODER_WORKER` asks for
/// the relay. Preferring either silently is how someone measuring the
/// relay with a key still in their shell measures the other transport and
/// gets a plausible number. Nothing is recorded, because a trace of this
/// run would have to claim a door.
#[test]
fn two_configured_doors_end_the_run_rather_than_measuring_one() {
    let dir = tempfile::tempdir().unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_coder"));
    for name in CREDENTIALS {
        command.env_remove(name);
    }
    let output = command
        .env_remove("CODER_TRACE")
        .env("CODER_TRACE_DIR", dir.path())
        .env("CODER_DOOR_KEY", "a-key")
        .env("CODER_WORKER", "0".repeat(64))
        .args(["-p", "--json", "hello"])
        .output()
        .expect("the binary runs");

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let report: Value = serde_json::from_slice(&output.stdout).expect("one JSON object");
    assert_eq!(report["outcome"], "failed");
    assert_eq!(report["cause"], "config");
    assert!(report["trace"].is_null());
    let error = report["error"].as_str().unwrap();
    assert!(error.contains("CODER_DOOR_KEY"), "{error}");
    assert!(error.contains("CODER_WORKER"), "{error}");
    assert!(
        atif::log::list(dir.path()).unwrap().is_empty(),
        "a run that cannot name its door records nothing"
    );
}

#[test]
fn malformed_decision_profiles_stop_headless_startup() {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_coder"))
        .env_clear()
        .current_dir(dir.path())
        .env("HOME", dir.path())
        .env("CODER_TRACE", "off")
        .env("CODER_DECISION_PROFILE", "unknown")
        .args(["-p", "hello"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("CODER_DECISION_PROFILE"));
}

#[test]
fn local_profile_routes_a_headless_turn_without_sending_a_provider_key() {
    use std::io::{Read, Write};
    use std::time::{Duration, Instant};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    listener.set_nonblocking(true).unwrap();
    let server = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut socket = loop {
            match listener.accept() {
                Ok((socket, _)) => break socket,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "decision call was not made");
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("{error}"),
            }
        };
        // Accepted sockets can inherit the listener's nonblocking mode.
        // Use the bounded blocking read below on every supported host.
        socket.set_nonblocking(false).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut bytes = Vec::new();
        let mut buffer = [0u8; 8192];
        loop {
            let count = socket.read(&mut buffer).unwrap();
            assert!(count > 0);
            bytes.extend_from_slice(&buffer[..count]);
            assert!(bytes.len() < 65536);
            if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&bytes[..end]).to_lowercase();
                let length = headers
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length: "))
                    .unwrap()
                    .parse::<usize>()
                    .unwrap();
                if bytes.len() >= end + 4 + length {
                    assert!(!headers.contains("authorization:"));
                    let request: Value =
                        serde_json::from_slice(&bytes[end + 4..end + 4 + length]).unwrap();
                    assert_eq!(request["model"], "local-model");
                    break;
                }
            }
        }
        let body=serde_json::json!({"model":"local-model","answers":{"action":{"type":"choice","choice":"respond","confidence":1.0,"probabilities":{"respond":1.0,"clarify":0.0,"end_conversation":0.0,"none":0.0}}}}).to_string();
        write!(
            socket,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
        .unwrap();
    });
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_coder"))
        .env_clear()
        .current_dir(dir.path())
        .env("HOME", dir.path())
        .env("CODER_TRACE", "off")
        .env("CODER_DECISION_PROFILE", "direct_local")
        .env("CODER_DECISION_URL", url)
        .env("CODER_DECISION_MODEL", "local-model")
        .env("TYPESAFE_API_KEY", "fixture-must-not-be-forwarded")
        .args(["-p", "hello"])
        .output()
        .unwrap();
    server.join().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(stub_answer(&String::from_utf8_lossy(&output.stdout)));
}

/// A routed turn's decision call names the function it asked: the
/// trace records the same question-set identity, wording digest, and
/// gate a file-defined set's provenance carries on a program step.
#[test]
fn a_routed_turn_records_the_functions_provenance() {
    use std::io::{Read, Write};
    use std::time::{Duration, Instant};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    listener.set_nonblocking(true).unwrap();
    let server = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut socket = loop {
            match listener.accept() {
                Ok((socket, _)) => break socket,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "decision call was not made");
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("{error}"),
            }
        };
        socket.set_nonblocking(false).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut buffer = [0u8; 8192];
        let _ = socket.read(&mut buffer).unwrap();
        let body=serde_json::json!({"model":"local-model","answers":{"action":{"type":"choice","choice":"respond","confidence":1.0,"probabilities":{"respond":1.0,"clarify":0.0,"end_conversation":0.0,"none":0.0}}}}).to_string();
        write!(
            socket,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
        .unwrap();
    });
    let dir = tempfile::tempdir().unwrap();
    let trace = dir.path().join("turn.atif.jsonl");
    let output = Command::new(env!("CARGO_BIN_EXE_coder"))
        .env_clear()
        .current_dir(dir.path())
        .env("HOME", dir.path())
        .env("CODER_DECISION_PROFILE", "direct_local")
        .env("CODER_DECISION_URL", url)
        .env("CODER_DECISION_MODEL", "local-model")
        .args(["-p", "hello", "--trace"])
        .arg(&trace)
        .output()
        .unwrap();
    server.join().unwrap();
    assert!(output.status.success(), "{output:?}");

    let recording = atif::log::read(&trace).expect("the trace reads back");
    let document = recording.document();
    let call = document["steps"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|step| {
            step["tool_calls"]
                .as_array()
                .and_then(|calls| calls.first())
        })
        .find(|call| call["function_name"] == "classify")
        .expect("the turn asked the classify function");
    assert_eq!(call["extra"]["question_set"], "coder-turns-v2");
    assert_eq!(call["extra"]["gate"], "action");
    assert_eq!(call["extra"]["set_digest"].as_str().unwrap().len(), 64);
}

/// The hosted profile keeps its two identities on the wire: the bearer
/// credential travels in the `authorization` header, and the request
/// names the model the profile asked for — what a gateway revalidates
/// on every dispatch.
#[test]
fn hosted_profile_forwards_its_key_and_model() {
    use std::io::{Read, Write};
    use std::time::{Duration, Instant};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    listener.set_nonblocking(true).unwrap();
    let server = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut socket = loop {
            match listener.accept() {
                Ok((socket, _)) => break socket,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "decision call was not made");
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("{error}"),
            }
        };
        socket.set_nonblocking(false).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut bytes = Vec::new();
        let mut buffer = [0u8; 8192];
        loop {
            let count = socket.read(&mut buffer).unwrap();
            assert!(count > 0);
            bytes.extend_from_slice(&buffer[..count]);
            assert!(bytes.len() < 65536);
            if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&bytes[..end]).to_lowercase();
                let length = headers
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length: "))
                    .unwrap()
                    .parse::<usize>()
                    .unwrap();
                if bytes.len() >= end + 4 + length {
                    assert_eq!(
                        headers
                            .lines()
                            .find_map(|line| line.strip_prefix("authorization: ")),
                        Some("bearer oak_test.secret"),
                        "the profile's key did not reach the door: {headers}"
                    );
                    let request: Value =
                        serde_json::from_slice(&bytes[end + 4..end + 4 + length]).unwrap();
                    assert_eq!(request["model"], "hosted-model");
                    break;
                }
            }
        }
        let body=serde_json::json!({"model":"hosted-model","answers":{"action":{"type":"choice","choice":"respond","confidence":1.0,"probabilities":{"respond":1.0,"clarify":0.0,"end_conversation":0.0,"none":0.0}}}}).to_string();
        write!(
            socket,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
        .unwrap();
    });
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_coder"))
        .env_clear()
        .current_dir(dir.path())
        .env("HOME", dir.path())
        .env("CODER_TRACE", "off")
        .env("CODER_DECISION_PROFILE", "hosted_http")
        .env("CODER_DECISION_URL", url)
        .env("CODER_DECISION_MODEL", "hosted-model")
        .env("CODER_DECISION_KEY", "oak_test.secret")
        .args(["-p", "hello"])
        .output()
        .unwrap();
    server.join().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(stub_answer(&String::from_utf8_lossy(&output.stdout)));
}

/// A door that refuses permission does not stop the turn — program
/// selection degrades to an ordinary chat — but the degradation is
/// visible: the trace records the refusal the door answered, rather
/// than the turn silently carrying on.
#[test]
fn a_door_refusing_permission_degrades_visibly() {
    use std::io::{Read, Write};
    use std::time::{Duration, Instant};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    listener.set_nonblocking(true).unwrap();
    let server = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut socket = loop {
            match listener.accept() {
                Ok((socket, _)) => break socket,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "decision call was not made");
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("{error}"),
            }
        };
        socket.set_nonblocking(false).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut buffer = [0u8; 8192];
        let _ = socket.read(&mut buffer).unwrap();
        let body = r#"{"error":{"message":"the key may not reach this door"}}"#;
        write!(
            socket,
            "HTTP/1.1 403 Forbidden\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
        .unwrap();
    });
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_coder"))
        .env_clear()
        .current_dir(dir.path())
        .env("HOME", dir.path())
        .env("CODER_DECISION_PROFILE", "direct_local")
        .env("CODER_DECISION_URL", url)
        .env("CODER_DECISION_MODEL", "local-model")
        .args(["-p", "hello"])
        .output()
        .unwrap();
    server.join().unwrap();
    // The refusal degraded the turn to an ordinary chat, and the chat
    // still answered — the refusal narrowed what ran, never widened it.
    assert!(output.status.success(), "{output:?}");
    assert!(stub_answer(&String::from_utf8_lossy(&output.stdout)));
    // And the degradation is recorded where an operator reads it.
    let traces = dir.path().join(".openagents/traces");
    let recording = std::fs::read_dir(&traces)
        .unwrap()
        .filter_map(Result::ok)
        .find_map(|entry| std::fs::read_to_string(entry.path()).ok())
        .expect("a degraded turn still records its trace");
    assert!(
        recording.contains("no program selected"),
        "the trace does not show the refused selection: {recording}"
    );
    assert!(
        recording.contains("403"),
        "the trace does not name the door's answer: {recording}"
    );
}
