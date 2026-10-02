//! The client's event stream against the in-process door and a fake host.

use std::sync::Mutex;

use super::*;
use crate::basic_chats::Spawned;
use crate::basic_coder::{Door, Reply, lock};
use crate::coder_events::{self, Asked};
use crate::router::{ClientWord, Meta, Offer, Surface};

/// A worker that streams one partial, then answers; a coding message gets
/// a `run_coder` offer. It keeps the context each turn was sent with.
struct Worker {
    contexts: Arc<Mutex<Vec<Context>>>,
    coding: bool,
}

impl Door for Worker {
    fn ask(
        &self,
        turns: Vec<Turn>,
        context: Context,
        reply: Arc<std::sync::Mutex<Reply>>,
    ) -> BoxFuture<'static, ()> {
        self.contexts.lock().unwrap().push(context);
        let coding = self.coding;
        Box::pin(async move {
            let asked = turns.last().unwrap().text.clone();
            lock(&reply).text = "Working".into();
            tokio::time::sleep(Duration::from_millis(200)).await;
            let mut reply = lock(&reply);
            reply.text = format!("You said: {asked}");
            if coding {
                reply.meta = Meta {
                    offers: vec![Offer::RunCoder],
                    engine: Some(nostr::cj_conversation::Engine::ClaudeCode),
                    ..Meta::default()
                };
            }
            reply.done = true;
        })
    }
}

/// A worker that never answers.
struct Silent;

impl Door for Silent {
    fn ask(
        &self,
        _: Vec<Turn>,
        _: Context,
        _: Arc<std::sync::Mutex<Reply>>,
    ) -> BoxFuture<'static, ()> {
        Box::pin(std::future::pending())
    }
}

/// Coder that starts at once and asks one question, recording what it was
/// started with.
#[derive(Default)]
struct FakeCoder {
    asks_first: bool,
    started: Mutex<Vec<(String, Option<nostr::cj_conversation::Engine>)>>,
    /// Who it says would run.
    predicts: Option<Runner>,
    /// The project folders it was asked to make ready.
    warmed: Mutex<Vec<PathBuf>>,
    /// The task's last turn ended.
    ended: bool,
    /// What it was told to continue with.
    answered: Mutex<Vec<String>>,
    /// What a working run was sent.
    steered: Mutex<Vec<String>>,
    /// What a pickup finds: the issue it picked, or why none is free.
    picks: Option<Result<(u64, String), String>>,
}

/// An issue a pickup chose, or the reason it found none (number 0).
struct Picked(u64, Result<String, String>);

impl Issue for Picked {
    fn number(&self) -> u64 {
        self.0
    }
    fn picked(&self) -> Option<String> {
        self.1.clone().ok()
    }
    fn begin(self: Box<Self>, _: &Path, _: &Path, _: &str) -> Result<IssueStarted, String> {
        self.1.map(|_| IssueStarted {
            started: Started {
                task: "t1".into(),
                project: "demo".into(),
                worktree: "/tmp/demo-t1".into(),
            },
            url: format!("https://github.com/o/r/issues/{}", self.0),
            finish: Box::new(|| {}),
        })
    }
}

