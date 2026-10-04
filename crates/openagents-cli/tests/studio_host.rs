//! `openagents studio` against a scratch host (#10566): a host with its
//! access store, host root, control socket, and task store under a
//! temporary directory, a scratch repository as its one workspace, and a
//! temporary `HOME` for every command. Nothing here reaches the person's
//! own host, home, or relays.
//!
//! A goal goes from submission through a plan answer, a review, and a
//! merge with only `openagents studio` commands, each under `--json`:
//! `seat set`, `goal submit`, `tasks`, `task cancel`, `sync`, `decisions`,
//! `answer`, `review`, `merge`, `seat pause` and `resume`, `status`, and
//! `watch`, and a refused intent prints the host's code.
//!
//! No engine runs, so the test stands in for one: it cancels the lead's
//! task so the coordinator opens the goal's plan decision, and commits a
//! change in a task's worktree, recorded as a local run records it. Every
//! task the test creates is archived through the host when it ends, pass
//! or fail.
#![cfg(unix)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
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
const WAIT: Duration = Duration::from_secs(60);
const GOAL: &str = "Greet with Hello, studio and document the greeting.";

/// A scratch host and everything it keeps.
struct Host {
    temp: tempfile::TempDir,
    runtime: tokio::runtime::Runtime,
    running: Option<coder_host::Running>,
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
    let fixture = studio_sim::Fixture::create(&temp.path().join("fixture")).expect("a repository");
    let checkout = std::fs::canonicalize(&fixture.checkout).expect("the checkout");
    let store = temp.path().join("tasks");
    let root = temp.path().join("host");
    let socket = temp.path().join("c/control.sock");
    let workspaces = BTreeMap::from([("scratch".to_owned(), checkout.clone())]);
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
        let tasks = Arc::new(coder::task::remote::Inbox::new(store.clone(), workspaces))
            as Arc<dyn coder_host::Tasks>;
        let running = coder_host::start(config, tasks).await.expect("the host");
        (running, task)
    });
    Host {
        temp,
        runtime,
        running: Some(running),
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

    fn shutdown(&mut self) {
        if let Some(running) = self.running.take() {
            self.runtime.block_on(running.shutdown());
        }
    }
}

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=seat",
            "-c",
            "user.email=seat@studio.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// The goal's task for plan entry `entry`, when the list holds it.
fn task<'a>(tasks: &'a Value, goal: &str, entry: &str) -> Option<&'a Value> {
    tasks["tasks"]
        .as_array()?
        .iter()
        .find(|task| task["goal"] == goal && task["entry"] == entry)
}

