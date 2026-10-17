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
