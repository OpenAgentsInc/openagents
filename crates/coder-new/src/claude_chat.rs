//! One long-lived Claude Code process per claude chat (#11447), on the
//! SDK's streaming input.
//!
//! The process outlives each turn, so background shell jobs, monitors and
//! background subagents keep running after the reply, and the turns they
//! start later reach the chat. A message typed while a turn runs is sent at
//! once, to be read next. Stopping a turn interrupts it, and the process and
//! its session stay. Closing a chat ends its input and lets the process
//! finish its work; quitting Coder ends it.
//!
//! The process runs on its own thread. [`Chat`] is the handle the terminal
//! keeps: it sends messages and stops, and [`Chat::poll`] hands back what
//! the process reported, each event marked with the number of the turn it
//! belongs to so a stopped turn's tail can be told apart from the next.
//!
//! A chat that already holds a session ([`crate::live::ClaudeSession`])
//! starts its process on that session. When Claude Code no longer has it,
//! the process starts again as a new session seeded with the pasted
//! conversation ([`crate::claude::task_with`]).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};

use claude_agent_sdk::{QueryInput, SdkUserMessage, UserMessagePriority};
use openrouter::Streamed;

use crate::bundled_runtime::RuntimeEvent;
use crate::claude;

/// What a chat's process reports.
pub enum Event {
    /// Part of turn `turn`: its text, model, tools and delegations.
    Runtime { turn: u64, event: RuntimeEvent },
    /// The Claude Code session the process runs in.
    Session(String),
    /// Turn `turn` ended.
    TurnEnd {
        turn: u64,
        result: Result<Streamed, String>,
    },
    /// The process is gone; the chat starts a new one for its next message.
    Exited,
}

/// What the terminal asks of the process thread.
enum Command {
    Say { text: String, next: bool },
    Close,
}

/// The turn the process is reading, from one, whether a message of it
/// arrived, and the flag that stops it.
struct Reading {
    turn: u64,
    active: bool,
    cancel: Arc<AtomicBool>,
}

impl Default for Reading {
    fn default() -> Self {
        Self {
            turn: 1,
            active: false,
            cancel: Arc::default(),
        }
    }
}

/// The handle to one chat's Claude Code process.
pub struct Chat {
    commands: tokio::sync::mpsc::UnboundedSender<Command>,
    events: mpsc::Receiver<Event>,
    reading: Arc<Mutex<Reading>>,
    kill: Option<tokio::sync::oneshot::Sender<()>>,
    /// The checkout the process works in.
    pub cwd: PathBuf,
    /// Whether the Solver tool is beside the rest.
    pub solver: bool,
    /// The session the process runs in, once Claude Code names it, or the
    /// one it was started to continue.
    pub session: Option<String>,
    exited: bool,
}

impl Chat {
    /// Starts the process and sends it the chat's first message: `newest`
    /// alone when it continues `resume`, otherwise `seed`, the pasted
    /// conversation, which is also what a session Claude Code lost falls
    /// back to.
    #[must_use]
    pub fn start(
        cwd: PathBuf,
        solver: bool,
        resume: Option<String>,
        newest: String,
        seed: String,
    ) -> Self {
        let (commands, receiver) = tokio::sync::mpsc::unbounded_channel();
        let (sender, events) = mpsc::channel();
        let (kill, killed) = tokio::sync::oneshot::channel();
        let reading = Arc::new(Mutex::new(Reading::default()));
        let thread = Thread {
            cwd: cwd.clone(),
            solver,
            resume: resume.clone(),
            newest,
            seed,
            commands: receiver,
            events: sender,
            reading: reading.clone(),
        };
        std::thread::spawn(move || thread.run(killed));
        Self {
            commands,
            events,
            reading,
            kill: Some(kill),
            cwd,
            solver,
            session: resume,
            exited: false,
        }
    }

    /// The turn the process is reading now: the one a message sent now
    /// belongs to.
    #[must_use]
    pub fn turn(&self) -> u64 {
        lock(&self.reading).turn
    }

    /// Sends a message. `next` asks Claude Code to read it as soon as the
    /// running turn allows rather than after the turn.
    pub fn say(&self, text: &str, next: bool) {
        let _ = self.commands.send(Command::Say {
            text: text.to_owned(),
            next,
        });
    }

    /// Stops turn `turn`, or the turn after it that a message sent during
    /// it started, and returns the newest turn the stop covers. A turn that
    /// already ended is left alone.
    pub fn interrupt(&self, turn: u64) -> u64 {
        let reading = lock(&self.reading);
        if reading.turn == turn || (reading.turn > turn && reading.active) {
            reading.cancel.store(true, Ordering::Relaxed);
        }
        reading.turn.max(turn)
    }

