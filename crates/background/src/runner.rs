//! The runner the host starts: one per computer (`runner.lock`), on its own
//! thread. It checks free space only on the triggers the rule names: at
//! host start (`HostStart`), on the rule's own `Interval`, and when a Coder
//! task ends (`TaskEnded`); a rule with no triggers never runs by itself.
//! A check runs the rule when a volume is below its start level and the
//! cooldown has passed (or at once in an emergency). A run someone asks for
//! (`openagents background run`, `/background`, `background.run`) is
//! separate and always allowed.

use std::collections::BTreeSet;
use std::fs::File;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::Duration;

use crate::inuse::System;
use crate::paths::{self, Layout};
use crate::plan::{Env, Facts, observe};
use crate::rule::{Rule, Trigger};
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
        // No interval trigger, no interval checks. The rule is read again
        // after every wake, so an edit takes effect without a restart.
        let mut next = self.interval().map(|every| paths::now() + every);
        let mut next_tasks = paths::now() + TASK_POLL.as_secs();
        loop {
            let deadline = next.map_or(next_tasks, |at| at.min(next_tasks));
            let wait = deadline.saturating_sub(paths::now()).max(1);
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
            next = match (self.interval(), next) {
                (None, _) => None,
                (Some(every), None) => Some(now + every),
                (Some(every), Some(at)) if now >= at => {
                    self.check(Cause::Interval);
                    Some(now + every)
                }
                (Some(every), Some(at)) => Some(at.min(now + every)),
            };
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

    /// The rule's interval, if it has an `Interval` trigger.
    fn interval(&self) -> Option<u64> {
        store::load(&self.layout, "disk")
            .ok()
            .and_then(|rule| rule.interval())
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
        if let Some(result) = check(&self.env(), &rule, cause) {
            self.finish(&rule.id, result);
        }
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

/// Whether an evaluation with `cause` may happen for `rule`: a run someone
/// asked for always may; an automatic one only on a trigger the rule names.
/// `Interval` (and `Threshold`, which is checked on the interval) need an
/// `Interval` trigger; `HostStart` and `TaskEnded` need their own.
#[must_use]
pub fn fires(rule: &Rule, cause: Cause) -> bool {
    match cause {
        Cause::Manual => true,
        Cause::Interval | Cause::Threshold => rule.interval().is_some(),
        Cause::HostStart => rule.has(&Trigger::HostStart),
        Cause::TaskEnded => rule.has(&Trigger::TaskEnded),
    }
}

/// One automatic check of `rule` for `cause`: nothing unless the rule names
/// that trigger ([`fires`]); otherwise observe, remember the free space, and
/// run when [`decide`] says so. `None` when nothing ran.
pub fn check(env: &Env<'_>, rule: &Rule, cause: Cause) -> Option<Result<Report, String>> {
    if cause == Cause::Manual || !fires(rule, cause) {
        return None;
    }
    let now = env.now;
    let volumes = observe(env, rule);
    let fullest = volumes.iter().min_by_key(|volume| volume.space.free);
    let state = State::load(env.layout)
        .rules
        .get(&rule.id)
        .cloned()
        .unwrap_or_default();
    State::update(env.layout, &rule.id, |state| {
        state.last_check = Some(now);
        state.next_check = rule.interval().map(|every| now + every);
        state.free = fullest.map(|volume| volume.space.free);
        state.total = fullest.map(|volume| volume.space.total);
    });
    let spaces: Vec<(u64, u64)> = volumes
        .iter()
        .map(|volume| (volume.space.free, volume.space.total))
        .collect();
    decide(rule, &spaces, state.last_run, now)?;
    // An interval check that finds space low is the threshold firing.
    let cause = if cause == Cause::Interval && rule.has(&Trigger::Threshold) {
        Cause::Threshold
    } else {
        cause
    };
    Some(run::run(env, rule, cause, false, false))
}

/// Whether a triggered check runs the rule now, and if so whether it is an
/// emergency: some volume (free, total) is below the start level, the rule
/// is active, and the cooldown has passed or free space is below the
/// emergency level.
#[must_use]
pub fn decide(
    rule: &Rule,
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
