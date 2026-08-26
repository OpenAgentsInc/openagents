//! Delegation, against real child processes.
//!
//! Every claim here is made by running something. The children are stand-in
//! harnesses — shell scripts that speak the wire format of the real ones —
//! reached through the same `OA_CHILD_*` seam the TypeScript harnesses expose
//! for the same reason: a test that cannot substitute the agent can only
//! assert against a real one, which means it costs money or does not run.
//!
//! What the stand-in cannot fake is the part under test: the process is a real
//! process with a real pid in a real directory, its output is read as it is
//! written, and killing it is a real signal to a real process group.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use openagents_cli::delegate::{ChildEvent, ChildLane, DelegationSupervisor};
use openagents_cli::workspace::{Isolation, WorkspacePlan};
use tokio::sync::{mpsc, watch};

/// The environment is one variable per lane and the tests share a process, so
/// two of them setting `OA_CHILD_CLAUDE` at once run each other's stand-in.
/// Held for as long as the variable is set rather than only while setting it.
static ENVIRONMENT: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn exclusive() -> std::sync::MutexGuard<'static, ()> {
    // A test that panics while holding it poisons it; the next test still
    // wants the lock, and what it protects has no invariant to have broken.
    ENVIRONMENT.lock().unwrap_or_else(|held| held.into_inner())
}

/// Write an executable stand-in and return its path.
fn stand_in(name: &str, body: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oa-delegate-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, body).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    path
}

/// Run a fan-out and collect both the outcomes and everything it reported.
async fn run(
    supervisor: &DelegationSupervisor,
    prompt: &str,
    stop_after: Option<Duration>,
) -> (
    Vec<openagents_cli::delegate::ChildWorkerResult>,
    Vec<(Instant, ChildEvent)>,
) {
    let (events, mut incoming) = mpsc::unbounded_channel();
    let collected = tokio::spawn(async move {
        let mut seen = Vec::new();
        while let Some(event) = incoming.recv().await {
            seen.push((Instant::now(), event));
        }
        seen
    });

    let (stop, cancel) = watch::channel(false);
    if let Some(after) = stop_after {
        tokio::spawn(async move {
            tokio::time::sleep(after).await;
            let _ = stop.send(true);
        });
    } else {
        // Held so the channel stays open for the life of the run.
        std::mem::forget(stop);
    }

    let results = supervisor
        .dispatch_streaming(prompt, events, cancel)
        .await
        .expect("no child could be started");
    (results, collected.await.unwrap())
}

/// Every child gets a git worktree of its own, and giving it back leaves
/// nothing behind.
#[tokio::test]
async fn each_child_gets_a_worktree_of_its_own() {
    let here = std::env::current_dir().unwrap();
    let plan = WorkspacePlan::resolve(here.clone(), Isolation::Worktree).await;
    assert_eq!(
        plan.isolation(),
        Isolation::Worktree,
        "the test runs inside a checkout, so a worktree is available"
    );

    let workspaces = plan.prepare(2).await.expect("the worktrees were not made");
    assert_eq!(workspaces.len(), 2);
    assert_ne!(
        workspaces[0].path, workspaces[1].path,
        "two children in one directory is the defect this replaces"
    );

    for workspace in &workspaces {
        assert!(workspace.path.is_dir(), "{}", workspace.path.display());
        // A worktree's `.git` is a file pointing back at the repository, not a
        // directory: this is a worktree and not a copied tree.
        assert!(
            workspace.path.join(".git").is_file(),
            "{} is not a git worktree",
            workspace.path.display()
        );
        // Each child has its own index, so what one writes the other does not
        // see.
        std::fs::write(workspace.path.join("only-mine.txt"), format!("{}", workspace.id)).unwrap();
    }
    assert!(!workspaces[0].path.join("only-mine.txt").is_symlink());
    assert_eq!(
        std::fs::read_to_string(workspaces[0].path.join("only-mine.txt")).unwrap(),
        "1"
    );
    assert_eq!(
        std::fs::read_to_string(workspaces[1].path.join("only-mine.txt")).unwrap(),
        "2"
    );

    for workspace in &workspaces {
        assert_eq!(workspace.release().await, None);
        assert!(
            !workspace.path.exists(),
            "{} outlived the fan-out",
            workspace.path.display()
        );
    }
}