impl Coder for FakeCoder {
    fn default_store(&self) -> PathBuf {
        std::env::temp_dir().join("openagents-chat-client-test-tasks")
    }
    fn context(&self, _: &Path, _: Option<&Path>) -> Context {
        Context {
            computer_ready: true,
            ..Context::default()
        }
    }
    fn predict(&self, _: &Path, _: Option<nostr::cj_conversation::Engine>) -> Option<Runner> {
        self.predicts.clone()
    }
    fn asks_first(&self) -> bool {
        self.asks_first
    }
    fn checkout(&self, _: &Path) -> Result<(), String> {
        Ok(())
    }
    fn warm(&self, _: &Path, dir: &Path) {
        self.warmed.lock().unwrap().push(dir.to_path_buf());
    }
    fn start(
        &self,
        _: &Path,
        _: &Path,
        _: &str,
        prompt: &str,
        _: &str,
        requested: Option<nostr::cj_conversation::Engine>,
    ) -> Result<Started, String> {
        self.started
            .lock()
            .unwrap()
            .push((prompt.to_owned(), requested));
        Ok(Started {
            task: "t1".into(),
            project: "demo".into(),
            worktree: "/tmp/demo-t1".into(),
        })
    }
    fn issue(&self, _: &str, _: &str, _: &Path) -> Option<Box<dyn Issue>> {
        Some(match self.picks.clone()? {
            Ok((number, title)) => Box::new(Picked(number, Ok(title))),
            Err(why) => Box::new(Picked(0, Err(why))),
        })
    }
    fn follow(&self, _: &Path, task: &str, chat: &str, hint: Option<String>) -> Box<dyn Follow> {
        struct Once(Option<Line>);
        impl Follow for Once {
            fn poll(&mut self) -> Result<(Vec<Line>, Progress), String> {
                Ok((self.0.take().into_iter().collect(), Progress::Waiting))
            }
        }
        Box::new(Once(Some(Line {
            seq: 1,
            task: task.to_owned(),
            thread: Some(chat.to_owned()),
            event: CoderEvent::Question(Asked {
                turn: 1,
                text: "Which branch?".into(),
                answer: hint,
            }),
        })))
    }
    fn stop(&self, _: &Path, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn answer(&self, _: &Path, _: &str, text: &str) -> Result<usize, String> {
        self.answered.lock().unwrap().push(text.to_owned());
        Ok(2)
    }
    fn steer(&self, _: &Path, _: &str, text: &str) -> Result<Steering, String> {
        self.steered.lock().unwrap().push(text.to_owned());
        Ok(Steering::NextStep)
    }
    fn result(&self, _: &Path, _: &str) -> Option<CoderRun> {
        self.ended.then(|| CoderRun {
            ending: crate::router::RunEnding::Finished,
            turn: 1,
            engine: Some("grok".into()),
            model: None,
            summary: "Done.".into(),
            files: Vec::new(),
            commands: Vec::new(),
        })
    }
    fn trajectories(&self, _: &Path, _: &str) -> Vec<Value> {
        Vec::new()
    }
}

fn hint(kind: Kind, thread: &str) -> String {
    format!("answer {} {thread}", kind.word())
}

fn options(dir: &Path) -> Options {
    Options {
        place: Place::Local,
        caller: Caller::TERMINAL,
        dir: Some(dir.to_path_buf()),
        home: dir.to_path_buf(),
        interrupt: never(),
        hint: Some(hint),
    }
}

fn in_process(door: Arc<dyn Door>, options: Options, coder: Arc<dyn Coder>) -> Client {
    let chats = BasicChats::new(Some(tokio::runtime::Handle::current()), Some(door), None);
    let home = options.home.clone();
    Client::in_process(chats, home, false, options, coder)
}

async fn drain(stream: Stream) -> (Vec<Event>, Client, Result<Ended, Error>) {
    let Stream { mut events, done } = stream;
    let mut seen = Vec::new();
    while let Some(event) = events.recv().await {
        seen.push(event);
    }
    let (client, ended) = done.await.unwrap();
    (seen, client, ended)
}

fn send(thread: &str, text: &str, start: Start) -> Op {
    Op::Send {
        thread: thread.into(),
        new: true,
        text: text.into(),
        start,
        timeout: Duration::from_secs(10),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_send_streams_typed_events_and_says_the_terminal_sent_it() {
    let dir = tempfile::tempdir().unwrap();
    let contexts = Arc::new(Mutex::new(Vec::new()));
    let door = Arc::new(Worker {
        contexts: contexts.clone(),
        coding: false,
    });
    let client = in_process(door, options(dir.path()), Arc::new(NoCoder));
    let thread = new_id();
    let (events, client, ended) =
        drain(client.stream(send(&thread, "hello", Start::Settings))).await;
    assert_eq!(ended, Ok(Ended::Done));
    assert!(matches!(
        &events[0],
        Event::Accepted { thread: t, new: true, backend: Kind::InProcess, .. } if *t == thread
    ));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::Partial { text, .. } if text == "Working")),
        "{events:?}"
    );
    let Some(Event::Reply { reply, running, .. }) = events.last() else {
        panic!("{events:?}");
    };
    assert_eq!(reply.text, "You said: hello");
    assert!(!running);
    // The turn told the worker it came from OpenAgents Terminal.
    let sent = contexts.lock().unwrap();
    assert_eq!(sent[0].surface, Surface::Terminal);
    assert_eq!(sent[0].client, Some(ClientWord::Terminal));
    assert_eq!(sent[0].client_word(), "openagents-terminal");
    assert_eq!(client.caller(), Caller::TERMINAL);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_coding_reply_starts_coder_at_once_and_streams_its_events() {
    let dir = tempfile::tempdir().unwrap();
    let door = Arc::new(Worker {
        contexts: Arc::default(),
        coding: true,
    });
    let coder = Arc::new(FakeCoder::default());
    let client = in_process(door, options(dir.path()), coder.clone());
    let thread = new_id();
    let (events, _, ended) =
        drain(client.stream(send(&thread, "fix the build", Start::Settings))).await;
    // A question ends the turn as asked, which is not a failure.
    assert_eq!(ended, Ok(Ended::Done));
    let names: Vec<&str> = events
        .iter()
        .map(|event| match event {
            Event::Accepted { .. } => "accepted",
            Event::Partial { .. } => "partial",
            Event::Reply { running: true, .. } => "reply-running",
            Event::Coder { accepted: true, .. } => "coder",
            Event::Line(line) => line.event.name(),
            _ => "other",
        })
        .filter(|name| *name != "partial")
        .collect();
    assert_eq!(
        names,
        ["accepted", "reply-running", "coder", "question"],
        "{events:?}"
    );
    let Some(Event::Line(line)) = events.last() else {
        unreachable!()
    };
    let CoderEvent::Question(asked) = &line.event else {
        unreachable!()
    };
    assert_eq!(
        asked.answer.as_deref(),
        Some(format!("answer in_process {thread}").as_str())
    );
    // It started on the shared handoff, asking for the engine the offer named.
    let started = coder.started.lock().unwrap();
    assert_eq!(started.len(), 1);
    assert!(
        started[0].0.starts_with("fix the build\n\n"),
        "{}",
        started[0].0
    );
    assert_eq!(
        started[0].1,
        Some(nostr::cj_conversation::Engine::ClaudeCode)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_pickup_names_the_issue_it_picked_or_says_none_is_free() {
    let dir = tempfile::tempdir().unwrap();
    let door = Arc::new(Worker {
        contexts: Arc::default(),
        coding: true,
    });
    let coder = Arc::new(FakeCoder {
        picks: Some(Ok((42, "Fix the thing".into()))),
        ..FakeCoder::default()
    });
    let client = in_process(door.clone(), options(dir.path()), coder.clone());
    let (events, _, _) = drain(client.stream(send(
        &new_id(),
        "pick an open issue nobody is on and take it",
        Start::Settings,
    )))
    .await;
    assert!(
        events.iter().any(|event| matches!(
            event,
            Event::Coder { accepted: true, message, .. } if message == "Picking up #42: Fix the thing."
        )),
        "{events:?}"
    );
    // The flow took it; nothing ran as plain coding work.
    assert!(coder.started.lock().unwrap().is_empty());

    let coder = Arc::new(FakeCoder {
        picks: Some(Err(
            "No open issue of o/r is free to pick up: all 3 are claimed.".into(),
        )),
        ..FakeCoder::default()
    });
    let client = in_process(door, options(dir.path()), coder.clone());
    let (events, _, _) = drain(client.stream(send(
        &new_id(),
        "pick an open issue nobody is on and take it",
        Start::Settings,
    )))
    .await;
    assert!(
        events.iter().any(|event| matches!(
            event,
            Event::Coder { accepted: false, message, .. }
                if message == "No open issue of o/r is free to pick up: all 3 are claimed."
        )),
        "{events:?}"
    );
    assert!(coder.started.lock().unwrap().is_empty());
}

/// A coding message to a thread whose run ended starts the run's next turn
/// with it; one to a thread whose run still works follows that run. A
/// follow of an ended run only replayed what the screen showed, and the
/// screen sat on "working" with nothing coming (owner, 2026-10-02).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_message_to_an_ended_run_continues_it_and_one_to_a_working_run_steers_it() {
    for ended in [true, false] {
        let dir = tempfile::tempdir().unwrap();
        let door = Arc::new(Worker {
            contexts: Arc::default(),
            coding: true,
        });
        let coder = Arc::new(FakeCoder {
            ended,
            ..FakeCoder::default()
        });
        let client = in_process(door, options(dir.path()), coder.clone());
        let thread = new_id();
        let (_, client, _) =
            drain(client.stream(send(&thread, "clone grok-build to ~", Start::Settings))).await;
        let mut again = send(&thread, "summarize its latest 3 commits", Start::Settings);
        if let Op::Send { new, .. } = &mut again {
            *new = false;
        }
        let (events, _, _) = drain(client.stream(again)).await;
        let said: Vec<&str> = events
            .iter()
            .filter_map(|event| match event {
                Event::Coder {
                    accepted: true,
                    message,
                    ..
                } => Some(message.as_str()),
                _ => None,
            })
            .collect();
        let answered = coder.answered.lock().unwrap().clone();
        let steered = coder.steered.lock().unwrap().clone();
        if ended {
            assert_eq!(
                said,
                ["Coder continues task t1 with your message."],
                "{events:?}"
            );
            assert_eq!(answered, ["summarize its latest 3 commits"]);
            assert!(steered.is_empty());
        } else {
            // The working run's session reads the message; following alone
            // would leave it unread.
            assert_eq!(
                said,
                ["Sent. Coder reads it at its next step."],
                "{events:?}"
            );
            assert!(answered.is_empty());
            assert_eq!(steered, ["summarize its latest 3 commits"]);
        }
        assert_eq!(
            coder.started.lock().unwrap().len(),
            1,
            "one task, never two"
        );
    }
}

/// A start says who is starting before its own work, then lets the run's
/// start line speak: the `coder` event carries the task for `--json` and
/// is quiet (#10115). Opening the client made its project ready.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_start_says_who_is_starting_then_one_line() {
    let dir = tempfile::tempdir().unwrap();
    let door = Arc::new(Worker {
        contexts: Arc::default(),
        coding: true,
    });
    let coder = Arc::new(FakeCoder {
        predicts: Some(Runner::Runs {
            provider: "grok".into(),
            model: "default".into(),
            passed: Vec::new(),
            requested: Some("grok".into()),
        }),
        ..FakeCoder::default()
    });
    let client = in_process(door, options(dir.path()), coder.clone());
    assert_eq!(*coder.warmed.lock().unwrap(), [dir.path().to_path_buf()]);
    let thread = new_id();
    let (events, _, ended) = drain(client.stream(send(
        &thread,
        "do a test delegation to grok",
        Start::Settings,
    )))
    .await;
    assert_eq!(ended, Ok(Ended::Done));
    let names: Vec<String> = events
        .iter()
        .filter_map(|event| match event {
            Event::Reply { running: true, .. } => Some("reply-running".into()),
            Event::Starting { engine, .. } => Some(format!("starting {engine}")),
            Event::Coder {
                accepted: true,
                quiet: true,
                task: Some(task),
                ..
            } => Some(format!("coder {}", task["worktree"])),
            Event::Coder { .. } => Some("loud coder".into()),
            Event::Line(line) => Some(line.event.name().into()),
            _ => None,
        })
        .collect();
    assert_eq!(
        names,
        [
            "reply-running",
            "starting grok",
            "coder \"/tmp/demo-t1\"",
            "question"
        ],
        "{events:?}"
    );
    assert_eq!(coder_events::starting("grok"), "Starting Grok Build…");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ask_first_keeps_only_the_offer_until_the_person_runs_it() {
    let dir = tempfile::tempdir().unwrap();
    let door = Arc::new(Worker {
        contexts: Arc::default(),
        coding: true,
    });
    let coder = Arc::new(FakeCoder {
        asks_first: true,
        ..FakeCoder::default()
    });
    let mut client = in_process(door, options(dir.path()), coder.clone());
    let thread = new_id();
    let mut events = Vec::new();
    let ended = client
        .run(
            send(&thread, "fix the build", Start::Settings),
            &mut |event| events.push(event),
        )
        .await;
    assert_eq!(ended, Ok(Ended::Done));
    assert!(matches!(
        events.last(),
        Some(Event::Reply { running: false, .. })
    ));
    assert!(coder.started.lock().unwrap().is_empty());
    // Accepting the offer runs it.
    events.clear();
    let ended = client
        .run(
            Op::RunCoder {
                thread: thread.clone(),
            },
            &mut |event| events.push(event),
        )
        .await;
    assert_eq!(ended, Ok(Ended::Done));
    assert_eq!(coder.started.lock().unwrap().len(), 1);
    assert!(
        matches!(&events[0], Event::Coder { accepted: true, .. }),
        "{events:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_interrupt_stops_receiving_the_reply() {
    let dir = tempfile::tempdir().unwrap();
    let mut options = options(dir.path());
    options.interrupt = Arc::new(|| Box::pin(tokio::time::sleep(Duration::from_millis(100))));
    let client = in_process(Arc::new(Silent), options, Arc::new(NoCoder));
    let (events, _, ended) = drain(client.stream(send(&new_id(), "hello", Start::Settings))).await;
    assert_eq!(ended, Ok(Ended::Failed));
    assert!(matches!(
        events.last(),
        Some(Event::ReplyFailed { stopped: true, message, .. }) if message == STOPPED
    ));
}

/// A host that answers every command with one snapshot and keeps who
/// asked.
struct FakeHost {
    callers: Arc<Mutex<Vec<(Caller, Command)>>>,
    snapshot: Snapshot,
}

impl Host for FakeHost {
    fn apply(
        &mut self,
        command: Command,
        caller: Caller,
    ) -> BoxFuture<'_, Result<Snapshot, String>> {
        self.callers.lock().unwrap().push((caller, command));
        let snapshot = self.snapshot.clone();
        Box::pin(async move { Ok(snapshot) })
    }

    fn migrate(&mut self, _: &Path) -> BoxFuture<'_, Migration> {
        Box::pin(async { Migration::Moved(2) })
    }
}

struct FakeDial(Arc<Mutex<Vec<(Caller, Command)>>>, Snapshot);

impl Dial for FakeDial {
    fn socket(&self) -> Option<PathBuf> {
        Some(PathBuf::from("/nowhere/control.sock"))
    }

