//! OpenAgents Terminal end to end, in a real pseudo-terminal.
//!
//! The test re-runs its own binary as the screen's process (the
//! `screen_process` test, which does nothing unless the parent set
//! `OA_TERMINAL_E2E`), on the slave side of a pseudo-terminal, and reads
//! the master side through `coder-vt`, the terminal emulator the phone's
//! terminal screen uses. The screen runs the
//! real input loop, guard, and drawing over the shared chat client with
//! the in-process backend; behind it are a scripted chat worker and a fake
//! Coder engine that works in a worktree of a scratch Git repository and
//! runs until it is stopped. Nothing reaches a relay, a model, a coding
//! agent, or the real home.
//!
//! The scenario: send a question and watch the answer stream; send a
//! coding message, which starts a Coder run at once; stop it with Esc;
//! start a second thread; open the first again from the thread list, which
//! shows its turns and its run up to the stop; quit with Ctrl+C twice.
//!
//! ```sh
//! cargo test -p openagents-terminal --test pty -- --nocapture
//! ```
//! prints the screens it checked.

#![cfg(unix)]

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::FromRawFd;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use openagents_chat::basic_chats::BasicChats;
use openagents_chat::basic_coder::{Door, Reply, Turn, lock};
use openagents_chat::client::{
    self, Client, Coder, Follow, Issue, Kind, Options, Place, Progress, Started,
};
use openagents_chat::coder_events::{
    self, Call, CoderEvent, Line, Output, Runner, Step, StepKind, Verb,
};
use openagents_chat::router::{
    Caller, CoderRun, Computer, Context, Engine, EngineState, Meta, Offer, Project,
};
use openagents_terminal::{Interrupter, Launch, NoExtras, Resume};
use serde_json::Value;

/// The screen as a terminal shows it (`coder-vt`, the emulator the
/// phone's terminal uses), and every byte the program wrote.
struct Grid {
    terminal: coder_vt::Terminal,
    raw: Vec<u8>,
}

impl Grid {
    fn new(rows: usize, cols: usize) -> Self {
        Self {
            terminal: coder_vt::Terminal::new(rows, cols, 0),
            raw: Vec::new(),
        }
    }

    fn feed(&mut self, bytes: &[u8]) {
        self.raw.extend_from_slice(bytes);
        self.terminal.feed(bytes);
    }

    fn text(&self) -> String {
        self.terminal
            .text()
            .lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

const ROWS: u16 = 40;
const COLS: u16 = 100;
const ENV: &str = "OA_TERMINAL_E2E";

// ---------------------------------------------------------------- the child

/// A chat worker that streams its answer in three pieces; a message that
/// asks for a fix gets a `run_coder` offer, the router's typed judgment
/// that it is coding work. (Only this fixture reads the words.)
struct Worker;

impl Door for Worker {
    fn ask(
        &self,
        turns: Vec<Turn>,
        _: Context,
        reply: Arc<Mutex<Reply>>,
    ) -> client::BoxFuture<'static, ()> {
        Box::pin(async move {
            let asked = turns
                .last()
                .map(|turn| turn.text.clone())
                .unwrap_or_default();
            if asked.contains("fix") {
                tokio::time::sleep(Duration::from_millis(300)).await;
                let mut reply = lock(&reply);
                reply.text = "Coder can fix that on this computer.".into();
                reply.meta = Meta {
                    offers: vec![Offer::RunCoder],
                    ..Meta::default()
                };
                reply.done = true;
                return;
            }
            for piece in ["Rain ", "on the ", "roof."] {
                tokio::time::sleep(Duration::from_millis(700)).await;
                lock(&reply).text.push_str(piece);
            }
            tokio::time::sleep(Duration::from_millis(300)).await;
            let mut reply = lock(&reply);
            reply.text = format!("Rain on the roof. You asked: {asked}");
            reply.done = true;
        })
    }
}

/// One fake task: its events so far, and whether it was asked to stop.
#[derive(Default)]
struct Task {
    lines: Mutex<Vec<Line>>,
    stop: AtomicBool,
    ended: AtomicBool,
}

/// A fake coding engine over a scratch Git repository: a run starts in a
/// real worktree, reports a start, a thought, a failing command, and
/// progress every 300 ms, and works until it is asked to stop.
struct FakeCoder {
    store: PathBuf,
    tasks: Mutex<HashMap<String, Arc<Task>>>,
}

impl FakeCoder {
    fn task(&self, id: &str) -> Option<Arc<Task>> {
        self.tasks.lock().unwrap().get(id).cloned()
    }
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
    }
}

