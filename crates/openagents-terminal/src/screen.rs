//! The input loop: it holds the terminal, draws a frame after every key,
//! event, and tick, and runs each [`Action`] against the client.
//!
//! The client runs one operation at a time ([`Client::stream`] takes it and
//! hands it back when the operation ends). While a Coder run is followed,
//! an action that needs the client (opening the thread list, sending a
//! message) first stops following, which leaves the run going, and runs
//! once the client is back; the run is followed again after. While a reply
//! streams, a send waits and says so.

use std::collections::VecDeque;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use coder_terminal::components::card::Card;
use coder_terminal::{Guard, Ladder};
use crossterm::event::{Event as TermEvent, EventStream, KeyEventKind};
use futures_util::StreamExt;
use openagents_chat::client::{self, Client, Ended, Error, Event, Op, Stopper, Stream};
use openagents_chat::router::Context;
use openagents_chat::service::Command;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::app::{Action, App, Overlay, Phase};
use crate::picker::Picker;
use crate::rows::Row;
use crate::{Exit, Extras, Interrupter, Invite, Launch, Resume, last, prompts};

/// How long the welcome card waits for this computer's Coder readiness.
const CONTEXT_WAIT: Duration = Duration::from_secs(5);
/// How often a pairing code checks for the phone that scanned it.
const PAIR_POLL: Duration = Duration::from_secs(1);
/// The spinner's clock: Grok Build's 30 fps animation tick, each frame
/// held four of them ([`coder_terminal::grok_spinner`]).
const TICK: Duration = coder_terminal::grok_spinner::TICK;
/// How often the screen looks for a background rule's notification.
const NOTICE_POLL: Duration = Duration::from_secs(30);
/// The most threads the picker lists.
const THREADS_MAX: usize = 200;

/// What a piece of work off the loop sends back.
enum Done {
    Rail(
        String,
        crate::rail::Refresh,
        Vec<(String, client::Progress)>,
    ),
    Stopped(Result<String, String>),
    Invite(Result<Invite, String>),
    Paired(Result<Option<String>, String>),
    Sync(Result<String, String>),
    Plugins(Result<Vec<crate::Plugin>, String>),
    Plugin(String, Result<String, String>),
    Turned(String, Result<String, String>),
    Imported(Result<String, String>),
    Efficiency(Result<crate::Efficiency, String>),
    Settings(Result<crate::Settings, String>),
    Background(Result<Vec<crate::BackgroundRow>, String>),
    BackgroundCard(String, crate::BackgroundAct, Result<Vec<String>, String>),
    Notice(Option<(u64, String)>),
    Steered(Result<client::Steering, String>),
    File(Result<Option<crate::view::FileView>, String>),
}

/// The screen with its client and the work it has started.
struct Screen {
    app: App,
    client: Option<Client>,
    events: Option<mpsc::UnboundedReceiver<Event>>,
    running: Option<JoinHandle<(Client, Result<Ended, Error>)>>,
    stopper: Option<Stopper>,
    queue: VecDeque<Action>,
    interrupter: Interrupter,
    extras: Arc<dyn Extras>,
    done: mpsc::UnboundedSender<Done>,
    invite: Option<Invite>,
    polling: bool,
    folder: Option<PathBuf>,
    home: PathBuf,
    version: String,
    rail_refresh: Option<(String, crate::rail::Refresh)>,
    rail_polled: Option<std::time::Instant>,
    quit: bool,
}

pub(crate) async fn run(launch: Launch) -> io::Result<Exit> {
    let ladder = Ladder::from_environment();
    let (screen, done) = prepare(launch, ladder).await;
    // Mouse reports on: the screen selects and copies text itself, and a
    // click on a file's path opens it.
    let guard = Guard::full_screen_with_mouse()?;
    guard.arm_panic_hook();
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let result = drive(screen, done, &mut terminal).await;
    let restored = guard.restore();
    let exit = result?;
    restored?;
    Ok(exit)
}

