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
        None
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
            "We'll dispatch Coder, asking for Claude Code, to take this on.",
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
        ["offline 2", "offline 4", "online", "You said: hello"],
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
        ["Wor", "offline 2", "offline 4", "online", "Working on it"],
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
    assert_eq!(
        names,
        ["offline 2", "offline 4", "online", "accepted"],
        "{events:?}"
    );
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
                "Coder will do that.".into()
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
            reply.text = "We're running the openagents command for that on this computer.".into();
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