impl Coder for FakeCoder {
    fn default_store(&self) -> PathBuf {
        self.store.clone()
    }
    fn context(&self, _: &Path, dir: Option<&Path>) -> Context {
        Context {
            computer_ready: true,
            computer: Some(Computer::Here {
                name: None,
                engines: vec![Engine {
                    engine: "codex".into(),
                    state: EngineState::Ready,
                }],
            }),
            project: dir.and_then(|dir| Project::at(&dir.display().to_string())),
            ..Context::default()
        }
    }
    fn predict(&self, _: &Path, _: Option<nostr::cj_conversation::Engine>) -> Option<Runner> {
        None
    }
    fn asks_first(&self) -> bool {
        false
    }
    fn checkout(&self, dir: &Path) -> Result<(), String> {
        git(dir, &["rev-parse", "--show-toplevel"]).map(|_| ())
    }
    fn start(
        &self,
        _: &Path,
        dir: &Path,
        _: &str,
        _: &str,
        chat: &str,
        _: Option<nostr::cj_conversation::Engine>,
    ) -> Result<Started, String> {
        let id = format!("t{}", self.tasks.lock().unwrap().len() + 1);
        let top = PathBuf::from(git(dir, &["rev-parse", "--show-toplevel"])?);
        let worktree = self.store.join("worktrees").join(&id);
        git(
            &top,
            &[
                "worktree",
                "add",
                "--detach",
                &worktree.display().to_string(),
            ],
        )?;
        let base = git(&worktree, &["rev-parse", "HEAD"])?;
        let task = Arc::new(Task::default());
        self.tasks.lock().unwrap().insert(id.clone(), task.clone());
        let (task_id, chat) = (id.clone(), chat.to_owned());
        let project = top
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let (checkout, place) = (top.display().to_string(), worktree.display().to_string());
        std::thread::spawn(move || {
            let mut seq = 0;
            let mut emit = |event: CoderEvent| {
                seq += 1;
                task.lines.lock().unwrap().push(Line {
                    seq,
                    task: task_id.clone(),
                    thread: Some(chat.clone()),
                    event,
                });
            };
            emit(CoderEvent::CoderStarted(coder_events::Started {
                turn: 1,
                project,
                checkout,
                worktree: place,
                base,
                provider: "codex".into(),
                model: "fake-engine".into(),
                reason: "Codex is signed in and has capacity.".into(),
                fallbacks: Vec::new(),
                via: "local".into(),
                runner: None,
            }));
            emit(CoderEvent::Step(Step {
                turn: 1,
                step_id: 1,
                kind: StepKind::Thinking,
                source: "agent".into(),
                text: "Reading the failing test.".into(),
                call: None,
            }));
            // Tool calls as an engine's typed steps carry them (#10117):
            // two that look, which group, then a command that fails.
            let tool = |kind: StepKind, verb: Verb, target: &str| {
                CoderEvent::Step(Step {
                    turn: 1,
                    step_id: 2,
                    kind,
                    source: "agent".into(),
                    text: target.into(),
                    call: Some(Call {
                        verb,
                        target: target.into(),
                        about: None,
                        failed: false,
                    }),
                })
            };
            emit(tool(StepKind::ToolCall, Verb::Read, "lib.rs"));
            emit(CoderEvent::Step(Step {
                turn: 1,
                step_id: 2,
                kind: StepKind::Observation,
                source: "system".into(),
                text: "pub fn add(a: i32, b: i32) -> i32 { a - b }".into(),
                call: None,
            }));
            emit(tool(StepKind::ToolCall, Verb::Search, "fn add"));
            emit(tool(StepKind::Command, Verb::Run, "cargo test"));
            emit(CoderEvent::Output(Output {
                turn: 1,
                step_id: 2,
                command: "cargo test".into(),
                exit: Some(101),
                timed_out: false,
                seconds: 1.5,
                text: "running 1 test\ntest adds ... FAILED\nerror: test failed".into(),
                truncated: false,
            }));
            let mut step = 2;
            while !task.stop.load(Ordering::SeqCst) {
                step += 1;
                emit(CoderEvent::Progress(coder_events::Progress {
                    turn: 1,
                    step,
                    seconds: step as f64,
                    done: None,
                    complete: Some(0.4),
                }));
                std::thread::sleep(Duration::from_millis(300));
            }
            emit(CoderEvent::Stopped(coder_events::Stopped {
                turn: 1,
                message: "Coder stopped this turn because you asked.".into(),
            }));
            task.ended.store(true, Ordering::SeqCst);
        });
        Ok(Started {
            task: id,
            project: top
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            worktree: worktree.display().to_string(),
        })
    }
    fn issue(&self, _: &str, _: &str, _: &Path) -> Option<Box<dyn Issue>> {
        None
    }
    fn follow(&self, _: &Path, task: &str, _: &str, _: Option<String>) -> Box<dyn Follow> {
        struct Following {
            task: Option<Arc<Task>>,
            at: usize,
        }
        impl Follow for Following {
            fn poll(&mut self) -> Result<(Vec<Line>, Progress), String> {
                let Some(task) = &self.task else {
                    return Err("no such task".into());
                };
                let ended = task.ended.load(Ordering::SeqCst);
                let lines = task.lines.lock().unwrap()[self.at..].to_vec();
                self.at += lines.len();
                let progress = if ended {
                    Progress::Ended
                } else {
                    Progress::Running
                };
                Ok((lines, progress))
            }
        }
        Box::new(Following {
            task: self.task(task),
            at: 0,
        })
    }
    fn stop(&self, _: &Path, task: &str) -> Result<(), String> {
        let task = self.task(task).ok_or("no such task")?;
        task.stop.store(true, Ordering::SeqCst);
        Ok(())
    }
    fn answer(&self, _: &Path, _: &str, _: &str) -> Result<usize, String> {
        Err("the fake engine takes no answers".into())
    }
    /// The engine reads a message at its next step: the trace records it
    /// as the person's.
    fn steer(&self, _: &Path, task: &str, text: &str) -> Result<client::Steering, String> {
        let task = self.task(task).ok_or("no such task")?;
        let mut lines = task.lines.lock().unwrap();
        let seq = lines.len() as u64 + 1;
        let (id, chat) = (lines[0].task.clone(), lines[0].thread.clone());
        lines.push(Line {
            seq,
            task: id,
            thread: chat,
            event: CoderEvent::Step(Step {
                turn: 1,
                step_id: 9,
                kind: StepKind::Message,
                source: "user".into(),
                text: text.into(),
                call: None,
            }),
        });
        Ok(client::Steering::NextStep)
    }
    fn result(&self, _: &Path, _: &str) -> Option<CoderRun> {
        None
    }
    fn trajectories(&self, _: &Path, _: &str) -> Vec<Value> {
        Vec::new()
    }
}