/// A child's output reaches the caller while the child is still running.
///
/// The version this replaces called `Command::output()`, which returns when
/// the child is finished. The assertion is a clock: the first line has to
/// arrive well before the last one, and the child does not exit until it has
/// written both.
#[tokio::test]
async fn a_child_streams_while_it_is_still_running() {
    let harness = stand_in(
        "slow-claude",
        r#"#!/bin/sh
echo '{"type":"assistant","message":{"content":[{"type":"text","text":"first"}]}}'
sleep 2
echo '{"type":"assistant","message":{"content":[{"type":"text","text":"last"}]}}'
echo '{"type":"result","is_error":false,"result":"first last"}'
"#,
    );
    let _exclusive = exclusive();
    unsafe { std::env::set_var("OA_CHILD_CLAUDE", &harness); }

    let supervisor = DelegationSupervisor::new(1, "claude", None)
        .with_isolation(Isolation::Directory);
    let started = Instant::now();
    let (results, events) = run(&supervisor, "ignored by the stand-in", None).await;
    unsafe { std::env::remove_var("OA_CHILD_CLAUDE"); }

    assert!(results[0].success, "{}", results[0].output);
    let total = started.elapsed();
    assert!(
        total >= Duration::from_secs(2),
        "the stand-in sleeps for two seconds; this run took {total:?}"
    );

    let first_said = events
        .iter()
        .find_map(|(at, event)| match event {
            ChildEvent::Output { text, .. } if text.contains("first") => Some(*at),
            _ => None,
        })
        .expect("nothing was streamed at all");
    let gap = first_said.duration_since(started);
    assert!(
        gap < Duration::from_secs(1),
        "the first line arrived after {gap:?}, which is the end of the run rather than the start of it"
    );
}

/// Children run at once, and the cap is what decides how many.
///
/// Three children that each sleep for a second finish in about a second when
/// they run together and about three when they are made to queue.
#[tokio::test]
async fn count_is_real_concurrency_and_the_cap_is_real_too() {
    let harness = stand_in(
        "sleepy-claude",
        r#"#!/bin/sh
sleep 1
echo "{\"type\":\"result\",\"is_error\":false,\"result\":\"pid $$\"}"
"#,
    );
    let _exclusive = exclusive();
    unsafe { std::env::set_var("OA_CHILD_CLAUDE", &harness); }

    let together = DelegationSupervisor::new(3, "claude", None)
        .with_isolation(Isolation::Directory);
    let at = Instant::now();
    let (parallel, events) = run(&together, "ignored", None).await;
    let parallel_took = at.elapsed();

    let queued = DelegationSupervisor::new(3, "claude", None)
        .with_isolation(Isolation::Directory)
        .with_max_parallel(1);
    let at = Instant::now();
    let (serial, _) = run(&queued, "ignored", None).await;
    let serial_took = at.elapsed();
    unsafe { std::env::remove_var("OA_CHILD_CLAUDE"); }

    assert_eq!(parallel.iter().filter(|r| r.success).count(), 3);
    assert_eq!(serial.iter().filter(|r| r.success).count(), 3);
    assert!(
        parallel_took < Duration::from_millis(2_500),
        "three one-second children took {parallel_took:?} together, which is not together"
    );
    assert!(
        serial_took > Duration::from_millis(2_500),
        "three one-second children capped at one at a time took {serial_took:?}, which is not a cap"
    );

    // Three real processes, three different pids, three different directories.
    let pids: Vec<u32> = parallel.iter().filter_map(|result| result.pid).collect();
    assert_eq!(pids.len(), 3, "a child with no pid is not a child process");
    assert_eq!(
        pids.iter().collect::<std::collections::BTreeSet<_>>().len(),
        3,
        "{pids:?} are not three separate processes"
    );
    let homes: std::collections::BTreeSet<&Path> = parallel
        .iter()
        .filter_map(|result| result.workspace.as_deref())
        .collect();
    assert_eq!(homes.len(), 3, "{homes:?} is not one directory per child");

    let announced = events
        .iter()
        .filter(|(_, event)| matches!(event, ChildEvent::Started { .. }))
        .count();
    assert_eq!(announced, 3);
}

