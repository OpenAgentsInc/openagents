//! The client's event stream against the in-process door and a fake host.

use std::sync::Mutex;

use super::*;
use crate::basic_chats::Spawned;
use crate::basic_coder::{Door, Reply, lock};
use crate::coder_events::Asked;
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
        None
    }
    fn asks_first(&self) -> bool {
        self.asks_first
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
    fn answer(&self, _: &Path, _: &str, _: &str) -> Result<usize, String> {
        Ok(2)
    }
    fn result(&self, _: &Path, _: &str) -> Option<CoderRun> {
        None
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
