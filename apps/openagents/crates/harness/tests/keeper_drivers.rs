//! Claude Code and Codex under keepers (#11930), against the fake CLIs in
//! `tests/fixtures`: an engine dies mid-turn, and a new engine picks the run
//! up with no gap or duplicate, asks a pending question again, and steers the
//! same process. No real `claude` or `codex` binary is involved.
#![cfg(unix)]

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use futures::stream::BoxStream;
use openagents_harness::keeper::{self, Detach, KeeperScope};
use openagents_harness::{
    CancellationToken, ClaudeHarness, CodexHarness, Harness, HarnessError, RunControls,
    SteerMessage, TurnControl,
};
use openagents_proto::{
    AgentEvent, DoneStatus, RunRequest, SandboxLevel, UserInputAnswer, UserInputQuestion,
};
use tokio::sync::{mpsc, oneshot};

/// Runs the keeper when this test binary is started as one.
#[test]
fn keeper_entry() {
    // The keeper finds its run folder in this variable; nothing else sets it.
    if std::env::var_os("OPENAGENTS_KEEPER_DIR").is_some() {
        std::process::exit(keeper::main(std::iter::empty()));
    }
}

#[derive(Clone, Copy, Debug)]
enum Agent {
    Claude,
    Codex,
}

fn fixture(name: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755));
    path
}

fn harness(agent: Agent, keepers: bool) -> Arc<dyn Harness> {
    match agent {
        Agent::Claude => Arc::new(
            ClaudeHarness::new()
                .with_executable(fixture("fake-claude.sh"))
                .with_keepers(keepers),
        ),
        Agent::Codex => Arc::new(
            CodexHarness::new()
                .with_executable(fixture("fake-codex.sh"))
                .with_keepers(keepers),
        ),
    }
}

fn scope(root: &Path) -> KeeperScope {
    KeeperScope {
        root: root.to_path_buf(),
        keeper_exe: std::env::current_exe().unwrap(),
        keeper_args: ["--exact", "keeper_entry", "--nocapture", "--test-threads=1"]
            .map(OsString::from)
            .to_vec(),
        meta: serde_json::json!({ "chat": "c1" }),
        orphan_after: Duration::from_secs(60),
        detach: Detach::default(),
    }
}

/// The fake reads its settings from the prompt.
fn request(dir: &Path, chunks: usize, ask_at: Option<usize>) -> RunRequest {
    RunRequest {
        prompt: format!(
            "scenario:kept chunks={chunks} ask={}",
            ask_at.map_or(-1, |at| at as i64)
        ),
        harness: None,
        model: None,
        reasoning: None,
        model_options: Default::default(),
        cwd: dir.display().to_string(),
        sandbox: SandboxLevel::WorkspaceWrite,
        auto_approve: false,
        resume: None,
        attachments: Vec::new(),
        worktree: None,
        mcp: None,
    }
}

type Asked = mpsc::UnboundedSender<(
    Vec<UserInputQuestion>,
    oneshot::Sender<Vec<UserInputAnswer>>,
)>;

fn controls(asked: Option<Asked>) -> (RunControls, mpsc::Sender<SteerMessage>) {
    let (steer, steering) = mpsc::channel(4);
    let controls = RunControls {
        realtime: None,
        execution_lease: None,
        request_input: Box::new(move |questions| {
            let (tx, rx) = oneshot::channel();
            if let Some(asked) = &asked {
                let _ = asked.send((questions, tx));
            } else {
                std::mem::forget(tx); // never answered
            }
            rx
        }),
        steering,
        interrupt: CancellationToken::new(),
        turn: TurnControl::default(),
    };
    (controls, steer)
}

