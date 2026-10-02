//! The runner the host starts: one per computer (`runner.lock`), on its own
//! thread. It evaluates each rule only on the triggers the rule names: at
//! host start (`HostStart`), on the rule's own `Interval` (jittered by up
//! to 10%), when a Coder task ends (`TaskEnded`, once per ended task for a
//! rule that reads the outcome), at a local time (`Daily`, catching up
//! once after sleep), and when a watched path changes (`FsEvent`, looked
//! at every 30 seconds); a rule with no triggers never runs by itself.
//! A check runs the rule when a volume is below its start level and the
//! cooldown has passed (or at once in an emergency). A run someone asks for
//! (`openagents background run`, `/background`, `background.run`) is
//! separate and always allowed.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::Duration;

use crate::engine::{self, Clock, Event, Judge};
use crate::inuse::System;
use crate::paths::{self, Layout};
use crate::plan::{Env, Facts, TaskFact, observe};
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
    start_with(layout, facts, None, say)
}

/// Start the runner with a judge for `Judgment` conditions.
#[must_use]
pub fn start_with(
    layout: Layout,
    facts: Option<Arc<dyn Facts>>,
    judge: Option<Arc<dyn Judge>>,
    say: Say,
) -> Handle {
    let (sender, receiver) = channel();
    let _ = std::thread::Builder::new()
        .name("background".into())
        .spawn(move || Runner::new(layout, facts, judge, say).serve(&receiver));
    Handle { sender }
}

struct Runner {
    layout: Layout,
    facts: Option<Arc<dyn Facts>>,
    judge: Option<Arc<dyn Judge>>,
    say: Say,
    /// The tasks seen ended at the last look; `None` before the first.
    ended: Option<BTreeSet<String>>,
    /// Each rule's next interval check.
    next: BTreeMap<String, u64>,
}

/// How often the runner looks for ended tasks, watched files, and daily
/// times.
const POLL: Duration = Duration::from_secs(30);
/// How long after the host starts it makes the first check.
const START_DELAY: Duration = Duration::from_secs(20);

