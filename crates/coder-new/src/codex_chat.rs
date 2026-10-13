//! One long-lived `codex app-server` per Codex chat (#11510), on the Coder
//! SDK's Codex backend (`coder_sdk::codex`).
//!
//! The process and its thread outlive each turn: a follow-up continues the
//! same Codex thread, so Codex keeps its context and prompt cache. A message
//! typed while a turn runs is steered into that turn. Stopping a turn
//! interrupts it; the process and thread stay. Closing a chat ends the
//! process.
//!
//! The handle speaks the same events as a Claude Code chat
//! ([`crate::claude_chat::Event`]), so the terminal shows both the same way.
//! A chat that holds a thread starts its process on it; a thread Codex no
//! longer has starts anew from the pasted conversation.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};

use coder_sdk::codex::{Codex, Control, Open, Output};
use openrouter::{Streamed, Usage};
use tokio::sync::mpsc::UnboundedSender;

use crate::bundled_runtime::{RoundUsage, RuntimeEvent};
pub use crate::claude_chat::Event;

/// What a stopped turn says, as on Claude Code.
pub const STOPPED: &str = crate::claude::STOPPED;

/// What the terminal asks of the process thread.
enum Command {
    Say {
        text: String,
        next: bool,
    },
    /// Run from the next turn on this model.
    Model(String),
    Close,
}

/// The turn the process is reading, from one, whether it is under way, and
/// where to send a stop or a steer while it is.
#[derive(Default)]
struct Reading {
    turn: u64,
    active: bool,
    control: Option<UnboundedSender<Control>>,
}

/// The handle to one chat's Codex process.
pub struct Chat {
    commands: tokio::sync::mpsc::UnboundedSender<Command>,
    events: mpsc::Receiver<Event>,
    reading: Arc<Mutex<Reading>>,
    kill: Option<tokio::sync::oneshot::Sender<()>>,
    /// The checkout the process works in.
    pub cwd: PathBuf,
    /// Coder's own tools, offered to Codex in process.
    pub tools: crate::claude_tools::Tools,
    /// The Codex thread, once Codex names it, or the one it continues.
    pub session: Option<String>,
    /// Codex asks no questions through Coder; kept for the terminal's
    /// shape.
    pub asks: crate::claude_ask::Asks,
    /// The key of the briefing the process last read (#11473).
    pub briefing: Option<String>,
    /// The model the next turn runs.
    pub model: String,
    exited: bool,
}

impl Chat {
    /// Starts the process and sends it the chat's first message: `newest`
    /// alone when it continues `resume`, otherwise `seed`, the pasted
    /// conversation, which a lost thread also falls back to.
    #[must_use]
    pub fn start(
        cwd: PathBuf,
        tools: crate::claude_tools::Tools,
        resume: Option<String>,
        newest: String,
        seed: String,
        briefing: Option<crate::context::Briefing>,
        model: String,
    ) -> Self {
        Self::start_on(None, cwd, tools, resume, newest, seed, briefing, model)
    }