/// The reply's text. Codex separates its messages with blank lines, so
/// trailing ones don't count.
fn texts(events: &[AgentEvent]) -> String {
    let text: String = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::TextDelta { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    format!("{}\n", text.trim_end())
}

fn count(events: &[AgentEvent], what: fn(&AgentEvent) -> bool) -> usize {
    events.iter().filter(|e| what(e)).count()
}

/// Engine A: start the run, hand over events until `enough`, then die with
/// its runtime, as a killed engine does.
fn first_engine(
    agent: Agent,
    root: PathBuf,
    request: RunRequest,
    enough: fn(&[AgentEvent]) -> bool,
) -> Vec<AgentEvent> {
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let (events, stream) = runtime.block_on(async {
            let harness = harness(agent, true);
            let (controls, steer) = controls(None);
            std::mem::forget(steer);
            let mut stream = keeper::scope(scope(&root), harness.run(request, controls))
                .await
                .unwrap();
            let mut events = Vec::new();
            while !enough(&events) {
                events.push(stream.next().await.unwrap().unwrap());
            }
            (events, stream)
        });
        drop(runtime);
        drop(stream);
        events
    })
    .join()
    .unwrap()
}

async fn wait_for(what: &str, mut check: impl FnMut() -> bool) {
    for _ in 0..1000 {
        if check() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("timed out waiting for {what}");
}

/// Engine B: pick up the one kept run.
async fn adopt(
    agent: Agent,
    root: &Path,
    request: RunRequest,
    asked: Option<Asked>,
) -> (
    BoxStream<'static, Result<AgentEvent, HarnessError>>,
    mpsc::Sender<SteerMessage>,
    PathBuf,
) {
    let found = keeper::client::discover(root);
    assert_eq!(found.len(), 1);
    let link = keeper::client::connect(&found[0].dir, None).await.unwrap();
    let (controls, steer) = controls(asked);
    let stream = keeper::scope(
        scope(root),
        harness(agent, true).attach(request, controls, link),
    )
    .await
    .expect("the driver picks up runs")
    .unwrap();
    (stream, steer, found[0].dir.clone())
}

async fn until_done(
    stream: &mut BoxStream<'static, Result<AgentEvent, HarnessError>>,
) -> Vec<AgentEvent> {
    let mut events = Vec::new();
    loop {
        let event = tokio::time::timeout(Duration::from_secs(20), stream.next())
            .await
            .expect("the run keeps going")
            .expect("the stream stays open")
            .unwrap();
        let done = matches!(event, AgentEvent::Done { .. });
        events.push(event);
        if done {
            return events;
        }
    }
}

fn chunks(upto: usize, answer: Option<(usize, &str)>) -> String {
    let mut text = String::new();
    for i in 0..upto {
        if let Some((at, word)) = answer
            && at == i
        {
            text.push_str(word);
            text.push('\n');
        }
        text.push_str(&format!("chunk {i}\n"));
    }
    text
}

async fn picks_up_mid_turn_and_steers(agent: Agent) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("keepers");
    let run = request(dir.path(), 20, None);
    let before = first_engine(agent, root.clone(), run.clone(), |events| {
        texts(events).contains("chunk 3")
    });
    let found = keeper::client::discover(&root);
    let journal = found[0].dir.join("journal.jsonl");
    // The agent keeps streaming with no engine.
    wait_for("the agent to keep streaming", || {
        std::fs::read_to_string(&journal).is_ok_and(|j| j.contains("chunk 8"))
    })
    .await;

    let (mut stream, steer, run) = adopt(agent, &root, run, None).await;
    let after = until_done(&mut stream).await;
    let all: Vec<AgentEvent> = before.iter().chain(&after).cloned().collect();
    assert_eq!(
        texts(&all),
        chunks(20, None),
        "{agent:?}: no gap or duplicate"
    );
    assert_eq!(
        count(&all, |e| matches!(e, AgentEvent::SessionStarted { .. })),
        1,
        "{agent:?}"
    );
    assert_eq!(count(&all, |e| matches!(e, AgentEvent::Done { .. })), 1);
    assert!(matches!(
        after.last(),
        Some(AgentEvent::Done {
            status: DoneStatus::Completed,
            ..
        })
    ));

    // Steering after the pickup reaches the same process.
    steer.send(SteerMessage::text("again")).await.unwrap();
    let next = until_done(&mut stream).await;
    assert_eq!(texts(&next), "heard again\n", "{agent:?}: {next:?}");
    if let Some(AgentEvent::Done { status, error, .. }) = next.last() {
        assert_eq!(*status, DoneStatus::Completed, "{agent:?}: {error:?}");
    }

    // Letting the run go stops the agent and removes its folder.
    drop(steer);
    drop(stream);
    wait_for("the run folder to go", || !run.exists()).await;
}

