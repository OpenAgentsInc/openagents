//! `openagents studio` against a simulated scratch host (#10566, #10572): a
//! host with its access store, host root, control socket, and task store
//! under a temporary directory, the simulated team's scratch repository as
//! its one workspace, and a temporary `HOME` for every command. Nothing
//! here reaches the person's own host, home, or relays.
//!
//! The host's turns are the scripted engine's
//! (`coder::task::studio_sim::Engine`): it ends each studio task's turn
//! from the simulated team's script through the task owner, with no model
//! and no spend, so the coordinator, the lead's reviews, the merges, and
//! the conflict flow all run on the host's own paths. The person's side is
//! only `openagents studio` commands, each under `--json`: `seat set`,
//! `goal submit`, `message`, `tasks`, `decisions`, `answer` (the lead's
//! question and the release's approval), `sync`, `seat pause` and
//! `resume`, `review`, `request-changes`, `merge` (a stale one refused, a
//! conflicting one sent back to its seat, and three that land in the
//! checkout), `status`, and `watch`, and a refused intent prints the
//! host's code. Every task the test creates is archived through the host
//! when it ends, pass or fail.
#![cfg(unix)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use coder::task::{Status, Store, studio_sim};
use coder_access::{Operation, RelayPolicy};
use coder_host::config::{Config, Control, Iroh};
use openagents_connect::control::{self, Op, Reply, Request};
use secp256k1::SecretKey;
use serde_json::Value;
use sha2::{Digest, Sha256};

#[path = "../../coder-control/src/tests/relay.rs"]
#[allow(dead_code)]
mod relay;

const POLICY: RelayPolicy = RelayPolicy::LoopbackTest;
/// How long the studio may take to show what a step did.
const WAIT: Duration = Duration::from_secs(90);
/// How often the scripted engine looks for a turn to end.
const ENGINE_EVERY: Duration = Duration::from_millis(150);

/// A scratch host and everything it keeps.
struct Host {
    _temp: tempfile::TempDir,
    runtime: tokio::runtime::Runtime,
    running: Option<coder_host::Running>,
    engine: Option<studio_sim::Running>,
    _relay: tokio::task::JoinHandle<()>,
    socket: PathBuf,
    root: PathBuf,
    store: PathBuf,
    home: PathBuf,
    checkout: PathBuf,
}

/// What one `openagents` command printed and how it exited.
struct Ran {
    code: i32,
    stdout: String,
    stderr: String,
}

impl Ran {
    /// The one JSON document a successful command printed.
    fn json(&self) -> Value {
        assert_eq!(
            self.code, 0,
            "the command failed:\n{}\n{}",
            self.stdout, self.stderr
        );
        self.document()
    }

    /// The one JSON document the command printed, whatever its exit.
    fn document(&self) -> Value {
        serde_json::from_str(self.stdout.trim())
            .unwrap_or_else(|error| panic!("not one JSON document ({error}):\n{}", self.stdout))
    }
}

fn host() -> Host {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .expect("a runtime");
    let temp = tempfile::tempdir().expect("a scratch directory");
    let home = temp.path().join("home");
    std::fs::create_dir_all(&home).expect("a scratch home");
    let scratch =
        studio_sim::Scratch::create(&temp.path().join("s")).expect("a simulated scratch host");
    let checkout = scratch.fixture.checkout.clone();
    let store = scratch.store.clone();
    let root = scratch.root.clone();
    let socket = temp.path().join("c/control.sock");
    let workspaces = BTreeMap::from([(studio_sim::WORKSPACE.to_owned(), checkout.clone())]);
    let (running, relay) = runtime.block_on(async {
        let (relay, task, _) = relay::start().await;
        let access = temp.path().join("access");
        let owner = SecretKey::new(&mut secp256k1::rand::rng());
        coder_host::access::host::Host::new(&access, POLICY)
            .init(&coder_host::reach::pubkey(&owner))
            .expect("the access store");
        let mut config = Config::new(access, vec![relay], 3);
        config.policy = POLICY;
        config.iroh = Some(Iroh::loopback());
        config.control = Some(Control {
            path: socket.clone(),
            root: root.clone(),
            autostart: None,
            tasks: store.clone(),
            uid: coder_host::control::own_uid(),
        });
        config.workspaces = workspaces.clone();
        // The scratch host's inbox: studio intents give each task its own
        // worktree under the root, and nothing starts a real engine.
        let tasks =
            Arc::new(studio_sim::inbox(&store, &root, &workspaces)) as Arc<dyn coder_host::Tasks>;
        let running = coder_host::start(config, tasks).await.expect("the host");
        (running, task)
    });
    let engine = studio_sim::Engine::open(&root, &store)
        .expect("the scripted engine")
        .spawn(ENGINE_EVERY);
    Host {
        _temp: temp,
        runtime,
        running: Some(running),
        engine: Some(engine),
        _relay: relay,
        socket,
        root,
        store,
        home,
        checkout,
    }
}