    /// [`Chat::start`] on `program` when given, else the `codex` this
    /// computer runs.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn start_on(
        program: Option<PathBuf>,
        cwd: PathBuf,
        tools: crate::claude_tools::Tools,
        resume: Option<String>,
        newest: String,
        seed: String,
        briefing: Option<crate::context::Briefing>,
        model: String,
    ) -> Self {
        let (commands, receiver) = tokio::sync::mpsc::unbounded_channel();
        let (sender, events) = mpsc::channel();
        let (kill, killed) = tokio::sync::oneshot::channel();
        let reading = Arc::new(Mutex::new(Reading {
            turn: 1,
            ..Reading::default()
        }));
        let key = briefing.as_ref().map(crate::context::Briefing::key);
        let thread = Thread {
            program,
            cwd: cwd.clone(),
            tools: tools.clone(),
            resume: resume.clone(),
            newest,
            seed,
            briefing,
            model: model.clone(),
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
            tools,
            session: resume,
            asks: crate::claude_ask::Asks::default(),
            briefing: key,
            model,
            exited: false,
        }
    }

    /// Runs the next turn on `model`, the thread kept (#11480).
    pub fn set_model(&mut self, model: &str) {
        if self.model != model {
            let _ = self.commands.send(Command::Model(model.to_owned()));
            self.model = model.to_owned();
        }
    }

    /// The turn a message sent now belongs to.
    #[must_use]
    pub fn turn(&self) -> u64 {
        lock(&self.reading).turn
    }

    /// Sends a message. `next` steers it into the running turn.
    pub fn say(&self, text: &str, next: bool) {
        let _ = self.commands.send(Command::Say {
            text: text.to_owned(),
            next,
        });
    }

    /// Stops turn `turn` if it still runs, and returns the newest turn the
    /// stop covers.
    pub fn interrupt(&self, turn: u64) -> u64 {
        let reading = lock(&self.reading);
        if reading.turn == turn
            && reading.active
            && let Some(control) = &reading.control
        {
            let _ = control.send(Control::Interrupt);
        }
        reading.turn.max(turn)
    }

    /// Ends the process once the running turn is done.
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
    /// Quitting ends the process, and waits briefly for that.
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