    /// Ends the process's input: it finishes what it is doing, background
    /// work included, and exits.
    pub fn close(&mut self) {
        let _ = self.commands.send(Command::Close);
    }

    /// Whether the process is gone.
    #[must_use]
    pub fn exited(&self) -> bool {
        self.exited
    }

    /// What the process reported since the last poll.
    pub fn poll(&mut self) -> Vec<Event> {
        let mut events = Vec::new();
        loop {
            match self.events.try_recv() {
                Ok(event) => {
                    match &event {
                        Event::Session(id) => self.session = Some(id.clone()),
                        Event::Exited => self.exited = true,
                        _ => {}
                    }
                    events.push(event);
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    if !self.exited {
                        self.exited = true;
                        events.push(Event::Exited);
                    }
                    break;
                }
            }
        }
        events
    }
}

impl Drop for Chat {
    /// Quitting ends the process and its process group, and waits briefly
    /// for that so no Claude Code is left running behind Coder.
    fn drop(&mut self) {
        if let Some(kill) = self.kill.take() {
            let _ = kill.send(());
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while !self.exited {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            match self.events.recv_timeout(left) {
                Ok(Event::Exited) | Err(_) => break,
                Ok(_) => {}
            }
        }
    }
}

fn lock(reading: &Mutex<Reading>) -> std::sync::MutexGuard<'_, Reading> {
    reading
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The process thread's side.
struct Thread {
    cwd: PathBuf,
    solver: bool,
    resume: Option<String>,
    newest: String,
    seed: String,
    commands: tokio::sync::mpsc::UnboundedReceiver<Command>,
    events: mpsc::Sender<Event>,
    reading: Arc<Mutex<Reading>>,
}

impl Thread {
    fn run(self, killed: tokio::sync::oneshot::Receiver<()>) {
        let events = self.events.clone();
        if let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            runtime.block_on(async move {
                tokio::select! {
                    // Dropping the reader drops the query, which ends the
                    // process group.
                    _ = killed => {}
                    () = self.serve() => {}
                }
            });
        }
        let _ = events.send(Event::Exited);
    }

