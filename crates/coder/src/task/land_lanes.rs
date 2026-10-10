//! The landing queue's lanes (#11248): one worker, several entries at once.
//!
//! The integrator used to land one entry at a time, so a docs fix waited
//! behind every Rust change ahead of it, each a half-hour check run. The
//! worker now plans each open entry ([`super::land_plan`]) and runs:
//!
//! - **the fast lane**: one slot for entries with no compiled code. They
//!   skip the build ([`super::land_queue::DocChecks`]), rebase, and push in
//!   seconds, never behind a code entry;
//! - **code slots** (two by default): entries whose packages do not
//!   [`overlap`] check side by side, each in its own worktree and build slot;
//! - **strict order where it matters**: a code entry waits while an
//!   earlier open entry overlaps it, so overlapping ones land one at a
//!   time in submission order.
//!
//! Pushing stays serialized: every slot takes one [`PushLock`] for fetch →
//! rebase → push only. When `main` moved during a slot's checks, the
//! landing rebases and re-runs them only when the new commits touch the
//! entry's packages or what they depend on ([`super::landing::affects`]);
//! otherwise it pushes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::issue_run::Checks;
use super::land_plan::{self, Lane, Plan, overlap};
use super::land_queue::{
    Busy, DocChecks, Effects, Entry, Heartbeat, Instance, Integrator, Outcome, PushLock, Queue,
    State, Store, ensure_worktree, fetch_entry, now,
};
use super::landing;

/// Plans an entry before it is scheduled.
pub trait Planner: Sync {
    fn plan(&self, entry: &Entry) -> Plan;
}

/// Plans entries in a worktree of its own: the branch checked out, its
/// changed paths classified against the workspace ([`land_plan::plan`]).
pub struct RepoPlanner {
    pub top: PathBuf,
    pub worktree: PathBuf,
}

impl Planner for RepoPlanner {
    fn plan(&self, entry: &Entry) -> Plan {
        let w = &self.worktree;
        let run = || -> Result<Plan, String> {
            ensure_worktree(&self.top, w)?;
            fetch_entry(w, entry)?;
            let git =
                |args: &[&str]| super::local::git_out(w, args).map(|out| out.trim().to_owned());
            git(&[
                "checkout",
                "-q",
                "-f",
                "--detach",
                &format!("origin/{}", entry.branch),
            ])?;
            let _ = git(&["clean", "-q", "-fd"]);
            let base = git(&["merge-base", "HEAD", &format!("origin/{}", entry.target)])?;
            Ok(land_plan::plan(w, &base))
        };
        run().unwrap_or_else(|why| Plan::unknown(&why))
    }
}

/// One worker's lanes over the queue.
pub struct Lanes<'a> {
    pub store: &'a dyn Store,
    /// The checkout whose `origin` the branches are on.
    pub top: PathBuf,
    /// Where the slots' worktrees live (`work-fast`, `work-code-1`, ...).
    pub root: PathBuf,
    pub machine: String,
    /// How many code entries check at once.
    pub code_slots: usize,
    /// The code lane's checks (the issue flow's gate).
    pub checks: &'a dyn Checks,
    pub planner: &'a dyn Planner,
    /// A new set of effects for each landing.
    pub effects: &'a (dyn Fn() -> Box<dyn Effects> + Sync),
    pub attempts: u32,
    pub backoff: landing::Backoff,
    pub instance: Option<Instance>,
    /// Write generated files again in the code lane.
    pub regenerate: bool,
    /// How long to wait between looks at an idle queue.
    pub every: Duration,
}

/// How a run of the lanes is steered from outside.
pub struct Control<'a> {
    /// Return once nothing runs and nothing can start (tests, `--once`).
    pub once: bool,
    /// While true, take nothing new and finish what runs.
    pub draining: &'a dyn Fn() -> bool,
    /// Told whether any slot is busy, for the idle stop's marker.
    pub busy: &'a dyn Fn(bool),
    /// Told each landing's end.
    pub done: &'a mut dyn FnMut(&Entry, &Outcome),
    /// Told the worker cannot run checks now (its reason); `None` when it
    /// can. While `Some`, nothing new starts.
    pub ready: &'a dyn Fn() -> Option<String>,
}

struct Running {
    id: String,
    lane: Lane,
    since: u64,
}

type Finished = (usize, Result<(Entry, Outcome), String>);