    fn dial<'a>(&'a self, _: &'a Path) -> BoxFuture<'a, Option<Box<dyn Host>>> {
        let host = FakeHost {
            callers: self.0.clone(),
            snapshot: self.1.clone(),
        };
        Box::pin(async move { Some(Box::new(host) as Box<dyn Host>) })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_host_hears_the_callers_surface_and_its_coder_events_stream() {
    let dir = tempfile::tempdir().unwrap();
    let callers = Arc::new(Mutex::new(Vec::new()));
    let thread = new_id();
    let snapshot = Snapshot {
        coder: Some(Spawned {
            host: LOCAL_HOST.into(),
            task: "t9".into(),
            project: None,
            at: None,
        }),
        ..Snapshot::default()
    };
    let mut options = options(dir.path());
    options.place = Place::Auto { socket: None };
    let dial = FakeDial(callers.clone(), snapshot);
    let mut opened = Vec::new();
    let client = Client::open(
        options,
        &dial,
        Arc::new(FakeCoder::default()),
        None,
        false,
        &mut |event| opened.push(event),
    )
    .await
    .unwrap();
    assert_eq!(client.kind(), Kind::Host);
    assert_eq!(client.place(), "/nowhere/control.sock");
    // Nothing was kept without a host here, so no migration was asked.
    assert!(opened.is_empty());
    let (events, _, ended) = drain(client.stream(Op::Follow {
        thread: thread.clone(),
    }))
    .await;
    assert_eq!(ended, Ok(Ended::Done));
    let Some(Event::Line(line)) = events.last() else {
        panic!("{events:?}");
    };
    assert_eq!(line.task, "t9");
    assert_eq!(
        callers.lock().unwrap().first().map(|(caller, _)| *caller),
        Some(Caller::TERMINAL)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_thread_without_coder_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let callers = Arc::new(Mutex::new(Vec::new()));
    let host = FakeHost {
        callers: callers.clone(),
        snapshot: Snapshot::default(),
    };
    let mut client = Client::over_host(
        Box::new(host),
        PathBuf::from("/x.sock"),
        options(dir.path()),
        Arc::new(FakeCoder::default()),
    );
    let mut events = Vec::new();
    let ended = client
        .run(Op::Stop { thread: new_id() }, &mut |event| {
            events.push(event)
        })
        .await;
    assert_eq!(ended, Ok(Ended::Failed));
    assert!(matches!(
        &events[..],
        [Event::Failure { message, .. }] if message == "This thread has not started Coder."
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_named_socket_must_answer_and_scratch_needs_a_thread() {
    let dir = tempfile::tempdir().unwrap();
    let mut named = options(dir.path());
    named.place = Place::Auto {
        socket: Some(dir.path().join("none.sock")),
    };
    let refused = Client::open(named, &NoHost, Arc::new(NoCoder), None, false, &mut |_| {}).await;
    assert!(
        matches!(refused, Err(Error::Failed(message)) if message.starts_with("no host answers at"))
    );
    let mut scratch = options(dir.path());
    scratch.place = Place::Scratch;
    let refused = Client::open(
        scratch,
        &NoHost,
        Arc::new(NoCoder),
        None,
        false,
        &mut |_| {},
    )
    .await;
    assert_eq!(
        refused.err(),
        Some(Error::Usage("--scratch needs a thread".into()))
    );
}

/// The CLI's and the terminal's run starts on the shared handoff (#10084):
/// the engine is told the person's request for Claude Code is done and its
/// job is the task, and the run asks for Claude Code.
#[test]
fn the_cli_handoff_tells_the_engine_the_routing_is_done() {
    let turns = vec![
        Turn::user("do a test delegation to claude"),
        Turn::assistant(
            "Starting Claude Code on this.",
            Some(Meta {
                offers: vec![Offer::RunCoder],
                engine: Some(nostr::cj_conversation::Engine::ClaudeCode),
                ..Meta::default()
            }),
        ),
    ];
    let (prompt, requested) = handoff("Chat", &turns);
    assert_eq!(requested, Some(nostr::cj_conversation::Engine::ClaudeCode));
    assert!(
        prompt.starts_with("do a test delegation to claude\n\n"),
        "{prompt}"
    );
    for needle in [
        "The person asked for this to run on Claude Code.",
        "never start another coding engine's command line",
        "a small, harmless check of this project",
    ] {
        assert!(prompt.contains(needle), "{needle:?} missing from {prompt}");
    }
}

#[test]
fn thread_ids_are_the_services_ids() {
    assert!(thread_id(&new_id()));
    assert!(!thread_id(&"A".repeat(32)));
    assert!(!thread_id("abc"));
}

#[test]
fn a_device_key_is_created_once_private_and_never_printed() {
    let dir = tempfile::tempdir().unwrap();
    assert!(device_key(dir.path(), false).is_err());
    let first = device_key(dir.path(), true).unwrap();
    let again = device_key(dir.path(), false).unwrap();
    assert_eq!(first, again);
    assert!(identity(dir.path()).is_some_and(|key| key.len() == 64));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(dir.path().join("device.key"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}

/// A worker the relay cannot reach for its first `misses` asks, then one
/// that answers.
struct Unreachable {
    misses: Mutex<u32>,
}

impl Door for Unreachable {
    fn ask(
        &self,
        turns: Vec<Turn>,
        _: Context,
        reply: Arc<std::sync::Mutex<Reply>>,
    ) -> BoxFuture<'static, ()> {
        let miss = {
            let mut misses = self.misses.lock().unwrap();
            let miss = *misses > 0;
            *misses = misses.saturating_sub(1);
            miss
        };
        Box::pin(async move {
            let mut reply = lock(&reply);
            if miss {
                reply.failure = Some(basic_coder::Failure::Transport(
                    "the relay could not be reached".into(),
                ));
            } else {
                reply.text = format!("You said: {}", turns.last().unwrap().text);
                reply.done = true;
            }
        })
    }
}

/// When the relay cannot be reached, the send says so, waits a pause that
/// doubles, asks again, and the reply streams in once it can (#10151).
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn an_unreachable_relay_is_said_and_retried_until_the_reply_comes() {
    let dir = tempfile::tempdir().unwrap();
    let door = Arc::new(Unreachable {
        misses: Mutex::new(2),
    });
    let client = in_process(door, options(dir.path()), Arc::new(NoCoder));
    let thread = new_id();
    let (events, _, ended) = drain(client.stream(send(&thread, "hello", Start::Settings))).await;
    assert_eq!(ended, Ok(Ended::Done));
    let names: Vec<String> = events
        .iter()
        .filter_map(|event| match event {
            Event::Offline { retry_in, .. } => Some(format!("offline {retry_in}")),
            Event::Online { .. } => Some("online".into()),
            Event::Reply { reply, .. } => Some(reply.text.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        names,
        ["offline 3", "online", "You said: hello"],
        "{events:?}"
    );
}

/// A blip shorter than `QUIET_FOR`, as a host restarting for an update,
/// is not said: no offline line, no "connected again" (2026-10-02).
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_short_blip_is_not_said() {
    let dir = tempfile::tempdir().unwrap();
    let took = Arc::new(Mutex::new(Vec::new()));
    let host = Restarting {
        misses: 1,
        unanswered: false,
        took: took.clone(),
    };
    let client = Client::over_host(
        Box::new(host),
        PathBuf::from("/nowhere/control.sock"),
        options(dir.path()),
        Arc::new(NoCoder),
    );
    let thread = new_id();
    let mut op = send(&thread, "hello", Start::Settings);
    if let Op::Send { new, .. } = &mut op {
        *new = false;
    }
    let (events, _, _) = drain(client.stream(op)).await;
    assert!(
        events
            .iter()
            .all(|event| !matches!(event, Event::Offline { .. } | Event::Online { .. })),
        "{events:?}"
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::Accepted { .. }))
    );
    assert_eq!(took.lock().unwrap().len(), 1);

    // Mid-reply too: one unanswered read, then the reply streams on.
    let reads = vec![
        Some(Snapshot {
            busy: true,
            partial: "Wor".into(),
            ..Snapshot::default()
        }),
        None,
        Some(Snapshot::default()),
    ];
    let client = Client::over_host(
        Box::new(Flaky {
            reads: reads.into(),
        }),
        PathBuf::from("/nowhere/control.sock"),
        options(dir.path()),
        Arc::new(NoCoder),
    );
    let mut op = send(&thread, "hello", Start::Settings);
    if let Op::Send { new, .. } = &mut op {
        *new = false;
    }
    let (events, _, _) = drain(client.stream(op)).await;
    assert!(
        events
            .iter()
            .all(|event| !matches!(event, Event::Offline { .. } | Event::Online { .. })),
        "{events:?}"
    );
}

/// Esc while it waits stops waiting: the send ends as stopped.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn stopping_while_offline_ends_the_send() {
    let dir = tempfile::tempdir().unwrap();
    let door = Arc::new(Unreachable {
        misses: Mutex::new(u32::MAX),
    });
    let fired = Arc::new(tokio::sync::Notify::new());
    let mut options = options(dir.path());
    let waiter = fired.clone();
    options.interrupt = Arc::new(move || {
        let waiter = waiter.clone();
        Box::pin(async move { waiter.notified().await })
    });
    let client = in_process(door, options, Arc::new(NoCoder));
    let thread = new_id();
    let Stream { mut events, done } = client.stream(send(&thread, "hello", Start::Settings));
    let mut seen = Vec::new();
    while let Some(event) = events.recv().await {
        let offline = matches!(event, Event::Offline { .. });
        seen.push(event);
        if offline {
            fired.notify_waiters();
        }
    }
    let (_, ended) = done.await.unwrap();
    assert_eq!(ended, Ok(Ended::Failed));
    assert!(
        matches!(seen.last(), Some(Event::ReplyFailed { stopped: true, .. })),
        "{seen:?}"
    );
}

#[test]
fn the_pause_doubles_to_half_a_minute() {
    let pauses: Vec<u64> = (1..=6).map(|attempt| backoff(attempt).as_secs()).collect();
    assert_eq!(pauses, [2, 4, 8, 16, 30, 30]);
    assert_eq!(backoff(u32::MAX).as_secs(), 30);
}

/// A host that answers from a script of reads: `None` is a read it did not
/// answer, as while it restarts.
struct Flaky {
    reads: std::collections::VecDeque<Option<Snapshot>>,
}

impl Host for Flaky {
    fn apply(&mut self, command: Command, _: Caller) -> BoxFuture<'_, Result<Snapshot, String>> {
        let answer = match command {
            Command::Read { .. } => self
                .reads
                .pop_front()
                .flatten()
                .ok_or_else(|| "the host did not answer".to_owned()),
            _ => Ok(Snapshot {
                busy: true,
                ..Snapshot::default()
            }),
        };
        Box::pin(async move { answer })
    }

    fn migrate(&mut self, _: &Path) -> BoxFuture<'_, Migration> {
        Box::pin(async { Migration::Quiet })
    }
}

/// The host stops answering while a reply streams: the client says so,
/// reads again once it is back, and the reply streams on from there.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_host_that_stops_answering_mid_reply_is_read_again() {
    let dir = tempfile::tempdir().unwrap();
    let streaming = |partial: &str| {
        Some(Snapshot {
            busy: true,
            partial: partial.into(),
            ..Snapshot::default()
        })
    };
    let reads = vec![
        streaming("Wor"),
        None,
        None,
        streaming("Working on it"),
        Some(Snapshot::default()),
    ];
    let host = Flaky {
        reads: reads.into(),
    };
    let client = Client::over_host(
        Box::new(host),
        PathBuf::from("/nowhere/control.sock"),
        options(dir.path()),
        Arc::new(NoCoder),
    );
    let thread = new_id();
    let mut op = send(&thread, "hello", Start::Settings);
    if let Op::Send { new, .. } = &mut op {
        *new = false;
    }
    let (events, _, _) = drain(client.stream(op)).await;
    let names: Vec<String> = events
        .iter()
        .filter_map(|event| match event {
            Event::Offline { retry_in, .. } => Some(format!("offline {retry_in}")),
            Event::Online { .. } => Some("online".into()),
            Event::Partial { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        names,
        ["Wor", "offline 3", "online", "Working on it"],
        "{events:?}"
    );
}

/// A host whose first sends go unanswered, as while it restarts, then
/// answers; it counts the sends it took.
struct Restarting {
    misses: u32,
    unanswered: bool,
    took: Arc<Mutex<Vec<String>>>,
}

impl Host for Restarting {
    fn apply(&mut self, command: Command, _: Caller) -> BoxFuture<'_, Result<Snapshot, String>> {
        self.unanswered = false;
        let answer = match command {
            Command::Send { .. } if self.misses > 0 => {
                self.misses -= 1;
                self.unanswered = true;
                Err("OpenAgents on this computer is not answering.".to_owned())
            }
            Command::Send { request, .. } => {
                self.took.lock().unwrap().push(request);
                Ok(Snapshot {
                    busy: true,
                    ..Snapshot::default()
                })
            }
            _ => Ok(Snapshot::default()),
        };
        Box::pin(async move { answer })
    }

    fn migrate(&mut self, _: &Path) -> BoxFuture<'_, Migration> {
        Box::pin(async { Migration::Quiet })
    }

    fn unanswered(&self) -> bool {
        self.unanswered
    }
}

/// The owner's follow-up hit a host that had restarted (2026-10-02): the
/// send failed "the host did not answer". Now a send the host does not
/// answer is sent again once it does, under the same send ID, and the
/// screen says it is waiting.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_send_to_a_restarting_host_goes_through_once_it_answers() {
    let dir = tempfile::tempdir().unwrap();
    let took = Arc::new(Mutex::new(Vec::new()));
    let host = Restarting {
        misses: 2,
        unanswered: false,
        took: took.clone(),
    };
    let client = Client::over_host(
        Box::new(host),
        PathBuf::from("/nowhere/control.sock"),
        options(dir.path()),
        Arc::new(NoCoder),
    );
    let thread = new_id();
    let mut op = send(&thread, "ok, now open issues for them", Start::Settings);
    if let Op::Send { new, .. } = &mut op {
        *new = false;
    }
    let (events, _, _) = drain(client.stream(op)).await;
    let names: Vec<String> = events
        .iter()
        .filter_map(|event| match event {
            Event::Offline { retry_in, .. } => Some(format!("offline {retry_in}")),
            Event::Online { .. } => Some("online".into()),
            Event::Accepted { .. } => Some("accepted".into()),
            Event::Failure { message, .. } => Some(format!("failure {message}")),
            _ => None,
        })
        .collect();
    assert_eq!(names, ["offline 3", "online", "accepted"], "{events:?}");
    assert_eq!(took.lock().unwrap().len(), 1);
}

/// A refusal in the host's own words is not waited out.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_refused_send_is_not_sent_again() {
    let dir = tempfile::tempdir().unwrap();
    struct Refusing;
    impl Host for Refusing {
        fn apply(&mut self, _: Command, _: Caller) -> BoxFuture<'_, Result<Snapshot, String>> {
            Box::pin(async { Err("Chat not found.".to_owned()) })
        }
        fn migrate(&mut self, _: &Path) -> BoxFuture<'_, Migration> {
            Box::pin(async { Migration::Quiet })
        }
    }
    let client = Client::over_host(
        Box::new(Refusing),
        PathBuf::from("/nowhere/control.sock"),
        options(dir.path()),
        Arc::new(NoCoder),
    );
    let thread = new_id();
    let mut op = send(&thread, "hello", Start::Settings);
    if let Op::Send { new, .. } = &mut op {
        *new = false;
    }
    let (events, _, ended) = drain(client.stream(op)).await;
    assert_eq!(ended, Ok(Ended::Refused));
    assert!(
        events
            .iter()
            .all(|event| !matches!(event, Event::Offline { .. })),
        "{events:?}"
    );
}

