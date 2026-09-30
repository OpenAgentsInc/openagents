//! `openagents chat` end to end, against a local NIP-42 relay with a
//! scripted chat worker behind it, the way `openagents-chat`'s own
//! controlled-stream tests run. The program runs as a child process with a
//! temporary HOME, so no real identity, store, or host is touched.
//!
//! One live test, ignored by default, sends a turn to the public chat
//! worker with `--scratch`:
//!
//! ```sh
//! cargo test -p openagents-cli --test chat -- --ignored --nocapture
//! ```

use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use nostr::domain::{Event, RelaySigner, Tag};
use nostr::kinds::{CJ_CONVERSATION_FEEDBACK, CJ_CONVERSATION_REQUEST, CJ_CONVERSATION_RESULT};
use nostr::nip44;
use secp256k1::SecretKey;
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::Message;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn public(secret: &SecretKey) -> secp256k1::XOnlyPublicKey {
    secp256k1::Keypair::from_secret_key(&secp256k1::Secp256k1::new(), secret)
        .x_only_public_key()
        .0
}

/// What the scripted worker answers to one request: feedback payloads in
/// order, then the result payload.
fn script(payload: &Value) -> (Vec<Value>, Value) {
    let task = payload["task"].as_str().unwrap_or_default().to_owned();
    let turns = payload["transcript"].as_array().map_or(0, Vec::len);
    let judgment = |lane: &str, route: &str, tier: &str| json!({"v": 2, "type": "judgment", "lane": lane, "route": route, "tier": tier, "model": "jev-fixture"});
    if task.contains("connect a phone") {
        // A prepared answer from product knowledge, cited the way an older
        // worker cited it; the citation never shows.
        return (
            vec![judgment("chat", "answer", "canned")],
            json!({"v": 2, "type": "result", "model": "bank:openagents",
                "tier": "canned", "answer": "openagents.connect-phone@1", "route": "answer",
                "text": "Open the desktop app and scan its QR code with your phone [openagents.connect-phone@1].",
                "followups": [{"label": "What can Coder do?"}]}),
        );
    }
    if task.contains("flaky test") {
        return (
            vec![
                judgment("computer", "coder", "model"),
                json!({"v": 2, "type": "offer", "offer": "run_coder", "label": "Run Coder", "target": "connected_computer"}),
            ],
            json!({"v": 2, "type": "result", "model": "fixture-model", "tier": "model",
                "text": "Coder can fix that on your computer."}),
        );
    }
    (
        vec![
            judgment("chat", "general", "model"),
            json!({"v": 2, "type": "partial", "seq": 0, "delta": "Rain on "}),
            json!({"v": 2, "type": "partial", "seq": 1, "delta": "the roof"}),
        ],
        json!({"v": 2, "type": "result", "model": "fixture-model", "tier": "model",
            "text": format!("Rain on the roof. (turns: {turns})")}),
    )
}