/// A fresh 64-hex request identity.
fn mint() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let seed = format!(
        "studio-host-test-{nanos}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    Sha256::digest(seed.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

impl Host {
    /// Runs `openagents --json studio ARGS` against this host, with the
    /// scratch home and stores.
    fn studio(&self, args: &[&str]) -> Ran {
        let output = Command::new(env!("CARGO_BIN_EXE_openagents"))
            .env_clear()
            .env("HOME", &self.home)
            .env("PATH", "/usr/bin:/bin")
            .current_dir(&self.home)
            .arg("--json")
            .arg("studio")
            .args(args)
            .arg("--control-socket")
            .arg(&self.socket)
            .arg("--tasks")
            .arg(&self.store)
            .arg("--root")
            .arg(&self.root)
            .output()
            .expect("openagents runs");
        Ran {
            code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }

    /// Runs `studio ARGS` until `test` holds of its JSON, or fails naming
    /// `what`.
    fn until(&self, args: &[&str], what: &str, mut test: impl FnMut(&Value) -> bool) -> Value {
        let deadline = Instant::now() + WAIT;
        loop {
            let value = self.studio(args).json();
            if test(&value) {
                return value;
            }
            assert!(
                Instant::now() < deadline,
                "the studio never showed {what}; it last said {value}"
            );
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    /// The open decision of `kind` that `task` asks, once the studio shows
    /// it: its identity and text.
    fn decision(&self, task: &str, kind: &str) -> (String, String) {
        let open = self.until(&["decisions"], &format!("a {kind} from {task}"), |open| {
            find_decision(open, task, kind).is_some()
        });
        let found = find_decision(&open, task, kind).expect("the decision");
        (
            found["decision"].as_str().expect("its identity").to_owned(),
            found["text"].as_str().unwrap_or_default().to_owned(),
        )
    }

    /// The task's review now: its tree and its diff.
    fn review(&self, task: &str) -> (String, String) {
        let review = self.studio(&["review", task, "--diff"]).json();
        let review = &review["review"];
        (
            review["head"]
                .as_str()
                .expect("the reviewed tree")
                .to_owned(),
            review["diff"].as_str().unwrap_or_default().to_owned(),
        )
    }

    /// Waits until `entry`'s change waits on the merge decision at a tree
    /// other than `not`, and returns that tree. The change must show the
    /// same tree on two reads in a row: a turn that just ended reads as
    /// done for the moment before the coordinator sends it to the lead's
    /// review.
    fn ready(&self, goal: &str, entry: &str, task_id: &str, not: Option<&str>) -> String {
        let mut head = String::new();
        let mut seen: Option<String> = None;
        self.until(
            &["tasks", goal],
            &format!("`{entry}` ready to merge"),
            |tasks| {
                if !task(tasks, goal, entry).is_some_and(|task| task["status"] == "done") {
                    seen = None;
                    return false;
                }
                let now = self.review(task_id).0;
                if not == Some(now.as_str()) {
                    seen = None;
                    return false;
                }
                let stable = seen.as_deref() == Some(now.as_str());
                seen = Some(now.clone());
                head = now;
                stable
            },
        );
        head
    }

    /// One NIP-HOST operation on the control socket, as the test's own
    /// cleanup sends it.
    fn control(&self, operation: Operation) -> Result<(), String> {
        let socket = self.socket.clone();
        let reply: Result<Reply, String> = self.runtime.block_on(async move {
            let mut stream = match tokio::net::UnixStream::connect(&socket).await {
                Ok(stream) => stream,
                Err(error) => return Err(error.to_string()),
            };
            let request = Request::new(
                1,
                Op::Task {
                    request: mint(),
                    operation,
                },
            );
            control::call(&mut stream, &request)
                .await
                .map_err(|error| error.to_string())
        });
        let reply = reply?;
        match reply {
            Reply::Task { .. } => Ok(()),
            Reply::Refused { code, message } => Err(format!("{code}: {message}")),
            other => Err(format!("an unexpected reply: {other:?}")),
        }
    }

    /// Cancels every task still open and archives every task in the store,
    /// through the host, as `task.cancel` and `task.archive` do.
    fn archive(&self) -> Result<usize, String> {
        let tasks = Store::open(&self.store)
            .and_then(|store| store.list())
            .map_err(|error| format!("{error:?}"))?;
        for task in &tasks {
            if matches!(
                task.status,
                Status::Queued | Status::Running | Status::CancelRequested
            ) {
                self.control(Operation::CancelTask {
                    task: task.task_id.clone(),
                    revision: task.revision,
                    reason: "The studio test ended.".into(),
                })
                .map_err(|error| format!("cancel {}: {error}", task.task_id))?;
            }
            self.control(Operation::ArchiveTask {
                task: task.task_id.clone(),
            })
            .map_err(|error| format!("archive {}: {error}", task.task_id))?;
        }
        Ok(tasks.len())
    }

    /// Stops the scripted engine, so no turn starts while the test cleans
    /// up.
    fn stop_engine(&mut self) {
        if let Some(engine) = self.engine.take() {
            engine.stop();
        }
    }

    fn shutdown(&mut self) {
        if let Some(running) = self.running.take() {
            self.runtime.block_on(running.shutdown());
        }
    }

    /// The checkout's file at `path`, or empty.
    fn checked_out(&self, path: &str) -> String {
        std::fs::read_to_string(self.checkout.join(path)).unwrap_or_default()
    }
}

/// The goal's task for plan entry `entry`, when the list holds it.
fn task<'a>(tasks: &'a Value, goal: &str, entry: &str) -> Option<&'a Value> {
    tasks["tasks"]
        .as_array()?
        .iter()
        .find(|task| task["goal"] == goal && task["entry"] == entry)
}

/// The open decision of `kind` that task `task` asks.
fn find_decision<'a>(open: &'a Value, task: &str, kind: &str) -> Option<&'a Value> {
    open["decisions"]
        .as_array()?
        .iter()
        .find(|decision| decision["task"] == task && decision["kind"] == kind)
}