/// The screen before its first frame: the thread chosen, the welcome card,
/// and the resumed thread's turns.
async fn prepare(launch: Launch, ladder: Ladder) -> (Screen, mpsc::UnboundedReceiver<Done>) {
    let Launch {
        mut client,
        coder,
        interrupter,
        extras,
        resume,
        folder,
        home,
        notices,
        version,
    } = launch;
    let mut find = None;
    let (thread, fresh) = match resume {
        Resume::Thread(id) => (id, false),
        Resume::New(Some(id)) => (id, true),
        Resume::New(None) => (client::new_id(), true),
        Resume::LastForFolder => {
            let last = folder
                .as_deref()
                .and_then(|folder| last::read(&home, folder));
            match last {
                Some(id) if client.collect(&id).await.is_ok() => (id, false),
                _ => (client::new_id(), true),
            }
        }
        Resume::Find(arg) => match found(&mut client, &arg).await {
            Some(id) => (id, false),
            None => {
                find = Some(arg);
                (client::new_id(), true)
            }
        },
    };
    let store = client.store(&thread);
    let context = {
        let (coder, folder) = (coder.clone(), folder.clone());
        let read = tokio::task::spawn_blocking(move || coder.context(&store, folder.as_deref()));
        match tokio::time::timeout(CONTEXT_WAIT, read).await {
            Ok(Ok(context)) => context,
            _ => Context::default(),
        }
    };
    let label = folder.as_deref().and_then(Path::file_name).map_or_else(
        || "no folder".to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    let resumed = if fresh {
        None
    } else {
        Some(match client.collect(&thread).await {
            Ok(whole) if !whole.summary.title.trim().is_empty() => whole.summary.title,
            _ => thread.clone(),
        })
    };
    let mut app = App::new(ladder, thread.clone(), fresh, client.kind(), label);
    if client.kind() == client::Kind::Computer {
        app.computer = Some(client.place());
    }
    app.editor.set_history(prompts::read(&home));
    app.welcome(&context, resumed.as_deref(), &extras.watchers());
    for notice in notices {
        app.note(notice);
    }
    let (done, receiver) = mpsc::unbounded_channel();
    let mut screen = Screen {
        app,
        client: Some(client),
        events: None,
        running: None,
        stopper: None,
        queue: VecDeque::new(),
        interrupter,
        extras,
        done,
        invite: None,
        polling: false,
        folder,
        home,
        version,
        rail_refresh: Some((thread.clone(), crate::rail::Refresh::default())),
        rail_polled: None,
        quit: false,
    };
    if !fresh {
        screen.open(thread, false).await;
    } else if let Some(arg) = find {
        screen.threads(None, &arg).await;
    } else if screen.app.computer.is_some() {
        // Another computer's threads start there: open one of them.
        screen.threads(None, "").await;
    }
    (screen, receiver)
}

async fn drive(
    mut screen: Screen,
    mut done: mpsc::UnboundedReceiver<Done>,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> io::Result<Exit> {
    let mut keys = EventStream::new();
    let mut tick = tokio::time::interval(TICK);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut poll = tokio::time::interval(PAIR_POLL);
    // A tick redraws only when a spinner is on the screen and turns to its
    // next frame (its timer moves with it); everything else redraws at
    // once.
    let mut dirty = true;
    let mut notices = tokio::time::interval(NOTICE_POLL);
    loop {
        if dirty {
            terminal.draw(|frame| {
                let area = frame.area();
                let caret = crate::draw::draw(&mut screen.app, area, frame.buffer_mut());
                frame.set_cursor_position(caret);
            })?;
        }
        dirty = true;
        if screen.quit {
            break;
        }
        tokio::select! {
            key = keys.next() => match key {
                Some(Ok(TermEvent::Key(key))) if key.kind == KeyEventKind::Press => {
                    let width = terminal.size()?.width;
                    let actions = screen.app.key(&key, width);
                    if let Some(prompt) = screen.app.take_sent() {
                        prompts::remember(&screen.home, &prompt);
                    }
                    for action in actions {
                        screen.act(action).await;
                    }
                }
                Some(Ok(TermEvent::Mouse(mouse))) => {
                    for action in screen.app.mouse(&mouse) {
                        screen.act(action).await;
                    }
                }
                Some(Ok(_)) => {}
                Some(Err(error)) => return Err(error),
                None => screen.quit().await,
            },
            event = next(&mut screen.events) => match event {
                Some(event) => screen.app.event(event),
                None => screen.finish().await,
            },
            Some(result) = done.recv() => {
                screen.done(result);
                // A message that started the run's next turn: follow it
                // when nothing else has the client.
                if screen.client.is_some() && screen.app.wants_follow() {
                    screen.start(Op::Follow {
                        thread: screen.app.thread.clone(),
                    });
                }
            }
            _ = tick.tick() => {
                screen.app.tick = screen.app.tick.wrapping_add(1);
                screen.refresh_rail();
                dirty = screen.app.animating()
                    && coder_terminal::grok_spinner::turns(screen.app.tick);
            }
            _ = poll.tick(), if screen.invite.is_some() && !screen.polling => screen.poll(),
            _ = notices.tick() => {
                // Nothing changes until the answer arrives.
                dirty = false;
                let (extras, done) = (screen.extras.clone(), screen.done.clone());
                tokio::task::spawn_blocking(move || {
                    let _ = done.send(Done::Notice(extras.background_notice()));
                });
            }
        }
    }
    Ok(Exit {
        thread: (!screen.app.fresh).then(|| screen.app.thread.clone()),
        running: screen.app.running,
    })
}

/// The next event of the running operation; `None` once it ended, and
/// never while none runs.
async fn next(events: &mut Option<mpsc::UnboundedReceiver<Event>>) -> Option<Event> {
    match events {
        Some(receiver) => receiver.recv().await,
        None => std::future::pending().await,
    }
}

impl Screen {
    /// Read all listed runs without borrowing the client that follows one.
    fn refresh_rail(&mut self) {
        if self.app.delegations.is_empty()
            || self
                .rail_polled
                .is_some_and(|at| at.elapsed() < Duration::from_secs(1))
        {
            return;
        }
        let reader = self
            .client
            .as_ref()
            .map(|client| client.stopper(&self.app.thread))
            .or_else(|| self.stopper.clone());
        let Some(reader) = reader else {
            return;
        };
        let Some((thread, mut refresh)) = self.rail_refresh.take() else {
            return;
        };
        self.rail_polled = Some(std::time::Instant::now());
        let current = self.app.thread.clone();
        if thread != current {
            refresh = crate::rail::Refresh::default();
        }
        let tasks = self
            .app
            .delegations
            .iter()
            .map(|held| held.task.clone())
            .collect::<Vec<_>>();
        let done = self.done.clone();
        tokio::task::spawn_blocking(move || {
            let states = refresh.poll(&tasks, |task| reader.follow(task, &current));
            let _ = done.send(Done::Rail(current, refresh, states));
        });
    }

    /// Run one action.
    async fn act(&mut self, action: Action) {
        match action {
            Action::Quit => self.quit().await,
            Action::Interrupt => self.interrupter.fire(),
            Action::Copy(text) => {
                if crate::copy::to_clipboard(&text).is_err() {
                    self.app.loud("Could not copy.");
                }
            }
            Action::Steer { task, text } => {
                let stopper = match &self.client {
                    Some(client) => client.stopper(&self.app.thread),
                    None => match self.stopper.clone() {
                        Some(stopper) => stopper,
                        None => return,
                    },
                };
                let done = self.done.clone();
                tokio::task::spawn_blocking(move || {
                    let _ = done.send(Done::Steered(stopper.steer(&task, &text)));
                });
            }
            Action::OpenFile { path, line } => {
                let bases = self.app.bases(self.folder.as_deref());
                let (ladder, done) = (self.app.ladder, self.done.clone());
                tokio::task::spawn_blocking(move || {
                    let opened = crate::view::open(&path, line, &bases, ladder);
                    let _ = done.send(Done::File(opened));
                });
            }
            Action::StopRun { task } => {
                let Some(stopper) = self.stopper.clone() else {
                    return;
                };
                self.app.note("Stopping Coder…");
                let done = self.done.clone();
                tokio::task::spawn_blocking(move || {
                    let _ = done.send(Done::Stopped(stopper.stop(&task)));
                });
            }
            Action::Connect => {
                if self.invite.is_some() {
                    self.app
                        .note("The pairing code is on the screen above; Esc cancels it.");
                    return;
                }
                self.app
                    .note("Asking this computer's host for a pairing code…");
                let (extras, done) = (self.extras.clone(), self.done.clone());
                tokio::task::spawn_blocking(move || {
                    let _ = done.send(Done::Invite(extras.invite()));
                });
            }
            Action::CancelInvite => {
                if let Some(invite) = self.invite.take() {
                    self.app.pairing = false;
                    let extras = self.extras.clone();
                    tokio::task::spawn_blocking(move || extras.cancel(&invite));
                    self.app.note("Cancelled the pairing code.");
                }
            }
            Action::Sync => {
                if self.app.backend == client::Kind::Computer {
                    self.app
                        .note("These are another computer's threads; they sync through its host.");
                    return;
                }
                if self.app.backend == client::Kind::Host {
                    self.app.note(
                        "This computer's host already holds these threads; they sync with \
                         the desktop app and your phone.",
                    );
                    return;
                }
                self.app
                    .note("Installing this computer's host as a service…");
                let (extras, done) = (self.extras.clone(), self.done.clone());
                tokio::task::spawn_blocking(move || {
                    let _ = done.send(Done::Sync(extras.sync()));
                });
            }
            Action::Plugins => {
                self.app.note("Reading the plugins…");
                let (extras, done) = (self.extras.clone(), self.done.clone());
                tokio::task::spawn_blocking(move || {
                    let _ = done.send(Done::Plugins(extras.plugins()));
                });
            }
            Action::TurnPlugin { id, name, on } => {
                let (extras, done) = (self.extras.clone(), self.done.clone());
                tokio::task::spawn_blocking(move || {
                    let _ = done.send(Done::Turned(name, extras.turn_plugin(&id, on)));
                });
            }
            Action::InstallPlugin { id, name } => {
                let (extras, done) = (self.extras.clone(), self.done.clone());
                tokio::task::spawn_blocking(move || {
                    let _ = done.send(Done::Turned(name, extras.install_plugin(&id)));
                });
            }
            Action::RunPlugin { key, name, request } => {
                self.app.note(format!("Running {name}…"));
                let (extras, done, folder) =
                    (self.extras.clone(), self.done.clone(), self.folder.clone());
                tokio::task::spawn_blocking(move || {
                    let ran = extras.run_plugin(&key, &request, folder.as_deref());
                    let _ = done.send(Done::Plugin(name, ran));
                });
            }
            Action::Background => {
                let (extras, done) = (self.extras.clone(), self.done.clone());
                tokio::task::spawn_blocking(move || {
                    let _ = done.send(Done::Background(extras.background()));
                });
            }
            Action::BackgroundAct { id, act } => {
                if matches!(
                    act,
                    crate::BackgroundAct::DryRun | crate::BackgroundAct::Run
                ) {
                    self.app.note(format!("Running {id}…"));
                }
                let (extras, done) = (self.extras.clone(), self.done.clone());
                tokio::task::spawn_blocking(move || {
                    let result = extras.background_act(&id, act);
                    let _ = done.send(Done::BackgroundCard(id, act, result));
                });
            }
            Action::Import => {
                self.app
                    .note("Copying Claude Code and Codex sessions into this computer's threads…");
                let (extras, done) = (self.extras.clone(), self.done.clone());
                tokio::task::spawn_blocking(move || {
                    let _ = done.send(Done::Imported(extras.import()));
                });
            }
            Action::Efficiency => {
                let (extras, done) = (self.extras.clone(), self.done.clone());
                tokio::task::spawn_blocking(move || {
                    let _ = done.send(Done::Efficiency(extras.efficiency()));
                });
            }
            Action::Settings => {
                let (extras, done) = (self.extras.clone(), self.done.clone());
                tokio::task::spawn_blocking(move || {
                    let _ = done.send(Done::Settings(Ok(extras.settings())));
                });
            }
            Action::Change { key, on } => {
                let (extras, done) = (self.extras.clone(), self.done.clone());
                tokio::task::spawn_blocking(move || {
                    let _ = done.send(Done::Settings(extras.change(&key, on)));
                });
            }
            Action::Secret { key, value } => {
                let (extras, done) = (self.extras.clone(), self.done.clone());
                tokio::task::spawn_blocking(move || {
                    let _ = done.send(Done::Settings(extras.set_secret(&key, &value)));
                });
            }
            // Everything below needs the client.
            action => {
                if self.client.is_none() {
                    self.wait(action);
                    return;
                }
                self.with_client(action).await;
            }
        }
    }

    /// An action that needs the client while an operation has it: stop
    /// following a run (it keeps going) and run the action after, or say
    /// to wait for a reply.
    fn wait(&mut self, action: Action) {
        if self.app.phase == Phase::Following {
            self.app.quiet_detach = true;
            self.interrupter.fire();
            self.queue.push_back(action);
        } else if let Action::Run(Op::Send { text, .. }) = action {
            self.app.note(
                "Wait for this reply, or press Esc to stop it. Your message is back in the box.",
            );
            self.app.editor.insert_str(&text);
        } else {
            self.app
                .note("Wait for this reply, or press Esc to stop it.");
        }
    }

    async fn with_client(&mut self, action: Action) {
        match action {
            Action::Run(op) => self.start(op),
            Action::Open(id) => self.open(id, true).await,
            Action::New => {
                let id = client::new_id();
                self.app.switch(id, true);
                self.app.note("New thread. Type a message to start it.");
            }
            Action::Threads => self.threads(None, "").await,
            Action::Resume(arg) => {
                let Some(client) = self.client.as_mut() else {
                    return;
                };
                match found(client, &arg).await {
                    Some(id) => self.open(id, true).await,
                    None => self.threads(None, &arg).await,
                }
            }
            Action::Archive(id) => {
                let result = self.apply(Command::Archive { chat: id }).await;
                let before = match self.app.overlay.take() {
                    Some(Overlay::Threads(picker)) => Some(picker),
                    _ => None,
                };
                if let Err(message) = result {
                    self.app.loud(message);
                }
                self.threads(before, "").await;
            }
            Action::Export => self.export().await,
            _ => {}
        }
    }

    async fn apply(&mut self, command: Command) -> Result<(), String> {
        let Some(client) = self.client.as_mut() else {
            return Err("busy".into());
        };
        client.apply(command).await.map(|_| ())
    }

    /// Start `op` on its own task, its events into the loop.
    fn start(&mut self, op: Op) {
        let Some(client) = self.client.take() else {
            return;
        };
        if let Op::Send { new: true, .. } = &op
            && let Some(folder) = &self.folder
        {
            last::remember(&self.home, folder, &self.app.thread, now());
        }
        self.stopper = Some(client.stopper(&self.app.thread));
        self.app.began(&op);
        let Stream { events, done } = client.stream(op);
        self.events = Some(events);
        self.running = Some(done);
    }

    /// The operation's events ended: take the client back, then run what
    /// waited, or follow the run again.
    async fn finish(&mut self) {
        self.events = None;
        let Some(handle) = self.running.take() else {
            return;
        };
        let failed = match handle.await {
            Ok((client, ended)) => {
                self.client = Some(client);
                ended.err().map(|error| error.message().to_owned())
            }
            Err(_) => {
                self.app.loud("The operation stopped unexpectedly.");
                self.quit = true;
                return;
            }
        };
        self.app.ended(failed);
        if self.quit {
            return;
        }
        if let Some(action) = self.queue.pop_front() {
            self.act(action).await;
            return;
        }
        if self.app.wants_follow() {
            self.start(Op::Follow {
                thread: self.app.thread.clone(),
            });
        }
    }

    /// Open thread `id`: its turns, then its Coder run, followed from its
    /// first event.
    async fn open(&mut self, id: String, switch: bool) {
        let Some(client) = self.client.as_mut() else {
            return;
        };
        let whole = match client.collect(&id).await {
            Ok(whole) => whole,
            Err(error) => {
                self.app
                    .loud(format!("Cannot open that thread: {}", error.message()));
                return;
            }
        };
        if switch {
            self.app.switch(id.clone(), false);
            self.app.note(if whole.summary.title.trim().is_empty() {
                "Opened a thread.".to_owned()
            } else {
                format!("Opened \"{}\".", whole.summary.title)
            });
        }
        self.app.show_turns(&whole.turns);
        if let Some(folder) = &self.folder {
            last::remember(&self.home, folder, &id, now());
        }
        if let Some(coder) = &whole.summary.coder {
            self.app.task = Some(coder.task.clone());
            self.start(Op::Follow { thread: id });
        }
    }

    /// Show the thread picker narrowed to `query`, or as `before` was
    /// (its query, its place, its open rows) over the threads read again.
    async fn threads(&mut self, before: Option<Picker>, query: &str) {
        let Some(client) = self.client.as_mut() else {
            return;
        };
        match client.threads(true, THREADS_MAX).await {
            Ok((rows, _)) => {
                let fresh = Picker::new(rows, &self.app.folder);
                let picker = match before {
                    Some(before) => {
                        let len = fresh.entries().len();
                        Picker {
                            selected: before.selected.min(len.saturating_sub(1)),
                            query: before.query,
                            search: before.search,
                            hidden: before.hidden,
                            expanded: before.expanded,
                            ..fresh
                        }
                    }
                    None if query.is_empty() => fresh,
                    None => fresh.with_query(query),
                };
                self.app.overlay = Some(Overlay::Threads(picker));
            }
            Err(error) => self.app.loud(error.message().to_owned()),
        }
    }

    /// Save the thread as an ATIF trajectory in the chat home's `exports`.
    async fn export(&mut self) {
        if self.app.fresh {
            self.app.note("This thread has no messages to export yet.");
            return;
        }
        let Some(client) = self.client.as_mut() else {
            return;
        };
        let id = self.app.thread.clone();
        let whole = match client.collect(&id).await {
            Ok(whole) => whole,
            Err(error) => {
                self.app.loud(error.message().to_owned());
                return;
            }
        };
        let tasks = client.trajectories(&id, &whole);
        let document = openagents_chat::thread::trajectory_with(&whole, &self.version, tasks);
        if let Some(problem) = atif::validate(&document).into_iter().next() {
            self.app
                .loud(format!("The trajectory is not valid ATIF: {problem}"));
            return;
        }
        let path = self.home.join("exports").join(format!("{id}.json"));
        let written = std::fs::create_dir_all(self.home.join("exports")).and_then(|()| {
            std::fs::write(
                &path,
                serde_json::to_vec_pretty(&document).unwrap_or_default(),
            )
        });
        match written {
            Ok(()) => self.app.note(format!(
                "Saved this thread as an ATIF trajectory: {}",
                path.display()
            )),
            Err(error) => self
                .app
                .loud(format!("Cannot write {}: {error}", path.display())),
        }
    }

    /// Ask the host whether a phone redeemed the code.
    fn poll(&mut self) {
        let Some(invite) = self.invite.clone() else {
            return;
        };
        if now() >= invite.expires_at {
            self.invite = None;
            self.app.pairing = false;
            let extras = self.extras.clone();
            tokio::task::spawn_blocking(move || extras.cancel(&invite));
            self.app
                .note("The pairing code expired. /connect shows a new one.");
            return;
        }
        self.polling = true;
        let (extras, done) = (self.extras.clone(), self.done.clone());
        tokio::task::spawn_blocking(move || {
            let _ = done.send(Done::Paired(extras.paired(&invite)));
        });
    }

    fn done(&mut self, result: Done) {
        match result {
            Done::Rail(thread, refresh, states) => {
                if thread == self.app.thread {
                    self.app.refresh_rail(states);
                }
                self.rail_refresh = Some((thread, refresh));
            }
            Done::Steered(result) => self.app.steered(result),
            Done::File(Ok(Some(file))) => self.app.show_file(file),
            // A click on a word that names no file here does nothing.
            Done::File(Ok(None)) => {}
            Done::File(Err(why)) => self.app.loud(why),
            Done::Stopped(Ok(message)) => self.app.note(message),
            Done::Stopped(Err(why)) => self.app.loud(why),
            Done::Invite(Ok(invite)) => {
                let expires = clock(invite.expires_at);
                self.app.push(Row::Card(Card {
                    title: "Connect a phone".into(),
                    rows: Vec::new(),
                    body: vec![
                        "Scan with the OpenAgents app on your phone: Connect a computer.".into(),
                        format!(
                            "The code works once, until {expires}. Esc cancels it. The phone gets \
                             full access to this computer."
                        ),
                    ],
                    art: invite.qr.clone(),
                    keys: vec![("Esc".into(), "cancel the code".into())],
                }));
                self.app.pairing = true;
                self.invite = Some(invite);
            }
            Done::Invite(Err(why)) => self.app.loud(why),
            Done::Paired(result) => {
                self.polling = false;
                match result {
                    Ok(Some(_)) => {
                        if let Some(invite) = self.invite.take() {
                            let extras = self.extras.clone();
                            tokio::task::spawn_blocking(move || extras.cancel(&invite));
                        }
                        self.app.pairing = false;
                        self.app
                            .note("Connected. The phone can reach this computer now.");
                    }
                    Ok(None) => {}
                    Err(why) => self.app.loud(why),
                }
            }
            Done::Sync(Ok(message)) => self.app.note(message),
            Done::Sync(Err(why)) => self.app.loud(why),
            Done::Plugins(Ok(rows)) => {
                self.app.overlay = Some(Overlay::Plugins { rows, selected: 0 });
            }
            Done::Plugins(Err(why)) => self.app.loud(why),
            Done::Turned(_, Ok(words)) => self.app.note(words),
            Done::Turned(name, Err(why)) => self.app.loud(format!("{name}: {why}")),
            Done::Plugin(name, Ok(reply)) => self.app.push(Row::Card(Card {
                title: format!("Plugin {name}"),
                rows: Vec::new(),
                body: if reply.trim().is_empty() {
                    vec!["It finished with nothing to say.".into()]
                } else {
                    reply.lines().map(str::to_owned).collect()
                },
                art: Vec::new(),
                keys: Vec::new(),
            })),
            Done::Plugin(name, Err(why)) => self.app.loud(format!("{name} did not run: {why}")),
            Done::Settings(Ok(settings)) => {
                if !matches!(self.app.overlay, Some(Overlay::Settings { .. })) {
                    self.app
                        .note(format!("Settings file: {}", settings.path.display()));
                }
                self.app.settings(settings);
            }
            Done::Settings(Err(why)) => self.app.loud(why),
            Done::Background(Ok(rows)) => {
                self.app.overlay = Some(Overlay::Background { rows, selected: 0 });
            }
            Done::Background(Err(why)) => self.app.loud(why),
            Done::BackgroundCard(id, act, Ok(lines)) => {
                let what = match act {
                    crate::BackgroundAct::Show => "rule",
                    crate::BackgroundAct::DryRun => "dry run",
                    crate::BackgroundAct::Run => "run",
                    crate::BackgroundAct::Pause | crate::BackgroundAct::Resume => "state",
                    crate::BackgroundAct::Log => "log",
                };
                self.app.push(Row::Card(Card {
                    title: format!("Background {id} · {what}"),
                    rows: Vec::new(),
                    body: lines,
                    art: Vec::new(),
                    keys: Vec::new(),
                }));
                if act == crate::BackgroundAct::DryRun {
                    self.app.background_armed = Some(id);
                    self.app
                        .note("Press r on it in /background again to run it.");
                }
            }
            Done::BackgroundCard(id, _, Err(why)) => self.app.loud(format!("{id}: {why}")),
            Done::Notice(Some((at, line))) if at > self.app.notice_seen => {
                self.app.notice_seen = at;
                self.app.notice(line);
            }
            Done::Notice(_) => {}
            Done::Imported(Ok(message)) => self.app.note(message),
            Done::Imported(Err(why)) => self.app.loud(why),
            Done::Efficiency(Ok(report)) => self.app.push(Row::Card(Card {
                title: report.title,
                rows: report.rows,
                body: report.body,
                art: Vec::new(),
                keys: Vec::new(),
            })),
            Done::Efficiency(Err(why)) => self.app.loud(why),
        }
    }

    /// Close: stop receiving a reply, stop following a run (it keeps
    /// going), and cancel a pairing code.
    async fn quit(&mut self) {
        self.quit = true;
        if self.client.is_none() {
            self.app.quiet_detach = true;
            self.interrupter.fire();
        }
        if let Some(invite) = self.invite.take() {
            let extras = self.extras.clone();
            let _ = tokio::task::spawn_blocking(move || extras.cancel(&invite)).await;
        }
        if let Some(handle) = self.running.take() {
            let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
        }
    }
}

/// The thread `arg` names (`/resume ARG`, `--resume ARG`): a whole ID
/// that opens, else one the picker's rules find among the threads.
async fn found(client: &mut Client, arg: &str) -> Option<String> {
    let arg = arg.trim();
    if client::thread_id(arg) && client.collect(arg).await.is_ok() {
        return Some(arg.to_owned());
    }
    let (rows, _) = client.threads(true, THREADS_MAX).await.ok()?;
    crate::picker::resolve(&rows, arg).map(|row| row.id.clone())
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// `HH:MM` UTC of a Unix time, for a code's expiry.
fn clock(at: u64) -> String {
    let day = at % 86_400;
    format!("{:02}:{:02} UTC", day / 3_600, (day % 3_600) / 60)
}