/// A child that fails is reported as failed.
///
/// Both shapes: one that runs and exits non-zero, and one whose binary is not
/// there at all. The version this replaces reported `2/2 children succeeded`
/// in both cases.
#[tokio::test]
async fn a_failing_child_is_reported_as_failed() {
    let harness = stand_in(
        "doomed-claude",
        r#"#!/bin/sh
echo "the tests did not pass" >&2
exit 3
"#,
    );
    let _exclusive = exclusive();
    unsafe { std::env::set_var("OA_CHILD_CLAUDE", &harness); }
    let supervisor = DelegationSupervisor::new(1, "claude", None)
        .with_isolation(Isolation::Directory);
    let (results, _) = run(&supervisor, "ignored", None).await;
    unsafe { std::env::remove_var("OA_CHILD_CLAUDE"); }

    assert!(!results[0].success, "a child that exited 3 reported success");
    assert!(results[0].failure.is_some());
    let why = results[0].failure.clone().unwrap();
    assert!(why.contains("code 3"), "{why}");
    assert!(why.contains("the tests did not pass"), "{why}");

    unsafe { std::env::set_var("OA_CHILD_CLAUDE", "/nonexistent/no-such-agent"); }
    let supervisor = DelegationSupervisor::new(1, "claude", None)
        .with_isolation(Isolation::Directory);
    let (missing, _) = run(&supervisor, "ignored", None).await;
    unsafe { std::env::remove_var("OA_CHILD_CLAUDE"); }

    assert!(!missing[0].success);
    assert!(
        missing[0].failure.clone().unwrap().contains("not on PATH"),
        "{:?}",
        missing[0].failure
    );
}

