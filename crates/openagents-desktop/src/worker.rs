//! Runs the model's requests off the UI thread.
//!
//! The worker owns the control client, so a slow host never stalls a
//! frame: each [`Request`] becomes at most one [`Outcome`], sent back with
//! a wake of the event loop. `run` is the same handling, inline, for the
//! capture mode and tests.
//!
//! Requests run on two lanes, each its own thread and each in order: the
//! host lane asks the control socket (every call bounded by its timeout),
//! and the local lane runs what can take as long as it likes: starting
//! Coder (an upgrade of an earlier setup runs `coder host adopt`), `coder
//! task list`, the folder chooser that waits for the person, and the
//! clipboard. So a slow start or an open chooser never keeps the window
//! from hearing the host.
//!
//! Against the in-process host (`--fake-host`) the local lane touches
//! nothing on this computer that the real app manages: starting Coder
//! answers at once without registering or upgrading anything, and the
//! sign-in check does not ask the keychain.

use crate::platform;
use openagents_chat_app::coder_run;
use openagents_desktop::codes::Action;
use openagents_desktop::control::{ControlError, HostControl, PATIENCE};
use openagents_desktop::fake::FakeHost;
use openagents_desktop::model::{Agent, Agents, Outcome, Refreshed, Request, Started, Task};
use rust_native_desktop::Waker;
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

/// The most recent tasks the home screen lists.
const RECENT_TASKS: usize = 5;
/// The most bytes of `coder task list` output read.
const TASK_LIST_MAX: usize = 16 * 1024 * 1024;

/// What the worker needs: the host lane, the local lane, and the Coder
/// lane.
pub struct Context {
    host: HostLane,
    local: LocalLane,
    coder: CoderLane,
}

/// Coder runs on this computer for chats: started, followed, answered, and
/// stopped through `coder::task::local`, the runner `openagents chat`
/// uses, over this computer's task store. Its own thread, so following a
/// run never waits behind a folder chooser or a slow host.
struct CoderLane {
    fake: bool,
    local: Option<coder::task::local::Local>,
    /// The runner follows the person's settings file, read again at each
    /// start so a change on Settings applies to the next run; a runner a
    /// test gave keeps its own settings.
    here: bool,
    /// One follower a task, kept across polls so each poll reads only
    /// what is new.
    follows: std::collections::BTreeMap<String, coder::task::local::Follow>,
}

/// The control client, and the in-process host in `--fake-host` mode.
struct HostLane {
    control: Box<dyn HostControl>,
    fake: Option<FakeHost>,
    /// In `--fake-host` mode, a phone scans the code this long after it
    /// first shows.
    fake_scan: Option<Duration>,
    first_code: Option<Instant>,
}

/// What runs on this computer: Coder's binary, the home folder, and
/// whether the host is the in-process one.
struct LocalLane {
    fake: bool,
    /// Whether this is the packaged release, which may run `coder host
    /// adopt` ([`openagents_desktop::RELEASE`]; a test may stand in).
    release: bool,
    coder: Option<PathBuf>,
    home: PathBuf,
    saved_config: Option<coder_history::Config>,
    saved_history: Option<Result<coder_history::History, String>>,
}

/// Whether `request` runs on the Coder lane.
fn coder(request: &Request) -> bool {
    matches!(request, Request::CoderRun { request, .. } if *request != coder_run::Request::Choose)
}

/// Whether `request` runs on the local lane.
fn local(request: &Request) -> bool {
    if let Request::CoderRun { request, .. } = request {
        return *request == coder_run::Request::Choose;
    }
    if let Request::Saved { request, .. } = request {
        return !matches!(
            request,
            openagents_chat_app::retained::Request::Continue { .. }
        );
    }
    matches!(
        request,
        Request::Start
            | Request::Coder
            | Request::ChooseFolder
            | Request::Copy { .. }
            | Request::ClearClipboard { .. }
            | Request::OpenLoginItems
    )
}

impl Context {
    pub fn is_fixture(&self) -> bool {
        self.local.fake
    }

    pub fn new(
        control: Box<dyn HostControl>,
        fake: Option<FakeHost>,
        fake_scan: Option<Duration>,
        coder: Option<PathBuf>,
        home: PathBuf,
    ) -> Context {
        Context {
            coder: CoderLane {
                fake: fake.is_some(),
                local: None,
                here: false,
                follows: Default::default(),
            },
            local: LocalLane {
                fake: fake.is_some(),
                release: openagents_desktop::RELEASE,
                coder,
                home,
                saved_config: None,
                saved_history: None,
            },
            host: HostLane {
                control,
                fake,
                fake_scan,
                first_code: None,
            },
        }
    }

    /// Start Coder as the packaged release would, adopting an earlier
    /// setup.
    #[cfg(test)]
    pub fn released(mut self) -> Self {
        self.local.release = true;
        self
    }

    /// Select explicit history roots for an isolated fixture or host adapter.
    #[cfg(test)]
    pub fn with_saved_history(mut self, config: coder_history::Config) -> Self {
        self.local.saved_config = Some(config);
        self
    }

    /// Run chats' Coder over the task store `store` with `local`, as a
    /// test's scripted runner does.
    #[cfg(test)]
    pub fn with_coder(mut self, local: coder::task::local::Local) -> Self {
        self.coder.fake = false;
        self.coder.local = Some(local);
        self
    }

    /// Runs one request.
    pub fn run(&mut self, request: Request) -> Option<Outcome> {
        if coder(&request) {
            self.coder.run(request)
        } else if local(&request) {
            self.local.run(request)
        } else {
            self.host.run(request)
        }
    }
}