/// The screen's process: runs only when the parent test started it.
#[test]
fn screen_process() {
    let Ok(dir) = std::env::var(ENV) else {
        return;
    };
    let dir = PathBuf::from(dir);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();
    let exit = runtime.block_on(async {
        let repo = dir.join("demo");
        let home = dir.join("home");
        let coder: Arc<dyn Coder> = Arc::new(FakeCoder {
            store: dir.join("tasks"),
            tasks: Mutex::default(),
        });
        let interrupter = Interrupter::new();
        let options = Options {
            place: Place::Local,
            caller: Caller::TERMINAL,
            dir: Some(repo.clone()),
            home: home.clone(),
            interrupt: interrupter.interrupt(),
            hint: Some(|_: Kind, _: &str| "Type your answer and press Enter.".to_owned()),
        };
        let chats = BasicChats::new(
            Some(tokio::runtime::Handle::current()),
            Some(Arc::new(Worker)),
            None,
        );
        let client = Client::in_process(chats, home.clone(), false, options, coder.clone());
        openagents_terminal::run(Launch {
            client,
            coder,
            interrupter,
            extras: Arc::new(NoExtras),
            resume: Resume::New(None),
            folder: Some(repo),
            home,
            notices: Vec::new(),
            version: "openagents-terminal e2e".into(),
        })
        .await
    });
    match exit {
        Ok(exit) => println!(
            "SCREEN CLOSED thread={} running={}",
            exit.thread.unwrap_or_default(),
            exit.running
        ),
        Err(error) => println!("SCREEN FAILED {error}"),
    }
}

// --------------------------------------------------------------- the parent

