//! Keepers (#11930): an agent outlives the engine that started it, and a new
//! engine picks the run up with no gap or duplicate.
#![cfg(unix)]

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use futures::StreamExt;
use openagents_harness::keeper::{self, Detach, KeeperScope, lines::LineHarness};
use openagents_harness::{CancellationToken, Harness, RunControls, TurnControl};
use openagents_proto::{
    AgentEvent, DoneStatus, HarnessId, RunRequest, SandboxLevel, UserInputAnswer, UserInputQuestion,
};
use tokio::sync::{mpsc, oneshot};

/// Runs the keeper when this test binary is started as one.
#[test]
fn keeper_entry() {
    if std::env::var_os("KEEPER_TEST_ENTRY").is_some() {
        std::process::exit(keeper::main(std::iter::empty()));
    }
}

fn harness(chunks: usize, ask_at: Option<usize>) -> LineHarness {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/keeper-agent.py");
    LineHarness {
        id: HarnessId::ClaudeCode,
        name: "Fake".into(),
        program: fixture,
        args: Vec::new(),
        env: vec![
            ("KEEPER_TEST_ENTRY".into(), "1".into()),
            ("KEEPER_FAKE_CHUNKS".into(), chunks.to_string().into()),
            ("KEEPER_FAKE_INTERVAL_MS".into(), "15".into()),
            (
                "KEEPER_FAKE_ASK_AT".into(),
                ask_at.map_or("-1".into(), |at| at.to_string()).into(),
            ),
        ],
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

fn request(dir: &Path) -> RunRequest {
    RunRequest {
        prompt: "go".into(),
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

fn controls(asked: Option<Asked>) -> RunControls {
    let (_steer_tx, steering) = mpsc::channel(4);
    RunControls {
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
    }
}

fn texts(events: &[AgentEvent]) -> String {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::TextDelta { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

/// Engine A: start the run, handle `take` events, then die with its runtime.
fn first_engine(
    root: PathBuf,
    work: PathBuf,
    take: usize,
    ask_at: Option<usize>,
) -> Vec<AgentEvent> {
    first_engine_with(scope(&root), work, 20, take, ask_at)
}

fn first_engine_with(
    scope: KeeperScope,
    work: PathBuf,
    chunks: usize,
    take: usize,
    ask_at: Option<usize>,
) -> Vec<AgentEvent> {
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let (events, stream) = runtime.block_on(async {
            let harness = harness(chunks, ask_at);
            let mut stream = keeper::scope(scope, harness.run(request(&work), controls(None)))
                .await
                .unwrap();
            let mut events = Vec::new();
            while events.len() < take {
                events.push(stream.next().await.unwrap().unwrap());
            }
            (events, stream)
        });
        // Dropping the runtime cancels every task, as a killed engine would;
        // only then does the stream go, closing the socket with no cleanup.
        drop(runtime);
        drop(stream);
        events
    })
    .join()
    .unwrap()
}

async fn wait_for(what: &str, mut check: impl FnMut() -> bool) {
    for _ in 0..500 {
        if check() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("timed out waiting for {what}");
}

#[tokio::test]
async fn a_new_engine_adopts_the_run_with_no_gap_or_duplicate() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("keepers");
    let before = first_engine(root.clone(), dir.path().to_path_buf(), 4, None);
    assert_eq!(texts(&before), "chunk 0\nchunk 1\nchunk 2\nchunk 3\n");

    // The agent keeps streaming with no engine.
    let found = keeper::client::discover(&root);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].meta["chat"], "c1");
    let journal = found[0].dir.join("journal.jsonl");
    wait_for("the agent to keep streaming", || {
        std::fs::read_to_string(&journal).is_ok_and(|j| j.contains("chunk 8"))
    })
    .await;

    // Engine B adopts it.
    let link = keeper::client::connect(&found[0].dir, None).await.unwrap();
    // SAFETY: signal 0 only checks that the agent is alive.
    assert_eq!(unsafe { libc::kill(link.pid as i32, 0) }, 0);
    let mut stream = harness(20, None)
        .attach(request(dir.path()), controls(None), link)
        .await
        .unwrap()
        .unwrap();
    let mut after = Vec::new();
    while let Some(event) = stream.next().await {
        let event = event.unwrap();
        let done = matches!(event, AgentEvent::Done { .. });
        after.push(event);
        if done {
            break;
        }
    }
    let expected: String = (0..20).map(|i| format!("chunk {i}\n")).collect();
    assert_eq!(format!("{}{}", texts(&before), texts(&after)), expected);
    assert!(matches!(
        after.last(),
        Some(AgentEvent::Done {
            status: DoneStatus::Completed,
            ..
        })
    ));
    // Once the end is acknowledged, the keeper removes its folder.
    drop(stream);
    wait_for("the run folder to go", || !found[0].dir.exists()).await;
}

#[tokio::test]
async fn a_pending_question_is_asked_again_after_adoption() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("keepers");
    // Engine A sees the first two chunks; the question comes at chunk 2 and
    // engine A never answers before it dies.
    let before = first_engine(root.clone(), dir.path().to_path_buf(), 2, Some(2));
    assert_eq!(texts(&before), "chunk 0\nchunk 1\n");
    let found = keeper::client::discover(&root);
    let journal = found[0].dir.join("journal.jsonl");
    wait_for("the question", || {
        std::fs::read_to_string(&journal).is_ok_and(|j| j.contains("\\\"ask\\\""))
    })
    .await;

    let (asked_tx, mut asked) = mpsc::unbounded_channel();
    let link = keeper::client::connect(&found[0].dir, None).await.unwrap();
    let mut stream = harness(20, Some(2))
        .attach(request(dir.path()), controls(Some(asked_tx)), link)
        .await
        .unwrap()
        .unwrap();
    let collect = tokio::spawn(async move {
        let mut events = Vec::new();
        while let Some(Ok(event)) = stream.next().await {
            let done = matches!(event, AgentEvent::Done { .. });
            events.push(event);
            if done {
                break;
            }
        }
        events
    });
    let (questions, answer) = tokio::time::timeout(Duration::from_secs(5), asked.recv())
        .await
        .expect("the question comes back")
        .unwrap();
    assert_eq!(questions[0].question, "Which one?");
    answer
        .send(vec![UserInputAnswer {
            question_id: "pick".into(),
            labels: vec!["B".into()],
        }])
        .unwrap();
    let after = collect.await.unwrap();
    let mut expected = String::from("chunk 0\nchunk 1\nanswered B\n");
    expected.extend((2..20).map(|i| format!("chunk {i}\n")));
    assert_eq!(format!("{}{}", texts(&before), texts(&after)), expected);
    // The question is asked exactly once by the new engine.
    assert!(asked.try_recv().is_err());
}

#[tokio::test]
async fn a_wrong_token_or_version_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("keepers");
    let mut stream = keeper::scope(
        scope(&root),
        harness(200, None).run(request(dir.path()), controls(None)),
    )
    .await
    .unwrap();
    stream.next().await.unwrap().unwrap();
    let found = keeper::client::discover(&root);
    let run = &found[0].dir;
    use std::os::unix::fs::PermissionsExt;
    for name in ["token", "meta.json", "launch.json", "journal.jsonl"] {
        let mode = std::fs::metadata(run.join(name))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "{name}");
    }
    let mode = std::fs::metadata(run).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o700);

    let launch: serde_json::Value =
        serde_json::from_slice(&std::fs::read(run.join("launch.json")).unwrap()).unwrap();
    let socket = PathBuf::from(launch["socket"].as_str().unwrap());
    let mode = std::fs::metadata(&socket).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    for hello in [
        r#"{"op":"hello","version":1,"token":"nope"}"#.to_owned(),
        format!(
            r#"{{"op":"hello","version":0,"token":"{}"}}"#,
            std::fs::read_to_string(run.join("token")).unwrap().trim()
        ),
    ] {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
        let mut stream = tokio::net::UnixStream::connect(&socket).await.unwrap();
        stream
            .write_all(format!("{hello}\n").as_bytes())
            .await
            .unwrap();
        let mut lines = tokio::io::BufReader::new(stream).lines();
        let reply = lines.next_line().await.unwrap().unwrap();
        assert!(reply.contains("\"refused\""), "{reply}");
    }
    // Finish stops the agent and removes the folder.
    drop(stream);
    keeper::client::connect(run, None)
        .await
        .unwrap()
        .sender()
        .finish()
        .unwrap();
    wait_for("the run folder to go", || !run.exists()).await;
}

#[tokio::test]
async fn an_orphaned_keeper_interrupts_its_agent_and_keeps_the_journal() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("keepers");
    let mut short = scope(&root);
    short.orphan_after = Duration::from_secs(1);
    first_engine_with(short, dir.path().to_path_buf(), 1000, 1, None);
    let found = keeper::client::discover(&root);
    let run = found[0].dir.clone();
    let journal = run.join("journal.jsonl");
    for _ in 0..600 {
        if !run.join("token").exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(!run.join("token").exists(), "the keeper exited");
    let journal = std::fs::read_to_string(journal).unwrap();
    assert!(journal.contains(r#""kind":"exit""#), "{journal}");
    assert!(
        journal.contains("interrupted"),
        "the agent was interrupted: {journal}"
    );
}
