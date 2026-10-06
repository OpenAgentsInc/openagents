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
    BoxFuture, Coder, Dial, Follow, Host, Issue, IssueStarted, Migration, Permit, Progress, Ran,
    Started, Steering,
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

/// How long a command a chat reply proposed may run here.
pub const COMMAND_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// The most of a command's output a chat shows.
pub const COMMAND_OUTPUT: usize = 16 * 1024;

/// Run `openagents ARGV` with this program, as [`Here::run_command`] does.
fn run_here(argv: &[String]) -> Result<Ran, String> {
    run_for(argv, COMMAND_TIMEOUT, true)
}

/// The first line of a test's task: the prompt after its front matter.
fn task_line(prompt: &str) -> String {
    let body = prompt
        .strip_prefix("+++")
        .and_then(|rest| rest.split_once("\n+++").map(|(_, body)| body))
        .unwrap_or(prompt);
    let line = body
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default();
    let mut cut: String = line.chars().take(160).collect();
    if line.chars().count() > 160 {
        cut.push('…');
    }
    cut
}

/// Run `openagents ARGV` with this program for up to `timeout`. Its
/// standard error joins the output when `show_errors`; otherwise only when it
/// fails, so a test run's progress lines stay out of its summary.
fn run_for(
    argv: &[String],
    timeout: std::time::Duration,
    show_errors: bool,
) -> Result<Ran, String> {
    use std::io::Read;
    use std::process::Stdio;
    let exe = std::env::current_exe().map_err(|e| format!("cannot find this program: {e}"))?;
    let mut child = std::process::Command::new(exe)
        .args(argv)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot start openagents {}: {e}", argv.join(" ")))?;
    let read = |pipe: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            if let Some(mut pipe) = pipe {
                let _ = pipe.read_to_end(&mut bytes);
            }
            bytes
        })
    };
    let out = read(
        child
            .stdout
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    );
    let err = read(
        child
            .stderr
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    );
    let started = std::time::Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "openagents {} did not finish within {} s and was stopped.",
                    argv.join(" "),
                    timeout.as_secs()
                ));
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(50)),
            Err(e) => return Err(format!("cannot wait for openagents: {e}")),
        }
    };
    let mut text = String::from_utf8_lossy(&out.join().unwrap_or_default()).into_owned();
    let errors = String::from_utf8_lossy(&err.join().unwrap_or_default()).into_owned();
    if !errors.trim().is_empty() && (show_errors || !status.success()) {
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&errors);
    }
    if text.len() > COMMAND_OUTPUT {
        let mut end = COMMAND_OUTPUT;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        text.push_str("\n…");
    }
    Ok(Ran {
        ok: status.success(),
        output: text.trim_end().to_owned(),
    })
}

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

    fn start_run(
        &self,
        store: &Path,
        dir: &Path,
        title: &str,
        prompt: &str,
        chat: &str,
        engine: nostr::cj_conversation::Engine,
        read_only: bool,
    ) -> Result<Started, String> {
        let record = Self::runner(store).start_shaped(
            dir,
            title,
            prompt,
            Some(chat),
            super::settings::provider_of(engine),
            super::local::Shape {
                only: true,
                read_only,
            },
        )?;
        Ok(Started {
            task: record.task,
            project: record.project,
            worktree: record.worktree,
        })
    }

    /// The issue the message asks to work, as Jev judges it after
    /// routing: one it names, or, when it asks Coder to choose one itself,
    /// an open issue nobody holds ([`super::issue_pick`]). The flow then
    /// claims it like any other, so an engine run never has to.
    fn issue(&self, request: &str, earlier: &str, dir: &Path) -> Option<Box<dyn Issue>> {
        // Issues live on GitHub: a checkout whose `origin` is elsewhere
        // (a local bare repository, another forge) has none to work, so
        // the message runs as an ordinary task (#10398).
        if !super::issue_run::on_github(dir) {
            return None;
        }
        Some(
            match super::issue_run::asked_work_blocking(request, earlier, dir)? {
                super::issue_run::Asked::Issue(reference) => Box::new(IssueRef {
                    reference,
                    picked: None,
                }),
                super::issue_run::Asked::Pick => {
                    match super::issue_run::pick_here(&super::issue_run::Gh, dir) {
                        Ok((repository, picked)) => Box::new(IssueRef {
                            reference: super::issue_run::Reference {
                                repository: Some(repository),
                                number: picked.number,
                            },
                            picked: Some(picked.title),
                        }),
                        Err(why) => Box::new(Unpicked(why)),
                    }
                }
            },
        )
    }

    /// The delegate recipe's groundwork for `prompt`, prepared on a thread
    /// of its own while the router judges the message (#10279), when a run
    /// started now would begin on the lean Claude Code session, whose
    /// recipe reads only the request. It is kept in the task store for the
    /// run's owner ([`coder_delegate::recipe::Ahead`]); without Jev or a
    /// checkout nothing is prepared.
    fn ahead(&self, store: &Path, dir: &Path, prompt: &str) {
        let (store, dir, prompt) = (store.to_path_buf(), dir.to_path_buf(), prompt.to_owned());
        let _ = std::thread::Builder::new()
            .name("coder-recipe-ahead".into())
            .spawn(move || {
                if !Self::runner(&store).starts_lean_claude() {
                    return;
                }
                let Ok(checkout) = local::checkout(&dir) else {
                    return;
                };
                let (Some(jev), _) =
                    crate::delegate_door::jev_from(&crate::delegate_door::env_value)
                else {
                    return;
                };
                let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                else {
                    return;
                };
                use coder_delegate::recipe::{Ahead, ahead, knowledge_dirs};
                let path = Ahead::path(&store, &prompt, "");
                Ahead::begin(&path);
                let prepared = runtime.block_on(ahead(
                    &checkout.top,
                    &prompt,
                    "",
                    jev,
                    knowledge_dirs(&checkout.top),
                ));
                Ahead::finish(&path, Some(&prepared));
            });
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

    fn steer(&self, store: &Path, task: &str, text: &str) -> Result<Steering, String> {
        Ok(match Self::runner(store).steer(task, text)? {
            local::Steered::NextStep => Steering::NextStep,
            local::Steered::NextTurn(record) => {
                Steering::NextTurn(record.turns.last().map_or(1, |start| start.turn))
            }
        })
    }

    fn result(&self, store: &Path, task: &str) -> Option<CoderRun> {
        local::result_in(Some(store), task)
    }

    fn endings(&self, store: &Path, task: &str) -> Vec<openagents_chat::coder_events::CoderEvent> {
        local::endings_in(Some(store), task)
    }

    fn apply(&self, store: &Path, task: &str) -> Result<(std::path::PathBuf, Vec<String>), String> {
        super::apply::apply(store, task).map(|applied| (applied.checkout, applied.files))
    }

    /// A command a reply proposed runs only as this build's own command
    /// tree declares it (#10170): read-only commands at once, commands
    /// that change something here after a confirm, and money, secrets,
    /// and anything the tree does not know never.
    fn permit(&self, argv: &[String]) -> Permit {
        use crate::cli_route::tree::Effect;
        match crate::cli_route::gate::effect_here(argv) {
            Some(Effect::ReadOnly) => Permit::Now,
            Some(Effect::LocalWrite | Effect::Publishes | Effect::Grants | Effect::LongRunning) => {
                Permit::Confirm
            }
            Some(Effect::Spends | Effect::Secret) | None => Permit::Never,
        }
    }

    /// The command's effect in this build's own command tree, in the
    /// route contract's words (13.4): what the route policy reads.
    fn effect(&self, argv: &[String]) -> Option<route_contract::route::Effect> {
        use crate::cli_route::tree::Effect;
        use route_contract::route::Effect as Contract;
        Some(match crate::cli_route::gate::effect_here(argv)? {
            Effect::ReadOnly => Contract::ReadOnly,
            Effect::LocalWrite => Contract::LocalWrite,
            Effect::Publishes => Contract::Publishes,
            Effect::Grants => Contract::Grants,
            Effect::Spends => Contract::Spends,
            Effect::Secret => Contract::Secret,
            Effect::LongRunning => Contract::LongRunning,
        })
    }

    /// The task owner's disposition of `task`, with its current turn's cost
    /// and wall time ([`super::lifecycle::observation`]).
    fn observe(&self, store: &Path, task: &str) -> Option<route_contract::record::Observation> {
        super::lifecycle::observe(store, task)
    }

    fn checking(&self, store: &Path, task: &str) -> bool {
        super::local_checks::pending(store, task)
    }

    /// `openagents ARGV` as a child of this program, as `openagents mcp
    /// serve` runs a tool call: no input, stopped after
    /// [`COMMAND_TIMEOUT`], what it printed kept to [`COMMAND_OUTPUT`].
    fn run_command(&self, argv: &[String]) -> Result<Ran, String> {
        run_here(argv)
    }

    fn worktree(&self, store: &Path, task: &str) -> Option<PathBuf> {
        // A worktree removed when its task ended comes back first (#10291).
        super::retire::ensure(store, task)
            .ok()
            .flatten()
            .map(|record| PathBuf::from(record.worktree))
    }

    /// The package record loads and resolves as `openagents plugin
    /// install` checks it, and the tests load as `openagents plugin test`
    /// loads them (`ext_eval::author::files::read`).
    fn plugin_tests(&self, dir: &Path) -> Result<Vec<openagents_chat::plugin_flow::Test>, String> {
        use crate::package::Package;
        let package = Package::load(&dir.join("package.json"))?;
        Package::resolve(dir, &package).map_err(|refusal| refusal.to_string())?;
        let evals = ext_eval::eval_dir(dir, None, package.eval_dir.as_deref())
            .map_err(|error| error.to_string())?;
        if !evals.is_dir() {
            return Ok(Vec::new());
        }
        let cases = ext_eval::author::files::read(&evals).map_err(|error| error.to_string())?;
        Ok(cases
            .into_iter()
            .take(openagents_chat::plugin_flow::MAX_TESTS)
            .map(|case| openagents_chat::plugin_flow::Test {
                name: case.id,
                kind: match case.kind {
                    nostr::eval_ext::CaseKind::ShouldNotFire => "should-not-fire".into(),
                    _ => "should-fire".into(),
                },
                task: task_line(&case.prompt),
            })
            .collect())
    }

    fn plugin_command(&self, argv: &[String], timeout: std::time::Duration) -> Result<Ran, String> {
        run_for(argv, timeout, false)
    }

    /// A draft `openagents background draft` kept in this user's
    /// `~/.openagents/background/drafts`, while it is fresh.
    #[cfg(unix)]
    fn drafted(&self, id: &str) -> bool {
        background::Layout::from_env()
            .is_ok_and(|layout| background::compile::drafted(&layout, id, background::paths::now()))
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

struct IssueRef {
    reference: super::issue_run::Reference,
    /// The title, when Coder chose the issue itself.
    picked: Option<String>,
}

impl Issue for IssueRef {
    fn number(&self) -> u64 {
        self.reference.number
    }

    fn picked(&self) -> Option<String> {
        self.picked.clone()
    }

    fn begin(
        self: Box<Self>,
        store: &Path,
        dir: &Path,
        chat: &str,
    ) -> Result<IssueStarted, String> {
        let mut runner = super::issue_run::Runner::new(store.to_path_buf());
        // A picked issue someone claimed since the pick is left to them.
        runner.skip_claimed = self.picked.is_some();
        let started = runner
            .begin(dir, &self.reference, Some(chat))
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

/// A pickup that found no free issue: it starts nothing and says why.
struct Unpicked(String);

impl Issue for Unpicked {
    fn number(&self) -> u64 {
        0
    }

    fn begin(self: Box<Self>, _: &Path, _: &Path, _: &str) -> Result<IssueStarted, String> {
        Err(self.0)
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
                used: false,
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
    /// The connection already carried a call. One that sat idle may have
    /// outlived its host: a restart (an update, `systemctl restart`)
    /// leaves the client a dead socket that fails the next write.
    used: bool,
}

/// What a host that is not answering says, in place of the transport's
/// own words.
pub(crate) const NOT_ANSWERING: &str =
    "OpenAgents on this computer is not answering. It may be restarting; try again.";

impl ControlHost {
    async fn call(&mut self, op: Op) -> openagents_connect::Result<Reply> {
        if self.broken {
            self.stream = connect(&self.socket).await.map_err(|error| {
                openagents_connect::Error::new(
                    openagents_connect::Code::Unavailable,
                    format!("no host at the control socket: {error}"),
                )
            })?;
            self.broken = false;
            self.used = false;
        }
        let id = self.next;
        self.next += 1;
        let request = Request::new(id, op);
        let mut result = control::call(&mut self.stream, &request).await;
        // The host restarted since this connection's last call: the
        // connection is dead, and the new host never saw the request. Ask
        // it again at once on a new connection. A chat send carries its
        // send ID, so the service takes it once however often it is asked.
        if self.used
            && matches!(&result, Err(error) if error.code == openagents_connect::Code::Unavailable)
            && let Ok(stream) = connect(&self.socket).await
        {
            self.stream = stream;
            result = control::call(&mut self.stream, &request).await;
        }
        self.used = true;
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
                        self.used = false;
                        result = self
                            .call(Op::Chat {
                                command,
                                caller: None,
                            })
                            .await;
                    }
                    Err(_) => return Err(NOT_ANSWERING.into()),
                }
            }
            match result {
                Ok(Reply::Chat { snapshot }) => Ok(snapshot),
                Ok(Reply::Refused { message, .. }) => Err(message),
                Ok(_) => Err("the host answered another question".into()),
                Err(_) => Err(NOT_ANSWERING.into()),
            }
        })
    }

    fn unanswered(&self) -> bool {
        self.broken
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

    /// One host on `socket` that answers every chat command with an empty
    /// snapshot, until it is aborted.
    #[cfg(unix)]
    fn serve(socket: &Path) -> tokio::task::JoinHandle<()> {
        let _ = std::fs::remove_file(socket);
        let listener = tokio::net::UnixListener::bind(socket).unwrap();
        tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    return;
                };
                // One connection at a time, on this task, so aborting it
                // ends the connection too, as a host's exit does.
                while let Ok(Some(request)) = control::next_request(&mut stream).await {
                    let reply = Reply::Chat {
                        snapshot: Snapshot::default(),
                    };
                    let response = control::Response::new(request.id, reply);
                    if control::respond(&mut stream, &response).await.is_err() {
                        break;
                    }
                }
            }
        })
    }

    /// The owner's follow-up (2026-10-02) went to a connection the host's
    /// restart had closed, and failed "the host did not answer —
    /// unavailable, write failed". A connection that outlived its host now
    /// opens a new one and asks again, so the send goes through.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_send_after_the_host_restarted_goes_through() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("control.sock");
        let first = serve(&socket);
        let mut host = Control.dial(&socket).await.expect("the host answers");
        let read = Command::Read {
            chat: "0".repeat(32),
            before: None,
        };
        host.apply(read.clone(), Caller::CLI).await.unwrap();
        // The host restarts: the old one and its connections end, and a
        // new one listens on the same socket.
        first.abort();
        let _ = first.await;
        tokio::task::yield_now().await;
        let second = serve(&socket);
        let sent = host
            .apply(
                Command::Send {
                    chat: "0".repeat(32),
                    request: "1".repeat(32),
                    text: "ok, create issues for them".into(),
                },
                Caller::CLI,
            )
            .await;
        assert!(sent.is_ok(), "{sent:?}");
        assert!(!host.unanswered());
        second.abort();
    }

    /// No host at all: the send fails in plain words, and says the host
    /// did not answer, so the client waits for it.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_host_that_is_gone_is_said_plainly() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("control.sock");
        let first = serve(&socket);
        let mut host = Control.dial(&socket).await.expect("the host answers");
        let read = Command::Read {
            chat: "0".repeat(32),
            before: None,
        };
        host.apply(read.clone(), Caller::CLI).await.unwrap();
        first.abort();
        let _ = first.await;
        std::fs::remove_file(&socket).unwrap();
        let sent = host.apply(read, Caller::CLI).await;
        assert_eq!(sent, Err(NOT_ANSWERING.to_owned()));
        assert!(host.unanswered());
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