    async fn serve(self) {
        let Thread {
            cwd,
            solver,
            resume,
            newest,
            seed,
            mut commands,
            events,
            reading,
        } = self;
        let input: Arc<Mutex<Option<QueryInput>>> = Arc::new(Mutex::new(None));
        let commander = {
            let input = input.clone();
            async move {
                while let Some(command) = commands.recv().await {
                    // A message sent while the process starts waits for it.
                    let current = loop {
                        let current = input
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .clone();
                        if let Some(current) = current {
                            break current;
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    };
                    let _ = match command {
                        Command::Say { text, next } => {
                            let mut message = SdkUserMessage::blocks(claude::content(&text));
                            if next {
                                message = message.with_priority(UserMessagePriority::Next);
                            }
                            current.send_message(message).await
                        }
                        Command::Close => current.end_input().await,
                    };
                }
                // The terminal let go of the chat without closing it: the
                // process stays until it is killed.
                std::future::pending::<()>().await;
            }
        };
        let reader = async move {
            let fail = |error: String| {
                let _ = events.send(Event::TurnEnd {
                    turn: lock(&reading).turn,
                    result: Err(error),
                });
            };
            // A chat with a session continues it; a session Claude Code
            // lost fails before it names itself, and the chat starts anew
            // from the pasted conversation.
            let resume = resume.filter(|id| claude::kept(id));
            let mut unconfirmed = resume.is_some();
            let opened = match resume {
                Some(id) => {
                    let resume = claude::Resume {
                        id,
                        note: claude::note(solver),
                    };
                    match start(&cwd, solver, Some(resume), &newest, &input).await {
                        Ok(opened) => Ok(opened),
                        Err(_) => {
                            unconfirmed = false;
                            start(&cwd, solver, None, &seed, &input).await
                        }
                    }
                }
                None => start(&cwd, solver, None, &seed, &input).await,
            };
            let mut opened = match opened {
                Ok(opened) => opened,
                Err(error) => return fail(error),
            };
            loop {
                let (turn, cancel) = {
                    let current = lock(&reading);
                    (current.turn, current.cancel.clone())
                };
                let mut named = false;
                let result = {
                    let mut report = |id: &str| {
                        named = true;
                        lock(&reading).active = true;
                        let _ = events.send(Event::Session(id.to_owned()));
                    };
                    let mut emit = |event: RuntimeEvent| {
                        lock(&reading).active = true;
                        let _ = events.send(Event::Runtime { turn, event });
                    };
                    claude::read_turn(
                        &mut opened.query,
                        &cwd,
                        &mut opened.delegations,
                        true,
                        &cancel,
                        &mut report,
                        &mut emit,
                    )
                    .await
                };
                let ended = opened.query.is_completed();
                if unconfirmed {
                    unconfirmed = false;
                    if !named && ended && result.is_err() && !cancel.load(Ordering::Relaxed) {
                        match start(&cwd, solver, None, &seed, &input).await {
                            Ok(fresh) => {
                                opened = fresh;
                                continue;
                            }
                            Err(error) => return fail(error),
                        }
                    }
                }
                let _ = events.send(Event::TurnEnd { turn, result });
                if ended {
                    return;
                }
                let mut current = lock(&reading);
                current.turn += 1;
                current.active = false;
                current.cancel = Arc::default();
            }
        };
        tokio::select! {
            () = reader => {}
            () = commander => {}
        }
    }
}

/// Opens the process, sends it `first`, and makes it the one later
/// messages go to.
async fn start(
    cwd: &std::path::Path,
    solver: bool,
    resume: Option<claude::Resume>,
    first: &str,
    input: &Mutex<Option<QueryInput>>,
) -> Result<claude::Opened, String> {
    let opened = claude::open(cwd, solver, resume).await?;
    opened
        .query
        .send_user_content(claude::content(first))
        .await
        .map_err(|error| format!("Claude Code did not take the message: {error}"))?;
    *input
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(opened.query.input());
    Ok(opened)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Waits for turn `turn` to end, gathering its text.
    fn finish_turn(chat: &mut Chat, turn: u64) -> Result<Streamed, String> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(180);
        while std::time::Instant::now() < deadline {
            for event in chat.poll() {
                match event {
                    Event::TurnEnd {
                        turn: ended,
                        result,
                    } if ended >= turn => return result,
                    Event::Exited => return Err("exited".into()),
                    _ => {}
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        Err("timed out".into())
    }

    /// One real chat on this computer's Claude login: two turns in one
    /// process, the second remembering the first, then a stop that leaves
    /// the process up for a third. Run by hand:
    /// `cargo test -p coder-new --lib a_live_chat -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs a Claude Code login"]
    fn a_live_chat_keeps_one_process_across_turns() {
        let dir = tempfile::tempdir().unwrap();
        let first = "Remember the word PELICAN. Reply with just OK.";
        let mut chat = Chat::start(
            dir.path().to_owned(),
            false,
            None,
            first.into(),
            first.into(),
        );
        let reply = finish_turn(&mut chat, 1).unwrap();
        println!("1: {}", reply.text);
        assert!(chat.session.is_some(), "the session was named");
        let session = chat.session.clone();

        let turn = chat.turn();
        chat.say("What word did I ask you to remember? One word.", false);
        let reply = finish_turn(&mut chat, turn).unwrap();
        println!("2: {}", reply.text);
        assert!(
            reply.text.to_uppercase().contains("PELICAN"),
            "{}",
            reply.text
        );
        assert_eq!(chat.session, session, "the same session");

        let turn = chat.turn();
        chat.say("Write a 4000-word essay on pelicans, in plain text.", false);
        std::thread::sleep(std::time::Duration::from_secs(3));
        chat.interrupt(turn);
        let stopped = finish_turn(&mut chat, turn);
        println!("3: {stopped:?}");
        assert_eq!(stopped.unwrap_err(), claude::STOPPED);
        assert!(!chat.exited(), "a stop leaves the process up");

        let turn = chat.turn();
        chat.say("Reply with just DONE.", false);
        let reply = finish_turn(&mut chat, turn).unwrap();
        println!("4: {}", reply.text);
        assert!(!chat.exited());

        // Quitting ends the process; a reopened chat continues its session
        // in a new one.
        drop(chat);
        let mut chat = Chat::start(
            dir.path().to_owned(),
            false,
            session,
            "What word did I ask you to remember? One word.".into(),
            "Reply with just LOST.".into(),
        );
        let reply = finish_turn(&mut chat, 1).unwrap();
        println!("5: {}", reply.text);
        assert!(
            reply.text.to_uppercase().contains("PELICAN"),
            "{}",
            reply.text
        );

        // A session Claude Code does not have starts anew from the seed.
        drop(chat);
        let mut chat = Chat::start(
            dir.path().to_owned(),
            false,
            Some("00000000-0000-4000-8000-000000000000".into()),
            "Reply with just RESUMED.".into(),
            "Reply with just FRESH.".into(),
        );
        let reply = finish_turn(&mut chat, 1).unwrap();
        println!("6: {}", reply.text);
        assert!(reply.text.contains("FRESH"), "{}", reply.text);
    }
}