/// A stand-in for the chat router's Jev reading: it reads each message as
/// more work (`work.dispatch`, a Coder offer) or not (`general`), from a
/// script, and keeps whether each turn told it about the thread's run.
struct StandInJev {
    work: Vec<&'static str>,
    saw_run: Arc<Mutex<Vec<(String, bool)>>>,
}

impl Door for StandInJev {
    fn ask(
        &self,
        turns: Vec<Turn>,
        context: Context,
        reply: Arc<std::sync::Mutex<Reply>>,
    ) -> BoxFuture<'static, ()> {
        let asked = turns.last().unwrap().text.clone();
        self.saw_run
            .lock()
            .unwrap()
            .push((asked.clone(), context.coder_run.is_some()));
        let work = self.work.contains(&asked.as_str());
        Box::pin(async move {
            let mut reply = lock(&reply);
            reply.text = if work {
                "Working on that.".into()
            } else {
                "The review found two issues.".into()
            };
            if work {
                reply.meta = Meta {
                    offers: vec![Offer::RunCoder],
                    ..Meta::default()
                };
            }
            reply.done = true;
        })
    }
}

/// The owner's thread (2026-10-02): Codex reviewed the latest commits and
/// its turn ended; the follow-up asked for more work on it. The router,
/// told about the run, reads it as more work, and the same task (the same
/// engine session, with its earlier turns) takes it as its next turn: no
/// new run, and not the chat model. A question about the run the router
/// reads as `general` is answered in chat and leaves the run alone.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_follow_up_the_router_reads_as_more_work_continues_the_same_session() {
    const FOLLOW_UP: &str = "ok, create issues for them and delegate each to a subagent in \
                             separeate worktree and merge to main when done";
    const QUESTION: &str = "which of the two is worse?";
    let dir = tempfile::tempdir().unwrap();
    let saw_run = Arc::new(Mutex::new(Vec::new()));
    let door = Arc::new(StandInJev {
        work: vec!["review the latest commits in this repo", FOLLOW_UP],
        saw_run: saw_run.clone(),
    });
    let coder = Arc::new(FakeCoder {
        ended: true,
        ..FakeCoder::default()
    });
    let client = in_process(door, options(dir.path()), coder.clone());
    let thread = new_id();
    let (_, client, _) = drain(client.stream(send(
        &thread,
        "review the latest commits in this repo",
        Start::Settings,
    )))
    .await;
    let again = |text: &str| {
        let mut op = send(&thread, text, Start::Settings);
        if let Op::Send { new, .. } = &mut op {
            *new = false;
        }
        op
    };
    let (asked, client, _) = drain(client.stream(again(QUESTION))).await;
    assert!(
        asked
            .iter()
            .all(|event| !matches!(event, Event::Coder { .. })),
        "{asked:?}"
    );
    assert!(coder.answered.lock().unwrap().is_empty());
    let (events, _, _) = drain(client.stream(again(FOLLOW_UP))).await;
    assert!(
        events.iter().any(
            |event| matches!(event, Event::Coder { accepted: true, message, .. }
            if message == "Coder continues task t1 with your message.")
        ),
        "{events:?}"
    );
    assert_eq!(*coder.answered.lock().unwrap(), [FOLLOW_UP]);
    assert_eq!(
        coder.started.lock().unwrap().len(),
        1,
        "one task, never two"
    );
    // Every follow-up told the router about the run that ended.
    let saw_run = saw_run.lock().unwrap().clone();
    assert_eq!(
        saw_run,
        [
            ("review the latest commits in this repo".to_owned(), false),
            (QUESTION.to_owned(), true),
            (FOLLOW_UP.to_owned(), true),
        ]
    );
}