/// A relay with the scripted chat worker behind it: it asks for NIP-42,
/// answers a subscription with `EOSE`, and answers each conversation
/// request with the worker's signed feedback and result. Returns its URL
/// and each request's decrypted payload.
async fn relay(worker: SecretKey) -> (String, Arc<Mutex<Vec<Value>>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let payloads = Arc::new(Mutex::new(vec![]));
    let seen = payloads.clone();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let seen = seen.clone();
            tokio::spawn(async move {
                let Ok(mut socket) = tokio_tungstenite::accept_async(stream).await else {
                    return;
                };
                let send = |value: Value| Message::Text(value.to_string().into());
                let _ = socket.send(send(json!(["AUTH", "challenge"]))).await;
                let mut subscription = String::new();
                let mut authed = false;
                while let Some(Ok(Message::Text(text))) = socket.next().await {
                    let frame: Value = serde_json::from_str(&text).unwrap();
                    match frame[0].as_str() {
                        Some("AUTH") => {
                            assert_eq!(frame[1]["kind"], 22242, "NIP-42 auth event");
                            authed = true;
                            let id = frame[1]["id"].clone();
                            let _ = socket.send(send(json!(["OK", id, true, ""]))).await;
                        }
                        Some("REQ") => {
                            subscription = frame[1].as_str().unwrap().to_owned();
                            let _ = socket.send(send(json!(["EOSE", subscription]))).await;
                        }
                        Some("EVENT") => {
                            assert!(authed, "the client signs in before it publishes");
                            let request: Event = serde_json::from_value(frame[1].clone()).unwrap();
                            assert_eq!(request.kind, CJ_CONVERSATION_REQUEST);
                            let _ = socket.send(send(json!(["OK", request.id, true, ""]))).await;
                            let bytes: [u8; 32] = (0..32)
                                .map(|at| {
                                    u8::from_str_radix(&request.pubkey[at * 2..at * 2 + 2], 16)
                                        .unwrap()
                                })
                                .collect::<Vec<u8>>()
                                .try_into()
                                .unwrap();
                            let from = secp256k1::XOnlyPublicKey::from_byte_array(bytes).unwrap();
                            let key = nip44::conversation_key(&worker, &from);
                            let payload: Value = serde_json::from_str(
                                &nip44::decrypt(&request.content, &key).unwrap(),
                            )
                            .unwrap();
                            seen.lock().unwrap().push(payload.clone());
                            let signer =
                                RelaySigner::from_secret_hex(&worker.display_secret().to_string())
                                    .unwrap();
                            let (feedback, result) = script(&payload);
                            let answers = feedback
                                .into_iter()
                                .map(|body| (CJ_CONVERSATION_FEEDBACK, body))
                                .chain(std::iter::once((CJ_CONVERSATION_RESULT, result)));
                            for (kind, body) in answers {
                                let event = signer.sign(
                                    request.created_at,
                                    kind,
                                    vec![
                                        Tag::new(vec!["e".into(), request.id.clone()]),
                                        Tag::new(vec!["p".into(), request.pubkey.clone()]),
                                    ],
                                    nip44::encrypt(&body.to_string(), &key, [9; 32]).unwrap(),
                                );
                                let _ = socket
                                    .send(send(json!(["EVENT", subscription, event])))
                                    .await;
                                // Partials arrive over time, as a model streams.
                                tokio::time::sleep(Duration::from_millis(250)).await;
                            }
                        }
                        _ => {}
                    }
                }
            });
        }
    });
    (url, payloads)
}

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

impl Run {
    fn events(&self) -> Vec<Value> {
        self.stdout
            .lines()
            .map(|line| serde_json::from_str(line).unwrap_or_else(|_| panic!("NDJSON: {line}")))
            .collect()
    }

    fn event(&self, name: &str) -> Value {
        self.events()
            .into_iter()
            .find(|event| event["event"] == name)
            .unwrap_or_else(|| panic!("no {name} event in {}", self.stdout))
    }
}

