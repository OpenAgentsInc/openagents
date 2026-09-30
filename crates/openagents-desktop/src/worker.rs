//! Runs the model's requests off the UI thread.
//!
//! The worker owns the control client, so a slow host never stalls a
//! frame: each [`Request`] becomes at most one [`Outcome`], sent back with
//! a wake of the event loop. `run` is the same handling, inline, for the
//! capture mode and tests.

use crate::platform;
use openagents_desktop::codes::Action;
use openagents_desktop::control::{ControlError, HostControl};
use openagents_desktop::fake::FakeHost;
use openagents_desktop::model::{Outcome, Refreshed, Request, Started, Task};
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

/// What the worker needs besides the control client.
pub struct Context {
    pub control: Box<dyn HostControl>,
    /// The in-process host, in `--fake-host` mode.
    pub fake: Option<FakeHost>,
    /// In `--fake-host` mode, a phone scans the code this long after it
    /// first shows.
    pub fake_scan: Option<Duration>,
    pub coder: Option<PathBuf>,
    pub home: PathBuf,
    first_code: Option<Instant>,
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
            control,
            fake,
            fake_scan,
            coder,
            home,
            first_code: None,
        }
    }

    /// Runs one request.
    pub fn run(&mut self, request: Request) -> Option<Outcome> {
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
            Request::AddProject { path } => self
                .control
                .add_project(&path.to_string_lossy())
                .err()
                .map(|_| Outcome::Failed {
                    message: "Coder couldn't use that folder. Choose another one.".into(),
                }),
            Request::SetAutostart(policy) => {
                self.control
                    .set_autostart(policy)
                    .err()
                    .map(|_| Outcome::Failed {
                        message: "Couldn't change that setting. Try again.".into(),
                    })
            }
            Request::Copy { code } => platform::copy(&code).then_some(Outcome::Copied),
            Request::ClearClipboard { code } => {
                platform::clear_if(&code);
                None
            }
            Request::ChooseFolder => Some(Outcome::Folder(platform::choose_folder())),
            Request::Coder => Some(Outcome::Coder {
                agents: platform::signed_in(&self.home),
                tasks: self.tasks(),
            }),
            Request::OpenLoginItems => {
                platform::open_login_items();
                None
            }
            Request::Start => Some(Outcome::Started(self.start())),
            Request::NearbyDecide { id, connect } => self
                .control
                .nearby_decide(id, connect)
                .err()
                .map(|_| Outcome::Failed {
                    message: "That phone stopped asking. Ask again from the phone.".into(),
                }),
        }
    }

    /// Starts Coder under this app, upgrading an earlier setup silently
    /// first ([`openagents_desktop::migrate::start`]).
    fn start(&self) -> Started {
        openagents_desktop::migrate::start(self.coder.as_deref(), &self.home, &mut |keys| {
            platform::register_agent(keys)
        })
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

    /// Coder's recent tasks, from the task store through `coder task list`.
    fn tasks(&self) -> Vec<Task> {
        if self.fake.is_some() {
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

/// A background worker.
pub struct Worker {
    requests: Sender<Request>,
    outcomes: Receiver<Outcome>,
}

impl Worker {
    /// Starts the worker thread; each outcome wakes `waker`.
    pub fn start(mut context: Context, waker: Waker) -> Worker {
        let (requests, inbox) = channel::<Request>();
        let (outbox, outcomes) = channel();
        std::thread::spawn(move || {
            while let Ok(request) = inbox.recv() {
                if let Some(outcome) = context.run(request) {
                    if outbox.send(outcome).is_err() {
                        return;
                    }
                    waker.wake();
                }
            }
        });
        Worker { requests, outcomes }
    }

    pub fn send(&self, request: Request) {
        let _ = self.requests.send(request);
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
