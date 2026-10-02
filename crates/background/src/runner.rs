//! The runner the host starts: one per computer (`runner.lock`), on its own
//! thread. It checks free space at host start, on the rule's interval, and
//! when a Coder task ends; it runs the rule when a volume is below its
//! start level and the cooldown has passed (or at once in an emergency),
//! and runs it whenever someone asks.

use std::collections::BTreeSet;
use std::fs::File;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::Duration;

use crate::inuse::System;
use crate::paths::{self, Layout};
use crate::plan::{Env, Facts, observe};
use crate::run::{self, Cause, Report};
use crate::store::{self, State};
use crate::volume::Statvfs;

/// A request to the runner.
enum Request {
    Run { rule: String },
    TaskEnded,
}

/// Talks to a running runner.
#[derive(Clone)]
pub struct Handle {
    sender: Sender<Request>,
}

impl Handle {
    /// Run `rule` now on the runner's thread. The result goes to the log
    /// and the rule's state. (A dry run is computed where it is asked for:
    /// it changes nothing, so it needs no runner.)
    pub fn run(&self, rule: &str) {
        let _ = self.sender.send(Request::Run { rule: rule.into() });
    }

    /// A Coder task ended: check now.
    pub fn task_ended(&self) {
        let _ = self.sender.send(Request::TaskEnded);
    }
}

/// How the runner says what happened: the host prints it to its log.
pub type Say = Box<dyn Fn(&str) + Send>;

/// Start the runner on its own thread.
#[must_use]
pub fn start(layout: Layout, facts: Option<Arc<dyn Facts>>, say: Say) -> Handle {
    let (sender, receiver) = channel();
    let _ = std::thread::Builder::new()
        .name("background".into())
        .spawn(move || Runner::new(layout, facts, say).serve(&receiver));
    Handle { sender }
}

struct Runner {
    layout: Layout,
    facts: Option<Arc<dyn Facts>>,
    say: Say,
    ended: Option<BTreeSet<String>>,
}

/// How often the runner looks for ended tasks.
const TASK_POLL: Duration = Duration::from_secs(30);
/// How long after the host starts it makes the first check.
const START_DELAY: Duration = Duration::from_secs(20);

impl Runner {
    fn new(layout: Layout, facts: Option<Arc<dyn Facts>>, say: Say) -> Self {
        Self {
            layout,
            facts,
            say,
            ended: None,
        }
    }

    fn serve(mut self, requests: &Receiver<Request>) {
        // One runner per computer: wait for the lock another host holds.
        let _lock = loop {
            if let Some(lock) = self.runner_lock() {
                break lock;
            }
            match requests.recv_timeout(Duration::from_secs(60)) {
                Err(RecvTimeoutError::Disconnected) => return,
                Ok(Request::Run { rule }) => {
                    // Another process is the runner; a request made here
                    // still runs, under the run lock.
                    self.manual(&rule);
                }
                _ => {}
            }
        };
        let pid = std::process::id();
        State::update(&self.layout, "disk", |state| state.runner = Some(pid));
        std::thread::sleep(START_DELAY);
        self.check(Cause::HostStart);
        let mut next = paths::now() + self.interval();
        let mut next_tasks = paths::now() + TASK_POLL.as_secs();
        loop {
            let wait = next.min(next_tasks).saturating_sub(paths::now()).max(1);
            match requests.recv_timeout(Duration::from_secs(wait)) {
                Ok(Request::Run { rule }) => self.manual(&rule),
                Ok(Request::TaskEnded) => self.check(Cause::TaskEnded),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            let now = paths::now();
            if now >= next_tasks {
                next_tasks = now + TASK_POLL.as_secs();
                if self.task_ended() {
                    self.check(Cause::TaskEnded);
                }
            }
            if now >= next {
                next = now + self.interval();
                self.check(Cause::Interval);
            }
        }
    }

    fn runner_lock(&self) -> Option<File> {
        std::fs::create_dir_all(self.layout.background()).ok()?;
        let file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.layout.runner_lock())
            .ok()?;
        file.try_lock().ok().map(|()| file)
    }

    fn interval(&self) -> u64 {
        store::load(&self.layout, "disk")
            .ok()
            .and_then(|rule| rule.interval())
            .unwrap_or(300)
    }

    /// Whether a task ended since the last look.
    fn task_ended(&mut self) -> bool {
        let Some(facts) = &self.facts else {
            return false;
        };
        let Ok(tasks) = facts.tasks() else {
            return false;
        };
        let ended: BTreeSet<String> = tasks
            .into_iter()
            .filter(|task| task.ended)
            .map(|task| task.id)
            .collect();
        let new = self
            .ended
            .as_ref()
            .is_some_and(|before| ended.difference(before).next().is_some());
        self.ended = Some(ended);
        new
    }

    fn env(&self) -> Env<'_> {
        Env {
            layout: &self.layout,
            facts: self.facts.as_deref(),
            volumes: &Statvfs,
            processes: &System,
            now: paths::now(),
        }
    }

    fn manual(&self, id: &str) {
        let Ok(rule) = store::load(&self.layout, id) else {
            return;
        };
        let result = run::run(&self.env(), &rule, Cause::Manual, false, true);
        self.finish(&rule.id, result);
    }

    fn check(&self, cause: Cause) {
        let Ok(rule) = store::load(&self.layout, "disk") else {
            return;
        };
        let env = self.env();
        let now = env.now;
        let volumes = observe(&env, &rule);
        let fullest = volumes.iter().min_by_key(|volume| volume.space.free);
        let state = State::load(&self.layout)
            .rules
            .get(&rule.id)
            .cloned()
            .unwrap_or_default();
        State::update(&self.layout, &rule.id, |state| {
            state.last_check = Some(now);
            state.next_check = Some(now + rule.interval().unwrap_or(300));
            state.free = fullest.map(|volume| volume.space.free);
            state.total = fullest.map(|volume| volume.space.total);
        });
        let spaces: Vec<(u64, u64)> = volumes
            .iter()
            .map(|volume| (volume.space.free, volume.space.total))
            .collect();
        if decide(&rule, &spaces, state.last_run, now).is_none() {
            return;
        }
        let cause = if cause == Cause::Interval {
            Cause::Threshold
        } else {
            cause
        };
        let result = run::run(&env, &rule, cause, false, false);
        self.finish(&rule.id, result);
    }

    fn finish(&self, id: &str, result: Result<Report, String>) {
        let report = match result {
            Ok(report) => report,
            Err(why) => {
                (self.say)(&format!("background {id}: {why}"));
                return;
            }
        };
        crate::view::remember(&self.layout, id, &report);
        let notice = report.notice.clone();
        if let Some(line) = notice {
            (self.say)(&line);
        }
    }
}

/// Whether a triggered check runs the rule now, and if so whether it is an
/// emergency: some volume (free, total) is below the start level, the rule
/// is active, and the cooldown has passed or free space is below the
/// emergency level.
#[must_use]
pub fn decide(
    rule: &crate::rule::Rule,
    volumes: &[(u64, u64)],
    last_run: Option<u64>,
    now: u64,
) -> Option<bool> {
    if !rule.active(now) {
        return None;
    }
    let low = volumes
        .iter()
        .any(|&(free, total)| free < rule.goal.start.of(total));
    let emergency = volumes
        .iter()
        .any(|&(free, total)| free < rule.goal.emergency.of(total));
    let cooling = last_run.is_some_and(|last| now < last + rule.cooldown_secs);
    (low && (emergency || !cooling)).then_some(emergency)
}