impl LocalLane {
    fn saved(
        &mut self,
        request: openagents_chat_app::retained::Request,
    ) -> Result<openagents_chat_app::retained::Answer, String> {
        use openagents_chat_app::retained::{Answer, Request};
        if self.fake && self.saved_config.is_none() && self.saved_history.is_none() {
            return match request {
                Request::Catalog(_) => Ok(Answer::Catalog(coder_history::CatalogPage {
                    snapshot: "fixture".into(),
                    entries: vec![],
                    next: None,
                    notices: vec![],
                })),
                _ => Err("No saved session is selected in this offline fixture.".into()),
            };
        }
        if self.saved_history.as_ref().is_none_or(Result::is_err) {
            #[cfg(test)]
            assert_ne!(
                Some(self.home.as_os_str()),
                std::env::var_os("HOME").as_deref(),
                "saved-session tests must use an explicit temporary home"
            );
            let config = self
                .saved_config
                .clone()
                .unwrap_or_else(|| coder_history::Config {
                    codex: self
                        .home
                        .join(".codex")
                        .is_dir()
                        .then(|| self.home.join(".codex")),
                    claude: self
                        .home
                        .join(".claude")
                        .is_dir()
                        .then(|| self.home.join(".claude")),
                    ..coder_history::Config::default()
                });
            if config.codex.is_none() && config.claude.is_none() {
                return match request {
                    Request::Catalog(_) => Ok(Answer::Catalog(coder_history::CatalogPage {
                        snapshot: "empty".into(),
                        entries: vec![],
                        next: None,
                        notices: vec![],
                    })),
                    _ => Err("The saved-session source is unavailable.".into()),
                };
            }
            self.saved_history = Some(
                coder_history::History::open(config)
                    .map_err(|error| format!("Saved sessions could not be opened: {error:?}")),
            );
        }
        let history = self
            .saved_history
            .as_ref()
            .unwrap()
            .as_ref()
            .map_err(Clone::clone)?;
        match request {
            Request::Catalog(query) => history.catalog(query).map(Answer::Catalog),
            Request::Page(query) => history.transcript(query).map(Answer::Page),
            Request::Continue { .. } => unreachable!("continuation runs on the admitted host lane"),
        }
        .map_err(|error| {
            format!("The saved-session read failed: {error:?}. Refresh the session if it changed.")
        })
    }

    fn run(&mut self, request: Request) -> Option<Outcome> {
        match request {
            Request::Saved { ticket, request } => Some(Outcome::Saved {
                ticket,
                result: Box::new(self.saved(request).map_err(|message| {
                    openagents_chat_app::retained::Failure {
                        message,
                        uncertain: false,
                    }
                })),
            }),
            Request::Copy { code } => platform::copy(&code).then_some(Outcome::Copied),
            Request::ClearClipboard { code } => {
                platform::clear_if(&code);
                None
            }
            Request::ChooseFolder => Some(Outcome::Folder(platform::choose_folder())),
            Request::CoderRun { chat, ticket, .. } => Some(Outcome::CoderRun {
                chat,
                ticket,
                result: Box::new(Ok(coder_run::Answer::Folder(if self.fake {
                    None
                } else {
                    match platform::choose_folder() {
                        openagents_desktop::folder::Chosen::Folder(path) => {
                            Some(path.display().to_string())
                        }
                        _ => None,
                    }
                }))),
            }),
            Request::Coder => Some(Outcome::Coder {
                agents: if self.fake {
                    Agents {
                        codex: true,
                        claude: false,
                        grok: None,
                        claude_problem: None,
                    }
                } else {
                    platform::signed_in(&self.home)
                },
                tasks: self.tasks(),
            }),
            Request::OpenLoginItems => {
                platform::open_login_items();
                None
            }
            Request::Start => Some(Outcome::Started(self.start())),
            other => unreachable!("{other:?} runs on the host lane"),
        }
    }

    /// Starts Coder under this app, upgrading an earlier setup silently
    /// first ([`openagents_desktop::migrate::start`]). Against the
    /// in-process host there is nothing to start: it answers at once and
    /// registers, upgrades, and reads nothing.
    fn start(&self) -> Started {
        if self.fake {
            return Started {
                agent: Agent::Enabled,
                note: None,
            };
        }
        // A dev build never runs `coder host adopt` (#10096).
        if !self.release {
            return openagents_desktop::migrate::start_dev(&self.home, &mut |keys| {
                platform::register_agent(keys)
            });
        }
        openagents_desktop::migrate::start(self.coder.as_deref(), &self.home, &mut |keys| {
            platform::register_agent(keys)
        })
    }
}

impl HostLane {
    fn run(&mut self, request: Request) -> Option<Outcome> {
        match request {
            Request::Saved {
                ticket,
                request:
                    openagents_chat_app::retained::Request::Continue {
                        request,
                        chat,
                        task,
                    },
            } => Some(Outcome::Saved {
                ticket,
                result: Box::new(
                    self.control
                        .import_task(request, chat, task)
                        .map(openagents_chat_app::retained::Answer::Continued)
                        .map_err(|error| openagents_chat_app::retained::Failure { uncertain: !matches!(&error, ControlError::Refused { code, .. } if code != "unavailable"), message: error.to_string() }),
                ),
            }),
            Request::Saved { .. } => unreachable!("saved-session reads run on the local lane"),
            Request::TaskChat {
                chat,
                ticket,
                request,
            } => Some(Outcome::TaskChat {
                chat,
                ticket,
                result: Box::new(self.control.task_chat(request)),
            }),
            Request::Chat { ticket, command } => Some(Outcome::Chat {
                ticket,
                result: Box::new(self.control.chat(command)),
            }),
            Request::Refresh => {
                self.fake_scan_if_due();
                Some(Outcome::Refreshed(self.refresh().map(Box::new)))
            }
            Request::Code(Action::Create { ticket }) => Some(match self.control.invite() {
                Ok(invite) => {
                    self.first_code.get_or_insert_with(Instant::now);
                    Outcome::Created {
                        ticket,
                        invitation: invite.invitation,
                        code: invite.code,
                    }
                }
                Err(_) => Outcome::CreateFailed { ticket },
            }),
            Request::Code(Action::Cancel { invitation }) => {
                let _ = self.control.cancel(&invitation);
                None
            }
            Request::Code(Action::CancelAll) => {
                let _ = self.control.cancel_all();
                None
            }
            Request::Revoke { device } => {
                self.control.revoke(&device).err().map(|_| Outcome::Failed {
                    message: "Couldn't remove that phone. Try again.".into(),
                })
            }
            Request::AddProject {
                path,
                replace,
                autostart,
            } => Some(Outcome::Picked(openagents_desktop::control::pick_project(
                self.control.as_mut(),
                &path.to_string_lossy(),
                replace.as_deref(),
                autostart,
            ))),
            // The host may be starting again after a project change; wait
            // for it rather than report a failure it would not have.
            Request::SetAutostart(policy) => {
                openagents_desktop::control::set_autostart(self.control.as_mut(), policy, PATIENCE)
                    .err()
                    .map(|_| Outcome::Failed {
                        message: openagents_desktop::model::SETTING_FAILED.into(),
                    })
            }
            Request::NearbyDecide { id, connect } => self
                .control
                .nearby_decide(id, connect)
                .err()
                .map(|_| Outcome::Failed {
                    message: "That phone stopped asking. Ask again from the phone.".into(),
                }),
            Request::Engine => Some(Outcome::Engine(self.control.engine_status())),
            other => unreachable!("{other:?} runs on the local lane"),
        }
    }

