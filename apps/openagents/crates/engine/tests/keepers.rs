//! Run keepers (#11930): a running agent outlives its engine, and the next
//! engine picks it up with no gap or duplicate in the transcript.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use openagents_doc::{MessagePart, MessageRole};
use openagents_engine::{EngineCore, HarnessRegistry, RunJournal};
use openagents_harness::Harness;
use openagents_harness::keeper::{self, lines::LineHarness};
use openagents_proto::{
    AgentEvent, HarnessId, RunRequest, SandboxLevel, SessionStatus, UserInputAnswer,
};

const CHAT: &str = "chat-keeper";

/// Runs the keeper when this test binary is started as one.
#[test]
fn keeper_entry() {
    // The keeper finds its run folder in this variable; nothing else sets it.
    if std::env::var_os("OPENAGENTS_KEEPER_DIR").is_some() {
        std::process::exit(keeper::main(std::iter::empty()));
    }
}

fn harness(ask_at: Option<usize>) -> Arc<dyn Harness> {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../harness/tests/fixtures/keeper-agent.py");
    Arc::new(LineHarness {
        id: HarnessId::Mock,
        name: "Fake".into(),
        program: fixture,
        args: Vec::new(),
        env: vec![
            ("KEEPER_TEST_ENTRY".into(), "1".into()),
            ("KEEPER_FAKE_CHUNKS".into(), "20".into()),
            ("KEEPER_FAKE_INTERVAL_MS".into(), "25".into()),
            (
                "KEEPER_FAKE_ASK_AT".into(),
                ask_at.map_or("-1".into(), |at| at.to_string()).into(),
            ),
        ],
    })
}

fn assemble(dir: &Path, ask_at: Option<usize>) -> EngineCore {
    assemble_with(dir, harness(ask_at))
}

/// Claude Code's driver over the fake CLI's slow scenario.
fn claude() -> Arc<dyn Harness> {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../harness/tests/fixtures/fake-claude.sh");
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&fixture, std::fs::Permissions::from_mode(0o755));
    }
    Arc::new(openagents_harness::ClaudeHarness::new().with_executable(fixture))
}

fn assemble_with(dir: &Path, harness: Arc<dyn Harness>) -> EngineCore {
    let id = harness.id();
    openagents_engine::set_keeper_launch(
        std::env::current_exe().unwrap(),
        ["--exact", "keeper_entry", "--nocapture", "--test-threads=1"]
            .map(Into::into)
            .to_vec(),
    );
    let registry = HarnessRegistry::new();
    registry.register(harness);
    EngineCore::assemble(dir, Arc::new(registry), id, None).unwrap()
}

fn request(dir: &Path) -> RunRequest {
    RunRequest {
        mcp: None,
        prompt: "go".into(),
        harness: None,
        model: None,
        reasoning: None,
        model_options: Default::default(),
        cwd: dir.display().to_string(),
        sandbox: SandboxLevel::WorkspaceWrite,
        auto_approve: false,
        attachments: Vec::new(),
        worktree: None,
        resume: None,
    }
}

fn journal(dir: &Path) -> Vec<AgentEvent> {
    RunJournal::open(dir.join("orgs/dev-org/dev-user/journals"))
        .unwrap()
        .replay(CHAT, 0)
        .unwrap()
        .into_iter()
        .map(|(_, event)| event)
        .collect()
}

