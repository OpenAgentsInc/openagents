//! What the chat client ([`openagents_chat::client`]) needs from this
//! computer, for `openagents chat` and OpenAgents Terminal: Coder runs
//! here ([`Here`], over [`super::local`] and [`super::issue_run`]) and this
//! computer's host over its control socket ([`Control`]).
//!
//! The client lives in `openagents-chat`, which this crate and the control
//! protocol depend on, so it reaches both through these two
//! implementations rather than linking them itself.

use std::path::{Path, PathBuf};

use openagents_chat::client::{
    BoxFuture, Coder, Dial, Follow, Host, Issue, IssueStarted, Migration, Progress, Started,
};
use openagents_chat::coder_events::{Line, Runner};
use openagents_chat::router::{Caller, CoderRun, Context};
use openagents_chat::service::{Command, Snapshot};
use openagents_connect::control::{self, Op, Reply, Request};
use serde_json::Value;
#[cfg(unix)]
use tokio::net::UnixStream as Stream;
// Windows has no host control socket ([`control::socket_path`] answers
// `None` there), so nothing dials; the type only keeps the code one shape.
#[cfg(not(unix))]
use tokio::net::TcpStream as Stream;

use super::local::{self, Local, State};

/// Coder on this computer, with the person's settings
/// ([`super::settings`]): Codex, Claude Code, or Grok Build, the first
/// signed in here with capacity, in a worktree of the checkout the client
/// runs in. Every step is approved (#10104): the settings' default access
/// is `full`, and nothing here prompts.
pub struct Here;

impl Here {
    fn runner(store: &Path) -> Local {
        Local::here(store.to_path_buf())
    }
}

impl Coder for Here {
    fn default_store(&self) -> PathBuf {
        local::default_store()
    }

    /// What a turn tells the chat worker about this computer (#10077):
    /// whether a run could start (the folder is a checkout that counts as
    /// a project in the settings, and an allowed provider is signed in with
    /// capacity), that this computer is where Coder runs with every coding
    /// agent installed or signed in here and its state ([`Local::engines`],
    /// #10113), and the project folder. Reads only.
    fn context(&self, store: &Path, dir: Option<&Path>) -> Context {
        use openagents_chat::router::{Computer, Project};
        let run = Self::runner(store);
        let engines = run.engines();
        let ready = dir.is_some_and(|dir| run.project(dir).is_ok()) && run.ready();
        let project = dir
            .and_then(|dir| local::checkout(dir).ok())
            .and_then(|checkout| Project::at(&checkout.top.display().to_string()));
        Context {
            computer_ready: ready,
            computer: Some(Computer::Here {
                name: None,
                engines,
            }),
            project,
            ..Context::default()
        }
    }

    fn predict(
        &self,
        store: &Path,
        engine: Option<nostr::cj_conversation::Engine>,
    ) -> Option<Runner> {
        Self::runner(store).predict(engine.map(super::settings::provider_of))
    }

    /// The settings' `coder.start` is `ask_first`, or they cannot be read
    /// (the refusal then shows when the person accepts).
    fn asks_first(&self) -> bool {
        Local::here(PathBuf::new()).asks_first()
    }

    fn checkout(&self, dir: &Path) -> Result<(), String> {
        local::checkout(dir).map(|_| ())
    }

    /// The project's spare worktree, made in the background (#10115), so
    /// the first start here takes it instead of waiting on Git.
    fn warm(&self, store: &Path, dir: &Path) {
        let (store, dir) = (store.to_path_buf(), dir.to_path_buf());
        let _ = std::thread::Builder::new()
            .name("coder-warm".into())
            .spawn(move || Self::runner(&store).warm(&dir));
    }

    fn start(
        &self,
        store: &Path,
        dir: &Path,
        title: &str,
        prompt: &str,
        chat: &str,
        requested: Option<nostr::cj_conversation::Engine>,
    ) -> Result<Started, String> {
        let record = Self::runner(store).start_requested(
            dir,
            title,
            prompt,
            Some(chat),
            &[],
            requested.map(super::settings::provider_of),
        )?;
        Ok(Started {
            task: record.task,
            project: record.project,
            worktree: record.worktree,
        })
    }