    fn refresh(&mut self) -> Option<Refreshed> {
        let status = match self.control.status() {
            Ok(status) => status,
            Err(ControlError::Unreachable | ControlError::Malformed) => return None,
            Err(ControlError::Refused { .. }) => return None,
        };
        Some(Refreshed {
            status,
            devices: self.control.devices().unwrap_or_default(),
            projects: self.control.projects().unwrap_or_default(),
            autostart: self
                .control
                .autostart()
                .unwrap_or(openagents_desktop::control::Autostart {
                    enabled: false,
                    projects: vec![],
                    max_running: 1,
                }),
            nearby: self.control.nearby_pending().unwrap_or_default(),
            watchers: self.watchers(),
            background: self.background_notice(),
        })
    }

    /// The newest background notice this computer's host sent; none for a
    /// stand-in host.
    fn background_notice(&self) -> Option<(u64, String)> {
        #[cfg(unix)]
        if self.fake.is_none() {
            return openagents_desktop::background_pane::here()
                .and_then(|layout| openagents_desktop::background_pane::latest(&layout));
        }
        None
    }

    /// The background watchers this computer's host runs, read from its
    /// store under `~/.openagents/background`; none for a stand-in host.
    fn watchers(&self) -> Vec<String> {
        #[cfg(unix)]
        if self.fake.is_none()
            && let Ok(layout) = background::Layout::from_env()
        {
            return background::view::watchers(&layout);
        }
        Vec::new()
    }

    /// In `--fake-host` mode, a phone scans the newest code once.
    fn fake_scan_if_due(&mut self) {
        let (Some(fake), Some(after), Some(first)) = (&self.fake, self.fake_scan, self.first_code)
        else {
            return;
        };
        if first.elapsed() < after {
            return;
        }
        if let Some(open) = fake.open().last() {
            let _ = fake.redeem(&open.invitation, "Kai's iPhone");
            self.fake_scan = None;
        }
    }
}

/// Where the window notes the project Coder last started in: beside the
/// local runs' records in the task store.
fn last_project(store: &std::path::Path) -> PathBuf {
    store.join("local").join("last-project")
}

/// The command that answers a thread's question from a terminal, as
/// `openagents chat` names it for a thread in this computer's host.
pub fn answer_hint(chat: &str) -> String {
    format!("openagents chat answer --thread {chat} \"YOUR ANSWER\"")
}

impl CoderLane {
    fn run(&mut self, request: Request) -> Option<Outcome> {
        let Request::CoderRun {
            chat,
            ticket,
            request,
        } = request
        else {
            unreachable!("{request:?} runs on another lane")
        };
        let result = self.answer(&chat, request);
        Some(Outcome::CoderRun {
            chat,
            ticket,
            result: Box::new(result),
        })
    }

    fn local(&mut self) -> &coder::task::local::Local {
        // With the person's settings (#10036): the providers, the usage
        // threshold, the project folders, and what commands may reach.
        if self.local.is_none() {
            self.here = true;
        }
        let fake = self.fake;
        self.local.get_or_insert_with(|| {
            let local = coder::task::local::Local::here(coder::task::local::default_store());
            if fake {
                return local;
            }
            // The last project's spare worktree, so the next start there
            // does not wait on Git (#10115).
            if let Ok(last) = std::fs::read_to_string(last_project(local.store())) {
                local.warm(std::path::Path::new(last.trim()));
            }
            // Before a start passes an engine over for capacity, or starts
            // the one the person asked for, the host on this computer reads
            // its usage now (#10105); only the host reads the probe's token.
            local.with_fresh(Box::new(|providers| {
                use openagents_desktop::control::{HostControl, SocketControl};
                let Some(path) = openagents_connect::control::socket_path() else {
                    return;
                };
                let names: Vec<String> = providers
                    .iter()
                    .map(|provider| provider.as_str().to_owned())
                    .collect();
                let _ = SocketControl::new(path).engine_refresh(&names);
            }))
        })
    }