fn journaled_text(dir: &Path) -> String {
    journal(dir)
        .iter()
        .filter_map(|event| match event {
            AgentEvent::TextDelta { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

fn wait_for(what: &str, mut check: impl FnMut() -> bool) {
    for _ in 0..1000 {
        if check() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for {what}");
}

/// Engine A: start the run, wait until `ready`, then stop. `graceful` quits
/// the way the app does; otherwise the engine dies with its runtime.
fn first_engine(dir: PathBuf, ask_at: Option<usize>, graceful: bool, ready: fn(&Path) -> bool) {
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let core = runtime.block_on(async {
            let core = assemble(&dir, ask_at);
            core.sessions
                .dispatch(CHAT, HarnessId::Mock, request(&dir), None)
                .await
                .unwrap();
            core
        });
        wait_for("the first engine to see output", || ready(&dir));
        if graceful {
            runtime.block_on(core.shutdown());
        }
        // Tasks stop where they are, as in a killed engine.
        drop(runtime);
        drop(core);
    })
    .join()
    .unwrap();
}

fn transcript(core: &EngineCore) -> (usize, Vec<String>) {
    let entries = core
        .doc_host
        .open(CHAT)
        .unwrap()
        .doc()
        .read_entries()
        .unwrap();
    let users = entries
        .iter()
        .filter(|e| e.role == MessageRole::User)
        .count();
    let replies = entries
        .iter()
        .filter(|e| e.role == MessageRole::Assistant)
        .map(|e| {
            e.parts
                .iter()
                .filter_map(|p| match p {
                    MessagePart::Text { text, .. } => Some(text.as_str()),
                    _ => None,
                })
                .collect()
        })
        .collect();
    (users, replies)
}

/// Once the run has ended and its end is handled, the keeper removes its folder.
fn keeper_finishes(dir: &Path) {
    let root = dir.join("orgs/dev-org/dev-user/keepers");
    wait_for("the keeper to finish", || {
        keeper::client::discover(&root).is_empty()
    });
}

fn expected(answer: Option<&str>) -> String {
    let mut text = String::new();
    for i in 0..20 {
        if i == 2
            && let Some(answer) = answer
        {
            text.push_str(&format!("answered {answer}\n"));
        }
        text.push_str(&format!("chunk {i}\n"));
    }
    text
}

fn picks_up(graceful: bool) {
    let dir = tempfile::tempdir().unwrap();
    first_engine(dir.path().to_path_buf(), None, graceful, |dir| {
        journaled_text(dir).contains("chunk 3")
    });
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let _guard = runtime.enter();
    let core = assemble(dir.path(), None);
    wait_for("the run to finish", || {
        matches!(journal(dir.path()).last(), Some(AgentEvent::Done { .. }))
    });
    // The journal has every chunk once and no restart note.
    assert_eq!(journaled_text(dir.path()), expected(None));
    let dones = journal(dir.path())
        .iter()
        .filter(|e| matches!(e, AgentEvent::Done { .. }))
        .count();
    assert_eq!(dones, 1);
    // The transcript has one prompt and one reply holding every chunk once.
    wait_for("the reply to be saved", || {
        transcript(&core)
            .1
            .last()
            .is_some_and(|t| t == &expected(None))
    });
    let (users, replies) = transcript(&core);
    assert_eq!(users, 1);
    assert_eq!(replies, vec![expected(None)]);
    keeper_finishes(dir.path());
    runtime.block_on(core.shutdown());
}

#[test]
fn a_restarted_engine_picks_up_a_running_agent_with_no_gap_or_duplicate() {
    picks_up(false);
}

#[test]
fn quitting_leaves_the_agent_running_for_the_next_engine() {
    picks_up(true);
}

#[test]
fn a_pending_question_is_asked_again_after_the_engine_restarts() {
    let dir = tempfile::tempdir().unwrap();
    first_engine(dir.path().to_path_buf(), Some(2), false, |dir| {
        journal(dir)
            .iter()
            .any(|e| matches!(e, AgentEvent::InputRequested { .. }))
    });
    let old: Vec<String> = journal(dir.path())
        .into_iter()
        .filter_map(|e| match e {
            AgentEvent::InputRequested { request_id, .. } => Some(request_id),
            _ => None,
        })
        .collect();
    assert_eq!(old.len(), 1);

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let _guard = runtime.enter();
    let core = assemble(dir.path(), Some(2));
    let mut asked = None;
    wait_for("the question to come back", || {
        asked = journal(dir.path()).into_iter().find_map(|e| match e {
            AgentEvent::InputRequested { request_id, .. } if request_id != old[0] => {
                Some(request_id)
            }
            _ => None,
        });
        asked.is_some()
    });
    // The old engine's question is withdrawn, so only the new one is open.
    assert!(
        journal(dir.path()).iter().any(
            |e| matches!(e, AgentEvent::InputResolved { request_id } if *request_id == old[0])
        )
    );
    wait_for("the chat to wait for an answer", || {
        core.sessions.session_status(CHAT).map(|s| s.status) == Some(SessionStatus::AwaitingInput)
    });
    assert!(
        core.sessions
            .respond_input(
                CHAT,
                asked.as_deref().unwrap(),
                vec![UserInputAnswer {
                    question_id: "pick".into(),
                    labels: vec!["B".into()],
                }],
            )
            .unwrap()
    );
    wait_for("the run to finish", || {
        matches!(journal(dir.path()).last(), Some(AgentEvent::Done { .. }))
    });
    assert_eq!(journaled_text(dir.path()), expected(Some("B")));
    wait_for("the reply to be saved", || {
        transcript(&core)
            .1
            .last()
            .is_some_and(|t| t == &expected(Some("B")))
    });
    let (users, replies) = transcript(&core);
    assert_eq!(users, 1);
    assert_eq!(replies, vec![expected(Some("B"))]);
    keeper_finishes(dir.path());
    runtime.block_on(core.shutdown());
}

#[test]
fn a_restarted_engine_picks_up_claude_code_mid_turn() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_path_buf();
    let mut run = request(dir.path());
    run.prompt = "scenario:kept chunks=20 ask=-1".into();
    std::thread::spawn({
        let run = run.clone();
        move || {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            let core = runtime.block_on(async {
                let core = assemble_with(&path, claude());
                core.sessions
                    .dispatch(CHAT, HarnessId::ClaudeCode, run, None)
                    .await
                    .unwrap();
                core
            });
            wait_for("the first engine to see output", || {
                journaled_text(&path).contains("chunk 3")
            });
            // Killed mid-turn: tasks stop where they are.
            drop(runtime);
            drop(core);
        }
    })
    .join()
    .unwrap();

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let _guard = runtime.enter();
    let core = assemble_with(dir.path(), claude());
    wait_for("the run to finish", || {
        matches!(journal(dir.path()).last(), Some(AgentEvent::Done { .. }))
    });
    let text: String = (0..20).map(|i| format!("chunk {i}\n")).collect();
    assert_eq!(journaled_text(dir.path()), text);
    let events = journal(dir.path());
    let count = |what: fn(&AgentEvent) -> bool| events.iter().filter(|e| what(e)).count();
    assert_eq!(count(|e| matches!(e, AgentEvent::Done { .. })), 1);
    assert_eq!(count(|e| matches!(e, AgentEvent::SessionStarted { .. })), 1);
    wait_for("the reply to be saved", || {
        transcript(&core).1.last().is_some_and(|t| t == &text)
    });
    let (users, replies) = transcript(&core);
    assert_eq!(users, 1);
    assert_eq!(replies, vec![text]);
    runtime.block_on(core.shutdown());
    // The parked session's keeper would outlive the test: end it.
    let root = dir.path().join("orgs/dev-org/dev-user/keepers");
    runtime.block_on(async {
        for found in keeper::client::discover(&root) {
            if let Ok(link) = keeper::client::connect(&found.dir, None).await {
                let _ = link.sender().finish();
            }
        }
    });
    keeper_finishes(dir.path());
}

// ── The update handoff (#11946) ─────────────────────────────────────────────

/// A run with no keeper: says one thing, then waits for `gate` before it ends.
struct Gated {
    gate: Arc<tokio::sync::Notify>,
}

#[async_trait::async_trait]
impl Harness for Gated {
    fn id(&self) -> HarnessId {
        HarnessId::Hermes
    }
    fn display_name(&self) -> &str {
        "Gated"
    }
    fn supports_steering(&self) -> bool {
        false
    }
    fn steering_mode(&self) -> openagents_proto::SteeringMode {
        openagents_proto::SteeringMode::TurnBoundary
    }
    fn reasoning_levels(&self) -> &[openagents_proto::ReasoningLevel] {
        &[]
    }
    async fn models(
        &self,
    ) -> Result<Vec<openagents_proto::Model>, openagents_harness::HarnessError> {
        Ok(Vec::new())
    }
    async fn run(
        &self,
        _request: RunRequest,
        _controls: openagents_harness::RunControls,
    ) -> Result<
        futures::stream::BoxStream<'static, Result<AgentEvent, openagents_harness::HarnessError>>,
        openagents_harness::HarnessError,
    > {
        use futures::StreamExt;
        let gate = self.gate.clone();
        Ok(futures::stream::unfold(0u8, move |step| {
            let gate = gate.clone();
            async move {
                let event = match step {
                    0 => AgentEvent::TextDelta {
                        text: "working\n".into(),
                    },
                    1 => {
                        gate.notified().await;
                        AgentEvent::Done {
                            status: openagents_proto::DoneStatus::Completed,
                            result: None,
                            error: None,
                            session_id: None,
                        }
                    }
                    _ => return None,
                };
                Some((Ok(event), step + 1))
            }
        })
        .boxed())
    }
}

#[test]
fn an_update_hands_a_live_turn_to_the_new_engine_with_no_gap_or_duplicate() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_path_buf();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let core = runtime.block_on(async {
            let core = assemble(&path, None);
            core.sessions
                .dispatch(CHAT, HarnessId::Mock, request(&path), None)
                .await
                .unwrap();
            core
        });
        wait_for("the first engine to see output", || {
            journaled_text(&path).contains("chunk 3")
        });
        // The run is under a keeper, so it doesn't hold the restart.
        wait_for("the run to be ready to hand over", || {
            core.sessions.handoff_ready()
        });
        assert!(
            core.sessions
                .hold_for_handoff(openagents_engine::sessions::HANDOFF_LEASE)
        );
        // New work waits for the new engine.
        let refused = runtime
            .block_on(
                core.sessions
                    .dispatch("chat-other", HarnessId::Mock, request(&path), None),
            )
            .unwrap_err();
        assert_eq!(
            refused.to_string(),
            openagents_engine::sessions::HANDOFF_REFUSAL
        );
        // The service manager stops the engine the way it stops for a restart.
        runtime.block_on(core.shutdown());
        drop(runtime);
        drop(core);
    })
    .join()
    .unwrap();

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let _guard = runtime.enter();
    let core = assemble(dir.path(), None);
    wait_for("the run to finish", || {
        matches!(journal(dir.path()).last(), Some(AgentEvent::Done { .. }))
    });
    assert_eq!(journaled_text(dir.path()), expected(None));
    let dones = journal(dir.path())
        .iter()
        .filter(|e| matches!(e, AgentEvent::Done { .. }))
        .count();
    assert_eq!(dones, 1);
    wait_for("the reply to be saved", || {
        transcript(&core)
            .1
            .last()
            .is_some_and(|t| t == &expected(None))
    });
    let (users, replies) = transcript(&core);
    assert_eq!(users, 1);
    assert_eq!(replies, vec![expected(None)]);
    keeper_finishes(dir.path());
    runtime.block_on(core.shutdown());
}