/// Stopping a fan-out stops the children, and the children's own children.
///
/// The stand-in starts a background process that writes a file after five
/// seconds. If the group was signalled, the file never appears; if only the
/// direct child was killed, it does.
#[tokio::test]
async fn stopping_a_fanout_kills_the_whole_group() {
    let witness = std::env::temp_dir().join(format!("oa-orphan-{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&witness);

    let harness = stand_in(
        "runaway-claude",
        &format!(
            r#"#!/bin/sh
sh -c 'sleep 5; echo orphaned > {}' &
sleep 30
"#,
            witness.display()
        ),
    );
    let _exclusive = exclusive();
    unsafe { std::env::set_var("OA_CHILD_CLAUDE", &harness); }

    let supervisor = DelegationSupervisor::new(1, "claude", None)
        .with_isolation(Isolation::Directory);
    let at = Instant::now();
    let (results, _) = run(&supervisor, "ignored", Some(Duration::from_millis(700))).await;
    let took = at.elapsed();
    unsafe { std::env::remove_var("OA_CHILD_CLAUDE"); }

    assert!(!results[0].success);
    assert_eq!(results[0].failure.as_deref(), Some("stopped before finishing"));
    assert!(
        took < Duration::from_secs(20),
        "a stopped child ran for {took:?}; the stand-in sleeps for thirty seconds"
    );

    // Long enough for the grandchild to have fired if it were still alive.
    tokio::time::sleep(Duration::from_secs(6)).await;
    assert!(
        !witness.exists(),
        "the child's own subprocess outlived the fan-out that started it"
    );
}

/// A lane name that is not a lane is refused rather than quietly redirected.
#[test]
fn an_unknown_lane_is_not_silently_ox_alpha() {
    assert!(ChildLane::known("claude"));
    assert!(ChildLane::known("opencode/x-preview-f-free"));
    assert!(!ChildLane::known("gemni"));
    assert!(!ChildLane::known(""));
}

// ──────────────────────────────────────────────────── the child's own thread
//
// A delegated child opens a thread of its own and used to leave revoking it to
// the `Drop` impl, which spawns a best-effort `DELETE` onto whatever runtime is
// still up. The audit in #89 found two threads still open from long-dead
// sessions; a thread left open holds its grant's remaining budget (issue #107).

/// How long the stub holds a revocation open.
///
/// Long enough to tell an awaited `close()` from the `Drop` impl's spawned
/// best effort, which returns at once and leaves its request in flight.
const REVOCATION_HELD: Duration = Duration::from_millis(600);

/// A stand-in for the account API and the proxy, which records what it took.
struct ProxyStub {
    origin: String,
    requests: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
}

impl ProxyStub {
    /// Every request this stub has taken, headers and body, most recent last.
    fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }

    fn request_lines(&self) -> Vec<String> {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .map(|request| request.lines().next().unwrap_or_default().to_string())
            .collect()
    }
}

/// Answers a thread open, transcript appends, the thread's report, and one turn.
async fn proxy_stub() -> ProxyStub {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let origin = format!("http://127.0.0.1:{port}");
    let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));

    let seen = std::sync::Arc::clone(&requests);
    let grant_url = format!("{origin}/proxy");
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let mut request = Vec::new();
            let mut buffer = [0u8; 4096];
            loop {
                let Ok(read) = socket.read(&mut buffer).await else {
                    break;
                };
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..read]);
                let text = String::from_utf8_lossy(&request);
                if let Some(end) = text.find("\r\n\r\n") {
                    let length = text
                        .lines()
                        .find_map(|line| {
                            line.strip_prefix("content-length: ")
                                .or_else(|| line.strip_prefix("Content-Length: "))
                        })
                        .and_then(|value| value.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    if request.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            let request = String::from_utf8_lossy(&request).to_string();
            let line = request.lines().next().unwrap_or_default().to_string();
            seen.lock().unwrap().push(request);

            let (status, content_type, body) = if line.starts_with("POST")
                && line.contains("/events")
            {
                (
                    201,
                    "application/json",
                    r#"{"events":[{"id":1}]}"#.to_string(),
                )
            } else if line.starts_with("POST") && line.contains("/report") {
                // Held open, so an awaited ending is measurably slower than a
                // spawned one. See the test below.
                tokio::time::sleep(REVOCATION_HELD).await;
                (
                    200,
                    "application/json",
                    r#"{"thread":{"id":"th_child","status":"succeeded"},
                        "grant":{"status":"revoked","spent":{"calls":1,"total_tokens":12}}}"#
                        .to_string(),
                )
            } else if line.starts_with("GET /api/v1/models") {
                // The catalog. A child runs on `glm-5.3-flash` as a directly-named
                // model, and a named model is checked against this before the
                // turn runs.
                (
                    200,
                    "application/json",
                    r#"{"models":[{"id":"glm-5.3-flash","availability":"available","default":true}]}"#
                        .to_string(),
                )
            } else if line.starts_with("POST /api/v1/threads") {
                (
                    200,
                    "application/json",
                    format!(
                        r#"{{"thread":{{"id":"th_child"}},"grant":{{"token":"tok","url":"{grant_url}","model":"glm-5.3-flash"}}}}"#
                    ),
                )
            } else {
                let frame =
                    serde_json::json!({"choices":[{"delta":{"content":"the child answered"}}]});
                let stream = format!("data: {frame}\n\ndata: [DONE]\n\n");
                let head = "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n";
                let _ = socket.write_all(head.as_bytes()).await;
                let _ = socket.write_all(stream.as_bytes()).await;
                let _ = socket.flush().await;
                continue;
            };

            let head = format!(
                "HTTP/1.1 {status} X\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                body.len()
            );
            let _ = socket.write_all(head.as_bytes()).await;
            let _ = socket.write_all(body.as_bytes()).await;
            let _ = socket.flush().await;
        }
    });

    ProxyStub { origin, requests }
}

/// A child on the proxy ends its own thread, awaited, saying what it did and
/// writing its turn down on the way.
///
/// A child used to leave the ending to the `Drop` impl, which spawns a request
/// onto whatever runtime is still up and may never be polled. Timed rather
/// than merely observed: the stub holds the ending open, so a child that
/// finishes faster than that did not wait for it.
///
/// A child that answered reports `succeeded` rather than being cancelled
/// (issue #106), which is also what leaves its thread resumable.
#[tokio::test]
async fn a_delegated_child_ends_its_own_thread_by_saying_what_it_did() {
    let _guard = exclusive();
    let stub = proxy_stub().await;
    unsafe { std::env::set_var("OPENAGENTS_API_BASE", format!("{}/api/v1", stub.origin)); }

    let supervisor = DelegationSupervisor::new(1, "glm-5.3-flash", Some("oat_test".to_string()))
        .with_isolation(Isolation::None)
        .in_directory(Some(std::env::temp_dir()));
    let (results, _events) = run(&supervisor, "say something", None).await;

    unsafe { std::env::remove_var("OPENAGENTS_API_BASE"); }

    assert_eq!(results.len(), 1);
    assert!(
        results[0].success,
        "the child failed: {}",
        results[0].output
    );

    let lines = stub.request_lines();
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("POST /api/v1/threads/th_child/report")),
        "the child left its thread open: {lines:?}"
    );
    assert!(
        !lines.iter().any(|line| line.starts_with("DELETE")),
        "the child cancelled a thread it had answered on: {lines:?}"
    );
    let reported: serde_json::Value = stub
        .requests()
        .into_iter()
        .find(|request| request.contains("/report"))
        .and_then(|request| {
            request
                .split_once("\r\n\r\n")
                .and_then(|(_, body)| serde_json::from_str(body).ok())
        })
        .expect("the child said nothing about how it ended");
    assert_eq!(reported["status"], "succeeded");
    assert_eq!(reported.get("error_code"), None);
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("POST") && line.contains("/events")),
        "the child recorded nothing of what it did: {lines:?}"
    );
    assert!(
        results[0].duration_ms >= REVOCATION_HELD.as_millis(),
        "the child finished in {}ms with the ending held open for {}ms, so it was \
         spawned and abandoned rather than awaited",
        results[0].duration_ms,
        REVOCATION_HELD.as_millis()
    );
}