impl Lanes<'_> {
    fn slot_name(index: usize) -> String {
        if index == 0 {
            "fast".to_owned()
        } else {
            format!("code-{index}")
        }
    }

    /// Runs the lanes until `control.once` finds nothing left to do (never,
    /// otherwise).
    ///
    /// # Errors
    /// Only when `control.once` and the queue cannot be read.
    pub fn run(&self, control: &mut Control<'_>) -> Result<(), String> {
        let queue = Queue { store: self.store };
        let push = PushLock::default();
        let docs = DocChecks;
        let (tx, rx) = mpsc::channel::<Finished>();
        let mut slots: Vec<Option<Running>> = (0..=self.code_slots).map(|_| None).collect();
        let mut plans: HashMap<String, Plan> = HashMap::new();
        let mut known: HashMap<String, Entry> = HashMap::new();
        let mut finished: Vec<Finished> = Vec::new();
        let mut last_beat: Option<(Instant, Vec<Busy>, bool)> = None;
        std::thread::scope(|scope| -> Result<(), String> {
            loop {
                while let Ok(done) = rx.try_recv() {
                    finished.push(done);
                }
                for (index, result) in finished.drain(..) {
                    slots[index] = None;
                    match result {
                        Ok((entry, outcome)) => {
                            if entry.state != State::Queued {
                                plans.remove(&entry.id);
                            }
                            (control.done)(&entry, &outcome);
                        }
                        Err(why) => eprintln!("land: slot {}: {why}", Self::slot_name(index)),
                    }
                }
                let draining = (control.draining)();
                let unready = (control.ready)();
                let entries = match queue.entries_cached(&mut known) {
                    Ok(entries) => entries,
                    Err(why) => {
                        if control.once && slots.iter().all(Option::is_none) {
                            return Err(why);
                        }
                        eprintln!("land: the queue could not be read: {why}");
                        std::thread::sleep(self.every);
                        continue;
                    }
                };
                let running: Vec<String> = slots.iter().flatten().map(|r| r.id.clone()).collect();
                // Open entries this worker can take or has, oldest first.
                let open: Vec<Entry> = entries
                    .into_iter()
                    .filter(|e| {
                        e.state == State::Queued
                            || (e.state == State::Landing
                                && e.worker.as_deref() == Some(self.machine.as_str()))
                    })
                    .collect();
                let mut started = false;
                if !draining && unready.is_none() {
                    for (k, entry) in open.iter().enumerate() {
                        if running.contains(&entry.id) {
                            continue;
                        }
                        if !plans.contains_key(&entry.id) {
                            let plan = self.planner.plan(entry);
                            if entry.lane != Some(plan.lane) {
                                let mut noted = entry.clone();
                                noted.lane = Some(plan.lane);
                                let _ = queue.put(&noted);
                            }
                            plans.insert(entry.id.clone(), plan);
                        }
                        let plan = plans[&entry.id].clone();
                        let index = match plan.lane {
                            Lane::Fast => (slots[0].is_none()).then_some(0),
                            Lane::Code => {
                                let blocker = open[..k].iter().find(|earlier| {
                                    plans.get(&earlier.id).is_none_or(|p| overlap(p, &plan))
                                });
                                if let Some(blocker) = blocker {
                                    if entry.waiting_for.as_deref() != Some(blocker.id.as_str()) {
                                        let mut noted = entry.clone();
                                        noted.waiting_for = Some(blocker.id.clone());
                                        noted.lane = Some(plan.lane);
                                        let _ = queue.put(&noted);
                                    }
                                    continue;
                                }
                                (1..slots.len()).find(|&i| slots[i].is_none())
                            }
                        };
                        let Some(index) = index else {
                            continue;
                        };
                        started = true;
                        slots[index] = Some(Running {
                            id: entry.id.clone(),
                            lane: plan.lane,
                            since: now(),
                        });
                        let entry = entry.clone();
                        let tx = tx.clone();
                        let push = push.clone();
                        let docs = &docs;
                        scope.spawn(move || {
                            let result =
                                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                    self.land(index, entry, &plan, push, docs)
                                }))
                                .unwrap_or_else(|_| Err("the landing panicked".to_owned()));
                            let _ = tx.send((index, result));
                        });
                    }
                }
                if let Some(why) = &unready
                    && last_beat.is_none()
                {
                    eprintln!("land: taking no entries: {why}");
                }
                let busy: Vec<Busy> = slots
                    .iter()
                    .enumerate()
                    .filter_map(|(i, r)| {
                        r.as_ref().map(|r| Busy {
                            slot: Self::slot_name(i),
                            lane: r.lane.word().to_owned(),
                            entry: r.id.clone(),
                            since: r.since,
                        })
                    })
                    .collect();
                (control.busy)(!busy.is_empty());
                let due = last_beat.as_ref().is_none_or(|(at, was, drained)| {
                    at.elapsed() >= Duration::from_secs(15) || *was != busy || *drained != draining
                });
                if due {
                    let beat = Heartbeat {
                        machine: self.machine.clone(),
                        at: now(),
                        current: busy.first().map(|b| b.entry.clone()),
                        instance: self.instance.clone(),
                        slots: busy.clone(),
                        capacity: Some(self.code_slots as u32),
                        draining,
                    };
                    if let Err(why) = queue.beat(&beat) {
                        eprintln!("land: the heartbeat was not written: {why}");
                    }
                    last_beat = Some((Instant::now(), busy.clone(), draining));
                }
                if busy.is_empty() && !started && control.once {
                    return Ok(());
                }
                // Wake at once when a slot finishes; otherwise look again.
                let wait = if busy.is_empty() {
                    self.every
                } else {
                    self.every.min(Duration::from_secs(5))
                };
                if !started && let Ok(done) = rx.recv_timeout(wait) {
                    finished.push(done);
                }
            }
        })
    }

    fn land(
        &self,
        index: usize,
        entry: Entry,
        plan: &Plan,
        push: PushLock,
        docs: &DocChecks,
    ) -> Result<(Entry, Outcome), String> {
        let mut effects = (self.effects)();
        let slot = Self::slot_name(index);
        let fast = plan.lane == Lane::Fast;
        let mut integrator = Integrator {
            queue: Queue { store: self.store },
            top: self.top.clone(),
            worktree: self.root.join(format!("work-{slot}")),
            machine: self.machine.clone(),
            checks: if fast { docs } else { self.checks },
            effects: effects.as_mut(),
            attempts: self.attempts,
            backoff: self.backoff,
            lane: plan.lane,
            slot,
            also: plan.also.clone(),
            push: Some(push),
            regenerate: self.regenerate && !fast,
        };
        integrator.run(entry)
    }
}

/// The folder the lanes keep their worktrees in, beside the busy marker.
#[must_use]
pub fn root_beside(marker: &Path) -> PathBuf {
    marker
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

#[cfg(test)]
#[path = "land_lanes_tests.rs"]
mod tests;