#[test]
fn a_drain_refuses_new_work_and_lets_a_run_without_a_keeper_finish() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let gate = Arc::new(tokio::sync::Notify::new());
    let core = {
        let _guard = runtime.enter();
        let registry = HarnessRegistry::new();
        registry.register(harness(None));
        registry.register(Arc::new(Gated { gate: gate.clone() }));
        EngineCore::assemble(dir.path(), Arc::new(registry), HarnessId::Hermes, None).unwrap()
    };
    runtime.block_on(async {
        core.sessions
            .dispatch(CHAT, HarnessId::Hermes, request(dir.path()), None)
            .await
            .unwrap();
        // A run without a keeper would stop, so it holds the handoff.
        assert!(!core.sessions.handoff_ready());
        assert!(
            !core
                .sessions
                .hold_for_handoff(openagents_engine::sessions::HANDOFF_LEASE)
        );
        // A drain that runs out of time lifts, and work is taken again.
        assert!(!core.sessions.drain(Duration::from_millis(100)).await);
        // Draining: new work is refused while the run finishes.
        let sessions = core.sessions.clone();
        let drained = tokio::spawn(async move { sessions.drain(Duration::from_secs(10)).await });
        tokio::time::sleep(Duration::from_millis(50)).await;
        let refused = core
            .sessions
            .dispatch("chat-other", HarnessId::Hermes, request(dir.path()), None)
            .await
            .unwrap_err();
        assert_eq!(
            refused.to_string(),
            openagents_engine::sessions::HANDOFF_REFUSAL
        );
        assert!(!drained.is_finished());
        gate.notify_one();
        assert!(drained.await.unwrap(), "the drain ends once the run has");
        assert!(matches!(
            journal(dir.path()).last(),
            Some(AgentEvent::Done {
                status: openagents_proto::DoneStatus::Completed,
                ..
            })
        ));
        // A handoff that doesn't happen takes work again.
        core.sessions.end_drain();
        core.sessions
            .dispatch("chat-other", HarnessId::Hermes, request(dir.path()), None)
            .await
            .unwrap();
        gate.notify_one();
        core.shutdown().await;
    });
}