/// A worker whose reply proposes an `openagents` command, as its `cli`
/// offer feedback carries it (#10170).
struct Proposes(Vec<&'static str>);

impl Door for Proposes {
    fn ask(
        &self,
        _: Vec<Turn>,
        _: Context,
        reply: Arc<std::sync::Mutex<Reply>>,
    ) -> BoxFuture<'static, ()> {
        let argv = self.0.clone();
        Box::pin(async move {
            let mut reply = lock(&reply);
            reply.text = "Running the openagents command for that on this computer.".into();
            let mut meta = Meta {
                route: Some("wallet".into()),
                tier: Some("cli".into()),
                ..Meta::default()
            };
            meta.offered(&serde_json::json!({
                "offer": "cli", "argv": argv, "effect": "read_only",
                "runs_on": "this_device", "confirm": true,
            }));
            reply.meta = meta;
            reply.done = true;
        })
    }
}

/// A stand-in for this computer's command tree and runner: `wallet
/// status` reads, `wallet init` changes something, `wallet send` moves
/// money; it records what it ran.
#[derive(Default)]
struct Commands {
    ran: Mutex<Vec<Vec<String>>>,
}

impl Coder for Commands {
    fn default_store(&self) -> PathBuf {
        std::env::temp_dir().join("openagents-chat-client-test-tasks")
    }
    fn context(&self, _: &Path, _: Option<&Path>) -> Context {
        Context::default()
    }
    fn predict(&self, _: &Path, _: Option<nostr::cj_conversation::Engine>) -> Option<Runner> {
        None
    }
    fn asks_first(&self) -> bool {
        false
    }
    fn checkout(&self, _: &Path) -> Result<(), String> {
        Err("no".into())
    }
    fn start(
        &self,
        _: &Path,
        _: &Path,
        _: &str,
        _: &str,
        _: &str,
        _: Option<nostr::cj_conversation::Engine>,
    ) -> Result<Started, String> {
        Err("no".into())
    }
    fn issue(&self, _: &str, _: &str, _: &Path) -> Option<Box<dyn Issue>> {
        None
    }
    fn follow(&self, s: &Path, t: &str, c: &str, h: Option<String>) -> Box<dyn Follow> {
        NoCoder.follow(s, t, c, h)
    }
    fn stop(&self, _: &Path, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn answer(&self, _: &Path, _: &str, _: &str) -> Result<usize, String> {
        Err("no".into())
    }
    fn result(&self, _: &Path, _: &str) -> Option<CoderRun> {
        None
    }
    fn trajectories(&self, _: &Path, _: &str) -> Vec<Value> {
        Vec::new()
    }
    fn permit(&self, argv: &[String]) -> Permit {
        match argv.join(" ").as_str() {
            "wallet status" => Permit::Now,
            "wallet init" => Permit::Confirm,
            _ => Permit::Never,
        }
    }
    fn run_command(&self, argv: &[String]) -> Result<Ran, String> {
        self.ran.lock().unwrap().push(argv.to_vec());
        Ok(Ran {
            ok: true,
            output: "balance: 2100 sats\naddress: bc1qexample".into(),
        })
    }
}

/// #10170: "balance and wallet address" in a terminal. The reply's
/// read-only command runs here at once, with no confirm and no Coder run,
/// and its output is what the person sees.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_read_only_command_a_reply_proposes_runs_at_once_and_shows_its_output() {
    let dir = tempfile::tempdir().unwrap();
    let coder = Arc::new(Commands::default());
    let door = Arc::new(Proposes(vec!["wallet", "status"]));
    let client = in_process(door, options(dir.path()), coder.clone());
    let thread = new_id();
    let (events, _, ended) = drain(client.stream(send(
        &thread,
        "balance and wallet address basic readonly identifying shit",
        Start::Settings,
    )))
    .await;
    assert_eq!(ended, Ok(Ended::Done));
    let words = |argv: &[String]| argv.join(" ");
    assert!(
        events.iter().any(|event| matches!(event,
            Event::Command { argv, confirm: false, .. } if words(argv) == "wallet status")),
        "{events:?}"
    );
    let Some(Event::Ran {
        ok: true, output, ..
    }) = events.last()
    else {
        panic!("{events:?}");
    };
    assert!(output.contains("2100 sats"), "{output}");
    assert_eq!(coder.ran.lock().unwrap().len(), 1);
    // No Coder offer, and no Coder run.
    assert!(
        events.iter().all(|event| !matches!(
            event,
            Event::Coder { .. } | Event::Reply { running: true, .. }
        )),
        "{events:?}"
    );
}