// ──────────────────────────────────────── the `--child-*` flags on `oa coder`

/// Parse a real `oa coder` command line and give back the child options a
/// fan-out started from it would run with.
///
/// The whole chain, argv first: clap parses the flags, `DelegationRequest`
/// carries them off the coder command, and `ChildOptions::resolve` settles
/// them. `oa coder --delegate` declared none of these flags, so every one of
/// them used to stop at the parser — or rather never reach it.
fn child_options_from(argv: &[&str]) -> openagents_cli::delegate::ChildOptions {
    use clap::Parser;
    let parsed = openagents_cli::cli::Cli::parse_from(argv);
    let Some(openagents_cli::cli::Commands::Coder(coder)) = parsed.command else {
        panic!("{argv:?} did not parse as `oa coder`");
    };
    let request = openagents_cli::delegate::DelegationRequest::from_coder(coder);
    openagents_cli::delegate::ChildOptions::resolve(
        request.child_model,
        request.child_command,
        request.child_config,
        request.child_ask,
    )
}

/// A stand-in that reports the argv and the `OPENCODE_CONFIG` it was started
/// with, in the claude lane's wire shape.
///
/// The marker is the point: if `--child-command` did not reach the child, the
/// `claude` on this machine ran instead and the marker is absent.
fn reporting_stand_in(name: &str) -> PathBuf {
    stand_in(
        name,
        r#"#!/bin/sh
printf '{"type":"result","is_error":false,"result":"MARK argv=%s config=%s END"}\n' \
  "$*" "${OPENCODE_CONFIG:-unset}"
"#,
    )
}

/// What one child of a fan-out configured by `argv` actually reported.
///
/// Under the shared guard: these start real child processes, and the streaming
/// test above asserts against a wall clock, so a fan-out running alongside it
/// is measured as its latency.
async fn one_child_reports(argv: &[&str]) -> String {
    let _exclusive = exclusive();
    let options = child_options_from(argv);
    let supervisor = DelegationSupervisor::new(1, "claude", None)
        .with_isolation(Isolation::Directory)
        .with_child_options(options);
    let (results, _) = run(&supervisor, "ignored", None).await;
    assert!(results[0].success, "the child failed: {}", results[0].output);
    let said = results[0].output.clone();
    assert!(
        said.contains("MARK"),
        "the stand-in did not run, so --child-command never reached the child: {said}"
    );
    said
}

/// `--child-config` reaches the child's environment.
///
/// This is the one with consequences: the CLI deliberately stores no provider
/// credential, so a harness config passed as `OPENCODE_CONFIG` is the only
/// route one has to a delegated child. Asserted against the environment of a
/// real process, not against the parsed flag.
#[tokio::test]
async fn child_config_reaches_a_real_child_process() {
    let harness = reporting_stand_in("config-reporting-claude");
    let config = std::env::temp_dir().join("oa-child-harness-config.json");
    std::fs::write(&config, "{}").unwrap();

    let said = one_child_reports(&[
        "oa",
        "coder",
        "--delegate",
        "--child-command",
        harness.to_str().unwrap(),
        "--child-config",
        config.to_str().unwrap(),
        "do the thing",
    ])
    .await;

    assert!(
        said.contains(&format!("config={}", config.display())),
        "OPENCODE_CONFIG did not reach the child: {said}"
    );
}

