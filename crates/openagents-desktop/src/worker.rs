//! Runs the model's requests off the UI thread.
//!
//! The worker owns the control client, so a slow host never stalls a
//! frame: each [`Request`] becomes at most one [`Outcome`], sent back with
//! a wake of the event loop. `run` is the same handling, inline, for the
//! capture mode and tests.

use crate::mac;
use openagents_desktop::codes::Action;
use openagents_desktop::control::{ControlError, HostControl};
use openagents_desktop::fake::FakeHost;
use openagents_desktop::model::{Agent, Outcome, Refreshed, Request, Task};
use rust_native_desktop::Waker;
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc::{Receiver, Sender, channel};
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
            Request::Code(Action::Create { ticket, terminal }) => {
                Some(match self.control.invite(terminal) {
                    Ok(invite) => {
                        self.first_code.get_or_insert_with(Instant::now);
                        Outcome::Created {
                            ticket,
                            invitation: invite.invitation,
                            code: invite.code,
                            terminal,
                        }
                    }
                    Err(_) => Outcome::CreateFailed { ticket },
                })
            }
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
            Request::Copy { code } => mac::copy(&code).then_some(Outcome::Copied),
            Request::ClearClipboard { code } => {
                mac::clear_if(&code);
                None
            }
            Request::ChooseFolder => Some(Outcome::Folder(mac::choose_folder())),
            Request::Coder => Some(Outcome::Coder {
                agents: mac::signed_in(&self.home),
                tasks: self.tasks(),
            }),
            Request::OpenLoginItems => {
                mac::open_login_items();
                None
            }
            Request::Adopt => Some(Outcome::Adopted(self.adopt())),
            Request::NearbyDecide {
                id,
                connect,
                terminal,
            } => self
                .control
                .nearby_decide(id, connect, terminal)
                .err()
                .map(|_| Outcome::Failed {
                    message: "That phone stopped asking. Ask again from the phone.".into(),
                }),
        }
    }

    /// Adopts the old-style setup with `coder host adopt`, run as a child,
    /// then registers this app's login agent.
    fn adopt(&self) -> Result<(), String> {
        let Some(coder) = &self.coder else {
            return Err(
                "Couldn't move your setup. Coder keeps running as it was. Try again later.".into(),
            );
        };
        openagents_desktop::migrate::adopt(coder, &self.home, &mut || match mac::register_agent() {
            Agent::Enabled | Agent::NeedsApproval => Ok(()),
            Agent::NotRegistered => Err("this is not the OpenAgents app bundle".into()),
            Agent::Failed(message) => Err(message),
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
                },
                Task {
                    title: "Update the README".into(),
                    status: "finished".into(),
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
        parse_tasks(&output.stdout)
    }
}

/// The recent tasks in `coder task list` output: running ones first, then
/// the newest, at most [`RECENT_TASKS`].
pub fn parse_tasks(json: &[u8]) -> Vec<Task> {
    let Ok(serde_json::Value::Array(entries)) = serde_json::from_slice(json) else {
        return Vec::new();
    };
    let mut tasks: Vec<Task> = entries
        .iter()
        .filter_map(|entry| {
            let title = entry.pointer("/intent/title")?.as_str()?;
            let status = entry.get("status")?.as_str()?;
            Some(Task {
                title: title.chars().take(80).collect(),
                status: status.into(),
            })
        })
        .collect();
    tasks.reverse();
    tasks.sort_by_key(|task| task.status != "running");
    tasks.truncate(RECENT_TASKS);
    tasks
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
    fn tasks_list_running_first_then_newest() {
        let json = br#"[
            {"task_id":"a","intent":{"title":"Oldest"},"status":"finished"},
            {"task_id":"b","intent":{"title":"Working one"},"status":"running"},
            {"task_id":"c","intent":{"title":"Newest"},"status":"finished"},
            {"task_id":"d","status":"finished"}
        ]"#;
        let titles: Vec<String> = parse_tasks(json).into_iter().map(|t| t.title).collect();
        assert_eq!(titles, vec!["Working one", "Newest", "Oldest"]);
        assert!(parse_tasks(b"not json").is_empty());
    }
}