    fn issue(&self, request: &str, earlier: &str, dir: &Path) -> Option<Box<dyn Issue>> {
        super::issue_run::asked_blocking(request, earlier, dir)
            .map(|reference| Box::new(IssueRef(reference)) as Box<dyn Issue>)
    }

    fn follow(
        &self,
        store: &Path,
        task: &str,
        chat: &str,
        hint: Option<String>,
    ) -> Box<dyn Follow> {
        Box::new(Following(Self::runner(store).follow(
            task,
            Some(chat),
            hint,
        )))
    }

    fn stop(&self, store: &Path, task: &str) -> Result<(), String> {
        Self::runner(store).stop(task)
    }

    fn answer(&self, store: &Path, task: &str, text: &str) -> Result<usize, String> {
        let record = Self::runner(store).answer(task, text)?;
        Ok(record.turns.last().map_or(1, |start| start.turn))
    }

    fn result(&self, store: &Path, task: &str) -> Option<CoderRun> {
        local::result_in(Some(store), task)
    }

    fn trajectories(&self, store: &Path, task: &str) -> Vec<Value> {
        let Ok(task) = super::Store::open(store).and_then(|tasks| tasks.show(task)) else {
            return Vec::new();
        };
        task.earlier
            .iter()
            .chain(task.run.iter())
            .filter_map(|run| atif::log::read(&store.join(&run.admission.trace_file)).ok())
            .map(|recording| recording.document())
            .filter(|document| atif::validate(document).is_empty())
            .collect()
    }
}

struct IssueRef(super::issue_run::Reference);

impl Issue for IssueRef {
    fn number(&self) -> u64 {
        self.0.number
    }

    fn begin(
        self: Box<Self>,
        store: &Path,
        dir: &Path,
        chat: &str,
    ) -> Result<IssueStarted, String> {
        let runner = super::issue_run::Runner::new(store.to_path_buf());
        let started = runner
            .begin(dir, &self.0, Some(chat))
            .map_err(|refused| refused.to_string())?;
        let record = &started.record;
        Ok(IssueStarted {
            started: Started {
                task: record.task.clone(),
                project: record.project.clone(),
                worktree: record.worktree.clone(),
            },
            url: started.issue.url.clone(),
            // The flow goes to a process of its own, so it survives the
            // screen or shell that started it; worked here only when no
            // engine can take it.
            finish: Box::new(move || {
                let _ = started.hand_off(local::controller().ok().as_deref());
            }),
        })
    }
}

struct Following(local::Follow);

impl Follow for Following {
    fn poll(&mut self) -> Result<(Vec<Line>, Progress), String> {
        let (lines, state) = self.0.poll()?;
        Ok((
            lines,
            match state {
                State::Running => Progress::Running,
                State::Waiting => Progress::Waiting,
                State::Ended => Progress::Ended,
            },
        ))
    }
}

/// This computer's host, over its control socket ([`control::socket_path`]).
pub struct Control;

#[cfg(unix)]
async fn connect(socket: &Path) -> std::io::Result<Stream> {
    Stream::connect(socket).await
}

#[cfg(not(unix))]
async fn connect(_socket: &Path) -> std::io::Result<Stream> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "no host control socket on this platform",
    ))
}

impl Dial for Control {
    fn socket(&self) -> Option<PathBuf> {
        control::socket_path()
    }

    fn dial<'a>(&'a self, socket: &'a Path) -> BoxFuture<'a, Option<Box<dyn Host>>> {
        Box::pin(async move {
            let stream = connect(socket).await.ok()?;
            Some(Box::new(ControlHost {
                stream,
                next: 1,
                socket: socket.to_path_buf(),
                older: false,
                broken: false,
            }) as Box<dyn Host>)
        })
    }
}