    fn answer(
        &mut self,
        chat: &str,
        request: coder_run::Request,
    ) -> Result<coder_run::Answer, String> {
        use coder::task::local::{self as run, State};
        use coder_run::{Answer, Request};
        if self.fake {
            return Err("Coder doesn't run in this offline fixture.".into());
        }
        match request {
            Request::Start {
                title,
                prompt,
                mut dirs,
                images,
                engine,
            } => {
                let store = self.local().store().to_path_buf();
                if self.here
                    && let Some(local) = self.local.as_mut()
                {
                    local.reload_settings();
                }
                if let Ok(last) = std::fs::read_to_string(last_project(&store)) {
                    dirs.push(last.trim().to_owned());
                }
                // The settings' project folders are projects too.
                if let Ok(settings) = self.local().settings() {
                    dirs.extend(
                        settings
                            .projects
                            .iter()
                            .map(|folder| folder.display().to_string()),
                    );
                }
                let mut why = None;
                let mut tried = std::collections::BTreeSet::new();
                for dir in dirs.iter().filter(|dir| !dir.is_empty()) {
                    if !tried.insert(dir.clone()) {
                        continue;
                    }
                    let path = std::path::Path::new(dir);
                    match self.local().project(path) {
                        Ok(_) => {
                            // A chat that asks Coder to work a GitHub issue
                            // runs the issue flow (#10049); Jev chooses the
                            // issue among the references the chat names.
                            // The issue flow takes text only; a start that
                            // carries images is an ordinary task with them.
                            if images.is_empty()
                                && let Some(reference) =
                                    coder::task::issue_run::asked_blocking(&prompt, "", path)
                            {
                                let record = start_issue(&store, path, reference, chat)?;
                                let _ = std::fs::write(last_project(&store), &record.checkout);
                                return Ok(Answer::Started {
                                    task: record.task,
                                    project: record.project,
                                    checkout: record.checkout,
                                });
                            }
                            // The engine the person asked for goes first
                            // (#10076).
                            let record = self.local().start_requested(
                                path,
                                &title,
                                &prompt,
                                Some(chat),
                                &images,
                                engine.map(coder::task::settings::provider_of),
                            )?;
                            let _ = std::fs::write(last_project(&store), &record.checkout);
                            return Ok(Answer::Started {
                                task: record.task,
                                project: record.project,
                                checkout: record.checkout,
                            });
                        }
                        Err(error) => {
                            why.get_or_insert(if error.contains("is not in a Git checkout") {
                                format!(
                                    "{dir} is not a Git checkout. Choose the project folder Coder \
                                     works in."
                                )
                            } else {
                                error
                            });
                        }
                    }
                }
                Ok(Answer::NeedsProject {
                    why: why.unwrap_or_else(|| {
                        "Choose the project folder Coder works in: a Git checkout on this \
                         computer."
                            .into()
                    }),
                })
            }
            Request::Poll { task } => {
                if !self.follows.contains_key(&task) {
                    let follow = self
                        .local()
                        .follow(&task, Some(chat), Some(answer_hint(chat)));
                    self.follows.insert(task.clone(), follow);
                }
                let follow = self.follows.get_mut(&task).expect("a follower");
                let (lines, state) = follow.poll()?;
                dump(&lines);
                Ok(Answer::Lines {
                    lines,
                    state: match state {
                        State::Running => coder_run::State::Running,
                        State::Waiting => coder_run::State::Waiting,
                        State::Ended => coder_run::State::Ended,
                    },
                })
            }
            Request::Stop { task } => self.local().stop(&task).map(|()| Answer::Stopping),
            Request::Continue { task, text } => {
                self.local().answer(&task, &text).map(|_| Answer::Continued)
            }
            // What the run changed, at exact revisions, up to the pane's
            // bound, with the task's last publication.
            Request::Review { task } => {
                let store = self.local().store().to_path_buf();
                let record = run::record(&store, &task)
                    .ok_or("This task was not started on this computer from a chat.")?;
                let mut review = coder::task::review::read(
                    &task,
                    std::path::Path::new(&record.worktree),
                    &record.base,
                    openagents_chat_app::changes::MAX_BYTES,
                )?;
                review.publication = coder::task::publish::last(&store, &task);
                Ok(Answer::Review(Box::new(review)))
            }
            // The person at this computer publishes their own run's change:
            // the same once-only publication a granted device asks the host
            // for.
            Request::Publish {
                task,
                base,
                head_commit,
                head,
            } => {
                let store = self.local().store().to_path_buf();
                let reviewed = coder::task::publish::Reviewed {
                    base,
                    head_commit,
                    head,
                };
                coder::task::publish::Publisher::new(&store, &coder::task::publish::GhForge)
                    .publish(&task, &reviewed)
                    .map(|publication| Answer::Published(Box::new(publication)))
                    .map_err(|refusal| match refusal {
                        coder::task::publish::Refusal::NoWorktree => {
                            "This task has no worktree on this computer.".to_owned()
                        }
                        coder::task::publish::Refusal::Store(why) => {
                            format!("Coder could not keep the publication's record: {why}")
                        }
                    })
            }
            Request::Choose => unreachable!("the folder chooser runs on the local lane"),
        }
    }
}

/// Starts the issue flow for `reference` in the checkout at `path` for the
/// chat, on a thread of its own that works it to its end; returns the
/// run's record once its first turn started. The chat follows the task as
/// any local run, and the flow's steps and issue link arrive in its
/// events.
fn start_issue(
    store: &std::path::Path,
    path: &std::path::Path,
    reference: coder::task::issue_run::Reference,
    chat: &str,
) -> Result<coder::task::local::Record, String> {
    let (sender, receiver) = std::sync::mpsc::channel();
    let (store, dir, chat) = (store.to_path_buf(), path.to_path_buf(), chat.to_owned());
    std::thread::spawn(move || {
        let runner = coder::task::issue_run::Runner::new(store);
        match runner.begin(&dir, &reference, Some(&chat)) {
            Ok(started) => {
                let _ = sender.send(Ok(started.record.clone()));
                let _ = started.finish();
            }
            Err(refused) => {
                let _ = sender.send(Err(format!(
                    "Coder did not take #{}: {refused}",
                    reference.number
                )));
            }
        }
    });
    receiver
        .recv()
        .unwrap_or_else(|_| Err("Coder could not start the issue flow.".into()))
}

/// Names a file every Coder event the window receives is appended to, as
/// NDJSON: the same lines `openagents --json chat follow` prints, for
/// comparing the two.
pub const EVENTS_VAR: &str = "OPENAGENTS_DESKTOP_CODER_EVENTS";

fn dump(lines: &[openagents_chat::coder_events::Line]) {
    use std::io::Write as _;
    let Some(path) = std::env::var_os(EVENTS_VAR).filter(|path| !path.is_empty()) else {
        return;
    };
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    else {
        return;
    };
    for line in lines {
        if let Ok(text) = serde_json::to_string(line) {
            let _ = writeln!(file, "{text}");
        }
    }
}

