//! The deterministic simulation: two refill policies, one clock.
//!
//! `run` drives a catalog through a discrete-event clock in stated
//! ticks and measures what the two supervision styles do with it:
//!
//! - [`Fill::Waves`] — a wave of admissions runs to *its* end; freed
//!   slots wait for the slowest task in the wave before the next plan.
//!   This is the shape of a fan-out that returns the batch only when
//!   every task has settled.
//! - [`Fill::Refill`] — every completion replans immediately, so a freed
//!   slot takes the next ready task while its wave-mates still run.
//!
//! Both policies schedule through the real [`crate::plan::select`], so
//! the simulation exercises the same dependencies, conflicts, quiet-host
//! lane, and resource bounds a live run obeys. Review is not modeled —
//! an attempt that completes here is instantly `completed` — because the
//! question this answers is how dispatch order moves the makespan, not
//! how fast a reviewer reads.
//!
//! The numbers are simulated ticks over pinned estimates. They compare
//! the two policies against each other on the same catalog and say
//! nothing about live wall-clock speed — no live-speedup claim is made
//! or licensed by this output.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::catalog::Catalog;
use crate::plan::{self, Input, Policy, Status};
use crate::resources::{Capacity, InUse};

/// How freed slots are refilled.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Fill {
    /// Admit a wave, run it to its slowest task, replan.
    Waves,
    /// Replan at every completion.
    Refill,
}

impl Fill {
    /// The policy's name in reports.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Waves => "waves",
            Self::Refill => "refill",
        }
    }
}

/// One task's simulated timeline.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Completion {
    /// The task's stable id.
    pub task: String,
    /// The tick the task became ready — its last dependency's
    /// completion, or 0 when it had none.
    pub ready_tick: u64,
    /// The tick the task was admitted.
    pub start_tick: u64,
    /// The tick the task finished.
    pub end_tick: u64,
    /// `start - ready`: how long the task waited, ready, for a slot.
    pub queue_wait_ticks: u64,
}

/// The busiest the host's declared resources became.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Peak {
    /// The most executor places held at once.
    pub executor_slots: u32,
    /// The most CPU units held at once.
    pub cpu_units: u32,
    /// The most memory reserved at once, in MiB.
    pub memory_mib: u64,
    /// Whether the quiet-host lane was ever held.
    pub quiet_host_held: bool,
}

/// What one simulated run produced.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Report {
    /// The refill policy.
    pub policy: Fill,
    /// The executor-slot capacity the run used.
    pub capacity_executor_slots: u32,
    /// The last completion tick — 0 when nothing scheduled.
    pub makespan_ticks: u64,
    /// How many waves the run took (one per plan round).
    pub rounds: u32,
    /// Tasks that ran.
    pub scheduled: u32,
    /// Tasks that never ran, with the plan's last stated reasons.
    pub unscheduled: BTreeMap<String, Vec<String>>,
    /// Every completion, in finish order.
    pub completions: Vec<Completion>,
    /// Mean queue wait over the scheduled tasks, in ticks.
    pub mean_queue_wait_ticks: f64,
    /// The worst queue wait, in ticks.
    pub max_queue_wait_ticks: u64,
    /// The busiest the host became.
    pub peak: Peak,
}