/// One connection to the host's control socket.
struct ControlHost {
    stream: Stream,
    next: u64,
    socket: PathBuf,
    /// The host predates the `chat` operation's `caller` (#10108): it
    /// refused the field as `malformed`, so turns go without it and the
    /// host hears them as the desktop's.
    older: bool,
    /// The last call did not get an answer: the host restarted, or the
    /// connection broke. The next call opens a new connection first, so a
    /// client reading a streaming reply picks it up again once the host is
    /// back (#10151).
    broken: bool,
}

impl ControlHost {
    async fn call(&mut self, op: Op) -> openagents_connect::Result<Reply> {
        if self.broken {
            self.stream = connect(&self.socket).await.map_err(|error| {
                openagents_connect::Error::new(
                    openagents_connect::Code::Unavailable,
                    format!("the host did not answer: {error}"),
                )
            })?;
            self.broken = false;
        }
        let id = self.next;
        self.next += 1;
        let result = control::call(&mut self.stream, &Request::new(id, op)).await;
        self.broken = result.is_err();
        result
    }
}

/// Whether a host refused a request it could not read: an older host ends
/// the connection after answering under ID 0.
fn unreadable(result: &openagents_connect::Result<Reply>) -> bool {
    match result {
        Ok(Reply::Refused { code, .. }) => code == "malformed",
        Err(error) => error.code == openagents_connect::Code::Malformed,
        Ok(_) => false,
    }
}

impl Host for ControlHost {
    fn apply(
        &mut self,
        command: Command,
        caller: Caller,
    ) -> BoxFuture<'_, Result<Snapshot, String>> {
        Box::pin(async move {
            let mut result = self
                .call(Op::Chat {
                    command: command.clone(),
                    caller: (!self.older).then_some(caller),
                })
                .await;
            if !self.older && unreadable(&result) {
                // An older host: ask again on a new connection, without the
                // caller, and keep doing so.
                self.older = true;
                match connect(&self.socket).await {
                    Ok(stream) => {
                        self.stream = stream;
                        result = self
                            .call(Op::Chat {
                                command,
                                caller: None,
                            })
                            .await;
                    }
                    Err(error) => return Err(format!("the host did not answer: {error}")),
                }
            }
            match result {
                Ok(Reply::Chat { snapshot }) => Ok(snapshot),
                Ok(Reply::Refused { message, .. }) => Err(message),
                Ok(_) => Err("the host answered another question".into()),
                Err(error) => Err(format!("the host did not answer: {error}")),
            }
        })
    }

    /// It asks on a connection of its own: an older host ends a connection
    /// that carried an operation it doesn't know.
    fn migrate(&mut self, home: &Path) -> BoxFuture<'_, Migration> {
        let home = home.display().to_string();
        Box::pin(async move {
            let Ok(mut stream) = connect(&self.socket).await else {
                return Migration::Quiet;
            };
            match control::call(&mut stream, &Request::new(1, Op::ChatMigrate { home })).await {
                Ok(Reply::ChatMigrated { moved, .. }) => Migration::Moved(moved),
                Ok(Reply::Refused { code, message }) if code != "malformed" => {
                    Migration::Kept(message)
                }
                _ => Migration::Quiet,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A person's own runs start at once and approve every step (#10104):
    /// with no settings file, the client never waits on an offer, and the
    /// access the runs get is `full`.
    #[test]
    fn the_client_starts_at_once_and_approves_everything_by_default() {
        let settings = super::super::settings::Coder::default();
        assert_eq!(settings.start, super::super::settings::Start::AtOnce);
        assert_eq!(settings.access, super::super::adapter::Access::Full);
    }

    #[test]
    fn an_older_host_is_one_that_cannot_read_the_caller() {
        assert!(unreadable(&Ok(Reply::Refused {
            code: "malformed".into(),
            message: "malformed request".into(),
        })));
        assert!(!unreadable(&Ok(Reply::Refused {
            code: "chat".into(),
            message: "no such chat".into(),
        })));
    }
}
