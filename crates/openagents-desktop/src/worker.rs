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
use openagents_desktop::codes::Action;
use openagents_desktop::control::{ControlError, HostControl, PickError};
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

/// What the worker needs: the host lane and the local lane.
pub struct Context {
    host: HostLane,
    local: LocalLane,
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
    coder: Option<PathBuf>,
    home: PathBuf,
}

/// Whether `request` runs on the local lane.
fn local(request: &Request) -> bool {
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
    pub fn new(
        control: Box<dyn HostControl>,
        fake: Option<FakeHost>,
        fake_scan: Option<Duration>,
        coder: Option<PathBuf>,
        home: PathBuf,
    ) -> Context {
        Context {
            local: LocalLane {
                fake: fake.is_some(),
                coder,
                home,
            },
            host: HostLane {
                control,
                fake,
                fake_scan,
                first_code: None,
            },
        }
    }

    /// Runs one request.
    pub fn run(&mut self, request: Request) -> Option<Outcome> {
        if local(&request) {
            self.local.run(request)
        } else {
            self.host.run(request)
        }
    }
}

impl LocalLane {
    fn run(&mut self, request: Request) -> Option<Outcome> {
        match request {
            Request::Copy { code } => platform::copy(&code).then_some(Outcome::Copied),
            Request::ClearClipboard { code } => {
                platform::clear_if(&code);
                None
            }
            Request::ChooseFolder => Some(Outcome::Folder(platform::choose_folder())),
            Request::Coder => Some(Outcome::Coder {
                agents: if self.fake {
                    Agents {
                        codex: true,
                        claude: false,
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
        openagents_desktop::migrate::start(self.coder.as_deref(), &self.home, &mut |keys| {
            platform::register_agent(keys)
        })
    }
}

impl HostLane {
    fn run(&mut self, request: Request) -> Option<Outcome> {
        match request {
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
            } => openagents_desktop::control::pick_project(
                self.control.as_mut(),
                &path.to_string_lossy(),
                replace.as_deref(),
                autostart,
            )
            .err()
            .map(|error| Outcome::Failed {
                message: match error {
                    PickError::Folder => "Coder couldn't use that folder. Choose another one.",
                    PickError::Setting => "Couldn't change that setting. Try again.",
                }
                .into(),
            }),
            Request::SetAutostart(policy) => {
                self.control
                    .set_autostart(policy)
                    .err()
                    .map(|_| Outcome::Failed {
                        message: "Couldn't change that setting. Try again.".into(),
                    })
            }
            Request::NearbyDecide { id, connect } => self
                .control
                .nearby_decide(id, connect)
                .err()
                .map(|_| Outcome::Failed {
                    message: "That phone stopped asking. Ask again from the phone.".into(),
                }),
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
        })
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
            // A task the host ended before it started says why.
            let reason = entry
                .get("cancellation_reason")
                .and_then(serde_json::Value::as_str)
                .filter(|_| status == "cancelled")
                .map(|reason| reason.chars().take(160).collect());
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
        } = context;
        Worker {
            host: spawn_lane(move |r| host.run(r), outbox.clone(), waker.clone()),
            local: spawn_lane(move |r| local.run(r), outbox, waker),
            outcomes,
        }
    }

    pub fn send(&self, request: Request) {
        let lane = if local(&request) {
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
        let context = Context::new(Box::new(fake), None, None, Some(coder), home);
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
}