async fn a_pending_question_comes_back(agent: Agent) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("keepers");
    let run = request(dir.path(), 6, Some(2));
    let before = first_engine(agent, root.clone(), run.clone(), |events| {
        texts(events).contains("chunk 1")
    });
    let found = keeper::client::discover(&root);
    let journal = found[0].dir.join("journal.jsonl");
    let ask = match agent {
        Agent::Claude => "control_request",
        Agent::Codex => "requestApproval",
    };
    wait_for("the question", || {
        std::fs::read_to_string(&journal).is_ok_and(|j| j.contains(ask))
    })
    .await;
    // Give the first engine's handover time to settle in the journal.
    tokio::time::sleep(Duration::from_millis(100)).await;

    let (asked_tx, mut asked) = mpsc::unbounded_channel();
    let (mut stream, _steer, _) = adopt(agent, &root, run, Some(asked_tx)).await;
    let collect = tokio::spawn(async move { until_done(&mut stream).await });
    let (questions, answer) = tokio::time::timeout(Duration::from_secs(5), asked.recv())
        .await
        .expect("the question comes back")
        .unwrap();
    let (label, word) = match agent {
        Agent::Claude => ("B", "answered B"),
        Agent::Codex => ("Yes", "accepted"),
    };
    answer
        .send(vec![UserInputAnswer {
            question_id: questions[0].id.clone(),
            labels: vec![label.into()],
        }])
        .unwrap();
    let after = collect.await.unwrap();
    let all: Vec<AgentEvent> = before.iter().chain(&after).cloned().collect();
    assert_eq!(texts(&all), chunks(6, Some((2, word))), "{agent:?}");
    // The new engine asks it exactly once.
    assert!(asked.try_recv().is_err());
}

async fn the_escape_hatch_starts_without_a_keeper(agent: Agent) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("keepers");
    let (controls, _steer) = controls(None);
    let mut stream = keeper::scope(
        scope(&root),
        harness(agent, false).run(request(dir.path(), 3, None), controls),
    )
    .await
    .unwrap();
    let events = until_done(&mut stream).await;
    assert_eq!(texts(&events), chunks(3, None), "{agent:?}");
    assert!(keeper::client::discover(&root).is_empty());
}

#[tokio::test]
async fn claude_code_is_picked_up_mid_turn_and_steered() {
    picks_up_mid_turn_and_steers(Agent::Claude).await;
}

#[tokio::test]
async fn codex_is_picked_up_mid_turn_and_steered() {
    picks_up_mid_turn_and_steers(Agent::Codex).await;
}

#[tokio::test]
async fn a_pending_claude_code_question_comes_back() {
    a_pending_question_comes_back(Agent::Claude).await;
}

#[tokio::test]
async fn a_pending_codex_approval_comes_back() {
    a_pending_question_comes_back(Agent::Codex).await;
}

#[tokio::test]
async fn claude_code_starts_without_a_keeper_when_turned_off() {
    the_escape_hatch_starts_without_a_keeper(Agent::Claude).await;
}

#[tokio::test]
async fn codex_starts_without_a_keeper_when_turned_off() {
    the_escape_hatch_starts_without_a_keeper(Agent::Codex).await;
}