/// The master side of a pseudo-terminal and the slave's path.
fn pty() -> (File, PathBuf) {
    // SAFETY: plain libc calls on a descriptor this function owns; the
    // returned name is copied before any other PTY call.
    unsafe {
        let master = libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY);
        assert!(master >= 0, "posix_openpt");
        assert_eq!(libc::grantpt(master), 0, "grantpt");
        assert_eq!(libc::unlockpt(master), 0, "unlockpt");
        let name = libc::ptsname(master);
        assert!(!name.is_null(), "ptsname");
        let path = std::ffi::CStr::from_ptr(name)
            .to_string_lossy()
            .into_owned();
        let size = libc::winsize {
            ws_row: ROWS,
            ws_col: COLS,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        libc::ioctl(master, libc::TIOCSWINSZ as _, &raw const size);
        (File::from_raw_fd(master), PathBuf::from(path))
    }
}

struct Session {
    master: File,
    grid: Arc<Mutex<Grid>>,
    child: std::process::Child,
}

impl Session {
    fn screen(&self) -> String {
        self.grid.lock().unwrap().text()
    }

    fn send(&mut self, bytes: &[u8]) {
        self.master.write_all(bytes).unwrap();
        self.master.flush().unwrap();
        // Let the key land before the next one: a lone Esc is told apart
        // from an escape sequence by the pause after it.
        std::thread::sleep(Duration::from_millis(150));
    }

    fn typed(&mut self, text: &str) {
        for c in text.chars() {
            let mut buffer = [0; 4];
            self.master
                .write_all(c.encode_utf8(&mut buffer).as_bytes())
                .unwrap();
        }
        self.master.flush().unwrap();
        std::thread::sleep(Duration::from_millis(100));
        self.send(b"\r");
    }

    /// A left click at `row`, `col` (from 0), as an SGR mouse report.
    fn click(&mut self, row: usize, col: usize) {
        let (x, y) = (col + 1, row + 1);
        self.send(format!("\x1b[<0;{x};{y}M\x1b[<0;{x};{y}m").as_bytes());
    }

    /// A left-button drag along `row` from `from` to `to`.
    fn drag(&mut self, row: usize, from: usize, to: usize) {
        let y = row + 1;
        self.send(format!("\x1b[<0;{};{y}M", from + 1).as_bytes());
        self.send(format!("\x1b[<32;{};{y}M", to + 1).as_bytes());
        self.send(format!("\x1b[<0;{};{y}m", to + 1).as_bytes());
    }

    /// Wait until the program has written `bytes`.
    fn wait_raw(&mut self, what: &str, bytes: &str) {
        let deadline = Instant::now() + Duration::from_secs(30);
        while !String::from_utf8_lossy(&self.grid.lock().unwrap().raw).contains(bytes) {
            if Instant::now() > deadline {
                let _ = self.child.kill();
                panic!("timed out waiting for {what}");
            }
            std::thread::sleep(Duration::from_millis(40));
        }
    }

    /// Wait until the screen shows every one of `texts`.
    fn wait(&mut self, what: &str, texts: &[&str]) -> String {
        self.wait_for(what, texts, 30)
    }

    fn wait_for(&mut self, what: &str, texts: &[&str], seconds: u64) -> String {
        let deadline = Instant::now() + Duration::from_secs(seconds);
        loop {
            let screen = self.screen();
            if texts.iter().all(|text| screen.contains(text)) {
                eprintln!("---- {what} ----\n{screen}\n");
                return screen;
            }
            if Instant::now() > deadline {
                let _ = self.child.kill();
                panic!("timed out waiting for {what} ({texts:?}); the screen:\n{screen}");
            }
            std::thread::sleep(Duration::from_millis(40));
        }
    }
}