/// A keeper from an earlier release, played by this test: it answers with
/// `welcome`, sends `frames`, and records every line an engine sends it.
struct FakeKeeper {
    received: Arc<std::sync::Mutex<Vec<String>>>,
    agent: std::process::Child,
}

impl Drop for FakeKeeper {
    fn drop(&mut self) {
        let _ = self.agent.kill();
        let _ = self.agent.wait();
    }
}

fn fake_keeper(dir: &Path, welcome: impl Fn(u32) -> String, frames: Vec<String>) -> FakeKeeper {
    use std::io::{BufRead, Write};
    // Stands in for the agent, so the run has a real process id.
    let agent = std::process::Command::new("sleep")
        .arg("60")
        .spawn()
        .unwrap();
    let welcome = welcome(agent.id());
    let run = dir.join("orgs/dev-org/dev-user/keepers/run-earlier");
    std::fs::create_dir_all(&run).unwrap();
    let socket = std::env::temp_dir().join(format!("oa-fake-{}.sock", uuid_like()));
    std::fs::write(run.join("token"), "token-earlier").unwrap();
    std::fs::write(
        run.join("launch.json"),
        serde_json::json!({
            "program": "/bin/false",
            "args": [],
            "cwd": null,
            "socket": socket,
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(
        run.join("meta.json"),
        serde_json::json!({
            "chatId": CHAT,
            "harness": HarnessId::Mock,
            "userMessageId": "user-earlier",
            "request": request(dir),
        })
        .to_string(),
    )
    .unwrap();
    let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
    let received = Arc::new(std::sync::Mutex::new(Vec::new()));
    let recorded = received.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { return };
            let recorded = recorded.clone();
            let welcome = welcome.clone();
            let frames = frames.clone();
            std::thread::spawn(move || {
                let mut writer = stream.try_clone().unwrap();
                let mut said_hello = false;
                for line in std::io::BufReader::new(stream).lines() {
                    let Ok(line) = line else { return };
                    recorded.lock().unwrap().push(line.clone());
                    if !said_hello && line.contains(r#""op":"hello""#) {
                        said_hello = true;
                        let _ = writeln!(writer, "{welcome}");
                        for frame in &frames {
                            let _ = writeln!(writer, "{frame}");
                        }
                    }
                }
            });
        }
    });
    FakeKeeper { received, agent }
}