/// The goal's task identity for plan entry `entry`, once it is released.
fn task_id(host: &Host, goal: &str, entry: &str) -> String {
    let tasks = host.until(&["tasks", goal], &format!("`{entry}` released"), |tasks| {
        task(tasks, goal, entry).is_some_and(|task| task["status"] != "held")
    });
    task(&tasks, goal, entry)
        .and_then(|task| task["task"].as_str())
        .expect("the task's identity")
        .to_owned()
}

/// Merges `task` at `head` and returns the command's JSON and exit code.
fn merge(host: &Host, task: &str, head: &str) -> (i32, Value) {
    let ran = host.studio(&["merge", task, "--head", head]);
    let code = ran.code;
    (code, ran.document())
}

fn run(host: &Host) {
    // Seats, set locally as the owner sets them: the script's lead and
    // workers.
    for (name, role) in [("lead", "lead"), ("ada", "worker"), ("grace", "worker")] {
        let seat = host
            .studio(&[
                "seat",
                "set",
                name,
                "--role",
                role,
                "--route",
                "codex:gpt-6-luna",
            ])
            .json();
        assert_eq!(seat["seat"]["name"], name);
    }
    let status = host.until(&["status"], "the seats", |status| {
        status["view"]["seats"]
            .as_array()
            .is_some_and(|seats| seats.len() == 3)
    });
    assert!(status["stream"].is_string());

    // The goal goes through the host, which names its lead's task.
    let submitted = host
        .studio(&[
            "goal",
            "submit",
            studio_sim::GOAL,
            "--workspace",
            studio_sim::WORKSPACE,
        ])
        .json();
    assert_eq!(submitted["operation"], "studio.goal.submit");
    let goal = submitted["goal_id"]
        .as_str()
        .expect("the goal's identity")
        .to_owned();
    let lead = task_id(host, &goal, "lead");

    // A message to a seat goes through the host.
    let sent = host
        .studio(&["message", "@grace", studio_sim::STEER])
        .json();
    assert_eq!(sent["operation"], "studio.seat.message");

    // The lead asks the person a question; the answer starts its next
    // turn, which plans the goal.
    let (question, text) = host.decision(&lead, "question");
    assert!(text.contains("Which greeting"), "{text}");
    let answered = host
        .studio(&["answer", &question[..12], studio_sim::ANSWER])
        .json();
    assert_eq!(answered["operation"], "studio.decision.answer");
    host.studio(&["sync"]).json();
    let greet = task_id(host, &goal, "greet");
    let docs = task_id(host, &goal, "docs");

    // Steering through the host: pause and resume a seat.
    let paused = host.studio(&["seat", "pause", "grace"]).json();
    assert_eq!(paused["operation"], "studio.seat.pause");
    let resumed = host.studio(&["seat", "resume", "@grace"]).json();
    assert_eq!(resumed["operation"], "studio.seat.resume");

    // A refused intent prints the host's code and message.
    let refused = host.studio(&["seat", "pause", "nobody"]);
    assert_eq!(refused.code, 1, "{}", refused.stderr);
    let refusal = refused.document();
    assert_eq!(refusal["operation"], "studio.seat.pause");
    assert!(
        refusal["code"]
            .as_str()
            .is_some_and(|code| !code.is_empty()),
        "{refusal}"
    );
    assert!(
        refused.stderr.contains("studio.seat.pause"),
        "{}",
        refused.stderr
    );

    // The greeting, reviewed by the lead, lands in the checkout.
    let head = host.ready(&goal, "greet", &greet, None);
    let (_, diff) = host.review(&greet);
    assert!(diff.contains("Hello, studio"), "{diff}");
    let moved = host.studio(&["merge", &greet, "--head", &"0".repeat(40)]);
    assert_eq!(moved.code, 1, "{}", moved.stderr);
    assert_eq!(moved.document()["code"], "stale");
    let (code, landed) = merge(host, &greet, &head);
    assert_eq!(code, 0, "{landed}");
    assert_eq!(landed["merged"]["publication"]["state"], "published");
    assert_eq!(host.checked_out("greeting.txt"), "Hello, studio\n");

    // The person asks for a change to the documentation; a merge at the
    // earlier review is then refused as stale.
    let first = host.ready(&goal, "docs", &docs, None);
    let sent = host
        .studio(&[
            "request-changes",
            &docs,
            studio_sim::CHANGES,
            "--head",
            &first,
        ])
        .json();
    assert_eq!(sent["merged"]["verdict"], "request_changes");
    let changed = host.ready(&goal, "docs", &docs, Some(&first));
    let (stale, refusal) = merge(host, &docs, &first);
    assert_eq!(stale, 1);
    assert_eq!(refusal["code"], "stale", "{refusal}");

    // Both workers changed the README's status line, so this merge
    // conflicts: nothing lands, and the change goes back to grace, who
    // merges the branch in and resolves it.
    let (code, conflicted) = merge(host, &docs, &changed);
    assert_eq!(code, 1, "{conflicted}");
    let publication = &conflicted["merged"]["publication"];
    assert_eq!(publication["state"], "refused");
    assert!(
        publication["note"]
            .as_str()
            .is_some_and(|note| note.contains("conflict") && note.contains("README.md")),
        "{publication}"
    );
    assert!(!host.checked_out("README.md").contains("documented"));
    let resolved = host.ready(&goal, "docs", &docs, Some(&changed));
    let (_, diff) = host.review(&docs);
    assert!(!diff.contains("<<<<<<<"), "{diff}");
    let (code, landed) = merge(host, &docs, &resolved);
    assert_eq!(code, 0, "{landed}");
    assert_eq!(landed["merged"]["publication"]["state"], "published");
    assert!(
        host.checked_out("README.md")
            .contains("Status: greets people, documented")
    );
    assert!(
        host.checked_out("docs/greeting.md")
            .contains("greeting.txt")
    );

    // The release waits on both, then asks the person to approve its
    // step; once allowed, its changelog lands too.
    let release = task_id(host, &goal, "release");
    let (approval, text) = host.decision(&release, "approval");
    assert!(text.contains("CHANGELOG.md"), "{text}");
    let allowed = host
        .studio(&["answer", &approval, studio_sim::ALLOW])
        .json();
    assert_eq!(allowed["kind"], "approval");
    let head = host.ready(&goal, "release", &release, None);
    let (code, landed) = merge(host, &release, &head);
    assert_eq!(code, 0, "{landed}");
    assert!(host.checked_out("CHANGELOG.md").contains("Hello, studio"));

    // The goal is done.
    host.until(&["status"], "the goal done", |status| {
        status["view"]["goals"].as_array().is_some_and(|goals| {
            goals
                .iter()
                .any(|item| item["goal"] == goal.as_str() && item["status"] == "done")
        })
    });

    // The live view: one snapshot line under --json.
    let watched = host.studio(&["watch", "--limit", "1"]);
    assert_eq!(watched.code, 0, "{}", watched.stderr);
    let lines: Vec<&str> = watched.stdout.lines().collect();
    assert_eq!(lines.len(), 1, "{}", watched.stdout);
    let line: Value = serde_json::from_str(lines[0]).expect("an NDJSON line");
    assert_eq!(line["kind"], "snapshot");
    assert!(
        line["view"]["goals"]
            .as_array()
            .is_some_and(|goals| goals.iter().any(|item| item["goal"] == goal.as_str()))
    );
}

#[test]
fn the_simulated_team_takes_a_goal_through_question_approval_conflict_and_merge() {
    let mut host = host();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(&host)));
    host.stop_engine();
    let archived = host.archive();
    host.shutdown();
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
    assert!(
        archived.expect("every task the test created is archived") >= 4,
        "the lead and three plan tasks were created"
    );
}