fn run(host: &Host) {
    // Seats, set locally as the owner sets them.
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
        .studio(&["goal", "submit", GOAL, "--workspace", "scratch"])
        .json();
    assert_eq!(submitted["operation"], "studio.goal.submit");
    let goal = submitted["goal_id"]
        .as_str()
        .expect("the goal's identity")
        .to_owned();
    let lead = host.until(&["tasks", &goal], "the lead's task", |tasks| {
        task(tasks, &goal, "lead").is_some()
    });
    let lead = task(&lead, &goal, "lead").expect("the lead's task").clone();
    assert_eq!(lead["status"], "queued");
    assert_eq!(lead["seat"], "lead");
    let lead_task = lead["task"].as_str().expect("its identity").to_owned();

    // The lead's task ends without a plan, and the coordinator's pass
    // opens the goal's decision. A shortened identity names the task.
    let cancelled = host.studio(&["task", "cancel", &lead_task[..12]]).json();
    assert_eq!(cancelled["operation"], "studio.task.cancel");
    assert_eq!(cancelled["task"], lead_task.as_str());
    host.until(&["tasks", &goal], "the lead's task cancelled", |tasks| {
        task(tasks, &goal, "lead").is_some_and(|task| task["status"] == "cancelled")
    });
    host.studio(&["sync"]).json();
    let open = host.until(&["decisions"], "the goal's decision", |open| {
        open["decisions"]
            .as_array()
            .is_some_and(|all| all.iter().any(|decision| decision["goal"] == goal.as_str()))
    });
    let decision = open["decisions"]
        .as_array()
        .and_then(|all| {
            all.iter()
                .find(|decision| decision["goal"] == goal.as_str())
        })
        .and_then(|decision| decision["decision"].as_str())
        .expect("the decision's identity")
        .to_owned();

    // The answer is a plan, which resumes the goal.
    let plan = host.temp.path().join("plan.json");
    std::fs::write(&plan, studio_sim::plan()).expect("the plan");
    let answered = host
        .studio(&[
            "answer",
            &decision,
            "--file",
            plan.to_str().expect("a path"),
        ])
        .json();
    assert_eq!(answered["operation"], "studio.decision.answer");
    let released = host.until(&["tasks", &goal], "the plan released", |tasks| {
        ["greet", "docs"]
            .iter()
            .all(|entry| task(tasks, &goal, entry).is_some_and(|task| task["status"] == "queued"))
    });
    let greet = task(&released, &goal, "greet")
        .and_then(|task| task["task"].as_str())
        .expect("the greeting task")
        .to_owned();

    // Steering through the host: pause and resume a seat.
    let paused = host.studio(&["seat", "pause", "grace"]).json();
    assert_eq!(paused["operation"], "studio.seat.pause");
    let resumed = host.studio(&["seat", "resume", "@grace"]).json();
    assert_eq!(resumed["operation"], "studio.seat.resume");

    // A refused intent prints the host's code and message.
    let refused = host.studio(&["seat", "pause", "nobody"]);
    assert_eq!(refused.code, 1, "{}", refused.stderr);
    let refusal: Value = serde_json::from_str(refused.stdout.trim()).expect("a JSON refusal");
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

    // A seat's change to review: a worktree with one commit, recorded as a
    // local run records it.
    let worktree = host.temp.path().join("worktrees/greet");
    std::fs::create_dir_all(worktree.parent().expect("a parent")).expect("the worktrees");
    git(
        &host.checkout,
        &[
            "worktree",
            "add",
            "-q",
            "--detach",
            worktree.to_str().expect("a path"),
            "HEAD",
        ],
    );
    let base = git(&worktree, &["rev-parse", "HEAD"]);
    std::fs::write(worktree.join("greeting.txt"), "Hello, studio\n").expect("a change");
    git(
        &worktree,
        &["commit", "-q", "-am", "Greet with Hello, studio"],
    );
    let record = serde_json::json!({
        "schema": coder::task::local::RECORD_SCHEMA,
        "task": greet,
        "project": "scratch",
        "checkout": host.checkout.to_string_lossy(),
        "worktree": worktree.to_string_lossy(),
        "base": base,
        "turns": [],
    });
    let records = host.store.join("local");
    std::fs::create_dir_all(&records).expect("the records");
    std::fs::write(
        records.join(format!("{greet}.json")),
        serde_json::to_vec(&record).expect("a record"),
    )
    .expect("the record");

    // The review, with its diff.
    let review = host.studio(&["review", &greet, "--diff"]).json();
    let review = &review["review"];
    assert_eq!(review["base"], base.as_str());
    assert!(
        review["diff"]
            .as_str()
            .is_some_and(|diff| diff.contains("Hello, studio")),
        "{review}"
    );
    let head = review["head"]
        .as_str()
        .expect("the reviewed tree")
        .to_owned();

    // A merge bound to a revision the change moved past is refused before
    // it is sent.
    let moved = host.studio(&["merge", &greet, "--head", &"0".repeat(40)]);
    assert_eq!(moved.code, 1, "{}", moved.stderr);
    let moved: Value = serde_json::from_str(moved.stdout.trim()).expect("a JSON refusal");
    assert_eq!(moved["code"], "stale");

    // The merge at the reviewed revisions lands in the checkout's branch,
    // and nothing is pushed.
    let merged = host.studio(&["merge", &greet, "--head", &head]).json();
    let merged = &merged["merged"];
    assert_eq!(merged["verdict"], "merge");
    assert_eq!(merged["head"], head.as_str());
    assert_eq!(merged["publication"]["state"], "published", "{merged}");
    assert_eq!(
        std::fs::read_to_string(host.checkout.join("greeting.txt")).expect("the greeting"),
        "Hello, studio\n"
    );

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
fn a_goal_is_planned_reviewed_and_merged_from_the_command_line() {
    let mut host = host();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(&host)));
    let archived = host.archive();
    host.shutdown();
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
    assert!(
        archived.expect("every task the test created is archived") >= 3,
        "the lead and two plan tasks were created"
    );
}
