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
        // The command runs outside any Git checkout: a coding reply then
        // never starts Coder in this repository.
        let output = Command::new(exe)
            .args(&args)
            .current_dir(&home)
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
    // A terminal is on a computer; outside a checkout it names no project.
    assert_eq!(
        payloads.lock().unwrap()[0]["context"]["computer"]["place"],
        "here"
    );
    assert!(
        payloads.lock().unwrap()[0]["context"]
            .get("project")
            .is_none()
    );
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

    // A coding reply starts Coder at once, in the checkout the command
    // runs in; outside one, the command says so plainly and exits 1.
    let fix = run!("--json", "chat", "--local", "fix the flaky test");
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
            .contains("is not in a Git checkout"),
        "{coder}"
    );
    // `--no-run` keeps only the offer.
    let offered = run!(
        "--json",
        "chat",
        "--local",
        "--no-run",
        "fix the flaky test"
    );
    assert_eq!(offered.code, 0, "{}\n{}", offered.stdout, offered.stderr);
    assert_eq!(offered.event("offer")["offer"]["offer"], "run_coder");
    assert!(!offered.stdout.contains("\"event\":\"coder\""));
    // A thread that started no Coder task has nothing to follow or stop.
    let fixed = fix.event("result")["thread"].as_str().unwrap().to_owned();
    assert_eq!(
        run!("chat", "follow", "--local", "--thread", &fixed).code,
        1
    );
    assert_eq!(run!("chat", "stop", "--local", "--thread", &fixed).code, 1);
    assert_eq!(
        run!("chat", "answer", "--local", "--thread", &fixed).code,
        64
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

/// `openagents ARGS` as [`openagents`] runs it, from the folder `cwd`.
async fn openagents_in(cwd: &Path, home: &Path, relay: &str, worker: &str, args: &[&str]) -> Run {
    let exe = env!("CARGO_BIN_EXE_openagents");
    let args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
    let (cwd, home, relay, worker) = (
        cwd.to_owned(),
        home.to_owned(),
        relay.to_owned(),
        worker.to_owned(),
    );
    tokio::task::spawn_blocking(move || {
        let output = Command::new(exe)
            .args(&args)
            .current_dir(&cwd)
            .env("HOME", &home)
            .env("TMPDIR", home.join("tmp"))
            .env("PATH", "/usr/bin:/bin")
            .env_remove("OPENAGENTS_CHAT_HOME")
            .env_remove("OPENAGENTS_SETTINGS")
            .env_remove("OPENAGENTS_TASKS")
            .env_remove("CLAUDE_BIN")
            .env_remove("CODEX_HOME")
            .env_remove("GROK_BIN")
            .env_remove("DEVIN_BIN")
            .env_remove("OPENCODE_BIN")
            // An engine to name; every run here is refused before it starts.
            .env("OPENAGENTS_CODER_CONTROLLER", "/bin/echo")
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

/// `openagents settings` edits the one settings file (#10036), and each
/// setting a terminal can observe changes `openagents chat`'s local run:
/// in a checkout on a computer with no coding agent signed in, the
/// default run tries every coding agent (#10091, #10184), turning every
/// other agent off names
/// Claude Code alone, a project-folder setting refuses a checkout outside
/// it, and `ask_first` keeps only the offer. (What each setting does to a
/// run that starts is in `crates/coder/src/task/local.rs`.)
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn settings_change_the_local_run_and_the_defaults_change_nothing() {
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(home.path().join("tmp")).unwrap();
    let top = home.path().join("slugs");
    std::fs::create_dir_all(&top).unwrap();
    for args in [
        vec!["init", "-q"],
        vec![
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.invalid",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "one",
        ],
    ] {
        assert!(
            Command::new("git")
                .args(args)
                .current_dir(&top)
                .status()
                .unwrap()
                .success()
        );
    }
    let worker = SecretKey::from_byte_array([0x53; 32]).unwrap();
    let worker_hex = hex(&public(&worker).serialize());
    let (url, payloads) = relay(worker).await;
    macro_rules! run {
        ($($arg:expr),* $(,)?) => {
            openagents_in(&top, home.path(), &url, &worker_hex, &[$($arg),*]).await
        };
    }
    let file = home.path().join(".openagents/settings.json");

    // No file: every setting is its default.
    let shown = run!("--json", "settings", "show");
    assert_eq!(shown.code, 0, "{}", shown.stderr);
    let shown: Value = serde_json::from_str(&shown.stdout).unwrap();
    assert_eq!(shown["exists"], false);
    assert_eq!(
        shown["settings"],
        json!({
            "coder.providers": [],
            "coder.disabled": [],
            "coder.start": "at_once",
            "coder.usage_threshold_percent": 90,
            "coder.projects": [],
            "coder.access": "full",
            "coder.shadow": null,
            "coder.shadow_budget_usd": null,
            "models.payer": "ours",
        })
    );
    let coder_message = |run: &Run| {
        run.event("coder")["message"]
            .as_str()
            .unwrap_or_default()
            .to_owned()
    };

    // The defaults: Coder runs at once, on any coding agent signed in
    // here (#10091, #10184); OpenCode names no model in this home.
    let plain = run!("--json", "chat", "--local", "fix the flaky test");
    assert_eq!(plain.code, 1, "{}\n{}", plain.stdout, plain.stderr);
    assert!(
        coder_message(&plain).contains(
            "None of the coding agents Coder can use (Codex, Claude Code, Grok Build, Devin) \
             is signed in"
        ),
        "{}",
        plain.stdout
    );
    // The turn told the worker that this computer is where Coder runs, its
    // agents' readiness, and the checkout as the project folder (#10077).
    let context = payloads.lock().unwrap()[0]["context"].clone();
    assert_eq!(context["surface"], "terminal");
    assert_eq!(context["computer"]["place"], "here");
    // Grok Build, allowed but not installed in this home, is left out
    // (#10113).
    assert_eq!(
        context["computer"]["engines"],
        json!([{"engine": "codex", "state": "not_signed_in"},
               {"engine": "claude", "state": "not_signed_in"}])
    );
    assert_eq!(context["project"]["name"], "slugs");
    assert!(
        context["project"]["path"]
            .as_str()
            .is_some_and(|path| path.ends_with("/slugs")),
        "{context}"
    );

    // Claude Code only: every other agent turned off.
    let set = run!("settings", "set", "coder.providers", "claude");
    assert_eq!(set.code, 0, "{}", set.stderr);
    assert_eq!(set.stdout.trim(), "coder.providers claude");
    assert!(file.exists());
    for agent in ["codex", "grok", "devin", "opencode"] {
        let off = run!("settings", "disable", agent);
        assert_eq!(off.code, 0, "{}", off.stderr);
        assert!(off.stdout.contains("is off"), "{}", off.stdout);
    }
    assert_eq!(
        run!("settings", "get", "coder.disabled").stdout.trim(),
        "codex,grok,devin,opencode"
    );
    // The last agent cannot be turned off, and a name that is no agent
    // changes nothing.
    assert_eq!(run!("settings", "disable", "claude").code, 1);
    assert_eq!(run!("settings", "disable", "vertex").code, 1);
    let claude = run!("--json", "chat", "--local", "fix the flaky test");
    assert_eq!(claude.code, 1);
    assert!(
        coder_message(&claude).starts_with(
            "Claude Code is not signed in on this computer, and every other coding agent is \
             turned off in your settings."
        ),
        "{}",
        claude.stdout
    );

    // Project folders that do not hold this checkout.
    let elsewhere = home.path().join("code");
    std::fs::create_dir_all(&elsewhere).unwrap();
    assert_eq!(
        run!(
            "settings",
            "set",
            "coder.projects",
            elsewhere.to_str().unwrap()
        )
        .code,
        0
    );
    let outside = run!("--json", "chat", "--local", "fix the flaky test");
    assert_eq!(outside.code, 1);
    assert!(
        coder_message(&outside).contains("is not in one of your project folders"),
        "{}",
        outside.stdout
    );

    // Ask first: the reply offers Coder and nothing runs.
    assert_eq!(run!("settings", "set", "coder.start", "ask_first").code, 0);
    let asked = run!("--json", "chat", "--local", "fix the flaky test");
    assert_eq!(asked.code, 0, "{}\n{}", asked.stdout, asked.stderr);
    assert_eq!(asked.event("offer")["offer"]["offer"], "run_coder");
    assert!(!asked.stdout.contains("\"event\":\"coder\""));
    // `--run-coder` still runs at once, into the same refusals.
    let forced = run!(
        "--json",
        "chat",
        "--local",
        "--run-coder",
        "fix the flaky test"
    );
    assert_eq!(forced.code, 1);
    assert!(coder_message(&forced).contains("is not in one of your project folders"));

    // Bad values change nothing; unknown keys are usage errors.
    assert_eq!(run!("settings", "set", "coder.access", "root").code, 1);
    assert_eq!(run!("settings", "set", "coder.providers", "vertex").code, 1);
    assert_eq!(run!("settings", "get", "coder.model").code, 64);
    assert_eq!(run!("settings", "frob").code, 64);
    assert_eq!(
        run!("settings", "set", "coder.access", "toolchains").code,
        0
    );
    assert_eq!(
        run!("settings", "set", "coder.usage_threshold_percent", "off").code,
        0
    );
    let saved: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    assert_eq!(saved["schema"], "openagents.settings.v1");
    assert_eq!(saved["coder"]["access"], "toolchains");
    assert_eq!(saved["coder"]["usage_threshold_percent"], Value::Null);

    // Every setting unset: the run is the default one again.
    assert_eq!(run!("settings", "enable", "codex").code, 0);
    for key in [
        "coder.providers",
        "coder.disabled",
        "coder.start",
        "coder.usage_threshold_percent",
        "coder.projects",
        "coder.access",
    ] {
        assert_eq!(run!("settings", "unset", key).code, 0, "{key}");
    }
    let again = run!("--json", "chat", "--local", "fix the flaky test");
    assert_eq!(again.code, 1);
    assert_eq!(coder_message(&again), coder_message(&plain));

    // A broken file is named, never read as the defaults.
    std::fs::write(&file, "{").unwrap();
    let broken = run!("--json", "chat", "--local", "fix the flaky test");
    assert_eq!(broken.code, 0, "a broken file asks first");
    assert!(!broken.stdout.contains("\"event\":\"coder\""));
    let accepted = run!(
        "--json",
        "chat",
        "--local",
        "--run-coder",
        "fix the flaky test"
    );
    assert!(
        coder_message(&accepted).contains("settings.json are not valid"),
        "{}",
        accepted.stdout
    );
    assert_eq!(run!("settings", "show").code, 1);
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

/// A coding request runs Coder on this computer with no host: the owner's
/// own Codex or Claude Code login, a scratch Python checkout, and the
/// public chat worker. Needs `cargo build -p microcoder` (the engine beside
/// the `openagents` binary) and a signed-in Codex or Claude Code.
/// `cargo test -p openagents-cli --test chat -- --ignored live_chat_runs --nocapture`
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "live: runs Coder with this computer's own Codex or Claude Code login"]
async fn live_chat_runs_coder_on_this_computer() {
    let scratch = tempfile::tempdir().unwrap();
    let top = scratch.path().join("slugs");
    std::fs::create_dir_all(&top).unwrap();
    std::fs::write(
        top.join("slugs.py"),
        "import re\n\n\ndef slugify(text):\n    return \"-\".join(re.findall(r\"[a-z0-9]+\", text.lower()))\n",
    )
    .unwrap();
    for args in [
        vec!["init", "-q"],
        vec!["add", "slugs.py"],
        vec![
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.invalid",
            "commit",
            "-qm",
            "one",
        ],
    ] {
        assert!(
            Command::new("git")
                .args(args)
                .current_dir(&top)
                .status()
                .unwrap()
                .success()
        );
    }
    let exe = env!("CARGO_BIN_EXE_openagents");
    let output = Command::new(exe)
        .args(["--json", "chat", "--scratch", "add a unit test for slugify"])
        .current_dir(&top)
        .env_remove("OPENAGENTS_CHAT_RELAY")
        .env_remove("OPENAGENTS_CHAT_WORKER")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    eprintln!("{stdout}{}", String::from_utf8_lossy(&output.stderr));
    let events: Vec<Value> = stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect();
    let named = |name: &str| {
        events
            .iter()
            .find(|event| event["event"] == name && event.get("seq").is_some())
    };
    let started = named("coder_started").expect("coder_started");
    assert!(["codex", "claude"].contains(&started["provider"].as_str().unwrap()));
    assert!(named("step").is_some() && named("progress").is_some());
    let result = named("result").expect("a result");
    let worktree = std::path::PathBuf::from(result["worktree"].as_str().unwrap());
    assert!(
        !result["files_changed"].as_array().unwrap().is_empty(),
        "{result}"
    );
    for file in result["files_changed"].as_array().unwrap() {
        let path = file["path"].as_str().unwrap();
        assert!(worktree.join(path).exists());
        assert!(!top.join(path).exists(), "the checkout is never written");
    }
    assert!(output.status.success());
}