impl LocalLane {
    /// Coder's recent tasks, from the task store through `coder task list`.
    fn tasks(&self) -> Vec<Task> {
        if self.fake {
            return vec![
                Task {
                    title: "Fix the login test".into(),
                    status: "running".into(),
                    reason: None,
                },
                Task {
                    title: "Update the README".into(),
                    status: "finished".into(),
                    reason: None,
                },
            ];
        }
        let Some(coder) = &self.coder else {
            return Vec::new();
        };
        let Ok(output) = Command::new(coder).args(["task", "list"]).output() else {
            return Vec::new();
        };
        if !output.status.success() || output.stdout.len() > TASK_LIST_MAX {
            return Vec::new();
        }
        let archived = archived(&self.home.join(".openagents/tasks/archive.json"));
        parse_tasks(&output.stdout, &archived)
    }
}

/// The most bytes of the task store's archive record read.
const ARCHIVE_MAX: u64 = 1024 * 1024;

/// The tasks the owner archived (`coder task archive`, or a phone's
/// Archive): taken off every device's list, and off this one. The record
/// holds task IDs and reasons, nothing secret; an unreadable one hides
/// nothing.
pub fn archived(path: &std::path::Path) -> std::collections::BTreeSet<String> {
    use std::io::Read as _;
    let mut bytes = Vec::new();
    let read =
        std::fs::File::open(path).and_then(|file| file.take(ARCHIVE_MAX).read_to_end(&mut bytes));
    if read.is_err() {
        return Default::default();
    }
    serde_json::from_slice::<serde_json::Value>(&bytes)
        .ok()
        .and_then(|record| {
            record
                .get("tasks")
                .and_then(serde_json::Value::as_object)
                .map(|tasks| tasks.keys().cloned().collect())
        })
        .unwrap_or_default()
}

/// The recent tasks in `coder task list` output, leaving out `archived`
/// ones: running ones first, then the newest, at most [`RECENT_TASKS`].
pub fn parse_tasks(json: &[u8], archived: &std::collections::BTreeSet<String>) -> Vec<Task> {
    let Ok(serde_json::Value::Array(entries)) = serde_json::from_slice(json) else {
        return Vec::new();
    };
    let mut tasks: Vec<Task> = entries
        .iter()
        .filter(|entry| {
            entry
                .get("task_id")
                .and_then(serde_json::Value::as_str)
                .is_none_or(|id| !archived.contains(id))
        })
        .filter_map(|entry| {
            let title = entry.pointer("/intent/title")?.as_str()?;
            let status = entry.get("status")?.as_str()?;
            // A task the host ended before it started says why, and so
            // does one whose process ended under it (#10124).
            let owner_ended = entry
                .pointer("/run/result/ending")
                .and_then(serde_json::Value::as_str)
                == Some(coder::task::owner::OWNER_ENDED);
            let reason = entry
                .get("cancellation_reason")
                .and_then(serde_json::Value::as_str)
                .filter(|_| status == "cancelled")
                .map(|reason| reason.chars().take(160).collect())
                .or_else(|| owner_ended.then(|| coder::task::owner::OWNER_ENDED_TEXT.to_owned()));
            Some(Task {
                title: title.chars().take(80).collect(),
                status: status.into(),
                reason,
            })
        })
        .collect();
    tasks.reverse();
    tasks.sort_by_key(|task| task.status != "running");
    tasks.truncate(RECENT_TASKS);
    tasks
}

/// Polls the screen lock independently of host requests. A slow host cannot
/// delay hiding a pairing code, and the UI reads only a cached value.
pub struct ScreenLock {
    locked: Arc<AtomicBool>,
    _stop: Sender<()>,
}

impl ScreenLock {
    pub fn start(waker: Waker) -> Self {
        Self::with_probe(waker, Duration::from_secs(1), platform::screen_locked)
    }

    fn with_probe(
        waker: Waker,
        interval: Duration,
        mut probe: impl FnMut() -> bool + Send + 'static,
    ) -> Self {
        let locked = Arc::new(AtomicBool::new(false));
        let shared = locked.clone();
        let (stop, stopped) = channel();
        std::thread::spawn(move || {
            let mut previous = None;
            loop {
                let locked = probe();
                shared.store(locked, Ordering::Relaxed);
                if previous != Some(locked) {
                    previous = Some(locked);
                    waker.wake();
                }
                if stopped.recv_timeout(interval) != Err(std::sync::mpsc::RecvTimeoutError::Timeout)
                {
                    return;
                }
            }
        });
        Self {
            locked,
            _stop: stop,
        }
    }

    pub fn locked(&self) -> bool {
        self.locked.load(Ordering::Relaxed)
    }
}

/// A background worker: one thread a lane.
pub struct Worker {
    host: Sender<Request>,
    local: Sender<Request>,
    coder: Sender<Request>,
    outcomes: Receiver<Outcome>,
}

/// Runs `lane` on its own thread until the window goes.
fn spawn_lane(
    mut run: impl FnMut(Request) -> Option<Outcome> + Send + 'static,
    outbox: Sender<Outcome>,
    waker: Waker,
) -> Sender<Request> {
    let (requests, inbox) = channel::<Request>();
    std::thread::spawn(move || {
        while let Ok(request) = inbox.recv() {
            if let Some(outcome) = run(request) {
                if outbox.send(outcome).is_err() {
                    return;
                }
                waker.wake();
            }
        }
    });
    requests
}

impl Worker {
    /// Starts the lanes; each outcome wakes `waker`.
    pub fn start(context: Context, waker: Waker) -> Worker {
        let (outbox, outcomes) = channel();
        let Context {
            mut host,
            mut local,
            mut coder,
        } = context;
        Worker {
            host: spawn_lane(move |r| host.run(r), outbox.clone(), waker.clone()),
            local: spawn_lane(move |r| local.run(r), outbox.clone(), waker.clone()),
            coder: spawn_lane(move |r| coder.run(r), outbox, waker),
            outcomes,
        }
    }

    pub fn send(&self, request: Request) {
        let lane = if coder(&request) {
            &self.coder
        } else if local(&request) {
            &self.local
        } else {
            &self.host
        };
        let _ = lane.send(request);
    }