/// A command that changes something here waits for the confirm, and the
/// confirm runs exactly it; one that moves money never runs.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_command_that_changes_something_waits_for_the_confirm_and_money_never_runs() {
    let dir = tempfile::tempdir().unwrap();
    let coder = Arc::new(Commands::default());
    let door = Arc::new(Proposes(vec!["wallet", "init"]));
    let client = in_process(door, options(dir.path()), coder.clone());
    let thread = new_id();
    let (events, client, ended) =
        drain(client.stream(send(&thread, "set up my wallet", Start::Settings))).await;
    assert_eq!(ended, Ok(Ended::Done));
    assert!(
        matches!(events.last(), Some(Event::Command { confirm: true, .. })),
        "{events:?}"
    );
    assert!(coder.ran.lock().unwrap().is_empty());
    let (events, _, ended) = drain(client.stream(Op::RunCommand {
        thread: thread.clone(),
    }))
    .await;
    assert_eq!(ended, Ok(Ended::Done));
    assert!(
        matches!(events.last(), Some(Event::Ran { ok: true, .. })),
        "{events:?}"
    );
    assert_eq!(
        coder.ran.lock().unwrap().as_slice(),
        [vec!["wallet".to_owned(), "init".to_owned()]]
    );

    let coder = Arc::new(Commands::default());
    let door = Arc::new(Proposes(vec!["wallet", "send", "bc1q", "--sats", "1000"]));
    let client = in_process(door, options(dir.path()), coder.clone());
    let thread = new_id();
    let (events, client, _) =
        drain(client.stream(send(&thread, "send 1000 sats", Start::Settings))).await;
    assert!(
        events
            .iter()
            .all(|event| !matches!(event, Event::Command { .. } | Event::Ran { .. })),
        "{events:?}"
    );
    let (_, _, ended) = drain(client.stream(Op::RunCommand { thread })).await;
    assert_eq!(ended, Ok(Ended::Failed));
    assert!(coder.ran.lock().unwrap().is_empty());
}

/// A worker that plans three read-only runs with a summary, then answers a
/// request carrying their results with a combined summary (#10183).
struct Planner {
    contexts: Arc<Mutex<Vec<Context>>>,
}

impl Door for Planner {
    fn ask(
        &self,
        _: Vec<Turn>,
        context: Context,
        reply: Arc<std::sync::Mutex<Reply>>,
    ) -> BoxFuture<'static, ()> {
        self.contexts.lock().unwrap().push(context.clone());
        Box::pin(async move {
            let mut reply = lock(&reply);
            if context.runs.is_empty() {
                use nostr::cj_conversation::{Engine, Plan};
                reply.text = "Exploring the repo with Codex, Claude Code, and Grok Build.".into();
                reply.meta = Meta {
                    offers: vec![Offer::RunCoder],
                    route: Some("work.dispatch".into()),
                    plan: Some(Plan {
                        runs: vec![Engine::Codex, Engine::ClaudeCode, Engine::GrokBuild],
                        read_only: true,
                        summarize: true,
                    }),
                    ..Meta::default()
                };
            } else {
                reply.text = format!("All {} runs agree: a Rust workspace.", context.runs.len());
            }
            reply.done = true;
        })
    }
}

/// Coder that starts each plan run after a pause, recording how many
/// started at once, and finishes each with a result naming its engine.
#[derive(Default)]
struct FanCoder {
    runs: Mutex<Vec<(nostr::cj_conversation::Engine, bool, String)>>,
    starting: std::sync::atomic::AtomicUsize,
    most: std::sync::atomic::AtomicUsize,
}