/// Run `command` on the slave side of a fresh pseudo-terminal, as its
/// session's controlling terminal, reading the screen through `coder-vt`.
fn spawn(mut command: Command) -> Session {
    let (master, slave) = pty();
    let open = || {
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&slave)
            .unwrap()
    };
    command
        .env("TERM", "xterm-256color")
        .env("COLORTERM", "truecolor")
        .env_remove("NO_COLOR")
        .stdin(Stdio::from(open()))
        .stdout(Stdio::from(open()))
        .stderr(Stdio::from(open()));
    // SAFETY: only async-signal-safe calls between fork and exec: a new
    // session, with the pseudo-terminal on standard input as its
    // controlling terminal.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::ioctl(0, libc::TIOCSCTTY as _, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    // The size goes on the slave too: some systems keep it per side.
    {
        let slave = open();
        let size = libc::winsize {
            ws_row: ROWS,
            ws_col: COLS,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        // SAFETY: TIOCSWINSZ reads the winsize passed and nothing else.
        unsafe {
            libc::ioctl(
                std::os::fd::AsRawFd::as_raw_fd(&slave),
                libc::TIOCSWINSZ as _,
                &raw const size,
            )
        };
    }
    let child = command.spawn().unwrap();
    let grid = Arc::new(Mutex::new(Grid::new(usize::from(ROWS), usize::from(COLS))));
    let mut reader = master.try_clone().unwrap();
    let feed = grid.clone();
    std::thread::spawn(move || {
        let mut buffer = [0; 8192];
        while let Ok(read) = reader.read(&mut buffer) {
            if read == 0 {
                break;
            }
            feed.lock().unwrap().feed(&buffer[..read]);
        }
    });
    Session {
        master,
        grid,
        child,
    }
}