/// Run one simulation.
///
/// `states` seeds durable state — externally completed dependencies and
/// already-finished tasks — the same way a live plan reads the ledger.
#[must_use]
pub fn run(
    catalog: &Catalog,
    capacity: &Capacity,
    fill: Fill,
    externally_completed: &BTreeSet<String>,
) -> Report {
    let policy = Policy {
        // Review is not modeled; keep the intake open.
        review_cap: u32::MAX,
    };
    let mut time: u64 = 0;
    let mut states: BTreeMap<String, Status> = BTreeMap::new();
    let mut running: BTreeMap<String, u64> = BTreeMap::new();
    let mut finished: BTreeMap<String, u64> = BTreeMap::new();
    let mut completions: Vec<Completion> = Vec::new();
    let mut ready_at: BTreeMap<String, u64> = BTreeMap::new();
    let mut peak = Peak::default();
    let mut rounds: u32 = 0;
    let mut last_blocked: BTreeMap<String, Vec<String>> = BTreeMap::new();

    loop {
        // Plan against the states as they stand.
        let input = Input {
            catalog,
            capacity,
            states: &states,
            externally_completed,
            exclusions: &[],
            policy: &policy,
        };
        let plan = plan::select(&input);
        rounds += 1;
        for blocked in &plan.blocked {
            last_blocked.insert(
                blocked.task.clone(),
                blocked
                    .reasons
                    .iter()
                    .map(|reason| reason.to_string())
                    .collect(),
            );
        }

        // Admit: each task's queue wait runs from when its dependencies
        // completed — a task may have become ready before this round.
        let mut used = InUse::default();
        for task in running.keys() {
            if let Some(task) = catalog.task(task) {
                used.add(&task.resources);
            }
        }
        for admission in &plan.admit {
            let task = catalog
                .task(&admission.task)
                .expect("the plan names a task");
            let ready = ready_at.get(&task.id).copied().unwrap_or_else(|| {
                let ready = task
                    .depends_on
                    .iter()
                    .filter_map(|dependency| finished.get(dependency))
                    .max()
                    .copied()
                    .unwrap_or(0);
                ready_at.insert(task.id.clone(), ready);
                ready
            });
            running.insert(task.id.clone(), time + task.estimate_ticks.max(1));
            states.insert(task.id.clone(), Status::Active);
            used.add(&task.resources);
            peak.executor_slots = peak.executor_slots.max(used.executor_slots);
            peak.cpu_units = peak.cpu_units.max(used.cpu_units);
            peak.memory_mib = peak.memory_mib.max(used.memory_mib);
            peak.quiet_host_held |= used.quiet;
            completions.push(Completion {
                task: task.id.clone(),
                ready_tick: ready,
                start_tick: time,
                end_tick: time + task.estimate_ticks.max(1),
                queue_wait_ticks: time - ready,
            });
        }

        if running.is_empty() {
            // Nothing in flight: either the backlog is done or the rest
            // is permanently blocked — the last plan's reasons say why.
            break;
        }

        // Advance the clock. Waves run the whole round to its slowest
        // task; refill stops at the next completion.
        let horizon = match fill {
            Fill::Waves => running.values().max().copied().unwrap_or(time),
            Fill::Refill => running.values().min().copied().unwrap_or(time),
        };
        time = time.max(horizon);
        let due: Vec<String> = running
            .iter()
            .filter(|(_, end)| **end <= time)
            .map(|(id, _)| id.clone())
            .collect();
        for id in due {
            running.remove(&id);
            finished.insert(id.clone(), time);
            states.insert(id, Status::Completed);
        }
        // A task whose dependencies just completed became ready now.
        for task in &catalog.tasks {
            if ready_at.contains_key(&task.id)
                || states.get(&task.id).copied().unwrap_or_default() != Status::Queued
            {
                continue;
            }
            if !task.depends_on.is_empty()
                && task
                    .depends_on
                    .iter()
                    .all(|dependency| finished.contains_key(dependency))
            {
                ready_at.insert(
                    task.id.clone(),
                    task.depends_on
                        .iter()
                        .filter_map(|dependency| finished.get(dependency))
                        .max()
                        .copied()
                        .unwrap_or(time),
                );
            }
        }
    }

    let scheduled = completions.len() as u32;
    let waits: Vec<u64> = completions
        .iter()
        .map(|completion| completion.queue_wait_ticks)
        .collect();
    let mean = if waits.is_empty() {
        0.0
    } else {
        waits.iter().sum::<u64>() as f64 / waits.len() as f64
    };
    let mut unscheduled = BTreeMap::new();
    for task in &catalog.tasks {
        if states.get(&task.id).copied().unwrap_or_default() == Status::Queued {
            unscheduled.insert(
                task.id.clone(),
                last_blocked.get(&task.id).cloned().unwrap_or_default(),
            );
        }
    }
    Report {
        policy: fill,
        capacity_executor_slots: capacity.executor_slots,
        makespan_ticks: completions
            .iter()
            .map(|completion| completion.end_tick)
            .max()
            .unwrap_or(0),
        rounds,
        scheduled,
        unscheduled,
        completions,
        mean_queue_wait_ticks: mean,
        max_queue_wait_ticks: waits.iter().copied().max().unwrap_or(0),
        peak,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{Catalog, Footprint, Task};
    use crate::resources::Resources;

    fn task(id: &str, ticks: u64) -> Task {
        Task {
            id: id.to_string(),
            issue: 1,
            base: "sha256:base".to_string(),
            input: format!("sha256:input-{id}"),
            depends_on: vec![],
            footprint: Footprint::Declared {
                reads: vec![],
                writes: vec![format!("crates/{id}/src/lib.rs")],
            },
            priority: 0,
            resources: Resources::default(),
            estimate_ticks: ticks,
        }
    }

    fn catalog(tasks: Vec<Task>) -> Catalog {
        let mut catalog = Catalog::new("t", tasks);
        catalog.seal().unwrap();
        catalog
    }

    fn host(slots: u32) -> Capacity {
        Capacity {
            executor_slots: slots,
            cpu_units: 64,
            memory_mib: 65536,
            integration_lanes: 1,
        }
    }

    #[test]
    fn one_slot_serializes_everything() {
        let catalog = catalog(vec![task("a", 3), task("b", 2), task("c", 4)]);
        for fill in [Fill::Waves, Fill::Refill] {
            let report = run(&catalog, &host(1), fill, &BTreeSet::new());
            assert_eq!(report.makespan_ticks, 9);
            assert_eq!(report.scheduled, 3);
        }
    }

    #[test]
    fn refill_never_loses_to_waves() {
        // Mixed durations: waves strand a freed slot behind the wave's
        // slowest task, refill does not. `slow` leads the stated order
        // so it shares the first wave.
        let mut slow = task("slow", 10);
        slow.issue = 0;
        let catalog = catalog(vec![slow, task("f1", 1), task("f2", 1), task("f3", 1)]);
        let waves = run(&catalog, &host(2), Fill::Waves, &BTreeSet::new());
        let refill = run(&catalog, &host(2), Fill::Refill, &BTreeSet::new());
        assert!(refill.makespan_ticks <= waves.makespan_ticks);
        assert_eq!(waves.makespan_ticks, 11, "wave two waits for `slow`");
        assert_eq!(refill.makespan_ticks, 10, "refill packs behind `slow`");
    }

    #[test]
    fn dependencies_order_the_simulation() {
        let mut b = task("b", 2);
        b.depends_on = vec!["a".to_string()];
        let catalog = catalog(vec![task("a", 3), b]);
        let report = run(&catalog, &host(4), Fill::Refill, &BTreeSet::new());
        assert_eq!(report.makespan_ticks, 5);
        let b = report
            .completions
            .iter()
            .find(|completion| completion.task == "b")
            .unwrap();
        assert_eq!(b.ready_tick, 3);
        assert_eq!(b.queue_wait_ticks, 0);
    }

    #[test]
    fn an_absent_dependency_never_schedules() {
        let mut a = task("a", 1);
        a.depends_on = vec!["ghost".to_string()];
        let catalog = catalog(vec![a, task("b", 1)]);
        let report = run(&catalog, &host(4), Fill::Refill, &BTreeSet::new());
        assert_eq!(report.scheduled, 1);
        assert!(report.unscheduled.contains_key("a"));
        assert!(
            report.unscheduled["a"]
                .iter()
                .any(|reason| reason.contains("unknown"))
        );
    }

    #[test]
    fn quiet_work_runs_alone() {
        let mut quiet = task("quiet", 2);
        quiet.resources.quiet_host = true;
        quiet.issue = 0;
        let catalog = catalog(vec![task("a", 5), task("b", 5), quiet]);
        let report = run(&catalog, &host(3), Fill::Refill, &BTreeSet::new());
        // The quiet task drains the host, then holds it alone.
        assert!(report.peak.quiet_host_held);
        let quiet = report
            .completions
            .iter()
            .find(|completion| completion.task == "quiet")
            .unwrap();
        let others: Vec<&Completion> = report
            .completions
            .iter()
            .filter(|completion| completion.task != "quiet")
            .collect();
        for other in others {
            assert!(
                other.end_tick <= quiet.start_tick || other.start_tick >= quiet.end_tick,
                "no task overlaps the quiet lane"
            );
        }
    }

    #[test]
    fn the_simulation_is_deterministic() {
        let mut chain = task("tail", 4);
        chain.depends_on = vec!["head".to_string()];
        let catalog = catalog(vec![task("head", 2), task("mid", 3), chain]);
        let first = run(&catalog, &host(2), Fill::Refill, &BTreeSet::new());
        let second = run(&catalog, &host(2), Fill::Refill, &BTreeSet::new());
        let first = serde_json::to_string(&first).unwrap();
        let second = serde_json::to_string(&second).unwrap();
        assert_eq!(first, second);
    }
}