impl Coder for FanCoder {
    fn default_store(&self) -> PathBuf {
        std::env::temp_dir().join("openagents-chat-fan-test-tasks")
    }
    fn context(&self, _: &Path, _: Option<&Path>) -> Context {
        Context {
            computer_ready: true,
            ..Context::default()
        }
    }
    fn predict(&self, _: &Path, _: Option<nostr::cj_conversation::Engine>) -> Option<Runner> {
        None
    }
    fn asks_first(&self) -> bool {
        false
    }
    fn checkout(&self, _: &Path) -> Result<(), String> {
        Ok(())
    }
    fn start(
        &self,
        _: &Path,
        _: &Path,
        _: &str,
        _: &str,
        _: &str,
        _: Option<nostr::cj_conversation::Engine>,
    ) -> Result<Started, String> {
        panic!("a plan never starts one plain run");
    }
    fn start_run(
        &self,
        _: &Path,
        _: &Path,
        _: &str,
        prompt: &str,
        _: &str,
        engine: nostr::cj_conversation::Engine,
        read_only: bool,
    ) -> Result<Started, String> {
        use std::sync::atomic::Ordering;
        let now = self.starting.fetch_add(1, Ordering::SeqCst) + 1;
        self.most.fetch_max(now, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(300));
        self.starting.fetch_sub(1, Ordering::SeqCst);
        self.runs
            .lock()
            .unwrap()
            .push((engine, read_only, prompt.to_owned()));
        Ok(Started {
            task: format!("task-{}", agent_word(engine)),
            project: "demo".into(),
            worktree: format!("/tmp/demo-{}", agent_word(engine)),
        })
    }
    fn issue(&self, _: &str, _: &str, _: &Path) -> Option<Box<dyn Issue>> {
        None
    }
    fn follow(&self, _: &Path, task: &str, chat: &str, _: Option<String>) -> Box<dyn Follow> {
        struct Done(Option<Line>);
        impl Follow for Done {
            fn poll(&mut self) -> Result<(Vec<Line>, Progress), String> {
                Ok((self.0.take().into_iter().collect(), Progress::Ended))
            }
        }
        Box::new(Done(Some(Line {
            seq: 1,
            task: task.to_owned(),
            thread: Some(chat.to_owned()),
            event: CoderEvent::Result(coder_events::Finished {
                turn: 1,
                summary: format!("{task} found a Rust workspace."),
                files_changed: Vec::new(),
                insertions: 0,
                deletions: 0,
                worktree: "/tmp".into(),
                trajectory: String::new(),
                issue: None,
                cost_microusd: None,
            }),
        })))
    }
    fn stop(&self, _: &Path, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn answer(&self, _: &Path, _: &str, _: &str) -> Result<usize, String> {
        panic!("a plan's runs are new tasks");
    }
    fn result(&self, _: &Path, task: &str) -> Option<CoderRun> {
        Some(CoderRun {
            ending: crate::router::RunEnding::Finished,
            turn: 1,
            engine: task.strip_prefix("task-").map(str::to_owned),
            model: None,
            summary: format!("{task} found a Rust workspace."),
            files: Vec::new(),
            commands: Vec::new(),
        })
    }
    fn trajectories(&self, _: &Path, _: &str) -> Vec<Value> {
        Vec::new()
    }
}

/// #10183: a reply that plans three read-only runs starts them in
/// parallel, one per engine, each read-only and told it is one of three;
/// says plainly what started; streams every run's events; and, once all
/// end, puts each result in the thread and asks the chat for one combined
/// summary of them.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_plan_starts_one_read_only_run_per_engine_and_summarizes_them() {
    use nostr::cj_conversation::Engine;
    let dir = tempfile::tempdir().unwrap();
    let contexts = Arc::new(Mutex::new(Vec::new()));
    let door = Arc::new(Planner {
        contexts: contexts.clone(),
    });
    let coder = Arc::new(FanCoder::default());
    let client = in_process(door, options(dir.path()), coder.clone());
    let thread = new_id();
    let ask = "do 3 readonly delegations, 1 per agent, explore repo and summarize briefly";
    let (events, mut client, ended) =
        drain(client.stream(send(&thread, ask, Start::Settings))).await;
    assert_eq!(ended, Ok(Ended::Done), "{events:?}");
    let runs = coder.runs.lock().unwrap().clone();
    let mut engines: Vec<Engine> = runs.iter().map(|(engine, _, _)| *engine).collect();
    engines.sort_by_key(|engine| engine.word());
    assert_eq!(
        engines,
        [Engine::ClaudeCode, Engine::Codex, Engine::GrokBuild]
    );
    assert!(runs.iter().all(|(_, read_only, _)| *read_only));
    for (engine, _, prompt) in &runs {
        assert!(prompt.starts_with(ask), "{prompt}");
        assert!(
            prompt.contains(&format!("this run is the one on {}", engine.name())),
            "{prompt}"
        );
        assert!(prompt.contains("This run is read-only"), "{prompt}");
        assert!(
            prompt.contains("never say you could not delegate"),
            "{prompt}"
        );
    }
    // The three started at once, not one after another.
    assert!(
        coder.most.load(std::sync::atomic::Ordering::SeqCst) >= 2,
        "the runs started one at a time"
    );
    let said: Vec<&str> = events
        .iter()
        .filter_map(|event| match event {
            Event::Coder {
                accepted: true,
                quiet: false,
                message,
                ..
            } => Some(message.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        said,
        ["Running Codex, Claude Code, and Grok Build, read-only."]
    );
    let lines: Vec<&str> = events
        .iter()
        .filter_map(|event| match event {
            Event::Line(line) => Some(line.task.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(lines.len(), 3, "{events:?}");
    let Some(Event::Reply { reply, .. }) = events.last() else {
        panic!("{events:?}");
    };
    assert_eq!(reply.text, "All 3 runs agree: a Rust workspace.");
    // The summary request carried the three results, each with its engine.
    let sent = contexts.lock().unwrap().clone();
    assert_eq!(sent.len(), 2);
    let engines: Vec<&str> = sent[1]
        .runs
        .iter()
        .filter_map(|run| run["engine"].as_str())
        .collect();
    assert_eq!(engines, ["codex", "claude", "grok"]);
    // The thread holds the request, the reply, each result, and the summary.
    let thread = client.collect(&thread).await.unwrap();
    let texts: Vec<&str> = thread.turns.iter().map(|turn| turn.text.as_str()).collect();
    assert_eq!(texts.len(), 6, "{texts:?}");
    assert_eq!(texts[2], "**Codex**: task-codex found a Rust workspace.");
    assert_eq!(texts[5], "All 3 runs agree: a Rust workspace.");
    assert_eq!(
        thread
            .summary
            .coder
            .as_ref()
            .map(|coder| coder.task.as_str()),
        Some("task-codex")
    );
}

/// A worker serving one plugin step per turn, in order (#10177): the
/// reply's typed `plugin` field, and a Run Coder offer at the draft.
struct PluginSteps(Mutex<Vec<crate::plugin_flow::Flow>>);

impl Door for PluginSteps {
    fn ask(
        &self,
        _: Vec<Turn>,
        _: Context,
        reply: Arc<std::sync::Mutex<Reply>>,
    ) -> BoxFuture<'static, ()> {
        let flow = self.0.lock().unwrap().remove(0);
        Box::pin(async move {
            let mut reply = lock(&reply);
            let step = flow.step().unwrap();
            reply.text = step.line().to_owned();
            let mut meta = Meta {
                route: Some("eval.author".into()),
                tier: Some("author".into()),
                ..Meta::default()
            };
            if step == crate::plugin_flow::Step::Draft {
                meta.offered(&serde_json::json!({"offer": "run_coder"}));
            }
            meta.resulted(&serde_json::json!({"plugin": flow.wire()}));
            reply.meta = meta;
            reply.done = true;
        })
    }
}

/// Coder that drafts the plugin `hello` with two tests, and records every
/// prompt and plugin command.
#[derive(Default)]
struct Drafts {
    prompts: Mutex<Vec<String>>,
    ran: Mutex<Vec<Vec<String>>>,
}

impl Coder for Drafts {
    fn default_store(&self) -> PathBuf {
        std::env::temp_dir().join("openagents-chat-client-test-tasks")
    }
    fn context(&self, _: &Path, _: Option<&Path>) -> Context {
        Context {
            computer_ready: true,
            ..Context::default()
        }
    }
    fn predict(&self, _: &Path, _: Option<nostr::cj_conversation::Engine>) -> Option<Runner> {
        None
    }
    fn asks_first(&self) -> bool {
        false
    }
    fn checkout(&self, _: &Path) -> Result<(), String> {
        Ok(())
    }
    fn start(
        &self,
        _: &Path,
        _: &Path,
        _: &str,
        prompt: &str,
        _: &str,
        _: Option<nostr::cj_conversation::Engine>,
    ) -> Result<Started, String> {
        self.prompts.lock().unwrap().push(prompt.to_owned());
        Ok(Started {
            task: "t1".into(),
            project: "demo".into(),
            worktree: "/tmp/demo-t1".into(),
        })
    }
    fn issue(&self, _: &str, _: &str, _: &Path) -> Option<Box<dyn Issue>> {
        None
    }
    fn follow(&self, _: &Path, task: &str, chat: &str, _: Option<String>) -> Box<dyn Follow> {
        struct Once(Option<Line>);
        impl Follow for Once {
            fn poll(&mut self) -> Result<(Vec<Line>, Progress), String> {
                Ok((self.0.take().into_iter().collect(), Progress::Waiting))
            }
        }
        Box::new(Once(Some(Line {
            seq: 1,
            task: task.to_owned(),
            thread: Some(chat.to_owned()),
            event: CoderEvent::Question(Asked {
                turn: 1,
                text: "Anything else?".into(),
                answer: None,
            }),
        })))
    }
    fn stop(&self, _: &Path, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn answer(&self, _: &Path, _: &str, _: &str) -> Result<usize, String> {
        Ok(2)
    }
    fn result(&self, _: &Path, _: &str) -> Option<CoderRun> {
        let file = |path: &str| crate::router::RunFile {
            path: path.into(),
            status: "added".into(),
        };
        Some(CoderRun {
            ending: crate::router::RunEnding::Finished,
            turn: 1,
            engine: Some("codex".into()),
            model: None,
            summary: "Drafted plugins/hello.".into(),
            files: vec![
                file("plugins/hello/package.json"),
                file("plugins/hello/skills/hello.md"),
                file("plugins/hello/evals/greets-by-name/prompt.md"),
                file("plugins/hello/evals/stays-out/prompt.md"),
            ],
            commands: Vec::new(),
        })
    }
    fn trajectories(&self, _: &Path, _: &str) -> Vec<Value> {
        Vec::new()
    }
    fn permit(&self, _: &[String]) -> Permit {
        // This build has no `plugin publish` yet.
        Permit::Never
    }
    fn worktree(&self, _: &Path, _: &str) -> Option<PathBuf> {
        Some(PathBuf::from("/tmp/demo-t1"))
    }
    fn plugin_tests(&self, dir: &Path) -> Result<Vec<crate::plugin_flow::Test>, String> {
        assert_eq!(dir, Path::new("/tmp/demo-t1/plugins/hello"));
        Ok(vec![
            crate::plugin_flow::Test {
                name: "greets-by-name".into(),
                kind: "should-fire".into(),
                task: "Say hello to Ada.".into(),
            },
            crate::plugin_flow::Test {
                name: "stays-out".into(),
                kind: "should-not-fire".into(),
                task: "What is 2 + 2?".into(),
            },
        ])
    }
    fn plugin_command(&self, argv: &[String], _: Duration) -> Result<Ran, String> {
        self.ran.lock().unwrap().push(argv.to_vec());
        // A test run that finishes writes its report where it was told.
        if let Some(at) = argv.iter().position(|word| word == "--output-dir") {
            let dir = Path::new(&argv[at + 1]).join("run-1");
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("report.json"), "{}").unwrap();
        }
        Ok(Ran {
            ok: true,
            output: format!("ran {}", argv[1]),
        })
    }
}

/// #10177: making a plugin from a terminal. The draft reply starts Coder
/// with the plugin brief and, once its run ended, this computer shows the
/// tests it drafted (step `tests`); an approved run reply runs them here
/// and asks the publish question (step `publish`); the done reply turns it
/// on here, and says publishing waits for a build that can.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_plugin_is_drafted_tested_and_turned_on_through_typed_steps() {
    use crate::plugin_flow::{Flow, Step};
    let dir = tempfile::tempdir().unwrap();
    let coder = Arc::new(Drafts::default());
    let mut done = Flow::at(Step::Done, Some("hello".into()));
    done.publish = true;
    done.enable = true;
    let door = Arc::new(PluginSteps(Mutex::new(vec![
        Flow::at(Step::Draft, None),
        Flow::at(Step::Run, Some("hello".into())),
        done,
    ])));
    let client = in_process(door, options(dir.path()), coder.clone());
    let thread = new_id();
    let plugin_events = |events: &[Event]| -> Vec<(Flow, String)> {
        events
            .iter()
            .filter_map(|event| match event {
                Event::Plugin {
                    flow,
                    text,
                    ok: true,
                    ..
                } => Some((flow.clone(), text.clone())),
                _ => None,
            })
            .collect()
    };

    let (events, client, ended) = drain(client.stream(send(
        &thread,
        "Help me make a plugin that greets people by name",
        Start::Settings,
    )))
    .await;
    assert_eq!(ended, Ok(Ended::Done), "{events:?}");
    let prompts = coder.prompts.lock().unwrap().clone();
    assert!(
        prompts[0].contains(crate::plugin_flow::BRIEF),
        "{prompts:?}"
    );
    let shown = plugin_events(&events);
    let [(flow, text)] = shown.as_slice() else {
        panic!("{events:?}");
    };
    assert_eq!(flow, &Flow::at(Step::Tests, Some("hello".into())));
    assert!(text.contains("greets-by-name") && text.contains("stays out of the way"));
    assert_eq!(Step::from_line(text), Some(Step::Tests));

    let (events, client, ended) = drain(client.stream(Op::Send {
        thread: thread.clone(),
        new: false,
        text: "yes".into(),
        start: Start::Settings,
        timeout: Duration::from_secs(10),
    }))
    .await;
    assert_eq!(ended, Ok(Ended::Done), "{events:?}");
    let shown = plugin_events(&events);
    let [(flow, text)] = shown.as_slice() else {
        panic!("{events:?}");
    };
    assert_eq!(flow.step(), Some(Step::Publish));
    assert_eq!(Step::from_line(text), Some(Step::Publish));
    let first = coder.ran.lock().unwrap()[0].clone();
    assert_eq!(
        first[..5],
        [
            "plugin",
            "test",
            "run",
            "/tmp/demo-t1/plugins/hello",
            "--trust"
        ]
    );
    assert_eq!(first[5], "--output-dir");

    let (events, _, ended) = drain(client.stream(Op::Send {
        thread: thread.clone(),
        new: false,
        text: "yes".into(),
        start: Start::Settings,
        timeout: Duration::from_secs(10),
    }))
    .await;
    assert_eq!(ended, Ok(Ended::Done), "{events:?}");
    let shown = plugin_events(&events);
    let [(flow, text)] = shown.as_slice() else {
        panic!("{events:?}");
    };
    assert_eq!(flow.step(), Some(Step::Done));
    assert!(
        text.contains("can't publish a plugin to the registry yet"),
        "{text}"
    );
    assert!(text.ends_with("It's on on this computer."), "{text}");
    let ran = coder.ran.lock().unwrap().clone();
    assert_eq!(
        ran[1..],
        [
            vec!["plugin", "install", "/tmp/demo-t1/plugins/hello"],
            vec!["plugin", "enable", "hello"],
        ]
        .map(|argv| argv.into_iter().map(str::to_owned).collect::<Vec<_>>())
    );
}