    /// Outcomes that arrived since the last call.
    pub fn outcomes(&self) -> Vec<Outcome> {
        self.outcomes.try_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slow_lock_probe_does_not_block_reads_and_changes_wake_the_window() {
        let (states, probe) = channel();
        let (wakes, changed) = channel();
        let monitor = ScreenLock::with_probe(
            Waker::new(move || {
                let _ = wakes.send(());
            }),
            Duration::from_millis(1),
            move || probe.recv().unwrap_or(false),
        );
        // The probe is waiting for a state while the UI can read its cache.
        assert!(!monitor.locked());
        states.send(true).expect("a lock state");
        changed
            .recv_timeout(Duration::from_secs(2))
            .expect("a wake");
        assert!(monitor.locked());
        states.send(false).expect("an unlock state");
        changed
            .recv_timeout(Duration::from_secs(2))
            .expect("a wake");
        assert!(!monitor.locked());
        drop(monitor);
        drop(states);
    }

    /// A slow start of Coder (an upgrade runs `coder host adopt`) runs on
    /// the local lane, so the window still hears the host meanwhile.
    #[cfg(unix)]
    #[test]
    fn a_slow_start_does_not_keep_the_window_from_the_host() {
        use openagents_desktop::model::Request;
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        // A `coder` that takes its time to answer `coder host help`.
        let coder = dir.path().join("coder");
        std::fs::write(&coder, "#!/bin/sh\nsleep 2\n").unwrap();
        std::fs::set_permissions(&coder, std::fs::Permissions::from_mode(0o755)).unwrap();
        let home = dir.path().join("home");
        std::fs::create_dir(&home).unwrap();
        let fake = FakeHost::new("Studio Mac", 1_790_000_000);
        let context = Context::new(Box::new(fake), None, None, Some(coder), home).released();
        let (wakes, woke) = channel();
        let worker = Worker::start(
            context,
            Waker::new(move || {
                let _ = wakes.send(());
            }),
        );
        worker.send(Request::Start);
        worker.send(Request::Refresh);
        woke.recv_timeout(Duration::from_secs(1))
            .expect("the host answers during the start");
        let first = worker.outcomes();
        assert!(
            matches!(first.as_slice(), [Outcome::Refreshed(Some(_))]),
            "{first:?}"
        );
        woke.recv_timeout(Duration::from_secs(10))
            .expect("the start finishes");
        assert!(matches!(
            worker.outcomes().as_slice(),
            [Outcome::Started(_)]
        ));
    }

    /// Against the in-process host, starting Coder registers, upgrades, and
    /// reads nothing: an earlier setup in the home folder stays as it was.
    #[test]
    fn the_fake_host_starts_nothing_on_this_computer() {
        use openagents_desktop::model::Request;
        let home = tempfile::tempdir().unwrap();
        let access = home.path().join(".openagents/coder-access");
        std::fs::create_dir_all(&access).unwrap();
        std::fs::write(access.join("access.json"), "{}").unwrap();
        let fake = FakeHost::new("Studio Mac", 1_790_000_000);
        let mut context = Context::new(
            Box::new(fake.clone()),
            Some(fake),
            None,
            Some(PathBuf::from("/nonexistent/coder")),
            home.path().to_path_buf(),
        );
        let Some(Outcome::Started(started)) = context.run(Request::Start) else {
            panic!("no start");
        };
        assert_eq!(started.agent, Agent::Enabled);
        assert_eq!(started.note, None);
        assert_eq!(std::fs::read(access.join("access.json")).unwrap(), b"{}");
        assert_eq!(std::fs::read_dir(&access).unwrap().count(), 1);
    }

    /// A chat's Coder run goes through the shared local runner: started in
    /// the first folder that is a checkout, followed from its first event,
    /// stopped, and continued; a folder that is not a checkout asks for
    /// one. The engine is a launcher that starts nothing.
    #[test]
    fn a_chat_run_starts_follows_stops_and_continues_through_the_local_runner() {
        use coder::task::autostart::{Engine, Launch, Launched};
        use coder::task::capacity::{Connection, Provider};
        use openagents_chat::coder_events::CoderEvent;
        use openagents_chat_app::coder_run::{Answer, Request as Run, State};

        struct Idle;
        impl Launch for Idle {
            fn launch(
                &self,
                _: &Engine,
                _: &std::path::Path,
                _: &std::path::Path,
            ) -> Result<Launched, String> {
                Ok(Launched {
                    owner_process: std::process::id(),
                    grant_digest: String::new(),
                })
            }
        }
        fn signed_in(_: Provider) -> Connection {
            Connection::Connected
        }
        let dir = tempfile::tempdir().unwrap();
        let top = dir.path().join("slugs");
        std::fs::create_dir_all(&top).unwrap();
        for args in [
            vec!["init", "-q"],
            vec![
                "-c",
                "user.name=F",
                "-c",
                "user.email=f@example.invalid",
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                "one",
            ],
        ] {
            assert!(
                Command::new("git")
                    .args(args)
                    .current_dir(&top)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        let store = dir.path().join("tasks");
        let fake = FakeHost::new("Studio Mac", 1_790_000_000);
        let mut context = Context::new(
            Box::new(fake.clone()),
            None,
            None,
            None,
            dir.path().to_path_buf(),
        )
        .with_coder(
            coder::task::local::Local::new(store.clone())
                .with_probe(signed_in)
                .with_controller(std::env::current_exe().unwrap())
                .with_launcher(Box::new(Idle)),
        );
        let chat = "c".repeat(32);
        let mut ask = |request: Run| match context.run(Request::CoderRun {
            chat: chat.clone(),
            ticket: 1,
            request,
        }) {
            Some(Outcome::CoderRun { result, .. }) => *result,
            other => panic!("{other:?}"),
        };
        let empty = dir.path().join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        let Ok(Answer::NeedsProject { why }) = ask(Run::Start {
            title: "t".into(),
            prompt: "add a test".into(),
            dirs: vec![empty.display().to_string()],
            images: vec![],
            engine: None,
        }) else {
            panic!("a folder that is not a checkout asks for one")
        };
        assert!(why.contains("is not a Git checkout"), "{why}");
        let Ok(Answer::Started { task, project, .. }) = ask(Run::Start {
            title: "add a test".into(),
            prompt: "add a test".into(),
            dirs: vec![empty.display().to_string(), top.display().to_string()],
            images: vec![],
            engine: None,
        }) else {
            panic!("start")
        };
        assert_eq!(project, "slugs");
        assert_eq!(
            std::fs::read_to_string(store.join("local/last-project")).unwrap(),
            top.canonicalize().unwrap().display().to_string()
        );
        let Ok(Answer::Lines { lines, state }) = ask(Run::Poll { task: task.clone() }) else {
            panic!("poll")
        };
        assert_eq!(state, State::Running);
        let CoderEvent::CoderStarted(started) = &lines[0].event else {
            panic!("{lines:?}")
        };
        assert_eq!(
            (started.provider.as_str(), started.via.as_str()),
            ("codex", "local")
        );
        assert_eq!(lines[0].thread.as_deref(), Some(chat.as_str()));
        assert_eq!(ask(Run::Stop { task: task.clone() }), Ok(Answer::Stopping));
        let Ok(Answer::Lines { lines, state }) = ask(Run::Poll { task: task.clone() }) else {
            panic!("poll")
        };
        assert_eq!(state, State::Ended);
        assert_eq!(lines.last().unwrap().event.name(), "stopped");
        // The same follower goes on: the stopped task continues.
        assert_eq!(
            ask(Run::Continue {
                task: task.clone(),
                text: "go on".into()
            }),
            Ok(Answer::Continued)
        );
        // With no project given, the last one Coder used.
        let Ok(Answer::Started { project, .. }) = ask(Run::Start {
            title: "again".into(),
            prompt: "again".into(),
            dirs: vec![],
            images: vec![],
            engine: None,
        }) else {
            panic!("start in the last project")
        };
        assert_eq!(project, "slugs");
    }

    /// The Start a chat's send with a screenshot leads to (#10070) keeps
    /// the screenshot's exact bytes with the task on this computer, named
    /// by the task's intent, for the engine (here a launcher that starts
    /// nothing) to read back checked.
    #[test]
    fn a_chat_runs_start_keeps_the_screenshots_exact_bytes_for_the_engine() {
        use coder::task::autostart::{Engine, Launch, Launched};
        use coder::task::capacity::{Connection, Provider};
        use openagents_chat_app::coder_run::{Answer, Request as Run};
        struct Idle;
        impl Launch for Idle {
            fn launch(
                &self,
                _: &Engine,
                _: &std::path::Path,
                _: &std::path::Path,
            ) -> Result<Launched, String> {
                Ok(Launched {
                    owner_process: std::process::id(),
                    grant_digest: String::new(),
                })
            }
        }
        fn signed_in(_: Provider) -> Connection {
            Connection::Connected
        }
        let dir = tempfile::tempdir().unwrap();
        let top = dir.path().join("site");
        std::fs::create_dir_all(&top).unwrap();
        for args in [
            vec!["init", "-q"],
            vec![
                "-c",
                "user.name=F",
                "-c",
                "user.email=f@example.invalid",
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                "one",
            ],
        ] {
            assert!(
                Command::new("git")
                    .args(args)
                    .current_dir(&top)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        let store = dir.path().join("tasks");
        let mut context = Context::new(
            Box::new(FakeHost::new("Studio Mac", 1_790_000_000)),
            None,
            None,
            None,
            dir.path().to_path_buf(),
        )
        .with_coder(
            coder::task::local::Local::new(store.clone())
                .with_probe(signed_in)
                .with_controller(std::env::current_exe().unwrap())
                .with_launcher(Box::new(Idle)),
        );
        // The draft's screenshot, as the chat's start carries it.
        let mut drafts = openagents_chat_app::attachments::Drafts::default();
        let pixels: Vec<u8> = (0..40u32 * 30 * 4).map(|i| (i % 249) as u8).collect();
        let image = openagents_chat_app::attachments::Image::pixels(40, 30, pixels).unwrap();
        let bytes = image.bytes.as_ref().clone();
        drafts.add("chat", image).unwrap();
        let images = drafts.uploads("chat").unwrap();
        let reference = images[0].reference.clone();
        let Some(Outcome::CoderRun { result, .. }) = context.run(Request::CoderRun {
            chat: "e".repeat(32),
            ticket: 1,
            request: Run::Start {
                title: "Fix this layout bug".into(),
                prompt: "fix this layout bug".into(),
                dirs: vec![top.display().to_string()],
                images,
                engine: None,
            },
        }) else {
            panic!("an answer")
        };
        let Ok(Answer::Started { task, .. }) = *result else {
            panic!("{result:?}")
        };
        assert_eq!(
            coder::task::media::load(&store, &task, &reference).unwrap(),
            bytes
        );
    }

    /// The desktop's local run honors the same settings as
    /// `openagents chat` (#10036): the allowed providers and their order,
    /// and the project folders: a checkout outside them is not a project.
    #[test]
    fn a_chat_run_follows_the_local_capability_settings() {
        use coder::task::autostart::{Engine, Launch, Launched};
        use coder::task::capacity::{Connection, Provider};
        use coder::task::settings;
        use openagents_chat::coder_events::CoderEvent;
        use openagents_chat_app::coder_run::{Answer, Request as Run};
        struct Idle;
        impl Launch for Idle {
            fn launch(
                &self,
                _: &Engine,
                _: &std::path::Path,
                _: &std::path::Path,
            ) -> Result<Launched, String> {
                Ok(Launched {
                    owner_process: std::process::id(),
                    grant_digest: String::new(),
                })
            }
        }
        fn signed_in(_: Provider) -> Connection {
            Connection::Connected
        }
        let dir = tempfile::tempdir().unwrap();
        let checkout = |name: &str| {
            let top = dir.path().join(name);
            std::fs::create_dir_all(&top).unwrap();
            for args in [
                vec!["init", "-q"],
                vec![
                    "-c",
                    "user.name=F",
                    "-c",
                    "user.email=f@example.invalid",
                    "commit",
                    "-q",
                    "--allow-empty",
                    "-m",
                    "one",
                ],
            ] {
                assert!(
                    Command::new("git")
                        .args(args)
                        .current_dir(&top)
                        .status()
                        .unwrap()
                        .success()
                );
            }
            top.canonicalize().unwrap()
        };
        let slugs = checkout("slugs");
        let allowed = dir.path().join("code");
        std::fs::create_dir_all(&allowed).unwrap();
        let parser = {
            let top = checkout("code/parser");
            assert!(top.starts_with(allowed.canonicalize().unwrap()));
            top
        };
        let settings = settings::Coder {
            providers: vec![
                settings::Choice::new(Provider::Claude),
                settings::Choice::new(Provider::Codex),
            ],
            // Agents are opt-out (#10184): only these two stay on.
            disabled: vec![Provider::Grok, Provider::Devin, Provider::OpenCode],
            projects: vec![allowed.canonicalize().unwrap()],
            ..settings::Coder::default()
        };
        let mut context = Context::new(
            Box::new(FakeHost::new("Studio Mac", 1_790_000_000)),
            None,
            None,
            None,
            dir.path().to_path_buf(),
        )
        .with_coder(
            coder::task::local::Local::new(dir.path().join("tasks"))
                .with_settings(settings)
                .with_probe(signed_in)
                .with_controller(std::env::current_exe().unwrap())
                .with_launcher(Box::new(Idle)),
        );
        let chat = "d".repeat(32);
        let mut ask = |request: Run| match context.run(Request::CoderRun {
            chat: chat.clone(),
            ticket: 1,
            request,
        }) {
            Some(Outcome::CoderRun { result, .. }) => *result,
            other => panic!("{other:?}"),
        };
        // A checkout outside the project folders is not a project.
        let Ok(Answer::NeedsProject { why }) = ask(Run::Start {
            title: "t".into(),
            prompt: "add a test".into(),
            dirs: vec![slugs.display().to_string()],
            images: vec![],
            engine: None,
        }) else {
            panic!("a checkout outside the project folders asks for one")
        };
        assert!(
            why.contains("is not in one of your project folders"),
            "{why}"
        );
        // A checkout inside them runs, on Claude Code first, as the
        // settings order it.
        let Ok(Answer::Started { task, project, .. }) = ask(Run::Start {
            title: "add a test".into(),
            prompt: "add a test".into(),
            dirs: vec![parser.display().to_string()],
            images: vec![],
            engine: None,
        }) else {
            panic!("start in a project folder")
        };
        assert_eq!(project, "parser");
        let Ok(Answer::Lines { lines, .. }) = ask(Run::Poll { task }) else {
            panic!("poll")
        };
        let CoderEvent::CoderStarted(started) = &lines[0].event else {
            panic!("{lines:?}")
        };
        assert_eq!(started.provider, "claude");
        assert_eq!(started.reason, "Claude Code is signed in and has capacity.");
        assert_eq!(started.fallbacks, vec!["codex:gpt-6.1-sol".to_owned()]);
    }

    #[test]
    fn tasks_list_running_first_then_newest() {
        let json = br#"[
            {"task_id":"a","intent":{"title":"Oldest"},"status":"finished"},
            {"task_id":"b","intent":{"title":"Working one"},"status":"running"},
            {"task_id":"c","intent":{"title":"Newest"},"status":"finished"},
            {"task_id":"d","status":"finished"}
        ]"#;
        let titles: Vec<String> = parse_tasks(json, &Default::default())
            .into_iter()
            .map(|t| t.title)
            .collect();
        assert_eq!(titles, vec!["Working one", "Newest", "Oldest"]);
        assert!(parse_tasks(b"not json", &Default::default()).is_empty());
    }

    /// Tasks the owner archived stay off the list, as they do on phones.
    #[test]
    fn archived_tasks_are_not_listed() {
        let dir = tempfile::tempdir().unwrap();
        let record = dir.path().join("archive.json");
        std::fs::write(
            &record,
            br#"{"schema":"openagents.coder.task-archive.v1","tasks":{"b":{"at":1,"reason":"Agent smoke","by":{"kind":"owner"}}}}"#,
        )
        .unwrap();
        let json = br#"[
            {"task_id":"a","intent":{"title":"Fix the login test"},"status":"finished"},
            {"task_id":"b","intent":{"title":"Run the command `sleep 45`"},"status":"finished"}
        ]"#;
        let titles: Vec<String> = parse_tasks(json, &archived(&record))
            .into_iter()
            .map(|t| t.title)
            .collect();
        assert_eq!(titles, ["Fix the login test"]);
        assert!(archived(&dir.path().join("missing.json")).is_empty());
    }

    #[test]
    fn a_task_that_never_started_says_why() {
        let json = br#"[
            {"task_id":"a","intent":{"title":"Fix it"},"status":"cancelled",
             "cancellation_reason":"Couldn't start: Claude Code isn't set up on this computer."},
            {"task_id":"b","intent":{"title":"Waiting"},"status":"queued",
             "cancellation_reason":null}
        ]"#;
        let tasks = parse_tasks(json, &Default::default());
        assert_eq!(tasks[1].title, "Fix it");
        assert_eq!(
            tasks[1].reason.as_deref(),
            Some("Couldn't start: Claude Code isn't set up on this computer.")
        );
        assert_eq!(tasks[0].reason, None);
    }

    #[test]
    fn a_task_whose_process_ended_says_so() {
        let json = br#"[
            {"task_id":"a","intent":{"title":"Fix it"},"status":"finished",
             "run":{"result":{"ending":"owner_process_ended"}}},
            {"task_id":"b","intent":{"title":"Done"},"status":"finished",
             "run":{"result":{"ending":"completed"}}}
        ]"#;
        let tasks = parse_tasks(json, &Default::default());
        assert_eq!(tasks[1].title, "Fix it");
        assert_eq!(
            tasks[1].reason.as_deref(),
            Some("Coder's process ended unexpectedly")
        );
        assert_eq!(tasks[0].reason, None);
    }
}