/// `openagents ARGS` with a temporary HOME and the fixture relay and worker.
async fn openagents(home: &Path, relay: &str, worker: &str, args: &[&str]) -> Run {
    let exe = env!("CARGO_BIN_EXE_openagents");
    let args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
    let (home, relay, worker) = (home.to_owned(), relay.to_owned(), worker.to_owned());
    tokio::task::spawn_blocking(move || {
        let output = Command::new(exe)
            .args(&args)
            .env("HOME", &home)
            .env("TMPDIR", home.join("tmp"))
            .env_remove("OPENAGENTS_CHAT_HOME")
            .env_remove("XDG_RUNTIME_DIR")
            .env("OPENAGENTS_CHAT_RELAY", &relay)
            .env("OPENAGENTS_CHAT_WORKER", &worker)
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap();
        Run {
            code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    })
    .await
    .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn chat_streams_routes_continues_threads_and_exports_atif() {
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(home.path().join("tmp")).unwrap();
    let worker = SecretKey::from_byte_array([0x52; 32]).unwrap();
    let worker_hex = hex(&public(&worker).serialize());
    let (url, payloads) = relay(worker).await;
    macro_rules! run {
        ($($arg:expr),* $(,)?) => {
            openagents(home.path(), &url, &worker_hex, &[$($arg),*]).await
        };
    }

    // A prepared knowledge answer, in a scratch thread: no citation shows.
    let phone = run!("--json", "chat", "--scratch", "How do I connect a phone");
    assert_eq!(phone.code, 0, "{}\n{}", phone.stdout, phone.stderr);
    let accepted = phone.event("accepted");
    assert_eq!(accepted["backend"], "scratch");
    let scratch = accepted["thread"].as_str().unwrap().to_owned();
    let route = phone.event("route");
    assert_eq!(route["served_answer"], "openagents.connect-phone@1");
    assert_eq!(route["tier"], "canned");
    assert_eq!(route["judgment"]["model"], "jev-fixture");
    assert_eq!(route["followups"][0]["label"], "What can Coder do?");
    let result = phone.event("result");
    assert_eq!(result["thread"], scratch.as_str());
    let text = result["text"].as_str().unwrap();
    assert!(
        text.contains("QR code") && !text.contains("[openagents."),
        "{text}"
    );
    assert_eq!(
        payloads.lock().unwrap()[0]["context"]["surface"],
        "terminal"
    );
    assert_eq!(payloads.lock().unwrap()[0]["client"], "openagents-cli");
    // The scratch store is its own, and nothing was written under HOME.
    assert!(!home.path().join(".openagents").exists());

    // A streamed general answer, in this command's own store.
    let haiku = run!("--json", "chat", "--local", "Write a haiku about rain");
    assert_eq!(haiku.code, 0, "{}\n{}", haiku.stdout, haiku.stderr);
    let events = haiku.events();
    let names: Vec<&str> = events.iter().filter_map(|e| e["event"].as_str()).collect();
    assert_eq!(names.first(), Some(&"accepted"));
    assert!(names.contains(&"partial"), "{names:?}");
    assert_eq!(&names[names.len() - 2..], ["route", "result"]);
    assert_eq!(haiku.event("accepted")["backend"], "in_process");
    assert_eq!(haiku.event("route")["route"], "general");
    assert_eq!(
        haiku.event("result")["text"],
        "Rain on the roof. (turns: 1)"
    );
    assert_eq!(haiku.event("result")["model"], "fixture-model");
    let thread = haiku.event("accepted")["thread"]
        .as_str()
        .unwrap()
        .to_owned();

    // A follow-up in the same thread carries its context to the worker.
    let again = run!(
        "--json",
        "chat",
        "send",
        "--local",
        "--thread",
        &thread,
        "Another one"
    );
    assert_eq!(again.code, 0, "{}\n{}", again.stdout, again.stderr);
    assert_eq!(again.event("accepted")["new"], false);
    assert_eq!(
        again.event("result")["text"],
        "Rain on the roof. (turns: 3)"
    );
    let last = payloads.lock().unwrap().last().unwrap().clone();
    assert_eq!(last["transcript"][0]["content"], "Write a haiku about rain");
    assert_eq!(last["transcript"][2]["content"], "Another one");

    // The thread as its ATIF trajectory, read back by `crates/atif`.
    let export = run!("--json", "chat", "export", "--local", "--thread", &thread);
    assert_eq!(export.code, 0, "{}", export.stderr);
    let mut document: Value = serde_json::from_str(&export.stdout).unwrap();
    assert!(atif::validate(&document).is_empty());
    assert_eq!(atif::upgrade(&mut document).unwrap(), "ATIF-v1.8");
    assert_eq!(document["session_id"], thread.as_str());
    let steps = document["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 4, "one step per turn");
    assert_eq!(
        steps[1]["tool_calls"][0]["extra"]["schema"],
        atif::DECISION_CALL_SCHEMA
    );
    assert_eq!(steps[1]["tool_calls"][0]["extra"]["route"], "general");
    assert_eq!(steps[3]["model_name"], "fixture-model");

    // The scratch thread reads back, with its served answer.
    let export = run!(
        "--json",
        "chat",
        "export",
        "--scratch",
        "--thread",
        &scratch
    );
    assert_eq!(export.code, 0, "{}", export.stderr);
    let document: Value = serde_json::from_str(&export.stdout).unwrap();
    assert_eq!(document["session_id"], scratch.as_str());
    assert_eq!(
        document["steps"][1]["extra"]["served_answer"],
        "openagents.connect-phone@1"
    );

    // The list and the reader see the same thread.
    let list = run!("--json", "chat", "threads", "--local");
    let list: Value = serde_json::from_str(&list.stdout).unwrap();
    assert_eq!(list["threads"][0]["thread"], thread.as_str());
    assert_eq!(list["threads"][0]["title"], "Write a haiku about rain");
    let read: Value = serde_json::from_str(
        &run!("--json", "chat", "read", "--local", "--thread", &thread).stdout,
    )
    .unwrap();
    assert_eq!(read["turns"].as_array().unwrap().len(), 4);

    // A Coder offer is shown; without a host it is not accepted, and the
    // command says so.
    let fix = run!(
        "--json",
        "chat",
        "--local",
        "--run-coder",
        "fix the flaky test"
    );
    assert_eq!(fix.code, 1, "{}\n{}", fix.stdout, fix.stderr);
    assert!(fix.stdout.contains("\"event\":\"result\""));
    let offer = fix.event("offer");
    assert_eq!(offer["offer"]["offer"], "run_coder");
    assert!(
        offer["accept"]
            .as_str()
            .unwrap()
            .contains("chat run-coder --thread")
    );
    let coder = fix.event("coder");
    assert_eq!(coder["accepted"], false);
    assert!(
        coder["message"]
            .as_str()
            .unwrap()
            .contains("no Coder broker")
    );

    // Text mode: the reply on stdout, the thread on stderr.
    let text = run!("chat", "--local", "Write a haiku about rain");
    assert_eq!(text.code, 0, "{}", text.stderr);
    assert_eq!(text.stdout.trim(), "Rain on the roof. (turns: 1)");
    assert!(text.stderr.contains("thread "), "{}", text.stderr);

    // Usage errors are 64.
    assert_eq!(run!("chat", "read").code, 64);
    assert_eq!(run!("chat", "--thread", "nope", "hi").code, 64);
    // An unknown thread fails with 1.
    let missing = run!(
        "chat",
        "read",
        "--local",
        "--thread",
        "0123456789abcdef0123456789abcdef"
    );
    assert_eq!(missing.code, 1);
    // No key is ever printed.
    let key = std::fs::read_to_string(home.path().join(".openagents/chat/device.key")).unwrap();
    for run in [&haiku, &again, &fix, &text] {
        assert!(!run.stdout.contains(key.trim()) && !run.stderr.contains(key.trim()));
    }
}

/// The public chat worker, from a throwaway identity, the way the smoke in
/// `docs/deployment/chat-worker.md` runs it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "network: needs the chat worker on the public relay"]
async fn live_chat_answers_from_product_knowledge() {
    let home = tempfile::tempdir().unwrap();
    let exe = env!("CARGO_BIN_EXE_openagents");
    let output = Command::new(exe)
        .args(["--json", "chat", "--scratch", "How do I connect a phone"])
        .env("HOME", home.path())
        .env_remove("OPENAGENTS_CHAT_RELAY")
        .env_remove("OPENAGENTS_CHAT_WORKER")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    eprintln!("{stdout}{}", String::from_utf8_lossy(&output.stderr));
    assert!(output.status.success());
    let result = stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find(|event| event["event"] == "result")
        .expect("a result");
    let text = result["text"].as_str().unwrap();
    assert!(
        text.contains("QR") && !text.contains("[openagents."),
        "{text}"
    );
}