impl Runner {
    fn new(
        layout: Layout,
        facts: Option<Arc<dyn Facts>>,
        judge: Option<Arc<dyn Judge>>,
        say: Say,
    ) -> Self {
        Self {
            layout,
            facts,
            judge,
            say,
            ended: None,
            next: BTreeMap::new(),
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
        for rule in self.rules() {
            State::update(&self.layout, &rule.id, |state| state.runner = Some(pid));
        }
        std::thread::sleep(START_DELAY);
        self.check_all(Cause::HostStart, &Event::default());
        // Baselines: the first look at tasks and files fires nothing.
        let _ = self.newly_ended();
        let _ = engine::poll_files(&self.layout, &self.rules());
        let mut next_poll = paths::now() + POLL.as_secs();
        loop {
            // Each rule's schedule is read again after every wake, so an
            // edit takes effect without a restart.
            let rules = self.rules();
            let now = paths::now();
            self.schedule(&rules, now);
            let deadline = self
                .next
                .values()
                .copied()
                .min()
                .map_or(next_poll, |at| at.min(next_poll));
            let wait = deadline.saturating_sub(now).max(1);
            match requests.recv_timeout(Duration::from_secs(wait)) {
                Ok(Request::Run { rule }) => self.manual(&rule),
                Ok(Request::TaskEnded) => self.tasks_ended(),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            let now = paths::now();
            if now >= next_poll {
                next_poll = now + POLL.as_secs();
                self.tasks_ended();
                self.files();
                self.daily(Clock::here());
            }
            let rules = self.rules();
            let due: Vec<Rule> = rules
                .into_iter()
                .filter(|rule| self.next.get(&rule.id).is_some_and(|at| now >= *at))
                .collect();
            for rule in due {
                if let Some(every) = rule.interval() {
                    self.next
                        .insert(rule.id.clone(), engine::next_interval(&rule.id, every, now));
                }
                self.check_one(&rule, Cause::Interval, &Event::default());
            }
        }
    }

    /// Keep one interval schedule per rule that has an `Interval`
    /// trigger, and none for the rest.
    fn schedule(&mut self, rules: &[Rule], now: u64) {
        self.next.retain(|id, _| {
            rules
                .iter()
                .any(|rule| rule.id == *id && rule.interval().is_some())
        });
        for rule in rules {
            if let Some(every) = rule.interval() {
                let at = self
                    .next
                    .entry(rule.id.clone())
                    .or_insert_with(|| engine::next_interval(&rule.id, every, now));
                // A shorter interval after an edit takes effect now.
                *at = (*at).min(now + every + every / 10);
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

    /// The rules this computer runs: the built-in one unless it is off,
    /// each enabled plugin's, and each made in conversation that is on.
    /// Read afresh on every check, so turning a plugin on or off takes
    /// effect at the next one.
    fn rules(&self) -> Vec<Rule> {
        store::list(&self.layout)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|rule| rule.enabled)
            .collect()
    }

    /// The tasks that ended since the last look.
    fn newly_ended(&mut self) -> Vec<TaskFact> {
        let Some(facts) = &self.facts else {
            return Vec::new();
        };
        let Ok(tasks) = facts.tasks() else {
            return Vec::new();
        };
        let ended: Vec<TaskFact> = tasks.into_iter().filter(|task| task.ended).collect();
        let ids: BTreeSet<String> = ended.iter().map(|task| task.id.clone()).collect();
        let new = match &self.ended {
            Some(before) => ended
                .into_iter()
                .filter(|task| !before.contains(&task.id))
                .collect(),
            None => Vec::new(),
        };
        self.ended = Some(ids);
        new
    }

    /// Each newly ended task is one `TaskEnded` evaluation for the rules
    /// that read its outcome, and one check for the rest.
    fn tasks_ended(&mut self) {
        let ended = self.newly_ended();
        if ended.is_empty() {
            return;
        }
        for rule in self.rules() {
            if engine::per_task(&rule) {
                for task in &ended {
                    let event = Event {
                        task: Some(task.clone()),
                        paths: Vec::new(),
                    };
                    self.check_one(&rule, Cause::TaskEnded, &event);
                }
            } else {
                let event = Event {
                    task: ended.last().cloned(),
                    paths: Vec::new(),
                };
                self.check_one(&rule, Cause::TaskEnded, &event);
            }
        }
    }

    fn files(&mut self) {
        let rules = self.rules();
        for (id, paths) in engine::poll_files(&self.layout, &rules) {
            if let Some(rule) = rules.iter().find(|rule| rule.id == id) {
                let event = Event { task: None, paths };
                self.check_one(rule, Cause::FsEvent, &event);
            }
        }
    }

    fn daily(&mut self, clock: Clock) {
        let state = State::load(&self.layout);
        for rule in self.rules() {
            let times: Vec<&String> = rule
                .triggers
                .iter()
                .filter_map(|trigger| match trigger {
                    Trigger::Daily { at } => Some(at),
                    _ => None,
                })
                .collect();
            if times.is_empty() {
                continue;
            }
            let last = state.rules.get(&rule.id).and_then(|s| s.last_daily);
            if last.is_none() {
                engine::mark_daily(&self.layout, &rule.id, clock.now);
                continue;
            }
            if times.iter().any(|at| engine::daily_due(at, last, clock)) {
                engine::mark_daily(&self.layout, &rule.id, clock.now);
                self.check_one(&rule, Cause::Daily, &Event::default());
            }
        }
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
        let result = if rule.cleans() && rule.conditions.is_empty() {
            run::run(&self.env(), &rule, Cause::Manual, false, true)
        } else {
            engine::evaluate(
                &self.env(),
                &rule,
                Cause::Manual,
                &Event::default(),
                Clock::here(),
                self.judge.as_deref(),
                false,
            )
            .map(|report| report.unwrap_or_else(engine::nothing))
        };
        self.finish(&rule, result);
    }

    fn check_all(&self, cause: Cause, event: &Event) {
        for rule in self.rules() {
            self.check_one(&rule, cause, event);
        }
    }

    fn check_one(&self, rule: &Rule, cause: Cause, event: &Event) {
        let result = check_with(
            &self.env(),
            rule,
            cause,
            event,
            Clock::here(),
            self.judge.as_deref(),
        );
        if let Some(result) = result {
            self.finish(rule, result);
        }
    }

    fn finish(&self, rule: &Rule, result: Result<Report, String>) {
        let id = rule.id.as_str();
        let report = match result {
            Ok(report) => report,
            Err(why) => {
                (self.say)(&format!("background {id}: {why}"));
                return;
            }
        };
        crate::view::remember(&self.layout, id, &report);
        // A plugin's rule notifies only when it asked to; its result is
        // still recorded and listed.
        if matches!(rule.origin, crate::rule::Origin::Plugin { .. }) && !rule.needs.notify {
            State::update(&self.layout, id, |state| state.notice = None);
            return;
        }
        let notice = report.notice.clone();
        if let Some(line) = notice {
            (self.say)(&line);
        }
    }
}

/// One automatic evaluation of any rule: nothing unless the rule names
/// the trigger ([`fires`]). A disk cleanup rule with no conditions is
/// [`check`]; any other rule acts when its conditions hold and its
/// cooldown has passed ([`engine::evaluate`]).
pub fn check_with(
    env: &Env<'_>,
    rule: &Rule,
    cause: Cause,
    event: &Event,
    clock: Clock,
    judge: Option<&dyn Judge>,
) -> Option<Result<Report, String>> {
    if rule.cleans() && rule.conditions.is_empty() {
        return check(env, rule, cause);
    }
    if cause == Cause::Manual || !fires(rule, cause) || !rule.active(env.now) {
        return None;
    }
    let last_run = State::load(env.layout)
        .rules
        .get(&rule.id)
        .and_then(|state| state.last_run);
    if last_run.is_some_and(|last| env.now < last + rule.cooldown_secs) {
        return None;
    }
    State::update(env.layout, &rule.id, |state| {
        state.last_check = Some(env.now);
        state.next_check = rule.interval().map(|every| env.now + every);
    });
    if rule.cleans() {
        // Conditions first, then the usual low-space check.
        engine::holds(env, rule, event, clock, judge).ok()?;
        return check(env, rule, cause);
    }
    engine::evaluate(env, rule, cause, event, clock, judge, false).transpose()
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
        Cause::Daily => rule
            .triggers
            .iter()
            .any(|trigger| matches!(trigger, Trigger::Daily { .. })),
        Cause::FsEvent => rule
            .triggers
            .iter()
            .any(|trigger| matches!(trigger, Trigger::FsEvent { .. })),
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