#[test]
fn the_screen_end_to_end() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("demo");
    std::fs::create_dir_all(&repo).unwrap();
    for args in [
        &["init", "-q", "-b", "main"][..],
        &["config", "user.email", "test@example.com"],
        &["config", "user.name", "Test"],
    ] {
        git(&repo, args).unwrap();
    }
    std::fs::write(repo.join("lib.rs"), "pub fn adds() -> u8 { 1 + 1 }\n").unwrap();
    git(&repo, &["add", "."]).unwrap();
    git(&repo, &["commit", "-q", "-m", "first"]).unwrap();

    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "screen_process",
            "--exact",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(ENV, dir.path())
        .env("HOME", dir.path().join("home"));
    let mut session = spawn(command);

    // The welcome card: the version, the project, the agents, the chats.
    session.wait(
        "the welcome card",
        &["OpenAgents dev build", "Project", "Codex"],
    );

    // A question streams its answer, then the whole reply shows.
    session.typed("what is the weather like");
    let streaming = session.wait("the reply streaming", &["Rain on the"]);
    assert!(
        !streaming.contains("You asked:"),
        "the screen showed the reply before it finished:\n{streaming}"
    );
    session.wait(
        "the finished reply",
        &[
            "Rain on the roof. You asked: what is the weather like",
            "ready",
        ],
    );

    // A coding message: the router's offer starts Coder at once, in a
    // worktree of the scratch repository, and its steps stream in.
    session.typed("please fix the failing test");
    let run = session.wait(
        "the Coder run",
        &[
            "Coder can fix that on this computer.",
            "Codex is working.",
            "◈ Read 1 file, Searched 1 pattern",
            "◆ Run cargo test · exit 101",
            "≈40% done",
        ],
    );
    // Condensed: the reasoning folded into the group, file contents and
    // command output hidden.
    for hidden in ["Reading the failing test.", "a - b", "FAILED"] {
        assert!(!run.contains(hidden), "{hidden:?} shows condensed:\n{run}");
    }
    // Ctrl+O shows each call and its output, and condenses them again.
    session.send(b"\x0f");
    let open = session.wait(
        "the tool calls expanded",
        &[
            "◆ Read lib.rs",
            "a - b",
            "◆ Search \"fn add\"",
            "◆ Run cargo test",
            "test adds ... FAILED",
        ],
    );
    eprintln!("==== expanded ====\n{open}\n");
    session.send(b"\x0f");
    let closed = session.wait("the tool calls condensed", &["◆ Run cargo test · exit 101"]);
    assert!(!closed.contains("test adds ... FAILED"), "{closed}");
    // Progress is Jev's estimate, never a step budget.
    for line in run.lines().filter(|line| line.contains("% done")) {
        assert!(!line.contains(" of "), "a budget in {line:?}");
    }
    assert!(!run.contains("could not record"), "{run}");
    // One short start line: no task, no worktree path, no "signed in"
    // (#10115).
    for noise in [
        "Coder started task",
        "worktree of",
        "signed in and has capacity",
    ] {
        assert!(!run.contains(noise), "{noise} in:\n{run}");
    }
    assert!(
        dir.path().join("tasks/worktrees/t1/lib.rs").exists(),
        "the run works in a worktree of the scratch repository"
    );

    // The rail under the composer lists the run: its number, its agent,
    // and what it is doing (#10169). Up moves into it, Enter opens it
    // full screen, Esc comes back.
    let rail = session.wait("the rail", &["1 Codex · cargo test"]);
    eprintln!("==== rail ====\n{rail}\n");
    session.send(b"\x1b[A");
    session.wait(
        "the rail selected",
        &["› ", "Enter opens the run full screen"],
    );
    session.send(b"\r");
    let opened = session.wait(
        "the run opened from the rail",
        &["Coder run 1 · Codex · working", "test adds ... FAILED"],
    );
    assert!(!opened.contains("1 Codex · cargo test"), "{opened}");
    session.send(b"\x1b");
    session.wait("the chat again", &["1 Codex · cargo test"]);
    // Alt+1 opens it directly.
    session.send(b"\x1b1");
    session.wait(
        "the run opened with Alt+1",
        &["Coder run 1 · Codex · working"],
    );
    session.send(b"\x1b");
    session.wait("the chat again", &["1 Codex · cargo test"]);

    // Ctrl+R: the run full screen, each command with its output, without
    // the chat's reply; its composer sends the run a message.
    session.send(b"\x12");
    let view = session.wait(
        "the run view",
        &[
            "Coder run · working",
            "test adds ... FAILED",
            "◆ Read lib.rs",
        ],
    );
    assert!(
        !view.contains("Coder can fix that on this computer."),
        "{view}"
    );
    session.typed("use spaces, not tabs");
    session.wait(
        "the message sent to the run",
        &[
            "use spaces, not tabs",
            "Coder reads it at its next step.",
            "Coder read your message.",
        ],
    );
    // A click on a file's path opens it, read only.
    let (row, col) = find(&session.wait("the path", &["◆ Read lib.rs"]), "lib.rs");
    session.click(row, col + 1);
    let file = session.wait(
        "the file view",
        &[
            "lib.rs · read only · Esc closes",
            "1  pub fn adds() -> u8 { 1 + 1 }",
        ],
    );
    eprintln!("==== file ====\n{file}\n");
    // A drag selects and copies through the terminal (OSC 52).
    let (row, col) = find(&file, "pub fn adds");
    session.drag(row, col, col + 10);
    session.wait_raw("the selection copied", "\x1b]52;c;cHViIGZuIGFkZHM=");
    session.send(b"\x1b");
    session.wait("the run view again", &["Coder run · working"]);
    session.send(b"\x1b");
    session.wait("the chat again", &["Coder can fix that on this computer."]);

    // Esc stops the run, not the screen.
    session.send(b"\x1b");
    session.wait(
        "the run stopped",
        &[
            "Asked Coder to stop task t1",
            "Coder stopped this turn because you asked.",
            "ready",
        ],
    );

    // A second thread.
    session.typed("/new");
    session.wait("a new thread", &["New thread.", "new thread"]);
    session.typed("is it still raining");
    session.wait(
        "the second thread's reply",
        &["You asked: is it still raining", "ready"],
    );

    // An unknown command says so and lists the ones like it.
    session.typed("/resum");
    session.wait(
        "the unknown command",
        &["/resum is not a command.", "/resume [ID or title]"],
    );

    // Ctrl+T opens the same picker /resume does; Esc closes it.
    session.send(b"\x14");
    let list = session.wait(
        "the thread picker",
        &[
            "Resume thread",
            "/ to search",
            " demo ─",
            " Chats ─",
            "Enter select",
        ],
    );
    assert!(list.contains("open"), "{list}");
    session.send(b"\x1b");

    // `/resume` with words nothing matches opens the picker narrowed to
    // them; Esc clears them, and Esc again closes it.
    session.typed("/resume zzzz");
    session.wait(
        "the picker narrowed",
        &["search: zzzz", "No threads match."],
    );
    session.send(b"\x1b");
    session.wait("the query cleared", &["/ to search", " demo ─"]);
    session.send(b"\x1b");

    // `/resume` resumes the first thread, the first row under this
    // folder's project: its turns and its run.
    session.typed("/resume");
    session.wait("the picker", &["Resume thread", " demo ─"]);
    session.send(b"\r");
    let resumed = session.wait(
        "the first thread again",
        &[
            "Opened",
            "please fix the failing test",
            "Coder can fix that on this computer.",
            "Coder stopped this turn because you asked.",
        ],
    );
    assert!(!resumed.contains("is it still raining"), "{resumed}");

    // Ctrl+C twice quits; the terminal comes back.
    session.send(b"\x03");
    session.wait("the quit prompt", &["Press Ctrl+C again to quit."]);
    session.send(b"\x03");
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = session.child.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline, "the screen did not close");
        std::thread::sleep(Duration::from_millis(50));
    };
    std::thread::sleep(Duration::from_millis(200));
    let grid = session.grid.lock().unwrap();
    let raw = String::from_utf8_lossy(&grid.raw);
    assert!(status.success(), "{raw}");
    assert!(raw.contains("SCREEN CLOSED thread="), "{raw}");
    // The look: white on Grok Night's near-black, never amber; the cursor is white and
    // its color is handed back, and the alternate screen is left.
    assert!(raw.contains("\x1b]12;#FFFFFF\x07"), "a white cursor");
    assert!(
        raw.contains("\x1b]112\x07"),
        "the cursor color is handed back"
    );
    assert!(!raw.contains("255;176;0"), "no amber anywhere");
    assert!(raw.contains("48;2;20;20;20"), "Grok Night's field, #141414");
    assert!(raw.contains("38;2;255;255;255"), "full white text");
    assert!(raw.contains("\x1b[?1049l"), "the alternate screen is left");
    // Mouse reports on while the screen runs, and off again after.
    assert!(raw.contains("\x1b[?1000h") && raw.contains("\x1b[?1000l"));
}