/// Milliseconds since `since`.
fn elapsed_ms(since: std::time::Instant) -> u64 {
    u64::try_from(since.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn lock(reading: &Mutex<Reading>) -> std::sync::MutexGuard<'_, Reading> {
    reading
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The process thread's side.
struct Thread {
    /// The `codex` to run, when not this computer's own.
    program: Option<PathBuf>,
    cwd: PathBuf,
    tools: crate::claude_tools::Tools,
    resume: Option<String>,
    newest: String,
    seed: String,
    briefing: Option<crate::context::Briefing>,
    model: String,
    commands: tokio::sync::mpsc::UnboundedReceiver<Command>,
    events: mpsc::Sender<Event>,
    reading: Arc<Mutex<Reading>>,
}

/// The developer instructions a chat's thread opens with: Coder's identity,
/// what Codex is told about Coder's tools, the session worktree's rule when
/// it has one, how to run noisy commands when Token savers is on, and the
/// chat's briefing (#11473) in place of Codex's own
/// `AGENTS.md` reading.
#[must_use]
pub fn instructions(
    tools: &crate::claude_tools::Tools,
    model: &str,
    briefing: Option<&crate::context::Briefing>,
    cwd: &std::path::Path,
) -> String {
    let mut text = crate::identity::prompt_on(crate::harness_plugin::Harness::Codex, model);
    let coder = crate::claude_tools::instructions(tools);
    if !coder.is_empty() {
        text.push('\n');
        text.push_str(&coder.replace(
            &format!("are under the `{}` server", crate::claude_tools::SERVER),
            "are offered to you directly",
        ));
    }
    if crate::checkout_guard::Boundary::around(cwd).is_some() {
        text.push('\n');
        text.push_str(crate::checkout_guard::INSTRUCTIONS);
    }
    if let Some(squeeze) = crate::token_savers::squeeze_instructions() {
        text.push('\n');
        text.push_str(&squeeze);
    }
    if let Some(briefing) = briefing {
        text.push('\n');
        text.push_str(briefing.render(false).trim_end());
        text.push('\n');
    }
    text
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
                    // Dropping the connection kills the process group.
                    _ = killed => {}
                    () = self.serve() => {}
                }
            });
        }
        let _ = events.send(Event::Exited);
    }

    async fn serve(self) {
        let Thread {
            program,
            cwd,
            tools,
            resume,
            newest,
            seed,
            briefing,
            mut model,
            mut commands,
            events,
            reading,
        } = self;
        let fail = |error: String| {
            let _ = events.send(Event::TurnEnd {
                turn: lock(&reading).turn,
                result: Err(error),
            });
        };
        if !crate::approval::tools_allowed() {
            return fail("This host refuses every tool, so Codex cannot work here.".into());
        }
        // A named program is a real app-server, given its full time.
        let named = program.is_some();
        let Some(program) = program.or_else(crate::codex::program) else {
            return fail(crate::codex::NEEDS_LOGIN.into());
        };
        // Coder's tools, in process; what a background agent reports goes
        // to the chat's tray as it happens.
        let (sender, mut delegations) = tokio::sync::mpsc::unbounded_channel();
        let stop = Arc::new(AtomicBool::new(false));
        let server = crate::claude_tools::tool_server(
            &tools,
            crate::delegate_host::CODEX,
            sender,
            stop.clone(),
        );
        let forward = {
            let events = events.clone();
            let reading = reading.clone();
            async move {
                while let Some(event) = delegations.recv().await {
                    let turn = lock(&reading).turn;
                    let _ = events.send(Event::Runtime { turn, event });
                }
                std::future::pending::<()>().await;
            }
        };
        let mut launch = crate::codex::launch(program, &cwd, briefing.is_some());
        if named {
            launch.start_wait = None;
        }
        let hooks = crate::sdk_hooks::codex(&cwd);
        let mut codex = match Codex::spawn(&launch, hooks).await {
            Ok(codex) => codex,
            Err(error) => {
                crate::codex::recheck();
                return fail(error);
            }
        };
        crate::routing_record::note_version(crate::harness_plugin::Harness::Codex, codex.version());
        let effort = crate::codex::choice().effort;
        let open = |resume: Option<String>| Open {
            cwd: cwd.clone(),
            model: Some(model.clone()),
            effort: effort.clone(),
            developer_instructions: Some(instructions(&tools, &model, briefing.as_ref(), &cwd)),
            resume,
            tools: server.clone(),
        };
        let mut emit_open = |output: Output| {
            if let Output::Thread(id) = output {
                let _ = events.send(Event::Session(id));
            }
        };
        // A chat with a thread continues it; a thread Codex lost starts
        // anew from the pasted conversation.
        let first = match resume {
            Some(id) => match codex.open(&open(Some(id)), &mut emit_open).await {
                Ok(_) => newest,
                Err(_) => match codex.open(&open(None), &mut emit_open).await {
                    Ok(_) => seed,
                    Err(error) => return fail(error),
                },
            },
            None => match codex.open(&open(None), &mut emit_open).await {
                Ok(_) => seed,
                Err(error) => return fail(error),
            },
        };
        let work = async {
            let mut waiting = std::collections::VecDeque::from([first]);
            let mut closing = false;
            loop {
                let text = match waiting.pop_front() {
                    Some(text) => text,
                    None if closing => return,
                    None => match commands.recv().await {
                        Some(Command::Say { text, .. }) => text,
                        Some(Command::Model(next)) => {
                            model = next;
                            continue;
                        }
                        Some(Command::Close) | None => return,
                    },
                };
                let (control, mut controls) = tokio::sync::mpsc::unbounded_channel();
                let turn = {
                    let mut current = lock(&reading);
                    current.active = true;
                    current.control = Some(control.clone());
                    current.turn
                };
                let mut used = Usage::default();
                let running_model = model.clone();
                // The reply names the model the turn runs on.
                let _ = events.send(Event::Runtime {
                    turn,
                    event: RuntimeEvent::Model(running_model.clone()),
                });
                // Codex's reasoning summary streams into a thinking entry;
                // reasoning that shows no words says so once (#11552).
                let mut thought = crate::claude_stream::Thought::default();
                let mut reasoning = false;
                // Each call's usage names the model it ran on and how long
                // it took since the turn started or the call before ended
                // (#11552).
                let mut call_model = running_model.clone();
                let mut call_started = std::time::Instant::now();
                let mut first_text: Option<u64> = None;
                let result = {
                    let mut emit = |output: Output| {
                        let event = match output {
                            Output::Thread(id) => {
                                let _ = events.send(Event::Session(id));
                                return;
                            }
                            Output::Model(model) => {
                                call_model.clone_from(&model);
                                RuntimeEvent::Model(model)
                            }
                            Output::Text(text) => {
                                if first_text.is_none() {
                                    first_text = Some(elapsed_ms(call_started));
                                }
                                RuntimeEvent::Text(text)
                            }
                            Output::Thinking(true) => {
                                reasoning = true;
                                RuntimeEvent::Thinking(true)
                            }
                            Output::ThinkingText(text) => {
                                for event in thought.piece(&text) {
                                    let _ = events.send(Event::Runtime { turn, event });
                                }
                                return;
                            }
                            Output::Thinking(false) => {
                                if thought.open() {
                                    for event in thought.end(None) {
                                        let _ = events.send(Event::Runtime { turn, event });
                                    }
                                    reasoning = false;
                                    return;
                                }
                                if std::mem::take(&mut reasoning)
                                    && let Some(event) = thought.hidden()
                                {
                                    let _ = events.send(Event::Runtime { turn, event });
                                }
                                RuntimeEvent::Thinking(false)
                            }
                            Output::Tool {
                                name,
                                input,
                                output,
                                running,
                            } => RuntimeEvent::Tool {
                                name,
                                input,
                                output,
                                running,
                            },
                            Output::Usage(usage) => {
                                let last = &usage["last"];
                                let round = RoundUsage {
                                    model: call_model.clone(),
                                    prompt_tokens: last["inputTokens"].as_u64().unwrap_or(0),
                                    completion_tokens: last["outputTokens"].as_u64().unwrap_or(0),
                                    reasoning_tokens: last["reasoningOutputTokens"].as_u64(),
                                    cost: None,
                                    milliseconds: elapsed_ms(call_started),
                                    first_token_ms: first_text.take(),
                                    cached_tokens: last["cachedInputTokens"].as_u64(),
                                    cache_write_tokens: None,
                                };
                                call_started = std::time::Instant::now();
                                used.prompt_tokens += round.prompt_tokens;
                                used.completion_tokens += round.completion_tokens;
                                used.total_tokens += round.prompt_tokens + round.completion_tokens;
                                RuntimeEvent::Usage(Box::new(round))
                            }
                            Output::RateLimits(limits) => {
                                crate::codex::observe_limits(&limits, coder_delegate::limit::now());
                                return;
                            }
                            Output::Gated(_) => return,
                        };
                        let _ = events.send(Event::Runtime { turn, event });
                    };
                    let running =
                        codex.turn(&text, Some(&running_model), Some(&mut controls), &mut emit);
                    tokio::pin!(running);
                    loop {
                        tokio::select! {
                            result = &mut running => break result,
                            command = commands.recv() => match command {
                                // A message sent while the turn runs is
                                // read in it.
                                Some(Command::Say { text, next: true }) => {
                                    let _ = control.send(Control::Steer(text));
                                }
                                Some(Command::Say { text, next: false }) => waiting.push_back(text),
                                Some(Command::Model(next)) => model = next,
                                Some(Command::Close) | None => closing = true,
                            },
                        }
                    }
                };
                let result = match result {
                    Ok(ended) if ended.interrupted() => Err(STOPPED.to_owned()),
                    Ok(ended) if ended.status == "failed" => Err(ended
                        .error
                        .unwrap_or_else(|| "The Codex turn failed.".into())),
                    Ok(ended) => Ok(Streamed {
                        text: ended.text,
                        model: ended.model.unwrap_or_else(|| running_model.clone()),
                        finish_reason: Some("stop".into()),
                        usage: used,
                        first_text_ms: None,
                        milliseconds: 0,
                    }),
                    Err(error) => Err(error),
                };
                let gone = result.as_ref().is_err_and(|error| {
                    error.starts_with("Codex stopped") || error.starts_with("Codex's output")
                });
                let _ = events.send(Event::TurnEnd { turn, result });
                {
                    let mut current = lock(&reading);
                    current.turn += 1;
                    current.active = false;
                    current.control = None;
                }
                if gone {
                    return;
                }
            }
        };
        tokio::select! {
            () = work => {}
            () = forward => {}
        }
        stop.store(true, Ordering::Relaxed);
        codex.close().await;
    }
}

#[cfg(test)]
#[path = "codex_chat_tests.rs"]
mod tests;