/// `--child-model` reaches the child's argument list, and `--child-ask`
/// changes the mode it is started in.
#[tokio::test]
async fn child_model_and_child_ask_reach_a_real_child_process() {
    let harness = reporting_stand_in("model-reporting-claude");
    let command = harness.to_str().unwrap();

    let said = one_child_reports(&[
        "oa",
        "coder",
        "--delegate",
        "--child-command",
        command,
        "--child-model",
        "claude-sonnet-4-5",
        "go",
    ])
    .await;
    assert!(
        said.contains("--model claude-sonnet-4-5"),
        "--child-model did not reach the child's argv: {said}"
    );
    // Without --child-ask a delegated child has nobody to ask, so it accepts
    // its own edits.
    assert!(
        said.contains("--permission-mode acceptEdits"),
        "{said}"
    );

    let asking = one_child_reports(&[
        "oa",
        "coder",
        "--delegate",
        "--child-command",
        command,
        "--child-ask",
        "go",
    ])
    .await;
    assert!(
        asking.contains("--permission-mode default"),
        "--child-ask did not change the mode the child was started in: {asking}"
    );
    assert!(
        !asking.contains("acceptEdits"),
        "--child-ask left the child accepting its own edits: {asking}"
    );
}

/// `--concurrency` on `oa coder` is the cap, under the name the TypeScript CLI
/// and `oa delegate` both use for it.
#[test]
fn concurrency_is_the_cap_on_the_coder_command_too() {
    use clap::Parser;
    for flag in ["--concurrency", "--max-parallel"] {
        let parsed =
            openagents_cli::cli::Cli::parse_from(["oa", "coder", "--delegate", flag, "3", "go"]);
        let Some(openagents_cli::cli::Commands::Coder(coder)) = parsed.command else {
            panic!("{flag} did not parse as `oa coder`");
        };
        assert_eq!(
            openagents_cli::delegate::DelegationRequest::from_coder(coder).max_parallel,
            Some(3),
            "{flag} did not reach the cap"
        );
    }
}

/// The `delegate` tool a session runs starts children on the session's own
/// `--child-*` flags.
///
/// `oa coder --child-config f` with no `--delegate` opens a session that can
/// still fan out — through `/delegate` or the model calling the tool — and that
/// path built its supervisor with no child options at all. The flag parsed,
/// said nothing, and the child never saw the file.
#[tokio::test]
async fn the_delegate_tool_carries_the_sessions_child_options() {
    let _exclusive = exclusive();
    let harness = reporting_stand_in("tool-reporting-claude");
    let config = std::env::temp_dir().join("oa-tool-harness-config.json");
    std::fs::write(&config, "{}").unwrap();

    let options = child_options_from(&[
        "oa",
        "coder",
        "--child-command",
        harness.to_str().unwrap(),
        "--child-config",
        config.to_str().unwrap(),
        "--child-model",
        "claude-sonnet-4-5",
    ]);

    // A directory of its own rather than this checkout: the tool's children
    // work where the session works, and a temporary directory keeps this test
    // from making a git worktree of the whole repository.
    let cwd = tempfile::tempdir().unwrap();
    let report = openagents_cli::delegate::fanout_for_tool(
        "do the thing",
        1,
        "claude",
        None,
        options,
        Some(cwd.path().to_path_buf()),
    )
    .await;

    assert!(
        report.contains("MARK"),
        "--child-command never reached the tool's child: {report}"
    );
    assert!(
        report.contains(&format!("config={}", config.display())),
        "OPENCODE_CONFIG never reached the tool's child: {report}"
    );
    assert!(
        report.contains("--model claude-sonnet-4-5"),
        "--child-model never reached the tool's child: {report}"
    );
}

/// A `--child-*` flag the session's lane cannot honour is said, not dropped.
#[tokio::test]
async fn the_delegate_tool_refuses_a_flag_its_lane_cannot_honour() {
    let options = child_options_from(&["oa", "coder", "--child-model", "gpt-5"]);
    // glm-5.3-flash children run on the grant the server issues, which pins the
    // model. There is no honouring `--child-model` there.
    let report =
        openagents_cli::delegate::fanout_for_tool("go", 1, "glm-5.3-flash", None, options, None).await;
    assert!(
        report.starts_with("No children were started:"),
        "the tool ran a fan-out without the model it was given: {report}"
    );
    assert!(report.contains("--child-model"), "{report}");
}