fn uuid_like() -> String {
    format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )
}

#[test]
fn a_new_engine_picks_up_a_keeper_from_the_previous_release() {
    let dir = tempfile::tempdir().unwrap();
    // Protocol 1's lines, as the previous release writes them.
    let mut frames =
        vec![r#"{"op":"frame","seq":1,"kind":"stdin","line":"{\"prompt\":\"go\"}"}"#.to_owned()];
    let mut text = String::new();
    for i in 0..3 {
        let event = AgentEvent::TextDelta {
            text: format!("chunk {i}\n"),
        };
        text.push_str(&format!("chunk {i}\n"));
        frames.push(
            serde_json::json!({
                "op": "frame",
                "seq": i + 2,
                "kind": "stdout",
                "line": serde_json::to_string(&event).unwrap(),
            })
            .to_string(),
        );
    }
    let done = AgentEvent::Done {
        status: openagents_proto::DoneStatus::Completed,
        result: None,
        error: None,
        session_id: None,
    };
    frames.push(
        serde_json::json!({
            "op": "frame",
            "seq": 5,
            "kind": "stdout",
            "line": serde_json::to_string(&done).unwrap(),
        })
        .to_string(),
    );
    let keeper = fake_keeper(
        dir.path(),
        |pid| format!(r#"{{"op":"welcome","version":1,"pid":{pid},"last_seq":5}}"#),
        frames,
    );
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let _guard = runtime.enter();
    let core = assemble(dir.path(), None);
    wait_for("the run to finish", || {
        matches!(journal(dir.path()).last(), Some(AgentEvent::Done { .. }))
    });
    assert_eq!(journaled_text(dir.path()), text);
    wait_for("the engine to acknowledge the last frame", || {
        keeper
            .received
            .lock()
            .unwrap()
            .iter()
            .any(|line| line == r#"{"op":"ack","seq":5}"#)
    });
    // Everything the engine sent is protocol 1.
    for line in keeper.received.lock().unwrap().iter() {
        let value: serde_json::Value = serde_json::from_str(line).unwrap();
        let op = value["op"].as_str().unwrap();
        assert!(
            [
                "hello",
                "stdin",
                "closeStdin",
                "ack",
                "hold",
                "release",
                "signal",
                "finish"
            ]
            .contains(&op),
            "{line}"
        );
        if op == "hello" {
            assert_eq!(value["version"], 1, "{line}");
        }
    }
    runtime.block_on(core.shutdown());
}

#[test]
fn a_keeper_with_an_unknown_protocol_is_left_running() {
    // Older than this build adopts, and newer than it speaks.
    for version in [0, keeper::protocol::PROTOCOL + 1] {
        left_running(version);
    }
}

fn left_running(version: u32) {
    let dir = tempfile::tempdir().unwrap();
    // The chat's last turn never ended: without a keeper, the engine would
    // close it as interrupted and send the prompt again.
    let journal_dir = dir.path().join("orgs/dev-org/dev-user/journals");
    RunJournal::open(&journal_dir)
        .unwrap()
        .append(
            CHAT,
            &AgentEvent::TextDelta {
                text: "chunk 0\n".into(),
            },
        )
        .unwrap();
    let mut keeper = fake_keeper(
        dir.path(),
        move |pid| format!(r#"{{"op":"welcome","version":{version},"pid":{pid},"last_seq":9}}"#),
        Vec::new(),
    );
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let _guard = runtime.enter();
    let core = assemble(dir.path(), None);
    wait_for("the engine to say hello", || {
        !keeper.received.lock().unwrap().is_empty()
    });
    std::thread::sleep(Duration::from_millis(500));
    // It said hello and nothing else: no acknowledgement, signal, or finish.
    let received = keeper.received.lock().unwrap().clone();
    assert!(
        received.iter().all(|line| line.contains(r#""op":"hello""#)),
        "{received:?}"
    );
    // The chat is as the earlier engine left it, and nothing runs it again.
    assert_eq!(journaled_text(dir.path()), "chunk 0\n");
    assert!(
        !journal(dir.path())
            .iter()
            .any(|e| matches!(e, AgentEvent::Done { .. }))
    );
    assert!(
        core.sessions
            .session_status(CHAT)
            .is_none_or(|s| s.status != SessionStatus::Working)
    );
    assert!(
        keeper.agent.try_wait().unwrap().is_none(),
        "the agent runs on"
    );
    runtime.block_on(core.shutdown());
}