/// The row and column, from 0, where `needle` first shows on `screen`.
fn find(screen: &str, needle: &str) -> (usize, usize) {
    screen
        .lines()
        .enumerate()
        .find_map(|(row, line)| {
            line.find(needle)
                .map(|at| (row, line[..at].chars().count()))
        })
        .unwrap_or_else(|| panic!("{needle:?} is not on the screen:\n{screen}"))
}

/// The real program against the live chat worker, by hand:
///
/// ```sh
/// OA_TERMINAL_LIVE='cargo run -q -p openagents-cli --bin openagents -- terminal --scratch' \
///   cargo test -p openagents-terminal --test pty -- --ignored --nocapture live
/// ```
///
/// It runs the command with `sh -c` in this checkout, with a throwaway
/// HOME (so no identity, settings, or coding agent login of the person's
/// is read; Cargo and rustup keep their own homes), asks one plain
/// question, prints the screen, and quits.
#[test]
#[ignore = "talks to the live chat worker"]
fn live() {
    let Ok(line) = std::env::var("OA_TERMINAL_LIVE") else {
        return;
    };
    let question = std::env::var("OA_TERMINAL_LIVE_ASK")
        .unwrap_or_else(|_| "In two sentences, what is OpenAgents?".into());
    let home = tempfile::tempdir().unwrap();
    let real = PathBuf::from(std::env::var("HOME").unwrap());
    let checkout = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut command = Command::new("sh");
    command
        .arg("-c")
        .arg(&line)
        .current_dir(&checkout)
        .env("HOME", home.path())
        .env(
            "CARGO_HOME",
            std::env::var_os("CARGO_HOME").unwrap_or_else(|| real.join(".cargo").into()),
        )
        .env(
            "RUSTUP_HOME",
            std::env::var_os("RUSTUP_HOME").unwrap_or_else(|| real.join(".rustup").into()),
        );
    let mut session = spawn(command);
    session.wait_for("the welcome card", &["OpenAgents dev build"], 600);
    session.typed(&question);
    session.wait_for("the reply streaming", &["replying"], 60);
    let deadline = Instant::now() + Duration::from_secs(180);
    loop {
        let screen = session.screen();
        let replied = screen.lines().any(|line| line.trim() == "openagents");
        if replied && !screen.contains("replying") {
            break;
        }
        assert!(Instant::now() < deadline, "no reply:\n{screen}");
        std::thread::sleep(Duration::from_millis(100));
    }
    std::thread::sleep(Duration::from_secs(1));
    eprintln!("==== live screen ====\n{}\n", session.screen());
    session.send(b"\x03");
    session.send(b"\x03");
    let deadline = Instant::now() + Duration::from_secs(20);
    while session.child.try_wait().unwrap().is_none() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    std::thread::sleep(Duration::from_millis(300));
    eprintln!("==== after quitting ====\n{}\n", session.screen());
}
